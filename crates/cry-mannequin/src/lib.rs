//! CryAction Mannequin asset parsing.
//!
//! Follows `dev/Gems/CryLegacy/Code/Source/CryAction/Mannequin/AnimationDatabaseManager.cpp`.

pub mod blend_space;
pub mod reflected;
pub mod source_transform;

use std::{
    borrow::Cow,
    fmt, io,
    num::{ParseFloatError, ParseIntError},
    path::{Path, PathBuf},
    str,
};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

pub use blend_space::{
    BLEND_SPACE_SOURCE_SCHEMA, BlendSpace, BlendSpaceAdditionalExtraction, BlendSpaceAnimationRef,
    BlendSpaceAnnotation, BlendSpaceCoordinate, BlendSpaceDimension, BlendSpaceExample,
    BlendSpaceJoint, BlendSpaceMotionCombination, BlendSpacePseudoExample, BlendSpaceReference,
    BlendSpaceSource, BlendSpaceSourceArtifact, BlendSpaceSourceError, BlendSpaceSourceInput,
    BlendSpaceSourceSchema, BlendSpaceSourceTransform, BlendSpaceSourceTransformError,
    BlendSpaceVirtualExample, BlendSpaceXmlKind, COMBINED_BLEND_SPACE_SOURCE_SCHEMA,
    CombinedBlendSpace, CombinedBlendSpaceDimension, CombinedBlendSpaceSource,
    blend_space_source_path, is_legacy_blend_space_source, motion_parameter_id,
};
pub use source_transform::{
    MANNEQUIN_ACTIONS_SOURCE_SCHEMA, MANNEQUIN_ANIMATION_DATABASE_SOURCE_SCHEMA,
    MANNEQUIN_CONTROLLER_SOURCE_SCHEMA, MANNEQUIN_TAGS_SOURCE_SCHEMA, MannequinActionsSource,
    MannequinAnimationDatabase, MannequinAnimationDatabaseSource,
    MannequinAnimationDatabaseSourceError, MannequinAnimationEntry, MannequinAnimationLayer,
    MannequinControllerDefinitionSource, MannequinFileReference, MannequinFragment,
    MannequinFragmentBlend, MannequinFragmentBlendVariant, MannequinFragmentDefinition,
    MannequinFragmentGroup, MannequinFragmentOverride, MannequinLayerBlend, MannequinProcedural,
    MannequinProceduralLayer, MannequinProceduralParameter, MannequinScopeContext,
    MannequinScopeDefinition, MannequinSourceArtifact, MannequinSourceInput, MannequinSourceSchema,
    MannequinSourceTransform, MannequinSourceTransformError, MannequinSubDatabase,
    MannequinTagDefinitionEntry, MannequinTagDefinitionSource, MannequinTagEntry,
    MannequinTagGroup, MannequinTagsSource, MannequinXmlKind, is_legacy_mannequin_source,
    mannequin_source_path, motion_path_from_animation_reference,
};

const ANIM_DB: &[u8] = b"AnimDB";
const SUB_ADBS: &[u8] = b"SubADBs";
const SUB_ADB: &[u8] = b"SubADB";
const FRAGMENT_ID: &[u8] = b"FragmentID";
const FRAGMENT_LIST: &[u8] = b"FragmentList";
const FRAGMENT: &[u8] = b"Fragment";
const ANIM_LAYER: &[u8] = b"AnimLayer";
const PROC_LAYER: &[u8] = b"ProcLayer";
const BLEND: &[u8] = b"Blend";
const ANIMATION: &[u8] = b"Animation";
const PROCEDURAL: &[u8] = b"Procedural";
const PROCEDURAL_PARAMS: &[u8] = b"ProceduralParams";
const FRAGMENT_BLEND_LIST: &[u8] = b"FragmentBlendList";
const VARIANT: &[u8] = b"Variant";

/// Number of Mannequin animation blend channels.
///
/// Defined by `dev/Code/CryEngine/CryCommon/ICryMannequin.h`.
pub const BLEND_CHANNELS: usize = 4;
pub const ANIMATION_DATABASE_EXTENSION: &str = "adb";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AnimationDatabaseInspectionError {
    #[error("read {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("parse animation database {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: AnimationDatabaseParseError,
    },
}

/// Summary returned after visiting a Mannequin animation database.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimationDatabaseStats {
    pub databases: usize,
    pub sub_databases: usize,
    pub sub_database_fragment_filters: usize,
    pub fragment_groups: usize,
    pub fragments: usize,
    pub transition_fragments: usize,
    pub anim_layers: usize,
    pub proc_layers: usize,
    pub layer_blends: usize,
    pub animations: usize,
    pub procedurals: usize,
    pub procedural_parameter_groups: usize,
    pub procedural_parameter_values: usize,
    pub fragment_blends: usize,
    pub fragment_blend_variants: usize,
}

impl fmt::Display for AnimationDatabaseStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} fragments, {} transitions, {} animations, {} procedurals",
            self.fragments, self.transition_fragments, self.animations, self.procedurals
        )
    }
}

