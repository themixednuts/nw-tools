use std::collections::BTreeMap;

use uuid::Uuid;

use crate::naming::{missing_reflected_type_name, rust_type_name};
use crate::types::{MapKind, ResolvedType, ScalarType, SequenceKind};

const AZ_BITSET_WORD_BITS: usize = u32::BITS as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustTypeOptions {
    pub use_support_aliases: bool,
    pub uuid_alias: &'static str,
    pub crc32_alias: &'static str,
    pub entity_id_alias: &'static str,
    pub asset_id_alias: &'static str,
    pub asset_alias: &'static str,
    pub uid_alias: &'static str,
    pub replicated_field_alias: &'static str,
}

impl Default for RustTypeOptions {
    fn default() -> Self {
        Self {
            use_support_aliases: true,
            uuid_alias: "Uuid",
            crc32_alias: "Crc32",
            entity_id_alias: "EntityId",
            asset_id_alias: "AssetId",
            asset_alias: "Asset",
            uid_alias: "Uid",
            replicated_field_alias: "ReplicatedField",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RustTypeRenderer {
    options: RustTypeOptions,
}

impl RustTypeRenderer {
    #[must_use]
    pub const fn new(options: RustTypeOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub fn render(&self, resolved: &ResolvedType) -> String {
        self.render_with_names(resolved, &BTreeMap::new())
    }

    #[must_use]
    pub fn render_with_names(
        &self,
        resolved: &ResolvedType,
        names_by_type_id: &BTreeMap<Uuid, String>,
    ) -> String {
        match resolved {
            ResolvedType::Scalar(scalar) => self.render_scalar(*scalar).to_owned(),
            ResolvedType::Named {
                type_id,
                source_name,
            } => names_by_type_id
                .get(type_id)
                .cloned()
                .unwrap_or_else(|| rust_type_name(source_name)),
            ResolvedType::Sequence {
                kind,
                element,
                capacity,
            } => self.render_sequence(*kind, element, *capacity, names_by_type_id),
            ResolvedType::Map { kind, key, value } => {
                self.render_map(*kind, key, value, names_by_type_id)
            }
            ResolvedType::Asset { .. } => self.render_asset().to_owned(),
            ResolvedType::Uid { .. } => self.render_uid().to_owned(),
            ResolvedType::ReplicatedField { value } => {
                let value = self.render_with_names(value, names_by_type_id);
                format!("{}<{value}>", self.render_replicated_field())
            }
            ResolvedType::RangedInteger { value, .. } => {
                self.render_with_names(value, names_by_type_id)
            }
            ResolvedType::ByteStream => "Vec<u8>".to_owned(),
            ResolvedType::Pair { first, second } => {
                format!(
                    "({}, {})",
                    self.render_with_names(first, names_by_type_id),
                    self.render_with_names(second, names_by_type_id)
                )
            }
            ResolvedType::Pointer { target, .. } => {
                format!(
                    "Option<{}>",
                    self.render_with_names(target, names_by_type_id)
                )
            }
            ResolvedType::Optional { value } => {
                format!(
                    "Option<{}>",
                    self.render_with_names(value, names_by_type_id)
                )
            }
            ResolvedType::Tuple { elements } => self.render_tuple(elements, names_by_type_id),
            ResolvedType::Unknown { type_id, .. } => missing_reflected_type_name(*type_id),
        }
    }

    fn render_scalar(&self, scalar: ScalarType) -> &'static str {
        match scalar {
            ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => "i8",
            ScalarType::U8 => "u8",
            ScalarType::I16 => "i16",
            ScalarType::U16 => "u16",
            ScalarType::I32 => "i32",
            ScalarType::U32 => "u32",
            ScalarType::I64 => "i64",
            ScalarType::U64 | ScalarType::UnsignedLong => "u64",
            ScalarType::F32 => "f32",
            ScalarType::F64 => "f64",
            ScalarType::Bool => "bool",
            ScalarType::Uuid if self.options.use_support_aliases => self.options.uuid_alias,
            ScalarType::Uuid => "uuid::Uuid",
            ScalarType::Crc32 if self.options.use_support_aliases => self.options.crc32_alias,
            ScalarType::Crc32 => "az_core::crc::Crc32",
            ScalarType::EntityId if self.options.use_support_aliases => {
                self.options.entity_id_alias
            }
            ScalarType::EntityId => "az_core::EntityId",
            ScalarType::AssetId if self.options.use_support_aliases => self.options.asset_id_alias,
            ScalarType::AssetId => "az_asset::AssetId",
            ScalarType::Vector2 if self.options.use_support_aliases => "bevy_math::Vec2",
            ScalarType::Vector2 => "bevy::math::Vec2",
            ScalarType::Vector3 if self.options.use_support_aliases => "bevy_math::Vec3",
            ScalarType::Vector3 => "bevy::math::Vec3",
            ScalarType::Vector4 if self.options.use_support_aliases => "bevy_math::Vec4",
            ScalarType::Vector4 => "bevy::math::Vec4",
            ScalarType::Quaternion if self.options.use_support_aliases => "bevy_math::Quat",
            ScalarType::Quaternion => "bevy::math::Quat",
            ScalarType::Transform if self.options.use_support_aliases => {
                "bevy_transform::components::Transform"
            }
            ScalarType::Transform => "bevy::transform::components::Transform",
            ScalarType::Color | ScalarType::ColorF if self.options.use_support_aliases => {
                "bevy_color::LinearRgba"
            }
            ScalarType::Color | ScalarType::ColorF => "bevy::color::LinearRgba",
            ScalarType::ColorB if self.options.use_support_aliases => "bevy_color::Srgba",
            ScalarType::ColorB => "bevy::color::Srgba",
            ScalarType::String => "String",
        }
    }

    fn render_asset(&self) -> &'static str {
        if self.options.use_support_aliases {
            self.options.asset_alias
        } else {
            "az_asset::UntypedAssetRef"
        }
    }

    fn render_uid(&self) -> &'static str {
        if self.options.use_support_aliases {
            self.options.uid_alias
        } else {
            "crate::refs::Uid"
        }
    }

    fn render_replicated_field(&self) -> &'static str {
        if self.options.use_support_aliases {
            self.options.replicated_field_alias
        } else {
            "gridmate::serialize::replicated_field::ReplicatedFieldHandler"
        }
    }

