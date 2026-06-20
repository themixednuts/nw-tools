//! Locate a New World install through explicit paths, environment, or Steam metadata.

use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::process::Command;

use thiserror::Error;

pub const APP_ID: u32 = 1_063_730;
pub const APP_ID_STR: &str = "1063730";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    root: PathBuf,
    source: Source,
    steam_app: Option<SteamApp>,
}

impl Install {
    /// Resolve the install from `NEW_WORLD_DIR`, then Steam library metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] when neither the environment nor Steam metadata
    /// points at an installed copy.
    pub fn locate() -> Result<Self, Error> {
        if let Some(install) = Self::from_env()? {
            return Ok(install);
        }
        Self::from_steam().ok_or(Error::NotFound)
    }

    /// Build an install from an explicit game directory, `Bin64`, `NewWorld.exe`,
    /// or `assets` path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRoot`] when the normalized path does not look like
    /// a New World install.
    pub fn from_dir(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = normalize_root(path.into());
        if !Self::is_root(&root) {
            return Err(Error::InvalidRoot { path: root });
        }
        Ok(Self {
            root,
            source: Source::Explicit,
            steam_app: None,
        })
    }

    pub fn from_env() -> Result<Option<Self>, Error> {
        let Some(root) = env_path("NEW_WORLD_DIR").map(normalize_root) else {
            return Ok(None);
        };
        if !Self::is_root(&root) {
            return Err(Error::InvalidRoot { path: root });
        }
        Ok(Some(Self {
            root,
            source: Source::Env,
            steam_app: None,
        }))
    }

    #[must_use]
    pub fn from_steam() -> Option<Self> {
        let app = Steam::new_world_app()?;
        Some(Self {
            root: app.install_dir.clone(),
            source: Source::Steam,
            steam_app: Some(app),
        })
    }

