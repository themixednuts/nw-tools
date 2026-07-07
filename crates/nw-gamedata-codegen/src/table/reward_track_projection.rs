use std::collections::BTreeMap;

use anyhow::{Result, bail};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitInt};

use super::cell_types::table_code_cell_type;
use super::loot_bucket_projection::numbered_column_parts;
use super::model::RustField;
use crate::game_system_schema::{GameSystemColumnSchema, GameSystemTableSchema};

const PVP_STORE_ROW_TYPE_NAME: &str = "PvPStoreData";

#[derive(Debug, Clone, Copy)]
struct RewardTrackRenderField<'a> {
    column: &'a GameSystemColumnSchema,
    field: &'a RustField,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RewardTrackRenderGroup<'a> {
    index: u16,
    bucket: RewardTrackRenderField<'a>,
    tag: Option<RewardTrackRenderField<'a>>,
    match_one: Option<RewardTrackRenderField<'a>>,
    reward_id: RewardTrackRenderField<'a>,
    random_weight: RewardTrackRenderField<'a>,
    budget_contribution: RewardTrackRenderField<'a>,
    reward_type: Option<RewardTrackRenderField<'a>>,
    stage_exclusion: Option<RewardTrackRenderField<'a>>,
    shop_exclusion: Option<RewardTrackRenderField<'a>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct RewardTrackRenderGroupBuilder<'a> {
    bucket: Option<RewardTrackRenderField<'a>>,
    tag: Option<RewardTrackRenderField<'a>>,
    match_one: Option<RewardTrackRenderField<'a>>,
    reward_id: Option<RewardTrackRenderField<'a>>,
    random_weight: Option<RewardTrackRenderField<'a>>,
    budget_contribution: Option<RewardTrackRenderField<'a>>,
    reward_type: Option<RewardTrackRenderField<'a>>,
    stage_exclusion: Option<RewardTrackRenderField<'a>>,
    shop_exclusion: Option<RewardTrackRenderField<'a>>,
}

impl<'a> RewardTrackRenderGroupBuilder<'a> {
    fn finish(self, index: u16) -> Option<RewardTrackRenderGroup<'a>> {
        Some(RewardTrackRenderGroup {
            index,
            bucket: self.bucket?,
            tag: self.tag,
            match_one: self.match_one,
            reward_id: self.reward_id?,
            random_weight: self.random_weight?,
            budget_contribution: self.budget_contribution?,
            reward_type: self.reward_type,
            stage_exclusion: self.stage_exclusion,
            shop_exclusion: self.shop_exclusion,
        })
    }
}

pub(super) fn reward_track_column_groups<'a>(
    schema: &'a GameSystemTableSchema,
    rust_fields: &'a [RustField],
) -> Vec<RewardTrackRenderGroup<'a>> {
    if schema.row_type_name != PVP_STORE_ROW_TYPE_NAME {
        return Vec::new();
    }

    let mut groups = BTreeMap::<u16, RewardTrackRenderGroupBuilder<'a>>::new();
    for (column, field) in schema.columns.iter().zip(rust_fields.iter()) {
        let Some((stem, index)) = numbered_column_parts(&column.name) else {
            continue;
        };
        let entry = RewardTrackRenderField { column, field };
        let group = groups.entry(index).or_default();
        match stem {
            "Bucket" => group.bucket = Some(entry),
            "Tag" => group.tag = Some(entry),
            "MatchOne" => group.match_one = Some(entry),
            "RewardId" => group.reward_id = Some(entry),
            "RandomWeights" => group.random_weight = Some(entry),
            "BudgetContribution" => group.budget_contribution = Some(entry),
            "Type" => group.reward_type = Some(entry),
            "ExcludeTypeStage" => group.stage_exclusion = Some(entry),
            "ExcludeTypeShop" => group.shop_exclusion = Some(entry),
            _ => {}
        }
    }

    let groups = groups
        .into_iter()
        .filter_map(|(index, group)| group.finish(index))
        .collect::<Vec<_>>();

    if groups
        .iter()
        .any(|group| !reward_track_group_has_strict_reward_id(group))
    {
        return Vec::new();
    }

    groups
}

fn reward_track_group_has_strict_reward_id(group: &RewardTrackRenderGroup<'_>) -> bool {
    matches!(
        table_code_cell_type(group.reward_id.column, group.reward_id.field),
        gamedata::CellType::Scalar(gamedata::ScalarType::ForeignKey)
    )
}

