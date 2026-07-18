package gameassets

import (
	"encoding/binary"
	"fmt"
	"math"
)

const (
	datasheetNameCRCOffset       = 0x04
	datasheetNameStringOffset    = 0x08
	datasheetTypeCRCOffset       = 0x0c
	datasheetTypeStringOffset    = 0x10
	datasheetDataSizeOffset      = 0x38
	datasheetColumnCountOffset   = 0x44
	datasheetRowCountOffset      = 0x48
	datasheetColumnRecordsOffset = 0x5c
	datasheetDataEndOffset       = 0x38
	datasheetU32Size             = 4
	datasheetColumnRecordSize    = 12
	datasheetCellRecordSize      = 8
)

var datasheetVersions = map[uint32]struct{}{
	0x11: {},
	0x12: {},
}

type DatasheetColumnType uint32

const (
	DatasheetColumnString  DatasheetColumnType = 0x01
	DatasheetColumnNumber  DatasheetColumnType = 0x02
	DatasheetColumnBoolean DatasheetColumnType = 0x03
)

type Datasheet struct {
	Version  uint32
	NameCRC  uint32
	Name     string
	TypeCRC  uint32
	TypeName string
	Columns  []DatasheetColumn
	Rows     []DatasheetRow
}

type DatasheetColumn struct {
	CRC        uint32
	Name       string
	ColumnType DatasheetColumnType
}

type DatasheetRow struct {
	Cells []DatasheetCell
}

type DatasheetCell struct {
	CRC   uint32
	Value DatasheetCellValue
}

type DatasheetCellKind uint8

const (
	DatasheetCellString DatasheetCellKind = iota + 1
	DatasheetCellNumber
	DatasheetCellBoolean
)

type DatasheetCellValue struct {
	Kind    DatasheetCellKind
	String  string
	Number  float32
	Boolean bool
}

func ParseDatasheet(bytes []byte) (Datasheet, error) {
	version, err := readDatasheetU32(bytes, 0x00, "datasheet version")
	if err != nil {
		return Datasheet{}, err
	}
	if _, ok := datasheetVersions[version]; !ok {
		return Datasheet{}, fmt.Errorf("unsupported datasheet version %#x", version)
	}

	nameCRC, err := readDatasheetU32(bytes, datasheetNameCRCOffset, "datasheet name crc")
	if err != nil {
		return Datasheet{}, err
	}
	nameOffset, err := readDatasheetU32(bytes, datasheetNameStringOffset, "datasheet name string offset")
	if err != nil {
		return Datasheet{}, err
	}
	typeCRC, err := readDatasheetU32(bytes, datasheetTypeCRCOffset, "datasheet type crc")
	if err != nil {
		return Datasheet{}, err
	}
	typeOffset, err := readDatasheetU32(bytes, datasheetTypeStringOffset, "datasheet type string offset")
	if err != nil {
		return Datasheet{}, err
	}
	dataSize, err := readDatasheetU32(bytes, datasheetDataSizeOffset, "datasheet data size")
	if err != nil {
		return Datasheet{}, err
	}
	columnCount, err := readDatasheetU32(bytes, datasheetColumnCountOffset, "datasheet column count")
	if err != nil {
		return Datasheet{}, err
	}
	rowCount, err := readDatasheetU32(bytes, datasheetRowCountOffset, "datasheet row count")
	if err != nil {
		return Datasheet{}, err
	}

	columnRecordsLength, err := checkedDatasheetProduct(columnCount, datasheetColumnRecordSize, "datasheet column records")
	if err != nil {
		return Datasheet{}, err
	}
	cellCount, err := checkedDatasheetProduct(rowCount, columnCount, "datasheet cell count")
	if err != nil {
		return Datasheet{}, err
	}
	cellRecordsLength, err := checkedDatasheetProduct(cellCount, datasheetCellRecordSize, "datasheet cell records")
	if err != nil {
		return Datasheet{}, err
	}
	columnsOffset := datasheetColumnRecordsOffset
	cellsOffset := columnsOffset + int(columnRecordsLength)
	stringsOffset := cellsOffset + int(cellRecordsLength)
	expectedStringsOffset := datasheetDataEndOffset + datasheetU32Size + int(dataSize)
	if stringsOffset != expectedStringsOffset {
		return Datasheet{}, fmt.Errorf("datasheet data_size points string table at %#x, expected %#x", expectedStringsOffset, stringsOffset)
	}
	if err := requireDatasheetRange(bytes, stringsOffset, len(bytes)-stringsOffset, "datasheet string table"); err != nil {
		return Datasheet{}, err
	}

	stringAt := func(offset uint32, label string) (string, error) {
		return readDatasheetString(bytes, stringsOffset, int(offset), label)
	}
	name, err := stringAt(nameOffset, "datasheet name")
	if err != nil {
		return Datasheet{}, err
	}
	typeName, err := stringAt(typeOffset, "datasheet type name")
	if err != nil {
		return Datasheet{}, err
	}

	columns := make([]DatasheetColumn, 0, columnCount)
	for columnIndex := uint32(0); columnIndex < columnCount; columnIndex++ {
		record := columnsOffset + int(columnIndex)*datasheetColumnRecordSize
		if err := requireDatasheetRange(bytes, record, datasheetColumnRecordSize, fmt.Sprintf("datasheet column %d", columnIndex)); err != nil {
			return Datasheet{}, err
		}
		crc := binary.LittleEndian.Uint32(bytes[record : record+4])
		nameOffset := int32(binary.LittleEndian.Uint32(bytes[record+4 : record+8]))
		if nameOffset < 0 {
			return Datasheet{}, fmt.Errorf("datasheet column %d has negative string offset %d", columnIndex, nameOffset)
		}
		name, err := readDatasheetString(bytes, stringsOffset, int(nameOffset), fmt.Sprintf("datasheet column %d name", columnIndex))
		if err != nil {
			return Datasheet{}, err
		}
		columnType, err := parseDatasheetColumnType(binary.LittleEndian.Uint32(bytes[record+8:record+12]), columnIndex)
		if err != nil {
			return Datasheet{}, err
		}
		columns = append(columns, DatasheetColumn{
			CRC:        crc,
			Name:       name,
			ColumnType: columnType,
		})
	}

	rows := make([]DatasheetRow, 0, rowCount)
	for rowIndex := uint32(0); rowIndex < rowCount; rowIndex++ {
		cells := make([]DatasheetCell, 0, columnCount)
		for columnIndex := uint32(0); columnIndex < columnCount; columnIndex++ {
			record := cellsOffset + int(rowIndex*columnCount+columnIndex)*datasheetCellRecordSize
			if err := requireDatasheetRange(bytes, record, datasheetCellRecordSize, fmt.Sprintf("datasheet row %d column %d", rowIndex, columnIndex)); err != nil {
				return Datasheet{}, err
			}
			value, err := parseDatasheetCellValue(bytes, stringsOffset, record+4, columns[columnIndex], rowIndex)
			if err != nil {
				return Datasheet{}, err
			}
			cells = append(cells, DatasheetCell{
				CRC:   binary.LittleEndian.Uint32(bytes[record : record+4]),
				Value: value,
			})
		}
		rows = append(rows, DatasheetRow{Cells: cells})
	}

	return Datasheet{
		Version:  version,
		NameCRC:  nameCRC,
		Name:     name,
		TypeCRC:  typeCRC,
		TypeName: typeName,
		Columns:  columns,
		Rows:     rows,
	}, nil
}