    #[must_use]
    pub fn is_root(path: &Path) -> bool {
        path.join("Bin64").join("NewWorld.exe").is_file()
            || path.join("EasyAntiCheat").join("Settings.json").is_file()
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn source(&self) -> Source {
        self.source
    }

    #[must_use]
    pub fn steam_app(&self) -> Option<&SteamApp> {
        self.steam_app.as_ref()
    }

    #[must_use]
    pub fn bin64(&self) -> PathBuf {
        self.root.join("Bin64")
    }

    #[must_use]
    pub fn game_exe(&self) -> PathBuf {
        self.bin64().join("NewWorld.exe")
    }

    #[must_use]
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Explicit,
    Env,
    Steam,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit => f.write_str("explicit path"),
            Self::Env => f.write_str("NEW_WORLD_DIR"),
            Self::Steam => f.write_str("Steam library metadata"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamApp {
    pub app_id: u32,
    pub name: Option<String>,
    pub install_dir_name: String,
    pub install_dir: PathBuf,
    pub library_dir: PathBuf,
    pub manifest_path: PathBuf,
}

impl SteamApp {
    #[must_use]
    pub fn bin64(&self) -> PathBuf {
        self.install_dir.join("Bin64")
    }

    #[must_use]
    pub fn steam_api64_dll(&self) -> PathBuf {
        self.bin64().join("steam_api64.dll")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    pub app_id: Option<u32>,
    pub name: Option<String>,
    pub install_dir: Option<String>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not find New World; set NEW_WORLD_DIR or install Steam app {APP_ID_STR}")]
    NotFound,
    #[error("{path} does not look like a New World install")]
    InvalidRoot { path: PathBuf },
}

pub struct Steam;

impl Steam {
    #[must_use]
    pub fn new_world_app() -> Option<SteamApp> {
        Self::app(APP_ID)
    }

    #[must_use]
    pub fn app(app_id: u32) -> Option<SteamApp> {
        let manifest_name = format!("appmanifest_{app_id}.acf");

        for library_dir in Self::library_dirs() {
            let manifest_path = library_dir.join("steamapps").join(&manifest_name);
            let Ok(raw) = fs::read_to_string(&manifest_path) else {
                continue;
            };
            let manifest = AppManifest::parse(&raw);

            if manifest
                .app_id
                .is_some_and(|manifest_app_id| manifest_app_id != app_id)
            {
                continue;
            }

            let Some(install_dir_name) = manifest.install_dir else {
                continue;
            };
            let install_dir = library_dir
                .join("steamapps")
                .join("common")
                .join(&install_dir_name);
            if !install_dir.exists() {
                continue;
            }

            return Some(SteamApp {
                app_id,
                name: manifest.name,
                install_dir_name,
                install_dir,
                library_dir,
                manifest_path,
            });
        }

        None
    }

    #[must_use]
    pub fn library_dirs() -> Vec<PathBuf> {
        let mut libraries = Vec::new();
        for root in Self::roots() {
            push_unique_path(&mut libraries, root.clone());
            for library in LibraryFolders::from_root(&root).paths {
                push_unique_path(&mut libraries, library);
            }
        }
        libraries
    }

    #[must_use]
    pub fn roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        for name in ["STEAM_DIR", "STEAM_PATH", "STEAMROOT", "SteamPath"] {
            if let Some(path) = env_path(name).and_then(steam_root_from_env_path) {
                push_unique_path(&mut roots, path);
            }
        }

        #[cfg(windows)]
        {
            for (key, value) in [
                (r"HKCU\Software\Valve\Steam", "SteamPath"),
                (r"HKCU\Software\Valve\Steam", "SteamExe"),
                (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
            ] {
                if let Some(path) = read_windows_registry_value(key, value)
                    .map(PathBuf::from)
                    .and_then(steam_root_from_env_path)
                {
                    push_unique_path(&mut roots, path);
                }
            }

            if let Some(program_files_x86) = env_path("ProgramFiles(x86)") {
                push_unique_path(&mut roots, program_files_x86.join("Steam"));
            }
            if let Some(program_files) = env_path("ProgramFiles") {
                push_unique_path(&mut roots, program_files.join("Steam"));
            }
            push_unique_path(&mut roots, PathBuf::from(r"C:\Program Files (x86)\Steam"));
            push_unique_path(&mut roots, PathBuf::from(r"C:\Program Files\Steam"));
        }

        #[cfg(not(windows))]
        {
            if let Some(home) = env_path("HOME") {
                push_unique_path(&mut roots, home.join(".steam").join("steam"));
                push_unique_path(&mut roots, home.join(".local").join("share").join("Steam"));
            }
        }

        roots
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LibraryFolders {
    paths: Vec<PathBuf>,
}

impl LibraryFolders {
    fn parse(raw: &str) -> Self {
        let tokens = parse_vdf_quoted_strings(raw);
        let mut paths = Vec::new();

        for pair in tokens.windows(2) {
            if pair[0].eq_ignore_ascii_case("path") {
                push_unique_path(&mut paths, PathBuf::from(&pair[1]));
            }
        }

        Self { paths }
    }

    fn from_root(root: &Path) -> Self {
        let path = root.join("config").join("libraryfolders.vdf");
        fs::read_to_string(path)
            .map(|raw| Self::parse(&raw))
            .unwrap_or_default()
    }
}

impl AppManifest {
    fn parse(raw: &str) -> Self {
        let tokens = parse_vdf_quoted_strings(raw);
        let mut app_id = None;
        let mut name = None;
        let mut install_dir = None;

        for pair in tokens.windows(2) {
            match pair[0].as_str() {
                key if key.eq_ignore_ascii_case("appid") => {
                    app_id = pair[1].parse::<u32>().ok();
                }
                key if key.eq_ignore_ascii_case("name") => {
                    name = Some(pair[1].clone());
                }
                key if key.eq_ignore_ascii_case("installdir") => {
                    install_dir = Some(pair[1].clone());
                }
                _ => {}
            }
        }

        Self {
            app_id,
            name,
            install_dir,
        }
    }
}

fn normalize_root(path: PathBuf) -> PathBuf {
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("NewWorld.exe"))
    {
        return path
            .parent()
            .and_then(Path::parent)
            .map_or(path.clone(), Path::to_path_buf);
    }

    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Bin64"))
    {
        return path.parent().map_or(path.clone(), Path::to_path_buf);
    }

    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("assets"))
        && let Some(root) = path.parent()
    {
        return root.to_path_buf();
    }

    path
}

fn parse_vdf_quoted_strings(raw: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in raw.chars() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                current.clear();
            }
            continue;
        }

        if escape {
            match ch {
                '\\' => current.push('\\'),
                '"' => current.push('"'),
                'n' => current.push('\n'),
                'r' => current.push('\r'),
                't' => current.push('\t'),
                other => current.push(other),
            }
            escape = false;
            continue;
        }

        match ch {
            '\\' => escape = true,
            '"' => {
                strings.push(std::mem::take(&mut current));
                in_string = false;
            }
            other => current.push(other),
        }
    }

    strings
}

fn env_path(name: &str) -> Option<PathBuf> {
    let value = env::var(name).ok()?;
    let trimmed = value.trim().trim_matches('"').trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn steam_root_from_env_path(path: PathBuf) -> Option<PathBuf> {
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("steam.exe"))
    {
        return path.parent().map(Path::to_path_buf);
    }
    Some(path)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.as_os_str().is_empty() {
        return;
    }
    let key = path_key(&path);
    if paths.iter().any(|existing| path_key(existing) == key) {
        return;
    }
    paths.push(path);
}

fn path_key(path: &Path) -> String {
    let key = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        key.to_ascii_lowercase()
    } else {
        key
    }
}

