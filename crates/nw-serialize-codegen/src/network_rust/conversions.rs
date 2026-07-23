use super::*;

pub(super) fn enum_marshaler_conversion_tokens(
    item: &SerializeCodegenItem,
) -> Vec<proc_macro2::TokenStream> {
    if item.kind != SerializeCodegenItemKind::Enum {
        return Vec::new();
    }
    let Some(underlying) = enum_underlying_scalar(item) else {
        return Vec::new();
    };
    let Some((min, max)) = enum_value_range(item) else {
        return Vec::new();
    };
    if min < 0 {
        return Vec::new();
    }

    let enum_ident = format_ident!("{}", rust_type_ident(&item.source_name));
    [
        UnsignedConversion::U8,
        UnsignedConversion::U16,
        UnsignedConversion::U32,
    ]
    .into_iter()
    .filter(|conversion| max <= i128::from(conversion.max_value()))
    .map(|conversion| {
        enum_marshaler_conversion_token(&enum_ident, underlying, conversion, min, max)
    })
    .collect()
}

pub(super) fn enum_underlying_scalar(item: &SerializeCodegenItem) -> Option<ScalarType> {
    match item.enum_underlying_type.as_ref()? {
        ResolvedType::Scalar(scalar) if is_integer_scalar(*scalar) => Some(*scalar),
        _ => None,
    }
}

pub(super) const fn is_integer_scalar(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Char
            | ScalarType::SignedChar
            | ScalarType::I8
            | ScalarType::U8
            | ScalarType::I16
            | ScalarType::U16
            | ScalarType::I32
            | ScalarType::U32
            | ScalarType::I64
            | ScalarType::U64
            | ScalarType::UnsignedLong
    )
}

pub(super) fn enum_value_range(item: &SerializeCodegenItem) -> Option<(i128, i128)> {
    let mut values = item
        .variants
        .iter()
        .map(|variant| {
            variant
                .value_i32
                .map(i128::from)
                .or_else(|| variant.value_u32.map(i128::from))
                .or_else(|| variant.value_u64.map(i128::from))
        })
        .collect::<Option<Vec<_>>>()?;
    values.sort_unstable();
    Some((*values.first()?, *values.last()?))
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UnsignedConversion {
    U8,
    U16,
    U32,
}

impl UnsignedConversion {
    const fn bit_width(self) -> u8 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
        }
    }

    const fn max_value(self) -> u32 {
        match self {
            Self::U8 => u8::MAX as u32,
            Self::U16 => u16::MAX as u32,
            Self::U32 => u32::MAX,
        }
    }

    fn rust_type(self) -> proc_macro2::TokenStream {
        match self {
            Self::U8 => quote!(u8),
            Self::U16 => quote!(u16),
            Self::U32 => quote!(u32),
        }
    }
}

