use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{GenericArgument, Ident, LitInt, LitStr, Path, PathArguments, Type};

use crate::CodegenContext;
use crate::rust::enum_plan::RustVariantPlan;
use crate::rust::identity::RustTypeIdentityKind;
use crate::rust::item_plan::{
    RustCodegenUnit, RustFieldPlan, RustIntegerRangePlan, RustItemKind, RustItemPlan,
};
use crate::rust::layout::RustStandaloneLayoutReport;

use super::support;

mod error;
mod project;

pub use error::RustSourceEmitError;
pub use project::{RustStandaloneProject, RustStandaloneProjectFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustSourceOptions {
    pub mode: RustSourceMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RustSourceMode {
    #[default]
    Integrated,
    Standalone,
}

impl Default for RustSourceOptions {
    fn default() -> Self {
        Self {
            mode: RustSourceMode::Integrated,
        }
    }
}

#[derive(Debug, Default)]
pub struct RustSourceEmitter {
    options: RustSourceOptions,
}

impl RustSourceEmitter {
    #[must_use]
    pub const fn new(options: RustSourceOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub const fn standalone() -> Self {
        Self::new(RustSourceOptions {
            mode: RustSourceMode::Standalone,
        })
    }

    pub fn emit_unit(
        unit: &RustCodegenUnit,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        Self::default().emit(unit, context)
    }

    pub fn emit_standalone_unit(
        unit: &RustCodegenUnit,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        Self::standalone().emit(unit, context)
    }

    pub fn emit_standalone_project(
        unit: &RustCodegenUnit,
        context: &CodegenContext,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        project::emit_standalone_project(unit, context)
    }

    pub fn emit_integrated_project(
        unit: &RustCodegenUnit,
        context: &CodegenContext,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        project::emit_integrated_project(unit, context)
    }

    #[must_use]
    pub fn standalone_layout_report(unit: &RustCodegenUnit) -> RustStandaloneLayoutReport {
        RustStandaloneLayoutReport::from_codegen_unit(unit)
    }

    pub fn emit(
        &self,
        unit: &RustCodegenUnit,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        let imports = imports_for_unit(unit, self.options);
        let support = support_items_for_unit(self.options)?;
        let rendered =
            context
                .runner()
                .try_map_until_cancelled(&unit.items, context.cancel(), |item| {
                    render_item_source(item, self.options)
                })?;
        let cancelled = rendered.was_cancelled();
        let items = rendered.into_completed();
        if cancelled {
            return Err(RustSourceEmitError::Cancelled);
        }
        let file = syn::parse2::<syn::File>(quote! {
            #(#imports)*

            #support
        })
        .map_err(RustSourceEmitError::File)?;

        let mut source = prettyplease::unparse(&file);
        for item in items {
            source.push('\n');
            source.push_str(&item);
        }

        Ok(space_rust_top_level_items(&source))
    }
}

fn render_item_source(
    item: &RustItemPlan,
    options: RustSourceOptions,
) -> Result<String, RustSourceEmitError> {
    let rendered = render_item(item, options)?;
    let file = syn::parse2::<syn::File>(quote! {
        #rendered
    })
    .map_err(RustSourceEmitError::File)?;
    Ok(prettyplease::unparse(&file))
}

fn imports_for_unit(unit: &RustCodegenUnit, options: RustSourceOptions) -> Vec<TokenStream> {
    if matches!(options.mode, RustSourceMode::Standalone) {
        let needs_bevy_component = unit.items.iter().any(|item| {
            item.derives
                .iter()
                .any(|derive_name| derive_name == "Component")
        });
        let mut imports = Vec::new();
        if needs_bevy_component {
            imports.push(quote!(
                use bevy_ecs::reflect::ReflectComponent;
            ));
        }
        return imports;
    }

    let needs_az_type_info = unit.items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "AzTypeInfo")
    });
    let needs_az_rtti = unit.items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "AzRtti")
    });
    let needs_bevy_component = unit.items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Component")
    });
    let needs_reflect = unit.items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Reflect")
    });
    let needs_marshaler = unit.items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Marshaler")
    });

    let mut imports = Vec::new();
    let mut az_derives = Vec::new();
    if needs_az_rtti {
        az_derives.push(Ident::new("AzRtti", Span::call_site()));
    }
    if needs_az_type_info {
        az_derives.push(Ident::new("AzTypeInfo", Span::call_site()));
    }
    if !az_derives.is_empty() {
        imports.push(quote!(
            use az_derive::{#(#az_derives),*};
        ));
    }
    match (needs_bevy_component, needs_reflect) {
        (true, true) => imports.push(quote!(
            use bevy::ecs::reflect::ReflectComponent;
        )),
        (true, false) | (false, true) => {}
        (false, false) => {}
    }
    if needs_marshaler {
        imports.push(quote!(
            use gridmate::Marshaler;
        ));
    }
    imports
}

fn rustfmt_source(source: &str) -> Result<String, RustSourceEmitError> {
    let file = syn::parse_file(source).map_err(RustSourceEmitError::File)?;
    Ok(space_rust_top_level_items(&prettyplease::unparse(&file)))
}

