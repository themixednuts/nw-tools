//! Catalog-mirroring output packages shared by all exporters.
//!
//! A package holds ordinary manifest/artifact files alongside their external
//! resources, laid out to mirror the game's own asset-catalog directory tree.
//! Raw dependency payloads keep their exact pak paths; derived payloads sit
//! next to the source (`<caf>.bin`) or next to the manifest (`<model>.bin`,
//! decoded `.png` at the texture's path). Identical bytes claimed at the same
//! normalized path are written once; a within-run path collision that carries
//! different bytes is disambiguated with a short content-hash infix.

use std::collections::HashMap;
use std::fmt;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// SHA-256 identity of one immutable shared payload.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentId([u8; 32]);

impl ContentId {
    /// Hash a payload using SHA-256.
    #[must_use]
    pub fn for_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Raw SHA-256 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Hash for ContentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Reference to one payload already published at its catalog path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
    content_id: ContentId,
    relative_path: PathBuf,
    byte_len: usize,
}

impl StoredBlob {
    #[must_use]
    pub const fn content_id(&self) -> ContentId {
        self.content_id
    }

    #[must_use]
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

type SharedWrite = OnceLock<Result<(), Arc<io::Error>>>;
/// Identity registry keyed by normalized package-relative path. The stored
/// [`ContentId`] records which payload first claimed the path so a later claim
/// with differing bytes can be disambiguated instead of clobbering it.
type SharedWrites = HashMap<String, (ContentId, Arc<SharedWrite>)>;

/// Thread-safe writer for one structured export root.
///
/// Clone this value into batch jobs. Concurrent attempts to publish the same
/// content at the same path wait on one write and all receive the same
/// [`StoredBlob`].
#[derive(Clone)]
pub struct PackageWriter {
    root: Arc<PathBuf>,
    writes: Arc<Mutex<SharedWrites>>,
}

impl PackageWriter {
    /// Create a package rooted at `root`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the output root cannot be created.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PackageError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).map_err(|source| PackageError::Io {
            path: root.clone(),
            source,
        })?;
        Ok(Self {
            root: Arc::new(root),
            writes: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Store immutable bytes at the package-relative `path`, mirroring the
    /// game's asset-catalog directory tree.
    ///
    /// The path is normalized (forward slashes, ASCII-lowercase) so the same
    /// authored asset arriving with different casing resolves to one file. The
    /// first payload to claim a normalized path wins it: identical bytes already
    /// on disk from a previous run are reused, differing bytes are replaced.
    /// A later claim on the same path within this run that carries *different*
    /// content is written to a disambiguated sibling — the first 12 hex chars of
    /// its SHA-256 are inserted before the final extension segment
    /// (`attack.caf` → `attack.<12hex>.caf`). The returned
    /// [`StoredBlob`] reports the final path actually used.
    ///
    /// # Errors
    ///
    /// Returns an error if the path escapes the package or the write fails.
    pub fn store_at(
        &self,
        path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<StoredBlob, PackageError> {
        let normalized = normalize_store_path(path.as_ref())?;
        let content_id = ContentId::for_bytes(bytes);
        let (final_path, cell) = {
            let mut writes = self
                .writes
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let target = match writes.get(&normalized) {
                Some((existing, _)) if *existing != content_id => {
                    suffixed_path(&normalized, content_id)
                }
                _ => normalized,
            };
            let entry = writes
                .entry(target.clone())
                .or_insert_with(|| (content_id, Arc::new(OnceLock::new())));
            (target, Arc::clone(&entry.1))
        };
        let destination = self.root.join(&final_path);
        let result =
            cell.get_or_init(|| publish_at(&destination, content_id, bytes).map_err(Arc::new));
        if let Err(source) = result {
            return Err(PackageError::Io {
                path: destination,
                source: io::Error::new(source.kind(), source.to_string()),
            });
        }
        Ok(StoredBlob {
            content_id,
            relative_path: PathBuf::from(final_path),
            byte_len: bytes.len(),
        })
    }

    /// URI from an artifact to a stored resource, using forward slashes as
    /// required by glTF and other URI-based formats.
    ///
    /// # Errors
    ///
    /// Returns an error if the artifact path is absolute or escapes the package.
    pub fn uri_from(
        &self,
        artifact: impl AsRef<Path>,
        blob: &StoredBlob,
    ) -> Result<String, PackageError> {
        let artifact = validate_relative(artifact.as_ref())?;
        let from = artifact.parent().unwrap_or_else(|| Path::new(""));
        Ok(relative_path(from, &blob.relative_path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"))
    }

    /// Write an ordinary package artifact such as a manifest atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the path escapes the package or the write fails.
    pub fn write(&self, artifact: impl AsRef<Path>, bytes: &[u8]) -> Result<usize, PackageError> {
        self.write_stream(artifact, |writer| writer.write_all(bytes))
    }

    /// Stream an ordinary package artifact through a buffered temporary file.
    ///
    /// The destination is published only after `write` returns successfully and
    /// the temporary file has been flushed. This lets exporters serialize large
    /// manifests without first allocating a second complete byte buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the path escapes the package, the callback fails, or
    /// the completed artifact cannot be published.
    pub fn write_stream(
        &self,
        artifact: impl AsRef<Path>,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<usize, PackageError> {
        let relative = validate_relative(artifact.as_ref())?;
        let destination = self.root.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PackageError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let (temporary, file) =
            create_temporary(&destination).map_err(|source| PackageError::Io {
                path: destination.clone(),
                source,
            })?;
        let mut writer = BufWriter::new(file);
        if let Err(source) = write(&mut writer)
            .and_then(|()| writer.flush())
            .and_then(|()| writer.get_ref().sync_all())
        {
            drop(writer);
            let _ = std::fs::remove_file(&temporary);
            return Err(PackageError::Io {
                path: destination,
                source,
            });
        }
        drop(writer);
        let byte_len = std::fs::metadata(&temporary)
            .map_err(|source| PackageError::Io {
                path: temporary.clone(),
                source,
            })?
            .len() as usize;
        replace_artifact(&temporary, &destination).map_err(|source| PackageError::Io {
            path: destination,
            source,
        })?;
        Ok(byte_len)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("package artifact path must be relative and contained: {0}")]
    UnsafeArtifact(PathBuf),
    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Normalize a package-relative resource path: forward slashes, ASCII-lowercase,
/// then reject anything that is absolute or escapes the package root.
fn normalize_store_path(path: &Path) -> Result<String, PackageError> {
    let normalized: String = path
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character == '\\' {
                '/'
            } else {
                character.to_ascii_lowercase()
            }
        })
        .collect();
    validate_relative(Path::new(&normalized))?;
    Ok(normalized)
}

/// Insert the first 12 hex chars of `content_id` before the final extension
/// segment of `path` (`a/attack.caf` → `a/attack.<12hex>.caf`). The
/// result is content-derived, so it cannot collide with a differing payload.
fn suffixed_path(path: &str, content_id: ContentId) -> String {
    let hex12: String = content_id.to_string().chars().take(12).collect();
    let name_start = path.rfind('/').map_or(0, |slash| slash + 1);
    match path[name_start..].rfind('.') {
        Some(dot) => {
            let dot = name_start + dot;
            format!("{}.{hex12}{}", &path[..dot], &path[dot..])
        }
        None => format!("{path}.{hex12}"),
    }
}

fn validate_relative(path: &Path) -> Result<&Path, PackageError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(PackageError::UnsafeArtifact(path.to_path_buf()));
    }
    Ok(path)
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = from
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .collect::<Vec<_>>();
    let to = to
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    relative
}

fn publish_at(destination: &Path, content_id: ContentId, bytes: &[u8]) -> io::Result<()> {
    if existing_file_matches(destination, content_id)? {
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "resource has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let (temporary, mut file) = create_temporary(destination)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    replace_artifact(&temporary, destination)
}

fn create_temporary(destination: &Path) -> io::Result<(PathBuf, std::fs::File)> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "artifact has no parent"))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

fn replace_artifact(temporary: &Path, destination: &Path) -> io::Result<()> {
    match std::fs::rename(temporary, destination) {
        Ok(()) => Ok(()),
        Err(_) if destination.is_file() => {
            std::fs::remove_file(destination)?;
            if let Err(error) = std::fs::rename(temporary, destination) {
                let _ = std::fs::remove_file(temporary);
                return Err(error);
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(temporary);
            Err(error)
        }
    }
}

/// Whether the file already on disk at `path` hashes to `content_id`. A plain
/// length match is no longer sufficient identity now that paths (not digests)
/// name resources, so the existing bytes are hashed and compared.
fn existing_file_matches(path: &Path, content_id: ContentId) -> io::Result<bool> {
    match std::fs::read(path) {
        Ok(existing) => Ok(ContentId::for_bytes(&existing) == content_id),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_at_catalog_path_and_builds_relative_uri() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let blob = writer
            .store_at("textures/alligator/alligator_diff.png", b"decoded png")
            .unwrap();

        assert_eq!(
            blob.relative_path(),
            Path::new("textures/alligator/alligator_diff.png")
        );
        assert_eq!(
            writer
                .uri_from(
                    "objects/characters/npc/natural/alligator/alligator.gltf",
                    &blob
                )
                .unwrap(),
            "../../../../../textures/alligator/alligator_diff.png"
        );
        assert_eq!(
            std::fs::read(temp.path().join(blob.relative_path())).unwrap(),
            b"decoded png"
        );
    }

    #[test]
    fn dedupes_same_path_and_is_case_insensitive() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let first = writer
            .store_at("slices/tree_oak.slice.meta", b"slice metadata")
            .unwrap();
        // Same authored asset, different casing and separators, same bytes.
        let second = writer
            .store_at("SLICES\\TREE_OAK.SLICE.META", b"slice metadata")
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first.relative_path(),
            Path::new("slices/tree_oak.slice.meta")
        );
        assert_eq!(
            std::fs::read_dir(temp.path().join("slices"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn disambiguates_within_run_collision_with_content_infix() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let first = writer
            .store_at("animations/attack.caf", b"skeleton A channels")
            .unwrap();
        // Same derived path, different bytes (e.g. retargeted onto another skeleton).
        let second = writer
            .store_at("animations/attack.caf", b"skeleton B channels")
            .unwrap();

        assert_eq!(first.relative_path(), Path::new("animations/attack.caf"));
        let hex12: String = second.content_id().to_string().chars().take(12).collect();
        assert_eq!(
            second.relative_path(),
            Path::new(&format!("animations/attack.{hex12}.caf"))
        );
        assert_eq!(
            std::fs::read(temp.path().join(first.relative_path())).unwrap(),
            b"skeleton A channels"
        );
        assert_eq!(
            std::fs::read(temp.path().join(second.relative_path())).unwrap(),
            b"skeleton B channels"
        );
    }

    #[test]
    fn cross_run_reuses_identical_and_replaces_differing_bytes() {
        let temp = tempfile::tempdir().unwrap();
        // A previous run leaves a file at the same path.
        PackageWriter::new(temp.path())
            .unwrap()
            .store_at("objects/alligator.bin", b"original geometry")
            .unwrap();

        // Identical bytes from a fresh registry are reused without rewriting.
        let reused = PackageWriter::new(temp.path())
            .unwrap()
            .store_at("objects/alligator.bin", b"original geometry")
            .unwrap();
        assert_eq!(reused.relative_path(), Path::new("objects/alligator.bin"));
        assert_eq!(
            std::fs::read(temp.path().join(reused.relative_path())).unwrap(),
            b"original geometry"
        );

        // Differing bytes replace the superseded file in place.
        let replaced = PackageWriter::new(temp.path())
            .unwrap()
            .store_at("objects/alligator.bin", b"rebuilt geometry")
            .unwrap();
        assert_eq!(replaced.relative_path(), Path::new("objects/alligator.bin"));
        assert_eq!(
            std::fs::read(temp.path().join(replaced.relative_path())).unwrap(),
            b"rebuilt geometry"
        );
    }

    #[test]
    fn rejects_paths_that_escape_the_package() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        assert!(matches!(
            writer.write("../outside", b"no"),
            Err(PackageError::UnsafeArtifact(_))
        ));
        assert!(matches!(
            writer.store_at("../outside.bin", b"no"),
            Err(PackageError::UnsafeArtifact(_))
        ));
    }

    #[test]
    fn concurrent_publish_uses_one_shared_result() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let handles = (0..8)
            .map(|_| {
                let writer = writer.clone();
                std::thread::spawn(move || {
                    writer
                        .store_at("meshes/shared.bin", &vec![7; 64 * 1024])
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let blobs = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(blobs.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            std::fs::read_dir(temp.path().join("meshes"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn streams_and_replaces_artifacts_without_exposing_failed_writes() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        assert_eq!(
            writer
                .write_stream("nested/manifest.json", |output| {
                    output.write_all(b"first")
                })
                .unwrap(),
            5
        );
        let failed = writer.write_stream("nested/manifest.json", |output| {
            output.write_all(b"partial")?;
            Err(io::Error::other("serialization failed"))
        });
        assert!(failed.is_err());
        assert_eq!(
            std::fs::read(temp.path().join("nested/manifest.json")).unwrap(),
            b"first"
        );
        assert_eq!(
            writer
                .write("nested/manifest.json", b"replacement")
                .unwrap(),
            11
        );
        assert_eq!(
            std::fs::read(temp.path().join("nested/manifest.json")).unwrap(),
            b"replacement"
        );
    }
}
