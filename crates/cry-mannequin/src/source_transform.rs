//! Legacy CryAction Mannequin XML import transform.

use std::{num::ParseIntError, str};

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AnimationDatabaseItem, AnimationDatabaseParseError, AnimationEntryRef, FragmentBlendVariantRef,
    FragmentContext, FragmentRef, LayerBlend, LayerKind, LayerRef, ProceduralParameterRef,
    ProceduralRef, SubDatabaseRef, visit_animation_database,
};

pub type MannequinSourceSchema = &'static str;

pub const MANNEQUIN_ACTIONS_SOURCE_SCHEMA: MannequinSourceSchema =
    "azoth.compat.cry.MannequinActionsSource";
pub const MANNEQUIN_ANIMATION_DATABASE_SOURCE_SCHEMA: MannequinSourceSchema =
    "azoth.compat.cry.MannequinAnimationDatabaseSource";
pub const MANNEQUIN_TAGS_SOURCE_SCHEMA: MannequinSourceSchema =
    "azoth.compat.cry.MannequinTagsSource";
pub const MANNEQUIN_CONTROLLER_SOURCE_SCHEMA: MannequinSourceSchema =
    "azoth.compat.cry.MannequinControllerDefinitionSource";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MannequinXmlKind {
    AnimationDatabase,
    Actions,
    Tags,
    ControllerDefinition,
}

impl MannequinXmlKind {
    #[must_use]
    pub fn from_source_path(source_path: &str) -> Option<Self> {
        let normalized = normalize_source_path(source_path);
        if normalized.ends_with(".adb") {
            return Some(Self::AnimationDatabase);
        }
        if !normalized.contains("/mannequin/") {
            return None;
        }

        let name = normalized.rsplit('/').next().unwrap_or(&normalized);
        if name.ends_with("controllerdefs.xml") {
            Some(Self::ControllerDefinition)
        } else if name.ends_with("actions.xml") {
            Some(Self::Actions)
        } else if name.ends_with("tags.xml") {
            Some(Self::Tags)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn source_schema(self) -> MannequinSourceSchema {
        match self {
            Self::AnimationDatabase => MANNEQUIN_ANIMATION_DATABASE_SOURCE_SCHEMA,
            Self::Actions => MANNEQUIN_ACTIONS_SOURCE_SCHEMA,
            Self::Tags => MANNEQUIN_TAGS_SOURCE_SCHEMA,
            Self::ControllerDefinition => MANNEQUIN_CONTROLLER_SOURCE_SCHEMA,
        }
    }

    #[must_use]
    pub const fn source_suffix(self) -> &'static str {
        match self {
            Self::AnimationDatabase => "adb.ron",
            Self::Actions => "mannequin.actions.ron",
            Self::Tags => "mannequin.tags.ron",
            Self::ControllerDefinition => "mannequin.controller.ron",
        }
    }

    #[must_use]
    pub fn source_path(self, source_path: &str) -> String {
        let normalized = normalize_source_path(source_path);
        let stem = normalized
            .strip_suffix(".xml")
            .or_else(|| normalized.strip_suffix(".adb"))
            .unwrap_or(&normalized);
        format!("{stem}.{}", self.source_suffix())
    }
}

pub type MannequinActionsSource = MannequinTagDefinitionSource;
pub type MannequinTagsSource = MannequinTagDefinitionSource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAnimationDatabaseSource {
    pub source_path: String,
    pub database: MannequinAnimationDatabase,
}

impl MannequinAnimationDatabaseSource {
    pub fn from_legacy(
        source_path: &str,
        bytes: &[u8],
    ) -> Result<Self, MannequinAnimationDatabaseSourceError> {
        Self::from_legacy_with_motion_resolver(source_path, bytes, |_| None)
    }

    pub fn from_legacy_with_motion_resolver<F>(
        source_path: &str,
        bytes: &[u8],
        resolver: F,
    ) -> Result<Self, MannequinAnimationDatabaseSourceError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut builder =
            MannequinAnimationDatabaseBuilder::new(normalize_source_path(source_path));
        let mut build_error = None;

        visit_animation_database(bytes, |item| {
            if build_error.is_none()
                && let Err(error) = builder.visit(item)
            {
                build_error = Some(error);
            }
            Ok(())
        })?;

        if let Some(error) = build_error {
            return Err(error);
        }

        let mut source = builder.finish()?;
        source.resolve_animation_references(resolver);
        Ok(source)
    }

    pub fn to_ron_bytes(&self) -> Result<Vec<u8>, ron::Error> {
        let ron = ron::ser::to_string_pretty(self, PrettyConfig::default())?;
        Ok(format!("{ron}\n").into_bytes())
    }

    pub fn from_ron_bytes(bytes: &[u8]) -> Result<Self, ron::error::SpannedError> {
        ron::de::from_bytes(bytes)
    }

