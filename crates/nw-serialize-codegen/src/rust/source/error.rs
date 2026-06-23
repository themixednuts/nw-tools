use thiserror::Error;

#[derive(Debug, Error)]
pub enum RustSourceEmitError {
    #[error(
        "unresolved reflected type `{type_id}` for Rust field `{item_name}.{field_name}`: {reason}"
    )]
    UnresolvedType {
        item_name: String,
        field_name: String,
        type_id: uuid::Uuid,
        reason: String,
    },

    #[error("invalid Rust item identifier `{identifier}` for reflected type `{source_name}`")]
    ItemIdent {
        source_name: String,
        identifier: String,
        #[source]
        source: syn::Error,
    },
    #[error(
        "invalid Rust field identifier `{identifier}` for reflected field `{source_name}` on `{item_name}`"
    )]
    FieldIdent {
        item_name: String,
        source_name: String,
        identifier: String,
        #[source]
        source: syn::Error,
    },
    #[error(
        "invalid Rust enum variant identifier `{identifier}` for reflected variant `{source_name}` on `{item_name}`"
    )]
    VariantIdent {
        item_name: String,
        source_name: String,
        identifier: String,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust derive path `{derive_name}` on `{item_name}`")]
    DerivePath {
        item_name: String,
        derive_name: String,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust type `{rust_type}` for field `{field_name}` on `{item_name}`")]
    FieldType {
        item_name: String,
        field_name: String,
        rust_type: String,
        #[source]
        source: syn::Error,
    },
    #[error(
        "invalid Rust associated const identifier `{identifier}` for range field `{field_name}` on `{item_name}`"
    )]
    RangeConstIdent {
        item_name: String,
        field_name: String,
        identifier: String,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust range type `{rust_type}` for field `{field_name}` on `{item_name}`")]
    RangeType {
        item_name: String,
        field_name: String,
        rust_type: String,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust range bound `{bound}` for field `{field_name}` on `{item_name}`")]
    RangeBound {
        item_name: String,
        field_name: String,
        bound: String,
        #[source]
        source: syn::Error,
    },
    #[error(
        "invalid Rust enum discriminant `{discriminant}` for variant `{variant_name}` on `{item_name}`"
    )]
    VariantDiscriminant {
        item_name: String,
        variant_name: String,
        discriminant: i32,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust repr `{repr}` on `{item_name}`")]
    Repr {
        item_name: String,
        repr: String,
        #[source]
        source: syn::Error,
    },
    #[error("invalid Rust raw conversion type `{raw_type}` on `{item_name}`")]
    RawConversionType {
        item_name: String,
        raw_type: String,
        #[source]
        source: syn::Error,
    },
    #[error("Rust source emission was cancelled")]
    Cancelled,
    #[error("failed to parse emitted Rust source")]
    File(#[source] syn::Error),
}
