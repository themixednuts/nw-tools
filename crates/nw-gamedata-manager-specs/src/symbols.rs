use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GhidraClassPath(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GhidraFunctionPath(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RustTypePath(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RustPath(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RustIdentifier(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameDataTableName(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameDataRowTypeName(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameDataColumnName(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameAssetPath(Box<str>);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SymbolNameError {
    #[error("{kind} name cannot be empty")]
    Empty { kind: &'static str },

    #[error("{kind} name `{value}` contains an empty path segment")]
    EmptySegment { kind: &'static str, value: String },

    #[error("{kind} name `{value}` must contain at least {minimum} `::`-separated segments")]
    TooFewSegments {
        kind: &'static str,
        value: String,
        minimum: usize,
    },

    #[error("{kind} `{value}` is not a canonical relative game asset path: {reason}")]
    InvalidAssetPath {
        kind: &'static str,
        value: String,
        reason: &'static str,
    },

    #[error("{kind} `{value}` is not a Rust identifier")]
    InvalidRustIdentifier { kind: &'static str, value: String },
}

impl GhidraClassPath {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "Ghidra class", 2).map(Self)
    }
}

impl GhidraFunctionPath {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "Ghidra function", 3).map(Self)
    }
}

impl RustTypePath {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "Rust type", 1).map(Self)
    }
}

impl RustPath {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "Rust path", 1).map(Self)
    }
}

impl RustIdentifier {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_rust_identifier(value, "Rust identifier").map(Self)
    }
}

impl GameDataTableName {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "GameData table", 1).map(Self)
    }
}

impl GameDataRowTypeName {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_path_name(value, "GameData row type", 1).map(Self)
    }
}

impl GameDataColumnName {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_non_empty_name(value, "GameData column").map(Self)
    }
}

impl GameAssetPath {
    pub fn new(value: impl Into<String>) -> Result<Self, SymbolNameError> {
        parse_game_asset_path(value, "game asset path").map(Self)
    }
}

macro_rules! impl_symbol_name {
    ($type:ty) => {
        impl $type {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $type {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $type {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $type {
            type Err = SymbolNameError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

impl_symbol_name!(GhidraClassPath);
impl_symbol_name!(GhidraFunctionPath);
impl_symbol_name!(RustTypePath);
impl_symbol_name!(RustPath);
impl_symbol_name!(RustIdentifier);
impl_symbol_name!(GameDataTableName);
impl_symbol_name!(GameDataRowTypeName);
impl_symbol_name!(GameDataColumnName);
impl_symbol_name!(GameAssetPath);

fn parse_non_empty_name(
    value: impl Into<String>,
    kind: &'static str,
) -> Result<Box<str>, SymbolNameError> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SymbolNameError::Empty { kind });
    }

    Ok(Box::from(trimmed))
}

fn parse_path_name(
    value: impl Into<String>,
    kind: &'static str,
    minimum_segments: usize,
) -> Result<Box<str>, SymbolNameError> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SymbolNameError::Empty { kind });
    }

    let segments = trimmed.split("::").collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(SymbolNameError::EmptySegment {
            kind,
            value: trimmed.to_owned(),
        });
    }
    if segments.len() < minimum_segments {
        return Err(SymbolNameError::TooFewSegments {
            kind,
            value: trimmed.to_owned(),
            minimum: minimum_segments,
        });
    }

    Ok(Box::from(trimmed))
}

fn parse_game_asset_path(
    value: impl Into<String>,
    kind: &'static str,
) -> Result<Box<str>, SymbolNameError> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SymbolNameError::Empty { kind });
    }

    if trimmed.contains('\\') {
        return Err(SymbolNameError::InvalidAssetPath {
            kind,
            value: trimmed.to_owned(),
            reason: "use `/` separators",
        });
    }
    if trimmed.starts_with('/') {
        return Err(SymbolNameError::InvalidAssetPath {
            kind,
            value: trimmed.to_owned(),
            reason: "catalog paths are relative",
        });
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(SymbolNameError::InvalidAssetPath {
            kind,
            value: trimmed.to_owned(),
            reason: "path segments must be non-empty and cannot be `.` or `..`",
        });
    }

    Ok(Box::from(trimmed))
}

fn parse_rust_identifier(
    value: impl Into<String>,
    kind: &'static str,
) -> Result<Box<str>, SymbolNameError> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SymbolNameError::Empty { kind });
    }

    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(SymbolNameError::Empty { kind });
    };
    let first_is_valid = first == '_' || first.is_ascii_alphabetic();
    let rest_is_valid = chars.all(|value| value == '_' || value.is_ascii_alphanumeric());
    if first_is_valid && rest_is_valid {
        Ok(Box::from(trimmed))
    } else {
        Err(SymbolNameError::InvalidRustIdentifier {
            kind,
            value: trimmed.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghidra_function_names_require_namespace_class_and_function() {
        assert!(
            GhidraFunctionPath::new("Javelin::ItemDataManager::CacheAllItemDataTables").is_ok()
        );
        assert!(GhidraFunctionPath::new("ItemDataManager::CacheAllItemDataTables").is_err());
    }

    #[test]
    fn symbol_names_reject_empty_segments() {
        assert!(GhidraClassPath::new("Javelin::::ItemDataManager").is_err());
        assert!(RustTypePath::new("crate::ItemDataManager").is_ok());
    }

    #[test]
    fn game_asset_paths_are_relative_catalog_paths() {
        assert!(GameAssetPath::new("sharedassets/genericassets/ui/uidatabase.uidb").is_ok());
        assert!(GameAssetPath::new("/sharedassets/genericassets/ui/uidatabase.uidb").is_err());
        assert!(GameAssetPath::new("sharedassets\\genericassets\\ui\\uidatabase.uidb").is_err());
    }

    #[test]
    fn rust_identifier_names_are_single_identifiers() {
        assert!(RustIdentifier::new("archetype_data").is_ok());
        assert!(RustIdentifier::new("9bad").is_err());
        assert!(RustIdentifier::new("crate::path").is_err());
    }
}