pub(super) fn reward_track_projection_types_tokens() -> TokenStream {
    quote! {
        #[derive(Debug, Clone)]
        pub struct PvPStoreRewardTrackSlot<'a> {
            pub index: u8,
            pub bucket_id: &'a str,
            pub entries: Vec<PvPStoreRewardTrackEntry<'a>>,
        }

        #[derive(Debug, Clone)]
        pub struct PvPStoreRewardTrackEntry<'a> {
            pub source_row: gamedata::RowIndex,
            pub reward_id: gamedata::ForeignKey<
                'a,
                super::super::reward_track_item_data::reward_track_items::RewardIdColumn,
            >,
            pub reward_type: Option<az_core::crc::Crc32>,
            pub tag_constraints: Vec<PvPStoreRewardTrackTagConstraint>,
            pub match_one: bool,
            pub random_weight: u32,
            pub budget_contribution: u32,
            pub stage_exclusion: Option<az_core::crc::Crc32>,
            pub shop_exclusion: Option<az_core::crc::Crc32>,
        }

        #[derive(Debug, Clone)]
        pub struct PvPStoreRewardTrackTagConstraint {
            pub tag: az_core::crc::Crc32,
            pub range: core::ops::RangeInclusive<u16>,
        }
    }
}

pub(super) fn reward_track_projection_method_tokens(
    groups: &[RewardTrackRenderGroup<'_>],
) -> Result<TokenStream> {
    let groups = groups
        .iter()
        .map(reward_track_group_tokens)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        pub fn reward_track_slots(
            &self,
        ) -> Result<Vec<PvPStoreRewardTrackSlot<'a>>, gamedata::GameDataError> {
            let mut slots = Vec::new();
            let header_row_index = reward_track_store_row_index(0)?;
            let Some(header) = self.row_at(header_row_index) else {
                return Ok(slots);
            };
            #(#groups)*
            Ok(slots)
        }
    })
}

fn reward_track_group_tokens(group: &RewardTrackRenderGroup<'_>) -> Result<TokenStream> {
    let header = ident("header");
    let row = ident("row");
    let bucket_id = reward_track_text_option_expr(&header, group.bucket)?;
    let reward_id = reward_track_foreign_key_expr(&row, group.reward_id)?;
    let reward_type = group
        .reward_type
        .map(|field| reward_track_crc_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(None));
    let tag_constraints = group
        .tag
        .map(|field| reward_track_tag_constraints_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(Vec::new()));
    let match_one = group
        .match_one
        .map(|field| reward_track_bool_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(false));
    let random_weight = reward_track_u32_expr(&row, group.random_weight)?;
    let budget_contribution = reward_track_u32_expr(&row, group.budget_contribution)?;
    let stage_exclusion = group
        .stage_exclusion
        .map(|field| reward_track_crc_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(None));
    let shop_exclusion = group
        .shop_exclusion
        .map(|field| reward_track_crc_expr(&row, field))
        .transpose()?
        .unwrap_or_else(|| quote!(None));
    let index = LitInt::new(&group.index.to_string(), Span::call_site());

    Ok(quote! {
        {
            if let Some(bucket_id) = #bucket_id {
                let mut entries = Vec::new();
                for (zero_based, row) in self.rows().enumerate() {
                    let Some(reward_id) = #reward_id else {
                        continue;
                    };
                    let source_row = reward_track_store_row_index(zero_based)?;
                    entries.push(PvPStoreRewardTrackEntry {
                        source_row,
                        reward_id,
                        reward_type: #reward_type,
                        tag_constraints: #tag_constraints,
                        match_one: #match_one,
                        random_weight: #random_weight,
                        budget_contribution: #budget_contribution,
                        stage_exclusion: #stage_exclusion,
                        shop_exclusion: #shop_exclusion,
                    });
                }
                slots.push(PvPStoreRewardTrackSlot {
                    index: #index,
                    bucket_id,
                    entries,
                });
            }
        }
    })
}

