package gameassets

import (
	"os"
	"path/filepath"
	"sort"
)

func LoadLooseDatasheets(root string) ([]DatasheetAsset, error) {
	var assets []DatasheetAsset
	if err := collectLooseDatasheets(root, root, &assets); err != nil {
		return nil, err
	}
	return assets, nil
}

func collectLooseDatasheets(root string, dir string, assets *[]DatasheetAsset) error {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	sort.Slice(entries, func(i int, j int) bool { return entries[i].Name() < entries[j].Name() })
	for _, entry := range entries {
		path := filepath.Join(dir, entry.Name())
		if entry.IsDir() {
			if err := collectLooseDatasheets(root, path, assets); err != nil {
				return err
			}
			continue
		}
		if !entry.Type().IsRegular() || !IsDatasheetPath(path) {
			continue
		}
		bytes, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if !IsDatasheetBytes(bytes) {
			continue
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		*assets = append(*assets, DatasheetAsset{
			Path:  NormalizeVirtualPath(relative),
			Bytes: bytes,
		})
	}
	return nil
}
