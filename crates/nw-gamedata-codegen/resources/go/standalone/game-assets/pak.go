package gameassets

import (
	"archive/zip"
	"bytes"
	"compress/zlib"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"path/filepath"
	"sort"
	"strings"
)

const (
	pakCompressionStored   uint16 = 0
	pakCompressionDeflated uint16 = 8
	pakCompressionOodle    uint16 = 15

	azcsZlib uint32 = 0x73887d3a
	azcsZstd uint32 = 0x72fd505e
)

type pakEntryInfo struct {
	Name              string
	Index             int
	CompressedSize    uint64
	UncompressedSize  uint64
	CompressionMethod uint16
}

type pakArchive struct {
	Path string

	reader  *zip.ReadCloser
	entries []pakEntryInfo
	byName  map[string]int
}

type AssetLoader struct {
	Catalog       AssetCatalog
	mountedArchives []mountedPakArchive
	entriesByPath   map[string]pakEntryRef
}

type unsupportedPakCompressionError struct {
	EntryName string
	Method    uint16
}

type mountedPakArchive struct {
	mountRoot string
	archive   *pakArchive
}

type pakEntryRef struct {
	archive *pakArchive
	entry   pakEntryInfo
}

func (err unsupportedPakCompressionError) Error() string {
	return fmt.Sprintf("pak entry %s uses unsupported compression method %d", err.EntryName, err.Method)
}

func openPakArchive(path string) (*pakArchive, error) {
	resolvedPath, err := canonicalPath(path)
	if err != nil {
		return nil, fmt.Errorf("resolve pak path %s: %w", path, err)
	}

	reader, err := zip.OpenReader(resolvedPath)
	if err != nil {
		return nil, err
	}

	entries := make([]pakEntryInfo, 0, len(reader.File))
	byName := make(map[string]int, len(reader.File))
	for index, file := range reader.File {
		lookupName := NormalizeArchivePath(file.Name)
		entries = append(entries, pakEntryInfo{
			Name:              file.Name,
			Index:             index,
			CompressedSize:    file.CompressedSize64,
			UncompressedSize:  file.UncompressedSize64,
			CompressionMethod: file.Method,
		})
		if _, exists := byName[lookupName]; !exists {
			byName[lookupName] = index
		}
	}

	return &pakArchive{
		Path:    resolvedPath,
		reader:  reader,
		entries: entries,
		byName:  byName,
	}, nil
}

func (archive *pakArchive) Close() error {
	return archive.reader.Close()
}

func (archive *pakArchive) Entries() []pakEntryInfo {
	entries := make([]pakEntryInfo, len(archive.entries))
	copy(entries, archive.entries)
	return entries
}

func (archive *pakArchive) Entry(name string) (pakEntryInfo, bool) {
	index, ok := archive.byName[NormalizeArchivePath(name)]
	if !ok {
		return pakEntryInfo{}, false
	}
	return archive.entries[index], true
}

func (archive *pakArchive) Read(name string) ([]byte, error) {
	entry, ok := archive.Entry(name)
	if !ok {
		return nil, fmt.Errorf("pak entry not found in %s: %s", archive.Path, name)
	}
	return archive.ReadEntry(entry)
}

func (archive *pakArchive) ReadEntry(entry pakEntryInfo) ([]byte, error) {
	if entry.Index < 0 || entry.Index >= len(archive.reader.File) {
		return nil, fmt.Errorf("pak entry index %d out of bounds", entry.Index)
	}
	file := archive.reader.File[entry.Index]
	if file.Method == pakCompressionOodle {
		stream, err := file.OpenRaw()
		if err != nil {
			return nil, err
		}
		compressed, err := io.ReadAll(stream)
		if err != nil {
			return nil, err
		}
		bytes, err := decompressOodle(file.Name, compressed, file.UncompressedSize64)
		if err != nil {
			return nil, err
		}
		return peelAzcs(bytes)
	}
	if file.Method != pakCompressionStored && file.Method != pakCompressionDeflated {
		return nil, unsupportedPakCompressionError{EntryName: file.Name, Method: file.Method}
	}
	stream, err := file.Open()
	if err != nil {
		return nil, err
	}

	bytes, err := io.ReadAll(stream)
	closeErr := stream.Close()
	if err != nil {
		return nil, errors.Join(err, closeErr)
	}
	if closeErr != nil {
		return nil, closeErr
	}
	return peelAzcs(bytes)
}

func Open(assetRoot string) (_ *AssetLoader, err error) {
	originalRoot := assetRoot
	assetRoot, err = canonicalPath(assetRoot)
	if err != nil {
		return nil, fmt.Errorf("resolve asset root %s: %w", originalRoot, err)
	}

	pakPaths, err := collectPakPaths(assetRoot)
	if err != nil {
		return nil, err
	}
	if len(pakPaths) == 0 {
		return nil, fmt.Errorf("no .pak files found under %s", assetRoot)
	}

	var mountedArchives []mountedPakArchive
	closeOnError := true
	defer func() {
		if closeOnError {
			if closeErr := closeMountedArchives(mountedArchives); closeErr != nil {
				err = errors.Join(err, closeErr)
			}
		}
	}()

	entriesByPath := make(map[string]pakEntryRef)
	claimedPaths := make(map[string]struct{})

	for _, pakPath := range pakPaths {
		archive, err := openPakArchive(pakPath)
		if err != nil {
			return nil, err
		}
		mountRoot, err := pakMountRoot(assetRoot, pakPath)
		if err != nil {
			if closeErr := archive.Close(); closeErr != nil {
				err = errors.Join(err, closeErr)
			}
			return nil, err
		}
		mountedArchives = append(mountedArchives, mountedPakArchive{
			mountRoot: mountRoot,
			archive:   archive,
		})

		for _, entry := range archive.entries {
			path := NormalizeVirtualPath(mountedEntryPath(mountRoot, entry.Name))
			if _, exists := claimedPaths[path]; exists {
				continue
			}
			claimedPaths[path] = struct{}{}
			entriesByPath[path] = pakEntryRef{archive: archive, entry: entry}
		}
	}

	catalog, err := loadCatalogFromPaks(mountedArchives)
	if err != nil {
		return nil, err
	}

	closeOnError = false
	return &AssetLoader{
		Catalog:         catalog,
		mountedArchives: mountedArchives,
		entriesByPath:   entriesByPath,
	}, nil
}