pub fn summarize_animation_database(
    bytes: &[u8],
) -> Result<AnimationDatabaseStats, AnimationDatabaseParseError> {
    visit_animation_database(bytes, |_| Ok(()))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimationDatabaseTotals {
    pub files: usize,
    pub databases: usize,
    pub sub_databases: usize,
    pub sub_database_fragment_filters: usize,
    pub fragment_groups: usize,
    pub fragments: usize,
    pub transition_fragments: usize,
    pub anim_layers: usize,
    pub proc_layers: usize,
    pub layer_blends: usize,
    pub animations: usize,
    pub procedurals: usize,
    pub procedural_parameter_groups: usize,
    pub procedural_parameter_values: usize,
    pub fragment_blends: usize,
    pub fragment_blend_variants: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationDatabaseFileSummary {
    pub source: String,
    pub stats: AnimationDatabaseStats,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AnimationDatabaseInspection {
    pub rows: Vec<AnimationDatabaseFileSummary>,
    pub totals: AnimationDatabaseTotals,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimationDatabaseInspectionReport<'a> {
    inspection: &'a AnimationDatabaseInspection,
    limit: usize,
}

impl AnimationDatabaseTotals {
    pub fn add_stats(&mut self, stats: AnimationDatabaseStats) {
        self.files += 1;
        self.databases += stats.databases;
        self.sub_databases += stats.sub_databases;
        self.sub_database_fragment_filters += stats.sub_database_fragment_filters;
        self.fragment_groups += stats.fragment_groups;
        self.fragments += stats.fragments;
        self.transition_fragments += stats.transition_fragments;
        self.anim_layers += stats.anim_layers;
        self.proc_layers += stats.proc_layers;
        self.layer_blends += stats.layer_blends;
        self.animations += stats.animations;
        self.procedurals += stats.procedurals;
        self.procedural_parameter_groups += stats.procedural_parameter_groups;
        self.procedural_parameter_values += stats.procedural_parameter_values;
        self.fragment_blends += stats.fragment_blends;
        self.fragment_blend_variants += stats.fragment_blend_variants;
    }
}

impl AnimationDatabaseInspection {
    pub fn add_file_summary(&mut self, row: AnimationDatabaseFileSummary) {
        self.totals.add_stats(row.stats);
        self.rows.push(row);
    }

    #[must_use]
    pub const fn report(&self, limit: usize) -> AnimationDatabaseInspectionReport<'_> {
        AnimationDatabaseInspectionReport {
            inspection: self,
            limit,
        }
    }
}

impl fmt::Display for AnimationDatabaseTotals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  files: {}", self.files)?;
        writeln!(f, "  databases: {}", self.databases)?;
        writeln!(f, "  sub databases: {}", self.sub_databases)?;
        writeln!(
            f,
            "  sub database fragment filters: {}",
            self.sub_database_fragment_filters
        )?;
        writeln!(f, "  fragment groups: {}", self.fragment_groups)?;
        writeln!(f, "  fragments: {}", self.fragments)?;
        writeln!(f, "  transition fragments: {}", self.transition_fragments)?;
        writeln!(f, "  animation layers: {}", self.anim_layers)?;
        writeln!(f, "  procedural layers: {}", self.proc_layers)?;
        writeln!(f, "  layer blends: {}", self.layer_blends)?;
        writeln!(f, "  animations: {}", self.animations)?;
        writeln!(f, "  procedurals: {}", self.procedurals)?;
        writeln!(
            f,
            "  procedural parameter groups: {}",
            self.procedural_parameter_groups
        )?;
        writeln!(
            f,
            "  procedural parameter values: {}",
            self.procedural_parameter_values
        )?;
        writeln!(f, "  fragment blends: {}", self.fragment_blends)?;
        writeln!(
            f,
            "  fragment blend variants: {}",
            self.fragment_blend_variants
        )
    }
}

impl fmt::Display for AnimationDatabaseInspectionReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.limit > 0 {
            for row in self.inspection.rows.iter().take(self.limit) {
                writeln!(f, "{}: {}", row.source, row.stats)?;
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

pub fn inspect_animation_database_file(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<AnimationDatabaseFileSummary, AnimationDatabaseParseError> {
    Ok(AnimationDatabaseFileSummary {
        source: path.as_ref().display().to_string(),
        stats: summarize_animation_database(bytes)?,
    })
}

pub fn inspect_animation_database_path(
    path: impl AsRef<Path>,
) -> Result<AnimationDatabaseFileSummary, AnimationDatabaseInspectionError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| AnimationDatabaseInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    inspect_animation_database_file(path, &bytes).map_err(|source| {
        AnimationDatabaseInspectionError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

pub fn inspect_animation_database_files<I, P>(
    paths: I,
) -> Result<AnimationDatabaseInspection, AnimationDatabaseInspectionError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut inspection = AnimationDatabaseInspection::default();
    for path in paths {
        inspection.add_file_summary(inspect_animation_database_path(path)?);
    }
    Ok(inspection)
}

#[must_use]
pub fn is_animation_database_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(ANIMATION_DATABASE_EXTENSION)
}

#[must_use]
pub fn is_animation_database_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| is_animation_database_extension(extension))
}

#[must_use]
pub fn is_animation_database_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_animation_database_extension)
}

