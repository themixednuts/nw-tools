use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use nw_objectstream::type_uuid::type_ids;

use crate::model::{ReflectedGenericClass, ReflectedMember, SerializeContextModel};
use crate::native::native_reflected_type_name;

const MAX_TYPE_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    Scalar(ScalarType),
    Named {
        type_id: Uuid,
        source_name: String,
    },
    Sequence {
        kind: SequenceKind,
        element: Box<ResolvedType>,
        capacity: Option<usize>,
    },
    Map {
        kind: MapKind,
        key: Box<ResolvedType>,
        value: Box<ResolvedType>,
    },
    Asset {
        type_id: Option<Uuid>,
        asset_type_id: Option<Uuid>,
    },
    Uid {
        type_id: Option<Uuid>,
    },
    ReplicatedField {
        value: Box<ResolvedType>,
    },
    RangedInteger {
        value: Box<ResolvedType>,
        min: Option<i128>,
        max: Option<i128>,
    },
    ByteStream,
    Pair {
        first: Box<ResolvedType>,
        second: Box<ResolvedType>,
    },
    Pointer {
        kind: PointerKind,
        target: Box<ResolvedType>,
    },
    Optional {
        value: Box<ResolvedType>,
    },
    Tuple {
        elements: Vec<ResolvedType>,
    },
    Unknown {
        type_id: Uuid,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnresolvedTypeRef<'a> {
    pub type_id: Uuid,
    pub reason: &'a str,
}

