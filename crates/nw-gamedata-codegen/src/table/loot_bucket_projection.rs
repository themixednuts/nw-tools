use std::collections::BTreeMap;

use anyhow::{Result, bail};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitFloat, LitInt};

use super::LOOT_BUCKET_ROW_TYPE_NAME;
use super::cell_types::table_code_cell_type;
use super::model::RustField;
use crate::game_system_schema::{GameSystemColumnSchema, GameSystemTableSchema};

#[derive(Debug, Clone, Copy)]
struct LootBucketRenderField<'a> {
    column: &'a GameSystemColumnSchema,
    field: &'a RustField,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LootBucketRenderGroup<'a> {
    index: u16,
    loot_bucket: LootBucketRenderField<'a>,
    filter_looted_items: Option<LootBucketRenderField<'a>>,
    loot_biasing_disabled: Option<LootBucketRenderField<'a>>,
    tags: Option<LootBucketRenderField<'a>>,
    match_one: Option<LootBucketRenderField<'a>>,
    item: Option<LootBucketRenderField<'a>>,
    quantity: Option<LootBucketRenderField<'a>>,
    odds: Option<LootBucketRenderField<'a>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LootBucketRenderGroupBuilder<'a> {
    loot_bucket: Option<LootBucketRenderField<'a>>,
    filter_looted_items: Option<LootBucketRenderField<'a>>,
    loot_biasing_disabled: Option<LootBucketRenderField<'a>>,
    tags: Option<LootBucketRenderField<'a>>,
    match_one: Option<LootBucketRenderField<'a>>,
    item: Option<LootBucketRenderField<'a>>,
    quantity: Option<LootBucketRenderField<'a>>,
    odds: Option<LootBucketRenderField<'a>>,
}

impl<'a> LootBucketRenderGroupBuilder<'a> {
    fn finish(self, index: u16) -> Option<LootBucketRenderGroup<'a>> {
        Some(LootBucketRenderGroup {
            index,
            loot_bucket: self.loot_bucket?,
            filter_looted_items: self.filter_looted_items,
            loot_biasing_disabled: self.loot_biasing_disabled,
            tags: self.tags,
            match_one: self.match_one,
            item: self.item,
            quantity: self.quantity,
            odds: self.odds,
        })
    }
}

pub(super) fn loot_bucket_column_groups<'a>(
    schema: &'a GameSystemTableSchema,
    rust_fields: &'a [RustField],
) -> Vec<LootBucketRenderGroup<'a>> {
    if schema.row_type_name != LOOT_BUCKET_ROW_TYPE_NAME {
        return Vec::new();
    }

    let mut groups = BTreeMap::<u16, LootBucketRenderGroupBuilder<'a>>::new();
    for (column, field) in schema.columns.iter().zip(rust_fields.iter()) {
        let Some((stem, index)) = numbered_column_parts(&column.name) else {
            continue;
        };
        let entry = LootBucketRenderField { column, field };
        let group = groups.entry(index).or_default();
        match stem {
            "LootBucket" => group.loot_bucket = Some(entry),
            "FilterLootedItems" => group.filter_looted_items = Some(entry),
            "LootBiasingDisabled" => group.loot_biasing_disabled = Some(entry),
            "Tags" => group.tags = Some(entry),
            "MatchOne" => group.match_one = Some(entry),
            "Item" => group.item = Some(entry),
            "Quantity" => group.quantity = Some(entry),
            "Odds" => group.odds = Some(entry),
            _ => {}
        }
    }

    groups
        .into_iter()
        .filter_map(|(index, group)| group.finish(index))
        .collect()
}

pub(super) fn numbered_column_parts(value: &str) -> Option<(&str, u16)> {
    let stem_len = value
        .trim_end_matches(|character: char| character.is_ascii_digit())
        .len();
    if stem_len == value.len() {
        return None;
    }
    let index = value[stem_len..].parse::<u16>().ok()?;
    (index != 0).then_some((&value[..stem_len], index))
}

pub(super) fn loot_bucket_projection_types_tokens() -> TokenStream {
    quote! {
        #[derive(Debug, Clone)]
        pub struct LootBucketColumn<'a> {
            pub index: u16,
            pub loot_bucket: &'a str,
            pub filter_looted_items: bool,
            pub loot_biasing_disabled: bool,
            pub entries: Vec<LootBucketColumnEntry<'a>>,
        }

        #[derive(Debug, Clone)]
        pub struct LootBucketColumnEntry<'a> {
            pub source_row: gamedata::RowIndex,
            pub items: Vec<&'a str>,
            pub tags: Vec<LootBucketColumnTag<'a>>,
            pub match_one: bool,
            pub quantity: core::ops::RangeInclusive<u16>,
            pub odds: f32,
        }

        #[derive(Debug, Clone)]
        pub struct LootBucketColumnTag<'a> {
            pub tag_id: &'a str,
            pub range: Option<core::ops::RangeInclusive<u16>>,
        }
    }
}

