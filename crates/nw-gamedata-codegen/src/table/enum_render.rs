use std::collections::BTreeSet;

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemEnumRepresentation,
    GameSystemEnumShape, GameSystemListAtomShape, GameSystemListElementShape,
    GameSystemTableSchema,
};
use anyhow::{Context, Result, bail};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Ident, LitInt, LitStr};

use crate::naming::to_upper_camel_ident;

pub(super) fn table_code_enum_shapes(
    schema: &GameSystemTableSchema,
) -> Result<Vec<GameSystemEnumShape>> {
    let mut shapes = Vec::new();
    for column in &schema.columns {
        collect_column_enum_shapes(column, &mut shapes)?;
    }
    Ok(shapes)
}

fn collect_column_enum_shapes(
    column: &GameSystemColumnSchema,
    shapes: &mut Vec<GameSystemEnumShape>,
) -> Result<()> {
    match &column.value_shape {
        GameSystemColumnValueShape::Enum { enum_shape } => {
            push_enum_shape(shapes, enum_shape)?;
        }
        GameSystemColumnValueShape::String {
            list: Some(list), ..
        } => {
            if let Some(element_shape) = &list.element_shape {
                collect_list_element_enum_shapes(element_shape, shapes)?;
            }
        }
        GameSystemColumnValueShape::Boolean
        | GameSystemColumnValueShape::Color { .. }
        | GameSystemColumnValueShape::Crc32
        | GameSystemColumnValueShape::Number { .. }
        | GameSystemColumnValueShape::Range { .. }
        | GameSystemColumnValueShape::String { list: None, .. } => {}
    }
    Ok(())
}

pub(crate) fn table_code_column_enum_shape(
    column: &GameSystemColumnSchema,
) -> Option<&GameSystemEnumShape> {
    match &column.value_shape {
        GameSystemColumnValueShape::Enum { enum_shape } => Some(enum_shape),
        GameSystemColumnValueShape::String {
            list: Some(list), ..
        } => match list.element_shape.as_ref() {
            Some(GameSystemListElementShape::Enum { enum_shape }) => Some(enum_shape),
            Some(
                GameSystemListElementShape::Boolean
                | GameSystemListElementShape::Color { .. }
                | GameSystemListElementShape::Number { .. }
                | GameSystemListElementShape::Range { .. }
                | GameSystemListElementShape::Crc32
                | GameSystemListElementShape::Pair { .. }
                | GameSystemListElementShape::String,
            )
            | None => None,
        },
        GameSystemColumnValueShape::Boolean
        | GameSystemColumnValueShape::Color { .. }
        | GameSystemColumnValueShape::Crc32
        | GameSystemColumnValueShape::Number { .. }
        | GameSystemColumnValueShape::Range { .. }
        | GameSystemColumnValueShape::String { list: None, .. } => None,
    }
}

fn collect_list_element_enum_shapes(
    element_shape: &GameSystemListElementShape,
    shapes: &mut Vec<GameSystemEnumShape>,
) -> Result<()> {
    match element_shape {
        GameSystemListElementShape::Enum { enum_shape } => push_enum_shape(shapes, enum_shape),
        GameSystemListElementShape::Pair { first, second, .. } => {
            collect_list_atom_enum_shapes(first, shapes)?;
            collect_list_atom_enum_shapes(second, shapes)
        }
        GameSystemListElementShape::Boolean
        | GameSystemListElementShape::Color { .. }
        | GameSystemListElementShape::Number { .. }
        | GameSystemListElementShape::Range { .. }
        | GameSystemListElementShape::Crc32
        | GameSystemListElementShape::String => Ok(()),
    }
}

fn collect_list_atom_enum_shapes(
    atom_shape: &GameSystemListAtomShape,
    shapes: &mut Vec<GameSystemEnumShape>,
) -> Result<()> {
    match atom_shape {
        GameSystemListAtomShape::Enum { enum_shape } => push_enum_shape(shapes, enum_shape),
        GameSystemListAtomShape::Boolean
        | GameSystemListAtomShape::Color { .. }
        | GameSystemListAtomShape::Number { .. }
        | GameSystemListAtomShape::Range { .. }
        | GameSystemListAtomShape::Crc32
        | GameSystemListAtomShape::String => Ok(()),
    }
}

fn push_enum_shape(
    shapes: &mut Vec<GameSystemEnumShape>,
    enum_shape: &GameSystemEnumShape,
) -> Result<()> {
    let enum_name = table_code_enum_type_name(enum_shape);
    if let Some(existing) = shapes
        .iter()
        .find(|existing| table_code_enum_type_name(existing) == enum_name)
    {
        if existing != enum_shape {
            bail!("enum shape collision for emitted type {enum_name}");
        }
        return Ok(());
    }
    shapes.push(enum_shape.clone());
    Ok(())
}

pub(super) fn render_table_code_enum_tokens(
    _schema: &GameSystemTableSchema,
    enum_shape: &GameSystemEnumShape,
) -> Result<TokenStream> {
    render_enum_tokens(enum_shape)
}

