package gameassets

import (
	"encoding/binary"
	"fmt"
	"path/filepath"
	"strings"

	gametypes "example.com/newworld/gamedata/types"
	"github.com/google/uuid"
)

const AssetCatalogPath = "assetcatalog.catalog"

type AssetCatalogEntry struct {
	AssetID      gametypes.AssetID
	AssetType    gametypes.UUID
	RelativePath string
	SizeBytes    uint32
}

type AssetCatalog struct {
	Version uint32
	Entries []AssetCatalogEntry
}

func IsAssetCatalogPath(path string) bool {
	return strings.EqualFold(filepath.Base(path), AssetCatalogPath)
}

func ParseRascCatalog(bytes []byte) (AssetCatalog, error) {
	if len(bytes) < 40 {
		return AssetCatalog{}, fmt.Errorf("RASC input too small: %d bytes", len(bytes))
	}
	if string(bytes[:4]) != "RASC" {
		return AssetCatalog{}, fmt.Errorf("invalid RASC signature %q", string(bytes[:4]))
	}
	version := binary.LittleEndian.Uint32(bytes[4:8])
	fileSize := binary.LittleEndian.Uint64(bytes[8:16])
	guidOffset := int(binary.LittleEndian.Uint32(bytes[16:20]))
	assetTypeOffset := int(binary.LittleEndian.Uint32(bytes[20:24]))
	dirOffset := int(binary.LittleEndian.Uint32(bytes[24:28]))
	fileNameOffset := int(binary.LittleEndian.Uint32(bytes[28:32]))
	endSentinel := binary.LittleEndian.Uint32(bytes[32:36])
	numEntries := int(binary.LittleEndian.Uint32(bytes[36:40]))
	if uint32(fileSize) != endSentinel {
		return AssetCatalog{}, fmt.Errorf("RASC size sentinel mismatch: %d vs %d", fileSize, endSentinel)
	}

	entries := make([]AssetCatalogEntry, 0, numEntries)
	for index := 0; index < numEntries; index++ {
		entryOffset := 40 + index*40
		if entryOffset+40 > len(bytes) {
			return AssetCatalog{}, fmt.Errorf("RASC entry %d out of bounds", index)
		}
		guidIndex := int(binary.LittleEndian.Uint32(bytes[entryOffset : entryOffset+4]))
		subID := binary.LittleEndian.Uint32(bytes[entryOffset+4 : entryOffset+8])
		assetTypeIndex := int(binary.LittleEndian.Uint32(bytes[entryOffset+16 : entryOffset+20]))
		sizeBytes := binary.LittleEndian.Uint32(bytes[entryOffset+24 : entryOffset+28])
		dirStringOffset := int(binary.LittleEndian.Uint32(bytes[entryOffset+32 : entryOffset+36]))
		fileStringOffset := int(binary.LittleEndian.Uint32(bytes[entryOffset+36 : entryOffset+40]))
		dir := readNullTerminated(bytes, dirOffset+dirStringOffset)
		fileName := readNullTerminated(bytes, fileNameOffset+fileStringOffset)
		relativePath := fileName
		if dir != "" {
			relativePath = dir + "/" + fileName
		}
		guid, err := uuidAt(bytes, guidOffset+guidIndex*16)
		if err != nil {
			return AssetCatalog{}, fmt.Errorf("RASC entry %d asset id: %w", index, err)
		}
		assetType, err := uuidAt(bytes, assetTypeOffset+assetTypeIndex*16)
		if err != nil {
			return AssetCatalog{}, fmt.Errorf("RASC entry %d asset type: %w", index, err)
		}
		entries = append(entries, AssetCatalogEntry{
			AssetID: gametypes.AssetID{
				GUID:  guid,
				SubID: subID,
			},
			AssetType:    assetType,
			RelativePath: NormalizeVirtualPath(relativePath),
			SizeBytes:    sizeBytes,
		})
	}

	return AssetCatalog{Version: version, Entries: entries}, nil
}

func readNullTerminated(bytes []byte, offset int) string {
	if offset < 0 || offset > len(bytes) {
		return ""
	}
	end := offset
	for end < len(bytes) && bytes[end] != 0 {
		end++
	}
	return string(bytes[offset:end])
}

func uuidAt(bytes []byte, offset int) (uuid.UUID, error) {
	if offset < 0 || offset+16 > len(bytes) {
		return uuid.Nil, fmt.Errorf("range %d..%d is outside %d catalog bytes", offset, offset+16, len(bytes))
	}
	return uuid.FromBytes(bytes[offset : offset+16])
}

func NormalizeVirtualPath(path string) string {
	path = strings.ReplaceAll(path, "\\", "/")
	for strings.Contains(path, "//") {
		path = strings.ReplaceAll(path, "//", "/")
	}
	path = strings.Trim(path, "/")
	return strings.ToLower(path)
}
