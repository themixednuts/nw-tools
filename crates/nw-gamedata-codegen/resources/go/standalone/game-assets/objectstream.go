package gameassets

import (
	"encoding/binary"
	"fmt"
	"math"
	"strings"
)

var objectStreamExtensions = map[string]struct{}{
	"slice":        {},
	"dynamicslice": {},
	"spawnable":   {},
	"uicanvas":    {},
	"aoffdb":      {},
	"equipdb":     {},
	"gds":         {},
	"uidb":        {},
	"pbadb":       {},
	"sprd":        {},
	"gdb":         {},
	"gactdb":      {},
	"rankdb":      {},
	"craftstationdb": {},
}

const (
	objectStreamTagBinary byte = 0
	objectStreamHeader    byte = 1 << 3
	objectStreamHasValue  byte = 1 << 4
	objectStreamExtraSize byte = 1 << 5
	objectStreamHasName   byte = 1 << 6
	objectStreamHasVer    byte = 1 << 7
	objectStreamSizeMask  byte = 0x07
)

type ObjectStreamAsset struct {
	Path  string
	Bytes []byte
}

type ObjectStream struct {
	Version  uint32
	Elements []ObjectStreamElement
}

type ObjectStreamElement struct {
	Flags   byte
	NameCRC uint32
	HasName bool
	Version byte
	HasVer  bool
	TypeID  string
	Data    []byte
	Children []ObjectStreamElement
}

type Vec3 struct {
	X float32
	Y float32
	Z float32
}

func IsObjectStreamPath(path string) bool {
	extension := strings.ToLower(pathExtension(path))
	_, ok := objectStreamExtensions[extension]
	return ok
}

func pathExtension(path string) string {
	path = strings.TrimRight(path, "/\\")
	index := strings.LastIndex(path, ".")
	if index < 0 || index == len(path)-1 {
		return ""
	}
	return path[index+1:]
}

func ParseObjectStream(bytes []byte) (ObjectStream, error) {
	reader := objectStreamReader{bytes: bytes}
	tag, err := reader.u8()
	if err != nil {
		return ObjectStream{}, err
	}
	if tag != objectStreamTagBinary {
		return ObjectStream{}, fmt.Errorf("unsupported ObjectStream tag %d", tag)
	}
	version, err := reader.u32()
	if err != nil {
		return ObjectStream{}, err
	}
	if version != 2 && version != 3 {
		return ObjectStream{}, fmt.Errorf("unsupported ObjectStream version %d", version)
	}

	stream := ObjectStream{Version: version}
	var stack []*ObjectStreamElement
	for !reader.isDone() {
		flags, err := reader.u8()
		if err != nil {
			return ObjectStream{}, err
		}
		if flags == 0 {
			if len(stack) == 0 {
				break
			}
			stack = stack[:len(stack)-1]
			continue
		}
		if flags&objectStreamHeader == 0 {
			return ObjectStream{}, fmt.Errorf("invalid ObjectStream element flags %d", flags)
		}

		element := ObjectStreamElement{Flags: flags}
		if flags&objectStreamHasName != 0 {
			nameCRC, err := reader.u32()
			if err != nil {
				return ObjectStream{}, err
			}
			element.NameCRC = nameCRC
			element.HasName = true
		}
		if flags&objectStreamHasVer != 0 {
			element.Version, err = reader.u8()
			if err != nil {
				return ObjectStream{}, err
			}
			element.HasVer = true
		}
		element.TypeID, err = reader.uuid()
		if err != nil {
			return ObjectStream{}, err
		}
		if version == 2 {
			if _, err := reader.uuid(); err != nil {
				return ObjectStream{}, err
			}
		}
		dataSize, err := objectStreamDataSize(&reader, flags)
		if err != nil {
			return ObjectStream{}, err
		}
		element.Data, err = reader.read(dataSize)
		if err != nil {
			return ObjectStream{}, err
		}

		if len(stack) == 0 {
			stream.Elements = append(stream.Elements, element)
			stack = append(stack, &stream.Elements[len(stream.Elements)-1])
		} else {
			parent := stack[len(stack)-1]
			parent.Children = append(parent.Children, element)
			stack = append(stack, &parent.Children[len(parent.Children)-1])
		}
	}
	if len(stack) != 0 {
		return ObjectStream{}, fmt.Errorf("ObjectStream ended before all elements were closed")
	}
	return stream, nil
}

func SingleObjectStreamRoot(stream ObjectStream, expectedTypeID string) (*ObjectStreamElement, error) {
	if len(stream.Elements) != 1 {
		return nil, fmt.Errorf("expected one ObjectStream root, found %d", len(stream.Elements))
	}
	root := &stream.Elements[0]
	if err := RequireObjectStreamType(root, expectedTypeID); err != nil {
		return nil, err
	}
	return root, nil
}

func RequireObjectStreamType(element *ObjectStreamElement, expectedTypeID string) error {
	if element.TypeID != strings.ToLower(expectedTypeID) {
		return fmt.Errorf("expected ObjectStream type %s, found %s", expectedTypeID, element.TypeID)
	}
	return nil
}

