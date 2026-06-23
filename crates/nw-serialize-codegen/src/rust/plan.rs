use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::CodegenContext;
use crate::field_projection::item_has_materialized_payload;
use crate::ir::{
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenPlanner, SerializeCodegenUnit,
    collect_resolved_named_type_ids,
};
use crate::layout::{LayoutIndex, dependency_ordered_codegen_items, reflected_base_type_ids};
use crate::model::SerializeContextModel;
use crate::naming::{rust_type_ident, rust_variant_ident};
use crate::role::ReflectedTypeRole;
use crate::rust::derive_plan::RustDerivePlanner;
use crate::rust::enum_plan::{RustEnumPlanner, RustVariantPlan, enum_has_duplicate_discriminants};
use crate::rust::field_plan::{RustFieldPlanner, integrated_custom_field_type};
use crate::rust::identity::RustTypeIdentityPlan;
use crate::rust::integrate::source_index::RustSourceTypeIndex;
use crate::rust::item_plan::{RustCodegenUnit, RustItemKind, RustItemPlan};
use crate::rust::name_plan::RustNamePlan;
use crate::rust::options::{RustCodegenMode, RustCodegenOptions};
use crate::rust::scope_plan::{
    rust_symbol_scope_key, standalone_family_symbol_module_paths_by_candidate,
};
use crate::rust::types::{RustTypeOptions, RustTypeRenderer};

#[derive(Debug)]
pub struct RustCodegenPlanner {
    options: RustCodegenOptions,
    derive_planner: RustDerivePlanner,
    enum_planner: RustEnumPlanner,
    field_planner: RustFieldPlanner,
    source_types: Option<RustSourceTypeIndex>,
    manual_default_type_ids: BTreeSet<Uuid>,
}

#[derive(Debug, Default)]
struct RustPlanWorkerState {
    eq_cache: BTreeMap<Uuid, bool>,
    default_cache: BTreeMap<Uuid, bool>,
    hash_cache: BTreeMap<Uuid, bool>,
    copy_cache: BTreeMap<Uuid, bool>,
    marshaler_cache: BTreeMap<Uuid, bool>,
    serde_cache: BTreeMap<Uuid, bool>,
}

impl Default for RustCodegenPlanner {
    fn default() -> Self {
        Self::new(RustCodegenOptions::default())
    }
}