pub(super) fn loot_bucket_columns_method_tokens(
    groups: &[LootBucketRenderGroup<'_>],
) -> Result<TokenStream> {
    let groups = groups
        .iter()
        .map(loot_bucket_column_group_tokens)
        .collect::<Result<Vec<_>>>()?;
    Ok(quote! {
        pub fn loot_bucket_columns(
            &self,
        ) -> Result<Vec<LootBucketColumn<'a>>, gamedata::GameDataError> {
            let mut columns = Vec::new();
            if self.is_empty() {
                return Ok(columns);
            }
            let header_row_index = loot_bucket_row_index(0)?;
            let Some(header) = self.row_at(header_row_index) else {
                return Ok(columns);
            };
            #(#groups)*
            Ok(columns)
        }
    })
}

fn loot_bucket_column_group_tokens(group: &LootBucketRenderGroup<'_>) -> Result<TokenStream> {
    let header = ident("header");
    let row = ident("row");
    let items = ident("items");
    let tags = ident("tags");
    let loot_bucket = loot_bucket_text_option_expr(&header, group.loot_bucket)?;
    let filter_looted_items = group
        .filter_looted_items
        .map(|field| loot_bucket_bool_expr(&header, field))
        .transpose()?
        .unwrap_or_else(|| quote!(false));
    let loot_biasing_disabled = group
        .loot_biasing_disabled
        .map(|field| loot_bucket_bool_expr(&header, field))
        .transpose()?
        .unwrap_or_else(|| quote!(false));
    let match_one = group
        .match_one
        .map(|field| loot_bucket_bool_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(false));
    let quantity = group
        .quantity
        .map(|field| loot_bucket_quantity_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(0..=0));
    let odds = group
        .odds
        .map(|field| loot_bucket_f32_expr(&row, field, 1.0))
        .transpose()?
        .unwrap_or_else(|| quote!(1.0));
    let item_fill = group
        .item
        .map(|entry| loot_bucket_text_vec_fill_tokens(&items, &row, entry))
        .transpose()?
        .unwrap_or_default();
    let tag_fill = group
        .tags
        .map(|entry| loot_bucket_tag_vec_fill_tokens(&tags, &row, entry))
        .transpose()?
        .unwrap_or_default();
    let index = LitInt::new(&group.index.to_string(), Span::call_site());

    Ok(quote! {
        {
            if let Some(loot_bucket) = #loot_bucket {
                let filter_looted_items = #filter_looted_items;
                let loot_biasing_disabled = #loot_biasing_disabled;
                let mut entries = Vec::new();
                for (zero_based, row) in self.rows().enumerate() {
                    let source_row = loot_bucket_row_index(zero_based)?;
                    let mut items = Vec::new();
                    #item_fill
                    if items.is_empty() {
                        continue;
                    }
                    let mut tags = Vec::new();
                    #tag_fill
                    let match_one = #match_one;
                    let quantity = #quantity;
                    let odds = #odds;
                    entries.push(LootBucketColumnEntry {
                        source_row,
                        items,
                        tags,
                        match_one,
                        quantity,
                        odds,
                    });
                }
                columns.push(LootBucketColumn {
                    index: #index,
                    loot_bucket,
                    filter_looted_items,
                    loot_biasing_disabled,
                    entries,
                });
            }
        }
    })
}

