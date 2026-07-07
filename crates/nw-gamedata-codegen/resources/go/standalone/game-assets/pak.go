package gameassets

import (
	"archive/zip"
	"bytes"
	"compress/zlib"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const (
	PakCompressionStored   uint16 = 0
	PakCompressionDeflated uint16 = 8
	PakCompressionOodle    uint16 = 15

	azcsZlib uint32 = 0x73887d3a
	azcsZstd uint32 = 0x72fd505e
)

type PakEntryInfo struct {
	Name              string
	Index             int
	CompressedSize    uint64
	UncompressedSize  uint64
	CompressionMethod uint16
}

type PakArchive struct {
	Path string

	reader  *zip.ReadCloser
	entries []PakEntryInfo
	byName  map[string]int
}

type PakDatasheetSource struct {
	Catalog    AssetCatalog
	Datasheets []DatasheetAsset
	Assets     []BinaryAsset
}

type BinaryAsset struct {
	Path  string
	Bytes []byte
}

type UnsupportedPakCompressionError struct {
	EntryName string
	Method    uint16
}

type mountedPakArchive struct {
	mountRoot string
	archive   *PakArchive
}

type pakEntryRef struct {
	archive *PakArchive
	entry   PakEntryInfo
}

func (err UnsupportedPakCompressionError) Error() string {
	return fmt.Sprintf("pak entry %s uses unsupported compression method %d", err.EntryName, err.Method)
}

func OpenPakArchive(path string) (*PakArchive, error) {
	reader, err := zip.OpenReader(path)
	if err != nil {
		return nil, err
	}

	entries := make([]PakEntryInfo, 0, len(reader.File))
	byName := make(map[string]int, len(reader.File))
	for index, file := range reader.File {
		lookupName := NormalizeArchivePath(file.Name)
		entries = append(entries, PakEntryInfo{
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

	return &PakArchive{
		Path:    path,
		reader:  reader,
		entries: entries,
		byName:  byName,
	}, nil
}

func (archive *PakArchive) Close() error {
	return archive.reader.Close()
}

func (archive *PakArchive) Entries() []PakEntryInfo {
	entries := make([]PakEntryInfo, len(archive.entries))
	copy(entries, archive.entries)
	return entries
}

func (archive *PakArchive) Entry(name string) (PakEntryInfo, bool) {
	index, ok := archive.byName[NormalizeArchivePath(name)]
	if !ok {
		return PakEntryInfo{}, false
	}
	return archive.entries[index], true
}

func (archive *PakArchive) Read(name string) ([]byte, error) {
	entry, ok := archive.Entry(name)
	if !ok {
		return nil, fmt.Errorf("pak entry not found in %s: %s", archive.Path, name)
	}
	return archive.ReadEntry(entry)
}

func (archive *PakArchive) ReadEntry(entry PakEntryInfo) ([]byte, error) {
	if entry.Index < 0 || entry.Index >= len(archive.reader.File) {
		return nil, fmt.Errorf("pak entry index %d out of bounds", entry.Index)
	}
	file := archive.reader.File[entry.Index]
	if file.Method == PakCompressionOodle {
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
	if file.Method != PakCompressionStored && file.Method != PakCompressionDeflated {
		return nil, UnsupportedPakCompressionError{EntryName: file.Name, Method: file.Method}
	}
	stream, err := file.Open()
	if err != nil {
		return nil, err
	}
	defer stream.Close()

	bytes, err := io.ReadAll(stream)
	if err != nil {
		return nil, err
	}
	return peelAzcs(bytes)
}

func LoadPakDatasheetSource(assetRoot string, pakPaths []string) (*PakDatasheetSource, error) {
	if len(pakPaths) == 0 {
		collected, err := CollectPakPaths(assetRoot)
		if err != nil {
			return nil, err
		}
		pakPaths = collected
	} else {
		pakPaths = append([]string(nil), pakPaths...)
		sort.Strings(pakPaths)
	}
	if len(pakPaths) == 0 {
		return nil, fmt.Errorf("no .pak files found under %s", assetRoot)
	}

	var mountedArchives []mountedPakArchive
	defer func() {
		for _, mounted := range mountedArchives {
			_ = mounted.archive.Close()
		}
	}()

	entriesByPath := make(map[string]pakEntryRef)
	claimedPaths := make(map[string]struct{})

	for _, pakPath := range pakPaths {
		archive, err := OpenPakArchive(pakPath)
		if err != nil {
			return nil, err
		}
		mountRoot, err := pakMountRoot(assetRoot, pakPath)
		if err != nil {
			_ = archive.Close()
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

	var datasheets []DatasheetAsset
	var assets []BinaryAsset
	for _, entry := range catalog.Entries {
		if !IsDatasheetPath(entry.RelativePath) && !IsManagerAssetPath(entry.RelativePath) {
			continue
		}
		path := NormalizeVirtualPath(entry.RelativePath)
		located, ok := entriesByPath[path]
		if !ok {
			return nil, fmt.Errorf("catalog asset %s was not present in selected paks", path)
		}
		bytes, err := located.archive.ReadEntry(located.entry)
		if err != nil {
			return nil, err
		}
		if IsDatasheetPath(entry.RelativePath) {
			datasheets = append(datasheets, DatasheetAsset{
				Path:  path,
				Bytes: bytes,
			})
		} else {
			assets = append(assets, BinaryAsset{
				Path:  path,
				Bytes: bytes,
			})
		}
	}

	return &PakDatasheetSource{
		Catalog:    catalog,
		Datasheets: datasheets,
		Assets:     assets,
	}, nil
}

func IsManagerAssetPath(path string) bool {
	normalized := NormalizeVirtualPath(path)
	if normalized == "libs/camera/gamecamera.xml" {
		return true
	}
	switch strings.ToLower(pathExtension(normalized)) {
	case "aoffdb", "equipdb", "gds", "uidb", "pbadb", "sprd", "gdb", "gactdb", "rankdb", "craftstationdb":
		return true
	default:
		return false
	}
}

func LoadRascCatalogFromLooseFile(path string) (AssetCatalog, error) {
	bytes, err := os.ReadFile(path)
	if err != nil {
		return AssetCatalog{}, err
	}
	return ParseRascCatalog(bytes)
}

func CollectPakPaths(root string) ([]string, error) {
	var paths []string
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
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

func pakMountRoot(assetRoot string, pakPath string) (string, error) {
	relative, err := filepath.Rel(assetRoot, pakPath)
	if err != nil {
		return "", err
	}
	dir := filepath.Dir(relative)
	if dir == "." {
		return "", nil
	}
	return NormalizeVirtualPath(dir), nil
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
