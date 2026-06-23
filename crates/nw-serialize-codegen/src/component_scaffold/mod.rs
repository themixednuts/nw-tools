use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::rust::integrate::identity::{
    az_identity_evidence_from_catalog, reconcile_az_type_identities,
};

mod api;
mod edit;
mod evidence;
mod existing;
mod existing_patch;
mod module_plan;
mod module_render;
mod naming;
mod source_render;
mod type_model;

use crate::CodegenContext;
pub use api::{
    ComponentScaffoldError, ComponentScaffoldReport, ComponentScaffoldRequest,
    ExistingComponentReport, ExistingFieldReport, FacetOwnerEvidence, ModuleScaffoldAction,
    ModuleScaffoldReport, SkippedFieldReport,
};
use evidence::collect_reflected_component_evidence;
pub use evidence::facet_owner_evidence_from_layout;
use existing::discover_existing_components;
use existing_patch::{ExistingComponentPatch, apply_existing_patches};
use module_plan::ModulePlanBuilder;
use module_render::{
    apply_module_plans, update_components_module_file, update_newworld_plugin_file,
};
use naming::module_name_for_component;
use type_model::{RustTypeContext, rust_field_from_reflected_field, unique_field_name};

#[cfg(test)]
use existing_patch::{
    ExistingComponentPatchShape, patch_manual_default_impl, reconcile_existing_struct_value_derives,
};
#[cfg(test)]
use source_render::reflected_struct_derive_tokens;
#[cfg(test)]
use type_model::{DeriveCapabilities, RustField};

