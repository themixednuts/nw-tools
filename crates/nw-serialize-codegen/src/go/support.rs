pub(super) fn single_file_source() -> &'static str {
    r#"
import (
	"crypto/sha1"
	"fmt"
	"reflect"
	"strconv"
	"strings"
	"sync"

	"github.com/google/uuid"
)

type Uuid struct {
	value uuid.UUID
}

var NilUuid = Uuid{}

func UuidFromGoogle(value uuid.UUID) Uuid {
	return Uuid{value: value}
}

func ParseUuid(value string) (Uuid, error) {
	parsed, err := uuid.Parse(value)
	if err != nil {
		return Uuid{}, err
	}
	return UuidFromGoogle(parsed), nil
}

func MustParseUuid(value string) Uuid {
	parsed, err := ParseUuid(value)
	if err != nil {
		panic(err)
	}
	return parsed
}

func CreateUuidData(bytes []byte) Uuid {
	if len(bytes) == 0 {
		return NilUuid
	}

	digest := sha1.Sum(bytes)
	var data [16]byte
	copy(data[:], digest[:16])
	data[8] = (data[8] & 0xbf) | 0x80
	data[6] = (data[6] & 0x5f) | 0x50
	return Uuid{value: uuid.UUID(data)}
}

func CreateUuidName(name string) Uuid {
	return CreateUuidData([]byte(name))
}

func CombineUuid(lhs, rhs Uuid) Uuid {
	bytes := make([]byte, 32)
	copy(bytes[:16], lhs.value[:])
	copy(bytes[16:], rhs.value[:])
	return CreateUuidData(bytes)
}

func AggregateUuidTypeIDs(typeIDs []Uuid) (Uuid, bool) {
	if len(typeIDs) == 0 {
		return NilUuid, false
	}

	acc := typeIDs[0]
	for _, typeID := range typeIDs[1:] {
		acc = CombineUuid(acc, typeID)
	}
	return acc, true
}

func AggregateUuidTypeIDsRight(typeIDs []Uuid) (Uuid, bool) {
	if len(typeIDs) == 0 {
		return NilUuid, false
	}

	first := typeIDs[0]
	tail, ok := AggregateUuidTypeIDsRight(typeIDs[1:])
	if ok && !tail.IsNil() {
		return CombineUuid(first, tail), true
	}
	return first, true
}

func SpecializedUuidTemplatePrefix(templateBase Uuid, args []Uuid) (Uuid, bool) {
	aggregate, ok := AggregateUuidTypeIDs(args)
	if !ok {
		return NilUuid, false
	}
	return CombineUuid(templateBase, aggregate), true
}

func SpecializedUuidTemplatePostfix(templateBase Uuid, args []Uuid) (Uuid, bool) {
	aggregate, ok := AggregateUuidTypeIDs(args)
	if !ok {
		return NilUuid, false
	}
	return CombineUuid(aggregate, templateBase), true
}

func TemplateAutoUuidTypeID(value uint) Uuid {
	return CreateUuidName(strconv.FormatUint(uint64(value), 10))
}

func (id Uuid) Google() uuid.UUID {
	return id.value
}

func (id Uuid) String() string {
	return id.value.String()
}

func (id Uuid) IsNil() bool {
	return id == NilUuid
}

type AzRtti struct {
	Name   string       `json:"name"`
	TypeID Uuid         `json:"typeId"`
	GoType reflect.Type `json:"-"`
}

type HasAzRtti interface {
	AzRtti() *AzRtti
}

type AzRttiRegistry struct {
	mu     sync.RWMutex
	byID   map[Uuid]*AzRtti
	byType map[reflect.Type]*AzRtti
}

var DefaultAzRttiRegistry = NewAzRttiRegistry()

func NewAzRttiRegistry() *AzRttiRegistry {
	return &AzRttiRegistry{
		byID:   make(map[Uuid]*AzRtti),
		byType: make(map[reflect.Type]*AzRtti),
	}
}

