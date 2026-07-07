use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;

use crate::naming::to_upper_camel_ident;

use super::super::model::TableCodeTypeGroup;
use super::{family_table_enum_name, family_table_module_name, family_table_variant, ident};

pub(super) fn render_family_identity_tokens(
    group: &TableCodeTypeGroup,
    in_child_module: bool,
) -> Result<TokenStream> {
    let row_type = to_upper_camel_ident(&group.row_type_name, "GameData");
    let table_enum = ident(&family_table_enum_name(&group.row_type_name));
    let handle_struct = ident(&format!("{row_type}Handle"));
    let table_variants = group
        .tables
        .iter()
        .map(|table| ident(&family_table_variant(group, table)))
        .collect::<Vec<_>>();
    let table_name_arms = group
        .tables
        .iter()
        .map(|table| {
            let variant = ident(&family_table_variant(group, table));
            let table_path = table_marker_path(group, table, in_child_module);
            quote! {
                Self::#variant => <#table_path as gamedata::Table>::NAME
            }
        })
        .collect::<Vec<_>>();
    let table_crc_arms = group
        .tables
        .iter()
        .map(|table| {
            let variant = ident(&family_table_variant(group, table));
            let table_path = table_marker_path(group, table, in_child_module);
            quote! {
                Self::#variant => <#table_path as gamedata::Table>::CRC
            }
        })
        .collect::<Vec<_>>();
    let table_from_name_checks = group
        .tables
        .iter()
        .map(|table| {
            let variant = ident(&family_table_variant(group, table));
            let table_path = table_marker_path(group, table, in_child_module);
            quote! {
                if table_name_matches::<#table_path>(name) {
                    return Some(Self::#variant);
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #table_enum {
            #(#table_variants,)*
        }

        impl #table_enum {
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    #(#table_name_arms,)*
                }
            }

            #[must_use]
            pub const fn crc(self) -> u32 {
                match self {
                    #(#table_crc_arms,)*
                }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                #(#table_from_name_checks)*
                None
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct #handle_struct {
            pub table: #table_enum,
            pub row: gamedata::RowIndex,
        }

        impl #handle_struct {
            #[must_use]
            pub const fn new(table: #table_enum, row: gamedata::RowIndex) -> Self {
                Self { table, row }
            }
        }

        fn table_name_matches<T: gamedata::Table>(name: &str) -> bool {
            name == T::NAME
        }
    })
}

fn table_marker_path(
    group: &TableCodeTypeGroup,
    table: &super::super::model::EmittedTableModule,
    in_child_module: bool,
) -> TokenStream {
    let module = ident(&family_table_module_name(group, table));
    let marker = ident(&table.marker_name);
    if in_child_module {
        quote!(super::#module::#marker)
    } else {
        quote!(#module::#marker)
    }
}