pub fn scaffold_components(
    request: ComponentScaffoldRequest<'_>,
    context: &CodegenContext,
) -> Result<ComponentScaffoldReport, ComponentScaffoldError> {
    let facet_owner_evidence = request
        .facet_owner_evidence
        .iter()
        .cloned()
        .map(|evidence| (evidence.facet_name.clone(), evidence))
        .collect::<BTreeMap<_, _>>();

    let evidence = collect_reflected_component_evidence(request.catalog, &facet_owner_evidence);
    let mut az_identity_evidence = az_identity_evidence_from_catalog(request.catalog);
    for facet in facet_owner_evidence.values() {
        az_identity_evidence
            .type_ids_by_name
            .insert(facet.facet_name.clone(), facet.facet_type_id);
        az_identity_evidence
            .facet_owners_by_name
            .insert(facet.facet_name.clone(), facet.owner_name.clone());
    }
    let (az_identity_reconciliations, skipped_az_identity_reconciliations) =
        reconcile_az_type_identities(
            &request.components_root,
            request.apply,
            &az_identity_evidence,
            context,
        )?;

    let existing = discover_existing_components(&request.components_root, context)?;

    let mut existing_components = Vec::new();
    let mut module_builders = BTreeMap::<String, ModulePlanBuilder>::new();
    let mut existing_patches = BTreeMap::<PathBuf, Vec<ExistingComponentPatch>>::new();
    let mut missing_existing_fields = Vec::new();
    let mut skipped_existing_fields = Vec::new();

    for component in evidence.values() {
        if let Some(existing_component) = existing.by_type_id.get(&component.type_id) {
            existing_components.push(ExistingComponentReport {
                component_name: existing_component.component_name.clone(),
                type_id: component.type_id,
                file_path: existing_component.file_path.clone(),
            });

            let mut inserted_fields = Vec::new();
            let mut context =
                RustTypeContext::new(&component.component_name, &existing.source_types);
            let mut used_field_names = existing_component.field_names.clone();
            let mut desired_field_order = Vec::new();
            let mut desired_field_names = BTreeSet::new();
            for reflected_field in &component.fields {
                let mut field =
                    rust_field_from_reflected_field(reflected_field, request.catalog, &mut context);
                if existing_component.field_names.contains(&field.name) {
                    if desired_field_names.insert(field.name.clone()) {
                        desired_field_order.push(field.name);
                    }
                    continue;
                }
                field.name = unique_field_name(&mut used_field_names, &field.name);
                if desired_field_names.insert(field.name.clone()) {
                    desired_field_order.push(field.name.clone());
                }

                missing_existing_fields.push(ExistingFieldReport {
                    component_name: existing_component.component_name.clone(),
                    file_path: existing_component.file_path.clone(),
                    field_name: field.name.clone(),
                    field_type: field.ty.clone(),
                });

                if existing_component.can_receive_fields() {
                    inserted_fields.push(field);
                } else {
                    skipped_existing_fields.push(SkippedFieldReport {
                        component_name: existing_component.component_name.clone(),
                        file_path: existing_component.file_path.clone(),
                        field_name: field.name.clone(),
                        reason: "component is a tuple or unsupported struct shape".to_owned(),
                    });
                    continue;
                };
            }

            for field_name in &existing_component.field_names_in_order {
                if desired_field_names.insert(field_name.clone()) {
                    desired_field_order.push(field_name.clone());
                }
            }

            if !inserted_fields.is_empty() {
                let patch = ExistingComponentPatch {
                    component_name: existing_component.component_name.clone(),
                    shape: existing_component.patch_shape(),
                    fields: inserted_fields,
                    desired_field_order,
                };
                existing_patches
                    .entry(existing_component.file_path.clone())
                    .or_default()
                    .push(patch);
            }
            continue;
        }

        let module_name = module_name_for_component(&component.component_name);
        module_builders
            .entry(module_name.clone())
            .or_insert_with(|| ModulePlanBuilder::new(module_name, &request.components_root))
            .components
            .push(component.clone());
    }

    let module_plans = module_builders
        .into_values()
        .map(|builder| builder.build())
        .collect::<Vec<_>>();

    if request.apply {
        apply_existing_patches(existing_patches, context)?;
        apply_module_plans(&module_plans, request.catalog, &existing.source_types)?;
        update_components_module_file(&request.module_file, &module_plans)?;
        if let Some(plugin_file) = &request.plugin_file {
            update_newworld_plugin_file(
                plugin_file,
                module_root_from_module_file(&request.module_file).as_deref(),
                &module_plans,
            )?;
        }
    }

    Ok(ComponentScaffoldReport {
        source_label: request
            .source_label
            .unwrap_or("reflected-type-catalog")
            .to_owned(),
        applied: request.apply,
        components_seen: evidence.len(),
        existing_components,
        created_or_extended_modules: module_plans
            .iter()
            .map(|plan| ModuleScaffoldReport {
                module_name: plan.module_name.clone(),
                file_path: plan.file_path.clone(),
                component_names: plan
                    .components
                    .iter()
                    .map(|component| component.component_name.clone())
                    .collect(),
                action: plan.action,
            })
            .collect(),
        missing_existing_fields,
        skipped_existing_fields,
        az_identity_reconciliations,
        skipped_az_identity_reconciliations,
        facet_owner_evidence: facet_owner_evidence
            .into_values()
            .map(|facet| FacetOwnerEvidence {
                facet_name: facet.facet_name,
                facet_type_id: facet.facet_type_id,
                owner_name: facet.owner_name,
                owner_type_id: facet.owner_type_id,
                field_name: facet.field_name,
            })
            .collect(),
    })
}

