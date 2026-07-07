use std::collections::BTreeMap;

use anyhow::{Result, bail};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Ident, LitInt};

use super::PERK_BUCKET_ROW_TYPE_NAME;
use super::cell_types::table_code_cell_type;
use super::loot_bucket_projection::numbered_column_parts;
use super::model::RustField;
use crate::game_system_schema::{GameSystemColumnSchema, GameSystemTableSchema};

#[derive(Debug, Clone, Copy)]
struct PerkBucketRenderField<'a> {
    column: &'a GameSystemColumnSchema,
    field: &'a RustField,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PerkBucketRenderGroup<'a> {
    index: u16,
    perk: PerkBucketRenderField<'a>,
}

pub(super) fn perk_bucket_column_groups<'a>(
    schema: &'a GameSystemTableSchema,
    rust_fields: &'a [RustField],
) -> Vec<PerkBucketRenderGroup<'a>> {
    if schema.row_type_name != PERK_BUCKET_ROW_TYPE_NAME {
        return Vec::new();
    }

    let mut groups = BTreeMap::<u16, PerkBucketRenderField<'a>>::new();
    for (column, field) in schema.columns.iter().zip(rust_fields.iter()) {
        let Some((stem, index)) = numbered_column_parts(&column.name) else {
            continue;
        };
        if stem == "Perk" {
            groups.insert(index, PerkBucketRenderField { column, field });
        }
    }

    groups
        .into_iter()
        .map(|(index, perk)| PerkBucketRenderGroup { index, perk })
        .collect()
}

pub(super) fn perk_bucket_projection_types_tokens(table_marker: &Ident) -> TokenStream {
    let key_alias = ident(&format!("{table_marker}Key"));
    perk_bucket_projection_types_tokens_for_key(quote!(#key_alias))
}

pub(super) fn perk_bucket_projection_types_tokens_for_key(key_alias: impl ToTokens) -> TokenStream {
    quote! {
        #[derive(Debug, Clone)]
        pub struct PerkBucketProjection<'a> {
            pub source_row: gamedata::RowIndex,
            pub perk_bucket_id: &'a str,
            pub ignore_exclusive_label_weights: bool,
            pub disable_perk_biasing: bool,
            pub perk_type: Option<&'a str>,
            pub perk_chance: f32,
            pub entries: Vec<PerkBucketProjectionEntry<'a>>,
        }

        #[derive(Debug, Clone, Copy)]
        pub struct PerkBucketProjectionEntry<'a> {
            pub index: u16,
            pub target: PerkBucketProjectionEntryTarget<'a>,
            pub weight: f32,
        }

        #[derive(Debug, Clone, Copy)]
        pub enum PerkBucketProjectionEntryTarget<'a> {
            Perk {
                perk_id: &'a str,
            },
            PerkBucket {
                perk_bucket_id: &'a str,
                key: #key_alias,
            },
        }

        impl<'a> PerkBucketProjectionEntryTarget<'a> {
            #[must_use]
            pub const fn as_str(self) -> &'a str {
                match self {
                    Self::Perk { perk_id } => perk_id,
                    Self::PerkBucket { perk_bucket_id, .. } => perk_bucket_id,
                }
            }
        }
    }
}

pub(super) fn perk_bucket_projection_method_tokens(
    table_marker: &Ident,
    groups: &[PerkBucketRenderGroup<'_>],
) -> Result<TokenStream> {
    let key_expr = quote!(#table_marker::key(weight_row_id.as_str()));
    perk_bucket_projection_method_tokens_for_key(key_expr, groups)
}

pub(super) fn perk_bucket_projection_method_tokens_for_key(
    weight_key_expr: impl ToTokens,
    groups: &[PerkBucketRenderGroup<'_>],
) -> Result<TokenStream> {
    let groups = groups
        .iter()
        .map(perk_bucket_projection_group_tokens)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        pub fn perk_bucket_projections(
            &self,
        ) -> Result<Vec<PerkBucketProjection<'a>>, gamedata::GameDataError> {
            let mut projections = Vec::new();
            for (zero_based, row) in self.rows().enumerate() {
                let source_row = perk_bucket_row_index(zero_based)?;
                let perk_bucket_id = row.perk_bucket_id()?.as_str();
                let Some(perk_bucket_id) = perk_bucket_text(Some(perk_bucket_id)) else {
                    continue;
                };
                if perk_bucket_id.contains("_Weights") {
                    continue;
                }

                let mut weight_row_id = String::with_capacity(perk_bucket_id.len() + 8);
                weight_row_id.push_str(perk_bucket_id);
                weight_row_id.push_str("_Weights");
                let weight_key = #weight_key_expr;
                let Some(weight_row) = self.get(&weight_key) else {
                    return Err(gamedata::GameDataError::Decode(format!(
                        "PerkBucketData row `{perk_bucket_id}` is missing companion row `{weight_row_id}`"
                    )));
                };

                let mut entries = Vec::new();
                #(#groups)*

                projections.push(PerkBucketProjection {
                    source_row,
                    perk_bucket_id,
                    ignore_exclusive_label_weights: row
                        .ignore_exclusive_label_weights()
                        .unwrap_or(false),
                    disable_perk_biasing: row.disable_perk_biasing().unwrap_or(false),
                    perk_type: perk_bucket_text(row.perk_type()),
                    perk_chance: row.perk_chance().unwrap_or(0.0),
                    entries,
                });
            }
            Ok(projections)
        }
    })
}