impl ResolvedType {
    #[must_use]
    pub fn unresolved(&self) -> Option<UnresolvedTypeRef<'_>> {
        match self {
            Self::Unknown { type_id, reason } => Some(UnresolvedTypeRef {
                type_id: *type_id,
                reason,
            }),
            Self::Sequence { element, .. } => element.unresolved(),
            Self::Map { key, value, .. }
            | Self::Pair {
                first: key,
                second: value,
            } => key.unresolved().or_else(|| value.unresolved()),
            Self::RangedInteger { value, .. } | Self::Optional { value } => value.unresolved(),
            Self::Pointer { target, .. } => target.unresolved(),
            Self::ReplicatedField { value } => value.unresolved(),
            Self::Tuple { elements } => elements.iter().find_map(Self::unresolved),
            Self::Scalar(_)
            | Self::Named { .. }
            | Self::Asset { .. }
            | Self::Uid { .. }
            | Self::ByteStream => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalarType {
    Char,
    SignedChar,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    UnsignedLong,
    F32,
    F64,
    Bool,
    Uuid,
    Crc32,
    EntityId,
    AssetId,
    Vector2,
    Vector3,
    Vector4,
    Quaternion,
    Transform,
    Color,
    ColorF,
    ColorB,
    String,
}

impl ScalarType {
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Char => "char",
            Self::SignedChar => "signed char",
            Self::I8 => "s8",
            Self::U8 => "u8",
            Self::I16 => "s16",
            Self::U16 => "u16",
            Self::I32 => "s32",
            Self::U32 => "u32",
            Self::I64 => "s64",
            Self::U64 => "u64",
            Self::UnsignedLong => "unsigned long",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Bool => "bool",
            Self::Uuid => "AZ::Uuid",
            Self::Crc32 => "AZ::Crc32",
            Self::EntityId => "AZ::EntityId",
            Self::AssetId => "AZ::Data::AssetId",
            Self::Vector2 => "Vector2",
            Self::Vector3 => "Vector3",
            Self::Vector4 => "Vector4",
            Self::Quaternion => "Quaternion",
            Self::Transform => "Transform",
            Self::Color => "Color",
            Self::ColorF => "ColorF",
            Self::ColorB => "ColorB",
            Self::String => "AZStd::string",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceKind {
    Vector,
    FixedVector,
    Array,
    List,
    ForwardList,
    Set,
    UnorderedSet,
    BitSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    Map,
    UnorderedMap,
    UnorderedFlatMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Unique,
    Shared,
    Intrusive,
}

#[derive(Debug, Clone)]
pub struct TypeResolver<'a> {
    model: &'a SerializeContextModel,
    bodyless_rtti_names: BTreeMap<Uuid, String>,
}

impl<'a> TypeResolver<'a> {
    #[must_use]
    pub fn new(model: &'a SerializeContextModel) -> Self {
        Self {
            model,
            bodyless_rtti_names: model
                .bodyless_rtti_types()
                .into_iter()
                .map(|(type_id, bodyless)| (type_id, bodyless.name))
                .collect(),
        }
    }

    #[must_use]
    pub fn resolve(&self, type_id: Uuid) -> ResolvedType {
        self.resolve_with_state(type_id, 0, &mut BTreeSet::new())
    }

    #[must_use]
    pub fn resolve_member_type(&self, member: &ReflectedMember) -> ResolvedType {
        if member.is_base_class
            && let Some(base_name) = member_base_type_name(member)
        {
            return ResolvedType::Named {
                type_id: member.type_id,
                source_name: base_name.to_owned(),
            };
        }
        if let Some(enum_type_id) = member.enum_type_id()
            && let Some(name) = self.model.type_name(enum_type_id)
        {
            return ResolvedType::Named {
                type_id: enum_type_id,
                source_name: name.to_owned(),
            };
        }
        self.resolve_member(member, 0, &mut BTreeSet::new())
    }

    fn resolve_with_state(
        &self,
        type_id: Uuid,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        if type_id.is_nil() {
            return ResolvedType::Unknown {
                type_id,
                reason: "nil type id has no stable reflected type".to_owned(),
            };
        }
        if let Some(scalar) = scalar_type(type_id) {
            return ResolvedType::Scalar(scalar);
        }
        if depth >= MAX_TYPE_DEPTH {
            return ResolvedType::Unknown {
                type_id,
                reason: "type recursion depth limit reached".to_owned(),
            };
        }
        if !visiting.insert(type_id) {
            return ResolvedType::Unknown {
                type_id,
                reason: "recursive type reference".to_owned(),
            };
        }

        let resolved = if let Some(generic) = self.model.generic_class(type_id) {
            self.resolve_generic(generic, depth + 1, visiting)
        } else if let Some(name) = self.model.type_name(type_id) {
            ResolvedType::Named {
                type_id,
                source_name: name.to_owned(),
            }
        } else if let Some(name) = self.bodyless_rtti_names.get(&type_id) {
            ResolvedType::Named {
                type_id,
                source_name: name.clone(),
            }
        } else if let Some(source_name) = native_reflected_type_name(type_id) {
            ResolvedType::Unknown {
                type_id,
                reason: format!(
                    "reflected type `{source_name}` is known but has no reflected class body"
                ),
            }
        } else {
            ResolvedType::Unknown {
                type_id,
                reason: "type id is not present in SerializeContext".to_owned(),
            }
        };

        visiting.remove(&type_id);
        resolved
    }

    fn resolve_generic(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let Some(base_name) = generic.class_name.as_deref() else {
            return self.unknown_generic(generic, "generic class has no classData.name");
        };

        match base_name {
            "AZStd::basic_string" | "AZStd::string" => ResolvedType::Scalar(ScalarType::String),
            "AZStd::vector" => {
                self.resolve_sequence(generic, SequenceKind::Vector, depth, visiting)
            }
            "AZStd::fixed_vector" => {
                self.resolve_sequence(generic, SequenceKind::FixedVector, depth, visiting)
            }
            "AZStd::array" => self.resolve_sequence(generic, SequenceKind::Array, depth, visiting),
            "AZStd::list" => self.resolve_sequence(generic, SequenceKind::List, depth, visiting),
            "AZStd::forward_list" => {
                self.resolve_sequence(generic, SequenceKind::ForwardList, depth, visiting)
            }
            "AZStd::set" => self.resolve_sequence(generic, SequenceKind::Set, depth, visiting),
            "AZStd::unordered_set" => {
                self.resolve_sequence(generic, SequenceKind::UnorderedSet, depth, visiting)
            }
            "BitSet" => ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::Bool)),
                capacity: generic.non_type_capacity(),
            },
            "ByteStream" => ResolvedType::ByteStream,
            "AZStd::ranged_int" => self.resolve_ranged_integer(generic, depth, visiting),
            "AZStd::map" => self.resolve_map(generic, MapKind::Map, depth, visiting),
            "AZStd::unordered_map" => {
                self.resolve_map(generic, MapKind::UnorderedMap, depth, visiting)
            }
            "AZStd::unordered_flat_map" => {
                self.resolve_map(generic, MapKind::UnorderedFlatMap, depth, visiting)
            }
            "Asset" | "AZ::Data::Asset" => ResolvedType::Asset {
                type_id: generic.type_id.or(generic.specialized_type_id),
                asset_type_id: generic.argument_type_ids().first().copied(),
            },
            "Amazon::Pervasives::UID" => ResolvedType::Uid {
                type_id: generic.type_id.or(generic.specialized_type_id),
            },
            "MB::ReplicatedField" => self.resolve_replicated_field(generic, depth, visiting),
            "AZStd::pair" => self.resolve_pair(generic, depth, visiting),
            "AZStd::unique_ptr" => {
                self.resolve_pointer(generic, PointerKind::Unique, depth, visiting)
            }
            "AZStd::shared_ptr" => {
                self.resolve_pointer(generic, PointerKind::Shared, depth, visiting)
            }
            "AZStd::intrusive_ptr" => {
                self.resolve_pointer(generic, PointerKind::Intrusive, depth, visiting)
            }
            "AZStd::optional" => self.resolve_optional(generic, depth, visiting),
            "Internal::RValueToLValueWrapper" => {
                self.resolve_transparent_value_wrapper(generic, depth, visiting)
            }
            "AZStd::tuple" => ResolvedType::Tuple {
                elements: self.resolve_argument_types(generic, depth, visiting),
            },
            _ => generic
                .concrete_type_ids()
                .find_map(|type_id| {
                    self.model
                        .type_name(type_id)
                        .map(|name| ResolvedType::Named {
                            type_id,
                            source_name: name.to_owned(),
                        })
                })
                .unwrap_or_else(|| self.unknown_generic(generic, "unsupported generic class")),
        }
    }

    fn resolve_replicated_field(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let value = self
            .member_named(generic, "value")
            .map(|member| self.resolve_member(member, depth, visiting))
            .or_else(|| {
                generic
                    .argument_type_ids()
                    .first()
                    .copied()
                    .map(|type_id| self.resolve_with_state(type_id, depth, visiting))
            })
            .unwrap_or_else(|| self.unknown_generic(generic, "replicated field has no value"));
        ResolvedType::ReplicatedField {
            value: Box::new(value),
        }
    }

    fn resolve_sequence(
        &self,
        generic: &ReflectedGenericClass,
        kind: SequenceKind,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let element = self
            .sequence_element(generic, depth, visiting)
            .unwrap_or_else(|| self.unknown_generic(generic, "sequence has no element type"));
        ResolvedType::Sequence {
            kind,
            element: Box::new(element),
            capacity: generic.non_type_capacity(),
        }
    }

    fn resolve_map(
        &self,
        generic: &ReflectedGenericClass,
        kind: MapKind,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        if let Some(ResolvedType::Pair { first, second }) = self
            .member_named(generic, "element")
            .and_then(|member| member.generic_class.as_deref())
            .map(|pair| self.resolve_pair(pair, depth, visiting))
        {
            return ResolvedType::Map {
                kind,
                key: first,
                value: second,
            };
        }

        let args = self.resolve_argument_types(generic, depth, visiting);
        let [key, value, ..] = args.as_slice() else {
            return self.unknown_generic(generic, "map has no key/value argument types");
        };
        ResolvedType::Map {
            kind,
            key: Box::new(key.clone()),
            value: Box::new(value.clone()),
        }
    }

    fn resolve_ranged_integer(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let value = self
            .sequence_element(generic, depth, visiting)
            .unwrap_or_else(|| self.unknown_generic(generic, "ranged_int has no value type"));
        let (min, max) = generic
            .non_type_integer_bounds()
            .map_or((None, None), |(min, max)| (Some(min), Some(max)));
        ResolvedType::RangedInteger {
            value: Box::new(value),
            min,
            max,
        }
    }

    fn resolve_pair(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let first = self
            .member_named(generic, "value1")
            .map(|member| self.resolve_member(member, depth, visiting));
        let second = self
            .member_named(generic, "value2")
            .map(|member| self.resolve_member(member, depth, visiting));

        match (first, second) {
            (Some(first), Some(second)) => ResolvedType::Pair {
                first: Box::new(first),
                second: Box::new(second),
            },
            _ => {
                let args = self.resolve_argument_types(generic, depth, visiting);
                let [first, second, ..] = args.as_slice() else {
                    return self.unknown_generic(generic, "pair has no value1/value2 types");
                };
                ResolvedType::Pair {
                    first: Box::new(first.clone()),
                    second: Box::new(second.clone()),
                }
            }
        }
    }

    fn resolve_pointer(
        &self,
        generic: &ReflectedGenericClass,
        kind: PointerKind,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let target = self
            .sequence_element(generic, depth, visiting)
            .unwrap_or_else(|| self.unknown_generic(generic, "pointer has no target type"));
        ResolvedType::Pointer {
            kind,
            target: Box::new(target),
        }
    }

    fn resolve_optional(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        let value = self
            .sequence_element(generic, depth, visiting)
            .unwrap_or_else(|| self.unknown_generic(generic, "optional has no value type"));
        ResolvedType::Optional {
            value: Box::new(value),
        }
    }

    fn resolve_transparent_value_wrapper(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        self.member_named(generic, "value")
            .map(|member| self.resolve_member(member, depth, visiting))
            .or_else(|| {
                generic
                    .argument_type_ids()
                    .first()
                    .copied()
                    .map(|type_id| self.resolve_with_state(type_id, depth, visiting))
            })
            .unwrap_or_else(|| self.unknown_generic(generic, "transparent wrapper has no value"))
    }

    fn sequence_element(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Option<ResolvedType> {
        self.member_named(generic, "element")
            .or_else(|| generic.members.first())
            .map(|member| self.resolve_member(member, depth, visiting))
            .or_else(|| {
                generic
                    .argument_type_ids()
                    .first()
                    .copied()
                    .map(|type_id| self.resolve_with_state(type_id, depth, visiting))
            })
    }

    fn resolve_argument_types(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Vec<ResolvedType> {
        generic
            .argument_type_ids()
            .into_iter()
            .map(|type_id| self.resolve_with_state(type_id, depth, visiting))
            .collect()
    }

    fn resolve_member(
        &self,
        member: &ReflectedMember,
        depth: usize,
        visiting: &mut BTreeSet<Uuid>,
    ) -> ResolvedType {
        if let Some(generic) = member.generic_class.as_deref() {
            return self.resolve_generic(generic, depth + 1, visiting);
        }

        let resolved = self.resolve_with_state(member.type_id, depth, visiting);
        if !matches!(resolved, ResolvedType::Unknown { .. }) {
            return resolved;
        }

        member_type_name(member)
            .map(|source_name| ResolvedType::Unknown {
                type_id: member.type_id,
                reason: format!(
                    "reflected type `{source_name}` is present on member type metadata but not as a SerializeContext class"
                ),
            })
            .unwrap_or(resolved)
    }

    fn member_named<'b>(
        &self,
        generic: &'b ReflectedGenericClass,
        name: &str,
    ) -> Option<&'b ReflectedMember> {
        generic.members.iter().find(|member| member.name == name)
    }

    fn unknown_generic(&self, generic: &ReflectedGenericClass, reason: &str) -> ResolvedType {
        ResolvedType::Unknown {
            type_id: generic
                .concrete_type_ids()
                .next()
                .unwrap_or_else(|| Uuid::from_u128(0)),
            reason: reason.to_owned(),
        }
    }
}

