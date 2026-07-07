use anyhow::{Context, Result};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitBool, LitInt, LitStr, Path, Type};

#[cfg(test)]
use super::cell_types::borrowed_cell_type_for_column;
use super::cell_types::{borrowed_cell_type_for_column_in_context, foreign_key_column_type};
use super::enum_render::{
    render_table_code_enum_tokens, table_code_column_enum_shape, table_code_enum_shapes,
    table_code_enum_type_name,
};
use super::loot_bucket_projection::{
    loot_bucket_column_groups, loot_bucket_columns_method_tokens,
    loot_bucket_projection_helpers_tokens, loot_bucket_projection_types_tokens,
};
use super::model::{RustField, TableCodeColumnIndex};
#[cfg(test)]
use super::perk_bucket_projection::{
    perk_bucket_column_groups, perk_bucket_projection_helpers_tokens,
    perk_bucket_projection_method_tokens, perk_bucket_projection_types_tokens,
};
use super::reward_track_projection::{
    reward_track_column_groups, reward_track_projection_helpers_tokens,
    reward_track_projection_method_tokens, reward_track_projection_types_tokens,
};
use super::{
    resolved_rust_fields_for_schema, row_key_field, table_key_type_for_column, table_marker_name,
};
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemEnumRepresentation,
    GameSystemListElementShape, GameSystemNumberShape, GameSystemTableSchema,
};
use crate::naming::to_upper_camel_ident;
use crate::rust::source::format_rust_tokens;

const DETAILED_TABLE_VIEW_COLUMN_LIMIT: usize = 128;
const SCHEMA_COLUMN_MARKER_CHUNK_SIZE: usize = 96;

#[derive(Debug)]
pub(super) struct RenderedTableSchemaCode {
    pub(super) root_rs: String,
    pub(super) chunks: Vec<RenderedTableSchemaChunk>,
}

#[derive(Debug)]
pub(super) struct RenderedTableSchemaChunk {
    pub(super) module_name: String,
    pub(super) rust_rs: String,
}

#[cfg(test)]
pub(super) fn render_table_code_rs(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
    _source_path: &str,
) -> Result<String> {
    format_rust_tokens(render_table_code_tokens(schema, table_code_columns)?).map_err(Into::into)
}

