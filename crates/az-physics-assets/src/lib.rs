//! Physics asset parsing.

use nw_objectstream::value::ObjectStreamValueError;
use nw_objectstream::visit::{ElementHeader, ElementVisitor, VisitFlow, parse_streaming_bytes};
use nw_objectstream::{ObjectStreamError, types};
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize)]
pub struct LinearRgba {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl LinearRgba {
    #[must_use]
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}
use quick_xml::{
    Reader, XmlVersion,
    escape::{EscapeError, resolve_predefined_entity},
    events::{BytesCData, BytesRef, BytesStart, BytesText, Event},
};
use std::{
    borrow::Cow,
    fmt, io,
    path::{Path, PathBuf},
    str,
};
use thiserror::Error;
use uuid::{Uuid, uuid};

pub const COLLISION_FILTERS_ASSET_TYPE_ID: Uuid = uuid!("3f5634a1-8683-4783-8acb-07478cb686fe");
pub const EDITABLE_COLLISION_FILTER_TYPE_ID: Uuid = uuid!("0f8a4615-8824-4e01-ba47-a5cbf14227ca");
pub const COLLISION_FILTER_COLOR_TYPE_ID: Uuid = uuid!("d6f2c792-d886-4600-b81c-548df895a5e6");

pub const STRING_VECTOR_TYPE_ID: Uuid = uuid!("99dad0bc-740e-5e82-826b-8fc7968cc02c");
pub const COLLISION_FILTER_VECTOR_TYPE_ID: Uuid = uuid!("ed8c17d0-9a78-5ca6-a371-2058b4a7986d");
pub const COLLISION_FILTER_TAG_VECTOR_TYPE_ID: Uuid = uuid!("661c1835-82e9-519e-852c-4586c3435b17");
pub const COLLISION_FILTER_COLOR_VECTOR_TYPE_ID: Uuid =
    uuid!("ffa1e556-f423-5a4d-ae2c-8f30d28be5fd");
pub const MATERIAL_SET_ASSET_TYPE_ID: Uuid = uuid!("9e366d8c-33bb-4825-9a1f-fa3adbe11d0f");
pub const MATERIAL_SET_TYPE_ID: Uuid = uuid!("84399e75-18ab-4000-8dca-07b9d4e0f8e8");
pub const MATERIAL_PROPERTIES_TYPE_ID: Uuid = uuid!("8807caa1-ad08-4238-8fdb-2154add084a1");
pub const MATERIAL_ENTRY_TYPE_ID: Uuid = uuid!("c5207cc2-ef1b-4a11-bc8f-f1898282fbe5");
pub const MATERIAL_ENTRY_LIST_TYPE_ID: Uuid = uuid!("9800688d-64a7-5c0d-9f79-e32e310bb924");

pub const CATEGORIES_FIELD_CRC: u32 = 989_021_800;
pub const FILTERS_FIELD_CRC: u32 = 2_021_091_213;
pub const CHARACTER_FILTER_COLOR_FIELD_CRC: u32 = 2_173_572_725;
pub const GHOST_FILTER_COLOR_FIELD_CRC: u32 = 1_732_364_474;
pub const SLEEPING_BODY_COLOR_FIELD_CRC: u32 = 2_936_879_165;
pub const CUSTOM_FILTER_COLORS_FIELD_CRC: u32 = 162_978_760;

pub const NAME_FIELD_CRC: u32 = 1_579_384_326;
pub const DESCRIPTION_FIELD_CRC: u32 = 1_843_675_174;
pub const INHERITS_FILTERS_FIELD_CRC: u32 = 2_716_852_536;
pub const IS_CATEGORIES_FIELD_CRC: u32 = 1_888_979_482;
pub const COLLIDE_WITH_CATEGORIES_FIELD_CRC: u32 = 3_869_464_348;
pub const FILTER_TAGS_FIELD_CRC: u32 = 3_305_311_246;
pub const COLOR_FIELD_CRC: u32 = 1_716_930_793;
pub const DEFAULT_MATERIAL_FIELD_CRC: u32 = 4_201_891_245;
pub const MATERIALS_FIELD_CRC: u32 = 2_601_981_621;
pub const CONFIGURATION_FIELD_CRC: u32 = 2_783_094_231;
pub const FRICTION_FIELD_CRC: u32 = 302_782_475;
pub const RESTITUTION_FIELD_CRC: u32 = 1_336_418_461;
pub const SURFACE_TYPE_FIELD_CRC: u32 = 2_334_114_560;
pub const TRAVERSABLE_FIELD_CRC: u32 = 3_953_697_547;
pub const COLLISION_FILTERS_EXTENSION: &str = "collisionfilters";
pub const PHYSICS_MATERIAL_SET_EXTENSION: &str = "physicsmaterialset";

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct CollisionFiltersAsset {
    categories: Vec<Box<str>>,
    filters: Vec<EditableCollisionFilter>,
    character_filter_color: LinearRgba,
    ghost_filter_color: LinearRgba,
    sleeping_body_color: LinearRgba,
    custom_filter_colors: Vec<CollisionFilterColor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollisionFiltersAssetParts {
    pub categories: Vec<Box<str>>,
    pub filters: Vec<EditableCollisionFilter>,
    pub character_filter_color: LinearRgba,
    pub ghost_filter_color: LinearRgba,
    pub sleeping_body_color: LinearRgba,
    pub custom_filter_colors: Vec<CollisionFilterColor>,
}

impl CollisionFiltersAsset {
    /// Parse a `.collisionfilters` asset.
    ///
    /// # Errors
    ///
    /// Returns an error when the ObjectStream envelope, root type, or reflected
    /// fields are invalid.
    pub fn parse(bytes: &[u8]) -> Result<Self, CollisionFiltersParseError> {
        let mut visitor = CollisionFiltersVisitor::default();
        let version = parse_streaming_bytes(bytes, None, &mut visitor)?;
        if version != 3 {
            return Err(CollisionFiltersParseError::UnsupportedObjectStreamVersion { version });
        }
        visitor.finish()
    }

    #[must_use]
    pub fn categories(&self) -> &[Box<str>] {
        &self.categories
    }

    #[must_use]
    pub fn filters(&self) -> &[EditableCollisionFilter] {
        &self.filters
    }

    #[must_use]
    pub const fn character_filter_color(&self) -> LinearRgba {
        self.character_filter_color
    }

    #[must_use]
    pub const fn ghost_filter_color(&self) -> LinearRgba {
        self.ghost_filter_color
    }

    #[must_use]
    pub const fn sleeping_body_color(&self) -> LinearRgba {
        self.sleeping_body_color
    }

    #[must_use]
    pub fn custom_filter_colors(&self) -> &[CollisionFilterColor] {
        &self.custom_filter_colors
    }

    #[must_use]
    pub fn into_parts(self) -> CollisionFiltersAssetParts {
        CollisionFiltersAssetParts {
            categories: self.categories,
            filters: self.filters,
            character_filter_color: self.character_filter_color,
            ghost_filter_color: self.ghost_filter_color,
            sleeping_body_color: self.sleeping_body_color,
            custom_filter_colors: self.custom_filter_colors,
        }
    }

    #[must_use]
    pub fn summary(&self) -> CollisionFiltersSummary {
        CollisionFiltersSummary::from_asset(self)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CollisionFiltersSummary {
    pub categories: usize,
    pub filters: usize,
    pub custom_colors: usize,
    pub filter_tag_bytes: usize,
}

impl CollisionFiltersSummary {
    #[must_use]
    pub fn from_asset(asset: &CollisionFiltersAsset) -> Self {
        Self {
            categories: asset.categories().len(),
            filters: asset.filters().len(),
            custom_colors: asset.custom_filter_colors().len(),
            filter_tag_bytes: asset
                .filters()
                .iter()
                .map(|filter| filter.filter_tags.len())
                .sum(),
        }
    }
}

impl fmt::Display for CollisionFiltersSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} categories, {} filters, {} custom colors",
            self.categories, self.filters, self.custom_colors
        )
    }
}

pub fn summarize_collision_filters_asset(
    bytes: &[u8],
) -> Result<CollisionFiltersSummary, CollisionFiltersParseError> {
    CollisionFiltersAsset::parse(bytes).map(|asset| asset.summary())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CollisionFiltersTotals {
    pub files: usize,
    pub categories: usize,
    pub filters: usize,
    pub custom_colors: usize,
    pub filter_tag_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionFiltersFileSummary {
    pub source: String,
    pub summary: CollisionFiltersSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CollisionFiltersInspection {
    pub rows: Vec<CollisionFiltersFileSummary>,
    pub totals: CollisionFiltersTotals,
}

#[derive(Debug, Clone, Copy)]
pub struct CollisionFiltersInspectionReport<'a> {
    inspection: &'a CollisionFiltersInspection,
    limit: usize,
}

impl CollisionFiltersTotals {
    pub fn add_summary(&mut self, summary: CollisionFiltersSummary) {
        self.files += 1;
        self.categories += summary.categories;
        self.filters += summary.filters;
        self.custom_colors += summary.custom_colors;
        self.filter_tag_bytes += summary.filter_tag_bytes;
    }
}

impl CollisionFiltersInspection {
    pub fn add_file_summary(&mut self, row: CollisionFiltersFileSummary) {
        self.totals.add_summary(row.summary);
        self.rows.push(row);
    }

    #[must_use]
    pub const fn report(&self, limit: usize) -> CollisionFiltersInspectionReport<'_> {
        CollisionFiltersInspectionReport {
            inspection: self,
            limit,
        }
    }
}

impl fmt::Display for CollisionFiltersTotals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  files: {}", self.files)?;
        writeln!(f, "  categories: {}", self.categories)?;
        writeln!(f, "  filters: {}", self.filters)?;
        writeln!(f, "  custom colors: {}", self.custom_colors)?;
        writeln!(f, "  filter tag bytes: {}", self.filter_tag_bytes)
    }
}