func RegisterAzRtti[T any](name string, typeID string) *AzRtti {
	return DefaultAzRttiRegistry.register(name, MustParseUuid(typeID), azRttiTypeOf[T]())
}

func AzRttiFor[T any]() *AzRtti {
	return DefaultAzRttiRegistry.typeFor(azRttiTypeOf[T]())
}

func LookupAzRtti(typeID Uuid) (*AzRtti, bool) {
	return DefaultAzRttiRegistry.lookup(typeID)
}

func LookupAzRttiString(typeID string) (*AzRtti, bool) {
	return LookupAzRtti(MustParseUuid(typeID))
}

func (registry *AzRttiRegistry) register(name string, typeID Uuid, goType reflect.Type) *AzRtti {
	registry.mu.Lock()
	defer registry.mu.Unlock()

	if existing, ok := registry.byID[typeID]; ok {
		if existing.Name != name || existing.GoType != goType {
			panic(fmt.Sprintf("AZ RTTI type id %s already registered for %s (%s), cannot register %s (%s)", typeID, existing.Name, existing.GoType, name, goType))
		}
		return existing
	}
	if existing, ok := registry.byType[goType]; ok {
		if existing.Name != name || existing.TypeID != typeID {
			panic(fmt.Sprintf("Go type %s already registered as %s (%s), cannot register %s (%s)", goType, existing.Name, existing.TypeID, name, typeID))
		}
		return existing
	}

	rtti := &AzRtti{Name: name, TypeID: typeID, GoType: goType}
	registry.byID[typeID] = rtti
	registry.byType[goType] = rtti
	return rtti
}

func (registry *AzRttiRegistry) typeFor(goType reflect.Type) *AzRtti {
	registry.mu.RLock()
	rtti, ok := registry.byType[goType]
	registry.mu.RUnlock()
	if !ok {
		panic(fmt.Sprintf("Go type %s is not registered with AZ RTTI", goType))
	}
	return rtti
}

func (registry *AzRttiRegistry) lookup(typeID Uuid) (*AzRtti, bool) {
	registry.mu.RLock()
	rtti, ok := registry.byID[typeID]
	registry.mu.RUnlock()
	return rtti, ok
}

func (rtti *AzRtti) IsZero() bool {
	if rtti == nil {
		return true
	}
	return rtti.Name == "" && rtti.TypeID == NilUuid
}

func azRttiTypeOf[T any]() reflect.Type {
	return normalizeAzRttiType(reflect.TypeFor[T]())
}

func normalizeAzRttiType(goType reflect.Type) reflect.Type {
	if goType == nil {
		panic("AZ RTTI requires a concrete Go type")
	}
	for goType.Kind() == reflect.Pointer {
		goType = goType.Elem()
	}
	return goType
}

type Crc32 uint32

const ZeroCrc32 Crc32 = 0

func NewCrc32(value uint32) Crc32 {
	return Crc32(value)
}

func Crc32FromStringLower(value string) Crc32 {
	return Crc32FromBytes([]byte(value), true)
}

func Crc32FromBytes(bytes []byte, forceLowerCase bool) Crc32 {
	return Crc32(crc32(bytes, forceLowerCase))
}

func (value Crc32) Value() uint32 {
	return uint32(value)
}

type Vector2 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
}

type Vector3 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
}

type Vector4 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
	W float32 `json:"w"`
}

type Quaternion struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
	W float32 `json:"w"`
}

type Transform struct {
	BasisX      Vector3 `json:"basisX"`
	BasisY      Vector3 `json:"basisY"`
	BasisZ      Vector3 `json:"basisZ"`
	Translation Vector3 `json:"translation"`
}

type Color struct {
	R float32 `json:"r"`
	G float32 `json:"g"`
	B float32 `json:"b"`
	A float32 `json:"a"`
}

type ColorF = Color

type ColorB struct {
	R uint8 `json:"r"`
	G uint8 `json:"g"`
	B uint8 `json:"b"`
	A uint8 `json:"a"`
}