/// Item produced by the streaming animation-database visitor.
#[derive(Debug, Clone)]
pub enum AnimationDatabaseItem<'a> {
    Database(AnimationDatabaseRef<'a>),
    SubDatabase(SubDatabaseRef<'a>),
    SubDatabaseFragment(FragmentIdRef<'a>),
    FragmentGroup(FragmentGroupRef<'a>),
    Fragment(FragmentRef<'a>),
    Layer(LayerRef),
    LayerBlend(LayerBlend),
    Animation(AnimationEntryRef<'a>),
    Procedural(ProceduralRef<'a>),
    ProceduralParameter(ProceduralParameterRef<'a>),
    FragmentBlend(FragmentBlendRef<'a>),
    FragmentBlendVariant(FragmentBlendVariantRef<'a>),
}

#[derive(Debug, Clone)]
pub struct AnimationDatabaseRef<'a> {
    pub definition: Option<Cow<'a, str>>,
    pub fragment_definition: Option<Cow<'a, str>>,
    pub tag_definition: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone)]
pub struct SubDatabaseRef<'a> {
    pub file: Cow<'a, str>,
    pub tags: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone)]
pub struct FragmentIdRef<'a> {
    pub name: Cow<'a, str>,
}

#[derive(Debug, Clone)]
pub struct FragmentGroupRef<'a> {
    pub name: Cow<'a, str>,
}

#[derive(Debug, Clone)]
pub struct FragmentRef<'a> {
    pub context: FragmentContext,
    pub blend_out_duration: Option<f32>,
    pub tags: Option<Cow<'a, str>>,
    pub fragment_tags: Option<Cow<'a, str>>,
    pub select_time: Option<f32>,
    pub start_time: Option<f32>,
    pub enter_time: Option<f32>,
    pub flags: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentContext {
    FragmentList,
    FragmentBlend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerRef {
    pub kind: LayerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Animation,
    Procedural,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerBlend {
    pub exit_time: Option<f32>,
    pub start_time: Option<f32>,
    pub duration: Option<f32>,
    pub curve_type: Option<i32>,
    pub terminal: Option<bool>,
    pub flags: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnimationEntryRef<'a> {
    pub name: Cow<'a, str>,
    pub flags: Option<Cow<'a, str>>,
    pub speed: Option<f32>,
    pub weight: Option<f32>,
    pub weight_list: Option<i32>,
    pub channels: [Option<f32>; BLEND_CHANNELS],
}

#[derive(Debug, Clone)]
pub struct ProceduralRef<'a> {
    pub ty: Cow<'a, str>,
    pub context_type: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone)]
pub struct ProceduralParameterRef<'a> {
    pub name: Cow<'a, str>,
    pub value: Option<Cow<'a, str>>,
    pub depth: usize,
}

impl ProceduralParameterRef<'_> {
    #[must_use]
    pub const fn has_value(&self) -> bool {
        self.value.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct FragmentBlendRef<'a> {
    pub from: Option<Cow<'a, str>>,
    pub to: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone)]
pub struct FragmentBlendVariantRef<'a> {
    pub from: Option<Cow<'a, str>>,
    pub to: Option<Cow<'a, str>>,
    pub from_fragment: Option<Cow<'a, str>>,
    pub to_fragment: Option<Cow<'a, str>>,
}

/// Parse a Mannequin `.adb` asset with a streaming visitor.
pub fn visit_animation_database<F>(
    bytes: &[u8],
    mut visitor: F,
) -> Result<AnimationDatabaseStats, AnimationDatabaseParseError>
where
    F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
{
    let xml = str::from_utf8(bytes)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut state = AnimationDatabaseState::default();

    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                let element = state.visit_start(&reader, &event, &mut visitor)?;
                state.stack.push(element);
            }
            Event::Empty(event) => {
                let element = state.visit_start(&reader, &event, &mut visitor)?;
                state.visit_end(element);
            }
            Event::End(_) => {
                let element = state.stack.pop().unwrap_or(ElementKind::Unknown);
                state.visit_end(element);
            }
            Event::Eof => break,
            Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::GeneralRef(_) => {}
        }
    }

    if state.stats.databases == 0 {
        return Err(AnimationDatabaseParseError::MissingRoot);
    }

    Ok(state.stats)
}

#[derive(Debug, Default)]
struct AnimationDatabaseState {
    stack: Vec<ElementKind>,
    stats: AnimationDatabaseStats,
    procedural_parameter_depth: usize,
}

impl AnimationDatabaseState {
    fn visit_start<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let parent = self.stack.last().copied();

