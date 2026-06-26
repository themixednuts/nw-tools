use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use heck::ToShoutySnakeCase;
use thiserror::Error;
use uuid::Uuid;

use crate::{CodegenContext, ReflectedTypeCatalog, rust_reflected_type_name};

#[derive(Debug, Clone)]
pub struct AzIdentityReconcileReport {
    pub component_name: String,
    pub file_path: PathBuf,
    pub type_id: Uuid,
    pub added_az_rtti_attr: bool,
    pub updated_az_rtti_attr: bool,
    pub added_az_rtti_derive: bool,
    pub added_az_type_info_attr: bool,
    pub updated_az_type_info_attr: bool,
    pub added_az_type_info_derive: bool,
    pub removed_type_id_const: bool,
    pub removed_type_id_const_name: Option<String>,
    pub removed_default_derive: bool,
    pub added_facet_owner_attr: bool,
    pub updated_facet_owner_attr: bool,
    pub facet_owner: Option<String>,
    pub normalized_class_desc_attr: bool,
    pub normalized_derive_attr: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AzIdentityEvidence {
    pub type_ids_by_name: BTreeMap<String, Uuid>,
    pub support_type_ids_by_name: BTreeMap<String, Uuid>,
    pub facet_owners_by_name: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SkippedAzIdentityReconcileReport {
    pub component_name: String,
    pub file_path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum SourceReconcileError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to walk {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub fn reconcile_az_type_identities(
    components_root: &Path,
    apply: bool,
    evidence: &AzIdentityEvidence,
    context: &CodegenContext,
) -> Result<
    (
        Vec<AzIdentityReconcileReport>,
        Vec<SkippedAzIdentityReconcileReport>,
    ),
    SourceReconcileError,
> {
    let mut fixes = Vec::new();
    let mut skipped = Vec::new();
    let mut files = Vec::new();
    let mut removed_type_id_consts = BTreeSet::new();
    collect_rust_files(components_root, &mut files)?;
    files.sort();

    let file_outcomes = context.runner().try_map(&files, |file_path| {
        plan_or_apply_az_identity_reconciliation_for_file(file_path, apply, evidence)
    })?;

    for outcome in file_outcomes {
        skipped.extend(outcome.skipped);
        for fix in outcome.fixes {
            if fix.remove_type_id_const
                && let Some(type_id_const) = &fix.type_id_const
            {
                removed_type_id_consts.insert(type_id_const.clone());
            }
            fixes.push(AzIdentityReconcileReport {
                component_name: fix.component_name,
                file_path: fix.file_path,
                type_id: fix.type_id,
                added_az_rtti_attr: fix.add_az_rtti_attr,
                updated_az_rtti_attr: fix.update_az_rtti_attr,
                added_az_rtti_derive: fix.add_az_rtti_derive,
                added_az_type_info_attr: fix.add_az_type_info_attr,
                updated_az_type_info_attr: fix.update_az_type_info_attr,
                added_az_type_info_derive: fix.add_az_type_info_derive,
                removed_type_id_const: fix.remove_type_id_const,
                removed_type_id_const_name: fix
                    .remove_type_id_const
                    .then(|| fix.type_id_const.clone())
                    .flatten(),
                removed_default_derive: fix.remove_default_derive,
                added_facet_owner_attr: fix.add_facet_owner_attr,
                updated_facet_owner_attr: fix.update_facet_owner_attr,
                facet_owner: fix.facet_owner,
                normalized_class_desc_attr: fix.normalize_class_desc_attr,
                normalized_derive_attr: fix.normalize_derive_tokens,
            });
        }
    }

    if apply && !removed_type_id_consts.is_empty() {
        for mod_path in collect_module_files(components_root)? {
            let mut text = read_to_string(&mod_path)?;
            cleanup_component_module_reexports(&mut text, &removed_type_id_consts);
            write_string(&mod_path, &text)?;
        }
    }

    Ok((fixes, skipped))
}

#[must_use]
pub fn az_identity_evidence_from_catalog(catalog: &ReflectedTypeCatalog) -> AzIdentityEvidence {
    let mut type_ids_by_name = catalog.component_type_ids_by_name();
    let mut support_type_ids_by_name = BTreeMap::new();

    for ty in catalog.reflected_types() {
        let rust_name = rust_reflected_type_name(&ty.name, ty.type_id);
        if ty.role.is_az_component_like() {
            type_ids_by_name.entry(rust_name).or_insert(ty.type_id);
            type_ids_by_name
                .entry(ty.name.clone())
                .or_insert(ty.type_id);
        } else if ty.is_support_type() && !ty.is_reflection_marker {
            support_type_ids_by_name
                .entry(rust_name)
                .or_insert(ty.type_id);
            support_type_ids_by_name
                .entry(ty.name.clone())
                .or_insert(ty.type_id);
        }
    }

    AzIdentityEvidence {
        type_ids_by_name,
        support_type_ids_by_name,
        facet_owners_by_name: BTreeMap::new(),
    }
}

#[derive(Debug)]
struct AzIdentityFileOutcome {
    fixes: Vec<AzIdentityReconcilePlan>,
    skipped: Vec<SkippedAzIdentityReconcileReport>,
}

fn plan_or_apply_az_identity_reconciliation_for_file(
    file_path: &Path,
    apply: bool,
    evidence: &AzIdentityEvidence,
) -> Result<AzIdentityFileOutcome, SourceReconcileError> {
    let text = read_to_string(file_path)?;
    let mut file_fixes =
        plan_az_identity_reconciliation_for_file_with_evidence(&text, file_path, evidence);
    let mut skipped = Vec::new();
    for fix in &file_fixes {
        if fix.type_id.is_nil() {
            skipped.push(SkippedAzIdentityReconcileReport {
                    component_name: fix.component_name.clone(),
                    file_path: fix.file_path.clone(),
                    reason: "component struct exists but no TYPE_ID const, az_rtti attribute, or evidence UUID was found"
                        .to_owned(),
                });
        }
    }
    file_fixes.retain(|fix| !fix.type_id.is_nil());

    if apply && !file_fixes.is_empty() {
        let mut updated = text;
        apply_az_identity_reconciliation(&mut updated, &file_fixes);
        write_string(file_path, &updated)?;
    }

    Ok(AzIdentityFileOutcome {
        fixes: file_fixes,
        skipped,
    })
}

impl fmt::Display for AzIdentityReconcileReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.added_az_rtti_attr {
            parts.push("attr");
        }
        if self.updated_az_rtti_attr {
            parts.push("attr-update");
        }
        if self.added_az_rtti_derive {
            parts.push("derive");
        }
        if self.added_az_type_info_attr {
            parts.push("type-info-attr");
        }
        if self.updated_az_type_info_attr {
            parts.push("type-info-attr-update");
        }
        if self.added_az_type_info_derive {
            parts.push("type-info-derive");
        }
        if self.removed_type_id_const {
            parts.push("remove-const");
        }
        if self.removed_default_derive {
            parts.push("remove-default-derive");
        }
        if self.added_facet_owner_attr {
            parts.push("facet-owner");
        }
        if self.updated_facet_owner_attr {
            parts.push("facet-owner-update");
        }
        if self.normalized_class_desc_attr {
            parts.push("class-desc");
        }
        if self.normalized_derive_attr {
            parts.push("derive-cleanup");
        }
        write!(
            f,
            "  reconcile {}::{} ({}): {}",
            self.file_path.display(),
            self.component_name,
            self.type_id,
            parts.join("+")
        )
    }
}

#[derive(Debug, Clone)]
pub struct ScannedComponent {
    pub component_name: String,
    item_kind: RustItemKind,
    pub native_name: Option<String>,
    pub file_path: PathBuf,
    pub type_id: Option<Uuid>,
    pub type_id_const: Option<String>,
    pub inline_az_rtti_type_id: Option<Uuid>,
    pub inline_az_type_info_type_id: Option<Uuid>,
    pub inline_class_desc_type_id: Option<Uuid>,
    pub class_desc_name: Option<String>,
    pub has_az_rtti_attr: bool,
    pub has_az_rtti_type_id: bool,
    pub has_inline_az_rtti_type_id: bool,
    pub has_az_rtti_derive: bool,
    pub has_az_type_info_attr: bool,
    pub has_az_type_info_type_id: bool,
    pub has_inline_az_type_info_type_id: bool,
    pub has_az_type_info_derive: bool,
    pub has_bevy_component_derive: bool,
    pub has_default_derive: bool,
    pub has_manual_default_impl: bool,
    pub has_facet_derive: bool,
    pub has_class_desc_derive: bool,
    pub has_duplicate_derive_tokens: bool,
    pub has_class_desc_attr: bool,
    pub class_desc_needs_normalization: bool,
    pub has_facet_owner_attr: bool,
    pub facet_owner: Option<String>,
    pub field_names: BTreeSet<String>,
    pub field_names_in_order: Vec<String>,
    pub insert_offset: Option<usize>,
    pub unit_struct_semicolon: Option<usize>,
    pub attr_block_start: usize,
    pub struct_start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustItemKind {
    Struct,
    Enum,
}

impl RustItemKind {
    fn keyword(self) -> &'static str {
        match self {
            RustItemKind::Struct => "pub struct ",
            RustItemKind::Enum => "pub enum ",
        }
    }
}

pub fn scan_component_structs(text: &str, file_path: &Path) -> Vec<ScannedComponent> {
    let consts = parse_type_id_consts(text);
    let mut results = Vec::new();
    let mut search_from = 0usize;

    while let Some((struct_start, item_kind)) = find_next_public_type_item(text, search_from) {
        let name_start = struct_start + item_kind.keyword().len();
        let Some((component_name, name_end)) = parse_ident(text, name_start) else {
            search_from = name_start;
            continue;
        };

        let attr_block_start = find_attr_block_start(text, struct_start);
        let attr_block = &text[attr_block_start..struct_start];
        let has_bevy_component_derive = has_derive_token(attr_block, "Component");
        let has_facet_derive = has_derive_token(attr_block, "Facet");
        let attr_type_id_const = parse_az_rtti_type_id_const_from_attr(attr_block);
        let attr_az_type_info_type_id_const =
            parse_az_type_info_type_id_const_from_attr(attr_block);
        let class_desc_name = parse_class_desc_name_from_attr(attr_block);
        let native_name =
            parse_identity_name_from_attr(attr_block).or_else(|| class_desc_name.clone());
        let type_id_const = find_component_type_id_const(
            &component_name,
            attr_type_id_const
                .as_deref()
                .or(attr_az_type_info_type_id_const.as_deref()),
            &consts,
        );

        let has_az_rtti_attr = find_attr_invocation(attr_block, &["az_rtti", "rtti"]).is_some();
        let has_az_type_info_attr = find_attr_invocation(
            attr_block,
            &["az_type_info", "type_info", "az_rtti", "rtti"],
        )
        .is_some();
        let has_az_rtti_derive = has_derive_token(attr_block, "AzRtti");
        let has_az_type_info_derive =
            has_derive_token(attr_block, "AzTypeInfo") || has_derive_token(attr_block, "AzRtti");
        let has_class_desc_derive = has_derive_token(attr_block, "ClassDesc");
        let has_duplicate_derive_tokens = derive_block_has_duplicate_tokens(attr_block);
        let (field_names, field_names_in_order, insert_offset, unit_struct_semicolon) =
            parse_struct_body(text, name_end);
        let const_type_id = type_id_const
            .as_ref()
            .and_then(|const_name| consts.get(const_name).copied());
        let inline_az_rtti_type_id = parse_az_rtti_type_id_from_attr(attr_block);
        let inline_az_type_info_type_id = parse_az_type_info_type_id_from_attr(attr_block);
        let inline_class_desc_type_id = parse_class_desc_type_id_from_attr(attr_block);
        let type_id = const_type_id
            .or(inline_az_rtti_type_id)
            .or(inline_az_type_info_type_id)
            .or(inline_class_desc_type_id);
        let has_az_rtti_type_id = attr_invocation_has_type_id_arg(attr_block, &["az_rtti", "rtti"]);
        let has_inline_az_rtti_type_id = inline_az_rtti_type_id.is_some();
        let has_az_type_info_type_id = attr_invocation_has_type_id_arg(
            attr_block,
            &["az_type_info", "type_info", "az_rtti", "rtti"],
        );
        let has_inline_az_type_info_type_id = inline_az_type_info_type_id.is_some();
        let has_class_desc_attr = find_attr_invocation(attr_block, &["class_desc"]).is_some();
        let class_desc_needs_normalization =
            attr_invocation_contains_key(attr_block, &["class_desc"], "uuid")
                || attr_invocation_contains_key(attr_block, &["class_desc"], "name");
        let has_default_derive = has_derive_token(attr_block, "Default");
        let has_manual_default_impl = has_manual_default_impl(text, &component_name);
        let facet_owner = parse_facet_owner_from_attr(attr_block);
        let has_facet_owner_attr = facet_owner.is_some();

        results.push(ScannedComponent {
            component_name,
            item_kind,
            native_name,
            file_path: file_path.to_path_buf(),
            type_id,
            type_id_const,
            inline_az_rtti_type_id,
            inline_az_type_info_type_id,
            inline_class_desc_type_id,
            class_desc_name,
            has_az_rtti_attr,
            has_az_rtti_type_id,
            has_inline_az_rtti_type_id,
            has_az_rtti_derive,
            has_az_type_info_attr,
            has_az_type_info_type_id,
            has_inline_az_type_info_type_id,
            has_az_type_info_derive,
            has_bevy_component_derive,
            has_default_derive,
            has_manual_default_impl,
            has_facet_derive,
            has_class_desc_derive,
            has_duplicate_derive_tokens,
            has_class_desc_attr,
            class_desc_needs_normalization,
            has_facet_owner_attr,
            facet_owner,
            field_names,
            field_names_in_order,
            insert_offset,
            unit_struct_semicolon,
            attr_block_start,
            struct_start,
        });
        search_from = name_end;
    }

    results
}

fn find_next_public_type_item(text: &str, search_from: usize) -> Option<(usize, RustItemKind)> {
    let struct_start = text[search_from..]
        .find(RustItemKind::Struct.keyword())
        .map(|relative| (search_from + relative, RustItemKind::Struct));
    let enum_start = text[search_from..]
        .find(RustItemKind::Enum.keyword())
        .map(|relative| (search_from + relative, RustItemKind::Enum));

    match (struct_start, enum_start) {
        (Some(struct_start), Some(enum_start)) => {
            if struct_start.0 <= enum_start.0 {
                Some(struct_start)
            } else {
                Some(enum_start)
            }
        }
        (Some(struct_start), None) => Some(struct_start),
        (None, Some(enum_start)) => Some(enum_start),
        (None, None) => None,
    }
}

#[derive(Debug, Clone)]
struct AzIdentityReconcilePlan {
    component_name: String,
    file_path: PathBuf,
    type_id: Uuid,
    type_id_const: Option<String>,
    type_id_expr: String,
    type_info_name: Option<String>,
    add_az_rtti_attr: bool,
    update_az_rtti_attr: bool,
    add_az_rtti_derive: bool,
    add_az_type_info_attr: bool,
    update_az_type_info_attr: bool,
    add_az_type_info_derive: bool,
    add_default_derive: bool,
    add_bevy_component_derive: bool,
    remove_type_id_const: bool,
    remove_default_derive: bool,
    facet_owner: Option<String>,
    add_facet_owner_attr: bool,
    update_facet_owner_attr: bool,
    normalize_class_desc_attr: bool,
    normalize_derive_tokens: bool,
    struct_start: usize,
}

#[cfg(test)]
fn plan_az_identity_reconciliation_for_file(
    text: &str,
    file_path: &Path,
    evidence_by_name: &BTreeMap<String, Uuid>,
) -> Vec<AzIdentityReconcilePlan> {
    let evidence = AzIdentityEvidence {
        type_ids_by_name: evidence_by_name.clone(),
        support_type_ids_by_name: BTreeMap::new(),
        facet_owners_by_name: BTreeMap::new(),
    };
    plan_az_identity_reconciliation_for_file_with_evidence(text, file_path, &evidence)
}

fn plan_az_identity_reconciliation_for_file_with_evidence(
    text: &str,
    file_path: &Path,
    evidence: &AzIdentityEvidence,
) -> Vec<AzIdentityReconcilePlan> {
    let mut plans = Vec::new();

    for scanned in scan_component_structs(text, file_path) {
        let component_evidence_type_id = evidence_type_id_for(&scanned, &evidence.type_ids_by_name);
        let support_evidence_type_id =
            evidence_type_id_for(&scanned, &evidence.support_type_ids_by_name);
        let component_like =
            is_component_like_struct(&scanned, component_evidence_type_id.is_some());
        let existing_type_info_type_id = (scanned.has_az_type_info_attr
            || scanned.has_az_type_info_derive)
            .then_some(scanned.type_id)
            .flatten();
        let existing_wire_class_desc_type_id = scanned
            .has_class_desc_derive
            .then_some(existing_type_info_type_id)
            .flatten();
        let type_id = if component_like {
            component_evidence_type_id.or(scanned.type_id)
        } else if let Some(type_id) = existing_wire_class_desc_type_id {
            Some(type_id)
        } else {
            support_evidence_type_id
                .or(scanned.inline_class_desc_type_id)
                .or(existing_type_info_type_id)
        };
        let facet_owner = evidence
            .facet_owners_by_name
            .get(&scanned.component_name)
            .cloned();

        let Some(type_id) = type_id else {
            if scanned.has_facet_derive {
                continue;
            }
            if !component_like {
                continue;
            }
            plans.push(AzIdentityReconcilePlan {
                component_name: scanned.component_name,
                file_path: scanned.file_path,
                type_id: Uuid::nil(),
                type_id_const: scanned.type_id_const,
                type_id_expr: String::new(),
                type_info_name: None,
                add_az_rtti_attr: false,
                update_az_rtti_attr: false,
                add_az_rtti_derive: false,
                add_az_type_info_attr: false,
                update_az_type_info_attr: false,
                add_az_type_info_derive: false,
                add_default_derive: false,
                add_bevy_component_derive: false,
                remove_type_id_const: false,
                remove_default_derive: false,
                facet_owner: None,
                add_facet_owner_attr: false,
                update_facet_owner_attr: false,
                normalize_class_desc_attr: false,
                normalize_derive_tokens: false,
                struct_start: scanned.struct_start,
            });
            continue;
        };

        let type_id_expr = identity_type_id_expr(type_id);
        let type_info_name = (!component_like)
            .then(|| scanned.native_name.clone())
            .flatten()
            .filter(|name| name != &scanned.component_name);
        let add_az_rtti_attr = component_like && !scanned.has_az_rtti_attr;
        let update_az_rtti_attr = component_like
            && scanned.has_az_rtti_attr
            && (!scanned.has_az_rtti_type_id
                || !scanned.has_inline_az_rtti_type_id
                || scanned.inline_az_rtti_type_id != Some(type_id));
        let add_az_type_info_attr = !component_like && !scanned.has_az_type_info_attr;
        let update_az_type_info_attr = !component_like
            && scanned.has_az_type_info_attr
            && (!scanned.has_az_type_info_type_id
                || !scanned.has_inline_az_type_info_type_id
                || scanned.inline_az_type_info_type_id != Some(type_id));
        let add_default_derive =
            component_like && !scanned.has_default_derive && !scanned.has_manual_default_impl;
        let add_az_rtti_derive = component_like
            && (!scanned.has_az_rtti_derive
                || !scanned.has_bevy_component_derive
                || add_default_derive);
        let add_az_type_info_derive = !component_like && !scanned.has_az_type_info_derive;
        let add_bevy_component_derive = add_az_rtti_derive && !scanned.has_bevy_component_derive;
        let remove_type_id_const = scanned.type_id_const.is_some();
        let remove_default_derive =
            component_like && scanned.has_default_derive && scanned.has_manual_default_impl;
        let add_facet_owner_attr =
            scanned.has_facet_derive && facet_owner.is_some() && !scanned.has_facet_owner_attr;
        let update_facet_owner_attr = scanned.has_facet_derive
            && facet_owner.is_some()
            && scanned.facet_owner.as_deref() != facet_owner.as_deref()
            && !add_facet_owner_attr;
        let normalize_class_desc_attr =
            scanned.has_class_desc_attr && scanned.class_desc_needs_normalization;
        let normalize_derive_tokens = scanned.has_duplicate_derive_tokens;

        if !add_az_rtti_attr
            && !update_az_rtti_attr
            && !add_az_rtti_derive
            && !add_az_type_info_attr
            && !update_az_type_info_attr
            && !add_az_type_info_derive
            && !remove_type_id_const
            && !remove_default_derive
            && !add_facet_owner_attr
            && !update_facet_owner_attr
            && !normalize_class_desc_attr
            && !normalize_derive_tokens
        {
            continue;
        }

        plans.push(AzIdentityReconcilePlan {
            component_name: scanned.component_name,
            file_path: scanned.file_path,
            type_id,
            type_id_const: scanned.type_id_const,
            type_id_expr,
            type_info_name,
            add_az_rtti_attr,
            update_az_rtti_attr,
            add_az_rtti_derive,
            add_az_type_info_attr,
            update_az_type_info_attr,
            add_az_type_info_derive,
            add_default_derive,
            add_bevy_component_derive,
            remove_type_id_const,
            remove_default_derive,
            facet_owner,
            add_facet_owner_attr,
            update_facet_owner_attr,
            normalize_class_desc_attr,
            normalize_derive_tokens,
            struct_start: scanned.struct_start,
        });
    }

    plans
}

fn apply_az_identity_reconciliation(text: &mut String, fixes: &[AzIdentityReconcilePlan]) {
    let mut ordered = fixes.to_vec();
    ordered.sort_by_key(|fix| std::cmp::Reverse(fix.struct_start));
    let needs_az_rtti_use = ordered
        .iter()
        .any(|fix| fix.add_az_rtti_attr || fix.update_az_rtti_attr || fix.add_az_rtti_derive);
    let needs_az_type_info_use = ordered.iter().any(|fix| {
        fix.add_az_type_info_attr || fix.update_az_type_info_attr || fix.add_az_type_info_derive
    });
    let needs_bevy_component_use = ordered.iter().any(|fix| fix.add_bevy_component_derive);

    for fix in ordered {
        let Some(struct_start) = find_component_item_start(text, &fix.component_name) else {
            continue;
        };
        let attr_block_start = find_attr_block_start(text, struct_start);

        if fix.normalize_derive_tokens {
            normalize_derive_tokens_in_attr_block(text, attr_block_start, struct_start);
        }

        if fix.add_az_rtti_derive {
            add_az_rtti_to_attr_block(text, attr_block_start, struct_start, fix.add_default_derive);
        }
        if fix.add_az_type_info_derive {
            add_az_type_info_to_attr_block(text, attr_block_start, struct_start);
        }

        let Some(struct_start) = find_component_item_start(text, &fix.component_name) else {
            continue;
        };
        let attr_block_start = find_attr_block_start(text, struct_start);
        if fix.update_az_rtti_attr {
            update_az_rtti_attr_line(text, attr_block_start, struct_start, &fix.type_id_expr);
        } else if fix.add_az_rtti_attr {
            text.insert_str(struct_start, &az_rtti_attr_line(&fix.type_id_expr));
        }
        if fix.update_az_type_info_attr {
            update_az_type_info_attr_line(
                text,
                attr_block_start,
                struct_start,
                &fix.type_id_expr,
                fix.type_info_name.as_deref(),
            );
        } else if fix.add_az_type_info_attr {
            text.insert_str(
                struct_start,
                &az_type_info_attr_line(&fix.type_id_expr, fix.type_info_name.as_deref()),
            );
        }

        if let Some(facet_owner) = &fix.facet_owner {
            let Some(struct_start) = find_component_item_start(text, &fix.component_name) else {
                continue;
            };
            let attr_block_start = find_attr_block_start(text, struct_start);
            if fix.update_facet_owner_attr {
                update_facet_owner_attr_line(text, attr_block_start, struct_start, facet_owner);
            } else if fix.add_facet_owner_attr {
                text.insert_str(struct_start, &facet_owner_attr_line(facet_owner));
            }
        }

        if fix.normalize_class_desc_attr {
            let Some(struct_start) = find_component_item_start(text, &fix.component_name) else {
                continue;
            };
            let attr_block_start = find_attr_block_start(text, struct_start);
            normalize_class_desc_attr_line(text, attr_block_start, struct_start);
        }

        if fix.remove_default_derive {
            let Some(struct_start) = find_component_item_start(text, &fix.component_name) else {
                continue;
            };
            let attr_block_start = find_attr_block_start(text, struct_start);
            remove_default_from_attr_block(text, attr_block_start, struct_start);
        }

        if fix.remove_type_id_const
            && let Some(type_id_const) = &fix.type_id_const
        {
            remove_type_id_const_declaration(text, type_id_const);
            replace_remaining_type_id_const_references(text, type_id_const, &fix.component_name);
        }
    }

    cleanup_unused_uuid_imports(text);

    if needs_az_rtti_use {
        ensure_module_use(text, "use az_derive::AzRtti;");
    }
    if needs_az_type_info_use {
        ensure_module_use(text, "use az_derive::AzTypeInfo;");
    }
    if needs_bevy_component_use {
        ensure_bevy_component_use(text);
    }
}

fn cleanup_component_module_reexports(
    text: &mut String,
    removed_type_id_consts: &BTreeSet<String>,
) {
    if removed_type_id_consts.is_empty() {
        return;
    }

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut changed = false;
    while let Some(relative_start) = text[cursor..].find("pub use ") {
        let start = cursor + relative_start;
        let Some(relative_end) = text[start..].find(';') else {
            break;
        };
        let end = start + relative_end + 1;
        let statement = &text[start..end];
        let Some(open) = statement.find('{') else {
            out.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        };
        let Some(close) = statement.rfind('}') else {
            out.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        };

        let body = &statement[open + 1..close];
        if !removed_type_id_consts
            .iter()
            .any(|const_name| body.contains(const_name))
        {
            out.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }

        out.push_str(&text[cursor..start]);
        let prefix = &statement[..open + 1];
        let suffix = &statement[close..];
        let retained = body
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .filter(|item| !removed_type_id_consts.contains(*item))
            .collect::<Vec<_>>();

        if !retained.is_empty() {
            out.push_str(prefix);
            out.push('\n');
            out.push_str("    ");
            out.push_str(&retained.join(", "));
            out.push(',');
            out.push('\n');
            out.push_str(suffix);
        }
        changed = true;
        cursor = end;
    }

    if !changed {
        return;
    }

    out.push_str(&text[cursor..]);
    *text = out;
}

fn find_component_item_start(text: &str, component_name: &str) -> Option<usize> {
    [RustItemKind::Struct, RustItemKind::Enum]
        .into_iter()
        .filter_map(|item_kind| find_named_public_item_start(text, component_name, item_kind))
        .min()
}

fn find_named_public_item_start(
    text: &str,
    component_name: &str,
    item_kind: RustItemKind,
) -> Option<usize> {
    let needle = format!("{}{component_name}", item_kind.keyword());
    let mut search_from = 0usize;
    while let Some(relative) = text[search_from..].find(&needle) {
        let start = search_from + relative;
        let after = text[start + needle.len()..].chars().next();
        if after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_') {
            return Some(start);
        }
        search_from = start + needle.len();
    }
    None
}

fn find_component_type_id_const(
    component_name: &str,
    attr_type_id_const: Option<&str>,
    consts: &BTreeMap<String, Uuid>,
) -> Option<String> {
    let candidate_type_id_const = type_id_const_name(component_name);
    if consts.contains_key(&candidate_type_id_const) {
        return Some(candidate_type_id_const);
    }

    if let Some(attr_type_id_const) = attr_type_id_const
        && consts.contains_key(attr_type_id_const)
    {
        return Some(attr_type_id_const.to_owned());
    }

    let normalized_target = normalized_const_name(&candidate_type_id_const);
    consts
        .keys()
        .find(|const_name| normalized_const_name(const_name) == normalized_target)
        .cloned()
}

fn normalized_const_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn az_rtti_attr_line(type_id_expr: &str) -> String {
    format!("#[az_rtti({type_id_expr})]\n")
}

fn az_type_info_attr_line(type_id_expr: &str, name: Option<&str>) -> String {
    if let Some(name) = name {
        format!("#[az_type_info(name = \"{name}\", {type_id_expr})]\n")
    } else {
        format!("#[az_type_info({type_id_expr})]\n")
    }
}

fn identity_type_id_expr(type_id: Uuid) -> String {
    format!("\"{}\"", type_id.to_string().to_ascii_uppercase())
}

fn update_az_rtti_attr_line(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    type_id_expr: &str,
) {
    let block = &text[attr_block_start..struct_start];
    let lines: Vec<(usize, String)> = block
        .lines()
        .scan(attr_block_start, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line.to_string()))
        })
        .collect();

    for (line_start, line) in lines {
        if !line.contains("#[az_rtti") {
            continue;
        }
        let line_end = line_start + line.len();
        text.replace_range(line_start..line_end, &format!("#[az_rtti({type_id_expr})]"));
        return;
    }
}