fn member_base_type_name(member: &ReflectedMember) -> Option<&str> {
    member_type_name(member)
}

fn member_type_name(member: &ReflectedMember) -> Option<&str> {
    let rtti = member.az_rtti.as_ref()?;
    rtti.type_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .or_else(|| {
            rtti.hierarchy
                .iter()
                .find(|entry| entry.type_id == member.type_id)
                .and_then(|entry| entry.type_name.as_deref())
                .filter(|name| !name.is_empty())
        })
}

#[must_use]
pub fn scalar_type(type_id: Uuid) -> Option<ScalarType> {
    match type_id {
        type_ids::CHAR => Some(ScalarType::Char),
        type_ids::SIGNED_CHAR => Some(ScalarType::SignedChar),
        type_ids::S8 => Some(ScalarType::I8),
        type_ids::U8 => Some(ScalarType::U8),
        type_ids::SHORT => Some(ScalarType::I16),
        type_ids::U16 => Some(ScalarType::U16),
        type_ids::INT => Some(ScalarType::I32),
        type_ids::U32 => Some(ScalarType::U32),
        type_ids::LONG | type_ids::S64 => Some(ScalarType::I64),
        type_ids::U64 => Some(ScalarType::U64),
        type_ids::ULONG => Some(ScalarType::UnsignedLong),
        type_ids::FLOAT => Some(ScalarType::F32),
        type_ids::DOUBLE => Some(ScalarType::F64),
        type_ids::BOOL => Some(ScalarType::Bool),
        type_ids::AZ_UUID => Some(ScalarType::Uuid),
        type_ids::CRC32 => Some(ScalarType::Crc32),
        type_ids::ENTITY_ID => Some(ScalarType::EntityId),
        type_ids::AZ_DATA_ASSET_ID => Some(ScalarType::AssetId),
        type_ids::VECTOR2 => Some(ScalarType::Vector2),
        type_ids::VECTOR3 => Some(ScalarType::Vector3),
        type_ids::VECTOR4 => Some(ScalarType::Vector4),
        type_ids::QUATERNION => Some(ScalarType::Quaternion),
        type_ids::TRANSFORM => Some(ScalarType::Transform),
        type_ids::COLOR => Some(ScalarType::Color),
        type_ids::COLORF => Some(ScalarType::ColorF),
        type_ids::COLORB => Some(ScalarType::ColorB),
        type_ids::AZSTD_BASIC_STRING
        | type_ids::AZSTD_STRING
        | type_ids::AZSTD_STRING_LEGACY_XML => Some(ScalarType::String),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nw_objectstream::type_uuid::type_ids;
    use uuid::uuid;

    use crate::document::SerializeContextDocument;
    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn resolves_az_math_type_ids_as_semantic_scalars() {
        assert_eq!(scalar_type(type_ids::VECTOR2), Some(ScalarType::Vector2));
        assert_eq!(scalar_type(type_ids::VECTOR3), Some(ScalarType::Vector3));
        assert_eq!(scalar_type(type_ids::VECTOR4), Some(ScalarType::Vector4));
        assert_eq!(
            scalar_type(type_ids::QUATERNION),
            Some(ScalarType::Quaternion)
        );
        assert_eq!(
            scalar_type(type_ids::TRANSFORM),
            Some(ScalarType::Transform)
        );
        assert_eq!(scalar_type(type_ids::COLOR), Some(ScalarType::Color));
        assert_eq!(scalar_type(type_ids::COLORF), Some(ScalarType::ColorF));
        assert_eq!(scalar_type(type_ids::COLORB), Some(ScalarType::ColorB));
        assert_eq!(
            scalar_type(type_ids::AZSTD_BASIC_STRING),
            Some(ScalarType::String)
        );
    }

    #[test]
    fn resolves_member_rtti_names_inside_generic_elements() {
        let document = SerializeContextDocument::from_slice(
            serde_json::json!({
                "$id": 1,
                "uuidMap": {
                    "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa": {
                        "$id": 10,
                        "name": "Owner",
                        "typeId": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                        "version": 1,
                        "factory": null,
                        "persistentId": null,
                        "doSave": null,
                        "serializer": null,
                        "eventHandler": null,
                        "container": null,
                        "converter": null,
                        "dataConverter": null,
                        "azRtti": null,
                        "editData": null,
                        "elements": [{
                            "$id": 11,
                            "name": "m_values",
                            "nameCrc": 0,
                            "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                            "dataSize": "32",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": false,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "azRtti": null,
                            "genericClassInfo": {
                                "$id": 12,
                                "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                                "registeredTypeIds": ["bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"],
                                "templatedArgumentCount": 1,
                                "templatedTypeIds": ["cccccccc-cccc-cccc-cccc-cccccccccccc"],
                                "typeIdFoldTypeIds": null,
                                "specializedTypeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                                "genericTypeId": "2BADE35A-6F1B-4698-B2BC-3373D010020C",
                                "legacySpecializedTypeId": null,
                                "nonTypeTemplateArguments": null,
                                "classData": {
                                    "$id": 13,
                                    "name": "AZStd::vector",
                                    "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                                    "version": 0,
                                    "doSave": null,
                                    "dataConverter": null,
                                    "editData": null,
                                    "elements": [],
                                    "attributes": []
                                },
                                "elements": [{
                                    "$id": 14,
                                    "name": "element",
                                    "nameCrc": 0,
                                    "typeId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                                    "dataSize": "16",
                                    "offset": "0",
                                    "attributeOwnership": 1,
                                    "flags": 0,
                                    "is_pointer": false,
                                    "is_base_class": false,
                                    "no_default_value": false,
                                    "is_dynamic_field": false,
                                    "is_ui_element": false,
                                    "azRtti": {
                                        "$id": 15,
                                        "typeId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                                        "typeName": "ClientView",
                                        "hierarchy": [{
                                            "typeId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                                            "typeName": "ClientView"
                                        }],
                                        "isAbstract": false
                                    },
                                    "genericClassInfo": null,
                                    "editData": null,
                                    "attributes": []
                                }]
                            },
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("schema document");
        let model = SerializeContextModel::from_document(&document);
        let owner = model
            .classes
            .get(&uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))
            .expect("owner class");
        let resolved = TypeResolver::new(&model).resolve_member_type(&owner.members[0]);

        let ResolvedType::Sequence { element, .. } = resolved else {
            panic!("expected vector");
        };
        assert_eq!(
            *element,
            ResolvedType::Unknown {
                type_id: uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                reason:
                    "reflected type `ClientView` is present on member type metadata but not as a SerializeContext class"
                        .to_owned(),
            }
        );
    }

    #[test]
    fn resolves_project_nested_map_generics_through_element_pair_shape() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("resources")
            .join("serialize.json");
        let document = SerializeContextDocument::from_path(path)
            .expect("project serialize.json should match generated schema");
        let model = SerializeContextModel::from_document(&document);

        let resolved =
            TypeResolver::new(&model).resolve(uuid!("CAB9E1F5-761E-54B8-916E-E7FB597E5EDE"));

        let ResolvedType::Map { kind, key, value } = resolved else {
            panic!("expected outer map");
        };
        assert_eq!(kind, MapKind::UnorderedMap);
        assert_eq!(*key, ResolvedType::Scalar(ScalarType::EntityId));

        let ResolvedType::Map {
            kind: inner_kind,
            key: inner_key,
            value: inner_value,
        } = *value
        else {
            panic!("expected inner map value");
        };
        assert_eq!(inner_kind, MapKind::UnorderedMap);
        assert_eq!(*inner_value, ResolvedType::Scalar(ScalarType::U8));
        assert!(matches!(
            *inner_key,
            ResolvedType::Named {
                ref source_name,
                ..
            } if source_name == "AddressType"
        ));
    }

    #[test]
    fn resolves_schema_non_type_values_for_bitset_and_ranged_int() {
        let document = SerializeContextDocument::from_slice(
            serde_json::json!({
                "$id": 1,
                "uuidMap": {},
                "classNameToUuid": [],
                "uuidGenericMap": [
                    [
                        "11111111-1111-1111-1111-111111111111",
                        {
                            "$id": 10,
                            "typeId": "11111111-1111-1111-1111-111111111111",
                            "registeredTypeIds": ["11111111-1111-1111-1111-111111111111"],
                            "templatedArgumentCount": 0,
                            "templatedTypeIds": [],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": "11111111-1111-1111-1111-111111111111",
                            "genericTypeId": "22222222-2222-2222-2222-222222222222",
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": {"values": [64]},
                            "classData": {
                                "$id": 11,
                                "name": "BitSet",
                                "typeId": "11111111-1111-1111-1111-111111111111",
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": []
                        }
                    ],
                    [
                        "33333333-3333-3333-3333-333333333333",
                        {
                            "$id": 20,
                            "typeId": "33333333-3333-3333-3333-333333333333",
                            "registeredTypeIds": ["33333333-3333-3333-3333-333333333333"],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::U16.hyphenated().to_string()],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": "33333333-3333-3333-3333-333333333333",
                            "genericTypeId": "44444444-4444-4444-4444-444444444444",
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": {"values": [0, 65535]},
                            "classData": {
                                "$id": 21,
                                "name": "AZStd::ranged_int",
                                "typeId": "33333333-3333-3333-3333-333333333333",
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": [{
                                "$id": 22,
                                "name": "value",
                                "nameCrc": 0,
                                "typeId": type_ids::U16.hyphenated().to_string(),
                                "dataSize": "2",
                                "offset": "0",
                                "attributeOwnership": 0,
                                "flags": 0,
                                "is_pointer": false,
                                "is_base_class": false,
                                "no_default_value": false,
                                "is_dynamic_field": false,
                                "is_ui_element": false,
                                "genericClassInfo": null,
                                "editData": null,
                                "attributes": []
                            }]
                        }
                    ]
                ],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("schema document");
        let model = SerializeContextModel::from_document(&document);

        assert_eq!(
            TypeResolver::new(&model).resolve(uuid!("11111111-1111-1111-1111-111111111111")),
            ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::Bool)),
                capacity: Some(64),
            }
        );
        assert_eq!(
            TypeResolver::new(&model).resolve(uuid!("33333333-3333-3333-3333-333333333333")),
            ResolvedType::RangedInteger {
                value: Box::new(ResolvedType::Scalar(ScalarType::U16)),
                min: Some(0),
                max: Some(65535),
            }
        );
    }

    #[test]
    fn resolves_byte_stream_and_transparent_value_wrappers() {
        let document = SerializeContextDocument::from_slice(
            serde_json::json!({
                "$id": 1,
                "uuidMap": {},
                "classNameToUuid": [],
                "uuidGenericMap": [
                    [
                        "11111111-1111-1111-1111-111111111111",
                        {
                            "$id": 10,
                            "typeId": "11111111-1111-1111-1111-111111111111",
                            "registeredTypeIds": ["11111111-1111-1111-1111-111111111111"],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::U8.hyphenated().to_string()],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": "11111111-1111-1111-1111-111111111111",
                            "genericTypeId": "22222222-2222-2222-2222-222222222222",
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": null,
                            "classData": {
                                "$id": 11,
                                "name": "ByteStream",
                                "typeId": "11111111-1111-1111-1111-111111111111",
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": []
                        }
                    ],
                    [
                        "33333333-3333-3333-3333-333333333333",
                        {
                            "$id": 20,
                            "typeId": "33333333-3333-3333-3333-333333333333",
                            "registeredTypeIds": ["33333333-3333-3333-3333-333333333333"],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::U16.hyphenated().to_string()],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": "33333333-3333-3333-3333-333333333333",
                            "genericTypeId": "44444444-4444-4444-4444-444444444444",
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": null,
                            "classData": {
                                "$id": 21,
                                "name": "Internal::RValueToLValueWrapper",
                                "typeId": "33333333-3333-3333-3333-333333333333",
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": [{
                                "$id": 22,
                                "name": "value",
                                "nameCrc": 0,
                                "typeId": type_ids::U16.hyphenated().to_string(),
                                "dataSize": "2",
                                "offset": "0",
                                "attributeOwnership": 1,
                                "flags": 0,
                                "is_pointer": false,
                                "is_base_class": false,
                                "no_default_value": false,
                                "is_dynamic_field": false,
                                "is_ui_element": false,
                                "genericClassInfo": null,
                                "editData": null,
                                "attributes": []
                            }]
                        }
                    ]
                ],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("schema document");
        let model = SerializeContextModel::from_document(&document);

        assert_eq!(
            TypeResolver::new(&model).resolve(uuid!("11111111-1111-1111-1111-111111111111")),
            ResolvedType::ByteStream
        );
        assert_eq!(
            TypeResolver::new(&model).resolve(uuid!("33333333-3333-3333-3333-333333333333")),
            ResolvedType::Scalar(ScalarType::U16)
        );
    }

    #[test]
    fn resolves_semantic_asset_uid_and_replicated_field_generics() {
        let asset_type_id = uuid!("77a19d40-8731-4d3c-9041-1b43047366a4");
        let uid_type_id = uuid!("3485f20a-98c0-5315-876b-21bcd23a7bc0");
        let replicated_type_id = uuid!("44bc0c45-da18-5e2c-9d9d-943f964cb90c");
        let document = SerializeContextDocument::from_slice(
            serde_json::json!({
                "$id": 1,
                "uuidMap": {},
                "classNameToUuid": [],
                "uuidGenericMap": [
                    [
                        asset_type_id.hyphenated().to_string(),
                        {
                            "$id": 10,
                            "typeId": asset_type_id.hyphenated().to_string(),
                            "registeredTypeIds": ["58D8A471-1618-5B3E-8CDD-EDFD88319C83"],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::AZ_UUID.hyphenated().to_string()],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": asset_type_id.hyphenated().to_string(),
                            "genericTypeId": asset_type_id.hyphenated().to_string(),
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": null,
                            "classData": {
                                "$id": 11,
                                "name": "Asset",
                                "typeId": asset_type_id.hyphenated().to_string(),
                                "version": 1,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": []
                        }
                    ],
                    [
                        uid_type_id.hyphenated().to_string(),
                        {
                            "$id": 20,
                            "typeId": uid_type_id.hyphenated().to_string(),
                            "registeredTypeIds": [uid_type_id.hyphenated().to_string()],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::U16.hyphenated().to_string()],
                            "typeIdFoldTypeIds": null,
                            "specializedTypeId": uid_type_id.hyphenated().to_string(),
                            "genericTypeId": null,
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": null,
                            "classData": {
                                "$id": 21,
                                "name": "Amazon::Pervasives::UID",
                                "typeId": uid_type_id.hyphenated().to_string(),
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": []
                        }
                    ],
                    [
                        replicated_type_id.hyphenated().to_string(),
                        {
                            "$id": 30,
                            "typeId": replicated_type_id.hyphenated().to_string(),
                            "registeredTypeIds": [replicated_type_id.hyphenated().to_string()],
                            "templatedArgumentCount": 1,
                            "templatedTypeIds": [type_ids::INT.hyphenated().to_string()],
                            "typeIdFoldTypeIds": ["5C059EC7-44B0-4666-9FC9-674192338F39"],
                            "specializedTypeId": replicated_type_id.hyphenated().to_string(),
                            "genericTypeId": "CCF0F660-785B-4056-9285-E6FA70557850",
                            "legacySpecializedTypeId": null,
                            "nonTypeTemplateArguments": null,
                            "classData": {
                                "$id": 31,
                                "name": "MB::ReplicatedField",
                                "typeId": replicated_type_id.hyphenated().to_string(),
                                "version": 0,
                                "doSave": null,
                                "dataConverter": null,
                                "editData": null,
                                "elements": [],
                                "attributes": []
                            },
                            "elements": [{
                                "$id": 32,
                                "name": "value",
                                "nameCrc": 0,
                                "typeId": type_ids::INT.hyphenated().to_string(),
                                "dataSize": "4",
                                "offset": "0",
                                "attributeOwnership": 0,
                                "flags": 0,
                                "is_pointer": false,
                                "is_base_class": false,
                                "no_default_value": false,
                                "is_dynamic_field": false,
                                "is_ui_element": false,
                                "genericClassInfo": null,
                                "editData": null,
                                "attributes": []
                            }]
                        }
                    ]
                ],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("schema document");
        let model = SerializeContextModel::from_document(&document);

        assert_eq!(
            TypeResolver::new(&model).resolve(asset_type_id),
            ResolvedType::Asset {
                type_id: Some(asset_type_id),
                asset_type_id: Some(type_ids::AZ_UUID),
            }
        );
        assert_eq!(
            TypeResolver::new(&model).resolve(uid_type_id),
            ResolvedType::Uid {
                type_id: Some(uid_type_id),
            }
        );
        assert_eq!(
            TypeResolver::new(&model).resolve(replicated_type_id),
            ResolvedType::ReplicatedField {
                value: Box::new(ResolvedType::Scalar(ScalarType::I32)),
            }
        );
    }

    #[test]
    fn resolves_base_class_edges_from_edge_local_rtti_name() {
        let document = SerializeContextDocument::from_slice(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "401EA5B5-DDE2-4848-BE17-FD45660FF8C5": {
                        "$id": 10,
                        "name": "ActionCondition",
                        "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                        "version": 0,
                        "factory": "NewWorld+0x10",
                        "persistentId": null,
                        "doSave": null,
                        "serializer": null,
                        "eventHandler": null,
                        "container": null,
                        "azRtti": {
                            "address": "NewWorld+0x20",
                            "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                            "typeName": "ActionCondition",
                            "hierarchy": [{
                                "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                "typeName": "ActionCondition"
                            }],
                            "isAbstract": true
                        },
                        "dataConverter": null,
                        "editData": null,
                        "elements": [],
                        "attributes": []
                    },
                    "D490719F-8531-4F82-A64E-EE29DC6AEA50": {
                        "$id": 20,
                        "name": "SetMannequinTagData",
                        "typeId": "D490719F-8531-4F82-A64E-EE29DC6AEA50",
                        "version": 0,
                        "factory": "NewWorld+0x30",
                        "persistentId": null,
                        "doSave": null,
                        "serializer": null,
                        "eventHandler": null,
                        "container": null,
                        "azRtti": {
                            "address": "NewWorld+0x40",
                            "typeId": "D490719F-8531-4F82-A64E-EE29DC6AEA50",
                            "typeName": "SetMannequinTagData",
                            "hierarchy": [{
                                "typeId": "D490719F-8531-4F82-A64E-EE29DC6AEA50",
                                "typeName": "SetMannequinTagData"
                            }, {
                                "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                "typeName": "ActivityData"
                            }],
                            "isAbstract": false
                        },
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 21,
                            "name": "BaseClass1",
                            "nameCrc": 0,
                            "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                            "dataSize": "1",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 0,
                            "is_pointer": false,
                            "is_base_class": true,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "azRtti": {
                                "address": "NewWorld+0x50",
                                "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                "typeName": "ActivityData",
                                "hierarchy": [{
                                    "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                    "typeName": "ActivityData"
                                }],
                                "isAbstract": true
                            },
                            "genericClassInfo": null,
                            "editData": null,
                            "attributes": []
                        }],
                        "attributes": []
                    }
                },
                "classNameToUuid": [],
                "uuidGenericMap": [],
                "uuidAnyCreationMap": {},
                "editContext": {"$id": 2, "classData": [], "enumData": []},
                "enumTypeIdToUnderlyingTypeIdMap": {}
            }"#,
        )
        .expect("schema document");
        let model = SerializeContextModel::from_document(&document);
        let class = model
            .classes
            .get(&uuid!("D490719F-8531-4F82-A64E-EE29DC6AEA50"))
            .expect("SetMannequinTagData class");
        let member = class.members.first().expect("base class member");

        assert_eq!(
            TypeResolver::new(&model).resolve_member_type(member),
            ResolvedType::Named {
                type_id: uuid!("401EA5B5-DDE2-4848-BE17-FD45660FF8C5"),
                source_name: "ActivityData".to_owned()
            }
        );
    }
}
