use thiserror::Error;

#[derive(Debug, Error)]
pub enum GoSourceEmitError {
    #[error("invalid Go package name `{package_name}`")]
    PackageName { package_name: String },

    #[error(
        "unresolved reflected type `{type_id}` for Go field `{item_name}.{field_name}`: {reason}"
    )]
    UnresolvedType {
        item_name: String,
        field_name: String,
        type_id: uuid::Uuid,
        reason: String,
    },

    #[error("failed to format Go source: {0}")]
    Format(String),

    #[error("formatted Go source was not UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("formatted Go source failed Go AST parse: {0}")]
    Syntax(String),
}