fn update_az_type_info_attr_line(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    type_id_expr: &str,
    name: Option<&str>,
) {
    let block = &text[attr_block_start..struct_start];
    let lines: Vec<(usize, String)> = block
        .lines()
        .scan(attr_block_start, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line.to_string()))
        })
        .collect();

    for (line_start, line) in lines {
        if !line.contains("#[az_type_info") && !line.contains("#[type_info") {
            continue;
        }
        let line_end = line_start + line.len();
        let replacement = az_type_info_attr_line(type_id_expr, name)
            .trim_end_matches('\n')
            .to_owned();
        text.replace_range(line_start..line_end, &replacement);
        return;
    }
}

fn normalize_class_desc_attr_line(text: &mut String, attr_block_start: usize, struct_start: usize) {
    let block = &text[attr_block_start..struct_start];
    let lines: Vec<(usize, String)> = block
        .lines()
        .scan(attr_block_start, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line.to_string()))
        })
        .collect();

    for (line_start, line) in lines {
        if !line.contains("#[class_desc") {
            continue;
        }
        let Some(replacement) = class_desc_attr_without_type_info(&line) else {
            continue;
        };
        let mut line_end = line_start + line.len();
        if replacement.is_empty() {
            if text.as_bytes().get(line_end) == Some(&b'\r') {
                line_end += 1;
            }
            if text.as_bytes().get(line_end) == Some(&b'\n') {
                line_end += 1;
            }
        }
        text.replace_range(line_start..line_end, &replacement);
        return;
    }
}