        let kind = if self.stack.is_empty() {
            self.visit_root(reader, event, visitor)?
        } else {
            match parent {
                Some(ElementKind::Root) => self.visit_root_child(reader, event)?,
                Some(ElementKind::SubDatabases) => {
                    self.visit_sub_databases(reader, event, visitor)?
                }
                Some(ElementKind::SubDatabase) => {
                    self.visit_sub_database_child(reader, event, visitor)?
                }
                Some(ElementKind::FragmentList) => {
                    self.visit_fragment_group(reader, event, visitor)?
                }
                Some(ElementKind::FragmentGroup) => self.visit_fragment(reader, event, visitor)?,
                Some(ElementKind::Fragment) => self.visit_fragment_child(reader, event, visitor)?,
                Some(ElementKind::AnimationLayer) => {
                    self.visit_animation_layer_child(reader, event, visitor)?
                }
                Some(ElementKind::ProceduralLayer) => {
                    self.visit_procedural_layer_child(reader, event, visitor)?
                }
                Some(ElementKind::Procedural) => self.visit_procedural_child(reader, event)?,
                Some(ElementKind::ProceduralParams) | Some(ElementKind::ProceduralParameter) => {
                    self.visit_procedural_parameter(reader, event, visitor)?
                }
                Some(ElementKind::FragmentBlendList) => {
                    self.visit_fragment_blend(reader, event, visitor)?
                }
                Some(ElementKind::FragmentBlend) => {
                    self.visit_fragment_blend_child(reader, event, visitor)?
                }
                Some(ElementKind::FragmentBlendVariant) => {
                    self.visit_transition_fragment(reader, event, visitor)?
                }
                Some(ElementKind::LayerBlend)
                | Some(ElementKind::Animation)
                | Some(ElementKind::SubDatabaseFragment)
                | Some(ElementKind::Unknown) => ElementKind::Unknown,
                None => ElementKind::Unknown,
            }
        };

