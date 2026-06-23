use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::field_projection::{
    CodegenFieldTypeProjection, CodegenTypeReferenceProjection, classify_codegen_field_type,
    visit_codegen_item_reference_roots,
};
use crate::ir::{SerializeCodegenItem, SerializeCodegenUnit};
use crate::types::{ResolvedType, ScalarType, SequenceKind};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CodegenSupportUsage {
    pub asset: bool,
    pub asset_id: bool,
    pub crc32: bool,
    pub math_scalars: BTreeSet<ScalarType>,
    pub uuid: bool,
}

impl CodegenSupportUsage {
    #[must_use]
    pub fn any(&self) -> bool {
        self.asset || self.asset_id || self.crc32 || self.has_math() || self.uuid
    }

    #[must_use]
    pub fn has_math(&self) -> bool {
        !self.math_scalars.is_empty()
    }

    #[must_use]
    pub fn for_items<'a>(
        items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        projection: CodegenTypeReferenceProjection,
    ) -> Self {
        let mut usage = Self::default();
        for item in items {
            visit_codegen_item_reference_roots(item, items_by_type_id, projection, |resolved| {
                usage.collect_resolved_type(resolved);
            });
        }
        usage
    }

    pub fn collect_resolved_type(&mut self, resolved: &ResolvedType) {
        match resolved {
            ResolvedType::Scalar(ScalarType::AssetId) => self.asset_id = true,
            ResolvedType::Scalar(ScalarType::Crc32) => self.crc32 = true,
            ResolvedType::Scalar(ScalarType::EntityId) => {}
            ResolvedType::Scalar(
                scalar @ (ScalarType::Vector2
                | ScalarType::Vector3
                | ScalarType::Vector4
                | ScalarType::Quaternion
                | ScalarType::Transform
                | ScalarType::Color
                | ScalarType::ColorF
                | ScalarType::ColorB),
            ) => {
                self.math_scalars.insert(*scalar);
            }
            ResolvedType::Scalar(ScalarType::Uuid) => self.uuid = true,
            ResolvedType::Asset { .. } => self.asset = true,
            ResolvedType::Uid { .. } => self.uuid = true,
            ResolvedType::ReplicatedField { value }
            | ResolvedType::Sequence { element: value, .. }
            | ResolvedType::RangedInteger { value, .. }
            | ResolvedType::Pointer { target: value, .. }
            | ResolvedType::Optional { value } => {
                self.collect_resolved_type(value);
            }
            ResolvedType::Map { key, value, .. }
            | ResolvedType::Pair {
                first: key,
                second: value,
            } => {
                self.collect_resolved_type(key);
                self.collect_resolved_type(value);
            }
            ResolvedType::Tuple { elements } => {
                for element in elements {
                    self.collect_resolved_type(element);
                }
            }
            ResolvedType::Scalar(_)
            | ResolvedType::Named { .. }
            | ResolvedType::ByteStream
            | ResolvedType::Unknown { .. } => {}
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CodegenContainerSupportUsage {
    pub fixed_array: bool,
    pub fixed_vector: bool,
    pub bit_set: bool,
    pub fixed_bytes: bool,
}

impl CodegenContainerSupportUsage {
    #[must_use]
    pub const fn any(self) -> bool {
        self.fixed_array || self.fixed_vector || self.bit_set || self.fixed_bytes
    }

    #[must_use]
    pub fn for_unit_fields(unit: &SerializeCodegenUnit) -> Self {
        let mut usage = Self::default();
        for item in &unit.items {
            for field in &item.fields {
                usage.collect_field(field);
            }
        }
        usage
    }

    #[must_use]
    pub fn for_items<'a>(
        items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        projection: CodegenTypeReferenceProjection,
    ) -> Self {
        let mut usage = Self::default();
        for item in items {
            for field in &item.fields {
                if projection.field_should_reference_type(item, field, items_by_type_id) {
                    usage.collect_field(field);
                }
            }
            if let Some(resolved) = &item.enum_underlying_type {
                usage.collect_resolved_type(resolved);
            }
        }
        usage
    }

    pub fn collect_field(&mut self, field: &crate::ir::SerializeCodegenField) {
        match classify_codegen_field_type(field) {
            CodegenFieldTypeProjection::FixedOpaqueBytes { .. } => {
                self.fixed_bytes = true;
            }
            CodegenFieldTypeProjection::Reflected(resolved_type) => {
                self.collect_resolved_type(resolved_type);
            }
        }
    }

    pub fn collect_resolved_type(&mut self, resolved: &ResolvedType) {
        match resolved {
            ResolvedType::Sequence {
                kind: SequenceKind::Array,
                capacity: Some(_),
                element,
            } => {
                self.fixed_array = true;
                self.collect_resolved_type(element);
            }
            ResolvedType::Sequence {
                kind: SequenceKind::FixedVector,
                capacity: Some(_),
                element,
            } => {
                self.fixed_vector = true;
                self.collect_resolved_type(element);
            }
            ResolvedType::Sequence {
                kind: SequenceKind::BitSet,
                capacity: Some(_),
                element,
            } => {
                self.bit_set = true;
                self.collect_resolved_type(element);
            }
            ResolvedType::Sequence { element, .. }
            | ResolvedType::RangedInteger { value: element, .. }
            | ResolvedType::Optional { value: element }
            | ResolvedType::ReplicatedField { value: element }
            | ResolvedType::Pointer {
                target: element, ..
            } => self.collect_resolved_type(element),
            ResolvedType::Map { key, value, .. }
            | ResolvedType::Pair {
                first: key,
                second: value,
            } => {
                self.collect_resolved_type(key);
                self.collect_resolved_type(value);
            }
            ResolvedType::Tuple { elements } => {
                for element in elements {
                    self.collect_resolved_type(element);
                }
            }
            ResolvedType::Scalar(_)
            | ResolvedType::Named { .. }
            | ResolvedType::Asset { .. }
            | ResolvedType::Uid { .. }
            | ResolvedType::ByteStream
            | ResolvedType::Unknown { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind};
    use crate::role::ReflectedTypeRole;
    use crate::types::{MapKind, ResolvedType};

    use super::*;

    #[test]
    fn semantic_support_usage_collects_nested_az_wrappers() {
        let item = struct_item(vec![field(ResolvedType::Map {
            kind: MapKind::UnorderedMap,
            key: Box::new(ResolvedType::Scalar(ScalarType::Crc32)),
            value: Box::new(ResolvedType::Tuple {
                elements: vec![
                    ResolvedType::Asset {
                        type_id: Some(uuid!("11111111-1111-1111-1111-111111111111")),
                        asset_type_id: None,
                    },
                    ResolvedType::Scalar(ScalarType::Vector3),
                    ResolvedType::Scalar(ScalarType::Transform),
                    ResolvedType::Scalar(ScalarType::Vector3),
                ],
            }),
        })]);
        let items = BTreeMap::from([(item.source_type_id, &item)]);

        let usage = CodegenSupportUsage::for_items(
            [&item],
            &items,
            CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges,
        );

        assert!(usage.any());
        assert!(usage.asset);
        assert!(usage.crc32);
        assert!(!usage.asset_id);
        assert!(!usage.uuid);
        assert!(usage.has_math());
        assert_eq!(
            usage.math_scalars,
            BTreeSet::from([ScalarType::Transform, ScalarType::Vector3])
        );
    }

    #[test]
    fn container_support_usage_collects_fixed_shapes_and_opaque_bytes() {
        let item = struct_item(vec![
            field(ResolvedType::Sequence {
                kind: SequenceKind::Array,
                element: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                capacity: Some(3),
            }),
            SerializeCodegenField {
                source_name: "opaque".to_owned(),
                source_type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                resolved_type: ResolvedType::Unknown {
                    type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                    reason: "fixed payload".to_owned(),
                },
                data_size: Some(16),
                offset: None,
                flags: None,
                is_base_class: false,
                is_pointer: false,
                is_dynamic_field: false,
            },
        ]);
        let unit = SerializeCodegenUnit {
            items: vec![item.clone()],
        };

        let usage = CodegenContainerSupportUsage::for_unit_fields(&unit);

        assert!(usage.any());
        assert!(usage.fixed_array);
        assert!(usage.fixed_bytes);
        assert!(!usage.fixed_vector);
        assert!(!usage.bit_set);
    }

    fn struct_item(fields: Vec<SerializeCodegenField>) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
            source_name: "Example".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn field(resolved_type: ResolvedType) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "value".to_owned(),
            source_type_id: uuid!("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
            resolved_type,
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