func crc32(bytes []byte, forceLowerCase bool) uint32 {
	crc := uint32(0xffffffff)
	for _, value := range bytes {
		if forceLowerCase && value >= 'A' && value <= 'Z' {
			value += 'a' - 'A'
		}
		crc ^= uint32(value)
		for bit := 0; bit < 8; bit++ {
			if crc&1 != 0 {
				crc = 0xedb88320 ^ (crc >> 1)
			} else {
				crc >>= 1
			}
		}
	}
	return crc ^ 0xffffffff
}

type AssetId struct {
	Guid  Uuid   `json:"guid"`
	SubId uint32 `json:"subId"`
}

func NewAssetId(guid Uuid, subID uint32) AssetId {
	return AssetId{Guid: guid, SubId: subID}
}

func NilAssetId() AssetId {
	return AssetId{Guid: NilUuid, SubId: 0}
}

func (assetID AssetId) IsNil() bool {
	return assetID.SubId == 0 && assetID.Guid == NilUuid
}

func (assetID AssetId) String() string {
	return fmt.Sprintf("{%s}:%x", strings.ToUpper(assetID.Guid.String()), assetID.SubId)
}

func ParseAssetId(value string) (AssetId, error) {
	separator := strings.LastIndex(value, ":")
	if separator < 0 {
		return AssetId{}, fmt.Errorf("missing ':' separator between guid and sub_id")
	}

	guidPart := strings.Trim(value[:separator], "{}")
	guid, err := ParseUuid(guidPart)
	if err != nil {
		return AssetId{}, fmt.Errorf("invalid guid: %w", err)
	}
	subID, err := strconv.ParseUint(value[separator+1:], 16, 32)
	if err != nil {
		return AssetId{}, fmt.Errorf("invalid sub_id: %w", err)
	}
	return AssetId{Guid: guid, SubId: uint32(subID)}, nil
}

type Asset struct {
	AssetId   AssetId `json:"assetId"`
	AssetType Uuid    `json:"assetType"`
	Hint      string  `json:"hint,omitempty"`
}

func EmptyAsset() Asset {
	return Asset{AssetId: NilAssetId(), AssetType: NilUuid}
}

func AssetFromId(assetID AssetId, assetType Uuid) Asset {
	return Asset{AssetId: assetID, AssetType: assetType}
}

func AssetFromHint(hint string) Asset {
	return Asset{AssetId: NilAssetId(), AssetType: NilUuid, Hint: strings.TrimSpace(hint)}
}

func (asset Asset) IsNil() bool {
	return asset.AssetId.IsNil()
}

func (asset Asset) IsEmpty() bool {
	return asset.IsNil() && asset.AssetType == NilUuid && strings.TrimSpace(asset.Hint) == ""
}

"#
}

pub(super) fn uuid_package_source() -> &'static str {
    r#"
package uuid

import (
	"crypto/sha1"
	"strconv"

	googleuuid "github.com/google/uuid"
)

type Uuid struct {
	value googleuuid.UUID
}

var Nil = Uuid{}

func FromGoogle(value googleuuid.UUID) Uuid {
	return Uuid{value: value}
}

func Parse(value string) (Uuid, error) {
	parsed, err := googleuuid.Parse(value)
	if err != nil {
		return Uuid{}, err
	}
	return FromGoogle(parsed), nil
}

func MustParse(value string) Uuid {
	parsed, err := Parse(value)
	if err != nil {
		panic(err)
	}
	return parsed
}

func CreateData(bytes []byte) Uuid {
	if len(bytes) == 0 {
		return Nil
	}

	digest := sha1.Sum(bytes)
	var data [16]byte
	copy(data[:], digest[:16])
	data[8] = (data[8] & 0xbf) | 0x80
	data[6] = (data[6] & 0x5f) | 0x50
	return Uuid{value: googleuuid.UUID(data)}
}

func CreateName(name string) Uuid {
	return CreateData([]byte(name))
}

func Combine(lhs, rhs Uuid) Uuid {
	bytes := make([]byte, 32)
	copy(bytes[:16], lhs.value[:])
	copy(bytes[16:], rhs.value[:])
	return CreateData(bytes)
}

