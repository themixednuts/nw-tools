use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use nw_filesystem::display_relative;
use nw_objectstream::lookup::NameLookup;

#[derive(Debug, Clone, Default)]
pub struct GlobSet {
    patterns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PathSelector {
    text: Option<String>,
    globs: GlobSet,
}

impl GlobSet {
    pub fn archive(patterns: Vec<String>) -> Self {
        Self {
            patterns: patterns
                .into_iter()
                .map(|pattern| nw_filesystem::normalize_archive_path(&pattern))
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn matches(&self, value: &str) -> bool {
        let value = value.to_ascii_lowercase();
        self.patterns
            .iter()
            .any(|pattern| wildcard_matches(pattern, &value))
    }
}

impl PathSelector {
    pub fn new(text: Option<String>, globs: Vec<String>) -> Self {
        Self {
            text: text.and_then(|text| {
                let text = text.trim().to_ascii_lowercase();
                (!text.is_empty()).then_some(text)
            }),
            globs: GlobSet::archive(globs),
        }
    }

    pub fn matches(&self, path: &str) -> bool {
        let text_matches = self
            .text
            .as_ref()
            .is_none_or(|text| path.to_ascii_lowercase().contains(text));
        text_matches && (self.globs.is_empty() || self.globs.matches(path))
    }
}

#[derive(Debug, Clone)]
pub struct PakSet {
    root: PathBuf,
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct AssetRootArg {
    #[arg(long, value_name = "ROOT")]
    root: Option<PathBuf>,
}

impl AssetRootArg {
    pub fn resolve(&self) -> Result<PathBuf> {
        match &self.root {
            Some(root) => Ok(root.clone()),
            None => Ok(nw_locator::Install::locate()?.assets()),
        }
    }
}

impl PakSet {
    pub fn collect(root: PathBuf, patterns: Vec<String>) -> Result<Self> {
        let filter = GlobSet::archive(patterns);
        let mut paths = collect_paks(&root)?;
        if !filter.is_empty() {
            paths.retain(|path| {
                filter.matches(&display_relative(&root, path))
                    || path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| filter.matches(name))
            });
        }

        Ok(Self { root, paths })
    }

    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    #[must_use]
    pub fn relative(&self, pak: &Path) -> String {
        let relative = display_relative(&self.root, pak);
        if relative.is_empty() {
            pak.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        } else {
            relative
        }
    }

    #[must_use]
    pub fn mount_root(&self, pak: &Path) -> String {
        let relative = PathBuf::from(self.relative(pak));
        relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.display().to_string().replace('\\', "/"))
            .unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct ScanIssues {
    label: &'static str,
    skipped: usize,
    cancelled: bool,
    errors: Vec<anyhow::Error>,
}

impl ScanIssues {
    #[must_use]
    pub fn new(
        label: &'static str,
        skipped: usize,
        cancelled: bool,
        errors: Vec<anyhow::Error>,
    ) -> Self {
        Self {
            label,
            skipped,
            cancelled,
            errors,
        }
    }

    pub fn finish(self) -> Result<()> {
        if self.cancelled {
            bail!(
                "{} cancelled ({} queued item(s) skipped)",
                self.label,
                self.skipped
            );
        }
        if !self.errors.is_empty() {
            for error in self.errors.iter().take(12) {
                eprintln!("{error:#}");
            }
            bail!("{} {} item(s) failed", self.errors.len(), self.label);
        }
        Ok(())
    }
}

pub fn collect_paks(path: &Path) -> Result<Vec<PathBuf>> {
    collect_matching(path, is_pak)
}

pub fn collect_matching(path: &Path, keep: impl Fn(&Path) -> bool + Copy) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_matching_inner(path, keep, &mut out)?;
    out.sort();
    Ok(out)
}

pub fn load_lookup(disabled: bool) -> Result<Option<NameLookup>> {
    if disabled {
        return Ok(None);
    }
    Ok(Some(NameLookup::from_serialize_json(
        nw_resources::SERIALIZE_JSON,
    )?))
}

pub fn path_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

/// What to do when an output file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overwrite {
    /// Replace the existing file.
    Replace,
    /// Keep it and refuse to write.
    Keep,
}

impl From<bool> for Overwrite {
    /// Maps a `--overwrite` flag: `true` ⇒ [`Overwrite::Replace`].
    fn from(replace: bool) -> Self {
        if replace { Self::Replace } else { Self::Keep }
    }
}

/// Refuse to clobber `path` under [`Overwrite::Keep`].
///
/// # Errors
///
/// Returns an error if the file exists and `overwrite` is [`Overwrite::Keep`].
pub fn guard_existing(path: &Path, overwrite: Overwrite) -> Result<()> {
    if overwrite == Overwrite::Keep && path.exists() {
        bail!("{} exists (pass --overwrite to replace it)", path.display());
    }
    Ok(())
}

/// Create `path`'s parent directory if it has one.
///
/// # Errors
///
/// Returns an error if the directory cannot be created.
pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

/// Write `bytes` to `path`, guarding against clobber and creating parent dirs.
///
/// # Errors
///
/// Returns an error if `path` exists under [`Overwrite::Keep`], the parent cannot
/// be created, or the write fails.
pub fn write_guarded(path: &Path, bytes: &[u8], overwrite: Overwrite) -> Result<()> {
    guard_existing(path, overwrite)?;
    ensure_parent(path)?;
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn collect_matching_inner(
    path: &Path,
    keep: impl Fn(&Path) -> bool + Copy,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    if path.is_file() {
        if keep(path) {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }

    if !path.exists() {
        bail!("no such file or directory: {}", path.display());
    }

    for entry in
        std::fs::read_dir(path).with_context(|| format!("scan directory {}", path.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_matching_inner(&entry.path(), keep, out)?;
        } else if file_type.is_file() && keep(&entry.path()) {
            out.push(entry.path());
        }
    }
    Ok(())
}

fn is_pak(path: &Path) -> bool {
    path_ext(path).is_some_and(|extension| extension == "pak")
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let value = value.replace('\\', "/");
    wildcard_matches_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_matches_bytes(pattern: &[u8], value: &[u8]) -> bool {
    let (mut pattern_index, mut value_index) = (0usize, 0usize);
    let (mut star_index, mut retry_value_index) = (None, 0usize);

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            retry_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            retry_value_index += 1;
            value_index = retry_value_index;
        } else {
            return false;
        }
    }

    pattern[pattern_index..].iter().all(|byte| *byte == b'*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pak_mount_root_omits_root_level_pak_name() {
        let paks = PakSet {
            root: PathBuf::from("assets"),
            paths: vec![PathBuf::from("assets/Engine.pak")],
        };

        assert_eq!(paks.mount_root(Path::new("assets/Engine.pak")), "");
    }

    #[test]
    fn pak_mount_root_preserves_nested_parent() {
        let paks = PakSet {
            root: PathBuf::from("assets"),
            paths: vec![PathBuf::from("assets/levels/ftue_v2/level.pak")],
        };

        assert_eq!(
            paks.mount_root(Path::new("assets/levels/ftue_v2/level.pak")),
            "levels/ftue_v2"
        );
    }

    #[test]
    fn path_selector_combines_text_and_globs() {
        let selector = PathSelector::new(
            Some("Player".to_string()),
            vec!["slices/**/*.slice".to_string()],
        );

        assert!(selector.matches("slices/characters/player.slice"));
        assert!(!selector.matches("slices/characters/player.dds"));
        assert!(!selector.matches("slices/characters/npc.slice"));
    }
}