impl RustCodegenPlanner {
    #[must_use]
    pub fn new(options: RustCodegenOptions) -> Self {
        let rust_types = RustTypeRenderer::new(RustTypeOptions {
            use_support_aliases: matches!(options.mode, RustCodegenMode::Standalone),
            uuid_alias: "AzUuid",
            crc32_alias: "AzCrc32",
            entity_id_alias: "u64",
            asset_id_alias: "AzAssetId",
            asset_alias: "AzAsset",
            uid_alias: "AzUuid",
            replicated_field_alias: "Option",
        });
        Self {
            options,
            derive_planner: RustDerivePlanner::new(options.mode),
            enum_planner: RustEnumPlanner::new(rust_types),
            field_planner: RustFieldPlanner::new(options.mode, rust_types),
            source_types: None,
            manual_default_type_ids: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_source_type_index(mut self, source_types: RustSourceTypeIndex) -> Self {
        self.source_types = Some(source_types);
        self
    }

    #[must_use]
    pub fn without_default_derive_for(mut self, type_ids: impl IntoIterator<Item = Uuid>) -> Self {
        self.manual_default_type_ids.extend(type_ids);
        self
    }

    #[must_use]
    pub fn standalone() -> Self {
        Self::new(RustCodegenOptions {
            mode: RustCodegenMode::Standalone,
        })
    }

    #[must_use]
    pub fn plan_model(model: &SerializeContextModel, context: &CodegenContext) -> RustCodegenUnit {
        Self::default().plan(model, context)
    }

    #[must_use]
    pub fn plan_standalone_model(
        model: &SerializeContextModel,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        Self::standalone().plan(model, context)
    }

    #[must_use]
    pub fn plan_codegen_unit(
        unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        Self::default().plan_serialize_codegen_unit(unit, context)
    }

    #[must_use]
    pub fn plan(&self, model: &SerializeContextModel, context: &CodegenContext) -> RustCodegenUnit {
        self.plan_serialize_codegen_unit(&SerializeCodegenPlanner::plan_model(model), context)
    }

    #[must_use]
    pub fn plan_serialize_codegen_unit(
        &self,
        unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        self.plan_serialize_codegen_units(unit, unit, context)
    }

    #[must_use]
    pub fn plan_selected_serialize_codegen_unit(
        &self,
        unit: &SerializeCodegenUnit,
        selection: crate::ir::SerializeCodegenSelection,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        let selected = unit.select(selection);
        self.plan_serialize_codegen_units(&selected, unit, context)
    }

    #[must_use]
    pub fn plan_serialize_codegen_units(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        let context_index = context_unit.index();
        self.plan_serialize_codegen_unit_from_indexes(
            emitted_unit,
            context_index.items_by_type_id(),
            context_unit,
            context,
        )
    }

    fn plan_serialize_codegen_unit_from_indexes(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        context_unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> RustCodegenUnit {
        debug_assert!(
            emitted_unit
                .items
                .iter()
                .all(|item| context_items_by_type_id.contains_key(&item.source_type_id))
        );
        let layout_index = LayoutIndex::from_codegen_unit(context_unit);
        let reflected_base_type_ids = reflected_base_type_ids(context_unit);
        let abstract_value_base_type_ids =
            abstract_value_base_type_ids(emitted_unit, context_items_by_type_id);
        let emitted_type_ids = emitted_unit
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();
        let name_plan = self.name_plan_for_codegen_unit(
            emitted_unit,
            context_items_by_type_id,
            &reflected_base_type_ids,
            &layout_index,
            &abstract_value_base_type_ids,
        );
        let ordered_items = dependency_ordered_codegen_items(emitted_unit)
            .into_iter()
            .filter(|item| !item.is_reflection_marker)
            .collect::<Vec<_>>();
        let mut items = context.runner().map_init(
            &ordered_items,
            RustPlanWorkerState::default,
            |state, item| {
                self.plan_serialize_item(
                    item,
                    context_items_by_type_id,
                    &mut state.eq_cache,
                    &mut state.default_cache,
                    &mut state.hash_cache,
                    &mut state.copy_cache,
                    &mut state.marshaler_cache,
                    &mut state.serde_cache,
                    &name_plan,
                    &reflected_base_type_ids,
                    &layout_index,
                    &abstract_value_base_type_ids,
                    &emitted_type_ids,
                )
            },
        );
        prune_derives_for_planned_dependencies(&mut items, &name_plan);
        prune_integrated_hash_derives(&mut items, self.options.mode, &name_plan);
        RustCodegenUnit { items }
    }

    fn name_plan_for_codegen_unit(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        reflected_base_type_ids: &BTreeSet<Uuid>,
        layout_index: &LayoutIndex,
        abstract_value_base_type_ids: &BTreeSet<Uuid>,
    ) -> RustNamePlan {
        let emitted_items = emitted_unit
            .items
            .iter()
            .filter(|item| !item.is_reflection_marker)
            .collect::<Vec<_>>();
        match self.options.mode {
            RustCodegenMode::Integrated => {
                let family_symbol_module_paths = standalone_family_symbol_module_paths_by_candidate(
                    &emitted_items,
                    items_by_type_id,
                    reflected_base_type_ids,
                    layout_index,
                );
                let scopes_by_type_id = emitted_items
                    .iter()
                    .map(|item| {
                        (
                            item.source_type_id,
                            rust_symbol_scope_key(
                                item,
                                items_by_type_id,
                                reflected_base_type_ids,
                                layout_index,
                                &family_symbol_module_paths,
                            ),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let scoped_entries = emitted_items
                    .iter()
                    .map(|item| {
                        (
                            item.source_type_id,
                            scopes_by_type_id
                                .get(&item.source_type_id)
                                .cloned()
                                .expect("emitted type should have a symbol scope"),
                            rust_candidate_type_name(item, abstract_value_base_type_ids),
                        )
                    })
                    .collect::<Vec<_>>();
                RustNamePlan::scoped_candidates_with_root(
                    scoped_entries,
                    scopes_by_type_id,
                    ["crate", "generated"],
                )
            }
            RustCodegenMode::Standalone => {
                let family_symbol_module_paths = standalone_family_symbol_module_paths_by_candidate(
                    &emitted_items,
                    items_by_type_id,
                    reflected_base_type_ids,
                    layout_index,
                );
                let scopes_by_type_id = emitted_items
                    .iter()
                    .map(|item| {
                        (
                            item.source_type_id,
                            rust_symbol_scope_key(
                                item,
                                items_by_type_id,
                                reflected_base_type_ids,
                                layout_index,
                                &family_symbol_module_paths,
                            ),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let scoped_entries = emitted_items
                    .iter()
                    .map(|item| {
                        (
                            item.source_type_id,
                            scopes_by_type_id
                                .get(&item.source_type_id)
                                .cloned()
                                .expect("emitted type should have a symbol scope"),
                            rust_candidate_type_name(item, abstract_value_base_type_ids),
                        )
                    })
                    .collect::<Vec<_>>();
                RustNamePlan::scoped_candidates_with_root(
                    scoped_entries,
                    scopes_by_type_id,
                    ["crate", "types"],
                )
            }
        }
    }

    fn plan_serialize_item(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        eq_cache: &mut BTreeMap<Uuid, bool>,
        default_cache: &mut BTreeMap<Uuid, bool>,
        hash_cache: &mut BTreeMap<Uuid, bool>,
        copy_cache: &mut BTreeMap<Uuid, bool>,
        marshaler_cache: &mut BTreeMap<Uuid, bool>,
        serde_cache: &mut BTreeMap<Uuid, bool>,
        name_plan: &RustNamePlan,
        reflected_base_type_ids: &BTreeSet<Uuid>,
        layout_index: &LayoutIndex,
        abstract_value_base_type_ids: &BTreeSet<Uuid>,
        emitted_type_ids: &BTreeSet<Uuid>,
    ) -> RustItemPlan {
        match item.kind {
            SerializeCodegenItemKind::Struct => self.plan_serialize_struct(
                item,
                items_by_type_id,
                eq_cache,
                default_cache,
                hash_cache,
                copy_cache,
                marshaler_cache,
                serde_cache,
                name_plan,
                reflected_base_type_ids,
                layout_index,
                abstract_value_base_type_ids,
                emitted_type_ids,
            ),
            SerializeCodegenItemKind::Enum => self.plan_serialize_enum(
                item,
                items_by_type_id,
                name_plan,
                reflected_base_type_ids,
                layout_index,
            ),
        }
    }

    fn plan_serialize_struct(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        eq_cache: &mut BTreeMap<Uuid, bool>,
        default_cache: &mut BTreeMap<Uuid, bool>,
        hash_cache: &mut BTreeMap<Uuid, bool>,
        copy_cache: &mut BTreeMap<Uuid, bool>,
        marshaler_cache: &mut BTreeMap<Uuid, bool>,
        serde_cache: &mut BTreeMap<Uuid, bool>,
        name_plan: &RustNamePlan,
        reflected_base_type_ids: &BTreeSet<Uuid>,
        layout_index: &LayoutIndex,
        abstract_value_base_type_ids: &BTreeSet<Uuid>,
        emitted_type_ids: &BTreeSet<Uuid>,
    ) -> RustItemPlan {
        if let Some(sum_enum) = self.plan_abstract_sum_enum(
            item,
            items_by_type_id,
            name_plan,
            reflected_base_type_ids,
            layout_index,
            abstract_value_base_type_ids,
            emitted_type_ids,
        ) {
            return sum_enum;
        }

        let is_bevy_component = is_bevy_component_role(item);
        let identity = self.rust_identity_for_struct(item, is_bevy_component);
        let is_slot_owner = layout_index.has_concrete_slot_children(item);
        let has_layout_family_descendants = layout_index.has_layout_family_descendants(item);
        let type_path = layout_index.type_path(item, items_by_type_id);
        let current_module = current_module_path(&type_path.scope_segments, &type_path.file_stem);
        let fields = self.field_planner.plan_struct_fields(
            item,
            items_by_type_id,
            name_plan,
            self.source_types.as_ref(),
            &current_module,
        );
        let rtti_bases = self.field_planner.plan_rtti_bases(
            item,
            items_by_type_id,
            name_plan,
            self.source_types.as_ref(),
            &current_module,
        );
        let mut derives = self.derive_planner.plan_struct_derives(
            is_bevy_component,
            identity.kind,
            item,
            items_by_type_id,
            eq_cache,
            default_cache,
            hash_cache,
            copy_cache,
            marshaler_cache,
            serde_cache,
        );
        prune_integrated_derives_for_rendered_fields(self.options.mode, &mut derives, &fields);
        prune_custom_field_identity_derives(self.options.mode, &mut derives, item);
        if self.manual_default_type_ids.contains(&item.source_type_id) {
            remove_default_derive(&mut derives);
        }

        RustItemPlan {
            source_type_id: item.source_type_id,
            source_name: item.source_name.clone(),
            is_reflected_base: reflected_base_type_ids.contains(&item.source_type_id),
            is_slot_owner,
            has_layout_family_descendants,
            is_bevy_component,
            file_stem_override: Some(type_path.file_stem),
            scope_path: type_path.scope_segments,
            family_scope_path: if is_slot_owner {
                layout_index.concrete_slot_owner_scope_segments(item, items_by_type_id)
            } else {
                layout_index.inheritance_family_scope_segments(item, items_by_type_id)
            },
            rust_name: name_plan.definition_name(item),
            kind: RustItemKind::Struct,
            identity,
            repr: None,
            raw_conversion: None,
            derives,
            rtti_bases,
            fields,
            variants: Vec::new(),
        }
    }

    fn plan_abstract_sum_enum(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        name_plan: &RustNamePlan,
        reflected_base_type_ids: &BTreeSet<Uuid>,
        layout_index: &LayoutIndex,
        abstract_value_base_type_ids: &BTreeSet<Uuid>,
        emitted_type_ids: &BTreeSet<Uuid>,
    ) -> Option<RustItemPlan> {
        if !abstract_value_base_type_ids.contains(&item.source_type_id)
            || item.is_abstract != Some(true)
            || item_has_materialized_payload(item, items_by_type_id)
        {
            return None;
        }

        let direct_type_ids = layout_index
            .direct_derived_type_ids_by_base_type_id
            .get(&item.source_type_id)?;
        if direct_type_ids.is_empty()
            || !direct_type_ids
                .iter()
                .all(|type_id| emitted_type_ids.contains(type_id))
        {
            return None;
        }

        let children = direct_type_ids
            .iter()
            .filter_map(|type_id| items_by_type_id.get(type_id).copied())
            .filter(|child| child.is_abstract != Some(true))
            .collect::<Vec<_>>();
        if children.len() != direct_type_ids.len() {
            return None;
        }

        let type_path = layout_index.type_path(item, items_by_type_id);
        let variants = children
            .iter()
            .map(|child| {
                let has_payload = item_has_materialized_payload(child, items_by_type_id);
                RustVariantPlan {
                    source_name: child.source_name.clone(),
                    rust_name: abstract_sum_variant_name(item, child, name_plan),
                    discriminant: None,
                    payload_type: has_payload.then(|| {
                        name_plan.reference_name(child.source_type_id, &child.source_name)
                    }),
                }
            })
            .collect::<Vec<_>>();
        let derives = self.plan_sum_enum_derives(&variants);

        Some(RustItemPlan {
            source_type_id: item.source_type_id,
            source_name: item.source_name.clone(),
            is_reflected_base: reflected_base_type_ids.contains(&item.source_type_id),
            is_slot_owner: false,
            has_layout_family_descendants: layout_index.has_layout_family_descendants(item),
            is_bevy_component: false,
            file_stem_override: Some(type_path.file_stem),
            scope_path: type_path.scope_segments,
            family_scope_path: layout_index
                .inheritance_family_scope_segments(item, items_by_type_id),
            rust_name: name_plan.definition_name(item),
            kind: RustItemKind::SumEnum,
            identity: RustTypeIdentityPlan::az_rtti(
                item.source_type_id,
                Some(item.source_name.clone()),
            ),
            repr: None,
            raw_conversion: None,
            derives,
            rtti_bases: Vec::new(),
            fields: Vec::new(),
            variants,
        })
    }

    fn plan_sum_enum_derives(&self, variants: &[RustVariantPlan]) -> Vec<String> {
        let mut derives = match self.options.mode {
            RustCodegenMode::Integrated => vec!["AzRtti".to_owned(), "Debug".to_owned()],
            RustCodegenMode::Standalone => vec!["Debug".to_owned()],
        };
        if default_sum_variant(variants).is_some() {
            derives.push("Default".to_owned());
        }
        derives.extend([
            "Clone".to_owned(),
            "PartialEq".to_owned(),
            "Reflect".to_owned(),
        ]);
        derives
    }

    fn plan_serialize_enum(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        name_plan: &RustNamePlan,
        reflected_base_type_ids: &BTreeSet<Uuid>,
        layout_index: &LayoutIndex,
    ) -> RustItemPlan {
        let raw_conversion = self.enum_planner.plan_raw_conversion(item);
        let has_alias_values = enum_has_duplicate_discriminants(item);
        let kind = if raw_conversion.is_some() && has_alias_values {
            RustItemKind::RawEnum
        } else {
            RustItemKind::Enum
        };
        let type_path = layout_index.type_path(item, items_by_type_id);
        let mut derives = if kind == RustItemKind::RawEnum {
            self.derive_planner.plan_raw_enum_derives()
        } else {
            self.derive_planner.plan_enum_derives(item)
        };
        if self.manual_default_type_ids.contains(&item.source_type_id) {
            remove_default_derive(&mut derives);
        }

        RustItemPlan {
            source_type_id: item.source_type_id,
            source_name: item.source_name.clone(),
            is_reflected_base: reflected_base_type_ids.contains(&item.source_type_id),
            is_slot_owner: false,
            has_layout_family_descendants: layout_index.has_layout_family_descendants(item),
            is_bevy_component: false,
            file_stem_override: Some(type_path.file_stem),
            scope_path: type_path.scope_segments,
            family_scope_path: layout_index
                .inheritance_family_scope_segments(item, items_by_type_id),
            rust_name: name_plan.definition_name(item),
            kind,
            identity: RustTypeIdentityPlan::az_type_info(
                item.source_type_id,
                Some(item.source_name.clone()),
            ),
            repr: raw_conversion
                .as_ref()
                .map(|conversion| conversion.raw_type.clone()),
            raw_conversion,
            derives,
            rtti_bases: Vec::new(),
            fields: Vec::new(),
            variants: self.enum_planner.plan_variants(item),
        }
    }

    fn rust_identity_for_struct(
        &self,
        item: &SerializeCodegenItem,
        is_bevy_component: bool,
    ) -> RustTypeIdentityPlan {
        let name = Some(item.source_name.clone());
        if is_bevy_component || item.is_abstract.is_some() || !item.rtti_base_chain.is_empty() {
            RustTypeIdentityPlan::az_rtti(item.source_type_id, name)
        } else {
            RustTypeIdentityPlan::az_type_info(item.source_type_id, name)
        }
    }
}

fn prune_integrated_derives_for_rendered_fields(
    mode: RustCodegenMode,
    derives: &mut Vec<String>,
    fields: &[crate::rust::item_plan::RustFieldPlan],
) {
    if !matches!(mode, RustCodegenMode::Integrated) {
        return;
    }
    if fields
        .iter()
        .any(|field| field.rust_type.contains("az_asset::AssetReference"))
    {
        derives.retain(|derive| derive != "Hash");
    }
}

fn prune_custom_field_identity_derives(
    mode: RustCodegenMode,
    derives: &mut Vec<String>,
    item: &SerializeCodegenItem,
) {
    if matches!(mode, RustCodegenMode::Integrated)
        && integrated_custom_field_type(&item.source_name).is_some()
    {
        derives.retain(|derive| derive != "Marshaler");
    }
}

fn abstract_value_base_type_ids(
    emitted_unit: &SerializeCodegenUnit,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeSet<Uuid> {
    let mut referenced_type_ids = BTreeSet::new();
    for item in &emitted_unit.items {
        for field in &item.fields {
            if !field.is_base_class {
                collect_resolved_named_type_ids(&field.resolved_type, &mut referenced_type_ids);
            }
        }
        if let Some(resolved) = &item.enum_underlying_type {
            collect_resolved_named_type_ids(resolved, &mut referenced_type_ids);
        }
    }

    referenced_type_ids
        .into_iter()
        .filter(|type_id| {
            items_by_type_id.get(type_id).is_some_and(|item| {
                item.is_abstract == Some(true)
                    && !item_has_materialized_payload(item, items_by_type_id)
            })
        })
        .collect()
}

fn rust_candidate_type_name(
    item: &SerializeCodegenItem,
    abstract_value_base_type_ids: &BTreeSet<Uuid>,
) -> String {
    let candidate = rust_type_ident(&item.source_name);
    if abstract_value_base_type_ids.contains(&item.source_type_id)
        && let Some(value_name) = candidate.strip_suffix("Base")
        && !value_name.is_empty()
    {
        return value_name.to_owned();
    }
    candidate
}

fn abstract_sum_variant_name(
    base: &SerializeCodegenItem,
    child: &SerializeCodegenItem,
    name_plan: &RustNamePlan,
) -> String {
    let base_name = name_plan.definition_name(base);
    let child_name = name_plan.reference_name(child.source_type_id, &child.source_name);
    let variant = child_name
        .strip_prefix(&base_name)
        .filter(|suffix| !suffix.is_empty())
        .unwrap_or(&child_name);
    rust_variant_ident(variant)
}

fn default_sum_variant(variants: &[RustVariantPlan]) -> Option<&RustVariantPlan> {
    variants
        .iter()
        .filter(|variant| variant.payload_type.is_none())
        .find(|variant| {
            matches!(
                variant.rust_name.as_str(),
                "None" | "Invalid" | "Disabled" | "Point"
            ) || variant.rust_name.ends_with("None")
                || variant.rust_name.ends_with("Invalid")
                || variant.rust_name.ends_with("Disabled")
        })
        .or_else(|| {
            variants
                .iter()
                .find(|variant| variant.payload_type.is_none())
        })
}

fn remove_default_derive(derives: &mut Vec<String>) {
    derives.retain(|derive| derive != "Default");
}

fn prune_derives_for_planned_dependencies(items: &mut [RustItemPlan], name_plan: &RustNamePlan) {
    let mut changed = true;
    while changed {
        changed = false;
        let derive_names_by_type = derive_names_by_type(items, name_plan);

        for item in items.iter_mut() {
            let dependency_types = item_dependency_types(item);
            let before = item.derives.clone();
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Copy",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "PartialEq",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Eq",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "PartialOrd",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Ord",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Hash",
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Marshaler",
            );
            prune_serde_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
            );
            prune_derive_if_dependency_missing(
                &mut item.derives,
                &dependency_types,
                &derive_names_by_type,
                "Reflect",
            );
            if item.derives != before {
                changed = true;
            }
        }
    }
}

fn prune_derive_if_dependency_missing(
    derives: &mut Vec<String>,
    dependency_types: &[String],
    derive_names_by_type: &BTreeMap<String, BTreeSet<String>>,
    derive_name: &str,
) {
    if !derives.iter().any(|derive| derive == derive_name)
        || dependency_types.iter().all(|rust_type| {
            field_dependencies_support(rust_type, derive_names_by_type, derive_name)
        })
    {
        return;
    }
    derives.retain(|derive| derive != derive_name);
    match derive_name {
        "PartialEq" => derives
            .retain(|derive| !matches!(derive.as_str(), "Eq" | "PartialOrd" | "Ord" | "Hash")),
        "Eq" => derives.retain(|derive| !matches!(derive.as_str(), "PartialOrd" | "Ord")),
        "PartialOrd" => derives.retain(|derive| derive != "Ord"),
        _ => {}
    }
}

fn prune_serde_if_dependency_missing(
    derives: &mut Vec<String>,
    dependency_types: &[String],
    derive_names_by_type: &BTreeMap<String, BTreeSet<String>>,
) {
    let needs_serialize = derives.iter().any(|derive| derive == "Serialize");
    let needs_deserialize = derives.iter().any(|derive| derive == "Deserialize");
    if !(needs_serialize || needs_deserialize)
        || dependency_types
            .iter()
            .all(|rust_type| field_dependencies_support_serde(rust_type, derive_names_by_type))
    {
        return;
    }
    derives.retain(|derive| !matches!(derive.as_str(), "Serialize" | "Deserialize"));
}

fn item_dependency_types(item: &RustItemPlan) -> Vec<String> {
    item.fields
        .iter()
        .map(|field| field.rust_type.clone())
        .chain(
            item.variants
                .iter()
                .filter_map(|variant| variant.payload_type.clone()),
        )
        .collect()
}

fn field_dependencies_support(
    rust_type: &str,
    derive_names_by_type: &BTreeMap<String, BTreeSet<String>>,
    derive_name: &str,
) -> bool {
    if !rust_wrappers_support_derive(rust_type, derive_name) {
        return false;
    }
    derive_names_by_type.iter().all(|(type_name, derives)| {
        !rust_type_mentions_name(rust_type, type_name) || derives.contains(derive_name)
    })
}

fn field_dependencies_support_serde(
    rust_type: &str,
    derive_names_by_type: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    derive_names_by_type.iter().all(|(type_name, derives)| {
        !rust_type_mentions_name(rust_type, type_name)
            || (derives.contains("Serialize") && derives.contains("Deserialize"))
    })
}

fn prune_integrated_hash_derives(
    items: &mut [RustItemPlan],
    mode: RustCodegenMode,
    name_plan: &RustNamePlan,
) {
    if !matches!(mode, RustCodegenMode::Integrated) {
        return;
    }

    let mut changed = true;
    while changed {
        changed = false;
        let non_hash_types = items
            .iter()
            .filter(|item| !item.derives.iter().any(|derive| derive == "Hash"))
            .map(|item| plan_reference_name(item, name_plan))
            .collect::<Vec<_>>();

        for item in items.iter_mut() {
            if !item.derives.iter().any(|derive| derive == "Hash") {
                continue;
            }
            if item.fields.iter().any(|field| {
                non_hash_types
                    .iter()
                    .any(|type_name| rust_type_mentions_name(&field.rust_type, type_name.as_str()))
            }) {
                item.derives.retain(|derive| derive != "Hash");
                changed = true;
            }
        }
    }
}

fn derive_names_by_type(
    items: &[RustItemPlan],
    name_plan: &RustNamePlan,
) -> BTreeMap<String, BTreeSet<String>> {
    items
        .iter()
        .map(|item| {
            (
                plan_reference_name(item, name_plan),
                item.derives.iter().cloned().collect(),
            )
        })
        .collect()
}

fn plan_reference_name(item: &RustItemPlan, name_plan: &RustNamePlan) -> String {
    name_plan.reference_name(item.source_type_id, &item.source_name)
}

fn rust_wrappers_support_derive(rust_type: &str, derive_name: &str) -> bool {
    if rust_type_mentions_name(rust_type, "Box") {
        !matches!(derive_name, "Copy" | "Marshaler" | "Reflect")
    } else {
        true
    }
}

fn rust_type_mentions_name(rust_type: &str, name: &str) -> bool {
    rust_type.match_indices(name).any(|(start, _)| {
        let end = start + name.len();
        is_rust_ident_boundary(rust_type[..start].chars().next_back())
            && is_rust_ident_boundary(rust_type[end..].chars().next())
    })
}

fn is_rust_ident_boundary(ch: Option<char>) -> bool {
    !ch.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn current_module_path(scope_segments: &[String], file_stem: &str) -> String {
    let mut segments = scope_segments.to_vec();
    segments.push(file_stem.to_owned());
    segments.join("::")
}

fn is_bevy_component_role(item: &SerializeCodegenItem) -> bool {
    if item.is_abstract == Some(true) {
        return false;
    }
    match item.role {
        ReflectedTypeRole::FacetedComponent | ReflectedTypeRole::AzComponent => true,
        ReflectedTypeRole::ClientFacet | ReflectedTypeRole::ServerFacet => !matches!(
            source_leaf_name(&item.source_name),
            "ClientFacet" | "ServerFacet"
        ),
        ReflectedTypeRole::AzEntity | ReflectedTypeRole::SupportType => false,
    }
}

fn source_leaf_name(source_name: &str) -> &str {
    source_name.rsplit("::").next().unwrap_or(source_name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nw_objectstream::type_uuid::type_ids;
    use serde_json::json;
    use uuid::uuid;

    use crate::document::SerializeContextDocument;
    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenPlanner, SerializeCodegenSelection, SerializeCodegenUnit,
        SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::rust::enum_plan::RustEnumRawConversionPlan;
    use crate::rust::identity::RustTypeIdentityKind;
    use crate::rust::integrate::source_index::RustSourceTypeIndex;
    use crate::rust::item_plan::RustIntegerRangePlan;
    use crate::rust::layout::RustStandaloneLayoutReport;
    use crate::types::{MapKind, ResolvedType, ScalarType, SequenceKind};

    use super::*;

    #[test]
    fn plans_structs_and_enums_from_serialize_model() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "Example::CounterComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [{
                        "$id": 11,
                        "name": "m_count",
                        "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                        "offset": "4",
                        "flags": 0,
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "22222222-2222-2222-2222-222222222222",
                    {
                        "$id": 20,
                        "name": "Mode",
                        "attributes": [[1, {
                            "$id": 21,
                            "attributeId": 1,
                            "attributeName": "EnumValue",
                            "value": {
                                "kind": "enumConstant",
                                "valueI32": 7,
                                "description": "Enabled"
                            }
                        }]]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "22222222-2222-2222-2222-222222222222": type_ids::U8.hyphenated().to_string()
            }
        }));

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component plan");
        let mode = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("22222222-2222-2222-2222-222222222222"))
            .expect("enum plan");

        assert_eq!(component.rust_name, "CounterComponent");
        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzTypeInfo);
        assert_eq!(component.identity.type_id, component.source_type_id);
        assert!(
            component
                .derives
                .iter()
                .any(|derive_name| derive_name == "AzTypeInfo")
        );
        assert_eq!(component.fields[0].rust_name, "count");
        assert_eq!(component.fields[0].rust_type, "u32");
        assert_eq!(mode.kind, RustItemKind::Enum);
        assert_eq!(mode.identity.kind, RustTypeIdentityKind::AzTypeInfo);
        assert_eq!(mode.repr.as_deref(), Some("u8"));
        assert_eq!(
            mode.raw_conversion,
            Some(RustEnumRawConversionPlan {
                raw_type: "u8".to_owned()
            })
        );
        assert_eq!(mode.variants[0].rust_name, "Enabled");
        assert_eq!(mode.variants[0].discriminant, Some(7));
    }

    #[test]
    fn integrated_planner_derives_marshaler_for_color_scalars() {
        fn color_field(name: &str, scalar: ScalarType) -> SerializeCodegenField {
            SerializeCodegenField {
                source_name: name.to_owned(),
                source_type_id: match scalar {
                    ScalarType::Color => type_ids::COLOR,
                    ScalarType::ColorF => type_ids::COLORF,
                    ScalarType::ColorB => type_ids::COLORB,
                    _ => unreachable!("test only covers color scalars"),
                },
                resolved_type: ResolvedType::Scalar(scalar),
                data_size: None,
                offset: None,
                flags: None,
                is_base_class: false,
                is_pointer: false,
                is_dynamic_field: false,
            }
        }

        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                    source_name: "FloatColors".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![
                        color_field("m_color", ScalarType::Color),
                        color_field("m_colorF", ScalarType::ColorF),
                    ],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    source_name: "ByteColor".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![color_field("m_colorB", ScalarType::ColorB)],
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit = RustCodegenPlanner::default()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let float_colors = rust_unit
            .items
            .iter()
            .find(|item| item.rust_name == "FloatColors")
            .expect("float color item");
        let byte_color = rust_unit
            .items
            .iter()
            .find(|item| item.rust_name == "ByteColor")
            .expect("byte color item");

        assert!(
            float_colors
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
        assert!(
            byte_color
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
    }

    #[test]
    fn integrated_planner_resolves_unresolved_member_rtti_through_source_type_index() {
        let external_type_id = uuid!("11111111-2222-3333-4444-555555555555");
        let source_types = RustSourceTypeIndex::from_source(
            Path::new("components"),
            Path::new("components/shared/external.rs"),
            r#"
use az_derive::AzRtti;

#[derive(AzRtti)]
#[az_rtti("11111111-2222-3333-4444-555555555555")]
pub struct ExternalPayload;
"#,
        )
        .expect("source type index");

        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                source_name: "OwnerData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_payloads".to_owned(),
                    source_type_id: external_type_id,
                    resolved_type: ResolvedType::Sequence {
                        kind: SequenceKind::Vector,
                        element: Box::new(ResolvedType::Unknown {
                            type_id: external_type_id,
                            reason: "reflected type `ExternalPayload` is present on member type metadata but not as a SerializeContext class"
                                .to_owned(),
                        }),
                        capacity: None,
                    },
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let rust_unit = RustCodegenPlanner::default()
            .with_source_type_index(source_types)
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let owner = rust_unit.items.first().expect("owner item");
        let field = owner.fields.first().expect("payload field");

        assert_eq!(
            field.rust_type,
            "Vec<crate::shared::external::ExternalPayload>"
        );
        assert!(field.unresolved_type.is_none());
    }

    #[test]
    fn plans_enum_scope_with_full_type_graph() {
        let owner_id = uuid!("11111111-1111-1111-1111-111111111111");
        let enum_id = uuid!("22222222-2222-2222-2222-222222222222");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: owner_id,
                    source_name: "Example::CounterComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_mode".to_owned(),
                        source_type_id: enum_id,
                        resolved_type: ResolvedType::Named {
                            type_id: enum_id,
                            source_name: "Mode".to_owned(),
                        },
                        data_size: Some(1),
                        offset: Some(0),
                        flags: Some(0),
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: enum_id,
                    source_name: "Mode".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Enum,
                    enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::U8)),
                    fields: Vec::new(),
                    variants: vec![SerializeCodegenVariant {
                        source_name: "Enabled".to_owned(),
                        value_u64: Some(1),
                        value_u32: Some(1),
                        value_i32: Some(1),
                    }],
                },
            ],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());
        let mode = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == enum_id)
            .expect("Mode enum plan");

        assert_eq!(
            mode.scope_path,
            vec!["example", "components", "counter_component"]
        );
        assert_eq!(
            mode.family_scope_path,
            vec!["example", "components", "counter_component", "mode"]
        );
    }