func AggregateTypeIDs(typeIDs []Uuid) (Uuid, bool) {
	if len(typeIDs) == 0 {
		return Nil, false
	}

	acc := typeIDs[0]
	for _, typeID := range typeIDs[1:] {
		acc = Combine(acc, typeID)
	}
	return acc, true
}

func AggregateTypeIDsRight(typeIDs []Uuid) (Uuid, bool) {
	if len(typeIDs) == 0 {
		return Nil, false
	}

	first := typeIDs[0]
	tail, ok := AggregateTypeIDsRight(typeIDs[1:])
	if ok && !tail.IsNil() {
		return Combine(first, tail), true
	}
	return first, true
}

func SpecializedTemplatePrefix(templateBase Uuid, args []Uuid) (Uuid, bool) {
	aggregate, ok := AggregateTypeIDs(args)
	if !ok {
		return Nil, false
	}
	return Combine(templateBase, aggregate), true
}

func SpecializedTemplatePostfix(templateBase Uuid, args []Uuid) (Uuid, bool) {
	aggregate, ok := AggregateTypeIDs(args)
	if !ok {
		return Nil, false
	}
	return Combine(aggregate, templateBase), true
}

func TemplateAutoTypeID(value uint) Uuid {
	return CreateName(strconv.FormatUint(uint64(value), 10))
}

func (id Uuid) Google() googleuuid.UUID {
	return id.value
}

func (id Uuid) String() string {
	return id.value.String()
}

func (id Uuid) IsNil() bool {
	return id == Nil
}
"#
}

pub(super) fn rtti_package_source(module_path: &str) -> String {
    rtti_package_template().replace("{{MODULE_PATH}}", module_path)
}

fn rtti_package_template() -> &'static str {
    r#"
package rtti

import (
	"fmt"
	"reflect"
	"sync"

	"{{MODULE_PATH}}/az/uuid"
)

type Type struct {
	Name   string       `json:"name"`
	TypeID uuid.Uuid    `json:"typeId"`
	GoType reflect.Type `json:"-"`
}

type HasRtti interface {
	AzRtti() *Type
}

type Registry struct {
	mu     sync.RWMutex
	byID   map[uuid.Uuid]*Type
	byType map[reflect.Type]*Type
}

var DefaultRegistry = NewRegistry()

func NewRegistry() *Registry {
	return &Registry{
		byID:   make(map[uuid.Uuid]*Type),
		byType: make(map[reflect.Type]*Type),
	}
}

func Register[T any](name string, typeID string) *Type {
	return DefaultRegistry.register(name, uuid.MustParse(typeID), rttiTypeOf[T]())
}

func TypeFor[T any]() *Type {
	return DefaultRegistry.typeFor(rttiTypeOf[T]())
}

func Lookup(typeID uuid.Uuid) (*Type, bool) {
	return DefaultRegistry.lookup(typeID)
}

func LookupString(typeID string) (*Type, bool) {
	return Lookup(uuid.MustParse(typeID))
}

func (registry *Registry) register(name string, typeID uuid.Uuid, goType reflect.Type) *Type {
	registry.mu.Lock()
	defer registry.mu.Unlock()

	if existing, ok := registry.byID[typeID]; ok {
		if existing.Name != name || existing.GoType != goType {
			panic(fmt.Sprintf("AZ RTTI type id %s already registered for %s (%s), cannot register %s (%s)", typeID, existing.Name, existing.GoType, name, goType))
		}
		return existing
	}
	if existing, ok := registry.byType[goType]; ok {
		if existing.Name != name || existing.TypeID != typeID {
			panic(fmt.Sprintf("Go type %s already registered as %s (%s), cannot register %s (%s)", goType, existing.Name, existing.TypeID, name, typeID))
		}
		return existing
	}

	rtti := &Type{Name: name, TypeID: typeID, GoType: goType}
	registry.byID[typeID] = rtti
	registry.byType[goType] = rtti
	return rtti
}