fn space_rust_top_level_items(source: &str) -> String {
    let mut out = String::new();
    let mut last_line_class = RustTopLevelLine::Other;
    let mut last_output_line_was_blank = true;
    let mut outer_attribute_depth = 0isize;
    let lines = source.lines().collect::<Vec<_>>();

    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            let next_line_class = lines
                .iter()
                .skip(index + 1)
                .find(|next| !next.trim().is_empty())
                .map(|next| classify_rust_top_level_line(next, outer_attribute_depth));
            if last_line_class == RustTopLevelLine::OuterAttribute
                && matches!(
                    next_line_class,
                    Some(RustTopLevelLine::OuterAttribute | RustTopLevelLine::Declaration)
                )
            {
                continue;
            }
            out.push('\n');
            last_output_line_was_blank = true;
            continue;
        }

        let current_line_class = classify_rust_top_level_line(line, outer_attribute_depth);
        if current_line_class.starts_statement()
            && !last_output_line_was_blank
            && !RustTopLevelLine::same_group(last_line_class, current_line_class)
        {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        last_output_line_was_blank = false;
        last_line_class = current_line_class;
        if line.starts_with("#[") || outer_attribute_depth > 0 {
            outer_attribute_depth += delimiter_delta(line);
            outer_attribute_depth = outer_attribute_depth.max(0);
        }
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustTopLevelLine {
    InnerAttribute,
    OuterAttribute,
    Import,
    Reexport,
    Module,
    Declaration,
    Other,
}

impl RustTopLevelLine {
    const fn starts_statement(self) -> bool {
        matches!(
            self,
            Self::InnerAttribute
                | Self::OuterAttribute
                | Self::Import
                | Self::Reexport
                | Self::Module
                | Self::Declaration
        )
    }

    const fn same_group(left: Self, right: Self) -> bool {
        matches!(
            (left, right),
            (Self::OuterAttribute, Self::OuterAttribute)
                | (Self::OuterAttribute, Self::Declaration)
                | (Self::Import, Self::Import)
                | (Self::Reexport, Self::Reexport)
                | (Self::Module, Self::Module)
        )
    }
}

fn classify_rust_top_level_line(line: &str, outer_attribute_depth: isize) -> RustTopLevelLine {
    if outer_attribute_depth > 0 {
        return RustTopLevelLine::OuterAttribute;
    }
    if line.starts_with(' ') || line.starts_with('\t') {
        return RustTopLevelLine::Other;
    }
    if line.starts_with("#![") {
        return RustTopLevelLine::InnerAttribute;
    }
    if line.starts_with("#[") {
        return RustTopLevelLine::OuterAttribute;
    }
    if line.starts_with("use ") {
        return RustTopLevelLine::Import;
    }
    if line.starts_with("pub use ") {
        return RustTopLevelLine::Reexport;
    }
    if line.starts_with("mod ") || line.starts_with("pub mod ") {
        return RustTopLevelLine::Module;
    }
    if line.starts_with("pub ")
        || line.starts_with("impl")
        || line.starts_with("unsafe impl")
        || line.starts_with("trait ")
        || line.starts_with("struct ")
        || line.starts_with("enum ")
        || line.starts_with("type ")
        || line.starts_with("const ")
        || line.starts_with("static ")
        || line.starts_with("fn ")
    {
        return RustTopLevelLine::Declaration;
    }

    RustTopLevelLine::Other
}

fn delimiter_delta(line: &str) -> isize {
    line.chars()
        .map(|ch| match ch {
            '(' | '[' | '{' => 1,
            ')' | ']' | '}' => -1,
            _ => 0,
        })
        .sum()
}

fn support_items_for_unit(options: RustSourceOptions) -> Result<TokenStream, RustSourceEmitError> {
    if !matches!(options.mode, RustSourceMode::Standalone) {
        return Ok(TokenStream::new());
    }

    let source = support::single_file_source();
    let file = syn::parse_file(&source).map_err(RustSourceEmitError::File)?;
    Ok(file.to_token_stream())
}

fn render_item(
    item: &RustItemPlan,
    options: RustSourceOptions,
) -> Result<TokenStream, RustSourceEmitError> {
    let ident = parse_item_ident(&item.rust_name, &item.source_name).map_err(|source| {
        RustSourceEmitError::ItemIdent {
            source_name: item.source_name.clone(),
            identifier: item.rust_name.clone(),
            source,
        }
    })?;
    let derives = item
        .derives
        .iter()
        .map(|derive_name| {
            let derive_path = standalone_derive_path(derive_name, options);
            syn::parse_str::<Path>(&derive_path).map_err(|source| RustSourceEmitError::DerivePath {
                item_name: item.rust_name.clone(),
                derive_name: derive_path,
                source,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let identity_attr = render_identity_attr(item, options)?;
    let reflect_attr = reflect_attr_for_item(item);
    let serde_attr = render_serde_container_attr(item);
    let standalone_identity = render_standalone_identity_impls(item, &ident, options);

    match item.kind {
        RustItemKind::Struct => {
            let fields = item
                .fields
                .iter()
                .map(|field| render_field(item, field, options))
                .collect::<Result<Vec<_>, _>>()?;
            let body = if fields.is_empty() {
                quote!(;)
            } else {
                quote!({ #(#fields)* })
            };
            let range_impl = render_range_impl(item, &ident)?;
            Ok(quote! {
                #[derive(#(#derives,)*)]
                #identity_attr
                #serde_attr
                #reflect_attr
                pub struct #ident #body

                #range_impl
                #standalone_identity
            })
        }
        RustItemKind::Enum => {
            let repr_attr = render_repr_attr(item)?;
            let default_variant =
                default_enum_variant(item).map(|variant| variant.rust_name.clone());
            let variants = item
                .variants
                .iter()
                .map(|variant| render_variant(item, variant, default_variant.as_deref()))
                .collect::<Result<Vec<_>, _>>()?;
            let raw_conversion_impls = render_raw_conversion_impls(item, &ident)?;
            let string_conversion_impls = render_string_conversion_impls(item, &ident)?;
            Ok(quote! {
                #[derive(#(#derives,)*)]
                #repr_attr
                #identity_attr
                #serde_attr
                pub enum #ident {
                    #(#variants)*
                }

                #raw_conversion_impls
                #string_conversion_impls
                #standalone_identity
            })
        }
        RustItemKind::SumEnum => {
            let default_variant =
                default_sum_variant(item).map(|variant| variant.rust_name.clone());
            let variants = item
                .variants
                .iter()
                .map(|variant| render_variant(item, variant, default_variant.as_deref()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(quote! {
                #[derive(#(#derives,)*)]
                #identity_attr
                #serde_attr
                pub enum #ident {
                    #(#variants)*
                }

                #standalone_identity
            })
        }
        RustItemKind::RawEnum => {
            let raw_enum = render_raw_enum_item(
                item,
                &ident,
                &derives,
                identity_attr,
                serde_attr,
                standalone_identity,
            )?;
            Ok(raw_enum)
        }
    }
}

fn render_serde_container_attr(item: &RustItemPlan) -> TokenStream {
    let has_serde = item.derives.iter().any(|derive| {
        is_serde_derive(derive, "Serialize") || is_serde_derive(derive, "Deserialize")
    });
    let Some(raw_conversion) = &item.raw_conversion else {
        return TokenStream::new();
    };
    if !has_serde {
        return TokenStream::new();
    }

    let raw_type = LitStr::new(&raw_conversion.raw_type, Span::call_site());
    quote!(#[serde(try_from = #raw_type, into = #raw_type)])
}

fn is_serde_derive(derive: &str, name: &str) -> bool {
    derive == name || derive.strip_prefix("serde::") == Some(name)
}

fn standalone_derive_path(derive_name: &str, options: RustSourceOptions) -> String {
    match (options.mode, derive_name) {
        (RustSourceMode::Integrated, "Component") => "::bevy::prelude::Component".to_owned(),
        (RustSourceMode::Integrated, "Reflect") => "::bevy::prelude::Reflect".to_owned(),
        (RustSourceMode::Standalone, "Component") => "bevy_ecs::component::Component".to_owned(),
        (RustSourceMode::Standalone, "Reflect") => "bevy_reflect::Reflect".to_owned(),
        (_, "Serialize") => "serde::Serialize".to_owned(),
        (_, "Deserialize") => "serde::Deserialize".to_owned(),
        _ => derive_name.to_owned(),
    }
}

fn render_raw_enum_item(
    item: &RustItemPlan,
    ident: &Ident,
    derives: &[Path],
    identity_attr: TokenStream,
    serde_attr: TokenStream,
    standalone_identity: TokenStream,
) -> Result<TokenStream, RustSourceEmitError> {
    let Some(raw_conversion) = &item.raw_conversion else {
        return Ok(TokenStream::new());
    };
    let raw_ty = syn::parse_str::<Type>(&raw_conversion.raw_type).map_err(|source| {
        RustSourceEmitError::RawConversionType {
            item_name: item.rust_name.clone(),
            raw_type: raw_conversion.raw_type.clone(),
            source,
        }
    })?;
    let constants = item
        .variants
        .iter()
        .map(|variant| {
            render_raw_enum_const(item, variant)
                .transpose()
                .map(Option::into_iter)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let variant_entries = render_raw_enum_variant_entries(item)?;
    let string_impls = render_raw_enum_string_impls(item, ident)?;

    Ok(quote! {
        #[derive(#(#derives,)*)]
        #[repr(transparent)]
        #identity_attr
        #serde_attr
        pub struct #ident(pub #raw_ty);

        impl #ident {
            pub const VARIANTS: &[(#raw_ty, &str)] = &[#(#variant_entries),*];

            #(#constants)*

            #[must_use]
            pub const fn value(self) -> #raw_ty {
                self.0
            }

            #[must_use]
            pub fn as_str(self) -> Option<&'static str> {
                Self::VARIANTS
                    .iter()
                    .find_map(|(value, name)| (*value == self.0).then_some(*name))
            }
        }

        impl From<#ident> for #raw_ty {
            fn from(value: #ident) -> Self {
                value.0
            }
        }

        impl From<#raw_ty> for #ident {
            fn from(value: #raw_ty) -> Self {
                Self(value)
            }
        }

        impl AsRef<#raw_ty> for #ident {
            fn as_ref(&self) -> &#raw_ty {
                &self.0
            }
        }

        #string_impls
        #standalone_identity
    })
}

fn render_range_impl(
    item: &RustItemPlan,
    ident: &Ident,
) -> Result<TokenStream, RustSourceEmitError> {
    let constants = item
        .fields
        .iter()
        .filter_map(|field| {
            field
                .integer_range
                .as_ref()
                .map(|range| render_range_const(item, field, range))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if constants.is_empty() {
        return Ok(TokenStream::new());
    }

    Ok(quote! {
        impl #ident {
            #(#constants)*
        }
    })
}

fn render_range_const(
    item: &RustItemPlan,
    field: &RustFieldPlan,
    range: &RustIntegerRangePlan,
) -> Result<TokenStream, RustSourceEmitError> {
    let const_name = format!("{}_RANGE", field.rust_name.to_shouty_snake_case());
    let ident = syn::parse_str::<Ident>(&const_name).map_err(|source| {
        RustSourceEmitError::RangeConstIdent {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            identifier: const_name.clone(),
            source,
        }
    })?;
    let ty = syn::parse_str::<Type>(&range.rust_type).map_err(|source| {
        RustSourceEmitError::RangeType {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            rust_type: range.rust_type.clone(),
            source,
        }
    })?;
    let start_bound = typed_range_bound_literal(&range.start, &range.value_type);
    let last_bound = typed_range_bound_literal(&range.last, &range.value_type);
    let start = syn::parse_str::<syn::Expr>(&start_bound).map_err(|source| {
        RustSourceEmitError::RangeBound {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            bound: range.start.clone(),
            source,
        }
    })?;
    let last = syn::parse_str::<syn::Expr>(&last_bound).map_err(|source| {
        RustSourceEmitError::RangeBound {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            bound: range.last.clone(),
            source,
        }
    })?;

    Ok(quote! {
        pub const #ident: #ty = #start..=#last;
    })
}

fn render_repr_attr(item: &RustItemPlan) -> Result<TokenStream, RustSourceEmitError> {
    let Some(repr) = &item.repr else {
        return Ok(TokenStream::new());
    };
    let repr = syn::parse_str::<Ident>(repr).map_err(|source| RustSourceEmitError::Repr {
        item_name: item.rust_name.clone(),
        repr: repr.clone(),
        source,
    })?;
    Ok(quote!(#[repr(#repr)]))
}

fn render_identity_attr(
    item: &RustItemPlan,
    options: RustSourceOptions,
) -> Result<TokenStream, RustSourceEmitError> {
    if matches!(options.mode, RustSourceMode::Standalone) {
        return Ok(TokenStream::new());
    }

    let identity = &item.identity;
    let type_id = LitStr::new(
        &identity.type_id.hyphenated().to_string().to_uppercase(),
        Span::call_site(),
    );
    let name_override = identity
        .name
        .as_ref()
        .filter(|name| name.as_str() != item.rust_name)
        .map(|name| LitStr::new(name, Span::call_site()));
    let attr = match (identity.kind, name_override) {
        (RustTypeIdentityKind::AzTypeInfo, Some(name)) => {
            quote!(#[az_type_info(name = #name, uuid = #type_id)])
        }
        (RustTypeIdentityKind::AzTypeInfo, None) => quote!(#[az_type_info(#type_id)]),
        (RustTypeIdentityKind::AzRtti, Some(name)) => {
            let bases = az_rtti_base_attr(item)?;
            if bases.is_empty() {
                quote!(#[az_rtti(name = #name, uuid = #type_id)])
            } else {
                quote! {
                    #[az_rtti(name = #name)]
                    #[az_rtti(#type_id #bases)]
                }
            }
        }
        (RustTypeIdentityKind::AzRtti, None) => {
            let bases = az_rtti_base_attr(item)?;
            quote!(#[az_rtti(#type_id #bases)])
        }
    };
    Ok(attr)
}

fn az_rtti_base_attr(item: &RustItemPlan) -> Result<TokenStream, RustSourceEmitError> {
    item.rtti_bases
        .iter()
        .map(|base| {
            let ty = syn::parse_str::<Type>(&base.rust_type).map_err(|source| {
                RustSourceEmitError::FieldType {
                    item_name: item.rust_name.clone(),
                    field_name: base.source_name.clone(),
                    rust_type: base.rust_type.clone(),
                    source,
                }
            })?;
            Ok(quote!(, #ty))
        })
        .collect::<Result<TokenStream, RustSourceEmitError>>()
}

fn render_standalone_identity_impls(
    item: &RustItemPlan,
    ident: &Ident,
    options: RustSourceOptions,
) -> TokenStream {
    if !matches!(options.mode, RustSourceMode::Standalone) {
        return TokenStream::new();
    }

    let type_id = az_uuid_expr(item.identity.type_id);
    let name = item
        .identity
        .name
        .as_deref()
        .unwrap_or(item.source_name.as_str());
    let name = LitStr::new(name, Span::call_site());
    let base_type_ids = item
        .rtti_bases
        .iter()
        .map(|base| az_uuid_expr(base.source_type_id))
        .collect::<Vec<_>>();
    let base_type_ids = (!base_type_ids.is_empty()).then(|| {
        quote! {
            const BASE_TYPE_IDS: &'static [AzUuid] = &[#(#base_type_ids),*];
        }
    });
    quote! {
        impl AzRtti for #ident {
            const NAME: &'static str = #name;
            const TYPE_ID: AzUuid = #type_id;
            #base_type_ids
        }
    }
}

fn az_uuid_expr(type_id: uuid::Uuid) -> TokenStream {
    let value = uuid_u128_literal(type_id);
    quote!(AzUuid::from_u128(#value))
}

fn uuid_u128_literal(type_id: uuid::Uuid) -> LitInt {
    let hex = format!("{:032X}", type_id.as_u128());
    let grouped = hex
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).expect("hex group is ASCII"))
        .collect::<Vec<_>>()
        .join("_");
    LitInt::new(&format!("0x{grouped}"), Span::call_site())
}

fn reflect_attr_for_item(item: &RustItemPlan) -> TokenStream {
    let has_component = item
        .derives
        .iter()
        .any(|derive_name| derive_name == "Component");
    let has_reflect = item
        .derives
        .iter()
        .any(|derive_name| derive_name == "Reflect");
    if has_component && has_reflect {
        quote!(#[reflect(Component)])
    } else {
        TokenStream::new()
    }
}

fn render_field(
    item: &RustItemPlan,
    field: &RustFieldPlan,
    _options: RustSourceOptions,
) -> Result<TokenStream, RustSourceEmitError> {
    if let Some(unresolved) = &field.unresolved_type {
        return Err(RustSourceEmitError::UnresolvedType {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            type_id: unresolved.type_id,
            reason: unresolved.reason.clone(),
        });
    }

    let ident = syn::parse_str::<Ident>(&field.rust_name).map_err(|source| {
        RustSourceEmitError::FieldIdent {
            item_name: item.rust_name.clone(),
            source_name: field.source_name.clone(),
            identifier: field.rust_name.clone(),
            source,
        }
    })?;
    let mut ty = syn::parse_str::<Type>(&field.rust_type).map_err(|source| {
        RustSourceEmitError::FieldType {
            item_name: item.rust_name.clone(),
            field_name: field.rust_name.clone(),
            rust_type: field.rust_type.clone(),
            source,
        }
    })?;
    replace_self_type_references(&mut ty, &item.rust_name);
    let serde_attr = render_field_serde_attr(item, field);
    Ok(quote!(#serde_attr pub #ident: #ty,))
}

fn render_field_serde_attr(item: &RustItemPlan, field: &RustFieldPlan) -> TokenStream {
    let has_serde = item.derives.iter().any(|derive| {
        is_serde_derive(derive, "Serialize") || is_serde_derive(derive, "Deserialize")
    });
    if !has_serde {
        return TokenStream::new();
    }

    let has_default = item.derives.iter().any(|derive| derive == "Default");
    let rename = (field.source_name != field.rust_name)
        .then(|| LitStr::new(&field.source_name, Span::call_site()));

    match (rename, has_default) {
        (Some(rename), true) => quote!(#[serde(rename = #rename, default)]),
        (Some(rename), false) => quote!(#[serde(rename = #rename)]),
        (None, true) => quote!(#[serde(default)]),
        (None, false) => TokenStream::new(),
    }
}

fn replace_self_type_references(ty: &mut Type, item_name: &str) {
    match ty {
        Type::Array(array) => replace_self_type_references(&mut array.elem, item_name),
        Type::Group(group) => replace_self_type_references(&mut group.elem, item_name),
        Type::Paren(paren) => replace_self_type_references(&mut paren.elem, item_name),
        Type::Path(path) => replace_self_type_references_in_path(path, item_name),
        Type::Ptr(ptr) => replace_self_type_references(&mut ptr.elem, item_name),
        Type::Reference(reference) => replace_self_type_references(&mut reference.elem, item_name),
        Type::Slice(slice) => replace_self_type_references(&mut slice.elem, item_name),
        Type::Tuple(tuple) => {
            for element in &mut tuple.elems {
                replace_self_type_references(element, item_name);
            }
        }
        Type::BareFn(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::TraitObject(_)
        | Type::Verbatim(_) => {}
        _ => {}
    }
}

fn replace_self_type_references_in_path(path: &mut syn::TypePath, item_name: &str) {
    if path.qself.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == item_name
    {
        path.path.segments[0].ident = Ident::new("Self", Span::call_site());
        return;
    }

    for segment in &mut path.path.segments {
        if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
            for argument in &mut arguments.args {
                if let GenericArgument::Type(ty) = argument {
                    replace_self_type_references(ty, item_name);
                }
            }
        }
    }
}

fn render_variant(
    item: &RustItemPlan,
    variant: &RustVariantPlan,
    default_variant: Option<&str>,
) -> Result<TokenStream, RustSourceEmitError> {
    let ident = parse_variant_ident(item, variant)?;
    let default_attr = if default_variant == Some(variant.rust_name.as_str()) {
        quote!(#[default])
    } else {
        TokenStream::new()
    };
    if let Some(payload_type) = &variant.payload_type {
        let ty = syn::parse_str::<Type>(payload_type).map_err(|source| {
            RustSourceEmitError::FieldType {
                item_name: item.rust_name.clone(),
                field_name: variant.rust_name.clone(),
                rust_type: payload_type.clone(),
                source,
            }
        })?;
        Ok(quote!(#ident(#ty),))
    } else if let Some(discriminant) = variant.discriminant {
        let expr = render_discriminant_expr(item, variant, discriminant)?;
        Ok(quote!(#default_attr #ident = #expr,))
    } else {
        Ok(quote!(#default_attr #ident,))
    }
}

fn render_raw_enum_const(
    item: &RustItemPlan,
    variant: &RustVariantPlan,
) -> Option<Result<TokenStream, RustSourceEmitError>> {
    let discriminant = variant.discriminant?;
    Some((|| {
        let const_name = variant.rust_name.to_shouty_snake_case();
        let const_ident = syn::parse_str::<Ident>(&const_name).map_err(|source| {
            RustSourceEmitError::VariantIdent {
                item_name: item.rust_name.clone(),
                source_name: variant.source_name.clone(),
                identifier: const_name.clone(),
                source,
            }
        })?;
        let expr = render_discriminant_expr(item, variant, discriminant)?;
        Ok(quote!(pub const #const_ident: Self = Self(#expr);))
    })())
}

fn render_raw_enum_variant_entries(
    item: &RustItemPlan,
) -> Result<Vec<TokenStream>, RustSourceEmitError> {
    item.variants
        .iter()
        .filter_map(|variant| {
            variant
                .discriminant
                .map(|discriminant| (variant, discriminant))
        })
        .map(|(variant, discriminant)| {
            let expr = render_discriminant_expr(item, variant, discriminant)?;
            let source_name = LitStr::new(&variant.source_name, Span::call_site());
            Ok(quote!((#expr, #source_name)))
        })
        .collect()
}

fn render_raw_enum_string_impls(
    item: &RustItemPlan,
    ident: &Ident,
) -> Result<TokenStream, RustSourceEmitError> {
    if item.variants.is_empty() || !variant_source_names_are_unique(item) {
        return Ok(TokenStream::new());
    }
    let Some(raw_conversion) = &item.raw_conversion else {
        return Ok(TokenStream::new());
    };
    let raw_ty = syn::parse_str::<Type>(&raw_conversion.raw_type).map_err(|source| {
        RustSourceEmitError::RawConversionType {
            item_name: item.rust_name.clone(),
            raw_type: raw_conversion.raw_type.clone(),
            source,
        }
    })?;

    Ok(quote! {
        impl<'a> ::core::convert::TryFrom<&'a str> for #ident {
            type Error = &'a str;

            fn try_from(value: &'a str) -> Result<Self, &'a str> {
                Self::VARIANTS
                    .iter()
                    .find_map(|(raw, name)| (*name == value).then_some(Self(*raw)))
                    .ok_or(value)
            }
        }

        impl ::core::str::FromStr for #ident {
            type Err = ::std::string::String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::VARIANTS
                    .iter()
                    .find_map(|(raw, name)| (*name == value).then_some(Self(*raw)))
                    .or_else(|| value.parse::<#raw_ty>().ok().map(Self))
                    .ok_or_else(|| value.to_owned())
            }
        }

        impl ::core::fmt::Display for #ident {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match (*self).as_str() {
                    Some(name) => formatter.write_str(name),
                    None => ::core::fmt::Display::fmt(&self.0, formatter),
                }
            }
        }
    })
}

fn render_discriminant_expr(
    item: &RustItemPlan,
    variant: &RustVariantPlan,
    discriminant: i32,
) -> Result<syn::Expr, RustSourceEmitError> {
    let literal = grouped_i32_literal(discriminant);
    syn::parse_str::<syn::Expr>(&literal).map_err(|source| {
        RustSourceEmitError::VariantDiscriminant {
            item_name: item.rust_name.clone(),
            variant_name: variant.rust_name.clone(),
            discriminant,
            source,
        }
    })
}

fn render_raw_conversion_impls(
    item: &RustItemPlan,
    ident: &Ident,
) -> Result<TokenStream, RustSourceEmitError> {
    let Some(raw_conversion) = &item.raw_conversion else {
        return Ok(TokenStream::new());
    };
    let raw_ty = syn::parse_str::<Type>(&raw_conversion.raw_type).map_err(|source| {
        RustSourceEmitError::RawConversionType {
            item_name: item.rust_name.clone(),
            raw_type: raw_conversion.raw_type.clone(),
            source,
        }
    })?;
    let mut into_arms = Vec::new();
    let mut try_arms = Vec::new();
    for variant in &item.variants {
        let Some(discriminant) = variant.discriminant else {
            return Ok(TokenStream::new());
        };
        let variant_ident = parse_variant_ident(item, variant)?;
        let expr = render_discriminant_expr(item, variant, discriminant)?;
        into_arms.push(quote!(#ident::#variant_ident => #expr,));
        try_arms.push(quote!(#expr => Ok(Self::#variant_ident),));
    }

    Ok(quote! {
        impl From<#ident> for #raw_ty {
            fn from(value: #ident) -> Self {
                match value {
                    #(#into_arms)*
                }
            }
        }

        impl ::core::convert::TryFrom<#raw_ty> for #ident {
            type Error = #raw_ty;

            fn try_from(value: #raw_ty) -> Result<Self, #raw_ty> {
                match value {
                    #(#try_arms)*
                    _ => Err(value),
                }
            }
        }
    })
}

fn default_enum_variant(item: &RustItemPlan) -> Option<&RustVariantPlan> {
    item.variants
        .iter()
        .find(|variant| is_semantic_default_variant(variant))
        .or_else(|| {
            item.variants
                .iter()
                .find(|variant| variant.discriminant == Some(0))
        })
        .or_else(|| item.variants.first())
}

fn default_sum_variant(item: &RustItemPlan) -> Option<&RustVariantPlan> {
    item.variants
        .iter()
        .filter(|variant| variant.payload_type.is_none())
        .find(|variant| {
            matches!(
                variant.rust_name.as_str(),
                "None" | "Invalid" | "Disabled" | "Point"
            ) || variant.rust_name.ends_with("None")
                || variant.rust_name.ends_with("Invalid")
                || variant.rust_name.ends_with("Disabled")
        })
        .or_else(|| {
            item.variants
                .iter()
                .find(|variant| variant.payload_type.is_none())
        })
}

fn is_semantic_default_variant(variant: &RustVariantPlan) -> bool {
    let rust_name = variant.rust_name.as_str();
    let source_name = variant.source_name.as_str();
    matches!(rust_name, "None" | "Invalid" | "Disabled")
        || rust_name.ends_with("None")
        || rust_name.ends_with("Invalid")
        || rust_name.ends_with("Disabled")
        || source_name
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|part| matches!(part, "None" | "Invalid" | "Disabled"))
}

fn render_string_conversion_impls(
    item: &RustItemPlan,
    ident: &Ident,
) -> Result<TokenStream, RustSourceEmitError> {
    if item.variants.is_empty() || !variant_source_names_are_unique(item) {
        return Ok(TokenStream::new());
    }

    let mut as_str_arms = Vec::new();
    let mut try_from_arms = Vec::new();
    for variant in &item.variants {
        let variant_ident = parse_variant_ident(item, variant)?;
        let source_name = LitStr::new(&variant.source_name, Span::call_site());
        as_str_arms.push(quote!(Self::#variant_ident => #source_name,));
        try_from_arms.push(quote!(#source_name => Ok(Self::#variant_ident),));
    }

    Ok(quote! {
        impl #ident {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    #(#as_str_arms)*
                }
            }
        }

        impl AsRef<str> for #ident {
            fn as_ref(&self) -> &str {
                (*self).as_str()
            }
        }

        impl<'a> ::core::convert::TryFrom<&'a str> for #ident {
            type Error = &'a str;

            fn try_from(value: &'a str) -> Result<Self, &'a str> {
                match value {
                    #(#try_from_arms)*
                    _ => Err(value),
                }
            }
        }

        impl ::core::str::FromStr for #ident {
            type Err = ::std::string::String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from(value).map_err(str::to_owned)
            }
        }

        impl ::core::fmt::Display for #ident {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str((*self).as_str())
            }
        }
    })
}

fn variant_source_names_are_unique(item: &RustItemPlan) -> bool {
    let mut names = std::collections::BTreeSet::new();
    item.variants
        .iter()
        .all(|variant| names.insert(variant.source_name.as_str()))
}

fn grouped_i32_literal(value: i32) -> String {
    grouped_decimal_literal(&value.to_string())
}

fn typed_range_bound_literal(value: &str, value_type: &str) -> String {
    format!("{}{value_type}", grouped_decimal_literal(value))
}

fn grouped_decimal_literal(value: &str) -> String {
    let Some(digits) = value.strip_prefix('-') else {
        return group_unsigned_decimal_literal(value).unwrap_or_else(|| value.to_owned());
    };
    group_unsigned_decimal_literal(digits)
        .map(|grouped| format!("-{grouped}"))
        .unwrap_or_else(|| value.to_owned())
}

fn group_unsigned_decimal_literal(digits: &str) -> Option<String> {
    if digits.len() <= 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let mut reversed = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, byte) in digits.bytes().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push('_');
        }
        reversed.push(char::from(byte));
    }
    Some(reversed.chars().rev().collect())
}

fn parse_variant_ident(
    item: &RustItemPlan,
    variant: &RustVariantPlan,
) -> Result<Ident, RustSourceEmitError> {
    syn::parse_str::<Ident>(&variant.rust_name).map_err(|source| {
        RustSourceEmitError::VariantIdent {
            item_name: item.rust_name.clone(),
            source_name: variant.source_name.clone(),
            identifier: variant.rust_name.clone(),
            source,
        }
    })
}

fn parse_item_ident(identifier: &str, source_name: &str) -> Result<Ident, syn::Error> {
    syn::parse_str::<Ident>(identifier)
        .map_err(|source| syn::Error::new(source.span(), format!("{source_name}: {source}")))
}

#[cfg(test)]
mod tests {
    use nw_objectstream::type_uuid::type_ids;
    use serde_json::json;
    use uuid::uuid;

    use crate::compiler::SerializeContextCompiler;
    use crate::ir::{
        SerializeCodegenField as IrField, SerializeCodegenItem as IrItem,
        SerializeCodegenItemKind as IrItemKind, SerializeCodegenUnit as IrUnit,
    };
    use crate::model::SerializeContextModel;
    use crate::role::ReflectedTypeRole;
    use crate::rust::enum_plan::RustEnumRawConversionPlan;
    use crate::rust::identity::{RustTypeIdentityKind, RustTypeIdentityPlan};
    use crate::rust::item_plan::{
        RustCodegenUnit, RustFieldPlan, RustIntegerRangePlan, RustItemKind, RustItemPlan,
        RustUnresolvedTypePlan,
    };
    use crate::rust::plan::RustCodegenPlanner;
    use crate::types::{MapKind, ResolvedType, ScalarType, SequenceKind};

    use super::*;

    #[test]
    fn emits_serde_renames_for_reflected_field_names() {
        let item_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = RustCodegenUnit {
            items: vec![RustItemPlan {
                source_type_id: item_id,
                source_name: "AppearanceData".to_owned(),
                is_reflected_base: false,
                is_slot_owner: false,
                has_layout_family_descendants: false,
                is_bevy_component: false,
                file_stem_override: None,
                scope_path: Vec::new(),
                family_scope_path: Vec::new(),
                rust_name: "AppearanceData".to_owned(),
                kind: RustItemKind::Struct,
                identity: RustTypeIdentityPlan::az_type_info(
                    item_id,
                    Some("AppearanceData".to_owned()),
                ),
                repr: None,
                raw_conversion: None,
                derives: vec![
                    "AzTypeInfo".to_owned(),
                    "Debug".to_owned(),
                    "Default".to_owned(),
                    "Clone".to_owned(),
                    "Serialize".to_owned(),
                    "Deserialize".to_owned(),
                ],
                rtti_bases: Vec::new(),
                fields: vec![RustFieldPlan {
                    source_name: "isSkin".to_owned(),
                    rust_name: "is_skin".to_owned(),
                    source_type_id: type_ids::BOOL,
                    rust_type: "bool".to_owned(),
                    unresolved_type: None,
                    integer_range: None,
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                }],
                variants: Vec::new(),
            }],
        };

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("Rust source");

        assert!(source.contains("#[serde(rename = \"isSkin\", default)]"));
        assert!(source.contains("pub is_skin: bool,"));
    }

    #[test]
    fn integrated_structs_with_supported_fields_derive_marshaler() {
        let item_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = IrUnit {
            items: vec![IrItem {
                source_type_id: item_id,
                source_name: "DyeData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: IrItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![scalar_field("m_rColorId", type_ids::U8, ScalarType::U8)],
                variants: Vec::new(),
            }],
        };

        let rust_unit =
            RustCodegenPlanner::plan_codegen_unit(&unit, &crate::CodegenContext::inline());
        let project = RustSourceEmitter::emit_integrated_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("integrated project");
        let source = project
            .files
            .iter()
            .find(|file| file.source.contains("pub struct DyeData"))
            .expect("DyeData source")
            .source
            .as_str();

        assert!(source.contains("use gridmate::Marshaler;"));
        assert!(source.contains("Marshaler,"));
        assert!(source.contains("#[serde(rename = \"m_rColorId\", default)]"));
    }

    #[test]
    fn emits_model_structs_and_enums_with_az_rtti_identity() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "Example::CounterComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [{
                        "$id": 11,
                        "name": "m_count",
                        "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                        "offset": "4",
                        "flags": 0,
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "22222222-2222-2222-2222-222222222222",
                    {
                        "$id": 20,
                        "name": "Mode",
                        "attributes": [[1, {
                            "$id": 21,
                            "attributeId": 1,
                            "attributeName": "EnumValue",
                            "value": {
                                "kind": "enumConstant",
                                "valueI32": 7,
                                "description": "Enabled"
                            }
                        }]]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "22222222-2222-2222-2222-222222222222": type_ids::U8.hyphenated().to_string()
            }
        }));
        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("use az_derive::AzTypeInfo;"));
        assert!(source.contains("use gridmate::Marshaler;"));
        assert!(!source.contains("use bevy::prelude::Reflect;"));
        assert!(source.contains("bevy::prelude::Reflect"));
        assert!(source.contains("AzTypeInfo"));
        assert!(source.contains("Debug"));
        assert!(source.contains("Default"));
        assert!(source.contains("Clone"));
        assert!(source.contains("PartialEq"));
        assert!(source.contains("Eq"));
        assert!(source.contains("Hash"));
        assert!(source.contains("name = \"Example::CounterComponent\""));
        assert!(source.contains("#[az_type_info("));
        assert!(source.contains("name = \"Example::CounterComponent\""));
        assert!(source.contains("uuid = \"11111111-1111-1111-1111-111111111111\""));
        assert!(source.contains("pub struct CounterComponent"));
        assert!(source.contains("pub count: u32"));
        assert!(source.contains("Copy"));
        assert!(source.contains("pub enum Mode"));
        assert!(source.contains("#[repr(u8)]"));
        assert!(source.contains("#[serde(try_from = \"u8\", into = \"u8\")]"));
        assert!(source.contains("#[default]\n    Enabled = 7"));
        assert!(source.contains("Enabled = 7"));
        assert!(source.contains("impl From<Mode> for u8"));
        assert!(source.contains("impl ::core::convert::TryFrom<u8> for Mode"));
        assert!(source.contains("pub const fn as_str(self) -> &'static str"));
        assert!(source.contains("impl AsRef<str> for Mode"));
        assert!(source.contains("impl<'a> ::core::convert::TryFrom<&'a str> for Mode"));
        assert!(source.contains("impl ::core::str::FromStr for Mode"));
        assert!(source.contains("impl ::core::fmt::Display for Mode"));
        assert!(source.contains("Marshaler"));
        assert!(!source.contains("impl ::gridmate::serialize::marshaler::Marshaler for Mode"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn formats_top_level_spacing_between_imports_types_and_impls() {
        let source = rustfmt_source(
            "use crate::a::A;\n\
             use crate::b::B;\n\
             #[derive(Debug)]\n\
             pub struct First;\n\
             impl First {}\n\
             pub struct Second;\n",
        )
        .expect("formatted Rust source");

        assert!(source.contains("a::A"), "{source}");
        assert!(source.contains("b::B"), "{source}");
        assert!(
            source.contains("\n\n#[derive(Debug)]\npub struct First;\n\n"),
            "{source}"
        );
        assert!(source.contains("impl First {}\n\n"), "{source}");
        assert!(source.contains("pub struct Second;"), "{source}");
        assert!(source.ends_with('\n'));
    }

    #[test]
    fn formats_multiline_attributes_as_part_of_their_item() {
        let source = rustfmt_source(
            "#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]\n\
             pub struct ReflectedData {\n\
                 pub value: u8,\n\
             }\n",
        )
        .expect("formatted Rust source");

        assert!(source.contains(")]\npub struct ReflectedData"), "{source}");
        assert!(
            !source.contains(")]\n\npub struct ReflectedData"),
            "{source}"
        );
    }

    #[test]
    fn formats_multiline_derives_with_following_outer_attributes() {
        let source = rustfmt_source(
            "#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect)]\n\
             #[repr(u8)]\n\
             #[serde(try_from = \"u8\", into = \"u8\")]\n\
             pub enum FactionType {\n\
                 None = 0,\n\
             }\n",
        )
        .expect("formatted Rust source");

        assert!(source.contains(")]\n#[repr(u8)]"), "{source}");
        assert!(!source.contains(")]\n\n#[repr(u8)]"), "{source}");
        assert!(
            source.contains("#[serde(try_from = \"u8\", into = \"u8\")]\npub enum FactionType"),
            "{source}"
        );
        assert!(
            !source.contains("#[serde(try_from = \"u8\", into = \"u8\")]\n\npub enum FactionType"),
            "{source}"
        );
    }

    #[test]
    fn standalone_project_keeps_multiline_derives_attached_to_item() {
        let unit = RustCodegenUnit {
            items: vec![RustItemPlan {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::ReflectedData".to_owned(),
                is_reflected_base: false,
                is_slot_owner: false,
                has_layout_family_descendants: false,
                is_bevy_component: false,
                file_stem_override: None,
                scope_path: vec!["example".to_owned()],
                family_scope_path: vec!["example".to_owned()],
                rust_name: "ReflectedData".to_owned(),
                kind: RustItemKind::Struct,
                identity: RustTypeIdentityPlan::az_rtti(
                    uuid!("11111111-1111-1111-1111-111111111111"),
                    Some("Example::ReflectedData".to_owned()),
                ),
                repr: None,
                raw_conversion: None,
                derives: vec![
                    "Debug".to_owned(),
                    "Default".to_owned(),
                    "Clone".to_owned(),
                    "PartialEq".to_owned(),
                    "serde::Serialize".to_owned(),
                    "serde::Deserialize".to_owned(),
                    "bevy_reflect::Reflect".to_owned(),
                ],
                rtti_bases: Vec::new(),
                fields: vec![RustFieldPlan {
                    source_name: "m_value".to_owned(),
                    rust_name: "value".to_owned(),
                    source_type_id: type_ids::U8,
                    rust_type: "u8".to_owned(),
                    unresolved_type: None,
                    integer_range: None,
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                }],
                variants: Vec::new(),
            }],
        };

        let project =
            RustSourceEmitter::emit_standalone_project(&unit, &crate::CodegenContext::inline())
                .expect("standalone Rust project");
        let source = &project
            .files
            .iter()
            .find(|file| file.path == "src/types/example/reflected_data.rs")
            .expect("reflected data module")
            .source;

        assert!(source.contains(")]\npub struct ReflectedData"), "{source}");
        assert!(
            !source.contains(")]\n\npub struct ReflectedData"),
            "{source}"
        );
    }

    #[test]
    fn standalone_project_keeps_multiline_enum_derives_attached_to_attrs() {
        let unit = RustCodegenUnit {
            items: vec![RustItemPlan {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "FactionType".to_owned(),
                is_reflected_base: false,
                is_slot_owner: false,
                has_layout_family_descendants: false,
                is_bevy_component: false,
                file_stem_override: Some("faction_type".to_owned()),
                scope_path: Vec::new(),
                family_scope_path: Vec::new(),
                rust_name: "FactionType".to_owned(),
                kind: RustItemKind::Enum,
                identity: RustTypeIdentityPlan::az_type_info(
                    uuid!("11111111-1111-1111-1111-111111111111"),
                    Some("FactionType".to_owned()),
                ),
                repr: Some("u8".to_owned()),
                raw_conversion: Some(RustEnumRawConversionPlan {
                    raw_type: "u8".to_owned(),
                }),
                derives: vec![
                    "Debug".to_owned(),
                    "Default".to_owned(),
                    "Clone".to_owned(),
                    "Copy".to_owned(),
                    "PartialEq".to_owned(),
                    "Eq".to_owned(),
                    "PartialOrd".to_owned(),
                    "Ord".to_owned(),
                    "Hash".to_owned(),
                    "serde::Serialize".to_owned(),
                    "serde::Deserialize".to_owned(),
                    "bevy_reflect::Reflect".to_owned(),
                ],
                rtti_bases: Vec::new(),
                fields: Vec::new(),
                variants: vec![
                    RustVariantPlan {
                        source_name: "None".to_owned(),
                        rust_name: "None".to_owned(),
                        discriminant: Some(0),
                        payload_type: None,
                    },
                    RustVariantPlan {
                        source_name: "Faction1".to_owned(),
                        rust_name: "Faction1".to_owned(),
                        discriminant: Some(1),
                        payload_type: None,
                    },
                ],
            }],
        };

        let project =
            RustSourceEmitter::emit_standalone_project(&unit, &crate::CodegenContext::inline())
                .expect("standalone Rust project");
        let source = &project
            .files
            .iter()
            .find(|file| file.path == "src/types/faction_type.rs")
            .expect("faction type module")
            .source;

        assert!(source.contains(")]\n#[repr(u8)]"), "{source}");
        assert!(!source.contains(")]\n\n#[repr(u8)]"), "{source}");
        assert!(
            source.contains("#[serde(try_from = \"u8\", into = \"u8\")]\npub enum FactionType"),
            "{source}"
        );
    }

    #[test]
    fn standalone_project_emits_fixed_opaque_bytes_as_az_collection_type() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let unit = IrUnit {
            items: vec![IrItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::OpaquePayload".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: IrItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![IrField {
                    source_name: "payload".to_owned(),
                    source_type_id: missing_id,
                    resolved_type: ResolvedType::Unknown {
                        type_id: missing_id,
                        reason: "type id is not present in SerializeContext".to_owned(),
                    },
                    data_size: Some(16),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                }],
                variants: Vec::new(),
            }],
        };
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let payload = project
            .files
            .iter()
            .find(|file| file.path == "src/types/example/opaque_payload.rs")
            .expect("opaque payload module");

        assert!(
            !project
                .files
                .iter()
                .any(|file| file.path == "src/az/collection.rs")
        );
        assert!(payload.source.contains("pub payload: [u8; 16]"));
        assert!(!payload.source.contains("Typeaaaaaaaa"));
    }

    #[test]
    fn standalone_project_keeps_duplicate_leaf_names_in_distinct_modules() {
        let catalog_item_id = uuid!("11111111-1111-1111-1111-111111111111");
        let runtime_item_id = uuid!("22222222-2222-2222-2222-222222222222");
        let holder_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unit = IrUnit {
            items: vec![
                fixture_item(
                    catalog_item_id,
                    "Catalog::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![scalar_field(
                        "m_catalogId",
                        uuid!("44444444-4444-4444-4444-444444444444"),
                        ScalarType::U32,
                    )],
                ),
                fixture_item(
                    runtime_item_id,
                    "Runtime::Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![scalar_field(
                        "m_instanceId",
                        uuid!("55555555-5555-5555-5555-555555555555"),
                        ScalarType::U32,
                    )],
                ),
                fixture_item(
                    holder_id,
                    "Runtime::Holder",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![
                        named_field("m_catalogItem", catalog_item_id, "Catalog::Item"),
                        named_field("m_runtimeItem", runtime_item_id, "Runtime::Item"),
                    ],
                ),
            ],
        };
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let catalog_item = project
            .files
            .iter()
            .find(|file| file.path == "src/types/catalog/item.rs")
            .expect("catalog item file");
        let runtime_item = project
            .files
            .iter()
            .find(|file| file.path == "src/types/runtime/item.rs")
            .expect("runtime item file");
        let holder = project
            .files
            .iter()
            .find(|file| file.path == "src/types/runtime/holder.rs")
            .expect("holder file");

        assert!(catalog_item.source.contains("pub struct Item"));
        assert!(runtime_item.source.contains("pub struct Item"));
        assert!(!catalog_item.source.contains("Item11111111"));
        assert!(!runtime_item.source.contains("Item22222222"));
        assert!(
            holder
                .source
                .contains("pub catalog_item: crate::types::catalog::item::Item")
        );
        assert!(
            holder
                .source
                .contains("pub runtime_item: crate::types::runtime::item::Item")
        );
        assert!(!holder.source.contains("use crate::types::Item"));
    }

    #[test]
    fn standalone_project_keeps_same_leaf_name_when_family_module_owns_name() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = IrUnit {
            items: vec![
                fixture_item(
                    item_family_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    item_record_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    ammo_id,
                    "Ammo",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(item_family_id, "Item")],
                ),
            ],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let item_family = project
            .files
            .iter()
            .find(|file| file.path == "src/types/item/mod.rs")
            .expect("item family module");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let item_record = project
            .files
            .iter()
            .find(|file| file.path == "src/types/item/item_type.rs")
            .unwrap_or_else(|| panic!("item record file; generated files:\n{paths}"));

        assert!(item_family.source.contains("pub struct Item"));
        assert!(item_record.source.contains("pub struct Item"));
        assert!(!item_family.source.contains("ItemB9F3747D"));
        assert!(!item_record.source.contains("ItemA6D8DB05"));
        assert!(
            !item_family
                .source
                .contains("pub use self::item_type::Item;")
        );
    }

    #[test]
    fn duplicate_leaf_name_derive_pruning_uses_scoped_references() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let slot_id = uuid!("22222222-2222-2222-2222-222222222222");
        let version_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unit = IrUnit {
            items: vec![
                fixture_item(
                    item_family_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![scalar_field(
                        "m_instanceId",
                        uuid!("44444444-4444-4444-4444-444444444444"),
                        ScalarType::U64,
                    )],
                ),
                fixture_item(
                    item_record_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_version", version_id, "ItemVersionData")],
                ),
                fixture_item(
                    version_id,
                    "ItemVersionData",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![IrField {
                        source_name: "m_childItemLinks".to_owned(),
                        source_type_id: uuid!("55555555-5555-5555-5555-555555555555"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::String)),
                            value: Box::new(ResolvedType::Optional {
                                value: Box::new(ResolvedType::Named {
                                    type_id: item_record_id,
                                    source_name: "Item".to_owned(),
                                }),
                            }),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    }],
                ),
                fixture_item(
                    ammo_id,
                    "Ammo",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(item_family_id, "Item")],
                ),
                fixture_item(
                    slot_id,
                    "ItemContainerSlot",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![named_field("m_item", item_family_id, "Item")],
                ),
            ],
        };

        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let slot = project
            .files
            .iter()
            .find(|file| file.source.contains("pub struct ItemContainerSlot"))
            .expect("slot type");
        let item_record = project
            .files
            .iter()
            .find(|file| file.source.contains("pub version:"))
            .expect("recursive duplicate item type");

        assert!(slot.source.contains("pub item: crate::types::"));
        assert!(slot.source.contains("::Item"));
        assert!(slot.source.contains("serde::Serialize"));
        assert!(slot.source.contains("serde::Deserialize"));
        assert!(item_record.source.contains("serde::Serialize"));
        assert!(item_record.source.contains("serde::Deserialize"));
    }

    #[test]
    fn integrated_project_uses_semantic_names_for_same_leaf_family_records() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let unit = IrUnit {
            items: vec![
                fixture_item(
                    item_family_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    item_record_id,
                    "Item",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    ammo_id,
                    "Ammo",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(item_family_id, "Item")],
                ),
            ],
        };

        let rust_unit = RustCodegenPlanner::default()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let project = RustSourceEmitter::emit_integrated_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("integrated project");
        let root = project
            .files
            .iter()
            .find(|file| file.path == "mod.rs")
            .expect("root generated module");
        let item_family = project
            .files
            .iter()
            .find(|file| file.path == "item/mod.rs")
            .expect("item family module");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let item_record = project
            .files
            .iter()
            .find(|file| file.path == "item/item_type.rs")
            .unwrap_or_else(|| panic!("item record file; generated files:\n{paths}"));

        assert!(item_family.source.contains("pub struct Item"));
        assert!(item_record.source.contains("pub struct Item"));
        assert!(!item_family.source.contains("ItemB9F3747D"));
        assert!(!item_record.source.contains("ItemA6D8DB05"));
        assert!(root.source.contains("pub use self::item::{Ammo, Item};"));
        assert!(
            !item_family
                .source
                .contains("pub use self::item_type::Item;")
        );
    }

    #[test]
    fn standalone_project_does_not_export_duplicate_descendant_names_from_ancestors() {
        let unit = RustCodegenUnit {
            items: vec![
                standalone_fixture_item(
                    uuid!("11111111-1111-1111-1111-111111111111"),
                    "AudioProxyComponent",
                    "AudioProxyComponent",
                    vec!["components"],
                    vec!["components"],
                    Some("audio_proxy_component"),
                    false,
                ),
                standalone_fixture_item(
                    uuid!("22222222-2222-2222-2222-222222222222"),
                    "AudioProxyComponent",
                    "AudioProxyComponent",
                    vec!["components", "faceted_components", "audio_proxy_component"],
                    vec!["components", "faceted_components", "audio_proxy_component"],
                    Some("audio_proxy_component"),
                    true,
                ),
                standalone_fixture_item(
                    uuid!("33333333-3333-3333-3333-333333333333"),
                    "AudioProxyComponentClientFacet",
                    "AudioProxyComponentClientFacet",
                    vec!["components", "faceted_components", "audio_proxy_component"],
                    vec!["components", "faceted_components", "audio_proxy_component"],
                    Some("client_facet"),
                    false,
                ),
            ],
        };

        let project =
            RustSourceEmitter::emit_standalone_project(&unit, &crate::CodegenContext::inline())
                .expect("standalone project");
        let root = project
            .files
            .iter()
            .find(|file| file.path == "src/types/mod.rs")
            .expect("root types module");
        let components = project
            .files
            .iter()
            .find(|file| file.path == "src/types/components/mod.rs")
            .expect("components module");
        let faceted_components = project
            .files
            .iter()
            .find(|file| file.path == "src/types/components/faceted_components/mod.rs")
            .expect("faceted components module");

        assert!(!root.source.contains("AudioProxyComponent,"));
        assert!(!root.source.contains("::AudioProxyComponent;"));
        assert!(!components.source.contains("AudioProxyComponent,"));
        assert!(!components.source.contains("::AudioProxyComponent;"));
        assert!(
            faceted_components
                .source
                .contains("pub use self::audio_proxy_component::{")
        );
        assert!(faceted_components.source.contains("AudioProxyComponent,"));
        assert!(
            faceted_components
                .source
                .contains("AudioProxyComponentClientFacet")
        );
    }

    #[test]
    fn standalone_project_emits_plain_sequences_for_non_native_map_and_set_shapes() {
        let unit = IrUnit {
            items: vec![IrItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::CollectionData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: IrItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![
                    IrField {
                        source_name: "m_floatLookup".to_owned(),
                        source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::F32)),
                            value: Box::new(ResolvedType::Scalar(ScalarType::String)),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    IrField {
                        source_name: "m_floatSet".to_owned(),
                        source_type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                        resolved_type: ResolvedType::Sequence {
                            kind: SequenceKind::Set,
                            element: Box::new(ResolvedType::Scalar(ScalarType::F32)),
                            capacity: None,
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    IrField {
                        source_name: "m_nameLookup".to_owned(),
                        source_type_id: uuid!("44444444-4444-4444-4444-444444444444"),
                        resolved_type: ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::String)),
                            value: Box::new(ResolvedType::Scalar(ScalarType::U32)),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
                variants: Vec::new(),
            }],
        };
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let payload = project
            .files
            .iter()
            .find(|file| file.path == "src/types/example/collection_data.rs")
            .expect("collection data module");

        assert!(
            payload
                .source
                .contains("pub float_lookup: Vec<(f32, String)>")
        );
        assert!(payload.source.contains("pub float_set: Vec<f32>"));
        assert!(
            payload
                .source
                .contains("pub name_lookup: std::collections::HashMap<String, u32>")
        );
        assert!(!payload.source.contains("AzMapEntries"));
        assert!(!payload.source.contains("AzSetEntries"));
        assert!(
            !project
                .files
                .iter()
                .any(|file| file.path == "src/az/collection.rs")
        );
    }

    #[test]
    fn emits_unique_enum_variant_names_from_reflected_labels() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "33333333-3333-3333-3333-333333333333",
                    {
                        "$id": 20,
                        "name": "StateChangeType",
                        "attributes": [
                            [1, {
                                "$id": 21,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 0,
                                    "description": "On Enter"
                                }
                            }],
                            [2, {
                                "$id": 22,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 1,
                                    "description": "On Exit"
                                }
                            }],
                            [3, {
                                "$id": 23,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 2,
                                    "description": "On Enter And On Exit"
                                }
                            }]
                        ]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "33333333-3333-3333-3333-333333333333": type_ids::U32.hyphenated().to_string()
            }
        }));
        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("OnEnter = 0"));
        assert!(source.contains("OnExit = 1"));
        assert!(source.contains("OnEnterAndOnExit = 2"));
        assert!(source.contains("Self::OnExit => \"On Exit\""));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn widens_enum_repr_when_reflected_values_do_not_fit_underlying_type() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "44444444-4444-4444-4444-444444444444",
                    {
                        "$id": 20,
                        "name": "ConversationType",
                        "attributes": [
                            [1, {
                                "$id": 21,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 544104704,
                                    "description": "Conversation"
                                }
                            }],
                            [2, {
                                "$id": 22,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 3,
                                    "description": "Community Objective"
                                }
                            }]
                        ]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "44444444-4444-4444-4444-444444444444": type_ids::U8.hyphenated().to_string()
            }
        }));
        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("#[repr(u32)]"));
        assert!(source.contains("impl From<ConversationType> for u32"));
        assert!(source.contains("544_104_704 => Ok(Self::Conversation)"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn emits_raw_enum_string_conversion_from_variant_table() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {},
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "55555555-5555-5555-5555-555555555555",
                    {
                        "$id": 20,
                        "name": "CharacterActionGridCellValue",
                        "attributes": [
                            [1, {
                                "$id": 21,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 1,
                                    "description": "TransitionAllowed"
                                }
                            }],
                            [2, {
                                "$id": 22,
                                "attributeId": 1,
                                "attributeName": "EnumValue",
                                "value": {
                                    "kind": "enumConstant",
                                    "valueI32": 1,
                                    "description": "Always"
                                }
                            }]
                        ]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "55555555-5555-5555-5555-555555555555": type_ids::U32.hyphenated().to_string()
            }
        }));
        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("pub struct CharacterActionGridCellValue(pub u32);"));
        assert!(source.contains("Marshaler"));
        assert!(source.contains("pub const VARIANTS: &[(u32, &str)]"));
        assert!(source.contains("(1, \"TransitionAllowed\")"));
        assert!(source.contains("(1, \"Always\")"));
        assert!(source.contains("Self::VARIANTS"));
        assert!(source.contains("impl ::core::str::FromStr for CharacterActionGridCellValue"));
        assert!(source.contains("value.parse::<u32>()"));
        assert!(source.contains("impl ::core::fmt::Display for CharacterActionGridCellValue"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn emits_component_roles_with_bevy_component_and_az_rtti_base_metadata() {
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
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("component plan");

        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzRtti);
        assert!(component.is_bevy_component);

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("use az_derive::AzRtti;"));
        assert!(!source.contains("AzComponent"));
        assert!(!source.contains("use bevy::prelude::{Component, Reflect};"));
        assert!(!source.contains("use bevy::prelude::Component;"));
        assert!(source.contains("use bevy::ecs::reflect::ReflectComponent;"));
        assert!(source.contains("bevy::prelude::Component"));
        assert!(source.contains("bevy::prelude::Reflect"));
        assert!(source.contains("#[az_rtti("));
        assert!(source.contains("name = \"Example::HealthComponent\""));
        assert!(source.contains("#[az_rtti(\"BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB\", Component)]"));
        assert!(!source.contains("AzTypeRegistration"));
        assert!(source.contains("HealthComponent"));
        assert!(source.contains("#[reflect(Component)]"));
        assert!(source.contains("pub struct HealthComponent"));
        assert!(source.contains("pub value: u32"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn integrated_unit_does_not_emit_root_registration_plugin() {
        let reflected_id = uuid!("11111111-1111-1111-1111-111111111111");
        let opaque_id = uuid!("22222222-2222-2222-2222-222222222222");
        let unit = RustCodegenUnit {
            items: vec![
                RustItemPlan {
                    source_type_id: reflected_id,
                    source_name: "Example::ReflectedComponent".to_owned(),
                    is_reflected_base: false,
                    is_slot_owner: false,
                    has_layout_family_descendants: false,
                    is_bevy_component: true,
                    file_stem_override: None,
                    scope_path: Vec::new(),
                    family_scope_path: Vec::new(),
                    rust_name: "ReflectedComponent".to_owned(),
                    kind: RustItemKind::Struct,
                    identity: RustTypeIdentityPlan::az_rtti(
                        reflected_id,
                        Some("Example::ReflectedComponent".to_owned()),
                    ),
                    repr: None,
                    raw_conversion: None,
                    derives: vec![
                        "Component".to_owned(),
                        "AzRtti".to_owned(),
                        "Debug".to_owned(),
                        "Clone".to_owned(),
                        "Reflect".to_owned(),
                    ],
                    rtti_bases: Vec::new(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                RustItemPlan {
                    source_type_id: opaque_id,
                    source_name: "Example::OpaqueComponent".to_owned(),
                    is_reflected_base: false,
                    is_slot_owner: false,
                    has_layout_family_descendants: false,
                    is_bevy_component: true,
                    file_stem_override: None,
                    scope_path: Vec::new(),
                    family_scope_path: Vec::new(),
                    rust_name: "OpaqueComponent".to_owned(),
                    kind: RustItemKind::Struct,
                    identity: RustTypeIdentityPlan::az_rtti(
                        opaque_id,
                        Some("Example::OpaqueComponent".to_owned()),
                    ),
                    repr: None,
                    raw_conversion: None,
                    derives: vec![
                        "Component".to_owned(),
                        "AzRtti".to_owned(),
                        "Debug".to_owned(),
                        "Clone".to_owned(),
                    ],
                    rtti_bases: Vec::new(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(!source.contains("use bevy::prelude::{App, Plugin};"));
        assert!(!source.contains("TypesPlugin"));
        assert!(!source.contains("impl Plugin"));
        assert!(!source.contains("app.register_type::<ReflectedComponent>();"));
        assert!(!source.contains("app.register_type::<OpaqueComponent>();"));
        assert!(!source.contains("Generated"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn emits_standalone_reflection_support_without_engine_dependencies() {
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
                    "name": "Example::TargetComponent",
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
                            "name": "m_entity",
                            "typeId": type_ids::ENTITY_ID.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 23,
                            "name": "m_asset",
                            "typeId": type_ids::AZ_DATA_ASSET_ID.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 24,
                            "name": "m_tag",
                            "typeId": type_ids::CRC32.hyphenated().to_string(),
                            "is_base_class": false
                        },
                        {
                            "$id": 25,
                            "name": "m_owner",
                            "typeId": type_ids::AZ_UUID.hyphenated().to_string(),
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
        let unit =
            RustCodegenPlanner::plan_standalone_model(&model, &crate::CodegenContext::inline());

        let source =
            RustSourceEmitter::emit_standalone_unit(&unit, &crate::CodegenContext::inline())
                .expect("emitted Rust source");

        assert!(!source.contains("az_derive"));
        assert!(!source.contains("#[az_component"));
        assert!(source.contains("use bevy_ecs::reflect::ReflectComponent;"));
        assert!(source.contains("pub mod az"));
        assert!(source.contains("pub struct Uuid(::uuid::Uuid);"));
        assert!(source.contains("pub use crate::az::uuid::Uuid;"));
        assert!(source.contains("pub use crate::az::uuid::Uuid as AzUuid;"));
        assert!(source.contains("pub trait AzRtti"));
        assert!(!source.contains("pub trait AzTypeInfo"));
        assert!(!source.contains("AzComponent"));
        assert!(source.contains("pub fn create_data(bytes: &[u8]) -> Self"));
        assert!(source.contains("pub fn combine(lhs: Self, rhs: Self) -> Self"));
        assert!(!source.contains("pub fn create_data(bytes: &[u8]) -> Uuid"));
        assert!(!source.contains("pub fn combine(lhs: Uuid, rhs: Uuid) -> Uuid"));
        assert!(source.contains("pub struct Crc32(pub u32);"));
        assert!(source.contains("pub struct AssetId"));
        assert!(source.contains("bevy_ecs::component::Component"));
        assert!(source.contains("bevy_reflect::Reflect"));
        assert!(source.contains("#[reflect(Component)]"));
        assert!(source.contains("pub entity: u64"));
        assert!(source.contains("pub asset: AzAssetId"));
        assert!(source.contains("pub tag: AzCrc32"));
        assert!(source.contains("pub owner: AzUuid"));
        assert!(source.contains("impl AzRtti for TargetComponent"));
        assert!(source.contains("const NAME: &'static str = \"Example::TargetComponent\""));
        assert!(source.contains("const TYPE_ID: AzUuid = AzUuid::from_u128"));
        assert!(source.contains("const BASE_TYPE_IDS: &'static [AzUuid]"));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn emits_ranged_integer_field_as_scalar_with_core_range_const() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 10,
                    "name": "Example::RangeComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [{
                        "$id": 11,
                        "name": "m_count",
                        "typeId": "33333333-3333-3333-3333-333333333333",
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [[
                "33333333-3333-3333-3333-333333333333",
                {
                    "$id": 20,
                    "typeId": "33333333-3333-3333-3333-333333333333",
                    "registeredTypeIds": ["33333333-3333-3333-3333-333333333333"],
                    "templatedArgumentCount": 1,
                    "templatedTypeIds": [type_ids::U16.hyphenated().to_string()],
                    "typeIdFoldTypeIds": null,
                    "specializedTypeId": "33333333-3333-3333-3333-333333333333",
                    "genericTypeId": "44444444-4444-4444-4444-444444444444",
                    "legacySpecializedTypeId": null,
                    "nonTypeTemplateArguments": {"values": [0, 65535]},
                    "classData": {
                        "$id": 21,
                        "name": "AZStd::ranged_int",
                        "typeId": "33333333-3333-3333-3333-333333333333",
                        "version": 0,
                        "doSave": null,
                        "dataConverter": null,
                        "editData": null,
                        "elements": [],
                        "attributes": []
                    },
                    "elements": [{
                        "$id": 22,
                        "name": "value",
                        "nameCrc": 0,
                        "typeId": type_ids::U16.hyphenated().to_string(),
                        "dataSize": "2",
                        "offset": "0",
                        "attributeOwnership": 0,
                        "flags": 0,
                        "is_pointer": false,
                        "is_base_class": false,
                        "no_default_value": false,
                        "is_dynamic_field": false,
                        "is_ui_element": false,
                        "genericClassInfo": null,
                        "editData": null,
                        "attributes": []
                    }]
                }
            ]],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let unit = RustCodegenPlanner::plan_model(&model, &crate::CodegenContext::inline());
        let component = unit
            .items
            .iter()
            .find(|item| item.source_type_id == uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component plan");

        assert_eq!(
            component.fields[0].integer_range,
            Some(RustIntegerRangePlan {
                rust_type: "::core::ops::RangeInclusive<u16>".to_owned(),
                value_type: "u16".to_owned(),
                start: "0".to_owned(),
                last: "65535".to_owned(),
            })
        );

        let source = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect("emitted Rust source");

        assert!(source.contains("pub count: u16"));
        assert!(source.contains("pub const COUNT_RANGE: ::core::ops::RangeInclusive<u16>"));
        assert!(source.contains(
            "pub const COUNT_RANGE: ::core::ops::RangeInclusive<u16> = 0u16..=65_535u16;"
        ));
        syn::parse_file(&source).expect("source should be parseable Rust");
    }

    #[test]
    fn standalone_project_places_namespace_before_component_family_and_uses_leaf_files() {
        let unit = namespace_and_base_family_fixture();
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone Rust project");
        let paths = project
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains("src/types/az_framework/components/input_system_component.rs"));
        assert!(paths.contains("src/types/az_framework/input_device_id.rs"));
        assert!(paths.contains("src/types/action_conditions/mod.rs"));
        assert!(paths.contains("src/types/action_conditions/action_condition_if_input.rs"));
        assert!(
            paths.contains("src/types/components/faceted_components/inventory_component/mod.rs")
        );
        assert!(paths.contains(
            "src/types/components/faceted_components/inventory_component/client_facet.rs"
        ));
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("src/types/components/az_framework"))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("src/types/components/faceted_components_"))
        );
        assert!(!paths.contains("src/types/facets/mod.rs"));
        assert!(!paths.contains("src/types/components/faceted_components/facets/mod.rs"));
        assert!(!paths.contains("src/types/facets/client_facets/inventory_client_facet.rs"));
        assert!(!paths.contains(
            "src/types/components/faceted_components/facets/client_facets/inventory_client_facet.rs"
        ));
        assert!(!paths.contains("src/types/global/action_condition.rs"));
        assert!(!paths.contains("src/types/action_conditions/action_condition_if_input/mod.rs"));

        let faceted_components = project
            .files
            .iter()
            .find(|file| file.path == "src/types/components/faceted_components/mod.rs")
            .expect("faceted components module");
        assert!(!faceted_components.source.contains("::*"));
        assert!(
            faceted_components
                .source
                .contains("pub use self::inventory_component::")
        );
    }

    #[test]
    fn standalone_project_does_not_emit_root_registration_plugin() {
        let unit = IrUnit {
            items: vec![fixture_item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "Example::HealthComponent",
                ReflectedTypeRole::AzComponent,
                false,
                vec![scalar_field(
                    "m_value",
                    uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"),
                    ScalarType::U32,
                )],
            )],
        };
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone Rust project");
        let root = project
            .files
            .iter()
            .find(|file| file.path == "src/types/mod.rs")
            .expect("types root module");
        let health = project
            .files
            .iter()
            .find(|file| file.path == "src/types/example/components/health_component.rs")
            .expect("component module");

        assert!(!root.source.contains("pub mod plugin;"));
        assert!(!root.source.contains("TypesPlugin"));
        assert!(
            !project
                .files
                .iter()
                .any(|file| file.path == "src/types/plugin.rs")
        );
        assert!(health.source.contains("pub struct HealthComponent"));
        assert!(!health.source.contains("app.register_type::<"));
    }

    #[test]
    fn integrated_project_emits_only_local_registration_functions() {
        let unit = IrUnit {
            items: vec![fixture_item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "Example::HealthComponent",
                ReflectedTypeRole::AzComponent,
                false,
                vec![scalar_field(
                    "m_value",
                    uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"),
                    ScalarType::U32,
                )],
            )],
        };
        let rust_unit = RustCodegenPlanner::default()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_integrated_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("integrated Rust project");
        let root = project
            .files
            .iter()
            .find(|file| file.path == "mod.rs")
            .expect("root module");
        let parent = project
            .files
            .iter()
            .find(|file| file.path == "example/components/mod.rs")
            .expect("component parent module");
        let health = project
            .files
            .iter()
            .find(|file| file.path == "example/components/health_component.rs")
            .expect("component module");

        for file in &project.files {
            assert!(!file.source.contains("impl Plugin"), "{}", file.path);
            assert!(!file.source.contains("add_plugins"), "{}", file.path);
        }
        assert!(!root.source.contains("pub fn register"));
        assert!(!parent.source.contains("pub fn register"));
        assert!(!parent.source.contains("health_component::register(app);"));
        assert!(!parent.source.contains("app.register_type::<"));
        assert!(health.source.contains("pub struct HealthComponent"));
        assert!(health.source.contains("pub fn register(app: &mut App)"));
        assert!(
            health
                .source
                .contains("app.register_type::<HealthComponent>();")
        );
        assert!(health.source.contains(
            "app.register_type_data::<HealthComponent, ::az_core::ReflectAzTypeInfo>();"
        ));
        assert!(
            health
                .source
                .contains("app.register_type_data::<HealthComponent, ::az_core::ReflectAzRtti>();")
        );
    }

    #[test]
    fn integrated_family_registration_includes_reflected_children() {
        let unit = namespace_and_base_family_fixture();
        let rust_unit = RustCodegenPlanner::default()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let project = RustSourceEmitter::emit_integrated_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("integrated Rust project");
        let parent = project
            .files
            .iter()
            .find(|file| file.path == "components/faceted_components/mod.rs")
            .expect("faceted components parent module");
        let inventory = project
            .files
            .iter()
            .find(|file| file.path == "components/faceted_components/inventory_component/mod.rs")
            .expect("inventory component module");
        let client_facet = project
            .files
            .iter()
            .find(|file| {
                file.path == "components/faceted_components/inventory_component/client_facet.rs"
            })
            .expect("inventory client facet module");

        assert!(parent.source.contains("pub fn register(app: &mut App)"));
        assert!(
            !parent
                .source
                .contains("inventory_component::register(app);")
        );
        assert!(
            parent
                .source
                .contains("app.register_type::<FacetedComponent>();")
        );
        assert!(inventory.source.contains("client_facet::register(app);"));
        assert!(
            inventory
                .source
                .contains("app.register_type::<InventoryComponent>();")
        );
        assert!(
            client_facet
                .source
                .contains("pub fn register(app: &mut App)")
        );
        assert!(
            client_facet
                .source
                .contains("app.register_type::<InventoryClientFacet>();")
        );
    }

    #[test]
    fn integrated_family_registration_includes_registerable_descendants() {
        let component_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let ref_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let unit = RustCodegenUnit {
            items: vec![
                RustItemPlan {
                    source_type_id: component_id,
                    source_name: "InventoryComponent".to_owned(),
                    is_reflected_base: false,
                    is_slot_owner: false,
                    has_layout_family_descendants: true,
                    is_bevy_component: true,
                    file_stem_override: None,
                    scope_path: vec![
                        "components".to_owned(),
                        "faceted_components".to_owned(),
                        "inventory_component".to_owned(),
                    ],
                    family_scope_path: vec![
                        "components".to_owned(),
                        "faceted_components".to_owned(),
                        "inventory_component".to_owned(),
                    ],
                    rust_name: "InventoryComponent".to_owned(),
                    kind: RustItemKind::Struct,
                    identity: RustTypeIdentityPlan::az_rtti(
                        component_id,
                        Some("InventoryComponent".to_owned()),
                    ),
                    repr: None,
                    raw_conversion: None,
                    derives: vec![
                        "Component".to_owned(),
                        "AzRtti".to_owned(),
                        "Debug".to_owned(),
                        "Default".to_owned(),
                        "Clone".to_owned(),
                        "Reflect".to_owned(),
                    ],
                    rtti_bases: Vec::new(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                RustItemPlan {
                    source_type_id: ref_id,
                    source_name: "LocalComponentRef<InventoryComponent>::GetTypeName".to_owned(),
                    is_reflected_base: false,
                    is_slot_owner: false,
                    has_layout_family_descendants: false,
                    is_bevy_component: false,
                    file_stem_override: None,
                    scope_path: vec![
                        "components".to_owned(),
                        "faceted_components".to_owned(),
                        "inventory_component".to_owned(),
                        "local_component_ref_base".to_owned(),
                    ],
                    family_scope_path: vec![
                        "components".to_owned(),
                        "faceted_components".to_owned(),
                        "inventory_component".to_owned(),
                        "local_component_ref_base".to_owned(),
                    ],
                    rust_name: "LocalComponentRefInventoryComponent".to_owned(),
                    kind: RustItemKind::Struct,
                    identity: RustTypeIdentityPlan::az_rtti(
                        ref_id,
                        Some("LocalComponentRef<InventoryComponent>::GetTypeName".to_owned()),
                    ),
                    repr: None,
                    raw_conversion: None,
                    derives: vec![
                        "AzRtti".to_owned(),
                        "Debug".to_owned(),
                        "Default".to_owned(),
                        "Clone".to_owned(),
                        "Reflect".to_owned(),
                    ],
                    rtti_bases: Vec::new(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let project =
            RustSourceEmitter::emit_integrated_project(&unit, &crate::CodegenContext::inline())
                .expect("integrated Rust project");
        let component = project
            .files
            .iter()
            .find(|file| file.path == "components/faceted_components/inventory_component/mod.rs")
            .expect("component module");
        let ref_base = project
            .files
            .iter()
            .find(|file| {
                file.path
                    == "components/faceted_components/inventory_component/local_component_ref_base/mod.rs"
            })
            .expect("local component ref base module");

        assert!(
            component
                .source
                .contains("local_component_ref_base::register(app);"),
            "{}",
            component.source
        );
        assert!(
            ref_base
                .source
                .contains("local_component_ref_inventory_component::register(app);"),
            "{}",
            ref_base.source
        );
    }

    #[test]
    fn standalone_layout_report_exposes_layout_without_emitting_sources() {
        let unit = namespace_and_base_family_fixture();
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());

        let report = RustSourceEmitter::standalone_layout_report(&rust_unit);
        let text = report.to_text();
        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let module_paths = report
            .modules
            .iter()
            .map(|module| module.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(paths.contains("src/types/az_framework/components/input_system_component.rs"));
        assert!(paths.contains("src/types/az_framework/input_device_id.rs"));
        assert!(paths.contains("src/types/action_conditions/action_condition_if_input.rs"));
        assert!(module_paths.contains("src/types/components/faceted_components/mod.rs"));
        assert!(!module_paths.contains("src/types/components/faceted_components/facets/mod.rs"));
        assert!(
            !module_paths
                .iter()
                .any(|path| path.starts_with("src/types/components/faceted_components_"))
        );
        assert!(paths.contains(
            "src/types/components/faceted_components/inventory_component/client_facet.rs"
        ));
        assert!(text.contains("module src/types/action_conditions/mod.rs"));
        assert!(text.contains("module src/types/components/faceted_components/mod.rs"));
        assert!(text.contains("file src/types/az_framework/components/input_system_component.rs"));
        assert!(
            text.contains(
                "module src/types/components/faceted_components/inventory_component/mod.rs"
            )
        );
        assert!(text.contains("file src/types/action_conditions/action_condition_if_input.rs"));
        assert!(text.contains(
            "file src/types/components/faceted_components/inventory_component/client_facet.rs"
        ));
        assert!(text.contains("  item Facet "));
        assert!(text.contains("  item ClientFacet "));
        assert!(text.contains("  item ActionConditionIfInput "));
        assert!(text.contains("  item InventoryClientFacet "));
        assert!(
            report
                .files
                .iter()
                .flat_map(|file| &file.items)
                .any(|item| {
                    item.rust_name == "InputSystemComponent"
                        && item.identity_kind == "AzRtti"
                        && item.source_name == "AzFramework::InputSystemComponent"
                })
        );
    }

    #[test]
    fn standalone_project_materializes_dataful_abstract_az_component_base() {
        let component_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let asset_catalog_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let unit = IrUnit {
            items: vec![
                IrItem {
                    source_type_id: component_type_id,
                    source_name: "AZ::Component".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(true),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: IrItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![scalar_field(
                        "Id",
                        uuid!("11111111-1111-1111-1111-111111111111"),
                        ScalarType::U64,
                    )],
                    variants: Vec::new(),
                },
                IrItem {
                    source_type_id: asset_catalog_type_id,
                    source_name: "AssetCatalogComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: IrItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![base_field(component_type_id, "AZ::Component")],
                    variants: Vec::new(),
                },
            ],
        };
        let rust_unit = RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
        let component = rust_unit
            .items
            .iter()
            .find(|item| item.source_name == "AZ::Component")
            .expect("AZ::Component plan");
        assert_eq!(component.identity.kind, RustTypeIdentityKind::AzRtti);
        assert!(component.is_reflected_base);
        assert!(!component.is_bevy_component);
        assert_eq!(component.fields[0].rust_name, "id");

        let asset_catalog = rust_unit
            .items
            .iter()
            .find(|item| item.source_name == "AssetCatalogComponent")
            .expect("AssetCatalogComponent plan");
        assert_eq!(asset_catalog.fields[0].rust_name, "az_component");
        assert_eq!(asset_catalog.fields[0].rust_type, "Component");
        assert!(asset_catalog.is_bevy_component);

        let project = RustSourceEmitter::emit_standalone_project(
            &rust_unit,
            &crate::CodegenContext::inline(),
        )
        .expect("standalone project");
        let az_component = project
            .files
            .iter()
            .find(|file| {
                file.source
                    .contains("const NAME: &'static str = \"AZ::Component\"")
            })
            .expect("AZ component file");
        assert!(
            az_component.path.starts_with("src/types/az/")
                && !az_component.path.starts_with("src/types/components/az"),
            "AZ::Component should emit under az, got {}",
            az_component.path
        );
        assert!(az_component.source.contains("pub struct Component"));
        assert!(az_component.source.contains("pub id: u64"));
        assert!(az_component.source.contains("impl AzRtti for Component"));
        assert!(!az_component.source.contains("AzComponent"));

        let asset_file = project
            .files
            .iter()
            .find(|file| file.path == "src/types/components/asset_catalog_component.rs")
            .expect("asset catalog component file");
        assert!(asset_file.source.contains("pub az_component: Component"));
        assert!(
            asset_file
                .source
                .contains("impl AzRtti for AssetCatalogComponent")
        );
    }

    #[test]
    #[ignore = "prints the full resources/serialize.json standalone layout for manual inspection"]
    fn prints_full_serialize_context_standalone_layout_report() {
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("resources");
        let compile_unit =
            SerializeContextCompiler::compile_from_paths_with_class_registration_trace(
                resources.join("serialize.json"),
                Some(resources.join("modules")),
                None::<&std::path::Path>,
                None::<&std::path::Path>,
                &crate::CodegenContext::inline(),
            )
            .expect("compiled serialize context");
        let rust_unit = RustCodegenPlanner::standalone().plan_serialize_codegen_unit(
            &compile_unit.codegen_unit,
            &crate::CodegenContext::inline(),
        );
        let report = RustSourceEmitter::standalone_layout_report(&rust_unit);

        println!("{}", report.to_text());

        assert!(
            !report.files.is_empty(),
            "full serialize context should produce standalone layout files"
        );
    }

    #[test]
    fn rejects_unresolved_field_types_before_emitting_source() {
        let unit = unresolved_type_unit();

        let err = RustSourceEmitter::emit_unit(&unit, &crate::CodegenContext::inline())
            .expect_err("unresolved type");

        assert!(matches!(
            err,
            RustSourceEmitError::UnresolvedType { field_name, .. } if field_name == "values"
        ));
    }

    #[test]
    fn standalone_project_rejects_unresolved_field_types_before_emitting_source() {
        let unit = unresolved_type_unit();

        let err =
            RustSourceEmitter::emit_standalone_project(&unit, &crate::CodegenContext::inline())
                .expect_err("unresolved type");

        assert!(matches!(
            err,
            RustSourceEmitError::UnresolvedType { field_name, .. } if field_name == "values"
        ));
    }

    fn standalone_fixture_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        rust_name: &str,
        scope_path: Vec<&str>,
        family_scope_path: Vec<&str>,
        file_stem_override: Option<&str>,
        has_layout_family_descendants: bool,
    ) -> RustItemPlan {
        RustItemPlan {
            source_type_id,
            source_name: source_name.to_owned(),
            is_reflected_base: false,
            is_slot_owner: false,
            has_layout_family_descendants,
            is_bevy_component: false,
            file_stem_override: file_stem_override.map(str::to_owned),
            scope_path: scope_path.into_iter().map(str::to_owned).collect(),
            family_scope_path: family_scope_path.into_iter().map(str::to_owned).collect(),
            rust_name: rust_name.to_owned(),
            kind: RustItemKind::Struct,
            identity: RustTypeIdentityPlan::az_rtti(source_type_id, Some(source_name.to_owned())),
            repr: None,
            raw_conversion: None,
            derives: vec!["Debug".to_owned(), "Default".to_owned(), "Clone".to_owned()],
            rtti_bases: Vec::new(),
            fields: Vec::new(),
            variants: Vec::new(),
        }
    }

    fn unresolved_type_unit() -> RustCodegenUnit {
        RustCodegenUnit {
            items: vec![RustItemPlan {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::BrokenComponent".to_owned(),
                is_reflected_base: false,
                is_slot_owner: false,
                has_layout_family_descendants: false,
                is_bevy_component: false,
                file_stem_override: None,
                scope_path: vec!["example".to_owned()],
                family_scope_path: vec!["example".to_owned(), "broken_components".to_owned()],
                rust_name: "BrokenComponent".to_owned(),
                kind: RustItemKind::Struct,
                identity: RustTypeIdentityPlan::az_rtti(
                    uuid!("11111111-1111-1111-1111-111111111111"),
                    Some("Example::BrokenComponent".to_owned()),
                ),
                repr: None,
                raw_conversion: None,
                derives: vec!["AzTypeInfo".to_owned(), "Debug".to_owned()],
                rtti_bases: Vec::new(),
                fields: vec![RustFieldPlan {
                    source_name: "m_values".to_owned(),
                    rust_name: "values".to_owned(),
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    rust_type: "Type33333333".to_owned(),
                    unresolved_type: Some(RustUnresolvedTypePlan {
                        type_id: uuid!("33333333-3333-3333-3333-333333333333"),
                        reason: "missing fixture type".to_owned(),
                    }),
                    integer_range: None,
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                }],
                variants: Vec::new(),
            }],
        }
    }

    fn namespace_and_base_family_fixture() -> IrUnit {
        let component_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let input_system_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let input_device_type_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let action_condition_type_id = uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd");
        let action_condition_child_type_id = uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee");
        let facet_type_id = uuid!("11111111-1111-1111-1111-111111111111");
        let faceted_component_type_id = uuid!("22222222-2222-2222-2222-222222222222");
        let client_facet_type_id = uuid!("33333333-3333-3333-3333-333333333333");
        let inventory_component_type_id = uuid!("55555555-5555-5555-5555-555555555555");
        let inventory_client_facet_type_id = uuid!("44444444-4444-4444-4444-444444444444");

        IrUnit {
            items: vec![
                abstract_fixture_item(
                    component_type_id,
                    "AZ::Component",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    input_system_type_id,
                    "AzFramework::InputSystemComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(component_type_id, "AZ::Component")],
                ),
                fixture_item(
                    input_device_type_id,
                    "AzFramework::InputDeviceId",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                abstract_fixture_item(
                    action_condition_type_id,
                    "ActionCondition",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    action_condition_child_type_id,
                    "ActionConditionIfInput",
                    ReflectedTypeRole::SupportType,
                    false,
                    vec![base_field(action_condition_type_id, "ActionCondition")],
                ),
                abstract_fixture_item(
                    facet_type_id,
                    "Facet",
                    ReflectedTypeRole::SupportType,
                    false,
                    Vec::new(),
                ),
                fixture_item(
                    faceted_component_type_id,
                    "FacetedComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![
                        base_field(component_type_id, "AZ::Component"),
                        pointer_field("m_clientFacetPtr", client_facet_type_id, "ClientFacet"),
                    ],
                ),
                fixture_item(
                    client_facet_type_id,
                    "ClientFacet",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(facet_type_id, "Facet")],
                ),
                fixture_item(
                    inventory_component_type_id,
                    "InventoryComponent",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(faceted_component_type_id, "FacetedComponent")],
                ),
                fixture_item(
                    inventory_client_facet_type_id,
                    "InventoryClientFacet",
                    ReflectedTypeRole::AzComponent,
                    false,
                    vec![base_field(client_facet_type_id, "ClientFacet")],
                ),
            ],
        }
    }

    fn fixture_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        is_reflection_marker: bool,
        fields: Vec<IrField>,
    ) -> IrItem {
        IrItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
            is_reflection_marker,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: IrItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn abstract_fixture_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        is_reflection_marker: bool,
        fields: Vec<IrField>,
    ) -> IrItem {
        let mut item = fixture_item(
            source_type_id,
            source_name,
            role,
            is_reflection_marker,
            fields,
        );
        item.is_abstract = Some(true);
        item
    }

    fn base_field(source_type_id: uuid::Uuid, source_name: &str) -> IrField {
        IrField {
            source_name: "BaseClass1".to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: source_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    fn pointer_field(source_name: &str, source_type_id: uuid::Uuid, type_name: &str) -> IrField {
        IrField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: type_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: true,
            is_dynamic_field: false,
        }
    }

    fn named_field(source_name: &str, source_type_id: uuid::Uuid, type_name: &str) -> IrField {
        IrField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: type_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    fn scalar_field(source_name: &str, source_type_id: uuid::Uuid, scalar: ScalarType) -> IrField {
        IrField {
            source_name: source_name.to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Scalar(scalar),
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