pub(super) fn enum_marshaler_conversion_token(
    enum_ident: &proc_macro2::Ident,
    underlying: ScalarType,
    conversion: UnsignedConversion,
    min: i128,
    max: i128,
) -> proc_macro2::TokenStream {
    let serialized_ty = conversion.rust_type();
    let underlying_ty = enum_underlying_rust_type(underlying);
    let serialize_value = enum_serialize_value_tokens(underlying, conversion);
    let deserialize_value = enum_deserialize_value_tokens(underlying, conversion, min, max);
    let min_i128 = syn::LitInt::new(&min.to_string(), proc_macro2::Span::call_site());
    let max_i128 = syn::LitInt::new(&max.to_string(), proc_macro2::Span::call_site());
    let min_u64 = u64::try_from(min).expect("unsigned enum conversion has nonnegative min");
    let max_u64 = u64::try_from(max).expect("unsigned enum conversion has nonnegative max");

    quote! {
        impl ::nw_network::serialize::MarshalerConversion<#serialized_ty>
            for ::nw_network::source::#enum_ident
        {
            fn to_serialized(self) -> #serialized_ty {
                let raw = #underlying_ty::from(self);
                let raw_i128 = i128::from(raw);
                debug_assert!((#min_i128..=#max_i128).contains(&raw_i128));
                #serialize_value
            }

            fn try_from_serialized(
                value: #serialized_ty,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                let raw = #deserialize_value;
                Self::try_from(raw).map_err(|_| {
                    ::nw_network::serialize::MarshalerError::InvalidRange {
                        value: u64::from(value),
                        min: #min_u64,
                        max: #max_u64,
                    }
                })
            }
        }
    }
}

pub(super) fn enum_serialize_value_tokens(
    underlying: ScalarType,
    conversion: UnsignedConversion,
) -> proc_macro2::TokenStream {
    let serialized_ty = conversion.rust_type();
    if underlying == conversion.scalar_type() {
        return quote!(raw);
    }
    if unsigned_scalar_bit_width(underlying).is_some_and(|bits| bits <= conversion.bit_width()) {
        return quote!(#serialized_ty::from(raw));
    }
    quote! {
        #serialized_ty::try_from(raw)
            .expect("generated enum discriminant fits serialized representation")
    }
}

pub(super) fn enum_deserialize_value_tokens(
    underlying: ScalarType,
    conversion: UnsignedConversion,
    min: i128,
    max: i128,
) -> proc_macro2::TokenStream {
    let underlying_ty = enum_underlying_rust_type(underlying);
    let min_u64 = u64::try_from(min).expect("unsigned enum conversion has nonnegative min");
    let max_u64 = u64::try_from(max).expect("unsigned enum conversion has nonnegative max");
    if underlying == conversion.scalar_type() {
        return quote!(value);
    }
    if scalar_accepts_all_unsigned_values(underlying, conversion) {
        return quote!(#underlying_ty::from(value));
    }
    quote! {
        #underlying_ty::try_from(value).map_err(|_| {
            ::nw_network::serialize::MarshalerError::InvalidRange {
                value: u64::from(value),
                min: #min_u64,
                max: #max_u64,
            }
        })?
    }
}

impl UnsignedConversion {
    const fn scalar_type(self) -> ScalarType {
        match self {
            Self::U8 => ScalarType::U8,
            Self::U16 => ScalarType::U16,
            Self::U32 => ScalarType::U32,
        }
    }
}

pub(super) const fn unsigned_scalar_bit_width(scalar: ScalarType) -> Option<u8> {
    match scalar {
        ScalarType::U8 => Some(8),
        ScalarType::U16 => Some(16),
        ScalarType::U32 => Some(32),
        ScalarType::U64 | ScalarType::UnsignedLong => Some(64),
        _ => None,
    }
}

pub(super) const fn scalar_accepts_all_unsigned_values(
    scalar: ScalarType,
    conversion: UnsignedConversion,
) -> bool {
    match scalar {
        ScalarType::U8 => conversion.bit_width() <= 8,
        ScalarType::U16 => conversion.bit_width() <= 16,
        ScalarType::U32 => conversion.bit_width() <= 32,
        ScalarType::U64 | ScalarType::UnsignedLong => true,
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => {
            conversion.max_value() <= i8::MAX as u32
        }
        ScalarType::I16 => conversion.max_value() <= i16::MAX as u32,
        ScalarType::I32 => conversion.max_value() <= i32::MAX as u32,
        ScalarType::I64 => true,
        _ => false,
    }
}

pub(super) fn enum_underlying_rust_type(scalar: ScalarType) -> proc_macro2::TokenStream {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => quote!(i8),
        ScalarType::U8 => quote!(u8),
        ScalarType::I16 => quote!(i16),
        ScalarType::U16 => quote!(u16),
        ScalarType::I32 => quote!(i32),
        ScalarType::U32 => quote!(u32),
        ScalarType::I64 => quote!(i64),
        ScalarType::U64 | ScalarType::UnsignedLong => quote!(u64),
        _ => unreachable!("non-integer enum underlyings are skipped before emission"),
    }
}