        Ok(kind)
    }

    fn visit_end(&mut self, element: ElementKind) {
        if matches!(element, ElementKind::ProceduralParameter) {
            self.procedural_parameter_depth = self.procedural_parameter_depth.saturating_sub(1);
        }
    }

    fn visit_root<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(b"<root>", ANIM_DB, name.as_ref())?;
        ensure_known_attributes(reader, event, "AnimDB", &[b"Def", b"FragDef", b"TagDef"])?;

        let database = AnimationDatabaseRef {
            definition: attr_value(reader, event, &[b"Def"])?,
            fragment_definition: attr_value(reader, event, &[b"FragDef"])?,
            tag_definition: attr_value(reader, event, &[b"TagDef"])?,
        };
        if database.definition.is_none()
            && (database.fragment_definition.is_none() || database.tag_definition.is_none())
        {
            return Err(AnimationDatabaseParseError::MissingDatabaseDefinitions);
        }
        self.stats.databases += 1;
        visitor(AnimationDatabaseItem::Database(database))?;
        Ok(ElementKind::Root)
    }

    fn visit_root_child(
        &self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ElementKind, AnimationDatabaseParseError> {
        let name = event.local_name();
        match name.as_ref() {
            SUB_ADBS => {
                ensure_no_attributes(reader, event, "SubADBs")?;
                Ok(ElementKind::SubDatabases)
            }
            FRAGMENT_LIST => {
                ensure_no_attributes(reader, event, "FragmentList")?;
                Ok(ElementKind::FragmentList)
            }
            FRAGMENT_BLEND_LIST => {
                ensure_no_attributes(reader, event, "FragmentBlendList")?;
                Ok(ElementKind::FragmentBlendList)
            }
            _ => Err(AnimationDatabaseParseError::UnexpectedElement {
                parent: "AnimDB".to_string(),
                child: String::from_utf8_lossy(name.as_ref()).into_owned(),
            }),
        }
    }

    fn visit_sub_databases<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(SUB_ADBS, SUB_ADB, name.as_ref())?;
        ensure_known_attributes(reader, event, "SubADB", &[b"File", b"Tags"])?;
        let sub_database = SubDatabaseRef {
            file: attr_required(reader, event, &[b"File"], "File")?,
            tags: attr_value(reader, event, &[b"Tags"])?,
        };
        self.stats.sub_databases += 1;
        visitor(AnimationDatabaseItem::SubDatabase(sub_database))?;
        Ok(ElementKind::SubDatabase)
    }

    fn visit_sub_database_child<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(SUB_ADB, FRAGMENT_ID, name.as_ref())?;
        ensure_known_attributes(reader, event, "FragmentID", &[b"Name"])?;
        let fragment_id = FragmentIdRef {
            name: attr_required(reader, event, &[b"Name"], "Name")?,
        };
        self.stats.sub_database_fragment_filters += 1;
        visitor(AnimationDatabaseItem::SubDatabaseFragment(fragment_id))?;
        Ok(ElementKind::SubDatabaseFragment)
    }

    fn visit_fragment_group<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        ensure_no_attributes(reader, event, "FragmentList entry")?;
        let name = element_name(reader, event)?;
        self.stats.fragment_groups += 1;
        visitor(AnimationDatabaseItem::FragmentGroup(FragmentGroupRef {
            name,
        }))?;
        Ok(ElementKind::FragmentGroup)
    }

    fn visit_fragment<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(b"fragment id", FRAGMENT, name.as_ref())?;
        let fragment = parse_fragment(reader, event, FragmentContext::FragmentList)?;
        self.stats.fragments += 1;
        visitor(AnimationDatabaseItem::Fragment(fragment))?;
        Ok(ElementKind::Fragment)
    }

    fn visit_transition_fragment<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(VARIANT, FRAGMENT, name.as_ref())?;
        let fragment = parse_fragment(reader, event, FragmentContext::FragmentBlend)?;
        self.stats.transition_fragments += 1;
        visitor(AnimationDatabaseItem::Fragment(fragment))?;
        Ok(ElementKind::Fragment)
    }

    fn visit_fragment_child<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        match name.as_ref() {
            ANIM_LAYER => {
                ensure_no_attributes(reader, event, "AnimLayer")?;
                self.stats.anim_layers += 1;
                visitor(AnimationDatabaseItem::Layer(LayerRef {
                    kind: LayerKind::Animation,
                }))?;
                Ok(ElementKind::AnimationLayer)
            }
            PROC_LAYER => {
                ensure_no_attributes(reader, event, "ProcLayer")?;
                self.stats.proc_layers += 1;
                visitor(AnimationDatabaseItem::Layer(LayerRef {
                    kind: LayerKind::Procedural,
                }))?;
                Ok(ElementKind::ProceduralLayer)
            }
            _ => Err(AnimationDatabaseParseError::UnexpectedElement {
                parent: "Fragment".to_string(),
                child: String::from_utf8_lossy(name.as_ref()).into_owned(),
            }),
        }
    }

    fn visit_animation_layer_child<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        match name.as_ref() {
            BLEND => {
                let blend = parse_layer_blend(reader, event)?;
                self.stats.layer_blends += 1;
                visitor(AnimationDatabaseItem::LayerBlend(blend))?;
                Ok(ElementKind::LayerBlend)
            }
            ANIMATION => {
                let animation = parse_animation_entry(reader, event)?;
                self.stats.animations += 1;
                visitor(AnimationDatabaseItem::Animation(animation))?;
                Ok(ElementKind::Animation)
            }
            _ => Err(AnimationDatabaseParseError::UnexpectedElement {
                parent: "AnimLayer".to_string(),
                child: String::from_utf8_lossy(name.as_ref()).into_owned(),
            }),
        }
    }

    fn visit_procedural_layer_child<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        match name.as_ref() {
            BLEND => {
                let blend = parse_layer_blend(reader, event)?;
                self.stats.layer_blends += 1;
                visitor(AnimationDatabaseItem::LayerBlend(blend))?;
                Ok(ElementKind::LayerBlend)
            }
            PROCEDURAL => {
                ensure_known_attributes(reader, event, "Procedural", &[b"type", b"contextType"])?;
                let procedural = ProceduralRef {
                    ty: attr_required(reader, event, &[b"type"], "type")?,
                    context_type: attr_value(reader, event, &[b"contextType"])?,
                };
                self.stats.procedurals += 1;
                visitor(AnimationDatabaseItem::Procedural(procedural))?;
                Ok(ElementKind::Procedural)
            }
            _ => Err(AnimationDatabaseParseError::UnexpectedElement {
                parent: "ProcLayer".to_string(),
                child: String::from_utf8_lossy(name.as_ref()).into_owned(),
            }),
        }
    }

    fn visit_procedural_child(
        &self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ElementKind, AnimationDatabaseParseError> {
        let name = event.local_name();
        ensure_element(PROCEDURAL, PROCEDURAL_PARAMS, name.as_ref())?;
        ensure_no_attributes(reader, event, "ProceduralParams")?;
        Ok(ElementKind::ProceduralParams)
    }

    fn visit_procedural_parameter<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        ensure_known_attributes(reader, event, "Procedural parameter", &[b"value"])?;
        let parameter = ProceduralParameterRef {
            name: element_name(reader, event)?,
            value: attr_value(reader, event, &[b"value"])?,
            depth: self.procedural_parameter_depth,
        };
        if parameter.value.is_some() {
            self.stats.procedural_parameter_values += 1;
        } else {
            self.stats.procedural_parameter_groups += 1;
        }
        self.procedural_parameter_depth += 1;
        visitor(AnimationDatabaseItem::ProceduralParameter(parameter))?;
        Ok(ElementKind::ProceduralParameter)
    }

    fn visit_fragment_blend<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(FRAGMENT_BLEND_LIST, BLEND, name.as_ref())?;
        ensure_known_attributes(reader, event, "Blend", &[b"from", b"to"])?;
        let fragment_blend = FragmentBlendRef {
            from: attr_value(reader, event, &[b"from"])?,
            to: attr_value(reader, event, &[b"to"])?,
        };
        self.stats.fragment_blends += 1;
        visitor(AnimationDatabaseItem::FragmentBlend(fragment_blend))?;
        Ok(ElementKind::FragmentBlend)
    }

    fn visit_fragment_blend_child<F>(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        visitor: &mut F,
    ) -> Result<ElementKind, AnimationDatabaseParseError>
    where
        F: FnMut(AnimationDatabaseItem<'_>) -> Result<(), AnimationDatabaseParseError>,
    {
        let name = event.local_name();
        ensure_element(BLEND, VARIANT, name.as_ref())?;
        ensure_known_attributes(
            reader,
            event,
            "Variant",
            &[b"from", b"to", b"fromFrag", b"toFrag"],
        )?;
        let variant = FragmentBlendVariantRef {
            from: attr_value(reader, event, &[b"from"])?,
            to: attr_value(reader, event, &[b"to"])?,
            from_fragment: attr_value(reader, event, &[b"fromFrag"])?,
            to_fragment: attr_value(reader, event, &[b"toFrag"])?,
        };
        self.stats.fragment_blend_variants += 1;
        visitor(AnimationDatabaseItem::FragmentBlendVariant(variant))?;
        Ok(ElementKind::FragmentBlendVariant)
    }
}