impl fmt::Display for CollisionFiltersInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.limit > 0 {
            for row in self.inspection.rows.iter().take(self.limit) {
                writeln!(f, "{}: {}", row.source, row.summary)?;
            }

            if self.inspection.rows.len() > self.limit {
                writeln!(
                    f,
                    "... {} more files",
                    self.inspection.rows.len() - self.limit
                )?;
            }
        }

        write!(f, "{}", self.inspection.totals)
    }
}

pub fn inspect_collision_filters_file(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<CollisionFiltersFileSummary, CollisionFiltersParseError> {
    Ok(CollisionFiltersFileSummary {
        source: path.as_ref().display().to_string(),
        summary: summarize_collision_filters_asset(bytes)?,
    })
}

#[derive(Debug, Error)]
pub enum CollisionFiltersInspectionError {
    #[error("read collision filters asset {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("parse collision filters asset {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: CollisionFiltersParseError,
    },
}

pub fn inspect_collision_filters_path(
    path: impl AsRef<Path>,
) -> Result<CollisionFiltersFileSummary, CollisionFiltersInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| CollisionFiltersInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_collision_filters_file(path, &bytes).map_err(|source| {
        CollisionFiltersInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

pub fn inspect_collision_filters_files(
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
) -> Result<CollisionFiltersInspection, CollisionFiltersInspectionError> {
    let mut inspection = CollisionFiltersInspection::default();
    for path in paths {
        inspection.add_file_summary(inspect_collision_filters_path(path)?);
    }
    Ok(inspection)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditableCollisionFilter {
    pub name: Box<str>,
    pub description: Box<str>,
    pub inherits_filters: Vec<Box<str>>,
    pub is_categories: Vec<Box<str>>,
    pub collide_with_categories: Vec<Box<str>>,
    pub filter_tags: Vec<u8>,
}

impl EditableCollisionFilter {
    #[must_use]
    pub const fn new(
        name: Box<str>,
        description: Box<str>,
        inherits_filters: Vec<Box<str>>,
        is_categories: Vec<Box<str>>,
        collide_with_categories: Vec<Box<str>>,
        filter_tags: Vec<u8>,
    ) -> Self {
        Self {
            name,
            description,
            inherits_filters,
            is_categories,
            collide_with_categories,
            filter_tags,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollisionFilterColor {
    pub name: Box<str>,
    pub color: LinearRgba,
}

impl CollisionFilterColor {
    #[must_use]
    pub const fn new(name: Box<str>, color: LinearRgba) -> Self {
        Self { name, color }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialSetAsset {
    material_set: MaterialSet,
}

impl MaterialSetAsset {
    /// Parse a `.physicsmaterialset` asset.
    ///
    /// # Errors
    ///
    /// Returns an error when the ObjectStream XML envelope, reflected type
    /// layout, or material fields are invalid.
    pub fn parse(bytes: &[u8]) -> Result<Self, MaterialSetParseError> {
        let xml = str::from_utf8(bytes)?;
        Self::parse_str(xml)
    }

    pub fn parse_str(xml: &str) -> Result<Self, MaterialSetParseError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut parser = MaterialSetXmlParser::default();
        loop {
            match reader.read_event()? {
                Event::Start(event) if event.name().as_ref() == b"ObjectStream" => {
                    parser.start_object_stream(&reader, &event)?;
                }
                Event::End(event) if event.name().as_ref() == b"ObjectStream" => {
                    parser.end_object_stream()?;
                }
                Event::Start(event) if event.name().as_ref() == b"Class" => {
                    parser.start_class(&reader, &event)?;
                }
                Event::Empty(event) if event.name().as_ref() == b"Class" => {
                    parser.empty_class(&reader, &event)?;
                }
                Event::End(event) if event.name().as_ref() == b"Class" => {
                    parser.end_class()?;
                }
                Event::Eof => break,
                Event::Text(event) => {
                    if !xml_text_content(&event)?.trim().is_empty() {
                        return Err(MaterialSetParseError::UnexpectedText);
                    }
                }
                Event::CData(event) => {
                    if !xml_cdata_content(&event)?.trim().is_empty() {
                        return Err(MaterialSetParseError::UnexpectedText);
                    }
                }
                Event::GeneralRef(event)
                    if !xml_general_reference_content(&event)?.trim().is_empty() =>
                {
                    return Err(MaterialSetParseError::UnexpectedText);
                }
                Event::GeneralRef(_) => {}
                _ => {}
            }
        }

        parser.finish()
    }

    #[must_use]
    pub const fn material_set(&self) -> &MaterialSet {
        &self.material_set
    }

    #[must_use]
    pub fn into_material_set(self) -> MaterialSet {
        self.material_set
    }

    #[must_use]
    pub fn summary(&self) -> MaterialSetSummary {
        MaterialSetSummary::from_asset(self)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MaterialSetSummary {
    pub material_entries: usize,
    pub materials: usize,
    pub traversable_materials: usize,
}

impl MaterialSetSummary {
    #[must_use]
    pub fn from_asset(asset: &MaterialSetAsset) -> Self {
        let material_set = asset.material_set();
        let traversable_materials = material_set
            .materials
            .iter()
            .filter(|entry| entry.configuration.traversable)
            .count()
            + usize::from(material_set.default_material.traversable);

        Self {
            material_entries: material_set.materials.len(),
            materials: 1 + material_set.materials.len(),
            traversable_materials,
        }
    }
}

impl fmt::Display for MaterialSetSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} materials", self.material_entries)
    }
}

pub fn summarize_material_set_asset(
    bytes: &[u8],
) -> Result<MaterialSetSummary, MaterialSetParseError> {
    MaterialSetAsset::parse(bytes).map(|asset| asset.summary())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MaterialSetTotals {
    pub files: usize,
    pub materials: usize,
    pub traversable_materials: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialSetFileSummary {
    pub source: String,
    pub summary: MaterialSetSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MaterialSetInspection {
    pub rows: Vec<MaterialSetFileSummary>,
    pub totals: MaterialSetTotals,
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialSetInspectionReport<'a> {
    inspection: &'a MaterialSetInspection,
    limit: usize,
}

impl MaterialSetTotals {
    pub fn add_summary(&mut self, summary: MaterialSetSummary) {
        self.files += 1;
        self.materials += summary.materials;
        self.traversable_materials += summary.traversable_materials;
    }
}

impl MaterialSetInspection {
    pub fn add_file_summary(&mut self, row: MaterialSetFileSummary) {
        self.totals.add_summary(row.summary);
        self.rows.push(row);
    }

    #[must_use]
    pub const fn report(&self, limit: usize) -> MaterialSetInspectionReport<'_> {
        MaterialSetInspectionReport {
            inspection: self,
            limit,
        }
    }
}

impl fmt::Display for MaterialSetTotals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  files: {}", self.files)?;
        writeln!(f, "  materials: {}", self.materials)?;
        writeln!(f, "  traversable materials: {}", self.traversable_materials)
    }
}

impl fmt::Display for MaterialSetInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.limit > 0 {
            for row in self.inspection.rows.iter().take(self.limit) {
                writeln!(f, "{}: {}", row.source, row.summary)?;
            }

            if self.inspection.rows.len() > self.limit {
                writeln!(
                    f,
                    "... {} more files",
                    self.inspection.rows.len() - self.limit
                )?;
            }
        }

        write!(f, "{}", self.inspection.totals)
    }
}

pub fn inspect_material_set_file(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<MaterialSetFileSummary, MaterialSetParseError> {
    Ok(MaterialSetFileSummary {
        source: path.as_ref().display().to_string(),
        summary: summarize_material_set_asset(bytes)?,
    })
}

#[derive(Debug, Error)]
pub enum MaterialSetInspectionError {
    #[error("read material set asset {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("parse material set asset {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: MaterialSetParseError,
    },
}

pub fn inspect_material_set_path(
    path: impl AsRef<Path>,
) -> Result<MaterialSetFileSummary, MaterialSetInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| MaterialSetInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_material_set_file(path, &bytes).map_err(|source| MaterialSetInspectionError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub fn inspect_material_set_files(
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
) -> Result<MaterialSetInspection, MaterialSetInspectionError> {
    let mut inspection = MaterialSetInspection::default();
    for path in paths {
        inspection.add_file_summary(inspect_material_set_path(path)?);
    }
    Ok(inspection)
}

#[must_use]
pub fn is_collision_filters_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(COLLISION_FILTERS_EXTENSION)
}

#[must_use]
pub fn is_collision_filters_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| is_collision_filters_extension(extension))
}

#[must_use]
pub fn is_collision_filters_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_collision_filters_extension)
}

#[must_use]
pub fn is_physics_material_set_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(PHYSICS_MATERIAL_SET_EXTENSION)
}

#[must_use]
pub fn is_physics_material_set_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| is_physics_material_set_extension(extension))
}

#[must_use]
pub fn is_physics_material_set_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_physics_material_set_extension)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialSet {
    pub default_material: MaterialProperties,
    pub materials: Vec<MaterialEntry>,
}

