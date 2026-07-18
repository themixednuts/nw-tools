//! Authored asset references shared by format parsers and dependency resolvers.

use std::fmt;

use crate::{AssetReference, normalize_virtual_path};

/// A target named by an authored asset.
///
/// Concrete references retain the engine's full [`AssetReference`] identity.
/// Patterns model Cry recursive wildcard directives, while symbols model
/// runtime names such as Mannequin animation aliases and ATL controls that need
/// an owning registry before they can become file paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetDependencyTarget {
    Asset(AssetReference),
    PathPattern(String),
    Symbol(String),
}

impl AssetDependencyTarget {
    #[must_use]
    pub fn path(path: impl Into<String>) -> Self {
        Self::Asset(AssetReference::from_hint(path))
    }

    #[must_use]
    pub fn pattern(pattern: impl Into<String>) -> Self {
        Self::PathPattern(pattern.into())
    }

    #[must_use]
    pub fn symbol(symbol: impl Into<String>) -> Self {
        Self::Symbol(symbol.into())
    }

    #[must_use]
    pub fn authored_name(&self) -> &str {
        match self {
            Self::Asset(reference) => reference.hint().unwrap_or_default(),
            Self::PathPattern(pattern) | Self::Symbol(pattern) => pattern,
        }
    }
}

impl fmt::Display for AssetDependencyTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(reference) => {
                if let Some(hint) = reference.hint() {
                    formatter.write_str(hint)
                } else {
                    write!(formatter, "{}", reference.asset_id)
                }
            }
            Self::PathPattern(pattern) => write!(formatter, "pattern:{pattern}"),
            Self::Symbol(symbol) => write!(formatter, "symbol:{symbol}"),
        }
    }
}

/// One direct authored relationship, independent of any catalog or filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetDependency {
    relation: String,
    target: AssetDependencyTarget,
    required: bool,
    association: Option<String>,
}

impl AssetDependency {
    #[must_use]
    pub fn required(relation: impl Into<String>, target: AssetDependencyTarget) -> Self {
        Self {
            relation: relation.into(),
            target,
            required: true,
            association: None,
        }
    }

    #[must_use]
    pub fn optional(relation: impl Into<String>, target: AssetDependencyTarget) -> Self {
        Self {
            relation: relation.into(),
            target,
            required: false,
            association: None,
        }
    }

    #[must_use]
    pub fn required_path(relation: impl Into<String>, path: impl Into<String>) -> Self {
        Self::required(relation, AssetDependencyTarget::path(path))
    }

    #[must_use]
    pub fn optional_path(relation: impl Into<String>, path: impl Into<String>) -> Self {
        Self::optional(relation, AssetDependencyTarget::path(path))
    }

    /// Correlate this dependency with another path selected by the same
    /// authored record.
    ///
    /// Formats use this for paired relationships such as a dynamic slice and
    /// its exact variant sidecar. The association is graph metadata, not an
    /// additional dependency.
    #[must_use]
    pub fn associated_with_path(mut self, path: impl AsRef<str>) -> Self {
        self.association = Some(normalize_virtual_path(path.as_ref()));
        self
    }

    #[must_use]
    pub fn relation(&self) -> &str {
        &self.relation
    }

    #[must_use]
    pub const fn target(&self) -> &AssetDependencyTarget {
        &self.target
    }

    #[must_use]
    pub fn association(&self) -> Option<&str> {
        self.association.as_deref()
    }

    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

/// Typed formats implement this projection without performing I/O.
pub trait AssetDependencies {
    #[must_use]
    fn asset_dependencies(&self) -> Vec<AssetDependency>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_path_keeps_a_real_asset_reference() {
        let dependency = AssetDependency::required_path("material", "Objects\\Hero.MTL");
        let AssetDependencyTarget::Asset(reference) = dependency.target() else {
            panic!("expected concrete asset reference");
        };

        assert_eq!(reference.hint(), Some("Objects\\Hero.MTL"));
        assert!(dependency.is_required());
    }

    #[test]
    fn dependency_association_normalizes_the_correlated_path() {
        let dependency = AssetDependency::optional_path("variant", "Slices/Hero_Red.slice.meta")
            .associated_with_path("Slices\\Hero.dynamicslice");

        assert_eq!(dependency.association(), Some("slices/hero.dynamicslice"));
    }
}