    pub fn resolve_animation_references<F>(&mut self, mut resolver: F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        self.database.resolve_animation_references(&mut resolver);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAnimationDatabase {
    pub definition: Option<String>,
    pub fragment_definition: Option<String>,
    pub tag_definition: Option<String>,
    pub sub_databases: Vec<MannequinSubDatabase>,
    pub fragment_groups: Vec<MannequinFragmentGroup>,
    pub fragment_blends: Vec<MannequinFragmentBlend>,
}

impl MannequinAnimationDatabase {
    fn resolve_animation_references<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        for group in &mut self.fragment_groups {
            for fragment in &mut group.fragments {
                fragment.resolve_animation_references(resolver);
            }
        }
        for blend in &mut self.fragment_blends {
            for variant in &mut blend.variants {
                for fragment in &mut variant.fragments {
                    fragment.resolve_animation_references(resolver);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinSubDatabase {
    pub file: String,
    pub tags: Option<String>,
    pub fragment_filters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinFragmentGroup {
    pub name: String,
    pub fragments: Vec<MannequinFragment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinFragment {
    pub blend_out_duration: Option<f32>,
    pub tags: Option<String>,
    pub fragment_tags: Option<String>,
    pub select_time: Option<f32>,
    pub start_time: Option<f32>,
    pub enter_time: Option<f32>,
    pub flags: Option<String>,
    pub animation_layers: Vec<MannequinAnimationLayer>,
    pub procedural_layers: Vec<MannequinProceduralLayer>,
}

impl MannequinFragment {
    fn resolve_animation_references<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        for layer in &mut self.animation_layers {
            for animation in &mut layer.animations {
                animation.resolve_motion_path(resolver);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAnimationLayer {
    pub blends: Vec<MannequinLayerBlend>,
    pub animations: Vec<MannequinAnimationEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinProceduralLayer {
    pub blends: Vec<MannequinLayerBlend>,
    pub procedurals: Vec<MannequinProcedural>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinLayerBlend {
    pub exit_time: Option<f32>,
    pub start_time: Option<f32>,
    pub duration: Option<f32>,
    pub curve_type: Option<i32>,
    pub terminal: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAnimationEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_motion_reason: Option<String>,
    pub flags: Option<String>,
    pub speed: Option<f32>,
    pub weight: Option<f32>,
    pub weight_list: Option<i32>,
    pub channels: [Option<f32>; crate::BLEND_CHANNELS],
}

impl MannequinAnimationEntry {
    fn resolve_motion_path<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        if self.name.is_empty() {
            self.motion_path = None;
            self.unresolved_motion_reason = None;
            return;
        }

        self.motion_path =
            resolver(&self.name).or_else(|| motion_path_from_animation_reference(&self.name));
        self.unresolved_motion_reason = self.motion_path.is_none().then(|| {
            "animation set name could not be resolved to a CAF/.anim.glb path".to_string()
        });
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinProcedural {
    pub ty: String,
    pub context_type: Option<String>,
    pub parameters: Vec<MannequinProceduralParameter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinProceduralParameter {
    pub name: String,
    pub value: Option<String>,
    pub children: Vec<MannequinProceduralParameter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinFragmentBlend {
    pub from: Option<String>,
    pub to: Option<String>,
    pub variants: Vec<MannequinFragmentBlendVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinFragmentBlendVariant {
    pub from: Option<String>,
    pub to: Option<String>,
    pub from_fragment: Option<String>,
    pub to_fragment: Option<String>,
    pub fragments: Vec<MannequinFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinTagDefinitionSource {
    pub source_path: String,
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    pub entries: Vec<MannequinTagDefinitionEntry>,
}

impl MannequinTagDefinitionSource {
    pub fn from_legacy(source_path: &str, bytes: &[u8]) -> Result<Self, MannequinXmlParseError> {
        parse_tag_definition_source(normalize_source_path(source_path), bytes)
    }

    pub fn to_ron_bytes(&self) -> Result<Vec<u8>, ron::Error> {
        let ron = ron::ser::to_string_pretty(self, PrettyConfig::default())?;
        Ok(format!("{ron}\n").into_bytes())
    }

    pub fn from_ron_bytes(bytes: &[u8]) -> Result<Self, ron::error::SpannedError> {
        ron::de::from_bytes(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MannequinTagDefinitionEntry {
    Tag(MannequinTagEntry),
    Group(MannequinTagGroup),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinTagGroup {
    pub name: String,
    pub tags: Vec<MannequinTagEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinTagEntry {
    pub name: String,
    pub priority: Option<i32>,
    pub sub_tag_definition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinControllerDefinitionSource {
    pub source_path: String,
    pub tags: MannequinFileReference,
    pub fragments: MannequinFileReference,
    pub fragment_definitions: Vec<MannequinFragmentDefinition>,
    pub scope_contexts: Vec<MannequinScopeContext>,
    pub scopes: Vec<MannequinScopeDefinition>,
}

impl MannequinControllerDefinitionSource {
    pub fn from_legacy(source_path: &str, bytes: &[u8]) -> Result<Self, MannequinXmlParseError> {
        parse_controller_definition_source(normalize_source_path(source_path), bytes)
    }

    pub fn to_ron_bytes(&self) -> Result<Vec<u8>, ron::Error> {
        let ron = ron::ser::to_string_pretty(self, PrettyConfig::default())?;
        Ok(format!("{ron}\n").into_bytes())
    }

    pub fn from_ron_bytes(bytes: &[u8]) -> Result<Self, ron::error::SpannedError> {
        ron::de::from_bytes(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinFileReference {
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinFragmentDefinition {
    pub name: String,
    pub scopes: String,
    pub flags: Option<String>,
    pub overrides: Vec<MannequinFragmentOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinFragmentOverride {
    pub tags: String,
    pub scopes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinScopeContext {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MannequinScopeDefinition {
    pub name: String,
    pub layer: i32,
    pub num_layers: i32,
    pub context: String,
    /// Legacy Mannequin spells this scope filter attribute as `Tags`.
    pub tags: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MannequinSourceTransform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MannequinSourceInput<'a> {
    pub source_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MannequinSourceArtifact {
    pub path: String,
    pub schema: MannequinSourceSchema,
    pub bytes: Vec<u8>,
}

impl MannequinSourceTransform {
    pub fn transform(
        &self,
        input: MannequinSourceInput<'_>,
    ) -> Result<MannequinSourceArtifact, MannequinSourceTransformError> {
        self.transform_with_motion_resolver(input, |_| None)
    }

    pub fn transform_with_motion_resolver<F>(
        &self,
        input: MannequinSourceInput<'_>,
        resolver: F,
    ) -> Result<MannequinSourceArtifact, MannequinSourceTransformError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let kind = MannequinXmlKind::from_source_path(input.source_path).ok_or_else(|| {
            MannequinSourceTransformError::UnsupportedPath {
                path: normalize_source_path(input.source_path),
            }
        })?;

        let bytes = match kind {
            MannequinXmlKind::AnimationDatabase => {
                MannequinAnimationDatabaseSource::from_legacy_with_motion_resolver(
                    input.source_path,
                    input.bytes,
                    resolver,
                )?
                .to_ron_bytes()?
            }
            MannequinXmlKind::Actions | MannequinXmlKind::Tags => {
                MannequinTagDefinitionSource::from_legacy(input.source_path, input.bytes)?
                    .to_ron_bytes()?
            }
            MannequinXmlKind::ControllerDefinition => {
                MannequinControllerDefinitionSource::from_legacy(input.source_path, input.bytes)?
                    .to_ron_bytes()?
            }
        };

        Ok(MannequinSourceArtifact {
            path: kind.source_path(input.source_path),
            schema: kind.source_schema(),
            bytes,
        })
    }
}

#[must_use]
pub fn is_legacy_mannequin_source(source_path: &str) -> bool {
    MannequinXmlKind::from_source_path(source_path).is_some()
}

#[must_use]
pub fn mannequin_source_path(source_path: &str) -> Option<String> {
    MannequinXmlKind::from_source_path(source_path).map(|kind| kind.source_path(source_path))
}

#[must_use]
pub fn normalize_source_path(source_path: &str) -> String {
    source_path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

#[derive(Debug)]
struct MannequinAnimationDatabaseBuilder {
    source_path: String,
    database: Option<MannequinAnimationDatabase>,
    current_sub_database: Option<usize>,
    current_fragment_group: Option<usize>,
    current_fragment: Option<FragmentTarget>,
    current_layer: Option<LayerTarget>,
    current_procedural: Option<ProceduralTarget>,
    current_parameter_path: Vec<usize>,
    current_fragment_blend: Option<usize>,
    current_fragment_blend_variant: Option<usize>,
}

impl MannequinAnimationDatabaseBuilder {
    fn new(source_path: String) -> Self {
        Self {
            source_path,
            database: None,
            current_sub_database: None,
            current_fragment_group: None,
            current_fragment: None,
            current_layer: None,
            current_procedural: None,
            current_parameter_path: Vec::new(),
            current_fragment_blend: None,
            current_fragment_blend_variant: None,
        }
    }

    fn visit(
        &mut self,
        item: AnimationDatabaseItem<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        match item {
            AnimationDatabaseItem::Database(database) => {
                self.database = Some(MannequinAnimationDatabase {
                    definition: database.definition.map(|value| value.into_owned()),
                    fragment_definition: database
                        .fragment_definition
                        .map(|value| value.into_owned()),
                    tag_definition: database.tag_definition.map(|value| value.into_owned()),
                    sub_databases: Vec::new(),
                    fragment_groups: Vec::new(),
                    fragment_blends: Vec::new(),
                });
            }
            AnimationDatabaseItem::SubDatabase(sub_database) => {
                self.push_sub_database(sub_database)?;
            }
            AnimationDatabaseItem::SubDatabaseFragment(fragment) => {
                let index = self.current_sub_database.ok_or(
                    MannequinAnimationDatabaseSourceError::Structure(
                        "FragmentID appeared before SubADB",
                    ),
                )?;
                self.database_mut()?.sub_databases[index]
                    .fragment_filters
                    .push(fragment.name.into_owned());
            }
            AnimationDatabaseItem::FragmentGroup(group) => {
                let database = self.database_mut()?;
                database.fragment_groups.push(MannequinFragmentGroup {
                    name: group.name.into_owned(),
                    fragments: Vec::new(),
                });
                self.current_fragment_group = Some(database.fragment_groups.len() - 1);
                self.current_fragment = None;
                self.current_layer = None;
                self.current_procedural = None;
                self.current_parameter_path.clear();
            }
            AnimationDatabaseItem::Fragment(fragment) => {
                self.push_fragment(fragment)?;
            }
            AnimationDatabaseItem::Layer(layer) => {
                self.push_layer(layer)?;
            }
            AnimationDatabaseItem::LayerBlend(blend) => {
                self.push_layer_blend(blend)?;
            }
            AnimationDatabaseItem::Animation(animation) => {
                self.push_animation(animation)?;
            }
            AnimationDatabaseItem::Procedural(procedural) => {
                self.push_procedural(procedural)?;
            }
            AnimationDatabaseItem::ProceduralParameter(parameter) => {
                self.push_procedural_parameter(parameter)?;
            }
            AnimationDatabaseItem::FragmentBlend(blend) => {
                let database = self.database_mut()?;
                database.fragment_blends.push(MannequinFragmentBlend {
                    from: blend.from.map(|value| value.into_owned()),
                    to: blend.to.map(|value| value.into_owned()),
                    variants: Vec::new(),
                });
                self.current_fragment_blend = Some(database.fragment_blends.len() - 1);
                self.current_fragment_blend_variant = None;
            }
            AnimationDatabaseItem::FragmentBlendVariant(variant) => {
                self.push_fragment_blend_variant(variant)?;
            }
        }
        Ok(())
    }

    fn finish(
        self,
    ) -> Result<MannequinAnimationDatabaseSource, MannequinAnimationDatabaseSourceError> {
        Ok(MannequinAnimationDatabaseSource {
            source_path: self.source_path,
            database: self
                .database
                .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                    "AnimDB root was not visited",
                ))?,
        })
    }

    fn push_sub_database(
        &mut self,
        sub_database: SubDatabaseRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let database = self.database_mut()?;
        database.sub_databases.push(MannequinSubDatabase {
            file: sub_database.file.into_owned(),
            tags: sub_database.tags.map(|value| value.into_owned()),
            fragment_filters: Vec::new(),
        });
        self.current_sub_database = Some(database.sub_databases.len() - 1);
        Ok(())
    }

    fn push_fragment(
        &mut self,
        fragment: FragmentRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let context = fragment.context;
        let fragment = MannequinFragment::from(fragment);
        match context {
            FragmentContext::FragmentList => {
                let group_index = self.current_fragment_group.ok_or(
                    MannequinAnimationDatabaseSourceError::Structure(
                        "Fragment appeared before a FragmentList group",
                    ),
                )?;
                let database = self.database_mut()?;
                database.fragment_groups[group_index]
                    .fragments
                    .push(fragment);
                self.current_fragment = Some(FragmentTarget::Group {
                    group: group_index,
                    fragment: database.fragment_groups[group_index].fragments.len() - 1,
                });
            }
            FragmentContext::FragmentBlend => {
                let blend = self.current_fragment_blend.ok_or(
                    MannequinAnimationDatabaseSourceError::Structure(
                        "transition Fragment appeared before Blend",
                    ),
                )?;
                let variant = self.current_fragment_blend_variant.ok_or(
                    MannequinAnimationDatabaseSourceError::Structure(
                        "transition Fragment appeared before Variant",
                    ),
                )?;
                let database = self.database_mut()?;
                database.fragment_blends[blend].variants[variant]
                    .fragments
                    .push(fragment);
                self.current_fragment = Some(FragmentTarget::Transition {
                    blend,
                    variant,
                    fragment: database.fragment_blends[blend].variants[variant]
                        .fragments
                        .len()
                        - 1,
                });
            }
        }
        self.current_layer = None;
        self.current_procedural = None;
        self.current_parameter_path.clear();
        Ok(())
    }

    fn push_layer(&mut self, layer: LayerRef) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let target =
            self.current_fragment
                .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                    "layer appeared before Fragment",
                ))?;
        let fragment = self.fragment_mut(target)?;
        match layer.kind {
            LayerKind::Animation => {
                fragment.animation_layers.push(MannequinAnimationLayer {
                    blends: Vec::new(),
                    animations: Vec::new(),
                });
                self.current_layer = Some(LayerTarget::Animation {
                    fragment: target,
                    layer: fragment.animation_layers.len() - 1,
                });
            }
            LayerKind::Procedural => {
                fragment.procedural_layers.push(MannequinProceduralLayer {
                    blends: Vec::new(),
                    procedurals: Vec::new(),
                });
                self.current_layer = Some(LayerTarget::Procedural {
                    fragment: target,
                    layer: fragment.procedural_layers.len() - 1,
                });
            }
        }
        self.current_procedural = None;
        self.current_parameter_path.clear();
        Ok(())
    }

    fn push_layer_blend(
        &mut self,
        blend: LayerBlend,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let layer = self
            .current_layer
            .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                "Blend appeared before layer",
            ))?;
        match layer {
            LayerTarget::Animation { fragment, layer } => {
                self.fragment_mut(fragment)?.animation_layers[layer]
                    .blends
                    .push(MannequinLayerBlend::from(blend));
            }
            LayerTarget::Procedural { fragment, layer } => {
                self.fragment_mut(fragment)?.procedural_layers[layer]
                    .blends
                    .push(MannequinLayerBlend::from(blend));
            }
        }
        Ok(())
    }

    fn push_animation(
        &mut self,
        animation: AnimationEntryRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let Some(LayerTarget::Animation { fragment, layer }) = self.current_layer else {
            return Err(MannequinAnimationDatabaseSourceError::Structure(
                "Animation appeared before AnimLayer",
            ));
        };
        self.fragment_mut(fragment)?.animation_layers[layer]
            .animations
            .push(MannequinAnimationEntry::from(animation));
        Ok(())
    }

    fn push_procedural(
        &mut self,
        procedural: ProceduralRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let Some(LayerTarget::Procedural { fragment, layer }) = self.current_layer else {
            return Err(MannequinAnimationDatabaseSourceError::Structure(
                "Procedural appeared before ProcLayer",
            ));
        };
        let procedurals = &mut self.fragment_mut(fragment)?.procedural_layers[layer].procedurals;
        procedurals.push(MannequinProcedural {
            ty: procedural.ty.into_owned(),
            context_type: procedural.context_type.map(|value| value.into_owned()),
            parameters: Vec::new(),
        });
        self.current_procedural = Some(ProceduralTarget {
            fragment,
            layer,
            procedural: procedurals.len() - 1,
        });
        self.current_parameter_path.clear();
        Ok(())
    }

    fn push_procedural_parameter(
        &mut self,
        parameter: ProceduralParameterRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let depth = parameter.depth;
        if self.current_parameter_path.len() < depth {
            return Err(MannequinAnimationDatabaseSourceError::Structure(
                "procedural parameter depth skipped a parent",
            ));
        }
        self.current_parameter_path.truncate(depth);
        let parent_path = self.current_parameter_path.clone();

        let new_index = {
            let parameters = self.current_procedural_parameters_mut()?;
            let children = parameter_children_mut(parameters, &parent_path)?;
            children.push(MannequinProceduralParameter {
                name: parameter.name.into_owned(),
                value: parameter.value.map(|value| value.into_owned()),
                children: Vec::new(),
            });
            children.len() - 1
        };
        self.current_parameter_path.push(new_index);
        Ok(())
    }

    fn push_fragment_blend_variant(
        &mut self,
        variant: FragmentBlendVariantRef<'_>,
    ) -> Result<(), MannequinAnimationDatabaseSourceError> {
        let blend =
            self.current_fragment_blend
                .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                    "Variant appeared before Blend",
                ))?;
        let database = self.database_mut()?;
        database.fragment_blends[blend]
            .variants
            .push(MannequinFragmentBlendVariant {
                from: variant.from.map(|value| value.into_owned()),
                to: variant.to.map(|value| value.into_owned()),
                from_fragment: variant.from_fragment.map(|value| value.into_owned()),
                to_fragment: variant.to_fragment.map(|value| value.into_owned()),
                fragments: Vec::new(),
            });
        self.current_fragment_blend_variant =
            Some(database.fragment_blends[blend].variants.len() - 1);
        Ok(())
    }

    fn database_mut(
        &mut self,
    ) -> Result<&mut MannequinAnimationDatabase, MannequinAnimationDatabaseSourceError> {
        self.database
            .as_mut()
            .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                "Mannequin item appeared before AnimDB root",
            ))
    }

    fn fragment_mut(
        &mut self,
        target: FragmentTarget,
    ) -> Result<&mut MannequinFragment, MannequinAnimationDatabaseSourceError> {
        let database = self.database_mut()?;
        match target {
            FragmentTarget::Group { group, fragment } => {
                Ok(&mut database.fragment_groups[group].fragments[fragment])
            }
            FragmentTarget::Transition {
                blend,
                variant,
                fragment,
            } => Ok(&mut database.fragment_blends[blend].variants[variant].fragments[fragment]),
        }
    }

    fn current_procedural_parameters_mut(
        &mut self,
    ) -> Result<&mut Vec<MannequinProceduralParameter>, MannequinAnimationDatabaseSourceError> {
        let target =
            self.current_procedural
                .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                    "ProceduralParams appeared before Procedural",
                ))?;
        Ok(
            &mut self.fragment_mut(target.fragment)?.procedural_layers[target.layer].procedurals
                [target.procedural]
                .parameters,
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum FragmentTarget {
    Group {
        group: usize,
        fragment: usize,
    },
    Transition {
        blend: usize,
        variant: usize,
        fragment: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum LayerTarget {
    Animation {
        fragment: FragmentTarget,
        layer: usize,
    },
    Procedural {
        fragment: FragmentTarget,
        layer: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct ProceduralTarget {
    fragment: FragmentTarget,
    layer: usize,
    procedural: usize,
}

fn parameter_children_mut<'a>(
    parameters: &'a mut Vec<MannequinProceduralParameter>,
    path: &[usize],
) -> Result<&'a mut Vec<MannequinProceduralParameter>, MannequinAnimationDatabaseSourceError> {
    if let Some((&index, rest)) = path.split_first() {
        let parameter =
            parameters
                .get_mut(index)
                .ok_or(MannequinAnimationDatabaseSourceError::Structure(
                    "procedural parameter parent index was missing",
                ))?;
        parameter_children_mut(&mut parameter.children, rest)
    } else {
        Ok(parameters)
    }
}

impl From<FragmentRef<'_>> for MannequinFragment {
    fn from(fragment: FragmentRef<'_>) -> Self {
        Self {
            blend_out_duration: fragment.blend_out_duration,
            tags: fragment.tags.map(|value| value.into_owned()),
            fragment_tags: fragment.fragment_tags.map(|value| value.into_owned()),
            select_time: fragment.select_time,
            start_time: fragment.start_time,
            enter_time: fragment.enter_time,
            flags: fragment.flags.map(|value| value.into_owned()),
            animation_layers: Vec::new(),
            procedural_layers: Vec::new(),
        }
    }
}

impl From<LayerBlend> for MannequinLayerBlend {
    fn from(blend: LayerBlend) -> Self {
        Self {
            exit_time: blend.exit_time,
            start_time: blend.start_time,
            duration: blend.duration,
            curve_type: blend.curve_type,
            terminal: blend.terminal,
        }
    }
}

impl From<AnimationEntryRef<'_>> for MannequinAnimationEntry {
    fn from(animation: AnimationEntryRef<'_>) -> Self {
        Self {
            name: animation.name.into_owned(),
            motion_path: None,
            unresolved_motion_reason: None,
            flags: animation.flags.map(|value| value.into_owned()),
            speed: animation.speed,
            weight: animation.weight,
            weight_list: animation.weight_list,
            channels: animation.channels,
        }
    }
}

#[must_use]
pub fn motion_path_from_animation_reference(animation_name: &str) -> Option<String> {
    let normalized = normalize_source_path(animation_name);
    if normalized.is_empty() {
        return None;
    }
    let stem = normalized
        .strip_suffix(".i_caf")
        .or_else(|| normalized.strip_suffix(".caf"));
    let stem = stem.or_else(|| normalized.contains('/').then_some(normalized.as_str()))?;
    let path = format!("{stem}.anim.glb");
    if path.starts_with("animations/") {
        Some(path)
    } else {
        Some(format!("animations/{path}"))
    }
}

fn parse_tag_definition_source(
    source_path: String,
    bytes: &[u8],
) -> Result<MannequinTagDefinitionSource, MannequinXmlParseError> {
    let xml = str::from_utf8(bytes)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut state = TagDefinitionParseState::new(source_path);
    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                let element = state.visit_start(&reader, &event, false)?;
                state.stack.push(element);
            }
            Event::Empty(event) => {
                let element = state.visit_start(&reader, &event, true)?;
                state.visit_end(element);
            }
            Event::End(_) => {
                let element = state.stack.pop().unwrap_or(TagDefinitionElement::Unknown);
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
    state.finish()
}

#[derive(Debug)]
struct TagDefinitionParseState {
    source_path: String,
    version: Option<String>,
    imports: Vec<String>,
    entries: Vec<MannequinTagDefinitionEntry>,
    stack: Vec<TagDefinitionElement>,
    current_group: Option<MannequinTagGroup>,
    root_seen: bool,
}

impl TagDefinitionParseState {
    fn new(source_path: String) -> Self {
        Self {
            source_path,
            version: None,
            imports: Vec::new(),
            entries: Vec::new(),
            stack: Vec::new(),
            current_group: None,
            root_seen: false,
        }
    }

    fn visit_start(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        empty: bool,
    ) -> Result<TagDefinitionElement, MannequinXmlParseError> {
        if self.stack.is_empty() {
            ensure_element(reader, "document", "TagDefinition", event)?;
            ensure_attributes(reader, event, "TagDefinition", &[b"version"])?;
            self.root_seen = true;
            self.version = attr_string(reader, event, b"version")?;
            return Ok(TagDefinitionElement::Root);
        }

        let parent = self
            .stack
            .last()
            .copied()
            .unwrap_or(TagDefinitionElement::Unknown);
        match parent {
            TagDefinitionElement::Root => self.visit_root_child(reader, event, empty),
            TagDefinitionElement::Imports => {
                ensure_element(reader, "Imports", "Import", event)?;
                ensure_attributes(reader, event, "Import", &[b"filename"])?;
                self.imports.push(attr_required_string(
                    reader,
                    event,
                    b"filename",
                    "filename",
                )?);
                Ok(TagDefinitionElement::Import)
            }
            TagDefinitionElement::Tags => match local_name_string(reader, event)?.as_str() {
                "Tag" => {
                    let tag = parse_tag_entry(reader, event)?;
                    self.entries.push(MannequinTagDefinitionEntry::Tag(tag));
                    Ok(TagDefinitionElement::Tag)
                }
                "Group" => {
                    ensure_attributes(reader, event, "Group", &[b"name"])?;
                    self.current_group = Some(MannequinTagGroup {
                        name: attr_required_string(reader, event, b"name", "name")?,
                        tags: Vec::new(),
                    });
                    Ok(TagDefinitionElement::Group)
                }
                child => Err(MannequinXmlParseError::UnexpectedElement {
                    parent: "Tags".to_string(),
                    child: child.to_string(),
                }),
            },
            TagDefinitionElement::Group => {
                let tag = if self.is_version_1() {
                    parse_v1_tag_entry(reader, event)?
                } else {
                    ensure_element(reader, "Group", "Tag", event)?;
                    parse_tag_entry(reader, event)?
                };
                let Some(group) = self.current_group.as_mut() else {
                    return Err(MannequinXmlParseError::MissingElement("Group"));
                };
                group.tags.push(tag);
                Ok(TagDefinitionElement::Tag)
            }
            TagDefinitionElement::Import => Err(MannequinXmlParseError::UnexpectedElement {
                parent: "Import".to_string(),
                child: local_name_string(reader, event)?,
            }),
            TagDefinitionElement::Tag => Err(MannequinXmlParseError::UnexpectedElement {
                parent: "Tag".to_string(),
                child: local_name_string(reader, event)?,
            }),
            TagDefinitionElement::Unknown => Ok(TagDefinitionElement::Unknown),
        }
    }

    fn visit_root_child(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        empty: bool,
    ) -> Result<TagDefinitionElement, MannequinXmlParseError> {
        let name = local_name_string(reader, event)?;
        if self.is_version_1() {
            if empty {
                self.entries
                    .push(MannequinTagDefinitionEntry::Tag(parse_v1_tag_entry(
                        reader, event,
                    )?));
                return Ok(TagDefinitionElement::Tag);
            }
            ensure_no_attributes(reader, event, "TagGroup")?;
            self.current_group = Some(MannequinTagGroup {
                name,
                tags: Vec::new(),
            });
            return Ok(TagDefinitionElement::Group);
        }

        match name.as_str() {
            "Imports" => {
                ensure_no_attributes(reader, event, "Imports")?;
                Ok(TagDefinitionElement::Imports)
            }
            "Tags" => {
                ensure_no_attributes(reader, event, "Tags")?;
                Ok(TagDefinitionElement::Tags)
            }
            child => Err(MannequinXmlParseError::UnexpectedElement {
                parent: "TagDefinition".to_string(),
                child: child.to_string(),
            }),
        }
    }

    fn visit_end(&mut self, element: TagDefinitionElement) {
        if matches!(element, TagDefinitionElement::Group)
            && let Some(group) = self.current_group.take()
        {
            self.entries.push(MannequinTagDefinitionEntry::Group(group));
        }
    }

    fn finish(self) -> Result<MannequinTagDefinitionSource, MannequinXmlParseError> {
        if !self.root_seen {
            return Err(MannequinXmlParseError::MissingRoot);
        }
        Ok(MannequinTagDefinitionSource {
            source_path: self.source_path,
            version: self.version,
            imports: self.imports,
            entries: self.entries,
        })
    }

    fn is_version_1(&self) -> bool {
        self.version.as_deref().unwrap_or("1") == "1"
    }
}

#[derive(Debug, Clone, Copy)]
enum TagDefinitionElement {
    Root,
    Imports,
    Import,
    Tags,
    Group,
    Tag,
    Unknown,
}

fn parse_controller_definition_source(
    source_path: String,
    bytes: &[u8],
) -> Result<MannequinControllerDefinitionSource, MannequinXmlParseError> {
    let xml = str::from_utf8(bytes)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut state = ControllerDefinitionParseState::new(source_path);
    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                let element = state.visit_start(&reader, &event)?;
                state.stack.push(element);
            }
            Event::Empty(event) => {
                let element = state.visit_start(&reader, &event)?;
                state.visit_end(element);
            }
            Event::End(_) => {
                let element = state
                    .stack
                    .pop()
                    .unwrap_or(ControllerDefinitionElement::Unknown);
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
    state.finish()
}

#[derive(Debug)]
struct ControllerDefinitionParseState {
    source_path: String,
    tags: Option<MannequinFileReference>,
    fragments: Option<MannequinFileReference>,
    fragment_definitions: Vec<MannequinFragmentDefinition>,
    scope_contexts: Vec<MannequinScopeContext>,
    scopes: Vec<MannequinScopeDefinition>,
    stack: Vec<ControllerDefinitionElement>,
    root_seen: bool,
}

impl ControllerDefinitionParseState {
    fn new(source_path: String) -> Self {
        Self {
            source_path,
            tags: None,
            fragments: None,
            fragment_definitions: Vec::new(),
            scope_contexts: Vec::new(),
            scopes: Vec::new(),
            stack: Vec::new(),
            root_seen: false,
        }
    }

    fn visit_start(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        if self.stack.is_empty() {
            ensure_element(reader, "document", "ControllerDef", event)?;
            ensure_no_attributes(reader, event, "ControllerDef")?;
            self.root_seen = true;
            return Ok(ControllerDefinitionElement::Root);
        }

        let parent = self
            .stack
            .last()
            .copied()
            .unwrap_or(ControllerDefinitionElement::Unknown);
        match parent {
            ControllerDefinitionElement::Root => self.visit_root_child(reader, event),
            ControllerDefinitionElement::FragmentDefs => self.visit_fragment_def(reader, event),
            ControllerDefinitionElement::Fragment(index) => {
                self.visit_fragment_child(reader, event, index)
            }
            ControllerDefinitionElement::ScopeContextDefs => {
                self.visit_scope_context(reader, event)
            }
            ControllerDefinitionElement::ScopeDefs => self.visit_scope(reader, event),
            ControllerDefinitionElement::TagsRef
            | ControllerDefinitionElement::FragmentsRef
            | ControllerDefinitionElement::FragmentOverride
            | ControllerDefinitionElement::ScopeContext
            | ControllerDefinitionElement::Scope => {
                Err(MannequinXmlParseError::UnexpectedElement {
                    parent: parent.name().to_string(),
                    child: local_name_string(reader, event)?,
                })
            }
            ControllerDefinitionElement::Unknown => Ok(ControllerDefinitionElement::Unknown),
        }
    }

    fn visit_root_child(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        match local_name_string(reader, event)?.as_str() {
            "Tags" => {
                ensure_attributes(reader, event, "Tags", &[b"filename"])?;
                self.tags = Some(MannequinFileReference {
                    filename: attr_required_string(reader, event, b"filename", "filename")?,
                });
                Ok(ControllerDefinitionElement::TagsRef)
            }
            "Fragments" => {
                ensure_attributes(reader, event, "Fragments", &[b"filename"])?;
                self.fragments = Some(MannequinFileReference {
                    filename: attr_required_string(reader, event, b"filename", "filename")?,
                });
                Ok(ControllerDefinitionElement::FragmentsRef)
            }
            "FragmentDefs" => {
                ensure_no_attributes(reader, event, "FragmentDefs")?;
                Ok(ControllerDefinitionElement::FragmentDefs)
            }
            "ScopeContextDefs" => {
                ensure_no_attributes(reader, event, "ScopeContextDefs")?;
                Ok(ControllerDefinitionElement::ScopeContextDefs)
            }
            "ScopeDefs" => {
                ensure_no_attributes(reader, event, "ScopeDefs")?;
                Ok(ControllerDefinitionElement::ScopeDefs)
            }
            child => Err(MannequinXmlParseError::UnexpectedElement {
                parent: "ControllerDef".to_string(),
                child: child.to_string(),
            }),
        }
    }

    fn visit_fragment_def(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        ensure_attributes(reader, event, "FragmentDef", &[b"scopes", b"flags"])?;
        let definition = MannequinFragmentDefinition {
            name: local_name_string(reader, event)?,
            scopes: attr_required_string(reader, event, b"scopes", "scopes")?,
            flags: attr_string(reader, event, b"flags")?,
            overrides: Vec::new(),
        };
        self.fragment_definitions.push(definition);
        Ok(ControllerDefinitionElement::Fragment(
            self.fragment_definitions.len() - 1,
        ))
    }

    fn visit_fragment_child(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        index: usize,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        ensure_element(reader, "fragment", "Override", event)?;
        ensure_attributes(reader, event, "Override", &[b"tags", b"scopes"])?;
        self.fragment_definitions[index]
            .overrides
            .push(MannequinFragmentOverride {
                tags: attr_required_string(reader, event, b"tags", "tags")?,
                scopes: attr_required_string(reader, event, b"scopes", "scopes")?,
            });
        Ok(ControllerDefinitionElement::FragmentOverride)
    }

    fn visit_scope_context(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        ensure_no_attributes(reader, event, "ScopeContext")?;
        self.scope_contexts.push(MannequinScopeContext {
            name: local_name_string(reader, event)?,
        });
        Ok(ControllerDefinitionElement::ScopeContext)
    }

    fn visit_scope(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<ControllerDefinitionElement, MannequinXmlParseError> {
        ensure_attributes(
            reader,
            event,
            "Scope",
            &[b"layer", b"numLayers", b"context", b"Tags"],
        )?;
        self.scopes.push(MannequinScopeDefinition {
            name: local_name_string(reader, event)?,
            layer: attr_required_i32(reader, event, b"layer", "layer")?,
            num_layers: attr_required_i32(reader, event, b"numLayers", "numLayers")?,
            context: attr_required_string(reader, event, b"context", "context")?,
            tags: attr_string(reader, event, b"Tags")?,
        });
        Ok(ControllerDefinitionElement::Scope)
    }

    const fn visit_end(&mut self, _element: ControllerDefinitionElement) {}

    fn finish(self) -> Result<MannequinControllerDefinitionSource, MannequinXmlParseError> {
        if !self.root_seen {
            return Err(MannequinXmlParseError::MissingRoot);
        }
        Ok(MannequinControllerDefinitionSource {
            source_path: self.source_path,
            tags: self
                .tags
                .ok_or(MannequinXmlParseError::MissingElement("Tags"))?,
            fragments: self
                .fragments
                .ok_or(MannequinXmlParseError::MissingElement("Fragments"))?,
            fragment_definitions: self.fragment_definitions,
            scope_contexts: self.scope_contexts,
            scopes: self.scopes,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum ControllerDefinitionElement {
    Root,
    TagsRef,
    FragmentsRef,
    FragmentDefs,
    Fragment(usize),
    FragmentOverride,
    ScopeContextDefs,
    ScopeContext,
    ScopeDefs,
    Scope,
    Unknown,
}

impl ControllerDefinitionElement {
    const fn name(self) -> &'static str {
        match self {
            Self::Root => "ControllerDef",
            Self::TagsRef => "Tags",
            Self::FragmentsRef => "Fragments",
            Self::FragmentDefs => "FragmentDefs",
            Self::Fragment(_) => "FragmentDef",
            Self::FragmentOverride => "Override",
            Self::ScopeContextDefs => "ScopeContextDefs",
            Self::ScopeContext => "ScopeContext",
            Self::ScopeDefs => "ScopeDefs",
            Self::Scope => "Scope",
            Self::Unknown => "unknown",
        }
    }
}

fn parse_tag_entry(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<MannequinTagEntry, MannequinXmlParseError> {
    ensure_attributes(reader, event, "Tag", &[b"name", b"priority", b"subTagDef"])?;
    Ok(MannequinTagEntry {
        name: attr_required_string(reader, event, b"name", "name")?,
        priority: attr_i32(reader, event, b"priority", "priority")?,
        sub_tag_definition: attr_string(reader, event, b"subTagDef")?,
    })
}

fn parse_v1_tag_entry(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<MannequinTagEntry, MannequinXmlParseError> {
    ensure_attributes(reader, event, "Tag", &[b"priority"])?;
    Ok(MannequinTagEntry {
        name: local_name_string(reader, event)?,
        priority: attr_i32(reader, event, b"priority", "priority")?,
        sub_tag_definition: None,
    })
}

fn ensure_element(
    reader: &Reader<&[u8]>,
    parent: &'static str,
    expected: &'static str,
    event: &BytesStart<'_>,
) -> Result<(), MannequinXmlParseError> {
    let actual = local_name_string(reader, event)?;
    if actual == expected {
        Ok(())
    } else {
        Err(MannequinXmlParseError::UnexpectedElement {
            parent: parent.to_string(),
            child: actual,
        })
    }
}

fn ensure_no_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
) -> Result<(), MannequinXmlParseError> {
    ensure_attributes(reader, event, element, &[])
}

fn ensure_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
    allowed: &[&[u8]],
) -> Result<(), MannequinXmlParseError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if !allowed
            .iter()
            .any(|allowed| attribute.key.as_ref() == *allowed)
        {
            return Err(MannequinXmlParseError::UnexpectedAttribute {
                element,
                attribute: decode_bytes(reader, attribute.key.as_ref())?,
            });
        }
    }
    Ok(())
}

fn attr_required_string(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<String, MannequinXmlParseError> {
    attr_string(reader, event, key)?.ok_or(MannequinXmlParseError::MissingAttribute(name))
}

fn attr_string(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
) -> Result<Option<String>, MannequinXmlParseError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if attribute.key.as_ref() == key {
            return Ok(Some(
                attribute
                    .decoded_and_normalized_value(
                        quick_xml::XmlVersion::default(),
                        reader.decoder(),
                    )?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

fn attr_required_i32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<i32, MannequinXmlParseError> {
    let value = attr_required_string(reader, event, key, name)?;
    parse_i32(name, value)
        .and_then(|value| value.ok_or(MannequinXmlParseError::MissingAttribute(name)))
}

fn attr_i32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<i32>, MannequinXmlParseError> {
    let Some(value) = attr_string(reader, event, key)? else {
        return Ok(None);
    };
    parse_i32(name, value)
}

fn parse_i32(name: &'static str, value: String) -> Result<Option<i32>, MannequinXmlParseError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| MannequinXmlParseError::InvalidInteger {
            name,
            value,
            source,
        })
}

fn local_name_string(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<String, MannequinXmlParseError> {
    decode_bytes(reader, event.local_name().as_ref())
}

fn decode_bytes(reader: &Reader<&[u8]>, bytes: &[u8]) -> Result<String, MannequinXmlParseError> {
    Ok(reader
        .decoder()
        .decode(bytes)
        .map_err(quick_xml::Error::from)?
        .into_owned())
}

#[derive(Debug, Error)]
pub enum MannequinSourceTransformError {
    #[error("unsupported Mannequin XML path {path}")]
    UnsupportedPath { path: String },
    #[error("parse Mannequin animation database source: {0}")]
    AnimationDatabase(#[from] MannequinAnimationDatabaseSourceError),
    #[error("parse Mannequin XML source: {0}")]
    Parse(#[from] MannequinXmlParseError),
    #[error("serialize Mannequin XML source RON: {0}")]
    Serialize(#[from] ron::Error),
}

#[derive(Debug, Error)]
pub enum MannequinAnimationDatabaseSourceError {
    #[error("parse AnimDB XML: {0}")]
    Parse(#[from] AnimationDatabaseParseError),
    #[error("reconstruct AnimDB source: {0}")]
    Structure(&'static str),
}

#[derive(Debug, Error)]
pub enum MannequinXmlParseError {
    #[error("expected Mannequin XML root")]
    MissingRoot,
    #[error("missing `{0}` element")]
    MissingElement(&'static str),
    #[error("missing `{0}` attribute")]
    MissingAttribute(&'static str),
    #[error("unexpected element `{child}` under `{parent}`")]
    UnexpectedElement { parent: String, child: String },
    #[error("unexpected attribute `{attribute}` on `{element}`")]
    UnexpectedAttribute {
        element: &'static str,
        attribute: String,
    },
    #[error("invalid integer `{value}` in `{name}`")]
    InvalidInteger {
        name: &'static str,
        value: String,
        #[source]
        source: ParseIntError,
    },
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
    fn routes_mannequin_sources_without_claiming_generated_ron() {
        assert_eq!(
            MannequinXmlKind::from_source_path("Animations/Mannequin/ADB/Player/combat.adb"),
            Some(MannequinXmlKind::AnimationDatabase)
        );
        assert_eq!(
            MannequinXmlKind::from_source_path(
                "Animations/Mannequin/ADB/Player/playermaleactions.xml"
            ),
            Some(MannequinXmlKind::Actions)
        );
        assert_eq!(
            MannequinXmlKind::from_source_path(
                "Animations/Mannequin/ADB/Player/playermaletags.xml"
            ),
            Some(MannequinXmlKind::Tags)
        );
        assert_eq!(
            MannequinXmlKind::from_source_path(
                "Animations/Mannequin/ADB/Player/playermalecontrollerdefs.xml"
            ),
            Some(MannequinXmlKind::ControllerDefinition)
        );
        assert_eq!(
            mannequin_source_path("animations/mannequin/adb/player/combat.adb").as_deref(),
            Some("animations/mannequin/adb/player/combat.adb.ron")
        );
        assert_eq!(
            mannequin_source_path("animations/mannequin/adb/player/playermaleactions.xml")
                .as_deref(),
            Some("animations/mannequin/adb/player/playermaleactions.mannequin.actions.ron")
        );
        assert!(!is_legacy_mannequin_source(
            "animations/mannequin/adb/player/combat.adb.ron"
        ));
        assert!(!is_legacy_mannequin_source(
            "animations/mannequin/adb/player/playermaleactions.mannequin.actions.ron"
        ));
        assert!(!is_legacy_mannequin_source(
            "libs/gameaudio/wwise/atl_controls.xml"
        ));
    }

    #[test]
    fn transforms_animation_database_to_authoring_source() {
        let output = MannequinSourceTransform
            .transform(MannequinSourceInput {
                source_path: "Animations/Mannequin/ADB/Player/combat.adb",
                bytes: sample_animation_database(),
            })
            .unwrap();

        let artifact = output;
        assert_eq!(
            artifact.path,
            "animations/mannequin/adb/player/combat.adb.ron"
        );
        assert_eq!(artifact.schema, MANNEQUIN_ANIMATION_DATABASE_SOURCE_SCHEMA);

        let source = MannequinAnimationDatabaseSource::from_ron_bytes(&artifact.bytes).unwrap();
        assert_eq!(
            source.source_path,
            "animations/mannequin/adb/player/combat.adb"
        );
        assert_eq!(
            source.database.fragment_definition.as_deref(),
            Some("actions.xml")
        );
        assert_eq!(source.database.tag_definition.as_deref(), Some("tags.xml"));
        assert_eq!(source.database.sub_databases[0].file, "sub.adb");
        assert_eq!(
            source.database.sub_databases[0].fragment_filters[0],
            "Attack"
        );
        let fragment = &source.database.fragment_groups[0].fragments[0];
        assert_eq!(source.database.fragment_groups[0].name, "Attack");
        assert_eq!(fragment.tags.as_deref(), Some("Elite"));
        assert_eq!(fragment.animation_layers[0].animations[0].name, "attack_a");
        assert_eq!(
            fragment.animation_layers[0].animations[0].channels[2],
            Some(0.5)
        );
        assert_eq!(
            fragment.procedural_layers[0].procedurals[0].parameters[0].name,
            "PosOffset"
        );
        assert_eq!(
            fragment.procedural_layers[0].procedurals[0].parameters[0].children[2]
                .value
                .as_deref(),
            Some("3")
        );
        assert_eq!(
            source.database.fragment_blends[0].from.as_deref(),
            Some("Idle")
        );
        assert_eq!(
            source.database.fragment_blends[0].variants[0]
                .from_fragment
                .as_deref(),
            Some("Light")
        );
        assert_eq!(
            source.database.fragment_blends[0].variants[0].fragments[0].enter_time,
            Some(0.1)
        );
    }

    #[test]
    fn transforms_actions_tag_definition_to_authoring_source() {
        let output = MannequinSourceTransform
            .transform(MannequinSourceInput {
                source_path: "Animations/Mannequin/ADB/Player/playermaleactions.xml",
                bytes: br#"<TagDefinition version="2">
 <Tags>
  <Tag name="Idle"/>
  <Group name="Combat">
   <Tag name="Attack" priority="3" subTagDef="attack_tags.xml"/>
  </Group>
 </Tags>
</TagDefinition>"#,
            })
            .unwrap();

        let artifact = output;
        assert_eq!(
            artifact.path,
            "animations/mannequin/adb/player/playermaleactions.mannequin.actions.ron"
        );
        assert_eq!(artifact.schema, MANNEQUIN_ACTIONS_SOURCE_SCHEMA);

        let source = MannequinActionsSource::from_ron_bytes(&artifact.bytes).unwrap();
        assert_eq!(
            source.source_path,
            "animations/mannequin/adb/player/playermaleactions.xml"
        );
        assert_eq!(source.version.as_deref(), Some("2"));
        assert_eq!(
            source.entries[0],
            MannequinTagDefinitionEntry::Tag(MannequinTagEntry {
                name: "Idle".to_string(),
                priority: None,
                sub_tag_definition: None,
            })
        );
        let MannequinTagDefinitionEntry::Group(group) = &source.entries[1] else {
            panic!("second entry should be a tag group");
        };
        assert_eq!(group.name, "Combat");
        assert_eq!(group.tags[0].name, "Attack");
        assert_eq!(group.tags[0].priority, Some(3));
        assert_eq!(
            group.tags[0].sub_tag_definition.as_deref(),
            Some("attack_tags.xml")
        );
    }

    #[test]
    fn transforms_controller_definition_to_authoring_source() {
        let output = MannequinSourceTransform
            .transform(MannequinSourceInput {
                source_path: "Animations/Mannequin/ADB/Player/playermalecontrollerdefs.xml",
                bytes: br#"<ControllerDef>
 <Tags filename="playermaletags.xml"/>
 <Fragments filename="playermaleactions.xml"/>
 <FragmentDefs>
  <Idle scopes="FullBody" flags="Loop">
   <Override tags="InCombat" scopes="UpperBody"/>
  </Idle>
 </FragmentDefs>
 <ScopeContextDefs>
  <Char3P/>
  <Audio/>
 </ScopeContextDefs>
 <ScopeDefs>
  <FullBody layer="0" numLayers="3" context="Char3P" Tags="Player"/>
  <Audio layer="0" numLayers="1" context="Audio"/>
 </ScopeDefs>
</ControllerDef>"#,
            })
            .unwrap();

        let artifact = output;
        assert_eq!(
            artifact.path,
            "animations/mannequin/adb/player/playermalecontrollerdefs.mannequin.controller.ron"
        );
        assert_eq!(artifact.schema, MANNEQUIN_CONTROLLER_SOURCE_SCHEMA);

        let source = MannequinControllerDefinitionSource::from_ron_bytes(&artifact.bytes).unwrap();
        assert_eq!(source.tags.filename, "playermaletags.xml");
        assert_eq!(source.fragments.filename, "playermaleactions.xml");
        assert_eq!(source.fragment_definitions[0].name, "Idle");
        assert_eq!(source.fragment_definitions[0].scopes, "FullBody");
        assert_eq!(
            source.fragment_definitions[0].overrides[0],
            MannequinFragmentOverride {
                tags: "InCombat".to_string(),
                scopes: "UpperBody".to_string(),
            }
        );
        assert_eq!(source.scope_contexts[0].name, "Char3P");
        assert_eq!(source.scopes[0].name, "FullBody");
        assert_eq!(source.scopes[0].num_layers, 3);
        assert_eq!(source.scopes[0].tags.as_deref(), Some("Player"));
    }

    #[test]
    fn rejects_unmapped_mannequin_attributes() {
        let error = MannequinTagDefinitionSource::from_legacy(
            "animations/mannequin/adb/player/playermaleactions.xml",
            br#"<TagDefinition version="2"><Tags><Tag name="Idle" unknown="x"/></Tags></TagDefinition>"#,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MannequinXmlParseError::UnexpectedAttribute {
                element: "Tag",
                attribute
            } if attribute == "unknown"
        ));
    }

    fn sample_animation_database() -> &'static [u8] {
        br#"
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
        "#
    }
}