fn render_enum_tokens(enum_shape: &GameSystemEnumShape) -> Result<TokenStream> {
    if enum_shape.variants.is_empty() {
        bail!("enum {} must have at least one variant", enum_shape.name);
    }
    let enum_name = table_code_enum_type_name(enum_shape);
    let enum_ident = ident(&enum_name);
    let repr = table_code_enum_repr(enum_shape.representation);
    let repr_ident = ident(repr);
    let mut used_variants = BTreeSet::new();
    let variants = enum_shape
        .variants
        .iter()
        .map(|variant| {
            let variant_name = to_upper_camel_ident(&variant.name, "Variant");
            if !used_variants.insert(variant_name.clone()) {
                bail!(
                    "enum {} has duplicate emitted variant {variant_name}",
                    enum_shape.name
                );
            }
            let variant_ident = ident(&variant_name);
            let discriminant = enum_discriminant(enum_shape.representation, variant.discriminant)?;
            Ok(quote! {
                #variant_ident = #discriminant
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let enum_impl = render_table_code_enum_impl_tokens(enum_shape, &enum_ident)?;
    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(#repr_ident)]
        pub enum #enum_ident {
            #(#variants,)*
        }

        #enum_impl
    })
}

fn render_table_code_enum_impl_tokens(
    enum_shape: &GameSystemEnumShape,
    enum_ident: &Ident,
) -> Result<TokenStream> {
    let storage_type = table_code_enum_repr_tokens(enum_shape.representation);
    let representation_type = table_code_enum_representation_type_tokens(enum_shape.representation);
    let representation_value =
        table_code_enum_representation_value_tokens(enum_shape.representation);
    let from_repr_arms = table_code_enum_from_repr_arms(enum_shape)?;
    let as_str_arms = table_code_enum_as_str_arms(enum_shape);
    let from_name_checks = table_code_enum_from_name_checks(enum_shape);
    let missing_message = LitStr::new(
        &format!("enum {} has no variant named `{{}}`", enum_shape.name),
        Span::call_site(),
    );
    let from_impl = render_table_code_enum_from_impl_tokens(enum_shape, enum_ident)?;

    Ok(quote! {
        impl #enum_ident {
            const fn from_repr(value: #storage_type) -> Option<Self> {
                match value {
                    #(#from_repr_arms,)*
                    _ => None,
                }
            }

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    #(#as_str_arms,)*
                }
            }

            #[must_use]
            pub fn from_name(value: &str) -> Option<Self> {
                let value = value.trim();
                #(#from_name_checks)*
                None
            }
        }

        impl AsRef<str> for #enum_ident {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl TryFrom<&str> for #enum_ident {
            type Error = gamedata::GameDataError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::from_name(value).ok_or_else(|| {
                    gamedata::GameDataError::Decode(format!(#missing_message, value))
                })
            }
        }

        impl core::str::FromStr for #enum_ident {
            type Err = gamedata::GameDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from(value)
            }
        }

        #from_impl

        impl<'a> gamedata::TableEnum<'a> for #enum_ident {
            type Representation = #representation_type;

            fn from_representation(value: Self::Representation) -> Option<Self> {
                Self::from_repr(#representation_value)
            }
        }
    })
}

fn table_code_enum_from_repr_arms(enum_shape: &GameSystemEnumShape) -> Result<Vec<TokenStream>> {
    let mut used_discriminants = BTreeSet::new();
    enum_shape
        .variants
        .iter()
        .map(|variant| {
            let discriminant = enum_discriminant(enum_shape.representation, variant.discriminant)?;
            if !used_discriminants.insert(discriminant.key.clone()) {
                bail!(
                    "enum {} has duplicate discriminant {}",
                    enum_shape.name,
                    discriminant.key
                );
            }
            let variant_ident = ident(&to_upper_camel_ident(&variant.name, "Variant"));
            Ok(quote! {
                #discriminant => Some(Self::#variant_ident)
            })
        })
        .collect()
}

fn table_code_enum_as_str_arms(enum_shape: &GameSystemEnumShape) -> Vec<TokenStream> {
    enum_shape
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = ident(&to_upper_camel_ident(&variant.name, "Variant"));
            let display_name = variant
                .source_tokens
                .first()
                .map_or(variant.name.as_str(), String::as_str);
            let display_name = LitStr::new(display_name, Span::call_site());
            quote! {
                Self::#variant_ident => #display_name
            }
        })
        .collect()
}

fn table_code_enum_from_name_checks(enum_shape: &GameSystemEnumShape) -> Vec<TokenStream> {
    enum_shape
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = ident(&to_upper_camel_ident(&variant.name, "Variant"));
            let mut tokens = BTreeSet::new();
            tokens.insert(variant.name.as_str());
            for token in &variant.source_tokens {
                tokens.insert(token.as_str());
            }
            let tokens = tokens
                .into_iter()
                .map(|token| LitStr::new(token, Span::call_site()))
                .collect::<Vec<_>>();
            quote! {
                if false #( || value.eq_ignore_ascii_case(#tokens) )* {
                    return Some(Self::#variant_ident);
                }
            }
        })
        .collect()
}