pub(super) fn render_table_schema_code_files(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
) -> Result<RenderedTableSchemaCode> {
    let rendered = render_table_schema_code_tokens(schema, table_code_columns)?;
    let root_rs = format_rust_tokens(rendered.root).map_err(anyhow::Error::from)?;
    let chunks = rendered
        .chunks
        .into_iter()
        .map(|chunk| {
            let rust_rs = format_rust_tokens(chunk.tokens).map_err(anyhow::Error::from)?;
            Ok(RenderedTableSchemaChunk {
                module_name: chunk.module_name,
                rust_rs,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RenderedTableSchemaCode { root_rs, chunks })
}

pub(super) fn render_table_shell_code_rs(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
    schema_module_name: &str,
) -> Result<String> {
    format_rust_tokens(render_table_shell_code_tokens(
        schema,
        table_code_columns,
        schema_module_name,
    )?)
    .map_err(Into::into)
}

#[cfg(test)]
fn render_table_code_tokens(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
) -> Result<TokenStream> {
    let root = path("super::super")?;
    let table_marker = table_marker_name(schema);
    let table_marker_ident = ident(&table_marker);
    let row_struct_ident = ident(&format!(
        "{}Row",
        to_upper_camel_ident(&schema.table_name, "Table")
    ));
    let view_struct_ident = ident(&format!("{table_marker}View"));
    let row_ref_struct_ident = ident(&format!("{table_marker}RowRef"));
    let row_name = lit_str(&schema.row_type_name);
    let table_name = lit_str(&schema.table_name);
    let rust_fields = resolved_rust_fields_for_schema(schema, table_code_columns);
    let row_key_field = row_key_field(schema, &rust_fields)?;
    let enum_tokens = table_code_enum_shapes(schema)?
        .iter()
        .map(|enum_shape| render_table_code_enum_tokens(schema, enum_shape))
        .collect::<Result<Vec<_>>>()?;
    let column_markers = schema
        .columns
        .iter()
        .zip(rust_fields.iter())
        .map(|(column, field)| column_marker_tokens(&root, &table_marker_ident, column, field))
        .collect::<Result<Vec<_>>>()?;
    let column_marker_idents = rust_fields
        .iter()
        .map(|field| ident(&field.rust_column_marker))
        .collect::<Vec<_>>();
    let typed_view = typed_view_tokens(
        schema,
        &table_marker_ident,
        &view_struct_ident,
        &row_ref_struct_ident,
        &rust_fields,
        row_key_field,
    )?;

    Ok(quote! {
        #![allow(dead_code)]
        #![allow(clippy::struct_excessive_bools)]

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct #row_struct_ident;

        impl gamedata::Row for #row_struct_ident {
            const NAME: &'static str = #row_name;
            const CRC: u32 = az_core::crc::Crc32::from_str_lower(Self::NAME).value();
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct #table_marker_ident;

        impl gamedata::Table for #table_marker_ident {
            type Row = #row_struct_ident;

            const NAME: &'static str = #table_name;
            const CRC: u32 = az_core::crc::Crc32::from_str_lower(Self::NAME).value();
        }

        pub const TABLE_REQUIREMENT: gamedata::TableRequirement =
            gamedata::TableRequirement::typed::<#table_marker_ident>();

        #(#enum_tokens)*

        #(#column_markers)*

        pub const TABLE: #root::TableMeta<#table_marker_ident> =
            #root::TableMeta::of(COLUMNS);

        pub const COLUMNS: &[#root::ColumnMeta<#table_marker_ident>] = &[
            #(#root::ColumnMeta::<#table_marker_ident>::of::<#column_marker_idents>(),)*
        ];

        #typed_view
    })
}

struct RenderedTableSchemaTokens {
    root: TokenStream,
    chunks: Vec<RenderedTableSchemaChunkTokens>,
}

struct RenderedTableSchemaChunkTokens {
    module_name: String,
    tokens: TokenStream,
}

fn render_table_schema_code_tokens(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
) -> Result<RenderedTableSchemaTokens> {
    let root_path = "super::super";
    let root = path(root_path)?;
    let row_struct_ident = ident(&format!(
        "{}Row",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let view_struct_ident = ident(&format!(
        "{}View",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let row_ref_struct_ident = ident(&format!(
        "{}RowRef",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let row_name = lit_str(&schema.row_type_name);
    let row_schema_name = lit_str(&format!("newworld.gamedata.{}", schema.row_type_name));
    let rust_fields = resolved_rust_fields_for_schema(schema, table_code_columns);
    let row_key_field = row_key_field(schema, &rust_fields)?;
    let row_fields = schema
        .columns
        .iter()
        .zip(rust_fields.iter())
        .map(authored_row_field_tokens)
        .collect::<Result<Vec<_>>>()?;
    let row_authoring_impl =
        authored_row_game_data_impl_tokens(schema, &row_struct_ident, row_key_field)?;
    let enum_tokens = table_code_enum_shapes(schema)?
        .iter()
        .map(|enum_shape| render_table_code_enum_tokens(schema, enum_shape))
        .collect::<Result<Vec<_>>>()?;
    let split_columns = schema.columns.len() > DETAILED_TABLE_VIEW_COLUMN_LIMIT;
    let column_markers = if split_columns {
        Vec::new()
    } else {
        schema
            .columns
            .iter()
            .zip(rust_fields.iter())
            .map(|(column, field)| {
                generic_column_marker_tokens(
                    &root,
                    root_path,
                    quote!(#row_struct_ident),
                    None,
                    column,
                    field,
                )
            })
            .collect::<Result<Vec<_>>>()?
    };
    let column_chunk_modules = if split_columns {
        schema_column_chunk_names(schema.columns.len())
    } else {
        Vec::new()
    };
    let column_chunk_decls = column_chunk_modules
        .iter()
        .map(|module_name| {
            let module = ident(module_name);
            quote! {
                mod #module;
                pub use #module::*;
            }
        })
        .collect::<Vec<_>>();
    let typed_view = generic_typed_view_tokens(
        schema,
        &row_struct_ident,
        &view_struct_ident,
        &row_ref_struct_ident,
        &rust_fields,
        row_key_field,
        emits_detailed_table_view(schema),
    )?;
    let chunks = if split_columns {
        render_schema_column_chunk_tokens(schema, &rust_fields, &row_struct_ident)?
    } else {
        Vec::new()
    };

    let root = quote! {
        #![allow(dead_code)]
        #![allow(clippy::struct_excessive_bools)]

        #[derive(Debug, Clone, PartialEq, gamedata::authoring::az_schema::AzSchema)]
        #[schema(name = #row_schema_name, version = 1)]
        pub struct #row_struct_ident {
            #(#row_fields)*
        }

        impl gamedata::Row for #row_struct_ident {
            const NAME: &'static str = #row_name;
            const CRC: u32 = az_core::crc::Crc32::from_str_lower(Self::NAME).value();
        }

        #row_authoring_impl

        #(#enum_tokens)*

        #(#column_chunk_decls)*
        #(#column_markers)*

        #typed_view
    };

    Ok(RenderedTableSchemaTokens { root, chunks })
}

fn authored_row_field_tokens(
    (column, field): (&GameSystemColumnSchema, &RustField),
) -> Result<TokenStream> {
    let field_ident = ident(&field.rust_name);
    let field_ty = authored_row_field_type(column)?;
    let field_id = LitInt::new(
        &schema_field_id(column, field).to_string(),
        Span::call_site(),
    );
    let color_editor = matches!(
        &column.value_shape,
        GameSystemColumnValueShape::Color { .. }
    )
    .then(|| quote!(#[editor(color)]))
    .unwrap_or_default();

    Ok(quote! {
        #[schema(id = #field_id)]
        #color_editor
        pub #field_ident: #field_ty,
    })
}

fn authored_row_game_data_impl_tokens(
    schema: &GameSystemTableSchema,
    row_struct_ident: &Ident,
    row_key_field: Option<&RustField>,
) -> Result<TokenStream> {
    let Some(row_key_field) = row_key_field else {
        return Ok(TokenStream::new());
    };
    let row_key_column = schema
        .columns
        .iter()
        .find(|column| column.row_key)
        .context("row key field came from schema columns")?;
    let row_key_ident = ident(&row_key_field.rust_name);
    let row_key_ty = authored_row_value_type(row_key_column)?;
    let row_key_field_id = LitInt::new(
        &schema_field_id(row_key_column, row_key_field).to_string(),
        Span::call_site(),
    );

    Ok(quote! {
        impl gamedata::authoring::GameDataRow for #row_struct_ident {
            type PrimaryKeyValue = #row_key_ty;

            const PRIMARY_KEY_FIELD: gamedata::authoring::az_schema::FieldId =
                gamedata::authoring::az_schema::FieldId(#row_key_field_id);
            const PRIMARY_KEY_FIELD_NAME: &'static str = stringify!(#row_key_ident);

            #[inline]
            fn primary_key(&self) -> &Self::PrimaryKeyValue {
                &self.#row_key_ident
            }
        }
    })
}

fn authored_row_field_type(column: &GameSystemColumnSchema) -> Result<Type> {
    let value_ty = authored_row_value_type(column)?;
    if column.required || column.row_key {
        return Ok(value_ty);
    }
    ty(&format!("Option<{}>", quote!(#value_ty)))
}

fn authored_row_value_type(column: &GameSystemColumnSchema) -> Result<Type> {
    let type_name = match &column.value_shape {
        GameSystemColumnValueShape::Boolean => "bool".to_owned(),
        GameSystemColumnValueShape::Color { .. } => "glam::Vec4".to_owned(),
        GameSystemColumnValueShape::Crc32 => "u32".to_owned(),
        GameSystemColumnValueShape::Enum { enum_shape } => {
            authored_enum_representation_type(enum_shape.representation).to_owned()
        }
        GameSystemColumnValueShape::Number { number_shape } => {
            authored_number_type(*number_shape).to_owned()
        }
        GameSystemColumnValueShape::Range { .. } => "String".to_owned(),
        GameSystemColumnValueShape::String {
            list: Some(list), ..
        } if !column.row_key => {
            let element = list
                .element_shape
                .as_ref()
                .map(authored_list_element_type)
                .transpose()?
                .unwrap_or_else(|| "String".to_owned());
            format!("Vec<{element}>")
        }
        GameSystemColumnValueShape::String { .. } => "String".to_owned(),
    };
    ty(&type_name)
}

fn authored_list_element_type(element_shape: &GameSystemListElementShape) -> Result<String> {
    Ok(match element_shape {
        GameSystemListElementShape::Boolean => "bool".to_owned(),
        GameSystemListElementShape::Color { .. } => "glam::Vec4".to_owned(),
        GameSystemListElementShape::Number { number_shape } => {
            authored_number_type(*number_shape).to_owned()
        }
        GameSystemListElementShape::Crc32 => "u32".to_owned(),
        GameSystemListElementShape::Enum { enum_shape } => {
            authored_enum_representation_type(enum_shape.representation).to_owned()
        }
        GameSystemListElementShape::Range { .. }
        | GameSystemListElementShape::Pair { .. }
        | GameSystemListElementShape::String => "String".to_owned(),
    })
}

fn authored_number_type(number_shape: GameSystemNumberShape) -> &'static str {
    match number_shape {
        GameSystemNumberShape::Float => "f32",
        GameSystemNumberShape::Integer => "i32",
        GameSystemNumberShape::NonNegativeInteger | GameSystemNumberShape::PositiveInteger => "u32",
        GameSystemNumberShape::U8 | GameSystemNumberShape::NonZeroU8 => "u8",
        GameSystemNumberShape::U16 | GameSystemNumberShape::NonZeroU16 => "u16",
    }
}

fn authored_enum_representation_type(representation: GameSystemEnumRepresentation) -> &'static str {
    match representation {
        GameSystemEnumRepresentation::U8 => "u8",
        GameSystemEnumRepresentation::I32 => "i32",
        GameSystemEnumRepresentation::U32 | GameSystemEnumRepresentation::Crc32 => "u32",
    }
}

fn schema_field_id(column: &GameSystemColumnSchema, field: &RustField) -> u32 {
    let id = if column.crc == 0 {
        az_core::crc::Crc32::from_str_lower(&field.rust_name).value()
    } else {
        column.crc
    };
    if id == 0 { 1 } else { id }
}

fn render_table_shell_code_tokens(
    schema: &GameSystemTableSchema,
    table_code_columns: &TableCodeColumnIndex,
    schema_module_name: &str,
) -> Result<TokenStream> {
    let root = path("super::super")?;
    let schema_module = ident(schema_module_name);
    let table_marker = table_marker_name(schema);
    let table_marker_ident = ident(&table_marker);
    let table_row_alias_ident = ident(&format!(
        "{}Row",
        to_upper_camel_ident(&schema.table_name, "Table")
    ));
    let schema_row_ident = ident(&format!(
        "{}Row",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let schema_view_ident = ident(&format!(
        "{}View",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let table_view_alias_ident = ident(&format!("{table_marker}View"));
    let schema_row_ref_ident = ident(&format!(
        "{}RowRef",
        to_upper_camel_ident(&schema.row_type_name, "GameData")
    ));
    let table_row_ref_alias_ident = ident(&format!("{table_marker}RowRef"));
    let table_name = lit_str(&schema.table_name);
    let rust_fields = resolved_rust_fields_for_schema(schema, table_code_columns);
    let detailed_view = emits_detailed_table_view(schema);
    let column_meta_alias = ident("TableColumnMeta");
    let column_aliases = schema
        .columns
        .iter()
        .zip(rust_fields.iter())
        .filter(|(column, _)| detailed_view || column.row_key)
        .map(|(_, field)| {
            let column = ident(&field.rust_column_marker);
            quote! {
                pub type #column = super::#schema_module::#column<#table_marker_ident>;
            }
        })
        .collect::<Vec<_>>();
    let enum_reexports = table_code_enum_shapes(schema)?
        .iter()
        .map(|enum_shape| {
            let enum_ident = ident(&table_code_enum_type_name(enum_shape));
            quote! {
                pub use super::#schema_module::#enum_ident;
            }
        })
        .collect::<Vec<_>>();
    let projection_reexports =
        projection_reexports_tokens(&schema.row_type_name, &schema_module, detailed_view);
    let column_meta_entries = rust_fields
        .iter()
        .map(|field| {
            let column = ident(&field.rust_column_marker);
            if detailed_view {
                quote! {
                    #column_meta_alias::of::<#column>()
                }
            } else {
                quote! {
                    #column_meta_alias::of::<super::#schema_module::#column<#table_marker_ident>>()
                }
            }
        })
        .collect::<Vec<_>>();
    let row_key_alias = row_key_field(schema, &rust_fields)?
        .map(|field| {
            let key_alias = ident(&format!("{table_marker}Key"));
            let row_key_column = ident(&field.rust_column_marker);
            let row_key_column_schema = schema
                .columns
                .iter()
                .find(|column| column.row_key)
                .context("row key field came from schema columns")?;
            let key_type = ty(&table_key_type_for_column(
                row_key_column_schema,
                field,
                "'_",
            )?)?;
            Ok::<TokenStream, anyhow::Error>(quote! {
                pub type #key_alias =
                    gamedata::TableKey<super::#schema_module::#row_key_column<#table_marker_ident>>;

                impl #table_marker_ident {
                    #[must_use]
                    pub fn key(key: #key_type) -> #key_alias {
                        gamedata::TableKey::<
                            super::#schema_module::#row_key_column<#table_marker_ident>
                        >::new(key)
                    }
                }
            })
        })
        .transpose()?
        .unwrap_or_default();

    Ok(quote! {
        #![allow(dead_code)]

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct #table_marker_ident;

        impl gamedata::Table for #table_marker_ident {
            type Row = super::#schema_module::#schema_row_ident;

            const NAME: &'static str = #table_name;
            const CRC: u32 = az_core::crc::Crc32::from_str_lower(Self::NAME).value();
        }

        pub const TABLE_REQUIREMENT: gamedata::TableRequirement =
            gamedata::TableRequirement::typed::<#table_marker_ident>();

        pub type #table_row_alias_ident = super::#schema_module::#schema_row_ident;
        pub type #table_view_alias_ident<'a> =
            super::#schema_module::#schema_view_ident<'a, #table_marker_ident>;
        pub type #table_row_ref_alias_ident<'t, 'a> =
            super::#schema_module::#schema_row_ref_ident<'t, 'a, #table_marker_ident>;
        type #column_meta_alias = #root::ColumnMeta<#table_marker_ident>;

        #(#enum_reexports)*
        #projection_reexports
        #(#column_aliases)*

        pub const TABLE: #root::TableMeta<#table_marker_ident> =
            #root::TableMeta::of(COLUMNS);

        pub const COLUMNS: &[#column_meta_alias] = &[
            #(#column_meta_entries,)*
        ];

        #row_key_alias
    })
}

fn emits_detailed_table_view(schema: &GameSystemTableSchema) -> bool {
    schema.columns.len() <= DETAILED_TABLE_VIEW_COLUMN_LIMIT
}

fn schema_column_chunk_names(column_count: usize) -> Vec<String> {
    (0..column_count.div_ceil(SCHEMA_COLUMN_MARKER_CHUNK_SIZE))
        .map(|index| format!("columns_{index:03}"))
        .collect()
}

fn render_schema_column_chunk_tokens(
    schema: &GameSystemTableSchema,
    rust_fields: &[RustField],
    row_struct_ident: &Ident,
) -> Result<Vec<RenderedTableSchemaChunkTokens>> {
    let root_path = "super::super::super";
    let root = path(root_path)?;
    let row_struct = quote!(super::#row_struct_ident);

    schema
        .columns
        .chunks(SCHEMA_COLUMN_MARKER_CHUNK_SIZE)
        .zip(rust_fields.chunks(SCHEMA_COLUMN_MARKER_CHUNK_SIZE))
        .enumerate()
        .map(|(index, (columns, fields))| {
            let column_markers = columns
                .iter()
                .zip(fields.iter())
                .map(|(column, field)| {
                    generic_column_marker_tokens(
                        &root,
                        root_path,
                        row_struct.clone(),
                        Some("super"),
                        column,
                        field,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(RenderedTableSchemaChunkTokens {
                module_name: format!("columns_{index:03}"),
                tokens: quote! {
                    #![allow(dead_code)]

                    #(#column_markers)*
                },
            })
        })
        .collect()
}

fn projection_reexports_tokens(
    row_type_name: &str,
    schema_module: &Ident,
    detailed_view: bool,
) -> TokenStream {
    if !detailed_view {
        return TokenStream::new();
    }

    match row_type_name {
        "LootBucketData" => quote! {
            pub use super::#schema_module::{
                LootBucketColumn, LootBucketColumnEntry, LootBucketColumnTag,
            };
        },
        "PvPStoreData" => quote! {
            pub use super::#schema_module::{
                PvPStoreRewardTrackEntry, PvPStoreRewardTrackSlot,
                PvPStoreRewardTrackTagConstraint,
            };
        },
        _ => TokenStream::new(),
    }
}

#[cfg(test)]
fn typed_view_tokens(
    schema: &GameSystemTableSchema,
    table_marker: &Ident,
    view_struct: &Ident,
    row_ref_struct: &Ident,
    rust_fields: &[RustField],
    row_key_field: Option<&RustField>,
) -> Result<TokenStream> {
    let row_key_tokens = row_key_field
        .map(|field| row_key_tokens(schema, table_marker, field))
        .transpose()?
        .unwrap_or_default();
    let loot_bucket_groups = loot_bucket_column_groups(schema, rust_fields);
    let loot_bucket_types = (!loot_bucket_groups.is_empty())
        .then(loot_bucket_projection_types_tokens)
        .unwrap_or_default();
    let loot_bucket_method = if loot_bucket_groups.is_empty() {
        TokenStream::new()
    } else {
        loot_bucket_columns_method_tokens(&loot_bucket_groups)?
    };
    let loot_bucket_helpers = (!loot_bucket_groups.is_empty())
        .then(loot_bucket_projection_helpers_tokens)
        .unwrap_or_default();
    let perk_bucket_groups = perk_bucket_column_groups(schema, rust_fields);
    let perk_bucket_types = if perk_bucket_groups.is_empty() {
        TokenStream::new()
    } else {
        perk_bucket_projection_types_tokens(table_marker)
    };
    let perk_bucket_method = if perk_bucket_groups.is_empty() {
        TokenStream::new()
    } else {
        perk_bucket_projection_method_tokens(table_marker, &perk_bucket_groups)?
    };
    let perk_bucket_helpers = if perk_bucket_groups.is_empty() {
        TokenStream::new()
    } else {
        perk_bucket_projection_helpers_tokens(table_marker, view_struct)
    };
    let reward_track_groups = reward_track_column_groups(schema, rust_fields);
    let reward_track_types = (!reward_track_groups.is_empty())
        .then(reward_track_projection_types_tokens)
        .unwrap_or_default();
    let reward_track_method = if reward_track_groups.is_empty() {
        TokenStream::new()
    } else {
        reward_track_projection_method_tokens(&reward_track_groups)?
    };
    let reward_track_helpers = (!reward_track_groups.is_empty())
        .then(reward_track_projection_helpers_tokens)
        .unwrap_or_default();
    let key_alias = row_key_field.map(|_| ident(&format!("{table_marker}Key")));
    let view_fields = rust_fields
        .iter()
        .map(|field| {
            let slot = ident(&field.rust_column_name);
            let marker = ident(&field.rust_column_marker);
            quote!(#slot: gamedata::game_system::ColumnSlot<#marker>,)
        })
        .collect::<Vec<_>>();
    let slot_lets = rust_fields
        .iter()
        .map(|field| {
            let slot = ident(&field.rust_column_name);
            let marker = ident(&field.rust_column_marker);
            quote!(let #slot = column_slot::<#marker>(table)?;)
        })
        .collect::<Vec<_>>();
    let slot_idents = rust_fields
        .iter()
        .map(|field| ident(&field.rust_column_name))
        .collect::<Vec<_>>();
    let inherent_get = row_key_field
        .map(|_| {
            let key_alias = key_alias.clone().expect("row key field has key alias");
            quote! {
                #[must_use]
                pub fn get(&self, key: &#key_alias) -> Option<#row_ref_struct<'_, 'a>> {
                    let row = self.table.get(key)?;
                    Some(#row_ref_struct { view: self, row })
                }
            }
        })
        .unwrap_or_default();
    let keyed_table_view_impl = row_key_field
        .map(|field| {
            keyed_table_view_tokens(
                table_marker,
                view_struct,
                row_ref_struct,
                key_alias.clone().expect("row key field has key alias"),
                field,
            )
        })
        .unwrap_or_default();
    let row_getters = schema
        .columns
        .iter()
        .zip(rust_fields.iter())
        .map(|(column, field)| row_getter_tokens(column, field))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #row_key_tokens

        #loot_bucket_types
        #perk_bucket_types
        #reward_track_types

        #[derive(Debug)]
        pub struct #view_struct<'a> {
            table: gamedata::game_system::TableRef<'a, #table_marker>,
            #(#view_fields)*
        }

        impl<'a> #view_struct<'a> {
            fn from_table(
                table: gamedata::game_system::TableRef<'a, #table_marker>,
            ) -> Result<Self, gamedata::GameDataError> {
                #(#slot_lets)*
                Ok(Self {
                    table,
                    #(#slot_idents,)*
                })
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.table.len()
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.table.is_empty()
            }

            #[must_use]
            pub fn row_at(
                &self,
                row: gamedata::RowIndex,
            ) -> Option<#row_ref_struct<'_, 'a>> {
                self.table
                    .row_at(row)
                    .map(|row| #row_ref_struct { view: self, row })
            }

            #inherent_get

            pub fn rows(&self) -> impl Iterator<Item = #row_ref_struct<'_, 'a>> + '_ {
                self.table
                    .rows()
                    .map(|row| #row_ref_struct { view: self, row })
            }

            #loot_bucket_method
            #perk_bucket_method
            #reward_track_method
        }

        impl<'a> gamedata::game_system::SystemView<'a> for #view_struct<'a> {
            const NAME: &'static str = <#table_marker as gamedata::Table>::NAME;
            const TABLES: &'static [gamedata::TableRequirement] =
                &[TABLE_REQUIREMENT];

            fn from_system(
                system: &'a gamedata::game_system::System,
                token: gamedata::game_system::SystemViewToken,
            ) -> Result<Self, gamedata::GameDataError> {
                Self::from_table(system.table::<#table_marker>(token)?)
            }
        }

        #keyed_table_view_impl

        #[derive(Debug, Clone, Copy)]
        pub struct #row_ref_struct<'t, 'a> {
            view: &'t #view_struct<'a>,
            row: gamedata::game_system::RowRef<'a, #table_marker>,
        }

        impl<'t, 'a> #row_ref_struct<'t, 'a> {
            #(#row_getters)*
        }

        fn column_slot<C: gamedata::TableColumn>(
            table: gamedata::game_system::TableRef<'_, C::Table>,
        ) -> Result<gamedata::game_system::ColumnSlot<C>, gamedata::GameDataError> {
            table.require_column::<C>().map_err(|error| {
                gamedata::GameDataError::Decode(format!(
                    "table `{}` required field `{}` column `{}`: {error}",
                    <C::Table as gamedata::Table>::NAME,
                    C::FIELD_NAME,
                    C::COLUMN,
                ))
            })
        }

        #loot_bucket_helpers
        #perk_bucket_helpers
        #reward_track_helpers
    })
}

fn generic_typed_view_tokens(
    schema: &GameSystemTableSchema,
    row_struct: &Ident,
    view_struct: &Ident,
    row_ref_struct: &Ident,
    rust_fields: &[RustField],
    row_key_field: Option<&RustField>,
    detailed_accessors: bool,
) -> Result<TokenStream> {
    let table_marker = ident("T");
    let table_bound = quote!(#table_marker: gamedata::Table<Row = #row_struct>);
    let key_alias = row_key_field.map(|_| ident(&format!("{view_struct}Key")));
    let row_key_tokens = row_key_field
        .map(|field| {
            generic_row_key_tokens(
                schema,
                &table_marker,
                row_struct,
                key_alias.as_ref().expect("row key field has key alias"),
                field,
            )
        })
        .transpose()?
        .unwrap_or_default();
    let loot_bucket_groups = detailed_accessors
        .then(|| loot_bucket_column_groups(schema, rust_fields))
        .unwrap_or_default();
    let loot_bucket_types = (!loot_bucket_groups.is_empty())
        .then(loot_bucket_projection_types_tokens)
        .unwrap_or_default();
    let loot_bucket_method = if loot_bucket_groups.is_empty() {
        TokenStream::new()
    } else {
        loot_bucket_columns_method_tokens(&loot_bucket_groups)?
    };
    let loot_bucket_helpers = (!loot_bucket_groups.is_empty())
        .then(loot_bucket_projection_helpers_tokens)
        .unwrap_or_default();
    let perk_bucket_types = TokenStream::new();
    let perk_bucket_method = TokenStream::new();
    let perk_bucket_helpers = TokenStream::new();
    let reward_track_groups = detailed_accessors
        .then(|| reward_track_column_groups(schema, rust_fields))
        .unwrap_or_default();
    let reward_track_types = (!reward_track_groups.is_empty())
        .then(reward_track_projection_types_tokens)
        .unwrap_or_default();
    let reward_track_method = if reward_track_groups.is_empty() {
        TokenStream::new()
    } else {
        reward_track_projection_method_tokens(&reward_track_groups)?
    };
    let reward_track_helpers = (!reward_track_groups.is_empty())
        .then(reward_track_projection_helpers_tokens)
        .unwrap_or_default();
    let detailed_fields = detailed_accessors
        .then_some(rust_fields)
        .unwrap_or_default();
    let view_fields = detailed_fields
        .iter()
        .map(|field| {
            let slot = ident(&field.rust_column_name);
            let marker = ident(&field.rust_column_marker);
            quote!(#slot: gamedata::game_system::ColumnSlot<#marker<#table_marker>>,)
        })
        .collect::<Vec<_>>();
    let slot_lets = detailed_fields
        .iter()
        .map(|field| {
            let slot = ident(&field.rust_column_name);
            let marker = ident(&field.rust_column_marker);
            quote!(let #slot = column_slot::<#marker<#table_marker>>(table)?;)
        })
        .collect::<Vec<_>>();
    let slot_idents = detailed_fields
        .iter()
        .map(|field| ident(&field.rust_column_name))
        .collect::<Vec<_>>();
    let inherent_get = row_key_field
        .map(|_| {
            let key_alias = key_alias.clone().expect("row key field has key alias");
            quote! {
                #[must_use]
                pub fn get(&self, key: &#key_alias<#table_marker>) -> Option<#row_ref_struct<'_, 'a, #table_marker>> {
                    let row = self.table.get(key)?;
                    Some(#row_ref_struct { view: self, row })
                }
            }
        })
        .unwrap_or_default();
    let keyed_table_view_impl = row_key_field
        .map(|field| {
            generic_keyed_table_view_tokens(
                &table_marker,
                row_struct,
                view_struct,
                row_ref_struct,
                key_alias.clone().expect("row key field has key alias"),
                field,
            )
        })
        .unwrap_or_default();
    let row_getters = if detailed_accessors {
        schema
            .columns
            .iter()
            .zip(rust_fields.iter())
            .map(|(column, field)| generic_row_getter_tokens(&table_marker, column, field))
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let generic_cell_accessors = generic_cell_accessor_tokens(&table_marker);

    Ok(quote! {
        #row_key_tokens

        #loot_bucket_types
        #perk_bucket_types
        #reward_track_types

        #[derive(Debug)]
        pub struct #view_struct<'a, #table_marker>
        where
            #table_bound,
        {
            table: gamedata::game_system::TableRef<'a, #table_marker>,
            #(#view_fields)*
        }

        impl<'a, #table_marker> #view_struct<'a, #table_marker>
        where
            #table_bound,
        {
            fn from_table(
                table: gamedata::game_system::TableRef<'a, #table_marker>,
            ) -> Result<Self, gamedata::GameDataError> {
                #(#slot_lets)*
                Ok(Self {
                    table,
                    #(#slot_idents,)*
                })
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.table.len()
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.table.is_empty()
            }

            #[must_use]
            pub fn row_at(
                &self,
                row: gamedata::RowIndex,
            ) -> Option<#row_ref_struct<'_, 'a, #table_marker>> {
                self.table
                    .row_at(row)
                    .map(|row| #row_ref_struct { view: self, row })
            }

            #inherent_get

            pub fn rows(&self) -> impl Iterator<Item = #row_ref_struct<'_, 'a, #table_marker>> + '_ {
                self.table
                    .rows()
                    .map(|row| #row_ref_struct { view: self, row })
            }

            #loot_bucket_method
            #perk_bucket_method
            #reward_track_method
        }

        impl<'a, #table_marker> gamedata::game_system::SystemView<'a> for #view_struct<'a, #table_marker>
        where
            #table_bound,
        {
            const NAME: &'static str = <#table_marker as gamedata::Table>::NAME;
            const TABLES: &'static [gamedata::TableRequirement] =
                &[gamedata::TableRequirement::typed::<#table_marker>()];

            fn from_system(
                system: &'a gamedata::game_system::System,
                token: gamedata::game_system::SystemViewToken,
            ) -> Result<Self, gamedata::GameDataError> {
                Self::from_table(system.table::<#table_marker>(token)?)
            }
        }

        #keyed_table_view_impl

        #[derive(Debug, Clone, Copy)]
        pub struct #row_ref_struct<'t, 'a, #table_marker>
        where
            #table_bound,
        {
            view: &'t #view_struct<'a, #table_marker>,
            row: gamedata::game_system::RowRef<'a, #table_marker>,
        }

        impl<'t, 'a, #table_marker> #row_ref_struct<'t, 'a, #table_marker>
        where
            #table_bound,
        {
            #generic_cell_accessors
            #(#row_getters)*
        }

        fn column_slot<C: gamedata::TableColumn>(
            table: gamedata::game_system::TableRef<'_, C::Table>,
        ) -> Result<gamedata::game_system::ColumnSlot<C>, gamedata::GameDataError> {
            table.require_column::<C>().map_err(|error| {
                gamedata::GameDataError::Decode(format!(
                    "table `{}` required field `{}` column `{}`: {error}",
                    <C::Table as gamedata::Table>::NAME,
                    C::FIELD_NAME,
                    C::COLUMN,
                ))
            })
        }

        #loot_bucket_helpers
        #perk_bucket_helpers
        #reward_track_helpers
    })
}

#[cfg(test)]
fn row_key_tokens(
    schema: &GameSystemTableSchema,
    table_marker: &Ident,
    row_key_field: &RustField,
) -> Result<TokenStream> {
    let row_key_column = schema
        .columns
        .iter()
        .find(|column| column.row_key)
        .context("row key field came from schema columns")?;
    let key_type = ty(&table_key_type_for_column(
        row_key_column,
        row_key_field,
        "'_",
    )?)?;
    let key_alias = ident(&format!("{table_marker}Key"));
    let row_key_column = ident(&row_key_field.rust_column_marker);
    Ok(quote! {
        pub type #key_alias = gamedata::TableKey<#row_key_column>;

        impl #table_marker {
            #[must_use]
            pub fn key(key: #key_type) -> #key_alias {
                gamedata::TableKey::<#row_key_column>::new(key)
            }
        }
    })
}

fn generic_row_key_tokens(
    schema: &GameSystemTableSchema,
    table_marker: &Ident,
    row_struct: &Ident,
    key_alias: &Ident,
    row_key_field: &RustField,
) -> Result<TokenStream> {
    let row_key_column_schema = schema
        .columns
        .iter()
        .find(|column| column.row_key)
        .context("row key field came from schema columns")?;
    let key_type = ty(&table_key_type_for_column(
        row_key_column_schema,
        row_key_field,
        "'_",
    )?)?;
    let row_key_column = ident(&row_key_field.rust_column_marker);
    Ok(quote! {
        pub type #key_alias<#table_marker> = gamedata::TableKey<#row_key_column<#table_marker>>;

        pub fn table_key<#table_marker>(
            key: #key_type,
        ) -> #key_alias<#table_marker>
        where
            #table_marker: gamedata::Table<Row = #row_struct>,
        {
            gamedata::TableKey::<#row_key_column<#table_marker>>::new(key)
        }
    })
}

#[cfg(test)]
fn keyed_table_view_tokens(
    table_marker: &Ident,
    view_struct: &Ident,
    row_ref_struct: &Ident,
    key_alias: Ident,
    row_key_field: &RustField,
) -> TokenStream {
    let row_key_column = ident(&row_key_field.rust_column_marker);
    quote! {
        impl<'a> gamedata::game_system::KeyedTableView<'a> for #view_struct<'a> {
            type Table = #table_marker;
            type KeyColumn = #row_key_column;
            type RowRef<'row> = #row_ref_struct<'row, 'a> where Self: 'row;

            fn get(&self, key: &#key_alias) -> Option<Self::RowRef<'_>> {
                #view_struct::get(self, key)
            }
        }
    }
}

fn generic_keyed_table_view_tokens(
    table_marker: &Ident,
    row_struct: &Ident,
    view_struct: &Ident,
    row_ref_struct: &Ident,
    key_alias: Ident,
    row_key_field: &RustField,
) -> TokenStream {
    let row_key_column = ident(&row_key_field.rust_column_marker);
    quote! {
        impl<'a, #table_marker> gamedata::game_system::KeyedTableView<'a> for #view_struct<'a, #table_marker>
        where
            #table_marker: gamedata::Table<Row = #row_struct>,
        {
            type Table = #table_marker;
            type KeyColumn = #row_key_column<#table_marker>;
            type RowRef<'row> = #row_ref_struct<'row, 'a, #table_marker> where Self: 'row;

            fn get(&self, key: &#key_alias<#table_marker>) -> Option<Self::RowRef<'_>> {
                #view_struct::get(self, key)
            }
        }
    }
}

fn generic_cell_accessor_tokens(table_marker: &Ident) -> TokenStream {
    quote! {
        pub fn cell<C>(self) -> Result<Option<C::Cell<'a>>, gamedata::GameDataError>
        where
            C: gamedata::TableColumn<Table = #table_marker>,
        {
            let column = self.view.table.require_column::<C>()?;
            Ok(self.row.cell_at(column))
        }

        pub fn require_cell<C>(self) -> Result<C::Cell<'a>, gamedata::GameDataError>
        where
            C: gamedata::TableColumn<Table = #table_marker>,
        {
            let column = self.view.table.require_column::<C>()?;
            self.row.require_cell_at(column)
        }
    }
}

#[cfg(test)]
fn row_getter_tokens(column: &GameSystemColumnSchema, field: &RustField) -> Result<TokenStream> {
    let getter = ident(&field.rust_name);
    let slot = ident(&field.rust_column_name);
    let cell_type = ty(&borrowed_cell_type_for_column(column, field, "'a"))?;
    if column.required {
        Ok(quote! {
            pub fn #getter(&self) -> Result<#cell_type, gamedata::GameDataError> {
                self.row.require_cell_at(self.view.#slot)
            }
        })
    } else {
        Ok(quote! {
            #[must_use]
            pub fn #getter(&self) -> Option<#cell_type> {
                self.row.cell_at(self.view.#slot)
            }
        })
    }
}

fn generic_row_getter_tokens(
    table_marker: &Ident,
    column: &GameSystemColumnSchema,
    field: &RustField,
) -> Result<TokenStream> {
    let getter = ident(&field.rust_name);
    let slot = ident(&field.rust_column_name);
    let column_marker_path = format!("{}<{table_marker}>", field.rust_column_marker);
    let cell_type = ty(&borrowed_cell_type_for_column_in_context(
        column,
        field,
        "'a",
        &column_marker_path,
        "super::super",
        None,
    ))?;
    if column.required {
        Ok(quote! {
            pub fn #getter(&self) -> Result<#cell_type, gamedata::GameDataError> {
                self.row.require_cell_at(self.view.#slot)
            }
        })
    } else {
        Ok(quote! {
            #[must_use]
            pub fn #getter(&self) -> Option<#cell_type> {
                self.row.cell_at(self.view.#slot)
            }
        })
    }
}

#[cfg(test)]
fn column_marker_tokens(
    root: &Path,
    table_marker: &Ident,
    column: &GameSystemColumnSchema,
    field: &RustField,
) -> Result<TokenStream> {
    let marker = ident(&field.rust_column_marker);
    let cell_type = ty(&borrowed_cell_type_for_column(column, field, "'cell"))?;
    let field_name = lit_str(&field.rust_name);
    let column_name = lit_str(&column.name);
    let row_key = LitBool::new(column.row_key, Span::call_site());
    let required = LitBool::new(column.required, Span::call_site());
    let foreign_keys = if field.foreign_key_meta_columns.is_empty() {
        TokenStream::new()
    } else {
        let targets = field
            .foreign_key_meta_columns
            .iter()
            .map(|column| ty(&foreign_key_column_type("super::super", column)))
            .collect::<Result<Vec<_>>>()?;
        quote! {
            const FOREIGN_KEYS: &'static [#root::ForeignKeyMeta] =
                &[#(#root::ForeignKeyMeta::of::<#targets>()),*];
        }
    };
    let enum_variants = table_code_column_enum_shape(column)
        .map(|enum_shape| {
            let variants = enum_shape
                .variants
                .iter()
                .map(|variant| {
                    let name = lit_str(&variant.name);
                    let source_tokens = variant.source_tokens.iter().map(|token| lit_str(token));
                    let discriminant = signed_i64_tokens(variant.discriminant);
                    quote! {
                        #root::EnumVariantMeta::new(#name, &[#(#source_tokens),*], #discriminant)
                    }
                })
                .collect::<Vec<_>>();
            quote! {
                const ENUM_VARIANTS: &'static [#root::EnumVariantMeta] = &[
                    #(#variants,)*
                ];
            }
        })
        .unwrap_or_default();
    let key_column_impl = if column.row_key {
        let key_type = ty(&table_key_type_for_column(column, field, "'key")?)?;
        quote! {
            impl gamedata::KeyColumn for #marker {
                type Key<'key> = #key_type;
            }
        }
    } else {
        TokenStream::new()
    };

    Ok(quote! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct #marker;

        impl gamedata::TableColumn for #marker {
            type Table = #table_marker;
            type Cell<'cell> = #cell_type;

            const FIELD_NAME: &'static str = #field_name;
            const COLUMN: &'static str = #column_name;
            const ROW_KEY: bool = #row_key;
            const REQUIRED: bool = #required;
            #foreign_keys
            #enum_variants
        }

        #key_column_impl
    })
}

fn generic_column_marker_tokens(
    root: &Path,
    root_path: &str,
    row_struct: TokenStream,
    enum_root: Option<&str>,
    column: &GameSystemColumnSchema,
    field: &RustField,
) -> Result<TokenStream> {
    let marker = ident(&field.rust_column_marker);
    let column_marker_path = format!("{}<T>", field.rust_column_marker);
    let cell_type = ty(&borrowed_cell_type_for_column_in_context(
        column,
        field,
        "'cell",
        &column_marker_path,
        root_path,
        enum_root,
    ))?;
    let field_name = lit_str(&field.rust_name);
    let column_name = lit_str(&column.name);
    let row_key = LitBool::new(column.row_key, Span::call_site());
    let required = LitBool::new(column.required, Span::call_site());
    let foreign_keys = if field.foreign_key_meta_columns.is_empty() {
        TokenStream::new()
    } else {
        let targets = field
            .foreign_key_meta_columns
            .iter()
            .map(|column| ty(&foreign_key_column_type(root_path, column)))
            .collect::<Result<Vec<_>>>()?;
        quote! {
            const FOREIGN_KEYS: &'static [#root::ForeignKeyMeta] =
                &[#(#root::ForeignKeyMeta::of::<#targets>()),*];
        }
    };
    let enum_variants = table_code_column_enum_shape(column)
        .map(|enum_shape| {
            let variants = enum_shape
                .variants
                .iter()
                .map(|variant| {
                    let name = lit_str(&variant.name);
                    let source_tokens = variant.source_tokens.iter().map(|token| lit_str(token));
                    let discriminant = signed_i64_tokens(variant.discriminant);
                    quote! {
                        #root::EnumVariantMeta::new(#name, &[#(#source_tokens),*], #discriminant)
                    }
                })
                .collect::<Vec<_>>();
            quote! {
                const ENUM_VARIANTS: &'static [#root::EnumVariantMeta] = &[
                    #(#variants,)*
                ];
            }
        })
        .unwrap_or_default();
    let key_column_impl = if column.row_key {
        let key_type = ty(&table_key_type_for_column(column, field, "'key")?)?;
        quote! {
            impl<T> gamedata::KeyColumn for #marker<T>
            where
                T: gamedata::Table<Row = #row_struct>,
            {
                type Key<'key> = #key_type;
            }
        }
    } else {
        TokenStream::new()
    };

    Ok(quote! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct #marker<T>(core::marker::PhantomData<fn() -> T>);

        impl<T> gamedata::TableColumn for #marker<T>
        where
            T: gamedata::Table<Row = #row_struct>,
        {
            type Table = T;
            type Cell<'cell> = #cell_type;

            const FIELD_NAME: &'static str = #field_name;
            const COLUMN: &'static str = #column_name;
            const ROW_KEY: bool = #row_key;
            const REQUIRED: bool = #required;
            #foreign_keys
            #enum_variants
        }

        #key_column_impl
    })
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}

fn lit_str(value: &str) -> LitStr {
    LitStr::new(value, Span::call_site())
}

fn ty(value: &str) -> Result<Type> {
    syn::parse_str::<Type>(value).with_context(|| format!("parse Rust type `{value}`"))
}

fn path(value: &str) -> Result<Path> {
    syn::parse_str::<Path>(value).with_context(|| format!("parse Rust path `{value}`"))
}

fn signed_i64_tokens(value: i64) -> TokenStream {
    let literal = LitInt::new(&value.unsigned_abs().to_string(), Span::call_site());
    if value.is_negative() {
        quote!(-#literal)
    } else {
        quote!(#literal)
    }
}