#[derive(Debug, Clone, Copy)]
enum ElementKind {
    Root,
    SubDatabases,
    SubDatabase,
    SubDatabaseFragment,
    FragmentList,
    FragmentGroup,
    Fragment,
    AnimationLayer,
    ProceduralLayer,
    LayerBlend,
    Animation,
    Procedural,
    ProceduralParams,
    ProceduralParameter,
    FragmentBlendList,
    FragmentBlend,
    FragmentBlendVariant,
    Unknown,
}

fn parse_fragment<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
    context: FragmentContext,
) -> Result<FragmentRef<'a>, AnimationDatabaseParseError> {
    ensure_known_attributes(
        reader,
        event,
        "Fragment",
        &[
            b"BlendOutDuration",
            b"Tags",
            b"FragTags",
            b"selectTime",
            b"startTime",
            b"enterTime",
            b"flags",
        ],
    )?;
    Ok(FragmentRef {
        context,
        blend_out_duration: attr_f32(reader, event, &[b"BlendOutDuration"], "BlendOutDuration")?,
        tags: attr_value(reader, event, &[b"Tags"])?,
        fragment_tags: attr_value(reader, event, &[b"FragTags"])?,
        select_time: attr_f32(reader, event, &[b"selectTime"], "selectTime")?,
        start_time: attr_f32(reader, event, &[b"startTime"], "startTime")?,
        enter_time: attr_f32(reader, event, &[b"enterTime"], "enterTime")?,
        flags: attr_value(reader, event, &[b"flags"])?,
    })
}

fn parse_layer_blend(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<LayerBlend, AnimationDatabaseParseError> {
    ensure_known_attributes(
        reader,
        event,
        "Blend",
        &[
            b"ExitTime",
            b"StartTime",
            b"Duration",
            b"CurveType",
            b"terminal",
            b"flags",
        ],
    )?;
    Ok(LayerBlend {
        exit_time: attr_f32(reader, event, &[b"ExitTime"], "ExitTime")?,
        start_time: attr_f32(reader, event, &[b"StartTime"], "StartTime")?,
        duration: attr_f32(reader, event, &[b"Duration"], "Duration")?,
        curve_type: attr_i32(reader, event, &[b"CurveType"], "CurveType")?,
        terminal: attr_bool(reader, event, &[b"terminal"], "terminal")?,
        flags: attr_value(reader, event, &[b"flags"])?.map(Cow::into_owned),
    })
}

fn parse_animation_entry<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
) -> Result<AnimationEntryRef<'a>, AnimationDatabaseParseError> {
    ensure_known_attributes(
        reader,
        event,
        "Animation",
        &[
            b"name",
            b"flags",
            b"speed",
            b"weight",
            b"weightList",
            b"channel0",
            b"channel1",
            b"channel2",
            b"channel3",
        ],
    )?;
    Ok(AnimationEntryRef {
        name: attr_required(reader, event, &[b"name"], "name")?,
        flags: attr_value(reader, event, &[b"flags"])?,
        speed: attr_f32(reader, event, &[b"speed"], "speed")?,
        weight: attr_f32(reader, event, &[b"weight"], "weight")?,
        weight_list: attr_i32(reader, event, &[b"weightList"], "weightList")?,
        channels: [
            attr_f32(reader, event, &[b"channel0"], "channel0")?,
            attr_f32(reader, event, &[b"channel1"], "channel1")?,
            attr_f32(reader, event, &[b"channel2"], "channel2")?,
            attr_f32(reader, event, &[b"channel3"], "channel3")?,
        ],
    })
}

fn element_name<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
) -> Result<Cow<'a, str>, AnimationDatabaseParseError> {
    Ok(reader
        .decoder()
        .decode(event.local_name().into_inner())
        .map_err(quick_xml::Error::from)?)
}

fn attr_required<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
    keys: &[&[u8]],
    name: &'static str,
) -> Result<Cow<'a, str>, AnimationDatabaseParseError> {
    attr_value(reader, event, keys)?.ok_or(AnimationDatabaseParseError::MissingAttribute(name))
}