fn read_to_string(path: &Path) -> Result<String, ComponentScaffoldError> {
    fs::read_to_string(path).map_err(|source| ComponentScaffoldError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn write_string(path: &Path, text: &str) -> Result<(), ComponentScaffoldError> {
    fs::write(path, text).map_err(|source| ComponentScaffoldError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn module_root_from_module_file(module_file: &Path) -> Option<String> {
    let root = module_file.parent()?.file_name()?.to_str()?;
    (root != "src").then(|| root.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_default_patch_updates_struct_literal_only() {
        let mut text = r#"
pub struct ExampleComponent {
    pub existing: i32,
    pub added: i32,
}

impl Default for ExampleComponent {
    fn default() -> Self {
        Self {
            existing: 1,
        }
    }
}
"#
        .to_owned();

        patch_manual_default_impl(
            &mut text,
            "ExampleComponent",
            &[RustField {
                name: "added".to_owned(),
                ty: "i32".to_owned(),
                nested_structs: Vec::new(),
                maps_entities: false,
                derive_capabilities: DeriveCapabilities::COPY_EQ,
            }],
        );

        assert!(text.contains(
            "        Self {\n            existing: 1,\n            added: Default::default(),\n        }"
        ));
        assert!(!text.contains("    }\n            added: Default::default(),"));
    }

    #[test]
    fn reflected_derives_keep_copy_for_float_vector_shapes_without_eq() {
        let fields = vec![
            RustField {
                name: "box_size".to_owned(),
                ty: "bevy::prelude::Vec3".to_owned(),
                nested_structs: Vec::new(),
                maps_entities: false,
                derive_capabilities: DeriveCapabilities::COPY_ONLY,
            },
            RustField {
                name: "radius".to_owned(),
                ty: "f32".to_owned(),
                nested_structs: Vec::new(),
                maps_entities: false,
                derive_capabilities: DeriveCapabilities::COPY_ONLY,
            },
        ];

        let tokens = reflected_struct_derive_tokens(&["AzTypeInfo"], &fields);

        assert!(tokens.contains(&"Copy".to_owned()));
        assert!(tokens.contains(&"PartialEq".to_owned()));
        assert!(!tokens.contains(&"Eq".to_owned()));
    }

    #[test]
    fn reflected_derives_keep_eq_for_string_backed_types_without_copy() {
        let fields = vec![RustField {
            name: "label".to_owned(),
            ty: "String".to_owned(),
            nested_structs: Vec::new(),
            maps_entities: false,
            derive_capabilities: DeriveCapabilities::EQ_ONLY,
        }];

        let tokens = reflected_struct_derive_tokens(&["AzTypeInfo"], &fields);

        assert!(!tokens.contains(&"Copy".to_owned()));
        assert!(tokens.contains(&"Eq".to_owned()));
    }

    #[test]
    fn existing_patch_preserves_copy_and_removes_only_invalid_eq() {
        let mut text = r#"
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct QueryShapeSphere {
    pub radius: f32,
}
"#
        .to_owned();
        let fields = vec![RustField {
            name: "radius".to_owned(),
            ty: "f32".to_owned(),
            nested_structs: Vec::new(),
            maps_entities: false,
            derive_capabilities: DeriveCapabilities::COPY_ONLY,
        }];

        reconcile_existing_struct_value_derives(
            &mut text,
            "QueryShapeSphere",
            ExistingComponentPatchShape::Braced { insert_offset: 0 },
            &fields,
        );

        assert!(text.contains("#[derive(Debug, Default, Clone, Copy, PartialEq, Reflect)]"));
    }

    #[test]
    fn existing_unit_patch_adds_copy_eq_when_all_added_fields_allow_it() {
        let mut text = r#"
#[derive(Debug, Default, Clone, PartialEq, Reflect)]
pub struct NumericSupport;
"#
        .to_owned();
        let fields = vec![RustField {
            name: "value".to_owned(),
            ty: "u32".to_owned(),
            nested_structs: Vec::new(),
            maps_entities: false,
            derive_capabilities: DeriveCapabilities::COPY_EQ,
        }];
        let semicolon_offset = text.find(';').expect("unit semicolon");

        reconcile_existing_struct_value_derives(
            &mut text,
            "NumericSupport",
            ExistingComponentPatchShape::Unit { semicolon_offset },
            &fields,
        );

        assert!(text.contains("#[derive(Debug, Default, Clone, PartialEq, Reflect, Copy, Eq)]"));
    }
}
