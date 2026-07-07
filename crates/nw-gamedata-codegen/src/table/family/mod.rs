use anyhow::Result;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitStr};

use crate::naming::{to_module_ident, to_upper_camel_ident, unique_ident};
use crate::rust::source::format_rust_tokens;

use super::model::{EmittedTableModule, TableCodeTypeGroup};

mod identity;

use identity::render_family_identity_tokens;

fn family_table_enum_name(row_type_name: &str) -> String {
    format!("{}Table", to_upper_camel_ident(row_type_name, "GameData"))
}

pub(super) fn render_table_code_type_mod(group: &TableCodeTypeGroup) -> Result<String> {
    format_rust_tokens(render_table_code_type_mod_tokens(group)?).map_err(Into::into)
}

pub(super) fn render_table_code_type_family_mod(
    group: &TableCodeTypeGroup,
) -> Result<Option<String>> {
    if group.tables.len() <= 1 {
        return Ok(None);
    }

    format_rust_tokens(render_family_identity_tokens(group, true)?)
        .map(Some)
        .map_err(Into::into)
}

pub(super) fn family_identity_module_name(group: &TableCodeTypeGroup) -> String {
    let mut used_modules = group.used_table_modules.clone();
    used_modules.extend(
        group
            .schema_modules
            .iter()
            .map(|schema| schema.module_name.clone()),
    );
    unique_ident("family".to_owned(), &mut used_modules)
}

fn render_table_code_type_mod_tokens(group: &TableCodeTypeGroup) -> Result<TokenStream> {
    let name = LitStr::new(&group.row_type_name, Span::call_site());
    let schema_module_decls = group
        .schema_modules
        .iter()
        .map(|schema| {
            let module = ident(&schema.module_name);
            quote! {
                pub mod #module;
            }
        })
        .collect::<Vec<_>>();
    let module_decls = group
        .tables
        .iter()
        .map(|table| {
            let module_name = family_table_module_name(group, table);
            let module = ident(&module_name);
            if module_name == table.module_name {
                quote! {
                    pub mod #module;
                }
            } else {
                let alias = ident(&table.module_name);
                let path = LitStr::new(&format!("{}.rs", table.module_name), Span::call_site());
                quote! {
                    #[path = #path]
                    pub mod #module;
                    pub use #module as #alias;
                }
            }
        })
        .collect::<Vec<_>>();
    let table_entries = group
        .tables
        .iter()
        .map(|table| {
            let module = ident(&family_table_module_name(group, table));
            quote! {
                &#module::TABLE
            }
        })
        .collect::<Vec<_>>();
    let requirement_entries = group
        .tables
        .iter()
        .map(|table| {
            let module = ident(&family_table_module_name(group, table));
            quote! {
                #module::TABLE_REQUIREMENT
            }
        })
        .collect::<Vec<_>>();
    let family_table_enum_name =
        (group.tables.len() > 1).then(|| family_table_enum_name(&group.row_type_name));
    let family_module_decl = family_table_enum_name
        .is_some()
        .then(|| {
            let module = ident(&family_identity_module_name(group));
            quote! {
                mod #module;
                pub use #module::*;
            }
        })
        .unwrap_or_default();
    let reexports = group
        .tables
        .iter()
        .filter(|table| family_table_enum_name.as_deref() != Some(table.marker_name.as_str()))
        .map(|table| {
            let module = ident(&family_table_module_name(group, table));
            let marker = ident(&table.marker_name);
            quote! {
                pub use #module::#marker;
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        #![allow(dead_code)]

        pub const NAME: &str = #name;

        #family_module_decl
        #(#schema_module_decls)*
        #(#module_decls)*

        pub const TABLES: &[&'static dyn gamedata::table_set::TableSchema] = &[
            #(#table_entries,)*
        ];

        pub const TABLE_REQUIREMENTS: &[gamedata::TableRequirement] = &[
            #(#requirement_entries,)*
        ];

        #(#reexports)*
    })
}

fn family_table_variant(group: &TableCodeTypeGroup, table: &EmittedTableModule) -> String {
    table
        .marker_name
        .strip_prefix(&group.row_type_name)
        .filter(|variant| !variant.is_empty())
        .unwrap_or(&table.marker_name)
        .to_owned()
}

fn family_table_module_name(group: &TableCodeTypeGroup, table: &EmittedTableModule) -> String {
    let parent_module = to_module_ident(&group.row_type_name, "rowtype");
    if table.module_name == parent_module {
        format!("{}_table", table.module_name)
    } else {
        table.module_name.clone()
    }
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}
