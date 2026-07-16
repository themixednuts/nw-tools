use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A checked-in, reviewable set of SerializeContext roots.
///
/// This intentionally matches the selection file consumed by the New World
/// runtime generator. `roots` and `data_roots` are both emitted roots;
/// `native_defaults` controls derive planning; and `engine_owned_types`
/// records definitions supplied by a handwritten domain module.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializeCodegenSelectionManifest {
    #[serde(default)]
    pub roots: Vec<SerializeCodegenRootEntry>,
    #[serde(default)]
    pub data_roots: Vec<SerializeCodegenRootEntry>,
    #[serde(default)]
    pub engine_owned_types: Vec<SerializeCodegenEngineOwnedTypeEntry>,
    #[serde(default)]
    pub native_defaults: Vec<SerializeCodegenRootEntry>,
}

impl SerializeCodegenSelectionManifest {
    /// Reads and validates a selection manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or does not match the
    /// supported selection schema.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, SelectionManifestError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| SelectionManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| SelectionManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn root_specs(&self) -> impl Iterator<Item = &str> {
        self.roots
            .iter()
            .chain(&self.data_roots)
            .map(SerializeCodegenRootEntry::spec)
    }

    pub fn native_default_specs(&self) -> impl Iterator<Item = &str> {
        self.native_defaults
            .iter()
            .map(SerializeCodegenRootEntry::spec)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SerializeCodegenRootEntry {
    Spec(String),
    Detailed {
        root: String,
        #[serde(default)]
        reason: Option<String>,
    },
}

impl SerializeCodegenRootEntry {
    #[must_use]
    pub fn spec(&self) -> &str {
        match self {
            Self::Spec(spec) => spec,
            Self::Detailed { root, .. } => root,
        }
    }

    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Spec(_) => None,
            Self::Detailed { reason, .. } => reason.as_deref(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializeCodegenEngineOwnedTypeEntry {
    pub root: String,
    pub rust_type: String,
    #[serde(default = "default_engine_owned_source")]
    pub source: String,
    #[serde(default)]
    pub reason: Option<String>,
}

fn default_engine_owned_source() -> String {
    "engine_types.rs".to_owned()
}

#[derive(Debug, thiserror::Error)]
pub enum SelectionManifestError {
    #[error("failed to read SerializeContext selection manifest {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse SerializeContext selection manifest {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_new_world_selection_shape() {
        let manifest = serde_json::from_str::<SerializeCodegenSelectionManifest>(
            r#"{
                "roots": ["AnimationEvent"],
                "data_roots": [{"root":"MannequinTagOptions","reason":"asset enum"}],
                "native_defaults": [{"root":"EventData","reason":"native ctor"}],
                "engine_owned_types": [{
                    "root":"AudioPreload",
                    "rust_type":"AudioPreload",
                    "source":"animation/events.rs",
                    "reason":"canonical native type"
                }]
            }"#,
        )
        .expect("selection should parse");

        assert_eq!(
            manifest.root_specs().collect::<Vec<_>>(),
            ["AnimationEvent", "MannequinTagOptions"]
        );
        assert_eq!(
            manifest.native_default_specs().collect::<Vec<_>>(),
            ["EventData"]
        );
        assert_eq!(manifest.engine_owned_types[0].source, "animation/events.rs");
    }

    #[test]
    fn rejects_unknown_manifest_fields() {
        let error = serde_json::from_str::<SerializeCodegenSelectionManifest>(
            r#"{"data_roots":[],"ad_hoc_types":["no"]}"#,
        )
        .expect_err("unknown fields must fail closed");
        assert!(error.to_string().contains("unknown field"));
    }
}
