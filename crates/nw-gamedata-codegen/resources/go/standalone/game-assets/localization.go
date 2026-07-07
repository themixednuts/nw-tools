package gameassets

import (
	"encoding/xml"
	"io"
	"strings"
)

const LocalizationExtension = "xml"

type LocalizationBundle struct {
	Entries map[string]string
}

func ParseLocalizationXML(bytes []byte) (LocalizationBundle, error) {
	decoder := xml.NewDecoder(strings.NewReader(string(bytes)))
	entries := make(map[string]string)
	var currentKey string

	for {
		token, err := decoder.Token()
		if err == io.EOF {
			break
		}
		if err != nil {
			return LocalizationBundle{}, err
		}

		switch token := token.(type) {
		case xml.StartElement:
			currentKey = localizationKey(token)
		case xml.CharData:
			if currentKey != "" {
				entries[currentKey] = strings.TrimSpace(string(token))
				currentKey = ""
			}
		case xml.EndElement:
			currentKey = ""
		}
	}

	return LocalizationBundle{Entries: entries}, nil
}

func IsLocalizationPath(path string) bool {
	return strings.HasSuffix(strings.ToLower(path), "."+LocalizationExtension)
}

func localizationKey(element xml.StartElement) string {
	for _, attr := range element.Attr {
		switch attr.Name.Local {
		case "key", "Key", "name", "Name", "id", "Id":
			return attr.Value
		}
	}
	return ""
}
