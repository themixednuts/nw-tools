//! Localization XML parsing, locale manifests, and text lookup.

#![forbid(unsafe_code)]

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, hash_map::Entry},
    fmt, fs, io,
    path::{Path, PathBuf},
    str::{self, FromStr},
};

use quick_xml::de::from_str;
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, MapAccess, Visitor},
};
use thiserror::Error;

pub const LANGUAGE_MANIFEST_ASSET_PATH: &str = "localization/localization.xml";
pub const TAG_MANIFEST_ASSET_PATH: &str = "libs/localization/localization.xml";
pub const INIT_LOCALIZATION_TAG: &str = "init";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LocalizationError {
    #[error("read language manifest {path:?}: {source}")]
    ReadLanguageManifest { path: PathBuf, source: io::Error },
    #[error("parse language manifest {path:?}: {source}")]
    ParseLanguageManifest {
        path: PathBuf,
        source: Box<LocalizationError>,
    },
    #[error("read source manifest {path:?}: {source}")]
    ReadSourceManifest { path: PathBuf, source: io::Error },
    #[error("parse source manifest {path:?}: {source}")]
    ParseSourceManifest {
        path: PathBuf,
        source: Box<LocalizationError>,
    },
    #[error("parse localization XML: {0}")]
    Xml(#[from] quick_xml::DeError),
    #[error("localization XML is not UTF-8: {0}")]
    Utf8(#[from] str::Utf8Error),
    #[error("<string> entry is missing required key attribute")]
    MissingKey,
    #[error("invalid language code `{value}`: {source}")]
    InvalidLanguage {
        value: Box<str>,
        source: LanguageCodeError,
    },
    #[error("localization tag `{tag}` is not present in {manifest:?}")]
    UnknownTag { tag: Box<str>, manifest: PathBuf },
    #[error("read localization source {path:?}: {source}")]
    ReadSource { path: PathBuf, source: io::Error },
    #[error("parse localization source {path:?}: {source}")]
    ParseSource {
        path: PathBuf,
        source: Box<LocalizationError>,
    },
    #[error("localization source {path:?} key `{key}` is invalid: {source}")]
    InvalidSourceKey {
        path: PathBuf,
        key: Box<str>,
        source: LocalizationKeyError,
    },
    #[error(transparent)]
    Asset(#[from] nw_asset::AssetStoreError),
}

pub type LocalizationParseError = LocalizationError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageCode {
    code: Box<str>,
    asset_folder: Box<str>,
}

impl LanguageCode {
    pub fn new(value: impl AsRef<str>) -> Result<Self, LanguageCodeError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(LanguageCodeError::Empty);
        }
        if !is_language_code(value) {
            return Err(LanguageCodeError::InvalidShape {
                value: value.into(),
            });
        }
        Ok(Self {
            code: value.into(),
            asset_folder: value.to_ascii_lowercase().into_boxed_str(),
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn asset_folder(&self) -> &str {
        &self.asset_folder
    }
}

impl AsRef<str> for LanguageCode {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LanguageCode {
    type Err = LanguageCodeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LanguageCodeError {
    #[error("language code is empty")]
    Empty,
    #[error("expected a BCP-47-style language code such as `en-US`: `{value}`")]
    InvalidShape { value: Box<str> },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalizationTag(Box<str>);

impl LocalizationTag {
    pub fn new(value: impl AsRef<str>) -> Result<Self, LocalizationTagError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(LocalizationTagError::Empty);
        }
        Ok(Self(value.into()))
    }

    #[must_use]
    pub fn init() -> Self {
        Self(INIT_LOCALIZATION_TAG.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for LocalizationTag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for LocalizationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LocalizationTag {
    type Err = LocalizationTagError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LocalizationTagError {
    #[error("localization tag is empty")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalizationKey {
    canonical: Box<str>,
    crc32: u32,
}

impl LocalizationKey {
    pub fn from_key(value: impl AsRef<str>) -> Result<Self, LocalizationKeyError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(LocalizationKeyError::Empty);
        }
        if value.starts_with('@') {
            return Err(LocalizationKeyError::UnexpectedLabel {
                value: value.into(),
            });
        }
        Ok(Self::from_key_unchecked(value))
    }

    pub fn from_label(value: impl AsRef<str>) -> Result<Self, LocalizationKeyError> {
        let value = value.as_ref().trim();
        let Some(key) = value.strip_prefix('@') else {
            return Err(LocalizationKeyError::ExpectedLabel {
                value: value.into(),
            });
        };
        if key.is_empty() {
            return Err(LocalizationKeyError::Empty);
        }
        Ok(Self::from_key_unchecked(key))
    }

    pub fn from_source_key(value: impl AsRef<str>) -> Result<Self, LocalizationKeyError> {
        let value = value.as_ref().trim();
        let key = value.strip_prefix('@').unwrap_or(value);
        if key.is_empty() {
            return Err(LocalizationKeyError::Empty);
        }
        Ok(Self::from_key_unchecked(key))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    #[must_use]
    pub const fn crc32(&self) -> u32 {
        self.crc32
    }

    fn from_key_unchecked(value: &str) -> Self {
        let canonical = value.to_ascii_lowercase().into_boxed_str();
        let crc32 = crc32fast::hash(canonical.as_bytes());
        Self { canonical, crc32 }
    }
}

impl AsRef<str> for LocalizationKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LocalizationKeyError {
    #[error("localization key is empty")]
    Empty,
    #[error("localization label must start with `@`: `{value}`")]
    ExpectedLabel { value: Box<str> },
    #[error("localization key must not start with `@`: `{value}`")]
    UnexpectedLabel { value: Box<str> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationAttribute {
    name: Box<str>,
    value: Box<str>,
}

impl LocalizationAttribute {
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, value: impl Into<Box<str>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedText {
    key: LocalizationKey,
    source_key: Box<str>,
    text: Box<str>,
    tag: Box<str>,
    source_path: Box<str>,
    attributes: Box<[LocalizationAttribute]>,
}

impl LocalizedText {
    #[must_use]
    pub fn key(&self) -> &LocalizationKey {
        &self.key
    }

    #[must_use]
    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    #[must_use]
    pub fn attributes(&self) -> &[LocalizationAttribute] {
        &self.attributes
    }

    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name() == name)
            .map(LocalizationAttribute::value)
    }
}

pub trait LocalizedTextResolver: fmt::Debug {
    fn localized_text(&self, key: &LocalizationKey) -> Option<&LocalizedText>;

    #[must_use]
    fn text(&self, key: &LocalizationKey) -> Option<&str> {
        self.localized_text(key).map(LocalizedText::text)
    }

    fn label_text(&self, label: &str) -> Result<Option<&str>, LocalizationKeyError> {
        let key = LocalizationKey::from_label(label)?;
        Ok(self.text(&key))
    }

    #[must_use]
    fn localize_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        localize_labels(self, text)
    }
}

impl<T> LocalizedTextResolver for &T
where
    T: LocalizedTextResolver + ?Sized,
{
    fn localized_text(&self, key: &LocalizationKey) -> Option<&LocalizedText> {
        T::localized_text(*self, key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationDocument {
    entries: Box<[LocalizationEntry]>,
}

impl LocalizationDocument {
    #[must_use]
    pub fn entries(&self) -> &[LocalizationEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, LocalizationError> {
        Self::parse_str(str::from_utf8(bytes)?)
    }

    pub fn parse_str(xml: &str) -> Result<Self, LocalizationError> {
        ResourcesXml::parse(xml)
    }
}

impl<'doc> IntoIterator for &'doc LocalizationDocument {
    type IntoIter = std::slice::Iter<'doc, LocalizationEntry>;
    type Item = &'doc LocalizationEntry;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalizationEntry {
    String(LocalizationString),
    Nil(LocalizationNil),
}

impl LocalizationEntry {
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.as_string().map(LocalizationString::key)
    }

    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::String(entry) => entry.value(),
            Self::Nil(entry) => entry.value(),
        }
    }

    #[must_use]
    pub fn attributes(&self) -> &[LocalizationAttribute] {
        match self {
            Self::String(entry) => entry.attributes(),
            Self::Nil(entry) => entry.attributes(),
        }
    }

    #[must_use]
    pub const fn as_string(&self) -> Option<&LocalizationString> {
        match self {
            Self::String(entry) => Some(entry),
            Self::Nil(_) => None,
        }
    }

    #[must_use]
    pub const fn is_nil(&self) -> bool {
        matches!(self, Self::Nil(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationString {
    key: Box<str>,
    value: Box<str>,
    attributes: Box<[LocalizationAttribute]>,
}

impl LocalizationString {
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn attributes(&self) -> &[LocalizationAttribute] {
        &self.attributes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationNil {
    value: Box<str>,
    attributes: Box<[LocalizationAttribute]>,
}

impl LocalizationNil {
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn attributes(&self) -> &[LocalizationAttribute] {
        &self.attributes
    }
}

#[derive(Debug, Deserialize)]
struct ResourcesXml {
    #[serde(rename = "string", default)]
    strings: Vec<ResourceStringXml>,
}

impl ResourcesXml {
    fn parse(xml: &str) -> Result<LocalizationDocument, LocalizationError> {
        let parsed: Self = from_str(xml)?;
        let mut entries = Vec::with_capacity(parsed.strings.len());

        for entry in parsed.strings {
            let attributes = attributes_from_map(entry.attributes);
            if entry.key.as_deref().is_none_or(str::is_empty) {
                if has_nil_attr(&attributes) {
                    entries.push(LocalizationEntry::Nil(LocalizationNil {
                        value: normalized_text(&entry.text).into_owned().into_boxed_str(),
                        attributes,
                    }));
                    continue;
                }
                return Err(LocalizationError::MissingKey);
            }

            entries.push(LocalizationEntry::String(LocalizationString {
                key: entry.key.unwrap_or_default().into_boxed_str(),
                value: normalized_text(&entry.text).into_owned().into_boxed_str(),
                attributes,
            }));
        }

        Ok(LocalizationDocument {
            entries: entries.into_boxed_slice(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ResourceStringXml {
    #[serde(rename = "@key", default)]
    key: Option<String>,
    #[serde(rename = "$text", default)]
    text: String,
    #[serde(flatten)]
    attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageManifest {
    languages: Box<[LanguageEntry]>,
}

impl LanguageManifest {
    pub fn load_from_asset_root(asset_root: impl AsRef<Path>) -> Result<Self, LocalizationError> {
        let path = asset_root.as_ref().join(LANGUAGE_MANIFEST_ASSET_PATH);
        let bytes = fs::read(&path).map_err(|source| LocalizationError::ReadLanguageManifest {
            path: path.clone(),
            source,
        })?;
        Self::parse_bytes(&bytes).map_err(|source| LocalizationError::ParseLanguageManifest {
            path,
            source: Box::new(source),
        })
    }

    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, LocalizationError> {
        Self::parse_str(str::from_utf8(bytes)?)
    }

    pub fn parse_str(source: &str) -> Result<Self, LocalizationError> {
        let parsed: LanguageManifestXml = from_str(source)?;
        let mut languages = Vec::with_capacity(parsed.languages.entries.len());
        for language in parsed.languages.entries {
            let code = LanguageCode::new(language.code.trim()).map_err(|source| {
                LocalizationError::InvalidLanguage {
                    value: language.code.trim().into(),
                    source,
                }
            })?;
            languages.push(LanguageEntry {
                code,
                attributes: attributes_from_map(language.attributes),
            });
        }
        Ok(Self {
            languages: languages.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn languages(&self) -> &[LanguageEntry] {
        &self.languages
    }
}

#[derive(Debug, Deserialize)]
struct LanguageManifestXml {
    languages: LanguageEntriesXml,
}

#[derive(Debug, Deserialize)]
struct LanguageEntriesXml {
    #[serde(rename = "language", default)]
    entries: Vec<LanguageXml>,
}

#[derive(Debug, Deserialize)]
struct LanguageXml {
    #[serde(rename = "$text", default)]
    code: String,
    #[serde(flatten)]
    attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEntry {
    code: LanguageCode,
    attributes: Box<[LocalizationAttribute]>,
}

impl LanguageEntry {
    #[must_use]
    pub fn code(&self) -> &LanguageCode {
        &self.code
    }

    #[must_use]
    pub fn has_sounds(&self) -> bool {
        self.attribute("sounds")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    }

    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name() == name)
            .map(LocalizationAttribute::value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifest {
    tags: HashMap<Box<str>, Box<[SourceManifestEntry]>>,
}

impl SourceManifest {
    pub fn load_from_asset_root(asset_root: impl AsRef<Path>) -> Result<Self, LocalizationError> {
        let path = asset_root.as_ref().join(TAG_MANIFEST_ASSET_PATH);
        let bytes = fs::read(&path).map_err(|source| LocalizationError::ReadSourceManifest {
            path: path.clone(),
            source,
        })?;
        Self::parse_bytes(&bytes).map_err(|source| LocalizationError::ParseSourceManifest {
            path,
            source: Box::new(source),
        })
    }

    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, LocalizationError> {
        Self::parse_str(str::from_utf8(bytes)?)
    }

    pub fn parse_str(source: &str) -> Result<Self, LocalizationError> {
        let parsed: SourceManifestXml = from_str(source)?;
        let tags = parsed
            .tags
            .into_iter()
            .filter(|(tag, _)| !tag.starts_with('@'))
            .map(|(tag, tag_xml)| {
                let entries = tag_xml
                    .entries
                    .into_iter()
                    .map(|entry| SourceManifestEntry {
                        file_name: entry.file_name.trim().into(),
                        attributes: attributes_from_map(entry.attributes),
                    })
                    .collect::<Box<_>>();
                (tag.into_boxed_str(), entries)
            })
            .collect();
        Ok(Self { tags })
    }

    #[must_use]
    pub fn tag(&self, tag: &LocalizationTag) -> Option<&[SourceManifestEntry]> {
        self.tags.get(tag.as_str()).map(Box::as_ref)
    }

    pub fn tags(&self) -> impl Iterator<Item = (&str, &[SourceManifestEntry])> {
        self.tags
            .iter()
            .map(|(tag, entries)| (tag.as_ref(), entries.as_ref()))
    }
}

#[derive(Debug)]
struct SourceManifestXml {
    tags: BTreeMap<String, SourceTagXml>,
}

impl<'de> Deserialize<'de> for SourceManifestXml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(SourceManifestVisitor)
    }
}

struct SourceManifestVisitor;

impl<'de> Visitor<'de> for SourceManifestVisitor {
    type Value = SourceManifestXml;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("localization source manifest")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut tags = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.starts_with('@') {
                map.next_value::<IgnoredAny>()?;
                continue;
            }
            tags.insert(key, map.next_value()?);
        }
        Ok(SourceManifestXml { tags })
    }
}

#[derive(Debug, Deserialize)]
struct SourceTagXml {
    #[serde(rename = "entry", default, deserialize_with = "one_or_many")]
    entries: Vec<SourceEntryXml>,
}

#[derive(Debug, Deserialize)]
struct SourceEntryXml {
    #[serde(rename = "$text", default)]
    file_name: String,
    #[serde(flatten)]
    attributes: BTreeMap<String, String>,
}

fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }

    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(value) => Ok(vec![value]),
        OneOrMany::Many(values) => Ok(values),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifestEntry {
    file_name: Box<str>,
    attributes: Box<[LocalizationAttribute]>,
}

impl SourceManifestEntry {
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[must_use]
    pub fn attributes(&self) -> &[LocalizationAttribute] {
        &self.attributes
    }
}

#[derive(Debug, Clone)]
pub struct LocalizationLoader<'a> {
    assets: &'a nw_asset::AssetStore,
    language: LanguageCode,
    tags: Vec<LocalizationTag>,
    keys: BTreeSet<LocalizationKey>,
}

#[derive(Debug, Clone)]
struct LocalizationSource {
    asset: nw_asset::AssetInfo,
    tag: LocalizationTag,
}

impl<'a> LocalizationLoader<'a> {
    #[must_use]
    pub fn new(assets: &'a nw_asset::AssetStore, language: LanguageCode) -> Self {
        Self {
            assets,
            language,
            tags: vec![LocalizationTag::init()],
            keys: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn tag(mut self, tag: LocalizationTag) -> Self {
        self.tags = vec![tag];
        self
    }

    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item = LocalizationTag>) -> Self {
        self.tags = tags.into_iter().collect();
        if self.tags.is_empty() {
            self.tags.push(LocalizationTag::init());
        }
        self
    }

    #[must_use]
    pub fn keys(mut self, keys: impl IntoIterator<Item = LocalizationKey>) -> Self {
        self.keys = keys.into_iter().collect();
        self
    }

    #[must_use]
    pub fn extend_keys(mut self, keys: impl IntoIterator<Item = LocalizationKey>) -> Self {
        self.keys.extend(keys);
        self
    }

    pub fn load(self) -> Result<LocalizationCatalog, LocalizationError> {
        if self.keys.is_empty() {
            return Ok(LocalizationCatalog::builder(self.language).build());
        }

        let sources = self.sources()?;
        let mut remaining = self.keys;
        let mut builder = LocalizationCatalog::builder(self.language);

        for source in sources {
            if remaining.is_empty() {
                break;
            }
            let Some(bytes) = self.assets.read(&source.asset)? else {
                continue;
            };
            let document = LocalizationDocument::parse_bytes(&bytes)?;
            if !document
                .entries()
                .iter()
                .filter_map(LocalizationEntry::key)
                .filter_map(|key| LocalizationKey::from_source_key(key).ok())
                .any(|key| remaining.contains(&key))
            {
                continue;
            }

            let loaded = builder.add_matching(
                source.asset.path().into(),
                &source.tag,
                &document,
                |key| remaining.contains(key),
            )?;
            for key in loaded {
                remaining.remove(&key);
            }
        }

        Ok(builder.build())
    }

    fn sources(&self) -> Result<Vec<LocalizationSource>, LocalizationError> {
        if let Some(bytes) = self.assets.read_path(TAG_MANIFEST_ASSET_PATH)? {
            let manifest = SourceManifest::parse_bytes(&bytes)?;
            return self.manifest_sources(&manifest);
        }

        let tag = self
            .tags
            .first()
            .cloned()
            .unwrap_or_else(LocalizationTag::init);
        let prefix = format!("localization/{}/", self.language.asset_folder());
        let mut sources = BTreeMap::<String, LocalizationTag>::new();
        for path in self.assets.catalog_paths() {
            let normalized = nw_asset::normalize_virtual_path(path);
            if normalized.starts_with(&prefix) && is_localization_source_name(&normalized) {
                sources.entry(normalized).or_insert_with(|| tag.clone());
            }
        }
        Ok(sources
            .into_iter()
            .map(|(path, tag)| LocalizationSource {
                asset: self.assets.info(&path),
                tag,
            })
            .collect())
    }

    fn manifest_sources(
        &self,
        manifest: &SourceManifest,
    ) -> Result<Vec<LocalizationSource>, LocalizationError> {
        let mut sources = BTreeMap::<String, LocalizationTag>::new();
        for tag in &self.tags {
            let Some(entries) = manifest.tag(tag) else {
                return Err(LocalizationError::UnknownTag {
                    tag: tag.as_str().into(),
                    manifest: PathBuf::from(TAG_MANIFEST_ASSET_PATH),
                });
            };

            for entry in entries {
                if entry.file_name().is_empty() {
                    continue;
                }
                let asset = self
                    .assets
                    .info(&localization_asset_path(&self.language, entry.file_name()));
                sources
                    .entry(asset.path().to_string())
                    .or_insert_with(|| tag.clone());
            }
        }
        Ok(sources
            .into_iter()
            .map(|(path, tag)| LocalizationSource {
                asset: self.assets.info(&path),
                tag,
            })
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationCatalog {
    language: LanguageCode,
    entries_by_crc: HashMap<u32, LocalizedText>,
    source_files: Box<[LocalizationSourceFile]>,
    report: LocalizationLoadReport,
}

impl LocalizationCatalog {
    #[must_use]
    pub fn builder(language: LanguageCode) -> LocalizationCatalogBuilder {
        LocalizationCatalogBuilder::new(language)
    }

    #[must_use]
    pub fn language(&self) -> &LanguageCode {
        &self.language
    }

    #[must_use]
    pub fn localized_text_by_crc32(&self, key: u32) -> Option<&LocalizedText> {
        self.entries_by_crc.get(&key)
    }

    #[must_use]
    pub fn source_files(&self) -> &[LocalizationSourceFile] {
        &self.source_files
    }

    #[must_use]
    pub const fn report(&self) -> &LocalizationLoadReport {
        &self.report
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries_by_crc.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries_by_crc.is_empty()
    }
}

impl LocalizedTextResolver for LocalizationCatalog {
    fn localized_text(&self, key: &LocalizationKey) -> Option<&LocalizedText> {
        self.localized_text_by_crc32(key.crc32())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationCatalogBuilder {
    language: LanguageCode,
    entries_by_crc: HashMap<u32, LocalizedText>,
    source_files: Vec<LocalizationSourceFile>,
    report: LocalizationLoadReport,
}

impl LocalizationCatalogBuilder {
    #[must_use]
    pub fn new(language: LanguageCode) -> Self {
        Self {
            language,
            entries_by_crc: HashMap::new(),
            source_files: Vec::new(),
            report: LocalizationLoadReport::default(),
        }
    }

    #[must_use]
    pub fn language(&self) -> &LanguageCode {
        &self.language
    }

    pub fn add_document_bytes(
        &mut self,
        source_path: impl Into<Box<str>>,
        tag: &LocalizationTag,
        bytes: &[u8],
    ) -> Result<Vec<LocalizationKey>, LocalizationError> {
        let source_path = source_path.into();
        let document = LocalizationDocument::parse_bytes(bytes).map_err(|source| {
            LocalizationError::ParseSource {
                path: PathBuf::from(source_path.as_ref()),
                source: Box::new(source),
            }
        })?;
        self.add_document(source_path, tag, &document)
    }

    pub fn add_document(
        &mut self,
        source_path: Box<str>,
        tag: &LocalizationTag,
        document: &LocalizationDocument,
    ) -> Result<Vec<LocalizationKey>, LocalizationError> {
        self.add_matching(source_path, tag, document, |_| true)
    }

    pub fn add_matching(
        &mut self,
        source_path: Box<str>,
        tag: &LocalizationTag,
        document: &LocalizationDocument,
        mut keep: impl FnMut(&LocalizationKey) -> bool,
    ) -> Result<Vec<LocalizationKey>, LocalizationError> {
        let mut loaded_entries = 0usize;
        let mut skipped_empty = 0usize;
        let mut skipped_nil = 0usize;
        let mut loaded_keys = Vec::new();

        for entry in document.entries() {
            let LocalizationEntry::String(entry) = entry else {
                skipped_nil += 1;
                continue;
            };
            if entry.value().is_empty() {
                skipped_empty += 1;
                continue;
            }

            let key = LocalizationKey::from_source_key(entry.key()).map_err(|source| {
                LocalizationError::InvalidSourceKey {
                    path: PathBuf::from(source_path.as_ref()),
                    key: entry.key().into(),
                    source,
                }
            })?;
            if !keep(&key) {
                continue;
            }
            let localized_text = LocalizedText {
                key,
                source_key: entry.key().into(),
                text: normalized_text(entry.value()).into_owned().into_boxed_str(),
                tag: tag.as_str().into(),
                source_path: source_path.clone(),
                attributes: entry.attributes().into(),
            };

            match self.entries_by_crc.entry(localized_text.key.crc32()) {
                Entry::Vacant(slot) => {
                    loaded_keys.push(localized_text.key.clone());
                    slot.insert(localized_text);
                    loaded_entries += 1;
                }
                Entry::Occupied(existing) => {
                    self.report.duplicates.push(LocalizationDuplicate {
                        key: localized_text.key,
                        source_key: localized_text.source_key,
                        existing_source_path: existing.get().source_path.clone(),
                        duplicate_source_path: source_path.clone(),
                    });
                }
            }
        }

        self.report.source_files += 1;
        self.report.entries += loaded_entries;
        self.report.skipped_empty += skipped_empty;
        self.report.skipped_nil += skipped_nil;
        self.source_files.push(LocalizationSourceFile {
            tag: tag.as_str().into(),
            path: source_path,
            entries: loaded_entries,
            skipped_empty,
            skipped_nil,
        });
        Ok(loaded_keys)
    }

    #[must_use]
    pub fn build(self) -> LocalizationCatalog {
        LocalizationCatalog {
            language: self.language,
            entries_by_crc: self.entries_by_crc,
            source_files: self.source_files.into_boxed_slice(),
            report: self.report,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LocalizationLoadReport {
    source_files: usize,
    entries: usize,
    skipped_empty: usize,
    skipped_nil: usize,
    duplicates: Vec<LocalizationDuplicate>,
}

impl LocalizationLoadReport {
    #[must_use]
    pub const fn source_files(&self) -> usize {
        self.source_files
    }

    #[must_use]
    pub const fn entries(&self) -> usize {
        self.entries
    }

    #[must_use]
    pub const fn skipped_empty(&self) -> usize {
        self.skipped_empty
    }

    #[must_use]
    pub const fn skipped_nil(&self) -> usize {
        self.skipped_nil
    }

    #[must_use]
    pub fn duplicates(&self) -> &[LocalizationDuplicate] {
        &self.duplicates
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationDuplicate {
    key: LocalizationKey,
    source_key: Box<str>,
    existing_source_path: Box<str>,
    duplicate_source_path: Box<str>,
}

impl LocalizationDuplicate {
    #[must_use]
    pub fn key(&self) -> &LocalizationKey {
        &self.key
    }

    #[must_use]
    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    #[must_use]
    pub fn existing_source_path(&self) -> &str {
        &self.existing_source_path
    }

    #[must_use]
    pub fn duplicate_source_path(&self) -> &str {
        &self.duplicate_source_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationSourceFile {
    tag: Box<str>,
    path: Box<str>,
    entries: usize,
    skipped_empty: usize,
    skipped_nil: usize,
}

impl LocalizationSourceFile {
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn entries(&self) -> usize {
        self.entries
    }

    #[must_use]
    pub const fn skipped_empty(&self) -> usize {
        self.skipped_empty
    }

    #[must_use]
    pub const fn skipped_nil(&self) -> usize {
        self.skipped_nil
    }
}

#[must_use]
pub fn localization_asset_path(language: &LanguageCode, file_name: &str) -> String {
    let file_name = file_name.trim().replace('\\', "/").to_ascii_lowercase();
    format!("localization/{}/{file_name}", language.asset_folder())
}

#[must_use]
pub fn is_localization_source_name(name: &str) -> bool {
    let normalized = name.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with(".loc.xml") || normalized.ends_with(".loc")
}

#[must_use]
pub fn is_localization_source_path(path: &Path) -> bool {
    path.to_str().is_some_and(is_localization_source_name)
}

pub fn localization_keys(text: &str) -> impl Iterator<Item = LocalizationKey> + '_ {
    LocalizationKeys { text, search: 0 }
}

struct LocalizationKeys<'a> {
    text: &'a str,
    search: usize,
}

impl Iterator for LocalizationKeys<'_> {
    type Item = LocalizationKey;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(relative_start) = self.text[self.search..].find('@') {
            let start = self.search + relative_start;
            let end = label_end(self.text, start);
            self.search = end.max(start + 1);

            if end <= start + 1 {
                continue;
            }
            if let Ok(key) = LocalizationKey::from_label(&self.text[start..end]) {
                return Some(key);
            }
        }
        None
    }
}

fn localize_labels<'a>(
    resolver: &(impl LocalizedTextResolver + ?Sized),
    text: &'a str,
) -> Cow<'a, str> {
    let mut output: Option<String> = None;
    let mut search = 0usize;
    let mut last = 0usize;

    while let Some(relative_start) = text[search..].find('@') {
        let start = search + relative_start;
        let end = label_end(text, start);

        if end > start + 1 {
            let label = &text[start..end];
            if let Ok(key) = LocalizationKey::from_label(label)
                && let Some(replacement) = resolver.text(&key)
            {
                let output = output.get_or_insert_with(|| String::with_capacity(text.len()));
                output.push_str(&text[last..start]);
                output.push_str(replacement);
                last = end;
            }
        }

        search = end.max(start + 1);
    }

    match output {
        Some(mut output) => {
            output.push_str(&text[last..]);
            Cow::Owned(output)
        }
        None => Cow::Borrowed(text),
    }
}

fn label_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut end = start + 1;
    while end < bytes.len() && !is_label_terminator(bytes[end]) {
        end += 1;
    }
    end
}

fn is_label_terminator(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'!'
            | b'"'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b'.'
            | b'/'
            | b':'
            | b';'
            | b'<'
            | b'='
            | b'>'
            | b'?'
            | b'['
            | b'\\'
            | b']'
            | b'^'
            | b'`'
            | b'{'
            | b'|'
            | b'}'
            | b'~'
            | b'\n'
            | b'\t'
            | b'\r'
    )
}

fn normalized_text(value: &str) -> Cow<'_, str> {
    if value.contains("\\n") {
        Cow::Owned(value.replace("\\n", " \n"))
    } else {
        Cow::Borrowed(value)
    }
}

fn attributes_from_map(values: BTreeMap<String, String>) -> Box<[LocalizationAttribute]> {
    values
        .into_iter()
        .filter_map(|(name, value)| {
            let name = clean_attribute_name(&name)?;
            Some(LocalizationAttribute::new(name, value))
        })
        .collect()
}

fn clean_attribute_name(name: &str) -> Option<String> {
    if name == "$text" || name == "$value" {
        return None;
    }
    let name = name.strip_prefix('@').unwrap_or(name);
    if name == "xmlns" || name.starts_with("xmlns:") {
        return None;
    }
    Some(name.to_string())
}

fn has_nil_attr(attributes: &[LocalizationAttribute]) -> bool {
    attributes.iter().any(|attribute| {
        matches!(attribute.name(), "xsi:nil" | "nil")
            && attribute.value().eq_ignore_ascii_case("true")
    })
}

fn is_language_code(value: &str) -> bool {
    let Some((language, region)) = value.split_once('-') else {
        return false;
    };
    matches!(language.len(), 2 | 3)
        && matches!(region.len(), 2 | 3)
        && language.bytes().all(|byte| byte.is_ascii_alphabetic())
        && region.bytes().all(|byte| byte.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_entries_and_attributes() {
        let document = LocalizationDocument::parse_str(
            r#"<resources><string key="Quest_1" speaker="Grace">Hello</string></resources>"#,
        )
        .expect("parse localization document");

        let entry = document.entries()[0].as_string().expect("string");
        assert_eq!(entry.key(), "Quest_1");
        assert_eq!(entry.value(), "Hello");
        assert_eq!(entry.attributes()[0].name(), "speaker");
    }

    #[test]
    fn parses_nil_entries() {
        let document = LocalizationDocument::parse_str(
            r#"<resources xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><string xsi:nil="true" /></resources>"#,
        )
        .expect("parse localization document");

        assert!(document.entries()[0].is_nil());
    }

    #[test]
    fn catalog_resolves_labels() {
        let mut builder =
            LocalizationCatalog::builder(LanguageCode::new("en-US").expect("language"));
        let tag = LocalizationTag::init();
        builder
            .add_document_bytes(
                "localization/en-us/main.loc.xml",
                &tag,
                br#"<resources><string key="Name">Grace</string></resources>"#,
            )
            .expect("add document");
        let catalog = builder.build();

        assert_eq!(catalog.label_text("@Name").expect("label"), Some("Grace"));
        assert_eq!(catalog.localize_text("Hello @Name!"), "Hello Grace!");
    }

    #[test]
    fn extracts_label_keys_from_text() {
        let keys = localization_keys("Hello @Name, meet @Other!")
            .map(|key| key.as_str().to_string())
            .collect::<Vec<_>>();

        assert_eq!(keys, ["name", "other"]);
    }

    #[test]
    fn parses_source_manifest_tag_entries() {
        let manifest = SourceManifest::parse_str(
            r#"<localization version="0"><init><entry rel_version="Launch">Main.loc.xml</entry></init></localization>"#,
        )
        .expect("manifest");
        let init = manifest
            .tag(&LocalizationTag::init())
            .expect("init tag entries");

        assert_eq!(init.len(), 1);
        assert_eq!(init[0].file_name(), "Main.loc.xml");
        assert_eq!(init[0].attributes()[0].name(), "rel_version");
    }

    #[test]
    fn parses_language_manifest() {
        let manifest = LanguageManifest::parse_str(
            r#"<localization version="0"><languages><language sounds="true">en-US</language></languages></localization>"#,
        )
        .expect("language manifest");

        assert_eq!(manifest.languages()[0].code().as_str(), "en-US");
        assert!(manifest.languages()[0].has_sounds());
    }
}
