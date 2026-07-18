use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::network_schema::{
    NetworkContainerCodec, NetworkFieldHandlerVtable, NetworkNestedTypeMember,
    NetworkNestedTypeShape,
};
use crate::{CompileUnit, NetworkSchema, collect_resolved_named_type_ids};

/// Plans the reflected data types required by a network schema.
///
/// Diagnostic candidates are deliberately excluded. Only selected identities and
/// exact native-layout matches may affect generated source.
pub struct NetworkSerializeRootPlanner<'schema> {
    schema: &'schema NetworkSchema,
    reflected: ReflectedRootIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSerializeRootPlan {
    pub root_specs: Vec<String>,
    pub explicit_root_count: usize,
    pub inferred_root_count: usize,
}

impl<'schema> NetworkSerializeRootPlanner<'schema> {
    #[must_use]
    pub fn new(schema: &'schema NetworkSchema) -> Self {
        Self {
            schema,
            reflected: ReflectedRootIndex::from_schema(schema),
        }
    }

    /// Makes reflected types from the compiled SerializeContext available to
    /// exact network identities discovered before schema merging.
    #[must_use]
    pub fn with_compile_unit(mut self, unit: &CompileUnit) -> Self {
        self.reflected.extend_compile_unit(unit);
        self
    }

    pub fn plan<I, S>(&self, explicit_roots: I) -> NetworkSerializeRootPlan
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut roots = explicit_roots
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        let explicit_root_count = roots.len();
        let reflected = &self.reflected;

        for type_id in required_serialize_type_ids(self.schema) {
            reflected.insert_root(&mut roots, type_id);
        }

        NetworkSerializeRootPlan {
            inferred_root_count: roots.len().saturating_sub(explicit_root_count),
            explicit_root_count,
            root_specs: roots.into_iter().collect(),
        }
    }
}

struct ReflectedRootIndex {
    emitted_type_ids: BTreeSet<Uuid>,
    generic_dependencies: BTreeMap<Uuid, BTreeSet<Uuid>>,
}

impl ReflectedRootIndex {
    fn from_schema(schema: &NetworkSchema) -> Self {
        Self {
            emitted_type_ids: schema
                .serialize_types
                .iter()
                .filter(|serialize| serialize.emits_source)
                .map(|serialize| serialize.type_id)
                .collect(),
            generic_dependencies: BTreeMap::new(),
        }
    }

    fn extend_compile_unit(&mut self, unit: &CompileUnit) {
        self.emitted_type_ids.extend(
            unit.codegen_unit
                .items
                .iter()
                .map(|item| item.source_type_id),
        );
        self.generic_dependencies
            .extend(unit.catalog.generic_types().map(|generic| {
                let mut dependencies = BTreeSet::new();
                collect_resolved_named_type_ids(&generic.resolved_type, &mut dependencies);
                (generic.type_id, dependencies)
            }));
    }

    fn insert_root(&self, roots: &mut BTreeSet<String>, type_id: Uuid) {
        let mut pending = vec![type_id];
        let mut visited = BTreeSet::new();
        while let Some(type_id) = pending.pop() {
            if !visited.insert(type_id) {
                continue;
            }
            if self.emitted_type_ids.contains(&type_id) {
                roots.insert(type_id.to_string());
                continue;
            }
            if let Some(dependencies) = self.generic_dependencies.get(&type_id) {
                pending.extend(dependencies.iter().copied());
            }
        }
    }
}

pub(crate) fn required_serialize_type_ids(schema: &NetworkSchema) -> BTreeSet<Uuid> {
    let mut type_ids = BTreeSet::new();
    for network_type in &schema.types {
        if let Some(serialize) = network_type
            .serialize
            .as_ref()
            .filter(|serialize| network_type.type_id == Some(serialize.type_id))
        {
            type_ids.insert(serialize.type_id);
        }
        for field in &network_type.fields {
            if let Some(serialize) = field
                .serialize
                .as_ref()
                .filter(|serialize| serialize.confidence.is_high_or_exact())
            {
                type_ids.insert(serialize.type_id);
            }
            if field.source_type_identity_proven {
                type_ids.extend(field.source_type_id);
            }
            if let Some(shape) = field.nested_type_shape.as_ref() {
                collect_exact_shape_type_ids(shape, &mut type_ids);
            }
        }
    }

    for vtable in &schema.field_handler_vtables {
        collect_vtable_type_ids(vtable, &mut type_ids);
    }
    type_ids
}

fn collect_vtable_type_ids(vtable: &NetworkFieldHandlerVtable, type_ids: &mut BTreeSet<Uuid>) {
    if let Some(shape) = vtable.value_type_shape.as_ref() {
        collect_exact_shape_type_ids(shape, type_ids);
    }
    if let Some(plan) = vtable.full_container_plan.as_ref() {
        for codec in plan.key_codecs.iter().chain(&plan.value_codecs) {
            collect_exact_codec_type_ids(codec, type_ids);
        }
    }
}

fn collect_exact_codec_type_ids(codec: &NetworkContainerCodec, type_ids: &mut BTreeSet<Uuid>) {
    if codec.type_identity_proven && (codec.members.is_empty() || codec.source_type_layout_complete)
    {
        type_ids.extend(codec.type_id);
    }
    for member in &codec.members {
        collect_exact_codec_type_ids(member, type_ids);
    }
}

fn collect_exact_shape_type_ids(shape: &NetworkNestedTypeShape, type_ids: &mut BTreeSet<Uuid>) {
    if !shape.has_exact_identity() {
        return;
    }
    type_ids.extend(shape.type_id);
    for member in &shape.members {
        collect_exact_member_type_id(member, type_ids);
    }
}

fn collect_exact_member_type_id(member: &NetworkNestedTypeMember, type_ids: &mut BTreeSet<Uuid>) {
    if member.type_id_source.as_deref() == Some("serialize-field-for-proven-type") {
        type_ids.extend(member.type_id);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    #[test]
    fn exact_generic_dependencies_select_only_emitted_roots() {
        let generic_type_id = Uuid::parse_str("3485f20a-98c0-5315-876b-21bcd23a7bc0").unwrap();
        let emitted_type_id = Uuid::parse_str("d821e0a4-1099-4cb3-95e3-aff582c4fb7b").unwrap();
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8123456",
                "fieldCount": 1,
                "fullContainerPlan": {
                    "storageKind": "index-map",
                    "keyCodecs": [{
                        "typeId": generic_type_id.to_string(),
                        "typeIdentityProven": true,
                        "wireLayout": "fixed-bytes-16"
                    }],
                    "valueCodecs": [{ "wireShape": "u32" }]
                },
                "slots": []
            }]
        }))
        .unwrap();
        let reflected = ReflectedRootIndex {
            emitted_type_ids: BTreeSet::from([emitted_type_id]),
            generic_dependencies: BTreeMap::from([(
                generic_type_id,
                BTreeSet::from([emitted_type_id]),
            )]),
        };
        let planner = NetworkSerializeRootPlanner {
            schema: &schema,
            reflected,
        };

        assert_eq!(
            planner.plan([] as [&str; 0]).root_specs,
            [emitted_type_id.to_string()]
        );
    }
}