fn attr_value<'a>(
    reader: &Reader<&[u8]>,
    event: &'a BytesStart<'a>,
    keys: &[&[u8]],
) -> Result<Option<Cow<'a, str>>, AnimationDatabaseParseError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if keys
            .iter()
            .any(|key| attribute.key.as_ref().eq_ignore_ascii_case(key))
        {
            return Ok(Some(attribute.decoded_and_normalized_value(
                quick_xml::XmlVersion::default(),
                reader.decoder(),
            )?));
        }
    }
    Ok(None)
}

fn attr_f32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    keys: &[&[u8]],
    name: &'static str,
) -> Result<Option<f32>, AnimationDatabaseParseError> {
    let Some(value) = attr_value(reader, event, keys)? else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| AnimationDatabaseParseError::InvalidFloat {
            name,
            value: value.into_owned(),
            source,
        })
}

fn attr_i32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    keys: &[&[u8]],
    name: &'static str,
) -> Result<Option<i32>, AnimationDatabaseParseError> {
    let Some(value) = attr_value(reader, event, keys)? else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| AnimationDatabaseParseError::InvalidInteger {
            name,
            value: value.into_owned(),
            source,
        })
}

fn attr_bool(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    keys: &[&[u8]],
    name: &'static str,
) -> Result<Option<bool>, AnimationDatabaseParseError> {
    let Some(value) = attr_value(reader, event, keys)? else {
        return Ok(None);
    };
    match value.trim() {
        "" => Ok(None),
        "0" | "false" | "False" => Ok(Some(false)),
        "1" | "true" | "True" => Ok(Some(true)),
        _ => Err(AnimationDatabaseParseError::InvalidBool {
            name,
            value: value.into_owned(),
        }),
    }
}

fn ensure_element(
    parent: &[u8],
    expected: &[u8],
    actual: &[u8],
) -> Result<(), AnimationDatabaseParseError> {
    if actual == expected {
        Ok(())
    } else {
        Err(AnimationDatabaseParseError::UnexpectedElement {
            parent: String::from_utf8_lossy(parent).into_owned(),
            child: String::from_utf8_lossy(actual).into_owned(),
        })
    }
}

fn ensure_no_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
) -> Result<(), AnimationDatabaseParseError> {
    ensure_known_attributes(reader, event, element, &[])
}