func (registry *Registry) typeFor(goType reflect.Type) *Type {
	registry.mu.RLock()
	rtti, ok := registry.byType[goType]
	registry.mu.RUnlock()
	if !ok {
		panic(fmt.Sprintf("Go type %s is not registered with AZ RTTI", goType))
	}
	return rtti
}

func (registry *Registry) lookup(typeID uuid.Uuid) (*Type, bool) {
	registry.mu.RLock()
	rtti, ok := registry.byID[typeID]
	registry.mu.RUnlock()
	return rtti, ok
}

func (rtti *Type) IsZero() bool {
	if rtti == nil {
		return true
	}
	return rtti.Name == "" && rtti.TypeID == uuid.Nil
}

func rttiTypeOf[T any]() reflect.Type {
	return normalizeType(reflect.TypeFor[T]())
}

func normalizeType(goType reflect.Type) reflect.Type {
	if goType == nil {
		panic("AZ RTTI requires a concrete Go type")
	}
	for goType.Kind() == reflect.Pointer {
		goType = goType.Elem()
	}
	return goType
}
"#
}

pub(super) fn crc_package_source() -> &'static str {
    r#"
package crc

type Crc32 uint32

const Zero Crc32 = 0

func New(value uint32) Crc32 {
	return Crc32(value)
}

func FromStringLower(value string) Crc32 {
	return FromBytesLower([]byte(value))
}

func FromBytesLower(bytes []byte) Crc32 {
	return Crc32(crc32(bytes, true))
}

func FromBytes(bytes []byte, forceLowerCase bool) Crc32 {
	return Crc32(crc32(bytes, forceLowerCase))
}

func (value Crc32) Value() uint32 {
	return uint32(value)
}

func crc32(bytes []byte, forceLowerCase bool) uint32 {
	crc := uint32(0xffffffff)
	for _, value := range bytes {
		if forceLowerCase && value >= 'A' && value <= 'Z' {
			value += 'a' - 'A'
		}
		crc ^= uint32(value)
		for bit := 0; bit < 8; bit++ {
			if crc&1 != 0 {
				crc = 0xedb88320 ^ (crc >> 1)
			} else {
				crc >>= 1
			}
		}
	}
	return crc ^ 0xffffffff
}
"#
}

pub(super) fn math_package_source() -> &'static str {
    r#"
package math

type Vector2 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
}

type Vector3 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
}

type Vector4 struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
	W float32 `json:"w"`
}

type Quaternion struct {
	X float32 `json:"x"`
	Y float32 `json:"y"`
	Z float32 `json:"z"`
	W float32 `json:"w"`
}

type Transform struct {
	BasisX      Vector3 `json:"basisX"`
	BasisY      Vector3 `json:"basisY"`
	BasisZ      Vector3 `json:"basisZ"`
	Translation Vector3 `json:"translation"`
}

type Color struct {
	R float32 `json:"r"`
	G float32 `json:"g"`
	B float32 `json:"b"`
	A float32 `json:"a"`
}

type ColorF = Color

type ColorB struct {
	R uint8 `json:"r"`
	G uint8 `json:"g"`
	B uint8 `json:"b"`
	A uint8 `json:"a"`
}
"#
}

pub(super) fn asset_package_source(module_path: &str) -> String {
    asset_package_template().replace("{{MODULE_PATH}}", module_path)
}

fn asset_package_template() -> &'static str {
    r#"
package asset

import (
	"fmt"
	"strconv"
	"strings"

	"{{MODULE_PATH}}/az/uuid"
)

type AssetId struct {
	Guid  uuid.Uuid `json:"guid"`
	SubId uint32    `json:"subId"`
}

func New(guid uuid.Uuid, subID uint32) AssetId {
	return AssetId{Guid: guid, SubId: subID}
}

func Nil() AssetId {
	return AssetId{Guid: uuid.Nil, SubId: 0}
}

func (assetID AssetId) IsNil() bool {
	return assetID.SubId == 0 && assetID.Guid == uuid.Nil
}

func (assetID AssetId) String() string {
	return fmt.Sprintf("{%s}:%x", strings.ToUpper(assetID.Guid.String()), assetID.SubId)
}