impl MaterialSet {
    #[must_use]
    pub const fn new(default_material: MaterialProperties, materials: Vec<MaterialEntry>) -> Self {
        Self {
            default_material,
            materials,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialEntry {
    pub configuration: MaterialProperties,
}

impl MaterialEntry {
    #[must_use]
    pub const fn new(configuration: MaterialProperties) -> Self {
        Self { configuration }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialProperties {
    pub name: Box<str>,
    pub friction: f32,
    pub restitution: f32,
    pub traversable: bool,
    pub surface_type: Box<str>,
}

impl MaterialProperties {
    #[must_use]
    pub const fn new(
        name: Box<str>,
        friction: f32,
        restitution: f32,
        traversable: bool,
        surface_type: Box<str>,
    ) -> Self {
        Self {
            name,
            friction,
            restitution,
            traversable,
            surface_type,
        }
    }
}

#[derive(Debug, Error)]
pub enum CollisionFiltersParseError {
    #[error("objectstream parse error")]
    ObjectStream(#[from] ObjectStreamError),

    #[error("objectstream value error")]
    Value(#[from] ObjectStreamValueError),

    #[error("unsupported ObjectStream version {version}")]
    UnsupportedObjectStreamVersion { version: u32 },

    #[error("missing CollisionFiltersAsset root")]
    MissingRoot,

    #[error("unexpected root type {type_id}")]
    UnexpectedRootType { type_id: Uuid },

    #[error("multiple CollisionFiltersAsset roots")]
    MultipleRoots,

    #[error("missing field {field}")]
    MissingRootField { field: &'static str },

    #[error("duplicate field {field}")]
    DuplicateRootField { field: &'static str },

    #[error("unexpected field type {type_id} for name CRC {name_crc:?}")]
    UnexpectedFieldType {
        type_id: Uuid,
        name_crc: Option<u32>,
    },

    #[error("unexpected EditableCollisionFilter type {type_id}")]
    UnexpectedFilterType { type_id: Uuid },

    #[error("unexpected CollisionFilterColor type {type_id}")]
    UnexpectedFilterColorType { type_id: Uuid },

    #[error("filter {index} is missing {field}")]
    MissingFilterField { index: usize, field: &'static str },

    #[error("filter {index} has duplicate {field}")]
    DuplicateFilterField { index: usize, field: &'static str },

    #[error("custom filter color {index} is missing {field}")]
    MissingColorField { index: usize, field: &'static str },

    #[error("custom filter color {index} has duplicate {field}")]
    DuplicateColorField { index: usize, field: &'static str },

    #[error("nested string vector")]
    NestedStringVector,

    #[error("string value without an active vector")]
    UnexpectedStringValue,

    #[error("nested filter tag vector")]
    NestedFilterTagVector,

    #[error("filter tag value without an active vector")]
    UnexpectedFilterTagValue,

    #[error("nested EditableCollisionFilter element")]
    NestedFilter,

    #[error("nested CollisionFilterColor element")]
    NestedFilterColor,
}

#[derive(Debug, Error)]
pub enum MaterialSetParseError {
    #[error("asset is not UTF-8 XML")]
    Utf8(#[from] str::Utf8Error),

    #[error("read ObjectStream XML")]
    Xml(#[from] quick_xml::Error),

    #[error("read ObjectStream XML attribute")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),

    #[error("ObjectStream XML text is not expected in physics material sets")]
    UnexpectedText,

    #[error("missing XML attribute {name}")]
    MissingAttribute { name: &'static str },

    #[error("invalid UUID in attribute {name}: {value}")]
    InvalidUuid {
        name: &'static str,
        value: String,
        #[source]
        source: uuid::Error,
    },

    #[error("invalid {name} value {value}")]
    InvalidNumber {
        name: &'static str,
        value: String,
        #[source]
        source: std::num::ParseFloatError,
    },

    #[error("invalid bool in {name}: {value}")]
    InvalidBool { name: &'static str, value: String },

    #[error("unsupported ObjectStream version {version}")]
    UnsupportedObjectStreamVersion { version: String },

    #[error("missing ObjectStream root")]
    MissingObjectStream,

    #[error("nested ObjectStream root")]
    NestedObjectStream,

    #[error("ObjectStream XML ended before closing {element}")]
    UnclosedElement { element: &'static str },

    #[error("unexpected class {name} field {field:?} type {type_id}")]
    UnexpectedClass {
        name: String,
        field: Option<String>,
        type_id: Uuid,
    },

    #[error("class {name} has type {found}, expected {expected}")]
    UnexpectedType {
        name: &'static str,
        expected: Uuid,
        found: Uuid,
    },

    #[error("class {name} has field {found:?}, expected {expected:?}")]
    UnexpectedField {
        name: &'static str,
        expected: Option<&'static str>,
        found: Option<String>,
    },

    #[error("missing MaterialSetAsset")]
    MissingAsset,

    #[error("multiple MaterialSetAsset roots")]
    MultipleAssets,

    #[error("{owner} is missing {field}")]
    MissingField {
        owner: &'static str,
        field: &'static str,
    },

    #[error("{owner} has duplicate {field}")]
    DuplicateField {
        owner: &'static str,
        field: &'static str,
    },

    #[error("end class without a matching start class")]
    EndWithoutClass,
}

#[derive(Debug, Default)]
struct MaterialSetXmlParser {
    stack: Vec<MaterialScope>,
    saw_object_stream: bool,
    saw_asset: bool,
    asset: Option<MaterialSetAsset>,
    current_set: Option<PartialMaterialSet>,
    current_entry: Option<PartialMaterialEntry>,
    current_properties: Option<PartialMaterialProperties>,
}

impl MaterialSetXmlParser {
    fn finish(self) -> Result<MaterialSetAsset, MaterialSetParseError> {
        if !self.saw_object_stream {
            return Err(MaterialSetParseError::MissingObjectStream);
        }
        if let Some(scope) = self.stack.last() {
            return Err(MaterialSetParseError::UnclosedElement {
                element: scope.name(),
            });
        }
        self.asset.ok_or(MaterialSetParseError::MissingAsset)
    }

    fn start_object_stream(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<(), MaterialSetParseError> {
        if self.saw_object_stream || !self.stack.is_empty() {
            return Err(MaterialSetParseError::NestedObjectStream);
        }
        let version = required_attr_value(reader, event, b"version", "version")?;
        if version.as_ref() != "3" {
            return Err(MaterialSetParseError::UnsupportedObjectStreamVersion {
                version: version.into_owned(),
            });
        }
        self.saw_object_stream = true;
        self.stack.push(MaterialScope::ObjectStream);
        Ok(())
    }

    fn end_object_stream(&mut self) -> Result<(), MaterialSetParseError> {
        match self.stack.pop() {
            Some(MaterialScope::ObjectStream) => Ok(()),
            Some(scope) => Err(MaterialSetParseError::UnclosedElement {
                element: scope.name(),
            }),
            None => Err(MaterialSetParseError::MissingObjectStream),
        }
    }

    fn start_class(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<(), MaterialSetParseError> {
        match self.stack.last().copied() {
            Some(MaterialScope::ObjectStream) => {
                expect_class(
                    reader,
                    event,
                    "MaterialSetAsset",
                    None,
                    MATERIAL_SET_ASSET_TYPE_ID,
                )?;
                if self.saw_asset {
                    return Err(MaterialSetParseError::MultipleAssets);
                }
                self.saw_asset = true;
                self.stack.push(MaterialScope::Asset);
            }
            Some(MaterialScope::Asset) => {
                expect_class(
                    reader,
                    event,
                    "MaterialSet",
                    Some("BaseClass1"),
                    MATERIAL_SET_TYPE_ID,
                )?;
                if self.current_set.is_some() {
                    return Err(MaterialSetParseError::DuplicateField {
                        owner: "MaterialSetAsset",
                        field: "BaseClass1",
                    });
                }
                self.current_set = Some(PartialMaterialSet::default());
                self.stack.push(MaterialScope::MaterialSet);
            }
            Some(MaterialScope::MaterialSet) => {
                let attrs = class_attrs(reader, event)?;
                match (attrs.name.as_deref(), attrs.field.as_deref(), attrs.type_id) {
                    (Some("MaterialProperties"), Some("DefaultMaterial"), Some(type_id)) => {
                        if type_id != MATERIAL_PROPERTIES_TYPE_ID {
                            return Err(MaterialSetParseError::UnexpectedType {
                                name: "MaterialProperties",
                                expected: MATERIAL_PROPERTIES_TYPE_ID,
                                found: type_id,
                            });
                        }
                        self.start_material_properties("MaterialSet", "DefaultMaterial")?;
                        self.stack.push(MaterialScope::DefaultMaterial);
                    }
                    (Some("AZStd::list"), Some("Materials"), Some(type_id)) => {
                        if type_id != MATERIAL_ENTRY_LIST_TYPE_ID {
                            return Err(MaterialSetParseError::UnexpectedType {
                                name: "AZStd::list",
                                expected: MATERIAL_ENTRY_LIST_TYPE_ID,
                                found: type_id,
                            });
                        }
                        let current = self.current_set_mut()?;
                        if current.saw_materials {
                            return Err(MaterialSetParseError::DuplicateField {
                                owner: "MaterialSet",
                                field: "Materials",
                            });
                        }
                        current.saw_materials = true;
                        self.stack.push(MaterialScope::Materials);
                    }
                    _ => return Err(unexpected_class(attrs)),
                }
            }
            Some(MaterialScope::Materials) => {
                expect_class(
                    reader,
                    event,
                    "MaterialEntry",
                    Some("element"),
                    MATERIAL_ENTRY_TYPE_ID,
                )?;
                if self.current_entry.is_some() {
                    return Err(MaterialSetParseError::DuplicateField {
                        owner: "Materials",
                        field: "element",
                    });
                }
                self.current_entry = Some(PartialMaterialEntry::default());
                self.stack.push(MaterialScope::MaterialEntry);
            }
            Some(MaterialScope::MaterialEntry) => {
                expect_class(
                    reader,
                    event,
                    "MaterialProperties",
                    Some("Configuration"),
                    MATERIAL_PROPERTIES_TYPE_ID,
                )?;
                self.start_material_properties("MaterialEntry", "Configuration")?;
                self.stack.push(MaterialScope::EntryConfiguration);
            }
            _ => return Err(unexpected_class(class_attrs(reader, event)?)),
        }
        Ok(())
    }

    fn empty_class(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<(), MaterialSetParseError> {
        match self.stack.last().copied() {
            Some(MaterialScope::MaterialSet) => self.read_empty_materials_list(reader, event),
            Some(MaterialScope::DefaultMaterial | MaterialScope::EntryConfiguration) => {
                self.read_material_property(reader, event)
            }
            _ => Err(unexpected_class(class_attrs(reader, event)?)),
        }
    }

    fn end_class(&mut self) -> Result<(), MaterialSetParseError> {
        match self
            .stack
            .pop()
            .ok_or(MaterialSetParseError::EndWithoutClass)?
        {
            MaterialScope::ObjectStream => Err(MaterialSetParseError::EndWithoutClass),
            MaterialScope::Asset => Ok(()),
            MaterialScope::MaterialSet => {
                let material_set =
                    self.current_set
                        .take()
                        .ok_or(MaterialSetParseError::MissingField {
                            owner: "MaterialSetAsset",
                            field: "BaseClass1",
                        })?;
                self.asset = Some(MaterialSetAsset {
                    material_set: material_set.finish()?,
                });
                Ok(())
            }
            MaterialScope::DefaultMaterial => {
                let properties = self.finish_material_properties("DefaultMaterial")?;
                let current = self.current_set_mut()?;
                set_material_field(
                    &mut current.default_material,
                    "MaterialSet",
                    "DefaultMaterial",
                    properties,
                )
            }
            MaterialScope::Materials => Ok(()),
            MaterialScope::MaterialEntry => {
                let entry =
                    self.current_entry
                        .take()
                        .ok_or(MaterialSetParseError::MissingField {
                            owner: "Materials",
                            field: "element",
                        })?;
                let current = self.current_set_mut()?;
                current.materials.push(entry.finish()?);
                Ok(())
            }
            MaterialScope::EntryConfiguration => {
                let properties = self.finish_material_properties("Configuration")?;
                let current =
                    self.current_entry
                        .as_mut()
                        .ok_or(MaterialSetParseError::MissingField {
                            owner: "MaterialEntry",
                            field: "Configuration",
                        })?;
                set_material_field(
                    &mut current.configuration,
                    "MaterialEntry",
                    "Configuration",
                    properties,
                )
            }
        }
    }

    fn start_material_properties(
        &mut self,
        owner: &'static str,
        field: &'static str,
    ) -> Result<(), MaterialSetParseError> {
        if self.current_properties.is_some() {
            return Err(MaterialSetParseError::DuplicateField { owner, field });
        }
        self.current_properties = Some(PartialMaterialProperties::default());
        Ok(())
    }

    fn finish_material_properties(
        &mut self,
        field: &'static str,
    ) -> Result<MaterialProperties, MaterialSetParseError> {
        let properties =
            self.current_properties
                .take()
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialProperties",
                    field,
                })?;
        properties.finish()
    }

    fn read_material_property(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<(), MaterialSetParseError> {
        let attrs = class_attrs(reader, event)?;
        let field = match attrs.field.as_deref() {
            Some("Name") => MaterialPropertyField::Name,
            Some("Friction") => MaterialPropertyField::Friction,
            Some("Restitution") => MaterialPropertyField::Restitution,
            Some("Traversable") => MaterialPropertyField::Traversable,
            Some("SurfaceType") => MaterialPropertyField::SurfaceType,
            _ => return Err(unexpected_class(attrs)),
        };
        let type_id = attrs
            .type_id
            .ok_or(MaterialSetParseError::MissingAttribute { name: "type" })?;
        let value = attrs
            .value
            .as_deref()
            .ok_or(MaterialSetParseError::MissingAttribute { name: "value" })?;
        let current =
            self.current_properties
                .as_mut()
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialProperties",
                    field: "value",
                })?;

        match field {
            MaterialPropertyField::Name => {
                expect_type_id("Name", type_id, types::AZSTD_STRING)?;
                set_material_field(
                    &mut current.name,
                    "MaterialProperties",
                    "Name",
                    value.into(),
                )
            }
            MaterialPropertyField::Friction => {
                expect_type_id("Friction", type_id, types::FLOAT)?;
                let value = parse_f32("Friction", value)?;
                set_material_field(
                    &mut current.friction,
                    "MaterialProperties",
                    "Friction",
                    value,
                )
            }
            MaterialPropertyField::Restitution => {
                expect_type_id("Restitution", type_id, types::FLOAT)?;
                let value = parse_f32("Restitution", value)?;
                set_material_field(
                    &mut current.restitution,
                    "MaterialProperties",
                    "Restitution",
                    value,
                )
            }
            MaterialPropertyField::Traversable => {
                expect_type_id("Traversable", type_id, types::BOOL)?;
                let value = parse_bool("Traversable", value)?;
                set_material_field(
                    &mut current.traversable,
                    "MaterialProperties",
                    "Traversable",
                    value,
                )
            }
            MaterialPropertyField::SurfaceType => {
                expect_type_id("SurfaceType", type_id, types::AZSTD_STRING)?;
                set_material_field(
                    &mut current.surface_type,
                    "MaterialProperties",
                    "SurfaceType",
                    value.into(),
                )
            }
        }
    }

    fn read_empty_materials_list(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<(), MaterialSetParseError> {
        let attrs = class_attrs(reader, event)?;
        match (attrs.name.as_deref(), attrs.field.as_deref(), attrs.type_id) {
            (Some("AZStd::list"), Some("Materials"), Some(type_id)) => {
                if type_id != MATERIAL_ENTRY_LIST_TYPE_ID {
                    return Err(MaterialSetParseError::UnexpectedType {
                        name: "AZStd::list",
                        expected: MATERIAL_ENTRY_LIST_TYPE_ID,
                        found: type_id,
                    });
                }
                let current = self.current_set_mut()?;
                if current.saw_materials {
                    return Err(MaterialSetParseError::DuplicateField {
                        owner: "MaterialSet",
                        field: "Materials",
                    });
                }
                current.saw_materials = true;
                Ok(())
            }
            _ => Err(unexpected_class(attrs)),
        }
    }

    fn current_set_mut(&mut self) -> Result<&mut PartialMaterialSet, MaterialSetParseError> {
        self.current_set
            .as_mut()
            .ok_or(MaterialSetParseError::MissingField {
                owner: "MaterialSetAsset",
                field: "BaseClass1",
            })
    }
}

#[derive(Debug, Clone, Copy)]
enum MaterialPropertyField {
    Name,
    Friction,
    Restitution,
    Traversable,
    SurfaceType,
}

#[derive(Debug, Clone, Copy)]
enum MaterialScope {
    ObjectStream,
    Asset,
    MaterialSet,
    DefaultMaterial,
    Materials,
    MaterialEntry,
    EntryConfiguration,
}

impl MaterialScope {
    const fn name(self) -> &'static str {
        match self {
            Self::ObjectStream => "ObjectStream",
            Self::Asset => "MaterialSetAsset",
            Self::MaterialSet => "MaterialSet",
            Self::DefaultMaterial => "DefaultMaterial",
            Self::Materials => "Materials",
            Self::MaterialEntry => "MaterialEntry",
            Self::EntryConfiguration => "Configuration",
        }
    }
}

#[derive(Debug, Default)]
struct PartialMaterialSet {
    default_material: Option<MaterialProperties>,
    saw_materials: bool,
    materials: Vec<MaterialEntry>,
}

impl PartialMaterialSet {
    fn finish(self) -> Result<MaterialSet, MaterialSetParseError> {
        if !self.saw_materials {
            return Err(MaterialSetParseError::MissingField {
                owner: "MaterialSet",
                field: "Materials",
            });
        }
        Ok(MaterialSet::new(
            self.default_material
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialSet",
                    field: "DefaultMaterial",
                })?,
            self.materials,
        ))
    }
}

#[derive(Debug, Default)]
struct PartialMaterialEntry {
    configuration: Option<MaterialProperties>,
}

impl PartialMaterialEntry {
    fn finish(self) -> Result<MaterialEntry, MaterialSetParseError> {
        Ok(MaterialEntry::new(self.configuration.ok_or(
            MaterialSetParseError::MissingField {
                owner: "MaterialEntry",
                field: "Configuration",
            },
        )?))
    }
}

#[derive(Debug, Default)]
struct PartialMaterialProperties {
    name: Option<Box<str>>,
    friction: Option<f32>,
    restitution: Option<f32>,
    traversable: Option<bool>,
    surface_type: Option<Box<str>>,
}

impl PartialMaterialProperties {
    fn finish(self) -> Result<MaterialProperties, MaterialSetParseError> {
        Ok(MaterialProperties::new(
            self.name.ok_or(MaterialSetParseError::MissingField {
                owner: "MaterialProperties",
                field: "Name",
            })?,
            self.friction.ok_or(MaterialSetParseError::MissingField {
                owner: "MaterialProperties",
                field: "Friction",
            })?,
            self.restitution
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialProperties",
                    field: "Restitution",
                })?,
            self.traversable
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialProperties",
                    field: "Traversable",
                })?,
            self.surface_type
                .ok_or(MaterialSetParseError::MissingField {
                    owner: "MaterialProperties",
                    field: "SurfaceType",
                })?,
        ))
    }
}

#[derive(Debug, Default)]
struct ClassAttrs {
    name: Option<String>,
    field: Option<String>,
    type_id: Option<Uuid>,
    value: Option<String>,
}

fn class_attrs(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<ClassAttrs, MaterialSetParseError> {
    let mut attrs = ClassAttrs::default();
    for attribute in event.attributes() {
        let attribute = attribute?;
        match attribute.key.as_ref() {
            b"name" => attrs.name = Some(decode_attr(reader, &attribute)?.into_owned()),
            b"field" => attrs.field = Some(decode_attr(reader, &attribute)?.into_owned()),
            b"type" => {
                let value = decode_attr(reader, &attribute)?;
                let trimmed = value.trim().trim_start_matches('{').trim_end_matches('}');
                attrs.type_id = Some(Uuid::parse_str(trimmed).map_err(|source| {
                    MaterialSetParseError::InvalidUuid {
                        name: "type",
                        value: value.into_owned(),
                        source,
                    }
                })?);
            }
            b"value" => attrs.value = Some(decode_attr(reader, &attribute)?.into_owned()),
            _ => {}
        }
    }
    Ok(attrs)
}

fn expect_class(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    expected_name: &'static str,
    expected_field: Option<&'static str>,
    expected_type: Uuid,
) -> Result<(), MaterialSetParseError> {
    let attrs = class_attrs(reader, event)?;
    if attrs.name.as_deref() != Some(expected_name) {
        return Err(unexpected_class(attrs));
    }
    if attrs.field.as_deref() != expected_field {
        return Err(MaterialSetParseError::UnexpectedField {
            name: expected_name,
            expected: expected_field,
            found: attrs.field,
        });
    }
    let found = attrs
        .type_id
        .ok_or(MaterialSetParseError::MissingAttribute { name: "type" })?;
    expect_type_id(expected_name, found, expected_type)
}

fn expect_type_id(
    name: &'static str,
    found: Uuid,
    expected: Uuid,
) -> Result<(), MaterialSetParseError> {
    if found == expected {
        Ok(())
    } else {
        Err(MaterialSetParseError::UnexpectedType {
            name,
            expected,
            found,
        })
    }
}

fn unexpected_class(attrs: ClassAttrs) -> MaterialSetParseError {
    MaterialSetParseError::UnexpectedClass {
        name: attrs.name.unwrap_or_default(),
        field: attrs.field,
        type_id: attrs.type_id.unwrap_or(Uuid::nil()),
    }
}

fn required_attr_value<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
    key: &[u8],
    name: &'static str,
) -> Result<Cow<'a, str>, MaterialSetParseError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if attribute.key.as_ref() == key {
            return decode_attr(reader, &attribute);
        }
    }
    Err(MaterialSetParseError::MissingAttribute { name })
}

fn decode_attr<'a>(
    reader: &Reader<&[u8]>,
    attribute: &quick_xml::events::attributes::Attribute<'a>,
) -> Result<Cow<'a, str>, MaterialSetParseError> {
    Ok(attribute
        .decoded_and_normalized_value(quick_xml::XmlVersion::default(), reader.decoder())?)
}

fn xml_text_content<'a>(event: &BytesText<'a>) -> Result<Cow<'a, str>, quick_xml::Error> {
    event
        .xml_content(XmlVersion::default())
        .map_err(quick_xml::Error::from)
}

fn xml_cdata_content<'a>(event: &BytesCData<'a>) -> Result<Cow<'a, str>, quick_xml::Error> {
    event
        .xml_content(XmlVersion::default())
        .map_err(quick_xml::Error::from)
}

fn xml_general_reference_content(
    event: &BytesRef<'_>,
) -> Result<Cow<'static, str>, quick_xml::Error> {
    if let Some(ch) = event.resolve_char_ref()? {
        return Ok(Cow::Owned(ch.to_string()));
    }

    let reference = event.decode().map_err(quick_xml::Error::from)?;
    let Some(value) = resolve_predefined_entity(&reference) else {
        return Err(quick_xml::Error::from(EscapeError::UnrecognizedEntity(
            0..event.len(),
            reference.into_owned(),
        )));
    };
    Ok(Cow::Borrowed(value))
}

fn parse_f32(name: &'static str, value: &str) -> Result<f32, MaterialSetParseError> {
    value
        .parse()
        .map_err(|source| MaterialSetParseError::InvalidNumber {
            name,
            value: value.to_owned(),
            source,
        })
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, MaterialSetParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MaterialSetParseError::InvalidBool {
            name,
            value: value.to_owned(),
        }),
    }
}