fn ensure_known_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
    allowed: &[&[u8]],
) -> Result<(), AnimationDatabaseParseError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if !allowed
            .iter()
            .any(|key| attribute.key.as_ref().eq_ignore_ascii_case(key))
        {
            return Err(AnimationDatabaseParseError::UnexpectedAttribute {
                element,
                attribute: reader
                    .decoder()
                    .decode(attribute.key.as_ref())
                    .map_err(quick_xml::Error::from)?
                    .into_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum AnimationDatabaseParseError {
    #[error("expected AnimDB root")]
    MissingRoot,
    #[error("AnimDB must provide Def or both FragDef and TagDef")]
    MissingDatabaseDefinitions,
    #[error("missing `{0}` attribute")]
    MissingAttribute(&'static str),
    #[error("unexpected element `{child}` under `{parent}`")]
    UnexpectedElement { parent: String, child: String },
    #[error("unexpected attribute `{attribute}` on `{element}`")]
    UnexpectedAttribute {
        element: &'static str,
        attribute: String,
    },
    #[error("invalid float `{value}` in `{name}`")]
    InvalidFloat {
        name: &'static str,
        value: String,
        #[source]
        source: ParseFloatError,
    },
    #[error("invalid integer `{value}` in `{name}`")]
    InvalidInteger {
        name: &'static str,
        value: String,
        #[source]
        source: ParseIntError,
    },
    #[error("invalid bool `{value}` in `{name}`")]
    InvalidBool { name: &'static str, value: String },
    #[error("xml parse error")]
    Xml(#[from] quick_xml::Error),
    #[error("xml attribute error")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),
    #[error("asset is not utf-8")]
    Utf8(#[from] str::Utf8Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_animation_database() {
        let xml = br#"
            <AnimDB FragDef="actions.xml" TagDef="tags.xml">
              <SubADBs>
                <SubADB Tags="Elite" File="sub.adb">
                  <FragmentID Name="Attack"/>
                </SubADB>
              </SubADBs>
              <FragmentList>
                <Attack>
                  <Fragment BlendOutDuration="0.2" Tags="Elite" FragTags="Heavy">
                    <AnimLayer>
                      <Blend ExitTime="0" StartTime="0" Duration="0.1" CurveType="0"/>
                      <Animation name="attack_a" flags="Loop" speed="1.25" channel2="0.5"/>
                    </AnimLayer>
                    <ProcLayer>
                      <Blend ExitTime="0" StartTime="0" Duration="0" CurveType="0"/>
                      <Procedural type="Spawn" contextType="">
                        <ProceduralParams>
                          <PosOffset>
                            <Element value="1"/>
                            <Element value="2"/>
                            <Element value="3"/>
                          </PosOffset>
                          <EffectName value="fx"/>
                        </ProceduralParams>
                      </Procedural>
                    </ProcLayer>
                  </Fragment>
                </Attack>
              </FragmentList>
              <FragmentBlendList>
                <Blend from="Idle" to="Attack">
                  <Variant from="" to="Elite" fromFrag="Light" toFrag="Heavy">
                    <Fragment selectTime="0" enterTime="0.1">
                      <AnimLayer>
                        <Blend ExitTime="0" StartTime="0" Duration="0.2" CurveType="0"/>
                      </AnimLayer>
                    </Fragment>
                  </Variant>
                </Blend>
              </FragmentBlendList>
            </AnimDB>
        "#;

        let mut groups = Vec::new();
        let mut parameters = Vec::new();
        let stats = visit_animation_database(xml, |item| {
            match item {
                AnimationDatabaseItem::FragmentGroup(group) => {
                    groups.push(group.name.into_owned());
                }
                AnimationDatabaseItem::ProceduralParameter(parameter) => {
                    parameters.push((parameter.name.into_owned(), parameter.depth));
                }
                _ => {}
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.databases, 1);
        assert_eq!(stats.sub_databases, 1);
        assert_eq!(stats.sub_database_fragment_filters, 1);
        assert_eq!(stats.fragment_groups, 1);
        assert_eq!(stats.fragments, 1);
        assert_eq!(stats.transition_fragments, 1);
        assert_eq!(stats.anim_layers, 2);
        assert_eq!(stats.proc_layers, 1);
        assert_eq!(stats.layer_blends, 3);
        assert_eq!(stats.animations, 1);
        assert_eq!(stats.procedurals, 1);
        assert_eq!(stats.procedural_parameter_groups, 1);
        assert_eq!(stats.procedural_parameter_values, 4);
        assert_eq!(stats.fragment_blends, 1);
        assert_eq!(stats.fragment_blend_variants, 1);
        assert_eq!(summarize_animation_database(xml).unwrap(), stats);

        let mut totals = AnimationDatabaseTotals::default();
        totals.add_stats(stats);
        assert_eq!(totals.files, 1);
        assert_eq!(totals.animations, 1);
        assert_eq!(
            stats.to_string(),
            "1 fragments, 1 transitions, 1 animations, 1 procedurals"
        );
        assert_eq!(
            totals.to_string(),
            "  files: 1\n  databases: 1\n  sub databases: 1\n  sub database fragment filters: 1\n  fragment groups: 1\n  fragments: 1\n  transition fragments: 1\n  animation layers: 2\n  procedural layers: 1\n  layer blends: 3\n  animations: 1\n  procedurals: 1\n  procedural parameter groups: 1\n  procedural parameter values: 4\n  fragment blends: 1\n  fragment blend variants: 1\n"
        );
        let row = inspect_animation_database_file("animations/combat.adb", xml).unwrap();
        let mut inspection = AnimationDatabaseInspection::default();
        inspection.add_file_summary(row);
        assert_eq!(
            inspection.report(20).to_string(),
            "animations/combat.adb: 1 fragments, 1 transitions, 1 animations, 1 procedurals\n  files: 1\n  databases: 1\n  sub databases: 1\n  sub database fragment filters: 1\n  fragment groups: 1\n  fragments: 1\n  transition fragments: 1\n  animation layers: 2\n  procedural layers: 1\n  layer blends: 3\n  animations: 1\n  procedurals: 1\n  procedural parameter groups: 1\n  procedural parameter values: 4\n  fragment blends: 1\n  fragment blend variants: 1\n"
        );

        let path = std::env::temp_dir().join(format!(
            "az-rs-cry-mannequin-{}-combat.adb",
            std::process::id()
        ));
        std::fs::write(&path, xml).expect("write animation database");
        let inspection =
            inspect_animation_database_files([&path]).expect("inspect animation databases");
        assert_eq!(inspection.rows.len(), 1);
        assert_eq!(inspection.totals.files, 1);
        assert_eq!(inspection.totals.fragments, 1);
        assert_eq!(inspection.totals.animations, 1);
        std::fs::remove_file(path).expect("remove animation database");

        assert!(is_animation_database_name("animation.ADB"));
        assert_eq!(groups, ["Attack"]);
        assert_eq!(
            parameters,
            [
                ("PosOffset".to_string(), 0),
                ("Element".to_string(), 1),
                ("Element".to_string(), 1),
                ("Element".to_string(), 1),
                ("EffectName".to_string(), 0),
            ]
        );
    }

    #[test]
    fn accepts_controller_definition_database() {
        let xml = br#"
            <AnimDB Def="controller.xml">
              <FragmentList/>
            </AnimDB>
        "#;
        let stats = visit_animation_database(xml, |_| Ok(())).unwrap();
        assert_eq!(stats.databases, 1);
    }

    #[test]
    fn rejects_unmapped_animation_database_attributes() {
        let xml = br#"
            <AnimDB FragDef="actions.xml" TagDef="tags.xml" mystery="x">
              <FragmentList/>
            </AnimDB>
        "#;
        let error = visit_animation_database(xml, |_| Ok(())).unwrap_err();
        assert!(matches!(
            error,
            AnimationDatabaseParseError::UnexpectedAttribute {
                element: "AnimDB",
                attribute
            } if attribute == "mystery"
        ));
    }
}