pub(super) fn struct_native_marshaler_tokens(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<proc_macro2::TokenStream> {
    if item.kind != SerializeCodegenItemKind::Struct
        || item.is_abstract == Some(true)
        || item.fields.is_empty()
    {
        return None;
    }

    let struct_ident = format_ident!("{}", rust_type_ident(&item.source_name));
    let fields = item
        .fields
        .iter()
        .filter(|field| {
            crate::field_projection::classify_codegen_field(field, items_by_type_id)
                .is_materialized()
        })
        .map(|field| struct_marshaler_field_tokens(field, items_by_type_id))
        .collect::<Option<Vec<_>>>()?;
    let marshal_fields = fields.iter().map(|field| &field.marshal);
    let unmarshal_fields = fields.iter().map(|field| &field.unmarshal);

    Some(quote! {
        impl ::nw_network::serialize::Marshal for ::nw_network::source::#struct_ident {
            fn marshal(&self, wb: &mut ::nw_network::serialize::WriteBuffer) {
                #(#marshal_fields)*
            }
        }

        impl ::nw_network::serialize::Unmarshal for ::nw_network::source::#struct_ident {
            fn unmarshal(
                rb: &mut ::nw_network::serialize::ReadBuffer,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                Ok(Self {
                    #(#unmarshal_fields)*
                })
            }
        }
    })
}

pub(super) struct StructMarshalerFieldTokens {
    marshal: proc_macro2::TokenStream,
    unmarshal: proc_macro2::TokenStream,
}

pub(super) fn struct_marshaler_field_tokens(
    field: &crate::ir::SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<StructMarshalerFieldTokens> {
    let field_ident = format_ident!("{}", rust_field_ident(&field.source_name));
    if let ResolvedType::Named { type_id, .. } = &field.resolved_type
        && let Some(enum_item) = items_by_type_id.get(type_id)
        && enum_item.kind == SerializeCodegenItemKind::Enum
    {
        return struct_enum_field_marshaler_tokens(&field_ident, enum_item);
    }

    Some(StructMarshalerFieldTokens {
        marshal: quote! {
            ::nw_network::serialize::Marshal::marshal(&self.#field_ident, wb);
        },
        unmarshal: quote! {
            #field_ident: ::nw_network::serialize::Unmarshal::unmarshal(rb)?,
        },
    })
}

pub(super) fn struct_enum_field_marshaler_tokens(
    field_ident: &proc_macro2::Ident,
    enum_item: &SerializeCodegenItem,
) -> Option<StructMarshalerFieldTokens> {
    let underlying = enum_underlying_scalar(enum_item)?;
    let (min, max) = enum_value_range(enum_item)?;
    let enum_ident = format_ident!("{}", rust_type_ident(&enum_item.source_name));
    let enum_type = quote!(::nw_network::source::#enum_ident);
    let underlying_ty = enum_underlying_rust_type(underlying);
    let min_u64 = u64::try_from(min).unwrap_or(0);
    let max_u64 = u64::try_from(max).ok()?;

    Some(StructMarshalerFieldTokens {
        marshal: quote! {
            let raw = #underlying_ty::from(self.#field_ident);
            ::nw_network::serialize::Marshal::marshal(&raw, wb);
        },
        unmarshal: quote! {
            #field_ident: {
                let raw = <#underlying_ty as ::nw_network::serialize::Unmarshal>::unmarshal(rb)?;
                <#enum_type as ::core::convert::TryFrom<#underlying_ty>>::try_from(raw).map_err(|_| {
                    ::nw_network::serialize::MarshalerError::InvalidRange {
                        value: raw as u64,
                        min: #min_u64,
                        max: #max_u64,
                    }
                })?
            },
        },
    })
}