fn set_material_field<T>(
    target: &mut Option<T>,
    owner: &'static str,
    field: &'static str,
    value: T,
) -> Result<(), MaterialSetParseError> {
    if target.replace(value).is_some() {
        return Err(MaterialSetParseError::DuplicateField { owner, field });
    }
    Ok(())
}

#[derive(Debug, Default)]
struct CollisionFiltersVisitor {
    depth: usize,
    roots: usize,
    categories_depth: Option<usize>,
    filters_depth: Option<usize>,
    filter_depth: Option<usize>,
    custom_colors_depth: Option<usize>,
    custom_color_depth: Option<usize>,
    string_list: Option<ActiveStringList>,
    tag_list_depth: Option<usize>,
    saw_categories: bool,
    saw_filters: bool,
    saw_custom_filter_colors: bool,
    current_filter: Option<PartialEditableCollisionFilter>,
    current_color: Option<PartialCollisionFilterColor>,
    categories: Vec<Box<str>>,
    filters: Vec<EditableCollisionFilter>,
    character_filter_color: Option<LinearRgba>,
    ghost_filter_color: Option<LinearRgba>,
    sleeping_body_color: Option<LinearRgba>,
    custom_filter_colors: Vec<CollisionFilterColor>,
}

impl CollisionFiltersVisitor {
    fn finish(self) -> Result<CollisionFiltersAsset, CollisionFiltersParseError> {
        if self.roots == 0 {
            return Err(CollisionFiltersParseError::MissingRoot);
        }
        if !self.saw_categories {
            return Err(CollisionFiltersParseError::MissingRootField {
                field: "Categories",
            });
        }
        if !self.saw_filters {
            return Err(CollisionFiltersParseError::MissingRootField { field: "Filters" });
        }
        if !self.saw_custom_filter_colors {
            return Err(CollisionFiltersParseError::MissingRootField {
                field: "CustomFilterColors",
            });
        }

        Ok(CollisionFiltersAsset {
            categories: self.categories,
            filters: self.filters,
            character_filter_color: self.character_filter_color.ok_or(
                CollisionFiltersParseError::MissingRootField {
                    field: "CharacterFilterColor",
                },
            )?,
            ghost_filter_color: self.ghost_filter_color.ok_or(
                CollisionFiltersParseError::MissingRootField {
                    field: "GhostFilterColor",
                },
            )?,
            sleeping_body_color: self.sleeping_body_color.ok_or(
                CollisionFiltersParseError::MissingRootField {
                    field: "SleepingBodyColor",
                },
            )?,
            custom_filter_colors: self.custom_filter_colors,
        })
    }