func ChildByNameCRC(element *ObjectStreamElement, nameCRC uint32) *ObjectStreamElement {
	for index := range element.Children {
		if element.Children[index].HasName && element.Children[index].NameCRC == nameCRC {
			return &element.Children[index]
		}
	}
	return nil
}

func RequiredChildByNameCRC(element *ObjectStreamElement, nameCRC uint32) (*ObjectStreamElement, error) {
	child := ChildByNameCRC(element, nameCRC)
	if child == nil {
		return nil, fmt.Errorf("ObjectStream element %s is missing field CRC %d", element.TypeID, nameCRC)
	}
	return child, nil
}

func ObjectStreamString(element *ObjectStreamElement) string {
	return string(element.Data)
}

func ObjectStreamBool(element *ObjectStreamElement) (bool, error) {
	if err := requireObjectStreamLength(element, 1); err != nil {
		return false, err
	}
	return element.Data[0] != 0, nil
}

func ObjectStreamU8(element *ObjectStreamElement) (uint8, error) {
	if err := requireObjectStreamLength(element, 1); err != nil {
		return 0, err
	}
	return element.Data[0], nil
}

func ObjectStreamI32(element *ObjectStreamElement) (int32, error) {
	if err := requireObjectStreamLength(element, 4); err != nil {
		return 0, err
	}
	return int32(binary.BigEndian.Uint32(element.Data)), nil
}

func ObjectStreamU32(element *ObjectStreamElement) (uint32, error) {
	if err := requireObjectStreamLength(element, 4); err != nil {
		return 0, err
	}
	return binary.BigEndian.Uint32(element.Data), nil
}

func ObjectStreamF32(element *ObjectStreamElement) (float32, error) {
	if err := requireObjectStreamLength(element, 4); err != nil {
		return 0, err
	}
	return math.Float32frombits(binary.BigEndian.Uint32(element.Data)), nil
}

func ObjectStreamVec3(element *ObjectStreamElement) (Vec3, error) {
	if err := requireObjectStreamLength(element, 12); err != nil {
		return Vec3{}, err
	}
	return Vec3{
		X: math.Float32frombits(binary.BigEndian.Uint32(element.Data[0:4])),
		Y: math.Float32frombits(binary.BigEndian.Uint32(element.Data[4:8])),
		Z: math.Float32frombits(binary.BigEndian.Uint32(element.Data[8:12])),
	}, nil
}

func objectStreamDataSize(reader *objectStreamReader, flags byte) (int, error) {
	if flags&objectStreamHasValue == 0 {
		return 0, nil
	}
	inlineSize := flags & objectStreamSizeMask
	if flags&objectStreamExtraSize == 0 {
		return int(inlineSize), nil
	}
	switch inlineSize {
	case 1:
		value, err := reader.u8()
		return int(value), err
	case 2:
		value, err := reader.u16()
		return int(value), err
	case 4:
		value, err := reader.u32()
		return int(value), err
	default:
		return 0, fmt.Errorf("unsupported ObjectStream value-size width %d", inlineSize)
	}
}

func requireObjectStreamLength(element *ObjectStreamElement, expected int) error {
	if len(element.Data) != expected {
		return fmt.Errorf("ObjectStream element %s has %d bytes, expected %d", element.TypeID, len(element.Data), expected)
	}
	return nil
}

type objectStreamReader struct {
	bytes  []byte
	offset int
}

func (reader *objectStreamReader) isDone() bool {
	return reader.offset >= len(reader.bytes)
}

func (reader *objectStreamReader) u8() (byte, error) {
	if err := reader.require(1); err != nil {
		return 0, err
	}
	value := reader.bytes[reader.offset]
	reader.offset++
	return value, nil
}

func (reader *objectStreamReader) u16() (uint16, error) {
	if err := reader.require(2); err != nil {
		return 0, err
	}
	value := binary.BigEndian.Uint16(reader.bytes[reader.offset:])
	reader.offset += 2
	return value, nil
}

func (reader *objectStreamReader) u32() (uint32, error) {
	if err := reader.require(4); err != nil {
		return 0, err
	}
	value := binary.BigEndian.Uint32(reader.bytes[reader.offset:])
	reader.offset += 4
	return value, nil
}

func (reader *objectStreamReader) uuid() (string, error) {
	bytes, err := reader.read(16)
	if err != nil {
		return "", err
	}
	return fmt.Sprintf("%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
		bytes[0], bytes[1], bytes[2], bytes[3],
		bytes[4], bytes[5],
		bytes[6], bytes[7],
		bytes[8], bytes[9],
		bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
	), nil
}

func (reader *objectStreamReader) read(length int) ([]byte, error) {
	if err := reader.require(length); err != nil {
		return nil, err
	}
	out := append([]byte(nil), reader.bytes[reader.offset:reader.offset+length]...)
	reader.offset += length
	return out, nil
}

func (reader *objectStreamReader) require(length int) error {
	if reader.offset+length > len(reader.bytes) {
		return fmt.Errorf("ObjectStream ended unexpectedly")
	}
	return nil
}
