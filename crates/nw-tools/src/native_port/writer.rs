//! Native-port writer: emit native source files and their `.azmeta.json`
//! sidecars into an output `assets/` tree.
//!
//! This is the entire writer-side artifact set. There is no import manifest,
//! staging transaction, or dependency inventory — the engine's asset processor
//! scans, cooks, and catalogs the emitted native sources itself (ADR 0028). Per
//! ported asset the writer emits:
//!
//! - the native source file at `<out>/assets/<native_path>`, and
//! - a sibling `<native_path>.azmeta.json` sidecar carrying the preserved
//!   catalog identity (only for cataloged assets; omitted for uncataloged ones,
//!   whose identity the engine mints from the native path).
//!
//! The writer is asset-family agnostic: given mapped sources plus a
//! [`PayloadReader`] for each source's native bytes, it writes the tree.

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::identity::IdentitySource;
use super::inventory::MappedSource;
use super::sidecar::{SIDECAR_SUFFIX, SourceAssetMeta, serialize_sidecar};

/// Native source subtree name at the output root.
pub const ASSETS_DIR: &str = "assets";

/// Reads the native payload bytes for a mapped source.
///
/// For a decoded family this is the transform output; for opaque product-cache
/// families (Wwise banks/media, fonts, Bink video) it is the payload bytes.
pub trait PayloadReader {
    /// Return the native payload bytes for `source`.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the payload cannot be produced/read.
    fn read_payload(&self, source: &MappedSource) -> io::Result<Vec<u8>>;
}

impl<F> PayloadReader for F
where
    F: Fn(&MappedSource) -> io::Result<Vec<u8>>,
{
    fn read_payload(&self, source: &MappedSource) -> io::Result<Vec<u8>> {
        self(source)
    }
}

/// A write operation failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The output directory already exists (no clobber).
    #[error("port output `{0}` already exists")]
    OutputExists(PathBuf),

    /// A source payload could not be read.
    #[error("reading payload for `{path}`: {source}")]
    Payload {
        /// The native source path.
        path: String,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// Filesystem I/O failed.
    #[error("filesystem error at `{path}`: {source}")]
    Io {
        /// The path being written.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// Sidecar serialization failed.
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// The result of a successful write.
#[derive(Debug, Clone)]
pub struct WriteSummary {
    /// Number of native sources written.
    pub source_count: usize,
    /// Number of sources whose sidecar preserved a catalog identity.
    pub preserved_count: usize,
    /// Output directory root.
    pub output: PathBuf,
}

/// Write mapped sources (native file + sidecar) into `out_dir`.
///
/// # Errors
///
/// Returns [`WriteError`] if `out_dir` already exists, a payload cannot be read,
/// sidecar serialization fails, or any filesystem operation fails.
pub fn write_native_sources(
    sources: &[MappedSource],
    reader: &dyn PayloadReader,
    out_dir: &Path,
) -> Result<WriteSummary, WriteError> {
    if out_dir.exists() {
        return Err(WriteError::OutputExists(out_dir.to_path_buf()));
    }
    let assets_root = out_dir.join(ASSETS_DIR);
    create_dir_all(&assets_root)?;

    let mut preserved_count = 0;
    for source in sources {
        let payload = reader
            .read_payload(source)
            .map_err(|source_err| WriteError::Payload {
                path: source.native_path.clone(),
                source: source_err,
            })?;

        let source_abs = assets_root.join(&source.native_path);
        if let Some(parent) = source_abs.parent() {
            create_dir_all(parent)?;
        }
        write_file(&source_abs, &payload)?;

        let meta = match source.identity_source {
            IdentitySource::Catalog => {
                preserved_count += 1;
                SourceAssetMeta::preserving(source.asset_id)
            }
            IdentitySource::Fallback => SourceAssetMeta::uncataloged(),
        };
        write_file(&sidecar_path(&source_abs), &serialize_sidecar(&meta)?)?;
    }

    Ok(WriteSummary {
        source_count: sources.len(),
        preserved_count,
        output: out_dir.to_path_buf(),
    })
}

/// The sidecar path for a native source file (`<file>` + `.azmeta.json`).
#[must_use]
pub fn sidecar_path(source_file: &Path) -> PathBuf {
    let mut sidecar = source_file.as_os_str().to_os_string();
    sidecar.push(SIDECAR_SUFFIX);
    PathBuf::from(sidecar)
}

fn create_dir_all(path: &Path) -> Result<(), WriteError> {
    std::fs::create_dir_all(path).map_err(|source| WriteError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
    std::fs::write(path, bytes).map_err(|source| WriteError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_port::identity::AssetId;
    use crate::native_port::inventory::{Mapper, SourceInput};
    use crate::native_port::route::DecodedClass;
    use uuid::uuid;

    #[test]
    fn writes_native_sources_and_sidecars() {
        let inv = Mapper::pinned().unwrap().map_all([
            // Uncataloged -> sidecar omits preserved id.
            SourceInput::new(
                "Textures/Items/Weapons/Sword_Diff.png",
                DecodedClass::Image,
                None,
            ),
            // Cataloged -> sidecar preserves the catalog id.
            SourceInput::new(
                "textures/terrain/rock_diff.png",
                DecodedClass::Image,
                Some(AssetId::new(
                    uuid!("11112222-3333-4444-5555-666677778888"),
                    0,
                )),
            ),
        ]);
        assert!(!inv.has_blocking_errors());

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("port-output");
        let reader = |source: &MappedSource| Ok(source.native_path.as_bytes().to_vec());
        let summary = write_native_sources(&inv.sources, &reader, &out).unwrap();

        assert_eq!(summary.source_count, 2);
        assert_eq!(summary.preserved_count, 1);

        let sword = out.join("assets/rendering/textures/items/weapons/sword_diff.png");
        let rock = out.join("assets/rendering/textures/terrain/rock_diff.png");
        assert!(sword.exists());
        assert!(rock.exists());

        // No manifest artifact is emitted.
        assert!(!out.join("azoth-import-manifest.json").exists());

        // Uncataloged sidecar omits preserved_asset_id.
        let sword_meta = std::fs::read_to_string(sidecar_path(&sword)).unwrap();
        assert_eq!(sword_meta, "{\"spec\":\"azoth.source-meta/v1\"}\n");

        // Cataloged sidecar preserves the catalog id verbatim.
        let rock_meta = std::fs::read_to_string(sidecar_path(&rock)).unwrap();
        assert_eq!(
            rock_meta,
            "{\"spec\":\"azoth.source-meta/v1\",\"preserved_asset_id\":{\"guid\":\"11112222-3333-4444-5555-666677778888\",\"sub_id\":0}}\n"
        );
    }

    #[test]
    fn refuses_to_clobber_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("exists");
        std::fs::create_dir_all(&out).unwrap();
        let reader = |source: &MappedSource| Ok(source.native_path.as_bytes().to_vec());
        let err = write_native_sources(&[], &reader, &out).unwrap_err();
        assert!(matches!(err, WriteError::OutputExists(_)));
    }
}
