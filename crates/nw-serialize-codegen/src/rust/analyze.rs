use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::{Attribute, Expr, Fields, Item, Lit, LitStr, Path, Type, punctuated::Punctuated};
use thiserror::Error;
use uuid::Uuid;

use crate::rust::enum_plan::RustVariantPlan;
use crate::rust::identity::{RustTypeIdentityKind, RustTypeIdentityPlan};
use crate::rust::item_plan::{RustFieldPlan, RustItemKind, RustItemPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceFile {
    items_by_name: BTreeMap<String, RustSourceItem>,
}

impl RustSourceFile {
    pub fn parse(source: &str) -> Result<Self, RustSourceAnalyzeError> {
        let file = syn::parse_file(source).map_err(RustSourceAnalyzeError::Parse)?;
        Ok(Self::from_syn_file(&file))
    }

    pub fn items(&self) -> impl Iterator<Item = &RustSourceItem> {
        self.items_by_name.values()
    }

    #[must_use]
    pub fn item(&self, name: &str) -> Option<&RustSourceItem> {
        self.items_by_name.get(name)
    }

    pub fn plan_item_update(
        &self,
        desired: &RustItemPlan,
    ) -> Result<RustItemUpdatePlan, RustSourceAnalyzeError> {
        let Some(existing) = self.item(&desired.rust_name) else {
            return Ok(RustItemUpdatePlan::missing(desired));
        };

        let kind_mismatch =
            (!item_kinds_match(existing.kind, desired.kind)).then_some(RustItemKindMismatch {
                existing: existing.kind,
                expected: desired.kind,
            });
        let missing_derives = desired
            .derives
            .iter()
            .filter(|derive_name| !existing.derives.contains(*derive_name))
            .cloned()
            .collect();
        let identity = identity_update(existing, &desired.identity);
        let (missing_fields, field_type_mismatches) =
            field_updates(existing, desired).map_err(|source| {
                RustSourceAnalyzeError::DesiredFieldType {
                    item_name: desired.rust_name.clone(),
                    field_name: source.field_name,
                    rust_type: source.rust_type,
                    source: source.source,
                }
            })?;
        let (missing_variants, variant_discriminant_mismatches) =
            variant_updates(existing, desired);

        Ok(RustItemUpdatePlan {
            item_name: desired.rust_name.clone(),
            status: RustItemStatus::Present,
            existing_kind: Some(existing.kind),
            expected_kind: desired.kind,
            kind_mismatch,
            missing_derives,
            identity,
            missing_fields,
            field_type_mismatches,
            missing_variants,
            variant_discriminant_mismatches,
        })
    }

    fn from_syn_file(file: &syn::File) -> Self {
        let items_by_name = file
            .items
            .iter()
            .filter_map(RustSourceItem::from_syn_item)
            .map(|item| (item.name.clone(), item))
            .collect();
        Self { items_by_name }
    }

    pub(crate) fn from_items_for_planning(items: impl IntoIterator<Item = RustSourceItem>) -> Self {
        let items_by_name = items
            .into_iter()
            .map(|item| (item.name.clone(), item))
            .collect();
        Self { items_by_name }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceItem {
    pub name: String,
    pub kind: RustItemKind,
    pub derives: BTreeSet<String>,
    pub identity_attrs: Vec<RustIdentityAttr>,
    pub field_order: Vec<String>,
    pub fields: BTreeMap<String, RustSourceField>,
    pub variants: BTreeMap<String, RustSourceVariant>,
}

impl RustSourceItem {
    fn from_syn_item(item: &Item) -> Option<Self> {
        match item {
            Item::Struct(item) => {
                let (fields, field_order) = source_fields(&item.fields);
                Some(Self {
                    name: item.ident.to_string(),
                    kind: RustItemKind::Struct,
                    derives: derive_names(&item.attrs),
                    identity_attrs: identity_attrs(&item.attrs),
                    field_order,
                    fields,
                    variants: BTreeMap::new(),
                })
            }
            Item::Enum(item) => Some(Self {
                name: item.ident.to_string(),
                kind: RustItemKind::Enum,
                derives: derive_names(&item.attrs),
                identity_attrs: identity_attrs(&item.attrs),
                field_order: Vec::new(),
                fields: BTreeMap::new(),
                variants: source_variants(item),
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn fields_in_order(&self) -> Vec<&RustSourceField> {
        self.field_order
            .iter()
            .filter_map(|field_name| self.fields.get(field_name))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceField {
    pub name: String,
    pub rust_type: String,
    pub marshal_as: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceVariant {
    pub name: String,
    pub discriminant: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustIdentityAttr {
    pub kind: RustTypeIdentityKind,
    pub name: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_id_expr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustItemUpdatePlan {
    pub item_name: String,
    pub status: RustItemStatus,
    pub existing_kind: Option<RustItemKind>,
    pub expected_kind: RustItemKind,
    pub kind_mismatch: Option<RustItemKindMismatch>,
    pub missing_derives: Vec<String>,
    pub identity: RustIdentityUpdate,
    pub missing_fields: Vec<RustFieldPlan>,
    pub field_type_mismatches: Vec<RustFieldTypeMismatch>,
    pub missing_variants: Vec<RustVariantPlan>,
    pub variant_discriminant_mismatches: Vec<RustVariantDiscriminantMismatch>,
}

impl RustItemUpdatePlan {
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.status == RustItemStatus::Present
            && self.kind_mismatch.is_none()
            && self.missing_derives.is_empty()
            && self.identity == RustIdentityUpdate::Current
            && self.missing_fields.is_empty()
            && self.field_type_mismatches.is_empty()
            && self.missing_variants.is_empty()
            && self.variant_discriminant_mismatches.is_empty()
    }

    fn missing(desired: &RustItemPlan) -> Self {
        Self {
            item_name: desired.rust_name.clone(),
            status: RustItemStatus::Missing,
            existing_kind: None,
            expected_kind: desired.kind,
            kind_mismatch: None,
            missing_derives: desired.derives.clone(),
            identity: RustIdentityUpdate::Missing {
                expected_kind: desired.identity.kind,
                expected_type_id: desired.identity.type_id,
                expected_name: desired.identity.name.clone(),
            },
            missing_fields: desired.fields.clone(),
            field_type_mismatches: Vec::new(),
            missing_variants: desired.variants.clone(),
            variant_discriminant_mismatches: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustItemStatus {
    Present,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustItemKindMismatch {
    pub existing: RustItemKind,
    pub expected: RustItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustIdentityUpdate {
    Current,
    Missing {
        expected_kind: RustTypeIdentityKind,
        expected_type_id: Uuid,
        expected_name: Option<String>,
    },
    Update {
        existing_kind: RustTypeIdentityKind,
        existing_type_id: Option<Uuid>,
        existing_name: Option<String>,
        expected_kind: RustTypeIdentityKind,
        expected_type_id: Uuid,
        expected_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFieldTypeMismatch {
    pub field_name: String,
    pub existing_type: String,
    pub expected_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustVariantDiscriminantMismatch {
    pub variant_name: String,
    pub existing_discriminant: Option<i32>,
    pub expected_discriminant: Option<i32>,
}

#[derive(Debug, Error)]
pub enum RustSourceAnalyzeError {
    #[error("failed to parse Rust source")]
    Parse(#[source] syn::Error),
    #[error(
        "invalid desired Rust field type `{rust_type}` for field `{field_name}` on `{item_name}`"
    )]
    DesiredFieldType {
        item_name: String,
        field_name: String,
        rust_type: String,
        #[source]
        source: syn::Error,
    },
}

struct DesiredFieldTypeError {
    field_name: String,
    rust_type: String,
    source: syn::Error,
}

fn derive_names(attrs: &[Attribute]) -> BTreeSet<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .filter_map(|attr| {
            attr.parse_args_with(Punctuated::<Path, syn::Token![,]>::parse_terminated)
                .ok()
        })
        .flat_map(|paths| paths.into_iter().filter_map(|path| path_leaf_name(&path)))
        .collect()
}

fn identity_attrs(attrs: &[Attribute]) -> Vec<RustIdentityAttr> {
    attrs.iter().filter_map(identity_attr).collect()
}

fn identity_attr(attr: &Attribute) -> Option<RustIdentityAttr> {
    let kind = if attr
        .path()
        .is_ident(RustTypeIdentityKind::AzTypeInfo.integrated_attr_name())
        || attr.path().is_ident("type_info")
    {
        RustTypeIdentityKind::AzTypeInfo
    } else if attr
        .path()
        .is_ident(RustTypeIdentityKind::AzRtti.integrated_attr_name())
        || attr.path().is_ident("rtti")
    {
        RustTypeIdentityKind::AzRtti
    } else {
        return None;
    };

    let mut parsed = RustIdentityAttr {
        kind,
        name: None,
        type_id: None,
        type_id_expr: None,
    };
    let _ = attr
        .parse_args_with(Punctuated::<Expr, syn::Token![,]>::parse_terminated)
        .map(|args| parse_identity_attr_args(args, &mut parsed));

    Some(parsed)
}

fn parse_identity_attr_args(args: Punctuated<Expr, syn::Token![,]>, parsed: &mut RustIdentityAttr) {
    for arg in args {
        match &arg {
            Expr::Assign(assign) if expr_assign_lhs_is(&assign.left, "name") => {
                if let Some(name) = lit_str_expr_value(&assign.right) {
                    parsed.name = Some(name);
                }
            }
            Expr::Assign(_) => {}
            _ if parsed.type_id_expr.is_none() => parse_identity_type_id_expr(&arg, parsed),
            _ => {}
        }
    }
}

fn parse_identity_type_id_expr(expr: &Expr, parsed: &mut RustIdentityAttr) {
    parsed.type_id_expr = Some(expr.to_token_stream().to_string());
    parsed.type_id = uuid_from_expr(expr);
}

fn expr_assign_lhs_is(expr: &Expr, name: &str) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn lit_str_expr_value(expr: &Expr) -> Option<String> {
    let Expr::Lit(expr) = expr else {
        return None;
    };
    let Lit::Str(lit) = &expr.lit else {
        return None;
    };
    Some(lit.value())
}

fn uuid_from_expr(expr: &Expr) -> Option<Uuid> {
    match expr {
        Expr::Lit(expr) => {
            let Lit::Str(lit) = &expr.lit else {
                return None;
            };
            parse_uuid(&lit.value())
        }
        Expr::Macro(expr) => {
            let lit = syn::parse2::<LitStr>(expr.mac.tokens.clone()).ok()?;
            parse_uuid(&lit.value())
        }
        _ => None,
    }
}

fn source_fields(fields: &Fields) -> (BTreeMap<String, RustSourceField>, Vec<String>) {
    let Fields::Named(fields) = fields else {
        return (BTreeMap::new(), Vec::new());
    };
    let mut field_map = BTreeMap::new();
    let mut field_order = Vec::new();
    for field in &fields.named {
        let Some(ident) = &field.ident else {
            continue;
        };
        let name = ident.to_string();
        let source_field = RustSourceField {
            name: name.clone(),
            rust_type: normalize_type(&field.ty),
            marshal_as: marshal_as_attr(&field.attrs),
        };
        field_order.push(name.clone());
        field_map.insert(name, source_field);
    }
    (field_map, field_order)
}

fn marshal_as_attr(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("marshal"))
        .and_then(|attr| {
            attr.parse_args_with(Punctuated::<Expr, syn::Token![,]>::parse_terminated)
                .ok()
        })
        .and_then(|args| {
            args.into_iter().find_map(|arg| {
                let Expr::Assign(assign) = arg else {
                    return None;
                };
                if !expr_assign_lhs_is(&assign.left, "as") {
                    return None;
                }
                lit_str_expr_value(&assign.right)
                    .or_else(|| Some(assign.right.to_token_stream().to_string()))
            })
        })
}

fn source_variants(item: &syn::ItemEnum) -> BTreeMap<String, RustSourceVariant> {
    item.variants
        .iter()
        .map(|variant| {
            let name = variant.ident.to_string();
            let discriminant = variant
                .discriminant
                .as_ref()
                .and_then(|(_, expr)| expr_i32(expr));
            (name.clone(), RustSourceVariant { name, discriminant })
        })
        .collect()
}

fn identity_update(
    existing: &RustSourceItem,
    expected: &RustTypeIdentityPlan,
) -> RustIdentityUpdate {
    let existing_attr = existing
        .identity_attrs
        .iter()
        .find(|attr| attr.kind == expected.kind)
        .or_else(|| existing.identity_attrs.first());
    let Some(existing_attr) = existing_attr else {
        return RustIdentityUpdate::Missing {
            expected_kind: expected.kind,
            expected_type_id: expected.type_id,
            expected_name: expected.name.clone(),
        };
    };

    if identity_kind_matches(existing_attr.kind, expected.kind)
        && existing_attr.type_id == Some(expected.type_id)
        && identity_name_matches(existing_attr.name.as_deref(), expected.name.as_deref())
    {
        RustIdentityUpdate::Current
    } else {
        RustIdentityUpdate::Update {
            existing_kind: existing_attr.kind,
            existing_type_id: existing_attr.type_id,
            existing_name: existing_attr.name.clone(),
            expected_kind: expected.kind,
            expected_type_id: expected.type_id,
            expected_name: expected.name.clone(),
        }
    }
}

fn identity_kind_matches(existing: RustTypeIdentityKind, expected: RustTypeIdentityKind) -> bool {
    existing == expected
}

fn item_kinds_match(existing: RustItemKind, desired: RustItemKind) -> bool {
    matches!(
        (existing, desired),
        (RustItemKind::Enum, RustItemKind::SumEnum)
    ) || existing == desired
}

fn identity_name_matches(existing: Option<&str>, expected: Option<&str>) -> bool {
    expected.is_none_or(|expected| existing == Some(expected))
}

fn field_updates(
    existing: &RustSourceItem,
    desired: &RustItemPlan,
) -> Result<(Vec<RustFieldPlan>, Vec<RustFieldTypeMismatch>), DesiredFieldTypeError> {
    if desired.kind != RustItemKind::Struct || existing.kind != RustItemKind::Struct {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut missing = Vec::new();
    let mut mismatches = Vec::new();
    for field in &desired.fields {
        let expected_type =
            normalize_type_str(&field.rust_type).map_err(|source| DesiredFieldTypeError {
                field_name: field.rust_name.clone(),
                rust_type: field.rust_type.clone(),
                source,
            })?;
        match existing.fields.get(&field.rust_name) {
            Some(existing_field) if existing_field.rust_type == expected_type => {}
            Some(existing_field) => mismatches.push(RustFieldTypeMismatch {
                field_name: field.rust_name.clone(),
                existing_type: existing_field.rust_type.clone(),
                expected_type,
            }),
            None => missing.push(field.clone()),
        }
    }

    Ok((missing, mismatches))
}

fn variant_updates(
    existing: &RustSourceItem,
    desired: &RustItemPlan,
) -> (Vec<RustVariantPlan>, Vec<RustVariantDiscriminantMismatch>) {
    if !matches!(desired.kind, RustItemKind::Enum | RustItemKind::SumEnum)
        || existing.kind != RustItemKind::Enum
    {
        return (Vec::new(), Vec::new());
    }

    let mut missing = Vec::new();
    let mut mismatches = Vec::new();
    for variant in &desired.variants {
        match existing.variants.get(&variant.rust_name) {
            Some(existing_variant) if existing_variant.discriminant == variant.discriminant => {}
            Some(existing_variant) => mismatches.push(RustVariantDiscriminantMismatch {
                variant_name: variant.rust_name.clone(),
                existing_discriminant: existing_variant.discriminant,
                expected_discriminant: variant.discriminant,
            }),
            None => missing.push(variant.clone()),
        }
    }
    (missing, mismatches)
}

fn normalize_type_str(value: &str) -> Result<String, syn::Error> {
    syn::parse_str::<Type>(value).map(|ty| normalize_type(&ty))
}

fn normalize_type(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn path_leaf_name(path: &Path) -> Option<String> {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn expr_i32(expr: &Expr) -> Option<i32> {
    expr.to_token_stream()
        .to_string()
        .replace(' ', "")
        .parse::<i32>()
        .ok()
}

fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;
    use crate::rust::enum_plan::RustEnumRawConversionPlan;
    use crate::rust::plan::RustCodegenPlanner;
    use crate::rust::source::RustSourceEmitter;

    use super::*;

    #[test]
    fn reports_no_delta_for_emitted_component_source() {
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
                    "name": "Example::HealthComponent",
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
                            "name": "m_value",
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
        let item = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("component plan");
        let source =
            RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline()).expect("source");

        let analyzed = RustSourceFile::parse(&source).expect("parse emitted source");
        let update = analyzed.plan_item_update(item).expect("update plan");

        assert!(update.is_current());
    }

    #[test]
    fn reports_derive_identity_and_field_deltas_for_existing_source() {
        let desired = RustItemPlan {
            source_type_id: uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"),
            source_name: "Example::HealthComponent".to_owned(),
            is_reflected_base: false,
            is_slot_owner: false,
            has_layout_family_descendants: false,
            is_bevy_component: true,
            file_stem_override: None,
            scope_path: vec!["example".to_owned()],
            family_scope_path: vec!["example".to_owned(), "health_components".to_owned()],
            rust_name: "HealthComponent".to_owned(),
            kind: RustItemKind::Struct,
            identity: RustTypeIdentityPlan::az_rtti(
                uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"),
                Some("Example::HealthComponent".to_owned()),
            ),
            repr: None,
            raw_conversion: None,
            derives: vec![
                "Component".to_owned(),
                "AzRtti".to_owned(),
                "Debug".to_owned(),
                "Default".to_owned(),
                "Clone".to_owned(),
                "PartialEq".to_owned(),
                "Serialize".to_owned(),
                "Deserialize".to_owned(),
                "Reflect".to_owned(),
            ],
            rtti_bases: Vec::new(),
            fields: vec![RustFieldPlan {
                source_name: "m_value".to_owned(),
                rust_name: "value".to_owned(),
                source_type_id: uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"),
                rust_type: "u32".to_owned(),
                unresolved_type: None,
                integer_range: None,
                data_size: None,
                offset: None,
                flags: None,
                is_base_class: false,
            }],
            variants: Vec::new(),
        };
        let source = r#"
use bevy::prelude::Component;

#[derive(Component, Debug)]
pub struct HealthComponent {
    pub value: i16,
}
"#;

        let analyzed = RustSourceFile::parse(source).expect("parse source");
        let update = analyzed.plan_item_update(&desired).expect("update plan");

        assert_eq!(update.status, RustItemStatus::Present);
        assert_eq!(
            update.missing_derives,
            vec![
                "AzRtti",
                "Default",
                "Clone",
                "PartialEq",
                "Serialize",
                "Deserialize",
                "Reflect"
            ]
        );
        assert_eq!(
            update.identity,
            RustIdentityUpdate::Missing {
                expected_kind: RustTypeIdentityKind::AzRtti,
                expected_type_id: uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"),
                expected_name: Some("Example::HealthComponent".to_owned()),
            }
        );
        assert!(update.missing_fields.is_empty());
        assert_eq!(
            update.field_type_mismatches,
            vec![RustFieldTypeMismatch {
                field_name: "value".to_owned(),
                existing_type: "i16".to_owned(),
                expected_type: "u32".to_owned(),
            }]
        );
        assert!(!update.is_current());
    }

    #[test]
    fn reports_enum_variant_discriminant_deltas() {
        let desired = RustItemPlan {
            source_type_id: uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"),
            source_name: "Mode".to_owned(),
            is_reflected_base: false,
            is_slot_owner: false,
            has_layout_family_descendants: false,
            is_bevy_component: false,
            file_stem_override: None,
            scope_path: vec!["global".to_owned()],
            family_scope_path: vec!["modes".to_owned()],
            rust_name: "Mode".to_owned(),
            kind: RustItemKind::Enum,
            identity: RustTypeIdentityPlan::az_type_info(
                uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"),
                Some("Mode".to_owned()),
            ),
            repr: Some("i32".to_owned()),
            raw_conversion: Some(RustEnumRawConversionPlan {
                raw_type: "i32".to_owned(),
            }),
            derives: vec!["AzTypeInfo".to_owned(), "Debug".to_owned()],
            rtti_bases: Vec::new(),
            fields: Vec::new(),
            variants: vec![
                RustVariantPlan {
                    source_name: "Enabled".to_owned(),
                    rust_name: "Enabled".to_owned(),
                    discriminant: Some(7),
                    payload_type: None,
                },
                RustVariantPlan {
                    source_name: "Disabled".to_owned(),
                    rust_name: "Disabled".to_owned(),
                    discriminant: Some(8),
                    payload_type: None,
                },
            ],
        };
        let source = r#"
#[derive(AzTypeInfo, Debug)]
#[az_type_info(name = "Mode", "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC")]
pub enum Mode {
    Enabled = 6,
}
"#;

        let analyzed = RustSourceFile::parse(source).expect("parse source");
        let update = analyzed.plan_item_update(&desired).expect("update plan");

        assert_eq!(update.identity, RustIdentityUpdate::Current);
        assert_eq!(
            update.variant_discriminant_mismatches,
            vec![RustVariantDiscriminantMismatch {
                variant_name: "Enabled".to_owned(),
                existing_discriminant: Some(6),
                expected_discriminant: Some(7),
            }]
        );
        assert_eq!(update.missing_variants.len(), 1);
        assert_eq!(update.missing_variants[0].rust_name, "Disabled");
    }
}
