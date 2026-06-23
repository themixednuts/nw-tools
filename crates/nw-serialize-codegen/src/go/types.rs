use crate::naming::rust_type_name;
use crate::types::{ResolvedType, ScalarType, SequenceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoTypeOptions {
    pub use_support_aliases: bool,
}

impl Default for GoTypeOptions {
    fn default() -> Self {
        Self {
            use_support_aliases: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GoTypeRenderer {
    options: GoTypeOptions,
}

impl GoTypeRenderer {
    #[must_use]
    pub const fn new(options: GoTypeOptions) -> Self {
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
            ResolvedType::Map { key, value, .. } => self.render_map(key, value),
            ResolvedType::Asset { .. } => {
                if self.options.use_support_aliases {
                    "Asset".to_owned()
                } else {
                    "az.Asset".to_owned()
                }
            }
            ResolvedType::Uid { .. } => {
                if self.options.use_support_aliases {
                    "Uuid".to_owned()
                } else {
                    "uuid.UUID".to_owned()
                }
            }
            ResolvedType::ReplicatedField { value } => {
                let value = self.render(value);
                format!("*{value}")
            }
            ResolvedType::RangedInteger { value, .. } => self.render(value),
            ResolvedType::ByteStream => "[]byte".to_owned(),
            ResolvedType::Pair { first, second } => {
                format!(
                    "struct {{ First {}; Second {} }}",
                    self.render(first),
                    self.render(second)
                )
            }
            ResolvedType::Pointer { target, .. } | ResolvedType::Optional { value: target } => {
                format!("*{}", self.render(target))
            }
            ResolvedType::Tuple { elements } => self.render_tuple(elements),
            ResolvedType::Unknown { .. } => "any".to_owned(),
        }
    }

    fn render_scalar(&self, scalar: ScalarType) -> &'static str {
        match scalar {
            ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => "int8",
            ScalarType::U8 => "uint8",
            ScalarType::I16 => "int16",
            ScalarType::U16 => "uint16",
            ScalarType::I32 => "int32",
            ScalarType::U32 => "uint32",
            ScalarType::I64 => "int64",
            ScalarType::U64 | ScalarType::UnsignedLong => "uint64",
            ScalarType::F32 => "float32",
            ScalarType::F64 => "float64",
            ScalarType::Bool => "bool",
            ScalarType::Uuid if self.options.use_support_aliases => "Uuid",
            ScalarType::Uuid => "uuid.UUID",
            ScalarType::Crc32 if self.options.use_support_aliases => "Crc32",
            ScalarType::Crc32 => "az.Crc32",
            ScalarType::EntityId => "uint64",
            ScalarType::AssetId if self.options.use_support_aliases => "AssetId",
            ScalarType::AssetId => "az.AssetId",
            ScalarType::Vector2 if self.options.use_support_aliases => "Vector2",
            ScalarType::Vector2 => "azmath.Vector2",
            ScalarType::Vector3 if self.options.use_support_aliases => "Vector3",
            ScalarType::Vector3 => "azmath.Vector3",
            ScalarType::Vector4 if self.options.use_support_aliases => "Vector4",
            ScalarType::Vector4 => "azmath.Vector4",
            ScalarType::Quaternion if self.options.use_support_aliases => "Quaternion",
            ScalarType::Quaternion => "azmath.Quaternion",
            ScalarType::Transform if self.options.use_support_aliases => "Transform",
            ScalarType::Transform => "azmath.Transform",
            ScalarType::Color if self.options.use_support_aliases => "Color",
            ScalarType::Color => "azmath.Color",
            ScalarType::ColorF if self.options.use_support_aliases => "ColorF",
            ScalarType::ColorF => "azmath.ColorF",
            ScalarType::ColorB if self.options.use_support_aliases => "ColorB",
            ScalarType::ColorB => "azmath.ColorB",
            ScalarType::String => "string",
        }
    }

    fn render_sequence(
        &self,
        kind: SequenceKind,
        element: &ResolvedType,
        capacity: Option<usize>,
    ) -> String {
        let element_type = self.render(element);
        match (kind, capacity) {
            (SequenceKind::Array | SequenceKind::BitSet, Some(capacity)) => {
                format!("[{capacity}]{element_type}")
            }
            (SequenceKind::Set | SequenceKind::UnorderedSet, _)
                if go_resolved_type_is_comparable(element) =>
            {
                format!("map[{element_type}]struct{{}}")
            }
            (SequenceKind::Set | SequenceKind::UnorderedSet, _) => format!("[]{element_type}"),
            _ => format!("[]{element_type}"),
        }
    }

    fn render_map(&self, key: &ResolvedType, value: &ResolvedType) -> String {
        if go_resolved_type_is_comparable(key) {
            let key = self.render(key);
            let value = self.render(value);
            format!("map[{key}]{value}")
        } else {
            let key = self.render(key);
            let value = self.render(value);
            format!("[]struct {{ Key {key}; Value {value} }}")
        }
    }

    fn render_tuple(&self, elements: &[ResolvedType]) -> String {
        if elements.is_empty() {
            return "struct{}".to_owned();
        }

        format!(
            "struct {{ {} }}",
            elements
                .iter()
                .enumerate()
                .map(|(index, element)| format!("Value{} {}", index + 1, self.render(element)))
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

fn go_resolved_type_is_comparable(resolved: &ResolvedType) -> bool {
    match resolved {
        ResolvedType::Scalar(_) | ResolvedType::Named { .. } | ResolvedType::Asset { .. } => true,
        ResolvedType::Uid { .. }
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::Pointer { .. }
        | ResolvedType::Optional { .. } => true,
        ResolvedType::Sequence {
            kind: SequenceKind::Array | SequenceKind::BitSet,
            element,
            capacity: Some(_),
        } => go_resolved_type_is_comparable(element),
        ResolvedType::Pair { first, second } => {
            go_resolved_type_is_comparable(first) && go_resolved_type_is_comparable(second)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(go_resolved_type_is_comparable),
        ResolvedType::RangedInteger { value, .. } => go_resolved_type_is_comparable(value),
        ResolvedType::Sequence { .. }
        | ResolvedType::Map { .. }
        | ResolvedType::ByteStream
        | ResolvedType::Unknown { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::types::{MapKind, PointerKind};

    use super::*;

    #[test]
    fn renders_semantic_aliases_when_support_types_are_enabled() {
        let renderer = GoTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "uint64"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::AssetId)),
            "AssetId"
        );
    }

    #[test]
    fn renders_engine_selectors_when_support_aliases_are_disabled() {
        let renderer = GoTypeRenderer::new(GoTypeOptions {
            use_support_aliases: false,
        });

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::EntityId)),
            "uint64"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Uuid)),
            "uuid.UUID"
        );
    }

    #[test]
    fn renders_az_math_semantics_through_support_package() {
        let renderer = GoTypeRenderer::new(GoTypeOptions {
            use_support_aliases: false,
        });

        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Vector3)),
            "azmath.Vector3"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::Transform)),
            "azmath.Transform"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Scalar(ScalarType::ColorB)),
            "azmath.ColorB"
        );
    }

    #[test]
    fn renders_nested_containers_from_resolved_type_shape() {
        let renderer = GoTypeRenderer::default();
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
            "map[uint64][2]struct { First AddressType; Second *uint8 }"
        );
    }

    #[test]
    fn renders_entry_containers_for_non_comparable_map_keys_and_sets() {
        let renderer = GoTypeRenderer::default();

        assert_eq!(
            renderer.render(&ResolvedType::Map {
                kind: MapKind::UnorderedMap,
                key: Box::new(ResolvedType::ByteStream),
                value: Box::new(ResolvedType::Scalar(ScalarType::String)),
            }),
            "[]struct { Key []byte; Value string }"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::UnorderedSet,
                element: Box::new(ResolvedType::ByteStream),
                capacity: None,
            }),
            "[][]byte"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::UnorderedSet,
                element: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                capacity: None,
            }),
            "map[uint8]struct{}"
        );
    }

    #[test]
    fn renders_inline_entry_containers_without_support_aliases() {
        let renderer = GoTypeRenderer::new(GoTypeOptions {
            use_support_aliases: false,
        });

        assert_eq!(
            renderer.render(&ResolvedType::Map {
                kind: MapKind::UnorderedMap,
                key: Box::new(ResolvedType::ByteStream),
                value: Box::new(ResolvedType::Scalar(ScalarType::String)),
            }),
            "[]struct { Key []byte; Value string }"
        );
        assert_eq!(
            renderer.render(&ResolvedType::Sequence {
                kind: SequenceKind::UnorderedSet,
                element: Box::new(ResolvedType::ByteStream),
                capacity: None,
            }),
            "[][]byte"
        );
    }

    #[test]
    fn renders_byte_streams_as_byte_slices() {
        let renderer = GoTypeRenderer::default();

        assert_eq!(renderer.render(&ResolvedType::ByteStream), "[]byte");
    }
}