func parseDatasheetColumnType(raw uint32, columnIndex uint32) (DatasheetColumnType, error) {
	switch DatasheetColumnType(raw) {
	case DatasheetColumnString:
		return DatasheetColumnString, nil
	case DatasheetColumnNumber:
		return DatasheetColumnNumber, nil
	case DatasheetColumnBoolean:
		return DatasheetColumnBoolean, nil
	default:
		return 0, fmt.Errorf("unknown datasheet column type %d at column %d", raw, columnIndex)
	}
}

func parseDatasheetCellValue(bytes []byte, stringsOffset int, valueOffset int, column DatasheetColumn, rowIndex uint32) (DatasheetCellValue, error) {
	switch column.ColumnType {
	case DatasheetColumnString:
		offset, err := readDatasheetU32(bytes, valueOffset, column.Name+" string offset")
		if err != nil {
			return DatasheetCellValue{}, err
		}
		value, err := readDatasheetString(bytes, stringsOffset, int(offset), fmt.Sprintf("row %d %s", rowIndex, column.Name))
		if err != nil {
			return DatasheetCellValue{}, err
		}
		return DatasheetCellValue{Kind: DatasheetCellString, String: value}, nil
	case DatasheetColumnNumber:
		if err := requireDatasheetRange(bytes, valueOffset, 4, column.Name+" number"); err != nil {
			return DatasheetCellValue{}, err
		}
		return DatasheetCellValue{
			Kind:   DatasheetCellNumber,
			Number: math.Float32frombits(binary.LittleEndian.Uint32(bytes[valueOffset : valueOffset+4])),
		}, nil
	case DatasheetColumnBoolean:
		if err := requireDatasheetRange(bytes, valueOffset, 4, column.Name+" boolean"); err != nil {
			return DatasheetCellValue{}, err
		}
		return DatasheetCellValue{
			Kind:    DatasheetCellBoolean,
			Boolean: int32(binary.LittleEndian.Uint32(bytes[valueOffset:valueOffset+4])) != 0,
		}, nil
	default:
		return DatasheetCellValue{}, fmt.Errorf("unknown datasheet column type %d for %s", column.ColumnType, column.Name)
	}
}

func readDatasheetU32(bytes []byte, offset int, label string) (uint32, error) {
	if err := requireDatasheetRange(bytes, offset, 4, label); err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint32(bytes[offset : offset+4]), nil
}

func readDatasheetString(bytes []byte, stringsOffset int, offset int, label string) (string, error) {
	if offset < 0 {
		return "", fmt.Errorf("%s has negative string offset %d", label, offset)
	}
	start := stringsOffset + offset
	if err := requireDatasheetRange(bytes, start, 1, label); err != nil {
		return "", err
	}
	end := start
	for end < len(bytes) && bytes[end] != 0 {
		end++
	}
	if end == len(bytes) {
		return "", fmt.Errorf("%s is unterminated", label)
	}
	return string(bytes[start:end]), nil
}

func checkedDatasheetProduct(left uint32, right uint32, label string) (uint32, error) {
	product := uint64(left) * uint64(right)
	if product > 0xffff_ffff {
		return 0, fmt.Errorf("%s length overflow", label)
	}
	return uint32(product), nil
}

func requireDatasheetRange(bytes []byte, offset int, length int, label string) error {
	if offset < 0 || length < 0 || offset+length < offset || offset+length > len(bytes) {
		return fmt.Errorf("%s out of bounds", label)
	}
	return nil
}