fn reward_track_text_option_expr(
    row: &Ident,
    entry: RewardTrackRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(reward_track_store_text(Some(#row.#field()?))))
            } else {
                Ok(quote!(reward_track_store_text(#row.#field())))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be text, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn reward_track_foreign_key_expr(
    row: &Ident,
    entry: RewardTrackRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::ForeignKey) => {
            if entry.column.required {
                Ok(quote!(Some(#row.#field()?)))
            } else {
                Ok(quote!(#row.#field()))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be a RewardTrackItems foreign key, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn reward_track_crc_expr(row: &Ident, entry: RewardTrackRenderField<'_>) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(reward_track_store_crc_from_text(Some(#row.#field()?))))
            } else {
                Ok(quote!(reward_track_store_crc_from_text(#row.#field())))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::Crc32) => {
            if entry.column.required {
                Ok(quote!(Some(#row.#field()?)))
            } else {
                Ok(quote!(#row.#field()))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be text or Crc32, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn reward_track_tag_constraints_expr(
    row: &Ident,
    entry: RewardTrackRenderField<'_>,
) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            if entry.column.required {
                Ok(quote!(reward_track_store_tag_constraints(Some(#row.#field()?))?))
            } else {
                Ok(quote!(reward_track_store_tag_constraints(#row.#field())?))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be reward-tag text, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn reward_track_bool_expr(row: &Ident, entry: RewardTrackRenderField<'_>) -> Result<TokenStream> {
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
                Ok(quote!(reward_track_store_bool_from_text(Some(#row.#field()?))?))
            } else {
                Ok(quote!(reward_track_store_bool_from_text(#row.#field())?))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be bool or text bool, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

fn reward_track_u32_expr(row: &Ident, entry: RewardTrackRenderField<'_>) -> Result<TokenStream> {
    let field = ident(&entry.field.rust_name);
    match table_code_cell_type(entry.column, entry.field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::U32) => {
            if entry.column.required {
                Ok(quote!(#row.#field()?))
            } else {
                Ok(quote!(#row.#field().unwrap_or_default()))
            }
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::NonZeroU32) => {
            if entry.column.required {
                Ok(quote!(#row.#field()?.get()))
            } else {
                Ok(quote!(#row.#field().map(core::num::NonZeroU32::get).unwrap_or_default()))
            }
        }
        cell_type => bail!(
            "PvPStoreData column {} must be u32 weight/budget, found {:?}",
            entry.column.name,
            cell_type
        ),
    }
}

pub(super) fn reward_track_projection_helpers_tokens() -> TokenStream {
    quote! {
        fn reward_track_store_row_index(
            zero_based: usize,
        ) -> Result<gamedata::RowIndex, gamedata::GameDataError> {
            let one_based = zero_based
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    gamedata::GameDataError::Decode(
                        "PvPStoreData row index exceeded u32 range".into(),
                    )
                })?;
            gamedata::RowIndex::from_one_based(one_based).ok_or_else(|| {
                gamedata::GameDataError::Decode(
                    "PvPStoreData produced invalid row index 0".into(),
                )
            })
        }

        fn reward_track_store_text(value: Option<&str>) -> Option<&str> {
            value
                .map(str::trim)
                .filter(|value| !value.is_empty())
        }

        fn reward_track_store_crc_from_text(value: Option<&str>) -> Option<az_core::crc::Crc32> {
            let value = reward_track_store_text(value)?;
            let crc = az_core::crc::Crc32::from_str_lower(value);
            (crc != az_core::crc::Crc32::ZERO).then_some(crc)
        }

        fn reward_track_store_bool_from_text(
            value: Option<&str>,
        ) -> Result<bool, gamedata::GameDataError> {
            let Some(value) = reward_track_store_text(value) else {
                return Ok(false);
            };
            match value.to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(gamedata::GameDataError::Decode(format!(
                    "PvPStoreData bool field contains `{value}`"
                ))),
            }
        }

        fn reward_track_store_tag_constraints(
            value: Option<&str>,
        ) -> Result<Vec<PvPStoreRewardTrackTagConstraint>, gamedata::GameDataError> {
            let Some(value) = reward_track_store_text(value) else {
                return Ok(Vec::new());
            };

            let mut constraints = Vec::new();
            for token in value.split(',').map(str::trim).filter(|token| !token.is_empty()) {
                let (tag, range) = match token.split_once(':') {
                    Some((tag, range)) => {
                        if range.contains(':') {
                            return Err(gamedata::GameDataError::Decode(format!(
                                "PvPStoreData reward-track tag `{token}` has too many `:` separators"
                            )));
                        }
                        let Some(tag) = reward_track_store_text(Some(tag)) else {
                            return Err(gamedata::GameDataError::Decode(
                                "PvPStoreData reward-track tag has an empty tag id".into(),
                            ));
                        };
                        (tag, reward_track_store_tag_range(range)?)
                    }
                    None => (token, 0..=0),
                };
                let tag = az_core::crc::Crc32::from_str_lower(tag);
                if tag != az_core::crc::Crc32::ZERO {
                    constraints.push(PvPStoreRewardTrackTagConstraint { tag, range });
                }
            }
            Ok(constraints)
        }

        fn reward_track_store_tag_range(
            value: &str,
        ) -> Result<core::ops::RangeInclusive<u16>, gamedata::GameDataError> {
            let value = value.trim();
            if value.is_empty() {
                return Err(gamedata::GameDataError::Decode(
                    "PvPStoreData reward-track tag range is empty".into(),
                ));
            }

            if let Some((left, right)) = value.split_once('-') {
                if right.contains('-') {
                    return Err(gamedata::GameDataError::Decode(format!(
                        "PvPStoreData reward-track tag range `{value}` has too many `-` separators"
                    )));
                }
                let left = reward_track_store_tag_range_bound(left)?;
                let right = reward_track_store_tag_range_bound(right)?;
                Ok(left.min(right)..=left.max(right))
            } else {
                let left = reward_track_store_tag_range_bound(value)?;
                Ok(left..=10_000)
            }
        }

        fn reward_track_store_tag_range_bound(value: &str) -> Result<u16, gamedata::GameDataError> {
            let value = value.trim();
            value.parse::<u16>().map_err(|error| {
                gamedata::GameDataError::Decode(format!(
                    "PvPStoreData reward-track tag range bound `{value}` is not u16: {error}"
                ))
            })
        }
    }
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}
