//! Bundled reflection dumps, plus a process-local overlay when the CLI
//! points at replacement files.
//!
//! The persisted index always ingests [`ResourceView::embedded`].
//! [`install_session`] extracts overlay facts once and does not rewrite sqlite.

use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result};

use crate::grep::adapter::FactDraft;
use crate::index::EntryFormat;

/// Virtual pak name for the bundled reflection dumps.
pub const PAK_NAME: &str = "nw-resources";

const SERIALIZE_PATH: &str = "serialize.json";
const BEHAVIOR_PATH: &str = "behavior-context.json";

static SESSION: RwLock<Option<Arc<Installed>>> = RwLock::new(None);

struct Installed {
    overlay: SessionOverlay,
    serialize: Option<Bytes>,
}

/// Already-extracted facts for replaced bundled dumps.
#[derive(Debug, Clone, Default)]
pub struct SessionOverlay {
    pub(crate) replaced: Vec<String>,
    pub(crate) rows: Vec<OverlayRow>,
}

#[derive(Debug, Clone)]
pub(crate) struct OverlayRow {
    pub entry: String,
    pub format: EntryFormat,
    pub facts: Vec<FactDraft>,
}

impl SessionOverlay {
    pub(crate) fn embedded() -> &'static Self {
        static EMBEDDED: OnceLock<SessionOverlay> = OnceLock::new();
        EMBEDDED.get_or_init(|| Self {
            replaced: Vec::new(),
            rows: ResourceView::embedded()
                .files()
                .map(|(entry, bytes)| {
                    let extracted = crate::extract::bytes(&entry, bytes);
                    OverlayRow {
                        entry,
                        format: extracted.format,
                        facts: extracted.facts,
                    }
                })
                .collect(),
        })
    }

    /// Extract facts for every replaced dump in `view`.
    #[must_use]
    pub fn extract(view: &ResourceView) -> Self {
        let mut overlay = Self::default();
        if view.embedded {
            return overlay;
        }
        for (path, bytes) in view.files() {
            overlay.replaced.push(path.clone());
            let extracted = crate::extract::bytes(&path, bytes);
            overlay.rows.push(OverlayRow {
                entry: path,
                format: extracted.format,
                facts: extracted.facts,
            });
        }
        overlay
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.replaced.is_empty()
    }

    #[must_use]
    pub fn replaces(&self, path: &str) -> bool {
        self.replaced.iter().any(|entry| entry == path)
    }
}

/// Installed session, or bundled serialize when none was set.
pub struct SessionHandle {
    installed: Option<Arc<Installed>>,
}

impl SessionHandle {
    #[must_use]
    pub fn serialize(&self) -> &[u8] {
        self.installed
            .as_ref()
            .and_then(|installed| installed.serialize.as_ref())
            .map(Bytes::as_slice)
            .unwrap_or(nw_resources::SERIALIZE_JSON)
    }

    #[must_use]
    pub fn overlay(&self) -> &SessionOverlay {
        match &self.installed {
            Some(installed) => &installed.overlay,
            None => empty_overlay(),
        }
    }
}

fn empty_overlay() -> &'static SessionOverlay {
    static EMPTY: OnceLock<SessionOverlay> = OnceLock::new();
    EMPTY.get_or_init(SessionOverlay::default)
}

/// Bundled dumps, or a delta of replacements.
#[derive(Debug, Clone)]
pub struct ResourceView {
    serialize: Option<Bytes>,
    behavior: Option<Bytes>,
    modules: Option<Vec<(String, Bytes)>>,
    embedded: bool,
}