    fn read_root_field(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<(), CollisionFiltersParseError> {
        match header.name_crc {
            Some(CATEGORIES_FIELD_CRC) => {
                expect_type(header, STRING_VECTOR_TYPE_ID)?;
                if self.saw_categories {
                    return Err(CollisionFiltersParseError::DuplicateRootField {
                        field: "Categories",
                    });
                }
                self.saw_categories = true;
                self.start_string_list(ActiveStringListTarget::Categories, self.depth)?;
                self.categories_depth = Some(self.depth);
            }
            Some(FILTERS_FIELD_CRC) => {
                expect_type(header, COLLISION_FILTER_VECTOR_TYPE_ID)?;
                if self.saw_filters {
                    return Err(CollisionFiltersParseError::DuplicateRootField {
                        field: "Filters",
                    });
                }
                self.saw_filters = true;
                self.filters_depth = Some(self.depth);
            }
            Some(CHARACTER_FILTER_COLOR_FIELD_CRC) => {
                expect_type(header, types::COLOR)?;
                set_root_field(
                    &mut self.character_filter_color,
                    "CharacterFilterColor",
                    read_color(header)?,
                )?;
            }
            Some(GHOST_FILTER_COLOR_FIELD_CRC) => {
                expect_type(header, types::COLOR)?;
                set_root_field(
                    &mut self.ghost_filter_color,
                    "GhostFilterColor",
                    read_color(header)?,
                )?;
            }
            Some(SLEEPING_BODY_COLOR_FIELD_CRC) => {
                expect_type(header, types::COLOR)?;
                set_root_field(
                    &mut self.sleeping_body_color,
                    "SleepingBodyColor",
                    read_color(header)?,
                )?;
            }
            Some(CUSTOM_FILTER_COLORS_FIELD_CRC) => {
                expect_type(header, COLLISION_FILTER_COLOR_VECTOR_TYPE_ID)?;
                if self.saw_custom_filter_colors {
                    return Err(CollisionFiltersParseError::DuplicateRootField {
                        field: "CustomFilterColors",
                    });
                }
                self.saw_custom_filter_colors = true;
                self.custom_colors_depth = Some(self.depth);
            }
            _ => {
                return Err(CollisionFiltersParseError::UnexpectedFieldType {
                    type_id: header.id,
                    name_crc: header.name_crc,
                });
            }
        }
        Ok(())
    }

    fn read_filter_field(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<(), CollisionFiltersParseError> {
        let index = self.filters.len();
        match header.name_crc {
            Some(NAME_FIELD_CRC) => {
                expect_type(header, types::AZSTD_STRING)?;
                let value = Box::<str>::from(header.decode::<&str>()?);
                let current = self.current_filter_mut(header)?;
                set_filter_field(&mut current.name, index, "Name", value)?;
            }
            Some(DESCRIPTION_FIELD_CRC) => {
                expect_type(header, types::AZSTD_STRING)?;
                let value = Box::<str>::from(header.decode::<&str>()?);
                let current = self.current_filter_mut(header)?;
                set_filter_field(&mut current.description, index, "Description", value)?;
            }
            Some(INHERITS_FILTERS_FIELD_CRC) => {
                expect_type(header, STRING_VECTOR_TYPE_ID)?;
                let current = self.current_filter_mut(header)?;
                if current.saw_inherits_filters {
                    return Err(CollisionFiltersParseError::DuplicateFilterField {
                        index,
                        field: "InheritsFilters",
                    });
                }
                current.saw_inherits_filters = true;
                self.start_string_list(ActiveStringListTarget::FilterInherits, self.depth)?;
            }
            Some(IS_CATEGORIES_FIELD_CRC) => {
                expect_type(header, STRING_VECTOR_TYPE_ID)?;
                let current = self.current_filter_mut(header)?;
                if current.saw_is_categories {
                    return Err(CollisionFiltersParseError::DuplicateFilterField {
                        index,
                        field: "IsCategories",
                    });
                }
                current.saw_is_categories = true;
                self.start_string_list(ActiveStringListTarget::FilterIsCategories, self.depth)?;
            }
            Some(COLLIDE_WITH_CATEGORIES_FIELD_CRC) => {
                expect_type(header, STRING_VECTOR_TYPE_ID)?;
                let current = self.current_filter_mut(header)?;
                if current.saw_collide_with_categories {
                    return Err(CollisionFiltersParseError::DuplicateFilterField {
                        index,
                        field: "CollideWithCategories",
                    });
                }
                current.saw_collide_with_categories = true;
                self.start_string_list(
                    ActiveStringListTarget::FilterCollideWithCategories,
                    self.depth,
                )?;
            }
            Some(FILTER_TAGS_FIELD_CRC) => {
                expect_type(header, COLLISION_FILTER_TAG_VECTOR_TYPE_ID)?;
                let current = self.current_filter_mut(header)?;
                if current.saw_filter_tags {
                    return Err(CollisionFiltersParseError::DuplicateFilterField {
                        index,
                        field: "FilterTags",
                    });
                }
                current.saw_filter_tags = true;
                if self.tag_list_depth.replace(self.depth).is_some() {
                    return Err(CollisionFiltersParseError::NestedFilterTagVector);
                }
            }
            _ => {
                return Err(CollisionFiltersParseError::UnexpectedFieldType {
                    type_id: header.id,
                    name_crc: header.name_crc,
                });
            }
        }
        Ok(())
    }

    fn read_color_field(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<(), CollisionFiltersParseError> {
        let index = self.custom_filter_colors.len();
        match header.name_crc {
            Some(NAME_FIELD_CRC) => {
                expect_type(header, types::AZSTD_STRING)?;
                let value = Box::<str>::from(header.decode::<&str>()?);
                let current = self.current_color_mut(header)?;
                set_color_field(&mut current.name, index, "Name", value)?;
            }
            Some(COLOR_FIELD_CRC) => {
                expect_type(header, types::COLOR)?;
                let value = read_color(header)?;
                let current = self.current_color_mut(header)?;
                set_color_field(&mut current.color, index, "Color", value)?;
            }
            _ => {
                return Err(CollisionFiltersParseError::UnexpectedFieldType {
                    type_id: header.id,
                    name_crc: header.name_crc,
                });
            }
        }
        Ok(())
    }

    fn read_string_list_value(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<(), CollisionFiltersParseError> {
        expect_type(header, types::AZSTD_STRING)?;
        let value = Box::<str>::from(header.decode::<&str>()?);
        match self
            .string_list
            .as_ref()
            .ok_or(CollisionFiltersParseError::UnexpectedStringValue)?
            .target
        {
            ActiveStringListTarget::Categories => self.categories.push(value),
            ActiveStringListTarget::FilterInherits => {
                let current = self.current_filter_mut(header)?;
                current.inherits_filters.push(value);
            }
            ActiveStringListTarget::FilterIsCategories => {
                let current = self.current_filter_mut(header)?;
                current.is_categories.push(value);
            }
            ActiveStringListTarget::FilterCollideWithCategories => {
                let current = self.current_filter_mut(header)?;
                current.collide_with_categories.push(value);
            }
        }
        Ok(())
    }

    fn read_filter_tag(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<(), CollisionFiltersParseError> {
        expect_type(header, types::UNSIGNED_CHAR)?;
        let value = header.decode::<u8>()?;
        let current = self.current_filter_mut(header)?;
        current.filter_tags.push(value);
        Ok(())
    }

    fn start_string_list(
        &mut self,
        target: ActiveStringListTarget,
        depth: usize,
    ) -> Result<(), CollisionFiltersParseError> {
        if self
            .string_list
            .replace(ActiveStringList { target, depth })
            .is_some()
        {
            return Err(CollisionFiltersParseError::NestedStringVector);
        }
        Ok(())
    }

    fn current_filter_mut(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<&mut PartialEditableCollisionFilter, CollisionFiltersParseError> {
        self.current_filter
            .as_mut()
            .ok_or(CollisionFiltersParseError::UnexpectedFieldType {
                type_id: header.id,
                name_crc: header.name_crc,
            })
    }

    fn current_color_mut(
        &mut self,
        header: &ElementHeader<'_>,
    ) -> Result<&mut PartialCollisionFilterColor, CollisionFiltersParseError> {
        self.current_color
            .as_mut()
            .ok_or(CollisionFiltersParseError::UnexpectedFieldType {
                type_id: header.id,
                name_crc: header.name_crc,
            })
    }

    fn finish_filter(&mut self) -> Result<(), CollisionFiltersParseError> {
        let index = self.filters.len();
        let current =
            self.current_filter
                .take()
                .ok_or(CollisionFiltersParseError::MissingFilterField {
                    index,
                    field: "EditableCollisionFilter",
                })?;
        self.filters.push(current.finish(index)?);
        Ok(())
    }

    fn finish_color(&mut self) -> Result<(), CollisionFiltersParseError> {
        let index = self.custom_filter_colors.len();
        let current =
            self.current_color
                .take()
                .ok_or(CollisionFiltersParseError::MissingColorField {
                    index,
                    field: "CollisionFilterColor",
                })?;
        self.custom_filter_colors.push(current.finish(index)?);
        Ok(())
    }
}

impl ElementVisitor for CollisionFiltersVisitor {
    type Error = CollisionFiltersParseError;

    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        let depth = self.depth;
        if depth == 0 {
            self.roots += 1;
            if self.roots > 1 {
                return Err(CollisionFiltersParseError::MultipleRoots);
            }
            if header.id != COLLISION_FILTERS_ASSET_TYPE_ID {
                return Err(CollisionFiltersParseError::UnexpectedRootType { type_id: header.id });
            }
        } else if self
            .string_list
            .as_ref()
            .is_some_and(|list| depth == list.depth + 1)
        {
            self.read_string_list_value(header)?;
        } else if self
            .tag_list_depth
            .is_some_and(|tag_depth| depth == tag_depth + 1)
        {
            self.read_filter_tag(header)?;
        } else if depth == 1 {
            self.read_root_field(header)?;
        } else if self.filters_depth == Some(1) && depth == 2 {
            if self.current_filter.is_some() {
                return Err(CollisionFiltersParseError::NestedFilter);
            }
            if header.id != EDITABLE_COLLISION_FILTER_TYPE_ID {
                return Err(CollisionFiltersParseError::UnexpectedFilterType {
                    type_id: header.id,
                });
            }
            self.current_filter = Some(PartialEditableCollisionFilter::default());
            self.filter_depth = Some(depth);
        } else if self.filter_depth == Some(2) && depth == 3 {
            self.read_filter_field(header)?;
        } else if self.custom_colors_depth == Some(1) && depth == 2 {
            if self.current_color.is_some() {
                return Err(CollisionFiltersParseError::NestedFilterColor);
            }
            if header.id != COLLISION_FILTER_COLOR_TYPE_ID {
                return Err(CollisionFiltersParseError::UnexpectedFilterColorType {
                    type_id: header.id,
                });
            }
            self.current_color = Some(PartialCollisionFilterColor::default());
            self.custom_color_depth = Some(depth);
        } else if self.custom_color_depth == Some(2) && depth == 3 {
            self.read_color_field(header)?;
        }

        self.depth += 1;
        Ok(VisitFlow::Continue)
    }

    fn close_element(&mut self) -> Result<(), Self::Error> {
        self.depth = self.depth.saturating_sub(1);

        if self
            .string_list
            .as_ref()
            .is_some_and(|list| list.depth == self.depth)
        {
            self.string_list = None;
        }
        if self.tag_list_depth == Some(self.depth) {
            self.tag_list_depth = None;
        }
        if self.filter_depth == Some(self.depth) {
            self.finish_filter()?;
            self.filter_depth = None;
        }
        if self.filters_depth == Some(self.depth) {
            self.filters_depth = None;
        }
        if self.categories_depth == Some(self.depth) {
            self.categories_depth = None;
        }
        if self.custom_color_depth == Some(self.depth) {
            self.finish_color()?;
            self.custom_color_depth = None;
        }
        if self.custom_colors_depth == Some(self.depth) {
            self.custom_colors_depth = None;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ActiveStringList {
    target: ActiveStringListTarget,
    depth: usize,
}

#[derive(Debug, Clone, Copy)]
enum ActiveStringListTarget {
    Categories,
    FilterInherits,
    FilterIsCategories,
    FilterCollideWithCategories,
}

#[derive(Debug, Default)]
struct PartialEditableCollisionFilter {
    name: Option<Box<str>>,
    description: Option<Box<str>>,
    saw_inherits_filters: bool,
    saw_is_categories: bool,
    saw_collide_with_categories: bool,
    saw_filter_tags: bool,
    inherits_filters: Vec<Box<str>>,
    is_categories: Vec<Box<str>>,
    collide_with_categories: Vec<Box<str>>,
    filter_tags: Vec<u8>,
}

impl PartialEditableCollisionFilter {
    fn finish(self, index: usize) -> Result<EditableCollisionFilter, CollisionFiltersParseError> {
        if !self.saw_inherits_filters {
            return Err(CollisionFiltersParseError::MissingFilterField {
                index,
                field: "InheritsFilters",
            });
        }
        if !self.saw_is_categories {
            return Err(CollisionFiltersParseError::MissingFilterField {
                index,
                field: "IsCategories",
            });
        }
        if !self.saw_collide_with_categories {
            return Err(CollisionFiltersParseError::MissingFilterField {
                index,
                field: "CollideWithCategories",
            });
        }
        if !self.saw_filter_tags {
            return Err(CollisionFiltersParseError::MissingFilterField {
                index,
                field: "FilterTags",
            });
        }
        Ok(EditableCollisionFilter::new(
            self.name
                .ok_or(CollisionFiltersParseError::MissingFilterField {
                    index,
                    field: "Name",
                })?,
            self.description
                .ok_or(CollisionFiltersParseError::MissingFilterField {
                    index,
                    field: "Description",
                })?,
            self.inherits_filters,
            self.is_categories,
            self.collide_with_categories,
            self.filter_tags,
        ))
    }
}

#[derive(Debug, Default)]
struct PartialCollisionFilterColor {
    name: Option<Box<str>>,
    color: Option<LinearRgba>,
}

impl PartialCollisionFilterColor {
    fn finish(self, index: usize) -> Result<CollisionFilterColor, CollisionFiltersParseError> {
        Ok(CollisionFilterColor::new(
            self.name
                .ok_or(CollisionFiltersParseError::MissingColorField {
                    index,
                    field: "Name",
                })?,
            self.color
                .ok_or(CollisionFiltersParseError::MissingColorField {
                    index,
                    field: "Color",
                })?,
        ))
    }
}

fn set_root_field<T>(
    target: &mut Option<T>,
    field: &'static str,
    value: T,
) -> Result<(), CollisionFiltersParseError> {
    if target.replace(value).is_some() {
        return Err(CollisionFiltersParseError::DuplicateRootField { field });
    }
    Ok(())
}

fn set_filter_field<T>(
    target: &mut Option<T>,
    index: usize,
    field: &'static str,
    value: T,
) -> Result<(), CollisionFiltersParseError> {
    if target.replace(value).is_some() {
        return Err(CollisionFiltersParseError::DuplicateFilterField { index, field });
    }
    Ok(())
}

fn set_color_field<T>(
    target: &mut Option<T>,
    index: usize,
    field: &'static str,
    value: T,
) -> Result<(), CollisionFiltersParseError> {
    if target.replace(value).is_some() {
        return Err(CollisionFiltersParseError::DuplicateColorField { index, field });
    }
    Ok(())
}

fn expect_type(
    header: &ElementHeader<'_>,
    expected: Uuid,
) -> Result<(), CollisionFiltersParseError> {
    if header.id != expected {
        return Err(CollisionFiltersParseError::UnexpectedFieldType {
            type_id: header.id,
            name_crc: header.name_crc,
        });
    }
    Ok(())
}

fn read_color(header: &ElementHeader<'_>) -> Result<LinearRgba, CollisionFiltersParseError> {
    let [red, green, blue, alpha] = header.decode::<[f32; 4]>()?;
    Ok(LinearRgba::new(red, green, blue, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nw_objectstream::{
        Element, ObjectStream, ST_BINARYFLAG_ELEMENT_HEADER, ST_BINARYFLAG_EXTRA_SIZE_FIELD,
        ST_BINARYFLAG_HAS_NAME, ST_BINARYFLAG_HAS_VALUE,
    };

    #[test]
    fn parses_collision_filters() {
        let bytes = collision_filters_fixture(true);

        let asset = CollisionFiltersAsset::parse(&bytes).unwrap();

        assert_eq!(
            asset.categories(),
            &[Box::<str>::from("Default"), Box::<str>::from("Player")]
        );
        assert_eq!(asset.filters().len(), 1);
        let filter = &asset.filters()[0];
        assert_eq!(filter.name.as_ref(), "PlayerFilter");
        assert_eq!(filter.description.as_ref(), "Player collision");
        assert_eq!(filter.inherits_filters, vec![Box::<str>::from("Base")]);
        assert_eq!(filter.is_categories, vec![Box::<str>::from("Player")]);
        assert_eq!(
            filter.collide_with_categories,
            vec![Box::<str>::from("World")]
        );
        assert_eq!(filter.filter_tags, vec![7]);
        assert_color(asset.character_filter_color(), [1.0, 0.0, 0.0, 1.0]);
        assert_color(asset.ghost_filter_color(), [0.0, 1.0, 0.0, 1.0]);
        assert_color(asset.sleeping_body_color(), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(asset.custom_filter_colors().len(), 1);
        assert_eq!(asset.custom_filter_colors()[0].name.as_ref(), "Custom");
        assert_color(
            asset.custom_filter_colors()[0].color,
            [0.25, 0.5, 0.75, 1.0],
        );

        let summary = asset.summary();
        assert_eq!(summary.categories, 2);
        assert_eq!(summary.filters, 1);
        assert_eq!(summary.custom_colors, 1);
        assert_eq!(summary.filter_tag_bytes, 1);
        assert_eq!(summarize_collision_filters_asset(&bytes).unwrap(), summary);

        let mut totals = CollisionFiltersTotals::default();
        totals.add_summary(summary);
        assert_eq!(totals.files, 1);
        assert_eq!(totals.filter_tag_bytes, 1);
        assert_eq!(
            summary.to_string(),
            "2 categories, 1 filters, 1 custom colors"
        );
        assert_eq!(
            totals.to_string(),
            "  files: 1\n  categories: 2\n  filters: 1\n  custom colors: 1\n  filter tag bytes: 1\n"
        );

        let row =
            inspect_collision_filters_file("physics/default.collisionfilters", &bytes).unwrap();
        let mut inspection = CollisionFiltersInspection::default();
        inspection.add_file_summary(row);
        assert_eq!(
            inspection.report(20).to_string(),
            "physics/default.collisionfilters: 2 categories, 1 filters, 1 custom colors\n  files: 1\n  categories: 2\n  filters: 1\n  custom colors: 1\n  filter tag bytes: 1\n"
        );

        let path = std::env::temp_dir().join(format!(
            "az-physics-assets-collision-filters-{}.collisionfilters",
            std::process::id()
        ));
        std::fs::write(&path, &bytes).expect("write collision filters");
        let inspection =
            inspect_collision_filters_files([&path]).expect("inspect collision filters files");
        assert_eq!(inspection.totals.files, 1);
        assert_eq!(inspection.totals.filter_tag_bytes, 1);
        std::fs::remove_file(path).expect("remove collision filters");

        assert!(is_collision_filters_name("default.COLLISIONFILTERS"));
    }

    #[test]
    fn rejects_missing_filter_tags() {
        let bytes = collision_filters_fixture(false);

        assert!(matches!(
            CollisionFiltersAsset::parse(&bytes),
            Err(CollisionFiltersParseError::MissingFilterField {
                field: "FilterTags",
                ..
            })
        ));
    }

    #[test]
    fn parses_material_set() {
        let asset = MaterialSetAsset::parse_str(material_set_fixture(true)).unwrap();

        let material_set = asset.material_set();
        assert_eq!(material_set.default_material.name.as_ref(), "Default");
        assert_eq!(material_set.default_material.friction, 0.5);
        assert_eq!(material_set.default_material.restitution, 0.0);
        assert!(material_set.default_material.traversable);
        assert_eq!(
            material_set.default_material.surface_type.as_ref(),
            "mat_default"
        );
        assert_eq!(material_set.materials.len(), 1);
        let material = &material_set.materials[0].configuration;
        assert_eq!(material.name.as_ref(), "Wood NoTraverse");
        assert_eq!(material.friction, 0.6);
        assert_eq!(material.restitution, 0.1);
        assert!(!material.traversable);
        assert_eq!(material.surface_type.as_ref(), "mat_wood_notraverse");

        let summary = asset.summary();
        assert_eq!(summary.material_entries, 1);
        assert_eq!(summary.materials, 2);
        assert_eq!(summary.traversable_materials, 1);
        assert_eq!(
            summarize_material_set_asset(material_set_fixture(true).as_bytes()).unwrap(),
            summary
        );

        let mut totals = MaterialSetTotals::default();
        totals.add_summary(summary);
        assert_eq!(totals.files, 1);
        assert_eq!(totals.materials, 2);
        assert_eq!(summary.to_string(), "1 materials");
        assert_eq!(
            totals.to_string(),
            "  files: 1\n  materials: 2\n  traversable materials: 1\n"
        );

        let row = inspect_material_set_file(
            "physics/default.physicsmaterialset",
            material_set_fixture(true).as_bytes(),
        )
        .unwrap();
        let mut inspection = MaterialSetInspection::default();
        inspection.add_file_summary(row);
        assert_eq!(
            inspection.report(20).to_string(),
            "physics/default.physicsmaterialset: 1 materials\n  files: 1\n  materials: 2\n  traversable materials: 1\n"
        );

        let path = std::env::temp_dir().join(format!(
            "az-physics-assets-material-set-{}.physicsmaterialset",
            std::process::id()
        ));
        std::fs::write(&path, material_set_fixture(true)).expect("write material set");
        let inspection = inspect_material_set_files([&path]).expect("inspect material set files");
        assert_eq!(inspection.totals.files, 1);
        assert_eq!(inspection.totals.materials, 2);
        std::fs::remove_file(path).expect("remove material set");

        assert!(is_physics_material_set_name("default.PHYSICSMATERIALSET"));
    }

    #[test]
    fn parses_material_set_with_empty_materials_list() {
        let asset = MaterialSetAsset::parse_str(material_set_empty_materials_fixture()).unwrap();

        assert_eq!(
            asset.material_set().default_material.name.as_ref(),
            "Default"
        );
        assert!(asset.material_set().materials.is_empty());
        assert_eq!(asset.summary().materials, 1);
    }

    #[test]
    fn rejects_material_set_missing_surface_type() {
        assert!(matches!(
            MaterialSetAsset::parse_str(material_set_fixture(false)),
            Err(MaterialSetParseError::MissingField {
                owner: "MaterialProperties",
                field: "SurfaceType"
            })
        ));
    }

    fn collision_filters_fixture(include_filter_tags: bool) -> Vec<u8> {
        let filter_tags = include_filter_tags.then(|| {
            named_element(
                COLLISION_FILTER_TAG_VECTOR_TYPE_ID,
                FILTER_TAGS_FIELD_CRC,
                vec![byte_field(0, 7)],
            )
        });
        let mut stream = ObjectStream::new(3);
        stream.elements = vec![Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER,
            id: COLLISION_FILTERS_ASSET_TYPE_ID,
            elements: vec![
                named_element(
                    STRING_VECTOR_TYPE_ID,
                    CATEGORIES_FIELD_CRC,
                    vec![string_value("Default"), string_value("Player")],
                ),
                named_element(
                    COLLISION_FILTER_VECTOR_TYPE_ID,
                    FILTERS_FIELD_CRC,
                    vec![filter_element(filter_tags)],
                ),
                color_field(CHARACTER_FILTER_COLOR_FIELD_CRC, [1.0, 0.0, 0.0, 1.0]),
                color_field(GHOST_FILTER_COLOR_FIELD_CRC, [0.0, 1.0, 0.0, 1.0]),
                color_field(SLEEPING_BODY_COLOR_FIELD_CRC, [0.0, 0.0, 1.0, 1.0]),
                named_element(
                    COLLISION_FILTER_COLOR_VECTOR_TYPE_ID,
                    CUSTOM_FILTER_COLORS_FIELD_CRC,
                    vec![Element {
                        flags: ST_BINARYFLAG_ELEMENT_HEADER,
                        id: COLLISION_FILTER_COLOR_TYPE_ID,
                        elements: vec![
                            string_field(NAME_FIELD_CRC, "Custom"),
                            color_field(COLOR_FIELD_CRC, [0.25, 0.5, 0.75, 1.0]),
                        ],
                        ..Default::default()
                    }],
                ),
            ],
            ..Default::default()
        }];
        let mut bytes = Vec::new();
        stream.write_to(&mut bytes).unwrap();
        bytes
    }

    fn material_set_fixture(include_surface_type: bool) -> &'static str {
        if include_surface_type {
            r#"<ObjectStream version="3">
  <Class name="MaterialSetAsset" type="{9E366D8C-33BB-4825-9A1F-FA3ADBE11D0F}">
    <Class name="MaterialSet" field="BaseClass1" version="1" type="{84399E75-18AB-4000-8DCA-07B9D4E0F8E8}">
      <Class name="MaterialProperties" field="DefaultMaterial" version="1" type="{8807CAA1-AD08-4238-8FDB-2154ADD084A1}">
        <Class name="AZStd::string" field="Name" value="Default" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
        <Class name="float" field="Friction" value="0.5000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="float" field="Restitution" value="0.0000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="bool" field="Traversable" value="true" type="{A0CA880C-AFE4-43CB-926C-59AC48496112}"/>
        <Class name="AZStd::string" field="SurfaceType" value="mat_default" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
      </Class>
      <Class name="AZStd::list" field="Materials" type="{9800688D-64A7-5C0D-9F79-E32E310BB924}">
        <Class name="MaterialEntry" field="element" version="1" type="{C5207CC2-EF1B-4A11-BC8F-F1898282FBE5}">
          <Class name="MaterialProperties" field="Configuration" version="1" type="{8807CAA1-AD08-4238-8FDB-2154ADD084A1}">
            <Class name="AZStd::string" field="Name" value="Wood NoTraverse" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
            <Class name="float" field="Friction" value="0.6000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
            <Class name="float" field="Restitution" value="0.1000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
            <Class name="bool" field="Traversable" value="false" type="{A0CA880C-AFE4-43CB-926C-59AC48496112}"/>
            <Class name="AZStd::string" field="SurfaceType" value="mat_wood_notraverse" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
          </Class>
        </Class>
      </Class>
    </Class>
  </Class>
</ObjectStream>"#
        } else {
            r#"<ObjectStream version="3">
  <Class name="MaterialSetAsset" type="{9E366D8C-33BB-4825-9A1F-FA3ADBE11D0F}">
    <Class name="MaterialSet" field="BaseClass1" version="1" type="{84399E75-18AB-4000-8DCA-07B9D4E0F8E8}">
      <Class name="MaterialProperties" field="DefaultMaterial" version="1" type="{8807CAA1-AD08-4238-8FDB-2154ADD084A1}">
        <Class name="AZStd::string" field="Name" value="Default" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
        <Class name="float" field="Friction" value="0.5000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="float" field="Restitution" value="0.0000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="bool" field="Traversable" value="true" type="{A0CA880C-AFE4-43CB-926C-59AC48496112}"/>
      </Class>
      <Class name="AZStd::list" field="Materials" type="{9800688D-64A7-5C0D-9F79-E32E310BB924}"/>
    </Class>
  </Class>
</ObjectStream>"#
        }
    }

    fn material_set_empty_materials_fixture() -> &'static str {
        r#"<ObjectStream version="3">
  <Class name="MaterialSetAsset" type="{9E366D8C-33BB-4825-9A1F-FA3ADBE11D0F}">
    <Class name="MaterialSet" field="BaseClass1" version="1" type="{84399E75-18AB-4000-8DCA-07B9D4E0F8E8}">
      <Class name="MaterialProperties" field="DefaultMaterial" version="1" type="{8807CAA1-AD08-4238-8FDB-2154ADD084A1}">
        <Class name="AZStd::string" field="Name" value="Default" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
        <Class name="float" field="Friction" value="0.5000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="float" field="Restitution" value="0.0000000" type="{EA2C3E90-AFBE-44D4-A90D-FAAF79BAF93D}"/>
        <Class name="bool" field="Traversable" value="true" type="{A0CA880C-AFE4-43CB-926C-59AC48496112}"/>
        <Class name="AZStd::string" field="SurfaceType" value="mat_default" type="{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}"/>
      </Class>
      <Class name="AZStd::list" field="Materials" type="{9800688D-64A7-5C0D-9F79-E32E310BB924}"/>
    </Class>
  </Class>
</ObjectStream>"#
    }

    fn filter_element(filter_tags: Option<Element>) -> Element {
        let mut elements = vec![
            string_field(NAME_FIELD_CRC, "PlayerFilter"),
            string_field(DESCRIPTION_FIELD_CRC, "Player collision"),
            named_element(
                STRING_VECTOR_TYPE_ID,
                INHERITS_FILTERS_FIELD_CRC,
                vec![string_value("Base")],
            ),
            named_element(
                STRING_VECTOR_TYPE_ID,
                IS_CATEGORIES_FIELD_CRC,
                vec![string_value("Player")],
            ),
            named_element(
                STRING_VECTOR_TYPE_ID,
                COLLIDE_WITH_CATEGORIES_FIELD_CRC,
                vec![string_value("World")],
            ),
        ];
        elements.extend(filter_tags);
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER,
            id: EDITABLE_COLLISION_FILTER_TYPE_ID,
            elements,
            ..Default::default()
        }
    }

    fn named_element(id: Uuid, name_crc: u32, elements: Vec<Element>) -> Element {
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER | ST_BINARYFLAG_HAS_NAME,
            name_crc: Some(name_crc),
            id,
            elements,
            ..Default::default()
        }
    }

    fn string_field(name_crc: u32, value: &str) -> Element {
        let mut element = string_value(value);
        element.flags |= ST_BINARYFLAG_HAS_NAME;
        element.name_crc = Some(name_crc);
        element
    }

    fn string_value(value: &str) -> Element {
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER
                | ST_BINARYFLAG_HAS_VALUE
                | ST_BINARYFLAG_EXTRA_SIZE_FIELD
                | 1,
            id: types::AZSTD_STRING,
            data_size: Some(value.len()),
            data: Some(value.as_bytes().into()),
            ..Default::default()
        }
    }

    fn byte_field(name_crc: u32, value: u8) -> Element {
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER
                | (if name_crc == 0 {
                    0
                } else {
                    ST_BINARYFLAG_HAS_NAME
                })
                | ST_BINARYFLAG_HAS_VALUE
                | ST_BINARYFLAG_EXTRA_SIZE_FIELD
                | 1,
            name_crc: (name_crc != 0).then_some(name_crc),
            id: types::UNSIGNED_CHAR,
            data_size: Some(1),
            data: Some(vec![value]),
            ..Default::default()
        }
    }

    fn color_field(name_crc: u32, value: [f32; 4]) -> Element {
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER
                | ST_BINARYFLAG_HAS_NAME
                | ST_BINARYFLAG_HAS_VALUE
                | ST_BINARYFLAG_EXTRA_SIZE_FIELD
                | 1,
            name_crc: Some(name_crc),
            id: types::COLOR,
            data_size: Some(16),
            data: Some(value.into_iter().flat_map(f32::to_be_bytes).collect()),
            ..Default::default()
        }
    }

    fn assert_color(actual: LinearRgba, expected: [f32; 4]) {
        assert_eq!(
            [actual.red, actual.green, actual.blue, actual.alpha],
            expected
        );
    }
}
