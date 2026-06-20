use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[must_use]
pub fn normalize_archive_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/").trim().to_ascii_lowercase();
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized.trim_start_matches('/').to_string()
}

pub fn archive_extension_key(path: &str) -> Option<String> {
    let normalized = normalize_archive_path(path);
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .map(str::to_string)
}

#[must_use]
pub fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Join an archive-relative path onto a filesystem root.
///
/// # Errors
///
/// Returns [`SafeJoinError`] if `child` is absolute or contains a parent
/// directory component.
pub fn safe_join(root: &Path, child: &str) -> Result<PathBuf, SafeJoinError> {
    let child = Path::new(child);
    if child.is_absolute() {
        return Err(SafeJoinError::AbsolutePath);
    }

    let mut out = root.to_path_buf();
    for component in child.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(SafeJoinError::ParentDir(child.display().to_string()));
            }
            Component::Prefix(_) | Component::RootDir => return Err(SafeJoinError::AbsolutePath),
        }
    }
    Ok(out)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafeJoinError {
    #[error("absolute paths are not accepted")]
    AbsolutePath,
    #[error("archive path contains a parent-directory component: {0}")]
    ParentDir(String),
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{SafeJoinError, archive_extension_key, normalize_archive_path, safe_join};

    #[test]
    fn normalizes_archive_paths() {
        assert_eq!(
            normalize_archive_path("./Textures\\Foo.DDS"),
            "textures/foo.dds"
        );
    }

    #[test]
    fn reads_extension_key() {
        assert_eq!(
            archive_extension_key("Textures/Foo.DDS").as_deref(),
            Some("dds")
        );
    }

    #[test]
    fn safe_join_rejects_escape() {
        let error = safe_join(Path::new("root"), "../x").unwrap_err();
        assert_eq!(error, SafeJoinError::ParentDir("../x".to_string()));
    }
}