#[cfg(windows)]
fn read_windows_registry_value(key: &str, value: &str) -> Option<String> {
    let output = Command::new("reg")
        .args(["query", key, "/v", value])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        if !name.eq_ignore_ascii_case(value) {
            continue;
        }
        let _kind = parts.next()?;
        let data = parts.collect::<Vec<_>>().join(" ");
        if !data.is_empty() {
            return Some(data);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_libraryfolders_paths() {
        let raw = r#"
            "libraryfolders"
            {
                "0"
                {
                    "path" "C:\\Program Files (x86)\\Steam"
                    "apps"
                    {
                        "730" "6524196981"
                    }
                }
                "1"
                {
                    "path" "D:\\SteamLibrary"
                    "apps"
                    {
                        "1063730" "76416676920"
                    }
                }
            }
        "#;

        let folders = LibraryFolders::parse(raw);
        assert_eq!(folders.paths.len(), 2);
        assert_eq!(
            folders.paths[0],
            PathBuf::from(r"C:\Program Files (x86)\Steam")
        );
        assert_eq!(folders.paths[1], PathBuf::from(r"D:\SteamLibrary"));
    }

    #[test]
    fn parses_appmanifest() {
        let raw = r#"
            "AppState"
            {
                "appid" "1063730"
                "name" "New World: Aeternum"
                "installdir" "New World"
            }
        "#;

        let manifest = AppManifest::parse(raw);
        assert_eq!(manifest.app_id, Some(APP_ID));
        assert_eq!(manifest.name.as_deref(), Some("New World: Aeternum"));
        assert_eq!(manifest.install_dir.as_deref(), Some("New World"));
    }

    #[test]
    fn normalizes_install_related_paths() {
        assert_eq!(
            normalize_root(PathBuf::from(
                r"D:\SteamLibrary\steamapps\common\New World\Bin64"
            )),
            PathBuf::from(r"D:\SteamLibrary\steamapps\common\New World")
        );
        assert_eq!(
            normalize_root(PathBuf::from(
                r"D:\SteamLibrary\steamapps\common\New World\Bin64\NewWorld.exe"
            )),
            PathBuf::from(r"D:\SteamLibrary\steamapps\common\New World")
        );
        assert_eq!(
            normalize_root(PathBuf::from(
                r"D:\SteamLibrary\steamapps\common\New World\assets"
            )),
            PathBuf::from(r"D:\SteamLibrary\steamapps\common\New World")
        );
    }
}
