//! Content-addressed output packages shared by all exporters.
//!
//! A package has ordinary manifest/artifact files plus a single shared blob
//! directory. Blobs are named by SHA-256, so independently exported assets can
//! reference identical geometry, animation, texture, audio, or other payloads
//! without writing duplicate bytes.

use std::collections::HashMap;
use std::fmt;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};

const SHARED_DIRECTORY: &str = "_shared/sha256";
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

/// Reference to one payload already present in the package's shared store.
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
type SharedWrites = HashMap<(ContentId, String), Arc<SharedWrite>>;

/// Thread-safe writer for one structured export root.
///
/// Clone this value into batch jobs. Concurrent attempts to publish the same
/// content wait on one write and all receive the same [`StoredBlob`].
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

    /// Store immutable bytes under `_shared/sha256/<digest>.<extension>`.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe extension or failed filesystem write.
    pub fn store(&self, bytes: &[u8], extension: &str) -> Result<StoredBlob, PackageError> {
        validate_extension(extension)?;
        let extension = extension.to_ascii_lowercase();
        let content_id = ContentId::for_bytes(bytes);
        let relative_path =
            PathBuf::from(SHARED_DIRECTORY).join(format!("{content_id}.{extension}"));
        let destination = self.root.join(&relative_path);
        let cell = {
            let mut writes = self
                .writes
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            Arc::clone(
                writes
                    .entry((content_id, extension))
                    .or_insert_with(|| Arc::new(OnceLock::new())),
            )
        };
        let result = cell.get_or_init(|| publish_blob(&destination, bytes).map_err(Arc::new));
        if let Err(source) = result {
            return Err(PackageError::Io {
                path: destination,
                source: io::Error::new(source.kind(), source.to_string()),
            });
        }
        Ok(StoredBlob {
            content_id,
            relative_path,
            byte_len: bytes.len(),
        })
    }

    /// URI from an artifact to a shared blob, using forward slashes as required
    /// by glTF and other URI-based formats.
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
    #[error("shared blob extension must contain only ASCII letters or digits: {0:?}")]
    UnsafeExtension(String),
    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

fn validate_extension(extension: &str) -> Result<(), PackageError> {
    if extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(PackageError::UnsafeExtension(extension.to_string()));
    }
    Ok(())
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

fn publish_blob(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    if existing_blob_matches(destination, bytes.len())? {
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "blob has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let (temporary, mut file) = create_temporary(destination)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    match std::fs::rename(&temporary, destination) {
        Ok(()) => Ok(()),
        Err(_) if existing_blob_matches(destination, bytes.len())? => {
            std::fs::remove_file(temporary)
        }
        Err(error) => {
            let _ = std::fs::remove_file(temporary);
            Err(error)
        }
    }
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

fn existing_blob_matches(path: &Path, expected_len: usize) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && metadata.len() == expected_len as u64),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_shared_bytes_and_builds_nested_uri() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let first = writer.store(b"same payload", "BIN").unwrap();
        let second = writer.store(b"same payload", "bin").unwrap();

        assert_eq!(first, second);
        assert_eq!(
            writer
                .uri_from("objects/characters/hero.gltf", &first)
                .unwrap(),
            "../../_shared/sha256/e94045d2493922f0ce901226bf668cfe954f6b503bfab1fef09370af7e972812.bin"
        );
        assert_eq!(
            std::fs::read(temp.path().join(first.relative_path())).unwrap(),
            b"same payload"
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
    }

    #[test]
    fn concurrent_publish_uses_one_shared_result() {
        let temp = tempfile::tempdir().unwrap();
        let writer = PackageWriter::new(temp.path()).unwrap();
        let handles = (0..8)
            .map(|_| {
                let writer = writer.clone();
                std::thread::spawn(move || writer.store(&vec![7; 64 * 1024], "dat").unwrap())
            })
            .collect::<Vec<_>>();
        let blobs = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(blobs.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            std::fs::read_dir(temp.path().join(SHARED_DIRECTORY))
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
