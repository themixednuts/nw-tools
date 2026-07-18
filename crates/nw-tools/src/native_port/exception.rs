//! Reviewed legacy-to-native mapping exceptions (ADR 0027 / ADR 0028).
//!
//! The reviewed exception data lives in `data/legacy-port-map.toml` alongside
//! this module and is embedded at build time. It never ships in Azoth
//! project/engine configuration, native sources, catalogs, packages, or
//! runtime products.
//!
//! An exception fixes a canonical destination but may block publication until
//! the source schema is reviewed. Unknown, duplicate, unused, or stale entries
//! fail the port.

use serde::Deserialize;
use thiserror::Error;

/// Embedded reviewed exception data.
const EXCEPTION_DATA: &str = include_str!("data/legacy-port-map.toml");

/// The versioned schema of the exception data file.
pub const EXCEPTION_SCHEMA: &str = "newworld.legacy-port-map/v1";

/// Publication disposition for an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Publication {
    /// Publication is blocked until review.
    Blocked,
    /// Publication is allowed (the route is simply an override).
    Allowed,
}

/// A single reviewed mapping exception.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Exception {
    /// The decoded legacy type this exception applies to.
    pub legacy_type: String,
    /// The exact normalized legacy path.
    pub legacy_path: String,
    /// The fixed canonical native destination.
    pub canonical_path: String,
    /// Whether publication proceeds or is blocked pending review.
    pub publication: Publication,
    /// Human-reviewed justification.
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct ExceptionFile {
    schema: String,
    revision: u32,
    #[serde(default)]
    exceptions: Vec<Exception>,
}

/// The loaded, validated exception table.
#[derive(Debug, Clone)]
pub struct ExceptionTable {
    revision: u32,
    entries: Vec<Exception>,
}

/// The exception data could not be loaded or is internally inconsistent.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExceptionError {
    /// The embedded TOML failed to parse.
    #[error("failed to parse reviewed exception data: {0}")]
    Parse(#[from] toml::de::Error),

    /// The data file schema is not the expected version.
    #[error("exception data schema `{actual}` is not `{expected}`")]
    BadSchema {
        /// The schema declared in the file.
        actual: String,
        /// The expected schema.
        expected: &'static str,
    },

    /// Two exceptions share the same legacy path.
    #[error("duplicate exception for legacy path `{0}`")]
    Duplicate(String),
}

/// The outcome of consulting the exception table for a normalized legacy path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionOutcome<'a> {
    /// No exception matched; route normally.
    None,
    /// An exception fixes the destination and permits publication.
    Route(&'a Exception),
    /// An exception fixes the destination but blocks publication (hard-fail).
    Blocked(&'a Exception),
}

impl ExceptionTable {
    /// Load and validate the pinned reviewed exception table.
    ///
    /// # Errors
    ///
    /// Returns [`ExceptionError`] if the embedded data fails to parse, declares
    /// an unexpected schema, or contains a duplicate legacy path.
    pub fn pinned() -> Result<Self, ExceptionError> {
        Self::from_toml(EXCEPTION_DATA)
    }

    /// Load and validate an exception table from TOML text.
    ///
    /// # Errors
    ///
    /// See [`Self::pinned`].
    pub fn from_toml(text: &str) -> Result<Self, ExceptionError> {
        let file: ExceptionFile = toml::from_str(text)?;
        if file.schema != EXCEPTION_SCHEMA {
            return Err(ExceptionError::BadSchema {
                actual: file.schema,
                expected: EXCEPTION_SCHEMA,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for entry in &file.exceptions {
            if !seen.insert(entry.legacy_path.clone()) {
                return Err(ExceptionError::Duplicate(entry.legacy_path.clone()));
            }
        }
        Ok(Self {
            revision: file.revision,
            entries: file.exceptions,
        })
    }

    /// The reviewed revision number of this table.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }

    /// All reviewed exceptions.
    #[must_use]
    pub fn entries(&self) -> &[Exception] {
        &self.entries
    }

    /// Consult the table for a normalized legacy path.
    #[must_use]
    pub fn lookup(&self, normalized_legacy_path: &str) -> ExceptionOutcome<'_> {
        match self
            .entries
            .iter()
            .find(|entry| entry.legacy_path == normalized_legacy_path)
        {
            None => ExceptionOutcome::None,
            Some(entry) if entry.publication == Publication::Blocked => {
                ExceptionOutcome::Blocked(entry)
            }
            Some(entry) => ExceptionOutcome::Route(entry),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_table_loads_the_five_unknown_gamedata_tables() {
        let table = ExceptionTable::pinned().unwrap();
        assert_eq!(table.revision(), 1);
        assert_eq!(table.entries().len(), 5);
        for entry in table.entries() {
            assert_eq!(entry.legacy_type, "gamedata-table");
            assert_eq!(entry.publication, Publication::Blocked);
            assert!(entry.canonical_path.starts_with("gameplay/gamedata/"));
        }
    }

    #[test]
    fn blocked_exception_hard_fails_publication() {
        let table = ExceptionTable::pinned().unwrap();
        match table.lookup("gamedata/unknown/socketables.ron") {
            ExceptionOutcome::Blocked(entry) => {
                assert_eq!(
                    entry.canonical_path,
                    "gameplay/gamedata/building/socketables.ron"
                );
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn non_exception_paths_route_normally() {
        let table = ExceptionTable::pinned().unwrap();
        assert_eq!(
            table.lookup("gamedata/items/weaponitemdefinitions.ron"),
            ExceptionOutcome::None
        );
    }

    #[test]
    fn duplicate_legacy_paths_are_rejected() {
        let toml = r#"
schema = "newworld.legacy-port-map/v1"
revision = 1
[[exceptions]]
legacy_type = "gamedata-table"
legacy_path = "gamedata/unknown/x.ron"
canonical_path = "gameplay/gamedata/a/x.ron"
publication = "blocked"
reason = "r"
[[exceptions]]
legacy_type = "gamedata-table"
legacy_path = "gamedata/unknown/x.ron"
canonical_path = "gameplay/gamedata/b/x.ron"
publication = "blocked"
reason = "r"
"#;
        assert!(matches!(
            ExceptionTable::from_toml(toml),
            Err(ExceptionError::Duplicate(_))
        ));
    }

    #[test]
    fn wrong_schema_is_rejected() {
        let toml = r#"
schema = "wrong/v9"
revision = 1
"#;
        assert!(matches!(
            ExceptionTable::from_toml(toml),
            Err(ExceptionError::BadSchema { .. })
        ));
    }
}