fn perk_bucket_projection_group_tokens(group: &PerkBucketRenderGroup<'_>) -> Result<TokenStream> {
    let row = ident("row");
    let weight_row = ident("weight_row");
    let value = perk_bucket_text_option_expr(&row, group.perk)?;
    let weight_value = perk_bucket_text_option_expr(&weight_row, group.perk)?;
    let index = LitInt::new(&group.index.to_string(), Span::call_site());

    Ok(quote! {
        if let Some(value) = #value {
            let weight = perk_bucket_entry_weight(#weight_value, perk_bucket_id, #index)?;
            let target = perk_bucket_entry_target(self, value, perk_bucket_id, #index)?;
            entries.push(PerkBucketProjectionEntry {
                index: #index,
                target,
                weight,
            });
        }
    })
}

fn perk_bucket_text_option_expr(
    row: &Ident,
    entry: PerkBucketRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(perk_bucket_text(Some(#row.#field()?))))
            } else {
                Ok(quote!(perk_bucket_text(#row.#field())))
            }
        }
        cell_type => bail!(
            "PerkBucketData column {} must be text, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

pub(super) fn perk_bucket_projection_helpers_tokens(
    table_marker: &Ident,
    view_struct: &Ident,
) -> TokenStream {
    let key_alias = ident(&format!("{table_marker}Key"));
    let view_type = quote!(#view_struct<'a>);
    let key_expr = quote!(#table_marker::key(perk_bucket_id));
    perk_bucket_projection_helpers_tokens_for_key(view_type, quote!(#key_alias), key_expr)
}

pub(super) fn perk_bucket_projection_helpers_tokens_for_key(
    view_type: impl ToTokens,
    key_alias: impl ToTokens,
    key_expr: impl ToTokens,
) -> TokenStream {
    quote! {
        fn perk_bucket_row_index(
            zero_based: usize,
        ) -> Result<gamedata::RowIndex, gamedata::GameDataError> {
            let one_based = zero_based
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    gamedata::GameDataError::Decode(
                        "PerkBucketData row index exceeded u32 range".into(),
                    )
                })?;
            gamedata::RowIndex::from_one_based(one_based).ok_or_else(|| {
                gamedata::GameDataError::Decode(
                    "PerkBucketData produced invalid row index 0".into(),
                )
            })
        }

        fn perk_bucket_text(value: Option<&str>) -> Option<&str> {
            value.map(str::trim).filter(|value| !value.is_empty())
        }

        fn perk_bucket_entry_weight(
            value: Option<&str>,
            perk_bucket_id: &str,
            index: u16,
        ) -> Result<f32, gamedata::GameDataError> {
            let Some(value) = perk_bucket_text(value) else {
                return Err(gamedata::GameDataError::Decode(format!(
                    "PerkBucketData row `{perk_bucket_id}` has Perk{index} without companion weight"
                )));
            };
            value.parse::<f32>().map_err(|error| {
                gamedata::GameDataError::Decode(format!(
                    "PerkBucketData row `{perk_bucket_id}` companion Perk{index} weight `{value}` is not f32: {error}"
                ))
            })
        }

        fn perk_bucket_entry_target<'a>(
            view: &#view_type,
            value: &'a str,
            source_perk_bucket_id: &str,
            index: u16,
        ) -> Result<PerkBucketProjectionEntryTarget<'a>, gamedata::GameDataError> {
            const BUCKET_REFERENCE_PREFIX: &str = "[PBID]";

            if let Some(perk_bucket_id) = value.strip_prefix(BUCKET_REFERENCE_PREFIX) {
                let Some(perk_bucket_id) = perk_bucket_text(Some(perk_bucket_id)) else {
                    return Err(gamedata::GameDataError::Decode(format!(
                        "PerkBucketData row `{source_perk_bucket_id}` Perk{index} has an empty [PBID] reference"
                    )));
                };
                let key: #key_alias = #key_expr;
                if view.get(&key).is_none() {
                    return Err(gamedata::GameDataError::Decode(format!(
                        "PerkBucketData row `{source_perk_bucket_id}` Perk{index} references missing bucket `{perk_bucket_id}`"
                    )));
                }
                Ok(PerkBucketProjectionEntryTarget::PerkBucket { perk_bucket_id, key })
            } else {
                Ok(PerkBucketProjectionEntryTarget::Perk { perk_id: value })
            }
        }
    }
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}