func (loader *AssetLoader) Close() error {
	return closeMountedArchives(loader.mountedArchives)
}

func (loader *AssetLoader) Read(path string) ([]byte, error) {
	located, ok := loader.entry(path)
	if !ok {
		return nil, fmt.Errorf("asset %s was not present in selected paks", path)
	}
	return located.archive.ReadEntry(located.entry)
}

func (loader *AssetLoader) entry(path string) (pakEntryRef, bool) {
	normalized := NormalizeVirtualPath(path)
	if located, ok := loader.entriesByPath[normalized]; ok {
		return located, true
	}
	suffix := "/" + normalized
	for candidate, located := range loader.entriesByPath {
		if strings.HasSuffix(candidate, suffix) {
			return located, true
		}
	}
	return pakEntryRef{}, false
}

func collectPakPaths(root string) ([]string, error) {
	resolvedRoot, err := canonicalPath(root)
	if err != nil {
		return nil, fmt.Errorf("resolve pak root %s: %w", root, err)
	}

	var paths []string
	err = filepath.WalkDir(resolvedRoot, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return nil
		}
		if entry.Type().IsRegular() && strings.EqualFold(filepath.Ext(path), ".pak") {
			paths = append(paths, path)
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Strings(paths)
	return paths, nil
}

func loadCatalogFromPaks(mountedArchives []mountedPakArchive) (AssetCatalog, error) {
	for _, mounted := range mountedArchives {
		if _, ok := mounted.archive.Entry(AssetCatalogPath); !ok {
			continue
		}
		bytes, err := mounted.archive.Read(AssetCatalogPath)
		if err != nil {
			return AssetCatalog{}, err
		}
		return ParseRascCatalog(bytes)
	}
	return AssetCatalog{}, fmt.Errorf("asset catalog %s was not found in selected paks", AssetCatalogPath)
}

func canonicalPath(path string) (string, error) {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}
	return filepath.EvalSymlinks(absolute)
}

func closeMountedArchives(mountedArchives []mountedPakArchive) error {
	var closeErr error
	for _, mounted := range mountedArchives {
		if err := mounted.archive.Close(); err != nil {
			closeErr = errors.Join(closeErr, err)
		}
	}
	return closeErr
}

func pakMountRoot(assetRoot string, pakPath string) (string, error) {
	relative, err := filepath.Rel(assetRoot, pakPath)
	if err != nil {
		return "", err
	}
	if relativePathEscapesRoot(relative) {
		return "", fmt.Errorf("pak path %s is outside asset root %s", pakPath, assetRoot)
	}
	dir := filepath.Dir(relative)
	if dir == "." {
		return "", nil
	}
	return NormalizeVirtualPath(dir), nil
}

func relativePathEscapesRoot(relative string) bool {
	return relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) || filepath.IsAbs(relative)
}

func mountedEntryPath(mountRoot string, entry string) string {
	entry = strings.TrimLeft(strings.ReplaceAll(entry, "\\", "/"), "/")
	if mountRoot == "" {
		return entry
	}
	if entry == "" {
		return mountRoot
	}
	return mountRoot + "/" + entry
}

func NormalizeArchivePath(path string) string {
	normalized := strings.ToLower(strings.TrimSpace(strings.ReplaceAll(path, "\\", "/")))
	for strings.HasPrefix(normalized, "./") {
		normalized = strings.TrimPrefix(normalized, "./")
	}
	return strings.TrimLeft(normalized, "/")
}

func peelAzcs(data []byte) ([]byte, error) {
	if len(data) < 16 || string(data[:4]) != "AZCS" {
		return data, nil
	}
	compressorID := binaryBigEndianUint32(data[4:8])
	switch compressorID {
	case azcsZlib:
		if len(data) < 20 {
			return nil, errors.New("AZCS zlib payload missing block-size prefix")
		}
		reader, err := zlib.NewReader(bytes.NewReader(data[20:]))
		if err != nil {
			return nil, err
		}
		defer reader.Close()
		return io.ReadAll(reader)
	case azcsZstd:
		return nil, errors.New("AZCS ZSTD payloads are unsupported by this Go package")
	default:
		return nil, fmt.Errorf("unsupported AZCS compressor id %#08x", compressorID)
	}
}

func binaryBigEndianUint32(bytes []byte) uint32 {
	return uint32(bytes[0])<<24 | uint32(bytes[1])<<16 | uint32(bytes[2])<<8 | uint32(bytes[3])
}
