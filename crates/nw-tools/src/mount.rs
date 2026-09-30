use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;

#[derive(Debug, Clone)]
pub(crate) struct MountedPath {
    rel: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PathClaims {
    inner: Arc<Mutex<BTreeSet<String>>>,
}

impl MountedPath {
    pub(crate) fn new(root: &Path, mount_root: &str, entry: &str) -> Result<Self> {
        Self::with_added_extension(root, mount_root, entry, None)
    }

    pub(crate) fn with_added_extension(
        root: &Path,
        mount_root: &str,
        entry: &str,
        extension: Option<&'static str>,
    ) -> Result<Self> {
        let mut rel = mounted_rel(mount_root, entry);
        if let Some(extension) = extension {
            rel.push('.');
            rel.push_str(extension);
        }
        let path = nw_filesystem::safe_join(root, &rel)?;
        Ok(Self { rel, path })
    }

    pub(crate) fn rel(&self) -> &str {
        &self.rel
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn display(&self) -> String {
        self.path.display().to_string()
    }
}

impl PathClaims {
    pub(crate) fn claim(&self, path: &MountedPath) -> bool {
        let mut claimed = self.inner.lock().expect("path claims lock poisoned");
        claimed.insert(path.rel().to_string())
    }
}

fn mounted_rel(mount_root: &str, entry: &str) -> String {
    let entry = nw_filesystem::normalize_archive_path(entry);
    let mount_root = nw_filesystem::normalize_archive_path(mount_root);
    if mount_root.is_empty() {
        entry
    } else if entry.is_empty() {
        mount_root
    } else {
        format!("{mount_root}/{entry}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_path_does_not_leak_top_level_pak_name() {
        assert_eq!(
            mounted_rel("", "slices/globalgde.dynamicslice"),
            "slices/globalgde.dynamicslice"
        );
    }

    #[test]
    fn mounted_path_preserves_nested_pak_parent() {
        assert_eq!(
            mounted_rel("levels/ftue_v2", "leveldata.xml"),
            "levels/ftue_v2/leveldata.xml"
        );
    }
}
