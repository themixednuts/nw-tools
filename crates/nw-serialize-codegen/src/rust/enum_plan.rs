use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{SerializeCodegenItem, SerializeCodegenVariant};
use crate::naming::rust_variant_ident;
use crate::rust::types::RustTypeRenderer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustEnumRawConversionPlan {
    pub raw_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustVariantPlan {
    pub source_name: String,
    pub rust_name: String,
    pub discriminant: Option<i32>,
    pub payload_type: Option<String>,
    pub payload_has_materialized_fields: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RustEnumPlanner {
    rust_types: RustTypeRenderer,
}

impl RustEnumPlanner {
    pub(super) const fn new(rust_types: RustTypeRenderer) -> Self {
        Self { rust_types }
    }

    pub(super) fn plan_variants(&self, item: &SerializeCodegenItem) -> Vec<RustVariantPlan> {
        let mut used = BTreeMap::<String, usize>::new();
        item.variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let base = rust_variant_ident(&variant.source_name);
                RustVariantPlan {
                    source_name: variant.source_name.clone(),
                    rust_name: unique_variant_name(base, variant, index, &mut used),
                    discriminant: variant.value_i32,
                    payload_type: None,
                    payload_has_materialized_fields: false,
                }
            })
            .collect()
    }

    pub(super) fn plan_raw_conversion(
        &self,
        item: &SerializeCodegenItem,
    ) -> Option<RustEnumRawConversionPlan> {
        if item.variants.is_empty()
            || item
                .variants
                .iter()
                .any(|variant| variant.value_i32.is_none())
        {
            return None;
        }

        let raw_type = item
            .enum_underlying_type
            .as_ref()
            .map(|resolved| self.rust_types.render(resolved))
            .filter(|raw_type| is_rust_integer_type(raw_type))
            .map(|raw_type| widen_raw_enum_type_if_needed(&raw_type, item))
            .unwrap_or_else(|| "i32".to_owned());

        Some(RustEnumRawConversionPlan { raw_type })
    }
}

fn unique_variant_name(
    base: String,
    variant: &SerializeCodegenVariant,
    index: usize,
    used: &mut BTreeMap<String, usize>,
) -> String {
    if !used.contains_key(&base) {
        used.insert(base.clone(), 1);
        return base;
    }

    let mut candidate = format!(
        "{base}{}",
        variant_value_suffix(variant).unwrap_or_else(|| format!("Variant{index}"))
    );
    while used.contains_key(&candidate) {
        let next_index = used.get(&base).copied().unwrap_or(1) + 1;
        used.insert(base.clone(), next_index);
        candidate = format!("{base}Variant{next_index}");
    }
    used.insert(candidate.clone(), 1);
    candidate
}

fn variant_value_suffix(variant: &SerializeCodegenVariant) -> Option<String> {
    variant
        .value_i32
        .map(signed_value_suffix)
        .or_else(|| variant.value_u32.map(|value| format!("Value{value}")))
        .or_else(|| variant.value_u64.map(|value| format!("Value{value}")))
}

fn signed_value_suffix(value: i32) -> String {
    if let Some(abs) = value.checked_abs() {
        if value < 0 {
            format!("ValueMinus{abs}")
        } else {
            format!("Value{abs}")
        }
    } else {
        "ValueMin".to_owned()
    }
}

pub(super) fn enum_has_duplicate_discriminants(item: &SerializeCodegenItem) -> bool {
    let mut seen = BTreeSet::new();
    item.variants
        .iter()
        .filter_map(|variant| variant.value_i32)
        .any(|value| !seen.insert(value))
}

fn widen_raw_enum_type_if_needed(raw_type: &str, item: &SerializeCodegenItem) -> String {
    if raw_enum_type_fits_values(raw_type, item) {
        return raw_type.to_owned();
    }

    if item
        .variants
        .iter()
        .filter_map(|variant| variant.value_i32)
        .all(|value| value >= 0)
    {
        "u32".to_owned()
    } else {
        "i32".to_owned()
    }
}

fn raw_enum_type_fits_values(raw_type: &str, item: &SerializeCodegenItem) -> bool {
    item.variants
        .iter()
        .filter_map(|variant| variant.value_i32)
        .all(|value| raw_enum_type_fits_value(raw_type, value))
}

fn raw_enum_type_fits_value(raw_type: &str, value: i32) -> bool {
    match raw_type {
        "u8" => u8::try_from(value).is_ok(),
        "u16" => u16::try_from(value).is_ok(),
        "u32" | "u64" | "usize" => value >= 0,
        "i8" => i8::try_from(value).is_ok(),
        "i16" => i16::try_from(value).is_ok(),
        "i32" | "i64" | "isize" => true,
        _ => true,
    }
}

pub(super) fn is_rust_integer_type(value: &str) -> bool {
    matches!(
        value,
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize"
    )
}
