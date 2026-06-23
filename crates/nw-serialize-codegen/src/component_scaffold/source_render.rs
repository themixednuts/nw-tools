use uuid::Uuid;

use crate::{ReflectedTypeCatalog, RustSourceTypeIndex};

use super::module_plan::ModulePlan;
use super::naming::{facet_bundle_field_name, inferred_facet_owner, is_facet_type_name};
use super::type_model::{
    DeriveCapabilities, NestedStruct, RustField, RustTypeContext, dedupe_rust_fields,
    derive_capabilities_for_fields, rust_field_from_reflected_field, rust_fields_map_entities,
};

pub(super) fn render_new_module(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> String {
    let mut code = String::new();
    if plan_has_nested_structs(plan, catalog, source_types) {
        code.push_str("use az_derive::{AzRtti, AzTypeInfo};\n");
    } else {
        code.push_str("use az_derive::AzRtti;\n");
    }
    if plan_maps_entities(plan, catalog, source_types) {
        code.push_str("use bevy::ecs::entity::MapEntities;\n");
    }
    code.push_str("use bevy::prelude::*;\n");
    if plan
        .components
        .iter()
        .any(|component| is_facet_type_name(&component.component_name))
    {
        code.push_str("use newworld_derive::Facet;\n");
    }
    code.push('\n');
    code.push_str(&render_components(plan, catalog, source_types));
    code
}

pub(super) fn render_module_append(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> String {
    let mut code = String::new();
    code.push_str(&render_components(plan, catalog, source_types));
    code
}

fn render_components(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> String {
    let mut code = String::new();

    for component in &plan.components {
        let mut context = RustTypeContext::new(&component.component_name, source_types);
        let mut fields = component
            .fields
            .iter()
            .map(|field| rust_field_from_reflected_field(field, catalog, &mut context))
            .collect::<Vec<_>>();
        dedupe_rust_fields(&mut fields);

        for nested in &context.nested_structs {
            code.push_str(&render_nested_struct(nested));
            code.push('\n');
        }

        if is_facet_type_name(&component.component_name) {
            render_facet_struct(
                &mut code,
                &component.component_name,
                component.type_id,
                component.facet_owner.as_deref(),
                &fields,
            );
        } else {
            render_component_struct(
                &mut code,
                &component.component_name,
                component.type_id,
                &fields,
            );
        }
    }

    for bundle in bundle_plans(plan) {
        render_bundle_struct(&mut code, &bundle);
    }

    if !plan.patch_existing_plugin {
        code.push_str("pub struct ");
        code.push_str(&plan.plugin_name);
        code.push_str(";\n\n");
        code.push_str("impl Plugin for ");
        code.push_str(&plan.plugin_name);
        code.push_str(" {\n");
        code.push_str("    fn build(&self, app: &mut App) {\n");
        render_register_type_lines(&mut code, plan, catalog, source_types);
        code.push_str("    }\n");
        code.push_str("}\n\n");
    }

    code
}

fn render_component_struct(
    code: &mut String,
    component_name: &str,
    type_id: Uuid,
    fields: &[RustField],
) {
    render_derive_attr(
        code,
        &reflected_struct_derive_tokens(&["Component", "AzRtti"], fields),
    );
    code.push_str("#[az_rtti(uuid = \"");
    code.push_str(&type_id.to_string());
    code.push_str("\", base = crate::generated::Component)]\n");
    code.push_str("#[reflect(Component)]\n");
    code.push_str("pub struct ");
    code.push_str(component_name);
    render_struct_body(code, fields);
}

fn render_facet_struct(
    code: &mut String,
    component_name: &str,
    type_id: Uuid,
    facet_owner: Option<&str>,
    fields: &[RustField],
) {
    render_derive_attr(
        code,
        &reflected_struct_derive_tokens(&["Component", "AzRtti", "Facet"], fields),
    );
    code.push_str("#[az_rtti(uuid = \"");
    code.push_str(&type_id.to_string());
    code.push_str("\", base = crate::generated::Component)]\n");
    code.push_str("#[reflect(Component)]\n");
    if let Some(facet_owner) = facet_owner
        && inferred_facet_owner(component_name).as_deref() != Some(facet_owner)
    {
        code.push_str("#[facet(owner = ");
        code.push_str(facet_owner);
        code.push_str(")]\n");
    }
    code.push_str("pub struct ");
    code.push_str(component_name);
    render_struct_body(code, fields);
}

fn render_struct_body(code: &mut String, fields: &[RustField]) {
    if fields.is_empty() {
        code.push_str(";\n\n");
        return;
    }

    code.push_str(" {\n");
    for field in fields {
        render_field_line(code, field);
    }
    code.push_str("}\n\n");
}

pub(super) fn render_field_line(code: &mut String, field: &RustField) {
    if field.maps_entities {
        code.push_str("    #[entities]\n");
    }
    code.push_str("    pub ");
    code.push_str(&field.name);
    code.push_str(": ");
    code.push_str(&field.ty);
    code.push_str(",\n");
}

pub(super) fn render_nested_struct(nested: &NestedStruct) -> String {
    let mut code = String::new();
    render_derive_attr(
        &mut code,
        &reflected_struct_derive_tokens_for_capabilities(
            &["AzTypeInfo"],
            nested.derive_capabilities,
            nested.maps_entities,
        ),
    );
    code.push_str("#[az_type_info(\"");
    code.push_str(&nested.type_id.to_string());
    code.push_str("\")]\n");
    code.push_str("pub struct ");
    code.push_str(&nested.name);
    if nested.fields.is_empty() {
        code.push_str(";\n");
        return code;
    }

    code.push_str(" {\n");
    for field in &nested.fields {
        render_field_line(&mut code, field);
    }
    code.push_str("}\n");
    code
}

fn render_derive_attr(code: &mut String, tokens: &[String]) {
    code.push_str("#[derive(");
    code.push_str(&tokens.join(", "));
    code.push_str(")]\n");
}

pub(super) fn reflected_struct_derive_tokens(prefix: &[&str], fields: &[RustField]) -> Vec<String> {
    reflected_struct_derive_tokens_for_capabilities(
        prefix,
        derive_capabilities_for_fields(fields),
        rust_fields_map_entities(fields),
    )
}

fn reflected_struct_derive_tokens_for_capabilities(
    prefix: &[&str],
    capabilities: DeriveCapabilities,
    maps_entities: bool,
) -> Vec<String> {
    let mut tokens = prefix
        .iter()
        .map(|token| (*token).to_owned())
        .collect::<Vec<_>>();
    tokens.extend(["Debug", "Default", "Clone"].into_iter().map(str::to_owned));
    if capabilities.copy {
        tokens.push("Copy".to_owned());
    }
    tokens.push("PartialEq".to_owned());
    if capabilities.eq {
        tokens.push("Eq".to_owned());
    }
    tokens.push("Reflect".to_owned());
    if maps_entities {
        tokens.push("MapEntities".to_owned());
    }
    tokens
}

fn render_register_type_lines(
    code: &mut String,
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) {
    for ty in registration_types(plan, catalog, source_types) {
        code.push_str("        app.register_type::<");
        code.push_str(&ty.name);
        code.push_str(">();\n");
        code.push_str("        app.register_type_data::<");
        code.push_str(&ty.name);
        code.push_str(", ::az_core::ReflectAzTypeInfo>();\n");
        if ty.rtti {
            code.push_str("        app.register_type_data::<");
            code.push_str(&ty.name);
            code.push_str(", ::az_core::ReflectAzRtti>();\n");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegistrationType {
    pub name: String,
    pub rtti: bool,
}

pub(super) fn registration_types(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> Vec<RegistrationType> {
    let mut types = std::collections::BTreeMap::new();
    for component in &plan.components {
        let mut context = RustTypeContext::new(&component.component_name, source_types);
        for reflected_field in &component.fields {
            let _field = rust_field_from_reflected_field(reflected_field, catalog, &mut context);
        }
        for nested in &context.nested_structs {
            types.entry(nested.name.clone()).or_insert(false);
        }
        types
            .entry(component.component_name.clone())
            .and_modify(|rtti| *rtti = true)
            .or_insert(true);
    }
    types
        .into_iter()
        .map(|(name, rtti)| RegistrationType { name, rtti })
        .collect()
}

pub(super) fn plan_maps_entities(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> bool {
    plan.components.iter().any(|component| {
        let mut context = RustTypeContext::new(&component.component_name, source_types);
        let fields = component
            .fields
            .iter()
            .map(|field| rust_field_from_reflected_field(field, catalog, &mut context))
            .collect::<Vec<_>>();
        rust_fields_map_entities(&fields)
            || context
                .nested_structs
                .iter()
                .any(|nested| nested.maps_entities)
    })
}

pub(super) fn plan_has_nested_structs(
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> bool {
    plan.components.iter().any(|component| {
        let mut context = RustTypeContext::new(&component.component_name, source_types);
        for field in &component.fields {
            let _field = rust_field_from_reflected_field(field, catalog, &mut context);
        }
        !context.nested_structs.is_empty()
    })
}

pub(super) fn bundle_names(plan: &ModulePlan) -> Vec<String> {
    bundle_plans(plan)
        .into_iter()
        .map(|bundle| bundle.name)
        .collect()
}

#[derive(Debug, Clone)]
struct BundlePlan {
    name: String,
    fields: Vec<BundleField>,
}

#[derive(Debug, Clone)]
struct BundleField {
    name: String,
    ty: String,
}

fn render_bundle_struct(code: &mut String, bundle: &BundlePlan) {
    code.push_str("#[derive(Bundle, Debug, Default, Clone)]\n");
    code.push_str("pub struct ");
    code.push_str(&bundle.name);
    code.push_str(" {\n");
    for field in &bundle.fields {
        code.push_str("    pub ");
        code.push_str(&field.name);
        code.push_str(": ");
        code.push_str(&field.ty);
        code.push_str(",\n");
    }
    code.push_str("}\n\n");
}

fn bundle_plans(plan: &ModulePlan) -> Vec<BundlePlan> {
    let mut bundles = Vec::new();
    let facets = plan
        .components
        .iter()
        .filter(|component| is_facet_type_name(&component.component_name))
        .collect::<Vec<_>>();

    for owner in plan
        .components
        .iter()
        .filter(|component| !is_facet_type_name(&component.component_name))
    {
        let mut fields = vec![BundleField {
            name: "component".to_owned(),
            ty: owner.component_name.clone(),
        }];
        for facet in &facets {
            if facet.facet_owner.as_deref() != Some(owner.component_name.as_str()) {
                continue;
            }
            fields.push(BundleField {
                name: facet_bundle_field_name(&facet.component_name),
                ty: facet.component_name.clone(),
            });
        }
        if fields.len() > 1 {
            bundles.push(BundlePlan {
                name: format!("{}Bundle", owner.component_name),
                fields,
            });
        }
    }

    bundles
}