    fn render_sequence(
        &self,
        kind: SequenceKind,
        element: &ResolvedType,
        capacity: Option<usize>,
        names_by_type_id: &BTreeMap<Uuid, String>,
    ) -> String {
        let element = self.render_with_names(element, names_by_type_id);
        match (kind, capacity) {
            (SequenceKind::Array, Some(capacity)) => format!("[{element}; {capacity}]"),
            (SequenceKind::BitSet, _) => rust_bitset_storage_type(capacity),
            (SequenceKind::FixedVector, Some(capacity)) => {
                format!("smallvec::SmallVec<[{element}; {capacity}]>")
            }
            (SequenceKind::Set, _) => format!("std::collections::BTreeSet<{element}>"),
            (SequenceKind::UnorderedSet, _) => format!("std::collections::HashSet<{element}>"),
            _ => format!("Vec<{element}>"),
        }
    }

    fn render_map(
        &self,
        kind: MapKind,
        key: &ResolvedType,
        value: &ResolvedType,
        names_by_type_id: &BTreeMap<Uuid, String>,
    ) -> String {
        let key = self.render_with_names(key, names_by_type_id);
        let value = self.render_with_names(value, names_by_type_id);
        match kind {
            MapKind::Map => format!("std::collections::BTreeMap<{key}, {value}>"),
            MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                format!("std::collections::HashMap<{key}, {value}>")
            }
        }
    }

    fn render_tuple(
        &self,
        elements: &[ResolvedType],
        names_by_type_id: &BTreeMap<Uuid, String>,
    ) -> String {
        let elements = elements
            .iter()
            .map(|element| self.render_with_names(element, names_by_type_id))
            .collect::<Vec<_>>();
        match elements.as_slice() {
            [] => "()".to_owned(),
            [single] => format!("({single},)"),
            _ => format!(
                "({})",
                elements
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

pub(super) fn rust_bitset_storage_type(bit_capacity: Option<usize>) -> String {
    bit_capacity.map_or_else(
        || "Vec<u32>".to_owned(),
        |bit_capacity| {
            let words = bit_capacity.div_ceil(AZ_BITSET_WORD_BITS);
            format!("[u32; {words}]")
        },
    )
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::types::{MapKind, PointerKind, ResolvedType, ScalarType, SequenceKind};

    use super::*;

    #[test]
    fn renders_semantic_aliases_when_support_types_are_enabled() {
        let renderer = RustTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "EntityId"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::AssetId)),
            "AssetId"
        );
    }

    #[test]
    fn renders_engine_paths_when_support_aliases_are_disabled() {
        let renderer = RustTypeRenderer::new(RustTypeOptions {
            use_support_aliases: false,
            ..RustTypeOptions::default()
        });

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "az_core::EntityId"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Uuid)),
            "uuid::Uuid"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Vector3)),
            "bevy::math::Vec3"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Quaternion)),
            "bevy::math::Quat"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Transform)),
            "bevy::transform::components::Transform"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Color)),
            "bevy::color::LinearRgba"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::ColorB)),
            "bevy::color::Srgba"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Asset {
                type_id: Some(uuid!("77f3e5cc-1ff4-4758-8c36-08ac7f994d3c")),
                asset_type_id: None,
            }),
            "az_asset::UntypedAssetRef"
        );
    }

    #[test]
    fn renders_az_math_semantics_as_bevy_crate_types() {
        let renderer = RustTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Vector3)),
            "bevy_math::Vec3"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Quaternion)),
            "bevy_math::Quat"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Transform)),
            "bevy_transform::components::Transform"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Color)),
            "bevy_color::LinearRgba"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::ColorB)),
            "bevy_color::Srgba"
        );
    }

    #[test]
    fn renders_nested_containers_from_resolved_type_shape() {
        let renderer = RustTypeRenderer::default();
        let resolved = ResolvedType::Map {
            kind: MapKind::UnorderedMap,
            key: Box::new(ResolvedType::Scalar(ScalarType::EntityId)),
            value: Box::new(ResolvedType::Sequence {
                kind: SequenceKind::Array,
                element: Box::new(ResolvedType::Pair {
                    first: Box::new(ResolvedType::Named {
                        type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                        source_name: "Example::AddressType".to_owned(),
                    }),
                    second: Box::new(ResolvedType::Pointer {
                        kind: PointerKind::Shared,
                        target: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                    }),
                }),
                capacity: Some(2),
            }),
        };

        assert_eq!(
            renderer.render(&resolved),
            "std::collections::HashMap<EntityId, [(AddressType, Option<u8>); 2]>"
        );
    }

    #[test]
    fn renders_byte_streams_as_byte_vectors() {
        let renderer = RustTypeRenderer::default();

        assert_eq!(renderer.render(&ResolvedType::ByteStream), "Vec<u8>");
    }

    #[test]
    fn renders_bitsets_as_az_word_storage() {
        let renderer = RustTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::Bool)),
                capacity: Some(96),
            }),
            "[u32; 3]"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::Bool)),
                capacity: None,
            }),
            "Vec<u32>"
        );
    }
}
