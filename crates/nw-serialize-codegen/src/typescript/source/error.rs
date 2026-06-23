use thiserror::Error;

#[derive(Debug, Error)]
pub enum TypeScriptSourceEmitError {
    #[error(
        "unresolved reflected type `{type_id}` for TypeScript field `{item_name}.{field_name}`: {reason}"
    )]
    UnresolvedType {
        item_name: String,
        field_name: String,
        type_id: uuid::Uuid,
        reason: String,
    },

    #[error("TypeScript output failed Oxc parse: {0}")]
    Syntax(String),
}