fn loot_bucket_text_option_expr(
    row: &Ident,
    entry: LootBucketRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_text(Some(#row.#field()?))))
            } else {
                Ok(quote!(loot_bucket_text(#row.#field())))
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be text, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn loot_bucket_text_vec_fill_tokens(
    target: &Ident,
    row: &Ident,
    entry: LootBucketRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_push_text(&mut #target, Some(#row.#field()?));))
            } else {
                Ok(quote!(loot_bucket_push_text(&mut #target, #row.#field());))
            }
        }
        gamedata::CellType::List(gamedata::ListElementType::Scalar(
            gamedata::ScalarType::String,
        )) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_extend_text_list(&mut #target, #row.#field()?)?;))
            } else {
                Ok(quote! {
                    if let Some(list) = #row.#field() {
                        loot_bucket_extend_text_list(&mut #target, list)?;
                    }
                })
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be text or text-list, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn loot_bucket_bool_expr(row: &Ident, entry: LootBucketRenderField<'_>) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::Bool) => {
            if entry.column.required {
                Ok(quote!(#row.#field()?))
            } else {
                Ok(quote!(#row.#field().unwrap_or(false)))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_bool_from_text(Some(#row.#field()?))?.unwrap_or(false)))
            } else {
                Ok(quote!(loot_bucket_bool_from_text(#row.#field())?.unwrap_or(false)))
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be bool or text bool, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn loot_bucket_tag_vec_fill_tokens(
    target: &Ident,
    row: &Ident,
    entry: LootBucketRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_extend_tags(&mut #target, Some(#row.#field()?))?;))
            } else {
                Ok(quote!(loot_bucket_extend_tags(&mut #target, #row.#field())?;))
            }
        }
        gamedata::CellType::List(gamedata::ListElementType::Scalar(
            gamedata::ScalarType::String,
        )) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_extend_tag_list(&mut #target, #row.#field()?)?;))
            } else {
                Ok(quote! {
                    if let Some(list) = #row.#field() {
                        loot_bucket_extend_tag_list(&mut #target, list)?;
                    }
                })
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be tag text or tag text-list, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn loot_bucket_quantity_expr(row: &Ident, entry: LootBucketRenderField<'_>) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::U32) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_range_from_u32(#row.#field()?)?))
            } else {
                Ok(quote!(#row.#field()
                    .map(loot_bucket_range_from_u32)
                    .transpose()?
                    .unwrap_or(0..=0)))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::NonZeroU32) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_range_from_u32(#row.#field()?.get())?))
            } else {
                Ok(quote!(#row.#field()
                    .map(|value| loot_bucket_range_from_u32(value.get()))
                    .transpose()?
                    .unwrap_or(0..=0)))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::F32) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_range_from_f32(#row.#field()?)?))
            } else {
                Ok(quote!(#row.#field()
                    .map(loot_bucket_range_from_f32)
                    .transpose()?
                    .unwrap_or(0..=0)))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(loot_bucket_range_from_text(Some(#row.#field()?))?))
            } else {
                Ok(quote!(loot_bucket_range_from_text(#row.#field())?))
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be quantity integer or text range, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn loot_bucket_f32_expr(
    row: &Ident,
    entry: LootBucketRenderField<'_>,
    default: f32,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    let default = LitFloat::new(&format!("{default:?}"), Span::call_site());
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::F32) => {
            if entry.column.required {
                Ok(quote!(#row.#field()?))
            } else {
                Ok(quote!(#row.#field().unwrap_or(#default)))
            }
        }
        cell_type => bail!(
            "LootBucketData column {} must be odds float, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

pub(super) fn loot_bucket_projection_helpers_tokens() -> TokenStream {
    quote! {
        fn loot_bucket_row_index(
            zero_based: usize,
        ) -> Result<gamedata::RowIndex, gamedata::GameDataError> {
            let one_based = zero_based
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    gamedata::GameDataError::Decode(
                        "LootBucketData row index exceeded u32 range".into(),
                    )
                })?;
            gamedata::RowIndex::from_one_based(one_based).ok_or_else(|| {
                gamedata::GameDataError::Decode(
                    "LootBucketData produced invalid row index 0".into(),
                )
            })
        }

        fn loot_bucket_text(value: Option<&str>) -> Option<&str> {
            value.filter(|value| !value.trim().is_empty())
        }

        fn loot_bucket_push_text<'a>(values: &mut Vec<&'a str>, value: Option<&'a str>) {
            if let Some(value) = loot_bucket_text(value) {
                values.push(value);
            }
        }

        fn loot_bucket_extend_text_list<'a, C: gamedata::TableColumn>(
            values: &mut Vec<&'a str>,
            list: gamedata::List<'a, C, &'a str>,
        ) -> Result<(), gamedata::GameDataError> {
            for value in list {
                loot_bucket_push_text(values, Some(value?));
            }
            Ok(())
        }

        fn loot_bucket_bool_from_text(
            value: Option<&str>,
        ) -> Result<Option<bool>, gamedata::GameDataError> {
            let Some(value) = loot_bucket_text(value) else {
                return Ok(None);
            };
            match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(Some(true)),
                "false" | "0" | "no" => Ok(Some(false)),
                _ => Err(gamedata::GameDataError::Decode(format!(
                    "LootBucketData bool field contains `{value}`"
                ))),
            }
        }

        fn loot_bucket_extend_tags<'a>(
            tags: &mut Vec<LootBucketColumnTag<'a>>,
            value: Option<&'a str>,
        ) -> Result<(), gamedata::GameDataError> {
            let Some(value) = loot_bucket_text(value) else {
                return Ok(());
            };
            for token in value.split(',').map(str::trim).filter(|token| !token.is_empty()) {
                let (tag_id, range) = match token.split_once(':') {
                    Some((tag_id, range)) => {
                        let Some(tag_id) = loot_bucket_text(Some(tag_id)) else {
                            return Err(gamedata::GameDataError::Decode(
                                "LootBucketData tag descriptor has an empty tag id".into(),
                            ));
                        };
                        (tag_id, Some(loot_bucket_tag_range_from_text(range)?))
                    }
                    None => (token, None),
                };
                tags.push(LootBucketColumnTag { tag_id, range });
            }
            Ok(())
        }

        fn loot_bucket_extend_tag_list<'a, C: gamedata::TableColumn>(
            tags: &mut Vec<LootBucketColumnTag<'a>>,
            list: gamedata::List<'a, C, &'a str>,
        ) -> Result<(), gamedata::GameDataError> {
            for value in list {
                loot_bucket_extend_tags(tags, Some(value?))?;
            }
            Ok(())
        }

        fn loot_bucket_range_from_text(
            value: Option<&str>,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            let Some(value) = loot_bucket_text(value) else {
                return Ok(0..=0);
            };
            loot_bucket_u16_range_from_text(value, LootBucketSingleRangeMax::SameValue)
        }

        fn loot_bucket_tag_range_from_text(
            value: &str,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            loot_bucket_u16_range_from_text(value, LootBucketSingleRangeMax::Fixed(10_000))
        }

        #[derive(Debug, Clone, Copy)]
        enum LootBucketSingleRangeMax {
            SameValue,
            Fixed(u16),
        }

        fn loot_bucket_u16_range_from_text(
            value: &str,
            single_max: LootBucketSingleRangeMax,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            let parts = value.split('-').map(str::trim).collect::<Vec<_>>();
            match parts.as_slice() {
                [] => Ok(0..=0),
                [single] => {
                    let start = loot_bucket_u16_from_text(single)?;
                    let end = match single_max {
                        LootBucketSingleRangeMax::SameValue => start,
                        LootBucketSingleRangeMax::Fixed(value) => value,
                    };
                    Ok(start.min(end)..=start.max(end))
                }
                [left, right] => {
                    let start = loot_bucket_u16_from_text(left)?;
                    let end = loot_bucket_u16_from_text(right)?;
                    Ok(start.min(end)..=start.max(end))
                }
                _ => Err(gamedata::GameDataError::Decode(format!(
                    "LootBucketData range `{value}` has too many `-` separators"
                ))),
            }
        }

        fn loot_bucket_range_from_u32(
            value: u32,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            let value = loot_bucket_u16_from_u32(value)?;
            Ok(value..=value)
        }

        fn loot_bucket_range_from_f32(
            value: f32,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            let value = loot_bucket_u16_from_f32(value)?;
            Ok(value..=value)
        }

        fn loot_bucket_u16_from_text(value: &str) -> Result<u16, gamedata::GameDataError> {
            let value = value.trim();
            if value.is_empty() {
                return Ok(0);
            }
            if let Ok(value) = value.parse::<i64>() {
                return loot_bucket_u16_from_i64(value);
            }
            if let Ok(value) = value.parse::<f32>()
                && value.is_finite()
            {
                return loot_bucket_u16_from_f32(value);
            }
            Err(gamedata::GameDataError::Decode(format!(
                "LootBucketData range value `{value}` is not u16-compatible"
            )))
        }

        fn loot_bucket_u16_from_u32(value: u32) -> Result<u16, gamedata::GameDataError> {
            u16::try_from(value).map_err(|_| {
                gamedata::GameDataError::Decode(format!(
                    "LootBucketData range value `{value}` exceeds u16 range"
                ))
            })
        }

        fn loot_bucket_u16_from_i64(value: i64) -> Result<u16, gamedata::GameDataError> {
            u16::try_from(value).map_err(|_| {
                gamedata::GameDataError::Decode(format!(
                    "LootBucketData range value `{value}` exceeds u16 range"
                ))
            })
        }

        fn loot_bucket_u16_from_f32(value: f32) -> Result<u16, gamedata::GameDataError> {
            if !value.is_finite() {
                return Err(gamedata::GameDataError::Decode(format!(
                    "LootBucketData range value `{value}` is not finite"
                )));
            }
            let integer = format!("{:.0}", value.trunc())
                .parse::<i64>()
                .map_err(|_| {
                    gamedata::GameDataError::Decode(format!(
                        "LootBucketData range value `{value}` cannot be represented as i64"
                    ))
                })?;
            loot_bucket_u16_from_i64(integer)
        }
    }
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}