fn class_desc_attr_without_type_info(line: &str) -> Option<String> {
    let open = line.find("#[class_desc(")?;
    let inner_start = open + "#[class_desc(".len();
    let close = line[inner_start..].rfind(')')? + inner_start;
    let inner = &line[inner_start..close];

    let parts = inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter(|part| !is_attr_key(part, "uuid") && !is_attr_key(part, "name"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if parts.is_empty() {
        Some(String::new())
    } else {
        Some(format!(
            "{}{}{}",
            &line[..inner_start],
            parts.join(", "),
            &line[close..]
        ))
    }
}

fn facet_owner_attr_line(owner: &str) -> String {
    format!("#[facet(owner = {owner})]\n")
}

fn update_facet_owner_attr_line(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    owner: &str,
) {
    let block = &text[attr_block_start..struct_start];
    let lines: Vec<(usize, String)> = block
        .lines()
        .scan(attr_block_start, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line.to_string()))
        })
        .collect();

    for (line_start, line) in lines {
        if !line.contains("#[facet") {
            continue;
        }
        let Some(updated) = set_facet_owner_in_attr_line(&line, owner) else {
            continue;
        };
        let line_end = line_start + line.len();
        text.replace_range(line_start..line_end, &updated);
        return;
    }
}

fn set_facet_owner_in_attr_line(line: &str, owner: &str) -> Option<String> {
    let open = line.find("#[facet(")?;
    let inner_start = open + "#[facet(".len();
    let close = line[inner_start..].rfind(')')? + inner_start;
    let inner = &line[inner_start..close];

    let mut parts = inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter(|part| !is_attr_key(part, "owner"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    parts.insert(0, format!("owner = {owner}"));

    Some(format!(
        "{}{}{}",
        &line[..inner_start],
        parts.join(", "),
        &line[close..]
    ))
}

fn remove_type_id_const_declaration(text: &mut String, type_id_const: &str) {
    let needle = format!("pub const {type_id_const}");
    let Some(start) = text.find(&needle) else {
        return;
    };
    let Some(relative_semicolon) = text[start..].find(';') else {
        return;
    };
    let mut end = start + relative_semicolon + 1;
    if text.as_bytes().get(end) == Some(&b'\r') {
        end += 1;
    }
    if text.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    text.replace_range(start..end, "");
}

fn replace_remaining_type_id_const_references(
    text: &mut String,
    type_id_const: &str,
    component_name: &str,
) {
    if !text.contains(type_id_const) {
        return;
    }

    let replacement = format!("<{component_name} as az_core::type_info::AzTypeInfo>::TYPE_ID");
    *text = text.replace(type_id_const, &replacement);
}

fn has_manual_default_impl(text: &str, component_name: &str) -> bool {
    text.contains(&format!("impl Default for {component_name}"))
}

fn parse_az_rtti_type_id_from_attr(attr_block: &str) -> Option<Uuid> {
    let attr = find_attr_invocation(attr_block, &["az_rtti", "rtti"])?;
    parse_identity_attr_uuid(&attr)
}

fn parse_az_type_info_type_id_from_attr(attr_block: &str) -> Option<Uuid> {
    let attr = find_attr_invocation(
        attr_block,
        &["az_type_info", "type_info", "az_rtti", "rtti"],
    )?;
    parse_identity_attr_uuid(&attr)
}

fn parse_class_desc_type_id_from_attr(attr_block: &str) -> Option<Uuid> {
    let attr = find_attr_invocation(attr_block, &["class_desc"])?;
    if !attr.contains("uuid") {
        return None;
    }
    parse_quoted_uuid_after_key(&attr, "uuid")
}

fn parse_az_rtti_type_id_const_from_attr(attr_block: &str) -> Option<String> {
    let attr = find_attr_invocation(attr_block, &["az_rtti", "rtti"])?;
    parse_identity_attr_symbol(&attr)
}

fn parse_az_type_info_type_id_const_from_attr(attr_block: &str) -> Option<String> {
    let attr = find_attr_invocation(
        attr_block,
        &["az_type_info", "type_info", "az_rtti", "rtti"],
    )?;
    parse_identity_attr_symbol(&attr)
}

fn parse_identity_name_from_attr(attr_block: &str) -> Option<String> {
    let attr = find_attr_invocation(
        attr_block,
        &["az_type_info", "type_info", "az_rtti", "rtti"],
    )?;
    if !attr.contains("name") {
        return None;
    }

    let name = attr.find("name")?;
    let after = &attr[name..];
    let quote = after.find('"')? + name;
    let name_start = quote + 1;
    let name_end = attr[name_start..].find('"')? + name_start;
    Some(attr[name_start..name_end].to_owned())
}

fn parse_class_desc_name_from_attr(attr_block: &str) -> Option<String> {
    let attr = find_attr_invocation(attr_block, &["class_desc"])?;
    if !attr.contains("name") {
        return None;
    }
    parse_quoted_string_after_key(&attr, "name")
}

fn parse_facet_owner_from_attr(attr_block: &str) -> Option<String> {
    let attr = find_attr_invocation(attr_block, &["facet"])?;
    if !attr.contains("owner") {
        return None;
    }
    parse_symbol_after_key(&attr, "owner")
}

fn find_attr_invocation(attr_block: &str, names: &[&str]) -> Option<String> {
    let mut collecting = false;
    let mut collected = String::new();

    for line in attr_block.lines() {
        let trimmed = line.trim();
        if !collecting && !names.iter().any(|name| line_starts_attr(trimmed, name)) {
            continue;
        }

        collecting = true;
        if !collected.is_empty() {
            collected.push('\n');
        }
        collected.push_str(line);

        if trimmed.ends_with(']') {
            return Some(collected);
        }
    }

    collecting.then_some(collected)
}

fn attr_invocation_contains_key(attr_block: &str, names: &[&str], key: &str) -> bool {
    find_attr_invocation(attr_block, names).is_some_and(|attr| attr.contains(key))
}

fn attr_invocation_has_type_id_arg(attr_block: &str, names: &[&str]) -> bool {
    find_attr_invocation(attr_block, names).is_some_and(|attr| identity_attr_has_type_id_arg(&attr))
}

fn line_starts_attr(line: &str, name: &str) -> bool {
    let Some(rest) = line.strip_prefix("#[") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(name) else {
        return false;
    };
    rest.starts_with('(') || rest.starts_with(']')
}

fn parse_identity_attr_uuid(line: &str) -> Option<Uuid> {
    parse_positional_quoted_uuid(line)
}

fn parse_identity_attr_symbol(line: &str) -> Option<String> {
    parse_positional_symbol(line)
}

fn identity_attr_has_type_id_arg(line: &str) -> bool {
    first_type_id_attr_arg(line).is_some()
}

fn parse_quoted_uuid_after_key(line: &str, key: &str) -> Option<Uuid> {
    let key_start = attr_key_start(line, key)?;
    let value_start = key_start + key.len();
    let after = &line[value_start..];
    let equals = after.find('=')? + value_start;
    let expr_start = trim_start_offset(line, equals + 1);
    let quote = line[expr_start..].find('"')? + expr_start;
    let uuid_start = quote + 1;
    let uuid_end = line[uuid_start..].find('"')? + uuid_start;
    Uuid::parse_str(line[uuid_start..uuid_end].trim_matches(['{', '}'])).ok()
}

fn parse_quoted_string_after_key(line: &str, key: &str) -> Option<String> {
    let key_start = attr_key_start(line, key)?;
    let value_start = key_start + key.len();
    let after = &line[value_start..];
    let equals = after.find('=')? + value_start;
    let expr_start = trim_start_offset(line, equals + 1);
    let quote = line[expr_start..].find('"')? + expr_start;
    let value_start = quote + 1;
    let value_end = line[value_start..].find('"')? + value_start;
    Some(line[value_start..value_end].to_owned())
}

fn parse_symbol_after_key(line: &str, key: &str) -> Option<String> {
    let key_start = attr_key_start(line, key)?;
    let value_start = key_start + key.len();
    let after_key = &line[value_start..];
    let equals = after_key.find('=')?;
    let expr = after_key[equals + 1..].trim_start();
    parse_symbol_expr(expr)
}

fn parse_positional_quoted_uuid(line: &str) -> Option<Uuid> {
    let arg = first_type_id_attr_arg(line)?.trim();
    if !arg.starts_with('"') && !arg.contains("uuid!") {
        return None;
    }
    let uuid_start = arg.find('"')? + 1;
    let uuid_end = arg[uuid_start..].find('"')? + uuid_start;
    Uuid::parse_str(arg[uuid_start..uuid_end].trim_matches(['{', '}'])).ok()
}

fn parse_positional_symbol(line: &str) -> Option<String> {
    let arg = first_type_id_attr_arg(line)?.trim();
    parse_symbol_expr(arg)
}

fn parse_symbol_expr(expr: &str) -> Option<String> {
    let expr = expr.trim_start();
    if expr.starts_with('"') {
        return None;
    }

    let symbol: String = expr
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == ':')
        .collect();
    if symbol.is_empty() {
        return None;
    }

    Some(symbol.rsplit("::").next().unwrap_or(&symbol).to_owned())
}

fn trim_start_offset(value: &str, start: usize) -> usize {
    start
        + value[start..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>()
}

fn attr_key_start(line: &str, key: &str) -> Option<usize> {
    let mut search_from = 0usize;
    while let Some(relative) = line[search_from..].find(key) {
        let key_start = search_from + relative;
        let before_is_ident = line[..key_start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let after_key = &line[key_start + key.len()..];
        if !before_is_ident && after_key.trim_start().starts_with('=') {
            return Some(key_start);
        }
        search_from = key_start + key.len();
    }
    None
}

fn first_type_id_attr_arg(line: &str) -> Option<&str> {
    attr_args(line).and_then(|args| {
        args.into_iter().find(|arg| {
            let arg = arg.trim();
            !arg.is_empty()
                && !is_attr_key(arg, "name")
                && !is_attr_key(arg, "path")
                && !arg.contains('=')
        })
    })
}

fn attr_args(line: &str) -> Option<Vec<&str>> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close <= open {
        return None;
    }
    Some(top_level_attr_args(&line[open + 1..close]))
}

fn top_level_attr_args(args: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaping = false;

    for (idx, ch) in args.char_indices() {
        if in_string {
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(args[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    result.push(args[start..].trim());
    result
}

fn is_attr_key(part: &str, key: &str) -> bool {
    let Some(rest) = part.strip_prefix(key) else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn add_az_rtti_to_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    include_default: bool,
) {
    let tokens = if include_default {
        &["Component", "AzRtti", "Default"][..]
    } else {
        &["Component", "AzRtti"][..]
    };
    if add_derive_tokens_to_attr_block(text, attr_block_start, struct_start, tokens) {
        return;
    }

    let derive_line = if include_default {
        "#[derive(Component, AzRtti, Default)]\n"
    } else {
        "#[derive(Component, AzRtti)]\n"
    };
    text.insert_str(struct_start, derive_line);
}

fn add_az_type_info_to_attr_block(text: &mut String, attr_block_start: usize, struct_start: usize) {
    if add_derive_tokens_to_attr_block(text, attr_block_start, struct_start, &["AzTypeInfo"]) {
        return;
    }

    text.insert_str(struct_start, "#[derive(AzTypeInfo)]\n");
}

fn add_derive_tokens_to_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    preferred_prefix: &[&str],
) -> bool {
    let Some(derive_attr) = find_derive_invocations(text, attr_block_start, struct_start)
        .into_iter()
        .next_back()
    else {
        return false;
    };

    let mut tokens = derive_tokens(&text[derive_attr.start..derive_attr.end]);
    tokens = merge_preferred_derive_tokens(tokens, preferred_prefix);
    replace_derive_invocation(text, derive_attr, &tokens);
    true
}

fn normalize_derive_tokens_in_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
) {
    let mut invocations = find_derive_invocations(text, attr_block_start, struct_start);
    invocations.sort_by_key(|derive_attr| std::cmp::Reverse(derive_attr.start));
    for derive_attr in invocations {
        let tokens = dedupe_derive_tokens(derive_tokens(&text[derive_attr.start..derive_attr.end]));
        replace_derive_invocation(text, derive_attr, &tokens);
    }
}

fn remove_default_from_attr_block(text: &mut String, attr_block_start: usize, struct_start: usize) {
    remove_derive_token_from_attr_block(text, attr_block_start, struct_start, "Default");
}

fn remove_derive_token_from_attr_block(
    text: &mut String,
    attr_block_start: usize,
    struct_start: usize,
    token: &str,
) {
    let mut invocations = find_derive_invocations(text, attr_block_start, struct_start);
    invocations.sort_by_key(|derive_attr| std::cmp::Reverse(derive_attr.start));
    for derive_attr in invocations {
        let tokens = derive_tokens(&text[derive_attr.start..derive_attr.end]);
        if !tokens
            .iter()
            .any(|part| part == token || part.rsplit("::").next() == Some(token))
        {
            continue;
        }
        let tokens = tokens
            .into_iter()
            .filter(|part| part != token && part.rsplit("::").next() != Some(token))
            .collect::<Vec<_>>();
        replace_derive_invocation(text, derive_attr, &tokens);
        return;
    }
}

fn has_derive_token(attr_block: &str, token: &str) -> bool {
    find_derive_invocations(attr_block, 0, attr_block.len())
        .into_iter()
        .any(|derive_attr| {
            derive_attr_has_token(&attr_block[derive_attr.start..derive_attr.end], token)
        })
}

#[derive(Debug, Clone, Copy)]
struct DeriveInvocation {
    start: usize,
    end: usize,
}

fn find_derive_invocations(text: &str, start: usize, end: usize) -> Vec<DeriveInvocation> {
    let mut invocations = Vec::new();
    let mut search_from = start;
    while search_from < end {
        let Some(relative_open) = text[search_from..end].find("#[derive(") else {
            break;
        };
        let attr_start = search_from + relative_open;
        let inner_start = attr_start + "#[derive(".len();
        let Some(relative_close) = text[inner_start..end].find(")]") else {
            break;
        };
        let inner_end = inner_start + relative_close;
        let attr_end = inner_end + ")]".len();
        invocations.push(DeriveInvocation {
            start: attr_start,
            end: attr_end,
        });
        search_from = attr_end;
    }
    invocations
}

fn derive_attr_has_token(attr: &str, token: &str) -> bool {
    derive_tokens(attr)
        .into_iter()
        .any(|part| part == token || part.rsplit("::").next() == Some(token))
}

fn derive_block_has_duplicate_tokens(attr_block: &str) -> bool {
    find_derive_invocations(attr_block, 0, attr_block.len())
        .into_iter()
        .any(|derive_attr| {
            let tokens = derive_tokens(&attr_block[derive_attr.start..derive_attr.end]);
            dedupe_derive_tokens(tokens.clone()).len() != tokens.len()
        })
}

fn derive_tokens(attr: &str) -> Vec<String> {
    let Some(open) = attr.find("#[derive(") else {
        return Vec::new();
    };
    let inner_start = open + "#[derive(".len();
    let Some(close) = attr[inner_start..]
        .find(")]")
        .map(|offset| inner_start + offset)
    else {
        return Vec::new();
    };

    attr[inner_start..close]
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn dedupe_derive_tokens(tokens: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tokens
        .into_iter()
        .filter(|token| {
            let key = token.rsplit("::").next().unwrap_or(token).to_owned();
            seen.insert(key)
        })
        .collect()
}

fn merge_preferred_derive_tokens(tokens: Vec<String>, preferred_prefix: &[&str]) -> Vec<String> {
    let mut remaining = dedupe_derive_tokens(tokens);
    let mut merged = Vec::new();

    for preferred in preferred_prefix {
        if let Some(index) = remaining
            .iter()
            .position(|token| token == preferred || token.rsplit("::").next() == Some(*preferred))
        {
            remaining.remove(index);
        }
        merged.push((*preferred).to_owned());
    }

    merged.extend(remaining);
    dedupe_derive_tokens(merged)
}

fn replace_derive_invocation(text: &mut String, derive_attr: DeriveInvocation, tokens: &[String]) {
    if tokens.is_empty() {
        text.replace_range(derive_attr.start..derive_attr.end, "");
        return;
    }

    let original = &text[derive_attr.start..derive_attr.end];
    let multiline = original.contains('\n');
    let replacement = if multiline {
        let line_start = text[..derive_attr.start]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let indent = &text[line_start..derive_attr.start];
        let item_indent = format!("{indent}    ");
        let mut out = String::new();
        out.push_str("#[derive(\n");
        for token in tokens {
            out.push_str(&item_indent);
            out.push_str(token);
            out.push_str(",\n");
        }
        out.push_str(indent);
        out.push_str(")]");
        out
    } else {
        format!("#[derive({})]", tokens.join(", "))
    };
    text.replace_range(derive_attr.start..derive_attr.end, &replacement);
}

fn parse_type_id_consts(text: &str) -> BTreeMap<String, Uuid> {
    let mut consts = BTreeMap::new();
    let mut search_from = 0usize;

    while let Some(relative) = text[search_from..].find("pub const ") {
        let start = search_from + relative;
        let Some((name, name_end)) = parse_ident(text, start + "pub const ".len()) else {
            search_from = start + "pub const ".len();
            continue;
        };

        if !name.ends_with("_TYPE_ID") || is_non_component_type_id_const(&name) {
            search_from = name_end;
            continue;
        }

        if let Some(type_id) = parse_const_uuid_at(text, start) {
            consts.insert(name, type_id);
        }
        search_from = name_end;
    }

    consts
}

fn is_non_component_type_id_const(name: &str) -> bool {
    name.ends_with("_REF_TYPE_ID")
        || name.ends_with("_VECTOR_TYPE_ID")
        || name.ends_with("_VALUE_TYPE_ID")
        || name.ends_with("_MAP_TYPE_ID")
        || name.ends_with("_ENTRY_TYPE_ID")
}

fn evidence_type_id_for(
    scanned: &ScannedComponent,
    evidence_by_name: &BTreeMap<String, Uuid>,
) -> Option<Uuid> {
    evidence_by_name
        .get(&scanned.component_name)
        .copied()
        .or_else(|| {
            scanned
                .native_name
                .as_deref()
                .and_then(|native_name| evidence_by_name.get(native_name).copied())
        })
}

fn is_component_like_struct(scanned: &ScannedComponent, evidence_matched: bool) -> bool {
    scanned.item_kind == RustItemKind::Struct
        && (evidence_matched
            || scanned.has_bevy_component_derive
            || scanned.has_az_rtti_attr
            || scanned.has_az_rtti_derive)
}

fn find_attr_block_start(text: &str, struct_start: usize) -> usize {
    let struct_line_start = text[..struct_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut block_start = struct_line_start;
    let mut pos = struct_line_start;

    while pos > 0 {
        let line_end = if text.as_bytes().get(pos.wrapping_sub(1)) == Some(&b'\n') {
            pos - 1
        } else {
            pos
        };
        let line_start = text[..line_end]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let line = text[line_start..line_end].trim();
        if line.is_empty() {
            break;
        }

        if line.starts_with("///")
            || line.starts_with("//")
            || line.starts_with('#')
            || is_attribute_continuation_line(line)
        {
            block_start = line_start;
            pos = line_start;
            if line_start == 0 {
                break;
            }
            continue;
        }
        break;
    }

    block_start
}

fn is_attribute_continuation_line(line: &str) -> bool {
    line == "]"
        || line == ")]"
        || line.ends_with(',')
        || line.contains(" = ")
        || line.starts_with(')')
}

fn parse_const_uuid_at(text: &str, const_start: usize) -> Option<Uuid> {
    let slice = &text[const_start..];
    let statement_end = slice.find(';')?;
    let statement = &slice[..statement_end];

    if let Some(macro_start) = statement.find("uuid!(\"") {
        let uuid_start = macro_start + "uuid!(\"".len();
        let uuid_end = statement[uuid_start..].find('"')? + uuid_start;
        return Uuid::parse_str(&statement[uuid_start..uuid_end]).ok();
    }

    let quote_start = statement.find('"')? + 1;
    let quote_end = statement[quote_start..].find('"')? + quote_start;
    Uuid::parse_str(&statement[quote_start..quote_end]).ok()
}

fn parse_ident(text: &str, start: usize) -> Option<(String, usize)> {
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }

    if end == start {
        None
    } else {
        Some((text[start..end].to_owned(), end))
    }
}

fn parse_struct_body(
    text: &str,
    name_end: usize,
) -> (BTreeSet<String>, Vec<String>, Option<usize>, Option<usize>) {
    let Some((offset, next_char)) = text[name_end..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
    else {
        return (BTreeSet::new(), Vec::new(), None, None);
    };
    if next_char == ';' {
        return (BTreeSet::new(), Vec::new(), None, Some(name_end + offset));
    }
    if next_char != '{' {
        return (BTreeSet::new(), Vec::new(), None, None);
    }

    let open = name_end + offset;
    let Some(close) = find_matching_brace(text, open) else {
        return (BTreeSet::new(), Vec::new(), None, None);
    };

    let mut field_names = BTreeSet::new();
    let mut field_names_in_order = Vec::new();
    let body = &text[open + 1..close];
    for line in body.lines() {
        let line = line.trim_start();
        let Some(public_field) = line.strip_prefix("pub ") else {
            continue;
        };
        let Some(colon) = public_field.find(':') else {
            continue;
        };
        let field_name = public_field[..colon].trim();
        if !field_name.is_empty() {
            let field_name = field_name.to_owned();
            if field_names.insert(field_name.clone()) {
                field_names_in_order.push(field_name);
            }
        }
    }

    (field_names, field_names_in_order, Some(close), None)
}

fn find_matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in text[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn type_id_const_name(component_name: &str) -> String {
    format!("{}_TYPE_ID", component_name.to_shouty_snake_case())
}

fn ensure_module_use(text: &mut String, use_line: &str) {
    if text.contains(use_line) {
        return;
    }

    let mut insert_at = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") || trimmed.is_empty() {
            insert_at += line.len();
            continue;
        }
        break;
    }
    text.insert_str(insert_at, &format!("{use_line}\n"));
}

fn ensure_bevy_component_use(text: &mut String) {
    if has_bevy_component_import(text) {
        return;
    }
    ensure_module_use(text, "use bevy::prelude::*;");
}

fn has_bevy_component_import(text: &str) -> bool {
    text.contains("use bevy::prelude::*;")
        || text.contains("use bevy::ecs::component::Component;")
        || text.lines().any(|line| {
            let line = line.trim();
            line.starts_with("use bevy::prelude::{") && line.contains("Component")
        })
}

fn cleanup_unused_uuid_imports(text: &mut String) {
    if !text.contains("use uuid::") {
        return;
    }

    let usage_text = text
        .split_inclusive('\n')
        .filter(|line| !line.trim_start().starts_with("use uuid::"))
        .collect::<String>();
    let uses_uuid_type = contains_ident(&usage_text, "Uuid");
    let uses_uuid_macro = contains_bare_uuid_macro(&usage_text);

    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        match cleanup_uuid_use_line(line, uses_uuid_type, uses_uuid_macro) {
            Some(updated) => out.push_str(&updated),
            None => changed = true,
        }
    }

    if changed || out != *text {
        *text = out;
    }
}

fn cleanup_uuid_use_line(
    line: &str,
    uses_uuid_type: bool,
    uses_uuid_macro: bool,
) -> Option<String> {
    let (body, ending) = split_line_ending(line);
    let indent_len = body.len() - body.trim_start().len();
    let indent = &body[..indent_len];
    let trimmed = body.trim();

    if let Some(inner) = trimmed
        .strip_prefix("use uuid::{")
        .and_then(|value| value.strip_suffix("};"))
    {
        let retained = inner
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .filter(|item| should_retain_uuid_import_item(item, uses_uuid_type, uses_uuid_macro))
            .collect::<Vec<_>>();
        return match retained.as_slice() {
            [] => None,
            [single] => Some(format!("{indent}use uuid::{single};{ending}")),
            _ => Some(format!(
                "{indent}use uuid::{{{}}};{ending}",
                retained.join(", ")
            )),
        };
    }

    if trimmed == "use uuid::Uuid;" && !uses_uuid_type {
        return None;
    }
    if trimmed == "use uuid::uuid;" && !uses_uuid_macro {
        return None;
    }

    Some(line.to_owned())
}

fn should_retain_uuid_import_item(item: &str, uses_uuid_type: bool, uses_uuid_macro: bool) -> bool {
    let name = item.split(" as ").next().unwrap_or(item).trim();
    match name {
        "Uuid" => uses_uuid_type,
        "uuid" => uses_uuid_macro,
        _ => true,
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

fn contains_ident(text: &str, ident: &str) -> bool {
    text.match_indices(ident).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + ident.len()..].chars().next();
        before.is_none_or(|ch| !is_ident_char(ch)) && after.is_none_or(|ch| !is_ident_char(ch))
    })
}

fn contains_bare_uuid_macro(text: &str) -> bool {
    text.match_indices("uuid!").any(|(index, _)| {
        let before = text[..index].chars().next_back();
        before.is_none_or(|ch| !is_ident_char(ch) && ch != ':')
    })
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), SourceReconcileError> {
    let entries = fs::read_dir(root).map_err(|source| SourceReconcileError::Walk {
        path: root.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| SourceReconcileError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    Ok(())
}

fn collect_module_files(root: &Path) -> Result<Vec<PathBuf>, SourceReconcileError> {
    let mut rust_files = Vec::new();
    collect_rust_files(root, &mut rust_files)?;
    let mut module_files = rust_files
        .into_iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("mod.rs"))
        .collect::<Vec<_>>();
    module_files.sort();
    Ok(module_files)
}

fn read_to_string(path: &Path) -> Result<String, SourceReconcileError> {
    fs::read_to_string(path).map_err(|source| SourceReconcileError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn write_string(path: &Path, text: &str) -> Result<(), SourceReconcileError> {
    fs::write(path, text).map_err(|source| SourceReconcileError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_evidence(name: &str, type_id: &str) -> BTreeMap<String, Uuid> {
        BTreeMap::from([(name.to_owned(), Uuid::parse_str(type_id).unwrap())])
    }

    fn support_evidence(name: &str, type_id: &str) -> AzIdentityEvidence {
        AzIdentityEvidence {
            support_type_ids_by_name: BTreeMap::from([(
                name.to_owned(),
                Uuid::parse_str(type_id).unwrap(),
            )]),
            ..Default::default()
        }
    }

    #[test]
    fn scans_top_level_struct_without_hanging_on_line_start() {
        let text = r#"pub const SIMPLE_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("22222222-2222-2222-2222-222222222222");

pub struct SimpleComponent;
"#;

        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("simple.rs"),
            &component_evidence("SimpleComponent", "22222222-2222-2222-2222-222222222222"),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_rtti_attr);
        assert!(fixes[0].add_az_rtti_derive);
    }

    #[test]
    fn moves_reflected_data_type_id_const_to_az_type_info() {
        let text = r#"pub const ABILITY_EDITOR_DATA_TYPE_ID: uuid::Uuid = uuid::uuid!("FDC9954B-ABAC-41E4-9E8F-4651472C480E");

#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect)]
pub struct AbilityEditorData {
    pub ability_crc: u32,
    pub ability_id: String,
}
"#;

        let evidence =
            support_evidence("AbilityEditorData", "FDC9954B-ABAC-41E4-9E8F-4651472C480E");
        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("ability.rs"),
            &evidence,
        );
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].component_name, "AbilityEditorData");
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].add_az_type_info_derive);
        assert!(!fixes[0].add_az_rtti_attr);
        assert!(!fixes[0].add_az_rtti_derive);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use az_derive::AzTypeInfo;"));
        assert!(
            updated
                .contains("#[derive(AzTypeInfo, Debug, Default, Clone, PartialEq, Eq, Reflect)]")
        );
        assert!(updated.contains("#[az_type_info(\"FDC9954B-ABAC-41E4-9E8F-4651472C480E\")]"));
        assert!(!updated.contains("pub const ABILITY_EDITOR_DATA_TYPE_ID"));
    }

    #[test]
    fn resolves_acronym_support_type_id_const() {
        let text = r#"use uuid::{Uuid, uuid};

pub const ALC_SCOPE_DATA_TYPE_ID: Uuid = uuid!("7DC5C9BB-B030-4FA3-B70F-F3F51C2C30CA");

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
pub struct ALCScopeData {
    pub value: u32,
}
"#;

        let evidence = support_evidence("ALCScopeData", "7DC5C9BB-B030-4FA3-B70F-F3F51C2C30CA");
        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("action_list.rs"),
            &evidence,
        );
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].component_name, "ALCScopeData");
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].remove_type_id_const);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_type_info(\"7DC5C9BB-B030-4FA3-B70F-F3F51C2C30CA\")]"));
        assert!(!updated.contains("pub const ALC_SCOPE_DATA_TYPE_ID"));
    }

    #[test]
    fn ignores_plain_type_id_const_without_reflected_support_evidence() {
        let text = r#"use uuid::{Uuid, uuid};

pub const GATHERABLE_CONTROLLER_ACHIEVEMENT_SERVER_STATE_TYPE_ID: Uuid =
    uuid!("72B9409A-7D1A-4831-9CFE-FCB3FADD3426");

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct GatherableControllerAchievementServerState(u8);
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("gatherable_controller.rs"),
            &AzIdentityEvidence::default(),
        );
        assert!(fixes.is_empty());
    }

    #[test]
    fn support_type_fix_keeps_default_derive_with_manual_default_impl() {
        let text = r#"pub const ENCOUNTER_OBJECTIVE_TYPE_ID: uuid::Uuid = uuid::uuid!("63171594-213E-49B1-8A92-66AD42A5EA75");

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
pub struct EncounterObjective {
    pub time_limit: u32,
}

impl Default for EncounterObjective {
    fn default() -> Self {
        Self { time_limit: 0 }
    }
}
"#;

        let evidence =
            support_evidence("EncounterObjective", "63171594-213E-49B1-8A92-66AD42A5EA75");
        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("encounter.rs"),
            &evidence,
        );
        assert_eq!(fixes.len(), 1);
        assert!(!fixes[0].remove_default_derive);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(
            updated.contains("#[derive(AzTypeInfo, Debug, Default, Clone, PartialEq, Reflect)]")
        );
    }

    #[test]
    fn reflected_message_gets_type_info_without_losing_wire_derives() {
        let text = r#"use gridmate::{ClassDesc, Message, Marshaler};

#[derive(Debug, Clone, Default, Marshaler, ClassDesc, Message)]
pub struct FieldUpdateMsg {
    pub field_id: u32,
}
"#;

        let evidence = support_evidence("FieldUpdateMsg", "11111111-2222-3333-4444-555555555555");
        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("actor.rs"),
            &evidence,
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].add_az_type_info_derive);
        assert!(!fixes[0].add_az_rtti_attr);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use az_derive::AzTypeInfo;"));
        assert!(updated.contains(
            "#[derive(AzTypeInfo, Debug, Clone, Default, Marshaler, ClassDesc, Message)]"
        ));
        assert!(updated.contains("#[az_type_info(\"11111111-2222-3333-4444-555555555555\")]"));
    }

    #[test]
    fn class_desc_uuid_moves_to_type_info_and_leaves_only_type_index() {
        let text = r#"use gridmate::{ClassDesc, Message, Marshaler};

#[derive(Debug, Clone, Default, Marshaler, ClassDesc, Message)]
#[class_desc(uuid = "11111111-2222-3333-4444-555555555555", type_index = 42)]
pub struct FieldUpdateMsg {
    pub field_id: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("actor.rs"),
            &AzIdentityEvidence::default(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].add_az_type_info_derive);
        assert!(fixes[0].normalize_class_desc_attr);
        assert!(!fixes[0].add_az_rtti_attr);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use az_derive::AzTypeInfo;"));
        assert!(updated.contains(
            "#[derive(AzTypeInfo, Debug, Clone, Default, Marshaler, ClassDesc, Message)]"
        ));
        assert!(updated.contains("#[az_type_info(\"11111111-2222-3333-4444-555555555555\")]"));
        assert!(updated.contains("#[class_desc(type_index = 42)]"));
        assert!(!updated.contains("class_desc(uuid"));
    }

    #[test]
    fn class_desc_name_moves_to_type_info_name() {
        let text = r#"use gridmate::{ClassDesc, Marshaler};

#[derive(Debug, Clone, Default, Marshaler, ClassDesc)]
#[class_desc(uuid = "11111111-2222-3333-4444-555555555555", name = "NativeFieldUpdate", type_index = 42)]
pub struct FieldUpdateMsg {
    pub field_id: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("actor.rs"),
            &AzIdentityEvidence::default(),
        );
        assert_eq!(fixes.len(), 1);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains(
            "#[az_type_info(name = \"NativeFieldUpdate\", \"11111111-2222-3333-4444-555555555555\")]"
        ));
        assert!(updated.contains("#[class_desc(type_index = 42)]"));
        assert!(!updated.contains("#[class_desc(uuid"));
        assert!(!updated.contains("#[class_desc(name"));
    }

    #[test]
    fn class_desc_uuid_on_enum_moves_to_type_info() {
        let text = r#"use gridmate::{ClassDesc, Marshaler};

#[repr(u8)]
#[derive(Debug, Clone, Marshaler, ClassDesc)]
#[class_desc(uuid = "11111111-2222-3333-4444-555555555555", type_index = 42)]
pub enum JsonItemData {
    Bool(bool) = 1,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("world.rs"),
            &AzIdentityEvidence::default(),
        );
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].component_name, "JsonItemData");
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].add_az_type_info_derive);
        assert!(!fixes[0].add_az_rtti_attr);
        assert!(fixes[0].normalize_class_desc_attr);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[derive(AzTypeInfo, Debug, Clone, Marshaler, ClassDesc)]"));
        assert!(updated.contains("#[az_type_info(\"11111111-2222-3333-4444-555555555555\")]"));
        assert!(updated.contains("#[class_desc(type_index = 42)]"));
        assert!(!updated.contains("#[class_desc(uuid"));
    }

    #[test]
    fn class_desc_normalizes_across_plain_comment_between_attrs() {
        let text = r#"use gridmate::{ClassDesc, Marshaler, Message};

#[derive(Debug, Clone, Default, Marshaler, ClassDesc, Message)]
// UUID recovered from native type info.
#[class_desc(uuid = "11111111-2222-3333-4444-555555555555", type_index = 42)]
#[derive(AzTypeInfo)]
#[az_type_info("11111111-2222-3333-4444-555555555555")]
pub struct FieldUpdateMsg {
    pub field_id: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("actor.rs"),
            &AzIdentityEvidence::default(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].normalize_class_desc_attr);
        assert!(!fixes[0].add_az_type_info_derive);
        assert!(!fixes[0].add_az_type_info_attr);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[class_desc(type_index = 42)]"));
        assert!(!updated.contains("#[class_desc(uuid"));

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            &updated,
            Path::new("actor.rs"),
            &AzIdentityEvidence::default(),
        );
        assert!(fixes.is_empty());
    }

    #[test]
    fn class_desc_uuid_ignores_doc_comment_uuid_words() {
        let text = r#"use gridmate::{ClassDesc, Marshaler, Message};

/// Wire order is the raw Uuid first.
/// This comment says "(item uuid, new instance id)" before the attr.
#[derive(Debug, Clone, Default, Marshaler, ClassDesc, Message)]
#[class_desc(uuid = "11111111-2222-3333-4444-555555555555", type_index = 42)]
pub struct FieldUpdateMsg {
    pub field_id: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("container.rs"),
            &AzIdentityEvidence::default(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_type_info_attr);
        assert!(fixes[0].add_az_type_info_derive);
        assert!(fixes[0].normalize_class_desc_attr);

        let mut updated = text.to_owned();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_type_info(\"11111111-2222-3333-4444-555555555555\")]"));
        assert!(updated.contains("#[class_desc(type_index = 42)]"));

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            &updated,
            Path::new("container.rs"),
            &AzIdentityEvidence::default(),
        );
        assert!(fixes.is_empty());
    }

    #[test]
    fn class_desc_existing_type_info_wins_over_catalog_name_collision() {
        let text = r#"use az_derive::AzTypeInfo;
use gridmate::{ClassDesc, Message};

#[derive(AzTypeInfo, Debug, Clone, Default, ClassDesc, Message)]
#[class_desc(type_index = 2133)]
#[az_type_info("F2C3B42E-DB86-4B2C-840F-64748FE26C73")]
pub struct EncounterEventObjective {
    pub status: u32,
}
"#;
        let evidence = support_evidence(
            "EncounterEventObjective",
            "EAD831AA-836B-4BA5-8E24-4E5BB8119559",
        );

        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("encounter_event_objective.rs"),
            &evidence,
        );
        assert!(fixes.is_empty());
    }

    #[test]
    fn attr_scan_does_not_cross_blank_line_from_previous_component() {
        let text = r#"pub const REAL_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("11111111-1111-1111-1111-111111111111");
pub const HELPER_VECTOR_TYPE_ID: uuid::Uuid = uuid::uuid!("22222222-2222-2222-2222-222222222222");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
#[az_rtti(REAL_COMPONENT_TYPE_ID)]
pub struct RealComponent;

#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect)]
pub struct Helper {
    pub value: u32,
}
"#;

        let fixes =
            plan_az_identity_reconciliation_for_file(text, Path::new("real.rs"), &BTreeMap::new());
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].component_name, "RealComponent");
        assert!(fixes[0].update_az_rtti_attr);
    }

    #[test]
    fn apply_struct_lookup_requires_identifier_boundary() {
        let text = r#"pub const INTERACT_TYPE_ID: uuid::Uuid = uuid::uuid!("B69045A3-A129-4334-BDFB-069C18CC84A2");

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct InteractOptionEntityRef {
    pub value: u32,
}

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Interact {
    pub value: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("interact.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(
            updated.contains(
                "#[az_rtti(\"B69045A3-A129-4334-BDFB-069C18CC84A2\")]\npub struct Interact"
            )
        );
        assert!(!updated.contains(
            "#[az_rtti(\"B69045A3-A129-4334-BDFB-069C18CC84A2\")]\npub struct InteractOptionEntityRef"
        ));
    }

    #[test]
    fn plans_attr_fix_for_existing_type_id_const() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;

pub const TURRET_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("19E7311E-D60B-4049-A589-C519D86880C3");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TurretComponent {
    pub value: u32,
}
"#;

        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("turret.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].component_name, "TurretComponent");
        assert_eq!(
            fixes[0].type_id,
            Uuid::parse_str("19E7311E-D60B-4049-A589-C519D86880C3").unwrap()
        );
        assert!(fixes[0].add_az_rtti_attr);
        assert!(!fixes[0].add_az_rtti_derive);
        assert!(fixes[0].remove_type_id_const);
    }

    #[test]
    fn apply_inserts_inline_az_rtti_attribute() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;

pub const ABILITY_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("DD164EEF-7DBD-4F7C-BCEF-B01A344A33F2");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
pub struct AbilityComponent {
    pub value: u32,
}
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("ability.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"DD164EEF-7DBD-4F7C-BCEF-B01A344A33F2\")]"));
        assert!(!updated.contains("pub const ABILITY_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn adds_derive_and_inline_attr_for_marker_component() {
        let text = r#"use bevy::ecs::component::Component;

#[derive(Component)]
pub struct InventoriesComponent;
"#;
        let mut evidence = BTreeMap::new();
        evidence.insert(
            "InventoriesComponent".to_owned(),
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        );
        let fixes =
            plan_az_identity_reconciliation_for_file(text, Path::new("inventories.rs"), &evidence);
        assert_eq!(fixes.len(), 1);
        assert!(!fixes[0].remove_type_id_const);
        assert!(fixes[0].add_az_rtti_derive);
        assert!(fixes[0].add_az_rtti_attr);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use az_derive::AzRtti;"));
        assert!(updated.contains("#[az_rtti(\"11111111-1111-1111-1111-111111111111\")]"));
        assert!(updated.contains("#[derive(Component, AzRtti, Default)]"));
        assert!(!updated.contains("INVENTORIES_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn reconciles_reflected_facet_with_owner_evidence() {
        let text = r#"use newworld_derive::Facet;
use uuid::{Uuid, uuid};

pub const ACTION_LIST_COMPONENT_SERVER_FACET_TYPE_ID: Uuid =
    uuid!("F37AB16D-4C98-4B21-899A-29548F0A788A");

#[derive(Facet)]
#[facet(owner = WrongOwnerComponent)]
pub struct ActionListComponentServerFacet;
"#;
        let evidence = AzIdentityEvidence {
            type_ids_by_name: BTreeMap::from([(
                "ActionListComponentServerFacet".to_owned(),
                Uuid::parse_str("F37AB16D-4C98-4B21-899A-29548F0A788A").unwrap(),
            )]),
            support_type_ids_by_name: BTreeMap::new(),
            facet_owners_by_name: BTreeMap::from([(
                "ActionListComponentServerFacet".to_owned(),
                "ActionListComponent".to_owned(),
            )]),
        };
        let fixes = plan_az_identity_reconciliation_for_file_with_evidence(
            text,
            Path::new("action_list.rs"),
            &evidence,
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_rtti_attr);
        assert!(fixes[0].add_az_rtti_derive);
        assert!(fixes[0].remove_type_id_const);
        assert!(fixes[0].update_facet_owner_attr);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use az_derive::AzRtti;"));
        assert!(updated.contains("use bevy::prelude::*;"));
        assert!(updated.contains("#[az_rtti(\"F37AB16D-4C98-4B21-899A-29548F0A788A\")]"));
        assert!(updated.contains("#[facet(owner = ActionListComponent)]"));
        assert!(updated.contains("#[derive(Component, AzRtti, Default, Facet)]"));
        assert!(!updated.contains("pub const ACTION_LIST_COMPONENT_SERVER_FACET_TYPE_ID"));
        assert!(!updated.contains("WrongOwnerComponent"));
    }

    #[test]
    fn ignores_protocol_only_facet_without_reflected_evidence() {
        let text = r#"use newworld_derive::Facet;

#[derive(Facet)]
#[facet(owner = ActionListComponent)]
pub struct SyntheticProtocolFacet;
"#;
        let fixes =
            plan_az_identity_reconciliation_for_file(text, Path::new("alc.rs"), &BTreeMap::new());
        assert!(fixes.is_empty());
    }

    #[test]
    fn updates_const_referenced_attr_to_inline_literal() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;

pub const VOICE_CHAT_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("8911D004-3EBD-4262-AF67-9D5EA803E8E3");

#[derive(Component, AzRtti, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[az_rtti(VOICE_CHAT_COMPONENT_TYPE_ID)]
pub struct VoiceChatComponent;
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("voice_chat.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].update_az_rtti_attr);
        assert!(fixes[0].remove_type_id_const);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"8911D004-3EBD-4262-AF67-9D5EA803E8E3\")]"));
        assert!(!updated.contains("pub const VOICE_CHAT_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn removes_orphan_uuid_import_after_const_migration() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;
use uuid::{Uuid, uuid};

pub const VOICE_CHAT_COMPONENT_TYPE_ID: Uuid = uuid!("8911D004-3EBD-4262-AF67-9D5EA803E8E3");

#[derive(Component, AzRtti, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[az_rtti(VOICE_CHAT_COMPONENT_TYPE_ID)]
pub struct VoiceChatComponent;
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("voice_chat.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(!updated.contains("use uuid::{Uuid, uuid};"));
        assert!(!updated.contains("use uuid::Uuid;"));
        assert!(!updated.contains("use uuid::uuid;"));
        assert!(updated.contains("#[az_rtti(\"8911D004-3EBD-4262-AF67-9D5EA803E8E3\")]"));
    }

    #[test]
    fn retains_uuid_type_import_after_const_migration_when_fields_use_it() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;
use uuid::{Uuid, uuid};

pub const GROUPS_COMPONENT_TYPE_ID: Uuid = uuid!("3097A91B-7D18-4178-AAD3-DB1A6BBA8E7B");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[az_rtti(GROUPS_COMPONENT_TYPE_ID)]
pub struct GroupsComponent {
    pub group_id: Uuid,
}
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("groups.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("use uuid::Uuid;"));
        assert!(!updated.contains("use uuid::{Uuid, uuid};"));
        assert!(!updated.contains("use uuid::uuid;"));
        assert!(updated.contains("pub group_id: Uuid"));
    }

    #[test]
    fn resolves_type_id_const_from_existing_attr_for_acronym_component_names() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;

pub const AI_TARGETABLE_COMPONENT_TYPE_ID: uuid::Uuid = uuid::uuid!("E3A3B44D-2E89-4F70-845A-07AE7D4BBF01");

#[derive(Component, AzRtti, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[az_rtti(AI_TARGETABLE_COMPONENT_TYPE_ID)]
#[reflect(Component)]
pub struct AITargetableComponent;
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("ai_targetable.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].update_az_rtti_attr);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"E3A3B44D-2E89-4F70-845A-07AE7D4BBF01\")]"));
        assert!(!updated.contains("pub const AI_TARGETABLE_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn resolves_acronym_type_id_const_without_existing_attr() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;
use uuid::{Uuid, uuid};

pub const AI_PATH_COMPONENT_TYPE_ID: Uuid = uuid!("FA7FA996-F8C3-4CD0-8313-042FB6A66578");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct AIPathComponent;
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("ai_path.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_rtti_attr);
        assert!(fixes[0].remove_type_id_const);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"FA7FA996-F8C3-4CD0-8313-042FB6A66578\")]"));
        assert!(!updated.contains("pub const AI_PATH_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn removes_const_and_rewrites_remaining_same_file_references() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;
use uuid::{Uuid, uuid};

pub const AI_PATH_COMPONENT_TYPE_ID: Uuid = uuid!("FA7FA996-F8C3-4CD0-8313-042FB6A66578");

#[derive(Component, AzRtti, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct AIPathComponent;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_id_matches() {
        assert_eq!(
            AI_PATH_COMPONENT_TYPE_ID,
            uuid!("FA7FA996-F8C3-4CD0-8313-042FB6A66578")
        );
    }
}
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("ai_path.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_rtti_attr);
        assert!(fixes[0].remove_type_id_const);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"FA7FA996-F8C3-4CD0-8313-042FB6A66578\")]"));
        assert!(!updated.contains("pub const AI_PATH_COMPONENT_TYPE_ID"));
        assert!(updated.contains("<AIPathComponent as az_core::type_info::AzTypeInfo>::TYPE_ID"));
    }

    #[test]
    fn resolves_string_uuid_component_type_id_const() {
        let text = r#"use az_derive::AzRtti;
use bevy::prelude::*;

pub const GAME_TRANSFORM_COMPONENT_TYPE_ID: &str = "484AE67D-ABD0-4D9C-B2C8-9BB0EEC900E0";

#[derive(Component, AzRtti, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct GameTransformComponent;
"#;
        let fixes = plan_az_identity_reconciliation_for_file(
            text,
            Path::new("transform.rs"),
            &BTreeMap::new(),
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].add_az_rtti_attr);
        assert!(fixes[0].remove_type_id_const);

        let mut updated = text.to_string();
        apply_az_identity_reconciliation(&mut updated, &fixes);
        assert!(updated.contains("#[az_rtti(\"484AE67D-ABD0-4D9C-B2C8-9BB0EEC900E0\")]"));
        assert!(!updated.contains("pub const GAME_TRANSFORM_COMPONENT_TYPE_ID"));
    }

    #[test]
    fn removes_deleted_component_type_ids_from_module_reexports() {
        let mut text = r#"pub use ability::{
    ABILITY_COMPONENT_CLIENT_FACET_TYPE_ID, ABILITY_COMPONENT_SERVER_FACET_TYPE_ID,
    ABILITY_COMPONENT_TYPE_ID, ABILITY_EDITOR_DATA_TYPE_ID, AbilityComponent,
};
pub use ai_path::{
    AI_PATH_COMPONENT_CLIENT_FACET_TYPE_ID, AI_PATH_COMPONENT_SERVER_FACET_TYPE_ID,
    AI_PATH_COMPONENT_TYPE_ID, AIPathComponent,
};
"#
        .to_string();
        let removed = BTreeSet::from([
            "ABILITY_COMPONENT_TYPE_ID".to_owned(),
            "AI_PATH_COMPONENT_TYPE_ID".to_owned(),
        ]);

        cleanup_component_module_reexports(&mut text, &removed);

        assert!(!text.contains("ABILITY_COMPONENT_TYPE_ID"));
        assert!(!text.contains("AI_PATH_COMPONENT_TYPE_ID"));
        assert!(text.contains("ABILITY_COMPONENT_CLIENT_FACET_TYPE_ID"));
        assert!(text.contains("AIPathComponent"));
    }
}