#[derive(Debug, Clone)]
enum Bytes {
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl Bytes {
    fn from_static(bytes: &'static [u8]) -> Self {
        Self::Static(bytes)
    }

    fn from_owned(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes.into())
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Static(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}

impl ResourceView {
    /// The copies compiled into `nw-resources`. Always what the index persists.
    #[must_use]
    pub fn embedded() -> Self {
        Self {
            serialize: Some(Bytes::from_static(nw_resources::SERIALIZE_JSON)),
            behavior: Some(Bytes::from_static(nw_resources::behavior_context_json())),
            modules: Some(
                nw_resources::MODULE_DESCRIPTORS
                    .iter()
                    .map(|module| (module.path.to_owned(), Bytes::from_static(module.bytes)))
                    .collect(),
            ),
            embedded: true,
        }
    }

    /// Load replacements from disk. Omitted paths are not in this view.
    ///
    /// # Errors
    ///
    /// Returns an error if a given file or modules directory cannot be read.
    pub fn from_overrides(
        serialize: Option<&Path>,
        behavior_context: Option<&Path>,
        modules: Option<&Path>,
    ) -> Result<Self> {
        Ok(Self {
            serialize: serialize
                .map(|path| {
                    std::fs::read(path)
                        .map(Bytes::from_owned)
                        .with_context(|| format!("read serialize override {}", path.display()))
                })
                .transpose()?,
            behavior: behavior_context
                .map(|path| load_behavior_override(path).map(Bytes::from_owned))
                .transpose()?,
            modules: modules.map(load_module_overrides).transpose()?,
            embedded: false,
        })
    }

    #[must_use]
    pub fn serialize(&self) -> &[u8] {
        self.serialize
            .as_ref()
            .map(Bytes::as_slice)
            .unwrap_or(nw_resources::SERIALIZE_JSON)
    }

    #[must_use]
    pub fn behavior_context(&self) -> &[u8] {
        self.behavior
            .as_ref()
            .map(Bytes::as_slice)
            .unwrap_or_else(|| nw_resources::behavior_context_json())
    }

    #[must_use]
    pub fn has_overrides(&self) -> bool {
        !self.embedded
            && (self.serialize.is_some() || self.behavior.is_some() || self.modules.is_some())
    }

    #[must_use]
    pub fn overrides_path(&self, path: &str) -> bool {
        if self.embedded {
            return false;
        }
        match path {
            SERIALIZE_PATH => self.serialize.is_some(),
            BEHAVIOR_PATH => self.behavior.is_some(),
            other => self.modules.is_some() && other.starts_with("modules/"),
        }
    }

    /// Dumps present in this view.
    pub fn files(&self) -> impl Iterator<Item = (String, &[u8])> + '_ {
        self.serialize
            .as_ref()
            .map(|bytes| (SERIALIZE_PATH.to_owned(), bytes.as_slice()))
            .into_iter()
            .chain(
                self.behavior
                    .as_ref()
                    .map(|bytes| (BEHAVIOR_PATH.to_owned(), bytes.as_slice())),
            )
            .chain(
                self.modules
                    .iter()
                    .flatten()
                    .map(|(path, bytes)| (path.clone(), bytes.as_slice())),
            )
    }

    /// Size + content signature used as the virtual-pak resume stamp.
    #[must_use]
    pub fn stamp(&self) -> (i64, i64) {
        if self.embedded {
            Self::embedded_stamp()
        } else {
            stamp_files(self.files())
        }
    }

    pub(crate) fn embedded_stamp() -> (i64, i64) {
        static STAMP: OnceLock<(i64, i64)> = OnceLock::new();
        *STAMP.get_or_init(|| {
            stamp_files(
                [
                    (SERIALIZE_PATH, nw_resources::SERIALIZE_JSON),
                    ("behavior-context.7z", nw_resources::BEHAVIOR_CONTEXT_7Z),
                ]
                .into_iter()
                .chain(
                    nw_resources::MODULE_DESCRIPTORS
                        .iter()
                        .map(|module| (module.path, module.bytes)),
                ),
            )
        })
    }
}

fn stamp_files<'a, P: AsRef<str>>(files: impl IntoIterator<Item = (P, &'a [u8])>) -> (i64, i64) {
    let mut size = 0i64;
    let mut signature = blake3::Hasher::new();
    for (path, bytes) in files {
        let path = path.as_ref();
        size = size.saturating_add(i64::try_from(bytes.len()).unwrap_or(i64::MAX));
        signature.update(&(path.len() as u64).to_le_bytes());
        signature.update(path.as_bytes());
        signature.update(&(bytes.len() as u64).to_le_bytes());
        signature.update(bytes);
    }
    let mut stamp = [0; 8];
    stamp.copy_from_slice(&signature.finalize().as_bytes()[..8]);
    (size, i64::from_le_bytes(stamp))
}

/// Extract overlay facts for this process. Does not rewrite the persisted index.
pub fn install_session(view: ResourceView) {
    let serialize = if view.embedded {
        None
    } else {
        view.serialize.clone()
    };
    let overlay = SessionOverlay::extract(&view);
    *SESSION.write().expect("resource session") = Some(Arc::new(Installed { overlay, serialize }));
}

/// The installed overlay, or bundled serialize when none was set.
#[must_use]
pub fn session() -> SessionHandle {
    SessionHandle {
        installed: SESSION.read().expect("resource session").clone(),
    }
}

fn load_behavior_override(path: &Path) -> Result<Vec<u8>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read behavior-context {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.ends_with(".7z") {
        nw_resources::decompress_behavior_context(&bytes)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("decompress behavior-context {}", path.display()))
    } else {
        Ok(bytes)
    }
}

fn load_module_overrides(dir: &Path) -> Result<Vec<(String, Bytes)>> {
    let mut modules = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read modules directory {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read module override {}", path.display()))?;
        modules.push((format!("modules/{name}"), Bytes::from_owned(bytes)));
    }
    Ok(modules)
}
