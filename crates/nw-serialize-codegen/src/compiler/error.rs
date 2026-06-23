use std::{io, path::PathBuf};

use thiserror::Error;

use crate::document::SerializeContextDocumentError;

#[derive(Debug, Error)]
pub enum SerializeContextCompileError {
    #[error(transparent)]
    Document(#[from] SerializeContextDocumentError),
    #[error("failed to read {path}: {source}")]
    ReadJson {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    ParseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}