fn render_table_code_enum_from_impl_tokens(
    enum_shape: &GameSystemEnumShape,
    enum_ident: &Ident,
) -> Result<TokenStream> {
    let representation_type = table_code_enum_representation_type_tokens(enum_shape.representation);
    let discriminant_arms = table_code_enum_discriminant_match_arms(enum_shape, enum_ident)?;
    let body = match enum_shape.representation {
        GameSystemEnumRepresentation::U8
        | GameSystemEnumRepresentation::I32
        | GameSystemEnumRepresentation::U32 => {
            quote! {
                match value {
                    #(#discriminant_arms,)*
                }
            }
        }
        GameSystemEnumRepresentation::Crc32 => {
            quote! {
                az_core::crc::Crc32::from_u32(match value {
                    #(#discriminant_arms,)*
                })
            }
        }
    };

    Ok(quote! {
        impl From<#enum_ident> for #representation_type {
            fn from(value: #enum_ident) -> Self {
                #body
            }
        }
    })
}

fn table_code_enum_discriminant_match_arms(
    enum_shape: &GameSystemEnumShape,
    enum_ident: &Ident,
) -> Result<Vec<TokenStream>> {
    enum_shape
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = ident(&to_upper_camel_ident(&variant.name, "Variant"));
            let discriminant = enum_discriminant(enum_shape.representation, variant.discriminant)?;
            Ok(quote! {
                #enum_ident::#variant_ident => #discriminant
            })
        })
        .collect()
}

fn ident(value: &str) -> Ident {
    Ident::new(value, Span::call_site())
}

fn table_code_enum_repr_tokens(representation: GameSystemEnumRepresentation) -> TokenStream {
    ident(table_code_enum_repr(representation)).into_token_stream()
}

fn table_code_enum_representation_type_tokens(
    representation: GameSystemEnumRepresentation,
) -> TokenStream {
    match representation {
        GameSystemEnumRepresentation::U8 => quote!(u8),
        GameSystemEnumRepresentation::I32 => quote!(i32),
        GameSystemEnumRepresentation::U32 => quote!(u32),
        GameSystemEnumRepresentation::Crc32 => quote!(az_core::crc::Crc32),
    }
}

fn table_code_enum_representation_value_tokens(
    representation: GameSystemEnumRepresentation,
) -> TokenStream {
    match representation {
        GameSystemEnumRepresentation::U8
        | GameSystemEnumRepresentation::I32
        | GameSystemEnumRepresentation::U32 => quote!(value),
        GameSystemEnumRepresentation::Crc32 => quote!(value.value()),
    }
}

pub(crate) fn table_code_enum_type_name(enum_shape: &GameSystemEnumShape) -> String {
    to_upper_camel_ident(&enum_shape.name, "EnumValue")
}

fn table_code_enum_repr(representation: GameSystemEnumRepresentation) -> &'static str {
    match representation {
        GameSystemEnumRepresentation::U8 => "u8",
        GameSystemEnumRepresentation::I32 => "i32",
        GameSystemEnumRepresentation::U32 | GameSystemEnumRepresentation::Crc32 => "u32",
    }
}

#[derive(Debug, Clone)]
struct EnumDiscriminant {
    tokens: TokenStream,
    key: String,
}

impl ToTokens for EnumDiscriminant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.tokens.clone());
    }
}

fn enum_discriminant(
    representation: GameSystemEnumRepresentation,
    discriminant: i64,
) -> Result<EnumDiscriminant> {
    let (tokens, key) = match representation {
        GameSystemEnumRepresentation::U8 => {
            let value = u8::try_from(discriminant)
                .with_context(|| format!("enum discriminant {discriminant} does not fit u8"))?;
            let literal = LitInt::new(&value.to_string(), Span::call_site());
            (quote!(#literal), value.to_string())
        }
        GameSystemEnumRepresentation::I32 => {
            let value = i32::try_from(discriminant)
                .with_context(|| format!("enum discriminant {discriminant} does not fit i32"))?;
            let literal = LitInt::new(&value.unsigned_abs().to_string(), Span::call_site());
            let tokens = if value.is_negative() {
                quote!(-#literal)
            } else {
                quote!(#literal)
            };
            (tokens, value.to_string())
        }
        GameSystemEnumRepresentation::U32 | GameSystemEnumRepresentation::Crc32 => {
            let value = u32::try_from(discriminant)
                .with_context(|| format!("enum discriminant {discriminant} does not fit u32"))?;
            let literal = LitInt::new(&value.to_string(), Span::call_site());
            (quote!(#literal), value.to_string())
        }
    };

    Ok(EnumDiscriminant { tokens, key })
}