    #[test]
    fn plans_fixed_opaque_byte_fields_without_missing_type_placeholder() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::OpaquePayload".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "payload".to_owned(),
                    source_type_id: missing_id,
                    resolved_type: ResolvedType::Unknown {
                        type_id: missing_id,
                        reason: "type id is not present in SerializeContext".to_owned(),
                    },
                    data_size: Some(16),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());
        let field = &rust_unit.items[0].fields[0];

        assert_eq!(field.rust_type, "[u8; 16]");
        assert!(field.unresolved_type.is_none());
    }

    #[test]
    fn integrated_plan_emits_actor_ref_identity_with_custom_field_payload() {
        let actor_ref_id = uuid!("0638e28c-ab7b-4ba4-84ac-0353038e6fdc");
        let client_ref_id = uuid!("c148c555-3264-41f7-a335-e48b65f91728");
        let clients_group_id = uuid!("ad006b8d-474d-418f-98aa-cc5b1cd87e78");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: actor_ref_id,
                    source_name: "Amazon::Hub::ActorRef".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: client_ref_id,
                    source_name: "ClientRef".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_clientRef".to_owned(),
                        source_type_id: actor_ref_id,
                        resolved_type: ResolvedType::Named {
                            type_id: actor_ref_id,
                            source_name: "Amazon::Hub::ActorRef".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: clients_group_id,
                    source_name: "ClientsGroup".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_clients".to_owned(),
                        source_type_id: client_ref_id,
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::Set,
                            element: Box::new(ResolvedType::Named {
                                type_id: client_ref_id,
                                source_name: "ClientRef".to_owned(),
                            }),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());

        let actor_ref = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == actor_ref_id)
            .expect("ActorRef identity plan");
        assert!(!actor_ref.derives.iter().any(|derive| derive == "Marshaler"));
        let client_ref = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == client_ref_id)
            .expect("ClientRef plan");
        assert_eq!(
            client_ref.fields[0].rust_type,
            "crate::refs::ClientActorRef"
        );
        assert!(
            client_ref
                .derives
                .iter()
                .any(|derive| derive == "PartialOrd")
        );
        assert!(client_ref.derives.iter().any(|derive| derive == "Ord"));

        let clients_group = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == clients_group_id)
            .expect("ClientsGroup plan");
        assert!(
            clients_group
                .derives
                .iter()
                .any(|derive| derive == "PartialEq")
        );
        assert!(clients_group.derives.iter().any(|derive| derive == "Eq"));
        assert!(
            clients_group
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
        assert!(
            clients_group
                .derives
                .iter()
                .any(|derive| derive == "Reflect")
        );
    }

    #[test]
    fn integrated_plan_emits_identity_types_with_custom_field_payloads() {
        let home_point_list_id = uuid!("5564452c-a740-496c-bf76-64d6550c7986");
        let persistent_player_home_data_id = uuid!("11111111-2222-3333-4444-555555555555");
        let crit_window_id = uuid!("81af21f2-caf1-4212-99e6-2e2e8d9d84e8");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: home_point_list_id,
                    source_name: "HomePointList".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: crit_window_id,
                    source_name: "CritWindow".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: persistent_player_home_data_id,
                    source_name: "PersistentPlayerHomeData".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_homePoints".to_owned(),
                        source_type_id: home_point_list_id,
                        resolved_type: ResolvedType::Named {
                            type_id: home_point_list_id,
                            source_name: "HomePointList".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());

        let home_point_list = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == home_point_list_id)
            .expect("HomePointList identity plan");
        assert!(
            !home_point_list
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
        let crit_window = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == crit_window_id)
            .expect("CritWindow identity plan");
        assert!(
            !crit_window
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
        let persistent_player_home_data = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == persistent_player_home_data_id)
            .expect("PersistentPlayerHomeData plan");
        assert_eq!(
            persistent_player_home_data.fields[0].rust_type,
            "::gridmate::serialize::ReplicatedVec<crate::housing::player_home::HomePointReplicatedState>"
        );
    }

    #[test]
    fn integrated_plan_does_not_order_entity_id_fields() {
        let local_entity_ref_id = uuid!("ea5fe48a-66f7-42d7-ee11-12391c964778");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: local_entity_ref_id,
                source_name: "LocalEntityRef".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "EntityId".to_owned(),
                    source_type_id: nw_objectstream::type_uuid::type_ids::ENTITY_ID,
                    resolved_type: ResolvedType::Scalar(ScalarType::EntityId),
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());
        let local_entity_ref = &rust_unit.items[0];

        assert!(
            !local_entity_ref
                .derives
                .iter()
                .any(|derive| derive == "PartialOrd")
        );
        assert!(
            !local_entity_ref
                .derives
                .iter()
                .any(|derive| derive == "Ord")
        );
        assert!(
            local_entity_ref
                .derives
                .iter()
                .any(|derive| derive == "Marshaler")
        );
    }

    #[test]
    fn standalone_plan_drops_default_for_large_fixed_opaque_byte_fields() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::KeybindData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_keybind".to_owned(),
                    source_type_id: missing_id,
                    resolved_type: ResolvedType::Unknown {
                        type_id: missing_id,
                        reason: "type id is not present in SerializeContext".to_owned(),
                    },
                    data_size: Some(40),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let item = &rust_unit.items[0];

        assert_eq!(item.fields[0].rust_type, "[u8; 40]");
        assert!(
            !item
                .derives
                .iter()
                .any(|derive_name| derive_name == "Default")
        );
        assert!(item.derives.iter().any(|derive_name| derive_name == "Eq"));
        assert!(item.derives.iter().any(|derive_name| derive_name == "Ord"));
        assert!(
            !item
                .derives
                .iter()
                .any(|derive_name| derive_name == "Serialize")
        );
        assert!(
            !item
                .derives
                .iter()
                .any(|derive_name| derive_name == "Deserialize")
        );
    }

    #[test]
    fn planner_can_leave_default_for_local_impl() {
        let type_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: type_id,
                source_name: "Example::EventData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![SerializeCodegenField {
                    source_name: "m_applyRecursively".to_owned(),
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    resolved_type: ResolvedType::Scalar(ScalarType::Bool),
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };

        let rust_unit = RustCodegenPlanner::default()
            .without_default_derive_for([type_id])
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let item = &rust_unit.items[0];

        assert!(
            !item
                .derives
                .iter()
                .any(|derive_name| derive_name == "Default")
        );
    }

    #[test]
    fn standalone_plan_uses_plain_sequences_when_native_rust_keys_are_invalid() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::CollectionData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![
                    SerializeCodegenField {
                        source_name: "m_floatLookup".to_owned(),
                        source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::F32)),
                            value: Box::new(ResolvedType::Scalar(ScalarType::String)),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "m_floatSet".to_owned(),
                        source_type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::Set,
                            element: Box::new(ResolvedType::Scalar(ScalarType::F32)),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "m_nameLookup".to_owned(),
                        source_type_id: uuid!("44444444-4444-4444-4444-444444444444"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::String)),
                            value: Box::new(ResolvedType::Scalar(ScalarType::U32)),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
                variants: Vec::new(),
            }],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let item = &rust_unit.items[0];

        assert_eq!(item.fields[0].rust_type, "Vec<(f32, String)>");
        assert_eq!(item.fields[1].rust_type, "Vec<f32>");
        assert_eq!(
            item.fields[2].rust_type,
            "std::collections::HashMap<String, u32>"
        );
        assert!(
            item.derives
                .iter()
                .any(|derive_name| derive_name == "PartialEq")
        );
        assert!(!item.derives.iter().any(|derive_name| derive_name == "Eq"));
        assert!(!item.derives.iter().any(|derive_name| derive_name == "Hash"));
    }

    #[test]
    fn integrated_planner_keeps_partial_eq_for_float_data_without_eq_or_hash() {
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                    source_name: "Example::FloatData".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_value".to_owned(),
                        source_type_id: type_ids::FLOAT,
                        resolved_type: ResolvedType::Scalar(ScalarType::F32),
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    source_name: "Example::SlotData".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_floatData".to_owned(),
                        source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                        resolved_type: ResolvedType::Named {
                            type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                            source_name: "Example::FloatData".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit = RustCodegenPlanner::default()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        for item_name in ["FloatData", "SlotData"] {
            let item = rust_unit
                .items
                .iter()
                .find(|item| item.rust_name == item_name)
                .expect("planned item");

            assert!(
                item.derives
                    .iter()
                    .any(|derive_name| derive_name == "PartialEq"),
                "{item_name} derives: {:?}",
                item.derives
            );
            assert!(!item.derives.iter().any(|derive_name| derive_name == "Eq"));
            assert!(!item.derives.iter().any(|derive_name| derive_name == "Hash"));
        }
    }

    #[test]
    fn plan_model_uses_shared_ir_roles_for_components() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_count",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component plan");

        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzRtti);
        assert!(component.is_bevy_component);
        assert_eq!(
            component.derives,
            vec![
                "Component",
                "AzRtti",
                "Debug",
                "Default",
                "Clone",
                "Copy",
                "PartialEq",
                "Eq",
                "PartialOrd",
                "Ord",
                "Hash",
                "Marshaler",
                "Serialize",
                "Deserialize",
                "Reflect"
            ]
        );
        assert_eq!(component.fields[0].rust_type, "u32");
    }

    #[test]
    fn standalone_plan_keeps_component_semantics_without_engine_derives() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 20,
                    "name": "Example::TargetComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_entity",
                            "typeId": type_ids::ENTITY_ID.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 23,
                            "name": "m_asset",
                            "typeId": type_ids::AZ_DATA_ASSET_ID.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 24,
                            "name": "m_tag",
                            "typeId": type_ids::CRC32.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 25,
                            "name": "m_owner",
                            "typeId": type_ids::AZ_UUID.hyphenated().to_string(),
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit =
            RustCodegenPlanner::plan_standalone_model(&model, &crate::CodegenContext::inline());
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component plan");

        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzRtti);
        assert!(component.is_bevy_component);
        assert_eq!(
            component.derives,
            vec![
                "Component",
                "Debug",
                "Default",
                "Clone",
                "Copy",
                "PartialEq",
                "Eq",
                "Hash",
                "Serialize",
                "Deserialize",
                "Reflect"
            ]
        );
        assert_eq!(component.fields[0].rust_type, "u64");
        assert_eq!(component.fields[1].rust_type, "AzAssetId");
        assert_eq!(component.fields[2].rust_type, "AzCrc32");
        assert_eq!(component.fields[3].rust_type, "AzUuid");
    }

    #[test]
    fn plans_concrete_facets_as_bevy_components() {
        let facet_id = uuid!("10000000-0000-0000-0000-000000000000");
        let client_facet_id = uuid!("20000000-0000-0000-0000-000000000000");
        let server_facet_id = uuid!("30000000-0000-0000-0000-000000000000");
        let concrete_client_id = uuid!("40000000-0000-0000-0000-000000000000");
        let concrete_server_id = uuid!("50000000-0000-0000-0000-000000000000");
        let unit = SerializeCodegenUnit {
            items: vec![
                facet_item(
                    facet_id,
                    "Facet",
                    ReflectedTypeRole::SupportType,
                    Vec::new(),
                ),
                facet_item(
                    client_facet_id,
                    "ClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    vec![base_field(facet_id, "Facet")],
                ),
                facet_item(
                    server_facet_id,
                    "ServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    vec![base_field(facet_id, "Facet")],
                ),
                facet_item(
                    concrete_client_id,
                    "ExampleComponentClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    vec![base_field(client_facet_id, "ClientFacet")],
                ),
                facet_item(
                    concrete_server_id,
                    "ExampleComponentServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    vec![base_field(server_facet_id, "ServerFacet")],
                ),
            ],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());
        let client_base = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == client_facet_id)
            .expect("client facet base");
        let server_base = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == server_facet_id)
            .expect("server facet base");
        let client_facet = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == concrete_client_id)
            .expect("client facet");
        let server_facet = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == concrete_server_id)
            .expect("server facet");

        assert!(!client_base.is_bevy_component);
        assert!(!server_base.is_bevy_component);
        assert!(client_facet.is_bevy_component);
        assert!(server_facet.is_bevy_component);
        assert!(
            client_facet
                .derives
                .iter()
                .any(|derive| derive == "Component")
        );
        assert!(
            server_facet
                .derives
                .iter()
                .any(|derive| derive == "Component")
        );
    }

    fn facet_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
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

    fn base_field(source_type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: source_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    #[test]
    fn plans_ranged_integer_metadata_with_core_range_type() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "Example::RangeComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [{
                        "$id": 11,
                        "name": "m_count",
                        "typeId": "33333333-3333-3333-3333-333333333333",
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [[
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
            ]],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component plan");

        assert_eq!(component.fields[0].rust_type, "u16");
        assert_eq!(
            component.fields[0].integer_range,
            Some(RustIntegerRangePlan {
                rust_type: "::core::ops::RangeInclusive<u16>".to_owned(),
                value_type: "u16".to_owned(),
                start: "0".to_owned(),
                last: "65535".to_owned(),
            })
        );
    }

    #[test]
    fn plans_rust_from_shared_serialize_codegen_unit() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "BaseClass1",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_count",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let serialize_unit = SerializeCodegenPlanner::plan_model(&model);

        let rust_unit = RustCodegenPlanner::plan_codegen_unit(
            &serialize_unit,
            &crate::CodegenContext::inline(),
        );
        let component = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component item");

        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzRtti);
        assert!(component.is_bevy_component);
        assert_eq!(component.fields.len(), 1);
        assert_eq!(component.fields[0].rust_name, "count");
    }

    #[test]
    fn standalone_plan_keeps_same_leaf_name_when_family_module_separates_symbol_scope() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: item_family_id,
                    source_name: "Item".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: item_record_id,
                    source_name: "Item".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: ammo_id,
                    source_name: "Ammo".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "BaseClass1".to_owned(),
                        source_type_id: item_family_id,
                        resolved_type: ResolvedType::Named {
                            type_id: item_family_id,
                            source_name: "Item".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: true,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let item_family = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == item_family_id)
            .expect("family Item plan");
        let item_record = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == item_record_id)
            .expect("record Item plan");

        assert_eq!(item_family.rust_name, "Item");
        assert_eq!(item_record.rust_name, "Item");
        assert_eq!(item_family.family_scope_path, vec!["item"]);
        assert_eq!(item_record.scope_path, Vec::<String>::new());
        assert_eq!(item_record.file_stem_override.as_deref(), Some("item"));
    }

    #[test]
    fn selected_plan_does_not_suffix_unemitted_type_name_collisions() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "AZ::Component",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_shared",
                            "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                },
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                    "$id": 30,
                    "name": "Example::Shared",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "elements": [],
                    "attributes": []
                },
                "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD": {
                    "$id": 40,
                    "name": "Other::Shared",
                    "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                    "elements": [],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let serialize_unit = SerializeCodegenPlanner::plan_model(&model);

        let selected = RustCodegenPlanner::standalone().plan_selected_serialize_codegen_unit(
            &serialize_unit,
            SerializeCodegenSelection::RuntimeRoots,
            &crate::CodegenContext::inline(),
        );
        let component = selected
            .items
            .iter()
            .find(|item| item.source_name == "Example::CounterComponent")
            .expect("selected component");
        let selected_names = selected
            .items
            .iter()
            .map(|item| item.rust_name.as_str())
            .collect::<BTreeSet<_>>();

        assert!(selected_names.contains("Shared"));
        assert!(!selected_names.contains("SharedDDDDDDDD"));
        assert_eq!(component.fields[0].rust_type, "Shared");
    }

    #[test]
    fn selected_plan_uses_full_context_for_family_layout() {
        let base = SerializeCodegenItem {
            source_type_id: uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"),
            source_name: "Example::BaseThing".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(true),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let mut derived = base.clone();
        derived.source_type_id = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        derived.source_name = "Example::DerivedThing".to_owned();
        derived.is_abstract = Some(false);
        derived.rtti_base_chain = vec![crate::ir::SerializeCodegenRttiBase {
            type_id: base.source_type_id,
            source_name: base.source_name.clone(),
        }];
        derived.fields = vec![SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id: base.source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: base.source_type_id,
                source_name: base.source_name.clone(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }];
        let emitted_unit = SerializeCodegenUnit {
            items: vec![base.clone()],
        };
        let context_unit = SerializeCodegenUnit {
            items: vec![base, derived],
        };

        let rust_unit = RustCodegenPlanner::standalone().plan_serialize_codegen_units(
            &emitted_unit,
            &context_unit,
            &crate::CodegenContext::inline(),
        );
        let report = RustStandaloneLayoutReport::from_codegen_unit(&rust_unit);

        assert!(report.modules.iter().any(|module| {
            module.path == "src/types/example/base_things/mod.rs"
                && module
                    .items
                    .iter()
                    .any(|item| item.rust_name == "BaseThing")
        }));
        assert!(
            report
                .files
                .iter()
                .all(|file| file.path != "src/types/example/base_thing.rs")
        );
    }

    #[test]
    fn standalone_plan_scopes_field_owned_same_name_records_under_owner_module() {
        let component_id = uuid!("8B8918AB-894D-4E44-B72C-F947844A3985");
        let layer_id = uuid!("72D040D6-14F1-489E-855F-84945BD0C7EA");
        let excluded_layer_id = uuid!("6D2846E7-2741-42B5-A6CC-795BC4D10929");
        let layer_vector_id = uuid!("11111111-1111-1111-1111-111111111111");
        let excluded_vector_id = uuid!("22222222-2222-2222-2222-222222222222");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: component_id,
                    source_name: "MusicManagerInfoComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_layerInfo".to_owned(),
                        source_type_id: layer_vector_id,
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::Vector,
                            element: Box::new(ResolvedType::Named {
                                type_id: layer_id,
                                source_name: "MusicManagerLayerInfo".to_owned(),
                            }),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: layer_id,
                    source_name: "MusicManagerLayerInfo".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: None,
                    factory: Some("NewWorld+0x9f06f78".to_owned()),
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![SerializeCodegenField {
                        source_name: "m_excludedLayerInfo".to_owned(),
                        source_type_id: excluded_vector_id,
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::Vector,
                            element: Box::new(ResolvedType::Named {
                                type_id: excluded_layer_id,
                                source_name: "MusicManagerLayerInfo".to_owned(),
                            }),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: excluded_layer_id,
                    source_name: "MusicManagerLayerInfo".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: None,
                    factory: Some("NewWorld+0x9f06f80".to_owned()),
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let layer = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == layer_id)
            .expect("layer info plan");
        let excluded_layer = rust_unit
            .items
            .iter()
            .find(|item| item.source_type_id == excluded_layer_id)
            .expect("excluded layer info plan");

        assert_eq!(layer.rust_name, "MusicManagerLayerInfo");
        assert_eq!(excluded_layer.rust_name, "MusicManagerLayerInfo");
        assert_eq!(
            layer.scope_path,
            vec![
                "components".to_owned(),
                "music_manager_info_component".to_owned()
            ]
        );
        assert_eq!(
            excluded_layer.scope_path,
            vec![
                "components".to_owned(),
                "music_manager_info_component".to_owned(),
                "music_manager_layer_info".to_owned()
            ]
        );
        assert_eq!(
            excluded_layer.file_stem_override.as_deref(),
            Some("excluded_layer_info")
        );
        assert_eq!(
            layer.fields[0].rust_type,
            "Vec<crate::types::components::music_manager_info_component::music_manager_layer_info::excluded_layer_info::MusicManagerLayerInfo>"
        );
    }

    #[test]
    fn materializes_dataful_base_classes_with_semantic_field_names() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "BaseData",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [{
                        "$id": 11,
                        "name": "m_value",
                        "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                        "is_base_class": false
                    }],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "DerivedData",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [{
                        "$id": 21,
                        "name": "BaseClass1",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "is_base_class": true
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let derived = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("derived data plan");

        assert_eq!(derived.fields.len(), 1);
        assert_eq!(derived.fields[0].rust_name, "base_data");
        assert_eq!(derived.fields[0].rust_type, "BaseData");
        assert!(derived.fields[0].is_base_class);
    }

    #[test]
    fn skips_uuid_collided_abstract_base_edges_as_payload_fields() {
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

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let set_mannequin = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("D490719F-8531-4F82-A64E-EE29DC6AEA50"))
            .expect("SetMannequinTagData plan");

        assert_eq!(set_mannequin.scope_path, vec!["activity_datas"]);
        assert!(set_mannequin.fields.is_empty());
    }

    #[test]
    fn skips_abstract_action_condition_base_for_concrete_condition_types() {
        let document = SerializeContextDocument::from_slice(
            br#"{
                "$id": 1,
                "uuidMap": {
                    "401EA5B5-DDE2-4848-BE17-FD45660FF8C5": {
                        "$id": 10,
                        "name": "ActionCondition",
                        "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                        "version": 1,
                        "factory": "NewWorld+0x9eb4750",
                        "persistentId": null,
                        "doSave": null,
                        "serializer": null,
                        "eventHandler": null,
                        "container": null,
                        "azRtti": {
                            "address": "NewWorld+0x9eb4550",
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
                    "84C87FC2-E60D-4FF3-9003-7E02EC84BBBB": {
                        "$id": 20,
                        "name": "ActionConditionIfInput",
                        "typeId": "84C87FC2-E60D-4FF3-9003-7E02EC84BBBB",
                        "version": 1,
                        "factory": "NewWorld+0x9eb6a88",
                        "persistentId": null,
                        "doSave": null,
                        "serializer": null,
                        "eventHandler": null,
                        "container": null,
                        "azRtti": {
                            "address": "NewWorld+0x9eb6a50",
                            "typeId": "84C87FC2-E60D-4FF3-9003-7E02EC84BBBB",
                            "typeName": "ActionConditionIfInput",
                            "hierarchy": [{
                                "typeId": "84C87FC2-E60D-4FF3-9003-7E02EC84BBBB",
                                "typeName": "ActionConditionIfInput"
                            }, {
                                "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                "typeName": "ActionCondition"
                            }],
                            "isAbstract": false
                        },
                        "dataConverter": null,
                        "editData": null,
                        "elements": [{
                            "$id": 21,
                            "name": "BaseClass1",
                            "nameCrc": 3566360373,
                            "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                            "dataSize": "24",
                            "offset": "0",
                            "attributeOwnership": 0,
                            "flags": 2,
                            "is_pointer": false,
                            "is_base_class": true,
                            "no_default_value": false,
                            "is_dynamic_field": false,
                            "is_ui_element": false,
                            "azRtti": {
                                "address": "NewWorld+0x9eb4550",
                                "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                "typeName": "ActionCondition",
                                "hierarchy": [{
                                    "typeId": "401EA5B5-DDE2-4848-BE17-FD45660FF8C5",
                                    "typeName": "ActionCondition"
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

        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let condition = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("84C87FC2-E60D-4FF3-9003-7E02EC84BBBB"))
            .expect("ActionConditionIfInput plan");

        assert_eq!(condition.scope_path, vec!["action_conditions"]);
        assert!(condition.fields.is_empty());
    }
}