func Parse(value string) (AssetId, error) {
	separator := strings.LastIndex(value, ":")
	if separator < 0 {
		return AssetId{}, fmt.Errorf("missing ':' separator between guid and sub_id")
	}

	guidPart := strings.Trim(value[:separator], "{}")
	guid, err := uuid.Parse(guidPart)
	if err != nil {
		return AssetId{}, fmt.Errorf("invalid guid: %w", err)
	}
	subID, err := strconv.ParseUint(value[separator+1:], 16, 32)
	if err != nil {
		return AssetId{}, fmt.Errorf("invalid sub_id: %w", err)
	}
	return AssetId{Guid: guid, SubId: uint32(subID)}, nil
}

type Asset struct {
	AssetId   AssetId   `json:"assetId"`
	AssetType uuid.Uuid `json:"assetType"`
	Hint      string    `json:"hint,omitempty"`
}

func Empty() Asset {
	return Asset{AssetId: Nil(), AssetType: uuid.Nil}
}

func FromId(assetID AssetId, assetType uuid.Uuid) Asset {
	return Asset{AssetId: assetID, AssetType: assetType}
}

func FromHint(hint string) Asset {
	return Asset{AssetId: Nil(), AssetType: uuid.Nil, Hint: strings.TrimSpace(hint)}
}

func (asset Asset) IsNil() bool {
	return asset.AssetId.IsNil()
}

func (asset Asset) IsEmpty() bool {
	return asset.IsNil() && asset.AssetType == uuid.Nil && strings.TrimSpace(asset.Hint) == ""
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_sources_keep_az_uuid_and_crc_behavior_in_wrappers() {
        let uuid = uuid_package_source();
        let crc = crc_package_source();

        assert!(uuid.contains("func CreateData(bytes []byte) Uuid"));
        assert!(uuid.contains("data[8] = (data[8] & 0xbf) | 0x80"));
        assert!(single_file_source().contains("func AggregateUuidTypeIDs(typeIDs []Uuid)"));
        assert!(single_file_source().contains("type AzRtti struct"));
        assert!(single_file_source().contains("type HasAzRtti interface"));
        assert!(single_file_source().contains("type AzRttiRegistry struct"));
        assert!(single_file_source().contains("func RegisterAzRtti[T any]"));
        assert!(single_file_source().contains("func AzRttiFor[T any]() *AzRtti"));
        assert!(single_file_source().contains("func (rtti *AzRtti) IsZero() bool"));
        assert!(uuid.contains("func AggregateTypeIDs(typeIDs []Uuid) (Uuid, bool)"));
        assert!(uuid.contains("func AggregateTypeIDsRight(typeIDs []Uuid) (Uuid, bool)"));
        assert!(uuid.contains("func SpecializedTemplatePrefix(templateBase Uuid, args []Uuid)"));
        assert!(crc.contains("func FromStringLower(value string) Crc32"));
        assert!(crc.contains("func crc32(bytes []byte, forceLowerCase bool) uint32"));
        assert!(!crc.contains("hash/crc32"));
    }

    #[test]
    fn rtti_support_uses_requested_module_path() {
        let source = rtti_package_source("example.com/types");

        assert!(source.contains("\"example.com/types/az/uuid\""));
        assert!(source.contains("type HasRtti interface"));
        assert!(source.contains("AzRtti() *Type"));
        assert!(source.contains("func Register[T any]"));
        assert!(source.contains("func TypeFor[T any]() *Type"));
        assert!(source.contains("func (rtti *Type) IsZero() bool"));
        assert!(!source.contains("func Must(name string, typeID string) Type"));
        assert!(!source.contains("{{MODULE_PATH}}"));
    }

    #[test]
    fn asset_support_uses_requested_module_path() {
        let source = asset_package_source("example.com/types");

        assert!(source.contains("\"example.com/types/az/uuid\""));
        assert!(!source.contains("azuuid \""));
        assert!(!source.contains("{{MODULE_PATH}}"));
    }
}
