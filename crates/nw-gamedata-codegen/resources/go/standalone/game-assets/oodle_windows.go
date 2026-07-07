//go:build windows

package gameassets

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"unsafe"
)

var oodleDLLNames = []string{
	"oo2core_9_win64.dll",
	"oo2core_8_win64.dll",
}

var (
	oodleOnce    sync.Once
	oodleDLL     *syscall.LazyDLL
	oodleProc    *syscall.LazyProc
	oodlePath    string
	oodleLoadErr error
)

func decompressOodle(entryName string, input []byte, outputSize uint64) ([]byte, error) {
	if len(input) == 0 {
		return nil, fmt.Errorf("pak entry %s has empty Oodle payload", entryName)
	}
	if outputSize > uint64(maxInt()) {
		return nil, fmt.Errorf("pak entry %s Oodle output is too large: %d bytes", entryName, outputSize)
	}
	output := make([]byte, int(outputSize))
	if len(output) == 0 {
		return output, nil
	}

	proc, path, err := loadOodle()
	if err != nil {
		return nil, err
	}
	decoded, _, callErr := proc.Call(
		uintptr(unsafe.Pointer(&input[0])),
		uintptr(len(input)),
		uintptr(unsafe.Pointer(&output[0])),
		uintptr(len(output)),
		1,
		0,
		0,
		0,
		0,
		0,
		0,
		0,
		0,
		3,
	)
	if decoded == 0 {
		return nil, fmt.Errorf("Oodle failed to decompress %s with %s: %v", entryName, path, callErr)
	}
	if decoded > uintptr(len(output)) {
		return nil, fmt.Errorf("Oodle decompressed %s past output buffer: %d > %d", entryName, decoded, len(output))
	}
	return output[:int(decoded)], nil
}

func loadOodle() (*syscall.LazyProc, string, error) {
	oodleOnce.Do(func() {
		var lastErr error
		for _, candidate := range oodleCandidatePaths() {
			dll := syscall.NewLazyDLL(candidate)
			proc := dll.NewProc("OodleLZ_Decompress")
			if err := proc.Find(); err != nil {
				lastErr = err
				continue
			}
			oodleDLL = dll
			oodleProc = proc
			oodlePath = candidate
			return
		}
		oodleLoadErr = fmt.Errorf("Oodle library not found. Set OODLE_DLL or OODLE_PATH. Last error: %v", lastErr)
	})
	if oodleLoadErr != nil {
		return nil, "", oodleLoadErr
	}
	return oodleProc, oodlePath, nil
}

func oodleCandidatePaths() []string {
	var candidates []string
	add := func(value string) {
		value = strings.TrimSpace(value)
		if value == "" {
			return
		}
		if strings.EqualFold(filepath.Ext(value), ".dll") {
			candidates = append(candidates, value)
			return
		}
		for _, name := range oodleDLLNames {
			candidates = append(candidates, filepath.Join(value, name))
		}
	}

	add(os.Getenv("OODLE_DLL"))
	for _, value := range packageLocalOodleDirs() {
		add(value)
	}
	for _, value := range filepath.SplitList(os.Getenv("OODLE_PATH")) {
		add(value)
	}
	for _, value := range filepath.SplitList(os.Getenv("PATH")) {
		add(value)
	}
	candidates = append(candidates, oodleDLLNames...)

	seen := make(map[string]struct{}, len(candidates))
	out := make([]string, 0, len(candidates))
	for _, candidate := range candidates {
		key := strings.ToLower(candidate)
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, candidate)
	}
	return out
}

func packageLocalOodleDirs() []string {
	var dirs []string
	addDir := func(path string, ok bool) {
		if !ok || strings.TrimSpace(path) == "" {
			return
		}
		dirs = append(dirs, path, filepath.Join(path, "bin"))
	}

	exe, err := os.Executable()
	addDir(filepath.Dir(exe), err == nil)
	cwd, err := os.Getwd()
	addDir(cwd, err == nil)
	_, source, _, ok := runtime.Caller(0)
	sourceDir := filepath.Dir(source)
	addDir(sourceDir, ok)
	addDir(filepath.Dir(sourceDir), ok)
	return dirs
}

func maxInt() int {
	return int(^uint(0) >> 1)
}
