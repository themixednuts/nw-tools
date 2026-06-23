use crate::naming::rust_type_name;
use crate::types::{ResolvedType, ScalarType, SequenceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeScriptTypeOptions {
    pub use_support_aliases: bool,
}

impl Default for TypeScriptTypeOptions {
    fn default() -> Self {
        Self {
            use_support_aliases: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptTypeRenderer {
    options: TypeScriptTypeOptions,
}

impl TypeScriptTypeRenderer {
    #[must_use]
    pub const fn new(options: TypeScriptTypeOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub fn render(&self, resolved: &ResolvedType) -> String {
        match resolved {
            ResolvedType::Scalar(scalar) => self.render_scalar(*scalar).to_owned(),
            ResolvedType::Named { source_name, .. } => rust_type_name(source_name),
            ResolvedType::Sequence {
                kind,
                element,
                capacity,
            } => self.render_sequence(*kind, element, *capacity),
            ResolvedType::Map { key, value, .. } => {
                format!("Map<{}, {}>", self.render(key), self.render(value))
            }
            ResolvedType::Asset { .. } => "Asset".to_owned(),
            ResolvedType::Uid { .. } => "Uuid".to_owned(),
            ResolvedType::ReplicatedField { value } => {
                format!("{} | undefined", self.render(value))
            }
            ResolvedType::RangedInteger { value, .. } => self.render(value),
            ResolvedType::ByteStream => "Uint8Array".to_owned(),
            ResolvedType::Pair { first, second } => {
                format!("[{}, {}]", self.render(first), self.render(second))
            }
            ResolvedType::Pointer { target, .. } => self.render_nullable(target),
            ResolvedType::Optional { value } => self.render_nullable(value),
            ResolvedType::Tuple { elements } => format!(
                "[{}]",
                elements
                    .iter()
                    .map(|element| self.render(element))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ResolvedType::Unknown { .. } => "unknown".to_owned(),
        }
    }

    fn render_scalar(&self, scalar: ScalarType) -> &'static str {
        match scalar {
            ScalarType::Char
            | ScalarType::SignedChar
            | ScalarType::I8
            | ScalarType::U8
            | ScalarType::I16
            | ScalarType::U16
            | ScalarType::I32
            | ScalarType::U32
            | ScalarType::F32
            | ScalarType::F64 => "number",
            ScalarType::I64 | ScalarType::U64 | ScalarType::UnsignedLong => "bigint",
            ScalarType::Bool => "boolean",
            ScalarType::Uuid if self.options.use_support_aliases => "Uuid",
            ScalarType::Uuid => "string",
            ScalarType::Crc32 if self.options.use_support_aliases => "Crc32",
            ScalarType::Crc32 => "number",
            ScalarType::EntityId => "bigint",
            ScalarType::AssetId if self.options.use_support_aliases => "AssetId",
            ScalarType::AssetId => "{ guid: string; subId: number }",
            ScalarType::Vector2 => "Vector2",
            ScalarType::Vector3 => "Vector3",
            ScalarType::Vector4 => "Vector4",
            ScalarType::Quaternion => "Quaternion",
            ScalarType::Transform => "Transform",
            ScalarType::Color => "Color",
            ScalarType::ColorF => "ColorF",
            ScalarType::ColorB => "ColorB",
            ScalarType::String => "string",
        }
    }

    fn render_sequence(
        &self,
        kind: SequenceKind,
        element: &ResolvedType,
        capacity: Option<usize>,
    ) -> String {
        let element = self.render(element);
        match (kind, capacity) {
            (SequenceKind::Array, Some(capacity)) => {
                format!("FixedArray<{element}, {capacity}>")
            }
            (SequenceKind::FixedVector, Some(capacity)) => {
                format!("FixedVector<{element}, {capacity}>")
            }
            (SequenceKind::BitSet, Some(capacity)) => format!("BitSet<{capacity}>"),
            (SequenceKind::Set | SequenceKind::UnorderedSet, _) => format!("Set<{element}>"),
            _ => {
                let needs_parens = element_needs_array_parens(&element);
                format!("{}[]", parenthesize_array_element(element, needs_parens))
            }
        }
    }

    fn render_nullable(&self, value: &ResolvedType) -> String {
        let rendered = self.render(value);
        if rendered.ends_with(" | null") {
            rendered
        } else {
            format!("{rendered} | null")
        }
    }
}

pub(super) fn parenthesize_array_element(rendered: String, needs_parens: bool) -> String {
    if needs_parens {
        format!("({rendered})")
    } else {
        rendered
    }
}

pub(super) fn element_needs_array_parens(rendered: &str) -> bool {
    rendered.contains(" | ")
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::types::{MapKind, PointerKind};

    use super::*;

    #[test]
    fn renders_semantic_aliases_when_support_types_are_enabled() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "bigint"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::AssetId)),
            "AssetId"
        );
    }

    #[test]
    fn renders_wire_primitives_when_support_aliases_are_disabled() {
        let renderer = TypeScriptTypeRenderer::new(TypeScriptTypeOptions {
            use_support_aliases: false,
        });

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "bigint"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Uuid)),
            "string"
        );
    }

    #[test]
    fn renders_az_math_semantics_through_support_types() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Vector3)),
            "Vector3"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Transform)),
            "Transform"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::ColorB)),
            "ColorB"
        );
    }

    #[test]
    fn renders_nested_containers_from_resolved_type_shape() {
        let renderer = TypeScriptTypeRenderer::default();
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
            "Map<bigint, FixedArray<[AddressType, number | null], 2>>"
        );
    }

    #[test]
    fn renders_pointer_targets_as_nullable_values() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Pointer {
                kind: PointerKind::Shared,
                target: Box::new(ResolvedType::Named {
                    type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                    source_name: "Example::Payload".to_owned(),
                }),
            }),
            "Payload | null"
        );
    }

    #[test]
    fn does_not_duplicate_null_for_optional_pointer_targets() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Optional {
                value: Box::new(ResolvedType::Pointer {
                    kind: PointerKind::Shared,
                    target: Box::new(ResolvedType::Named {
                        type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                        source_name: "Example::Payload".to_owned(),
                    }),
                }),
            }),
            "Payload | null"
        );
    }

    #[test]
    fn renders_sized_containers_with_capacity_preserving_aliases() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::Array,
                element: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                capacity: Some(16),
            }),
            "FixedArray<number, 16>"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::FixedVector,
                element: Box::new(ResolvedType::Scalar(ScalarType::String)),
                capacity: Some(8),
            }),
            "FixedVector<string, 8>"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::Bool)),
                capacity: Some(64),
            }),
            "BitSet<64>"
        );
    }

    #[test]
    fn renders_byte_streams_as_uint8_arrays() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(renderer.render(&ResolvedType::ByteStream), "Uint8Array");
    }

    #[test]
    fn renders_maps_with_non_property_keys_as_map_containers() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Map {
                kind: MapKind::Map,
                key: Box::new(ResolvedType::Scalar(ScalarType::AssetId)),
                value: Box::new(ResolvedType::Scalar(ScalarType::String)),
            }),
            "Map<AssetId, string>"
        );
    }

    #[test]
    fn parenthesizes_optional_array_elements() {
        let renderer = TypeScriptTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::Vector,
                element: Box::new(ResolvedType::Optional {
                    value: Box::new(ResolvedType::Scalar(ScalarType::U32)),
                }),
                capacity: None,
            }),
            "(number | null)[]"
        );
    }
}
