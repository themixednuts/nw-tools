//! Legacy CryAnimation parametric blend-space XML import transform.
//!
//! Mirrors the Lumberyard blend-space XML loader in
//! `CryAnimation/GlobalAnimationHeaderLMG.cpp` and the editor structures in
//! `CharacterTool/BlendSpace.h`.

use std::{
    num::{ParseFloatError, ParseIntError},
    str,
};

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::source_transform::{motion_path_from_animation_reference, normalize_source_path};

pub type BlendSpaceSourceSchema = &'static str;

pub const BLEND_SPACE_SOURCE_SCHEMA: BlendSpaceSourceSchema = "azoth.compat.cry.BlendSpaceSource";
pub const COMBINED_BLEND_SPACE_SOURCE_SCHEMA: BlendSpaceSourceSchema =
    "azoth.compat.cry.CombinedBlendSpaceSource";

const PARA_GROUP: &str = "ParaGroup";
const COMBINED_BLEND_SPACE: &str = "CombinedBlendSpace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendSpaceXmlKind {
    BlendSpace,
    CombinedBlendSpace,
}

impl BlendSpaceXmlKind {
    #[must_use]
    pub fn from_source_path(source_path: &str) -> Option<Self> {
        let normalized = normalize_source_path(source_path);
        if normalized.ends_with(".bspace") {
            Some(Self::BlendSpace)
        } else if normalized.ends_with(".comb") {
            Some(Self::CombinedBlendSpace)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn source_schema(self) -> BlendSpaceSourceSchema {
        match self {
            Self::BlendSpace => BLEND_SPACE_SOURCE_SCHEMA,
            Self::CombinedBlendSpace => COMBINED_BLEND_SPACE_SOURCE_SCHEMA,
        }
    }

    #[must_use]
    pub const fn source_suffix(self) -> &'static str {
        match self {
            Self::BlendSpace => "bspace.ron",
            Self::CombinedBlendSpace => "comb.ron",
        }
    }

    #[must_use]
    pub fn source_path(self, source_path: &str) -> String {
        let normalized = normalize_source_path(source_path);
        let stem = normalized
            .strip_suffix(".bspace")
            .or_else(|| normalized.strip_suffix(".comb"))
            .unwrap_or(&normalized);
        format!("{stem}.{}", self.source_suffix())
    }

    const fn root_name(self) -> &'static str {
        match self {
            Self::BlendSpace => PARA_GROUP,
            Self::CombinedBlendSpace => COMBINED_BLEND_SPACE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceSource {
    pub source_path: String,
    pub blend_space: BlendSpace,
}

impl BlendSpaceSource {
    pub fn from_legacy(source_path: &str, bytes: &[u8]) -> Result<Self, BlendSpaceSourceError> {
        Self::from_legacy_with_motion_resolver(source_path, bytes, |_| None)
    }

    pub fn from_legacy_with_motion_resolver<F>(
        source_path: &str,
        bytes: &[u8],
        resolver: F,
    ) -> Result<Self, BlendSpaceSourceError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut source = match parse_blend_space_document(bytes)? {
            BlendSpaceDocument::BlendSpace(blend_space) => Self {
                source_path: normalize_source_path(source_path),
                blend_space,
            },
            BlendSpaceDocument::CombinedBlendSpace(_) => {
                return Err(BlendSpaceSourceError::UnexpectedRoot {
                    expected: PARA_GROUP,
                    actual: COMBINED_BLEND_SPACE.to_string(),
                });
            }
        };
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
        self.blend_space.resolve_animation_references(&mut resolver);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombinedBlendSpaceSource {
    pub source_path: String,
    pub combined_blend_space: CombinedBlendSpace,
}

impl CombinedBlendSpaceSource {
    pub fn from_legacy(source_path: &str, bytes: &[u8]) -> Result<Self, BlendSpaceSourceError> {
        Self::from_legacy_with_motion_resolver(source_path, bytes, |_| None)
    }

    pub fn from_legacy_with_motion_resolver<F>(
        source_path: &str,
        bytes: &[u8],
        resolver: F,
    ) -> Result<Self, BlendSpaceSourceError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut source = match parse_blend_space_document(bytes)? {
            BlendSpaceDocument::CombinedBlendSpace(combined_blend_space) => Self {
                source_path: normalize_source_path(source_path),
                combined_blend_space,
            },
            BlendSpaceDocument::BlendSpace(_) => {
                return Err(BlendSpaceSourceError::UnexpectedRoot {
                    expected: COMBINED_BLEND_SPACE,
                    actual: PARA_GROUP.to_string(),
                });
            }
        };
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
        self.combined_blend_space
            .resolve_animation_references(&mut resolver);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpace {
    pub threshold: Option<f32>,
    pub idle_to_move: bool,
    pub dimensions: Vec<BlendSpaceDimension>,
    pub examples: Vec<BlendSpaceExample>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pseudo_examples: Vec<BlendSpacePseudoExample>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_extraction: Vec<BlendSpaceAdditionalExtraction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<BlendSpaceAnnotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion_combinations: Vec<BlendSpaceMotionCombination>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<BlendSpaceJoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtual_examples: Vec<BlendSpaceVirtualExample>,
}

impl BlendSpace {
    fn resolve_animation_references<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        for example in &mut self.examples {
            example.animation.resolve_motion_path(resolver);
        }
        for combination in &mut self.motion_combinations {
            combination.animation.resolve_motion_path(resolver);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombinedBlendSpace {
    pub idle_to_move: bool,
    pub dimensions: Vec<CombinedBlendSpaceDimension>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_extraction: Vec<BlendSpaceAdditionalExtraction>,
    pub blend_spaces: Vec<BlendSpaceReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion_combinations: Vec<BlendSpaceMotionCombination>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<BlendSpaceJoint>,
}

impl CombinedBlendSpace {
    fn resolve_animation_references<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        for combination in &mut self.motion_combinations {
            combination.animation.resolve_motion_path(resolver);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceDimension {
    pub name: String,
    pub parameter_id: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_parameter_reason: Option<String>,
    pub min: f32,
    pub max: f32,
    pub cells: u8,
    pub debug_visual_scale: f32,
    pub start_key: f32,
    pub end_key: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint_name: Option<String>,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombinedBlendSpaceDimension {
    pub name: String,
    pub parameter_id: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_parameter_reason: Option<String>,
    pub locked: bool,
    pub parameter_scale: f32,
    pub choose_blend_space: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceAdditionalExtraction {
    pub name: String,
    pub parameter_id: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_parameter_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceExample {
    pub animation: BlendSpaceAnimationRef,
    pub coordinates: Vec<BlendSpaceCoordinate>,
    pub playback_scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceAnimationRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_motion_reason: Option<String>,
}

impl BlendSpaceAnimationRef {
    fn new(name: String) -> Self {
        Self {
            name,
            motion_path: None,
            unresolved_motion_reason: None,
        }
    }

    fn resolve_motion_path<F>(&mut self, resolver: &mut F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        if self.name.trim().is_empty() {
            self.motion_path = None;
            self.unresolved_motion_reason = None;
            return;
        }

        self.motion_path =
            resolver(&self.name).or_else(|| motion_path_from_animation_reference(&self.name));
        self.unresolved_motion_reason = self.motion_path.is_none().then(|| {
            "animation reference could not be resolved to a CAF/.anim.glb path".to_string()
        });
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceCoordinate {
    pub dimension: String,
    pub value: Option<f32>,
    pub use_directly_for_delta_motion: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpacePseudoExample {
    pub i0: i32,
    pub i1: i32,
    pub w0: f32,
    pub w1: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlendSpaceAnnotation {
    pub indices: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceMotionCombination {
    pub animation: BlendSpaceAnimationRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlendSpaceJoint {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceVirtualExample {
    pub indices: Vec<i32>,
    pub weights: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceReference {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authoring_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_reference_reason: Option<String>,
}

impl BlendSpaceReference {
    fn new(path: String) -> Self {
        let normalized = normalize_source_path(&path);
        let authoring_path = normalized
            .strip_suffix(".bspace")
            .map(|stem| format!("{stem}.bspace.ron"));
        let unresolved_reference_reason = authoring_path
            .is_none()
            .then(|| "combined blend-space member is not a .bspace source path".to_string());
        Self {
            path: normalized,
            authoring_path,
            unresolved_reference_reason,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlendSpaceSourceTransform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendSpaceSourceInput<'a> {
    pub source_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlendSpaceSourceArtifact {
    pub path: String,
    pub schema: BlendSpaceSourceSchema,
    pub bytes: Vec<u8>,
}

impl BlendSpaceSourceTransform {
    pub fn transform(
        &self,
        input: BlendSpaceSourceInput<'_>,
    ) -> Result<BlendSpaceSourceArtifact, BlendSpaceSourceTransformError> {
        self.transform_with_motion_resolver(input, |_| None)
    }

    pub fn transform_with_motion_resolver<F>(
        &self,
        input: BlendSpaceSourceInput<'_>,
        resolver: F,
    ) -> Result<BlendSpaceSourceArtifact, BlendSpaceSourceTransformError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let kind = BlendSpaceXmlKind::from_source_path(input.source_path).ok_or_else(|| {
            BlendSpaceSourceTransformError::UnsupportedPath {
                path: normalize_source_path(input.source_path),
            }
        })?;

        let bytes = match kind {
            BlendSpaceXmlKind::BlendSpace => BlendSpaceSource::from_legacy_with_motion_resolver(
                input.source_path,
                input.bytes,
                resolver,
            )?
            .to_ron_bytes()?,
            BlendSpaceXmlKind::CombinedBlendSpace => {
                CombinedBlendSpaceSource::from_legacy_with_motion_resolver(
                    input.source_path,
                    input.bytes,
                    resolver,
                )?
                .to_ron_bytes()?
            }
        };

        Ok(BlendSpaceSourceArtifact {
            path: kind.source_path(input.source_path),
            schema: kind.source_schema(),
            bytes,
        })
    }
}

#[must_use]
pub fn is_legacy_blend_space_source(source_path: &str) -> bool {
    BlendSpaceXmlKind::from_source_path(source_path).is_some()
}

#[must_use]
pub fn blend_space_source_path(source_path: &str) -> Option<String> {
    BlendSpaceXmlKind::from_source_path(source_path).map(|kind| kind.source_path(source_path))
}

#[must_use]
pub fn motion_parameter_id(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "movespeed" => Some(0),
        "turnspeed" => Some(1),
        "travelangle" => Some(2),
        "travelslope" => Some(3),
        "turnangle" => Some(4),
        "traveldist" => Some(5),
        "stopleg" => Some(6),
        "blendweight" => Some(7),
        "blendweight2" => Some(8),
        "blendweight3" => Some(9),
        "blendweight4" => Some(10),
        "blendweight5" => Some(11),
        "blendweight6" => Some(12),
        "blendweight7" => Some(13),
        _ => None,
    }
}

fn additional_extraction_parameter_id(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "movespeed" => Some(0),
        "turnspeed" => Some(1),
        "travelangle" => Some(2),
        "travelslope" => Some(3),
        "turnangle" => Some(4),
        "traveldist" => Some(5),
        _ => None,
    }
}

enum BlendSpaceDocument {
    BlendSpace(BlendSpace),
    CombinedBlendSpace(CombinedBlendSpace),
}

fn parse_blend_space_document(bytes: &[u8]) -> Result<BlendSpaceDocument, BlendSpaceSourceError> {
    let xml = str::from_utf8(bytes)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut state = BlendSpaceParseState::default();
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
                let element = state.stack.pop().unwrap_or(BlendSpaceElement::Unknown);
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

#[derive(Default)]
struct BlendSpaceParseState {
    stack: Vec<BlendSpaceElement>,
    root_kind: Option<BlendSpaceXmlKind>,
    blend_space: Option<BlendSpace>,
    combined_blend_space: Option<CombinedBlendSpace>,
}

impl BlendSpaceParseState {
    fn visit_start(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        if self.stack.is_empty() {
            return self.visit_root(reader, event);
        }

        match self
            .stack
            .last()
            .copied()
            .unwrap_or(BlendSpaceElement::Unknown)
        {
            BlendSpaceElement::Root(kind) => self.visit_root_child(reader, event, kind),
            BlendSpaceElement::Dimensions => self.visit_dimension(reader, event),
            BlendSpaceElement::AdditionalExtraction => {
                self.visit_additional_extraction(reader, event)
            }
            BlendSpaceElement::ExampleList => self.visit_example(reader, event),
            BlendSpaceElement::ExamplePseudo => self.visit_pseudo_example(reader, event),
            BlendSpaceElement::Blendable => self.visit_annotation(reader, event),
            BlendSpaceElement::VGrid => self.visit_virtual_example(reader, event),
            BlendSpaceElement::BlendSpaces => self.visit_blend_space_reference(reader, event),
            BlendSpaceElement::MotionCombination => self.visit_motion_combination(reader, event),
            BlendSpaceElement::JointList => self.visit_joint(reader, event),
            parent => Err(BlendSpaceSourceError::UnexpectedElement {
                parent: parent.name().to_string(),
                child: local_name_string(reader, event)?,
            }),
        }
    }

    fn visit_root(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        let root = local_name_string(reader, event)?;
        let kind = match root.as_str() {
            PARA_GROUP => BlendSpaceXmlKind::BlendSpace,
            COMBINED_BLEND_SPACE => BlendSpaceXmlKind::CombinedBlendSpace,
            _ => {
                return Err(BlendSpaceSourceError::UnexpectedRoot {
                    expected: "ParaGroup or CombinedBlendSpace",
                    actual: root,
                });
            }
        };
        ensure_no_attributes(reader, event, kind.root_name())?;
        self.root_kind = Some(kind);
        match kind {
            BlendSpaceXmlKind::BlendSpace => {
                self.blend_space = Some(BlendSpace {
                    threshold: None,
                    idle_to_move: false,
                    dimensions: Vec::new(),
                    examples: Vec::new(),
                    pseudo_examples: Vec::new(),
                    additional_extraction: Vec::new(),
                    annotations: Vec::new(),
                    motion_combinations: Vec::new(),
                    joints: Vec::new(),
                    virtual_examples: Vec::new(),
                });
            }
            BlendSpaceXmlKind::CombinedBlendSpace => {
                self.combined_blend_space = Some(CombinedBlendSpace {
                    idle_to_move: false,
                    dimensions: Vec::new(),
                    additional_extraction: Vec::new(),
                    blend_spaces: Vec::new(),
                    motion_combinations: Vec::new(),
                    joints: Vec::new(),
                });
            }
        }
        Ok(BlendSpaceElement::Root(kind))
    }

    fn visit_root_child(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
        kind: BlendSpaceXmlKind,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        let name = local_name_string(reader, event)?;
        match name.as_str() {
            "Dimensions" => {
                ensure_no_attributes(reader, event, "Dimensions")?;
                Ok(BlendSpaceElement::Dimensions)
            }
            "AdditionalExtraction" => {
                ensure_no_attributes(reader, event, "AdditionalExtraction")?;
                Ok(BlendSpaceElement::AdditionalExtraction)
            }
            "MotionCombination" => {
                ensure_no_attributes(reader, event, "MotionCombination")?;
                Ok(BlendSpaceElement::MotionCombination)
            }
            "JointList" => {
                ensure_no_attributes(reader, event, "JointList")?;
                Ok(BlendSpaceElement::JointList)
            }
            "VEGPARAMS" => {
                ensure_attributes(reader, event, "VEGPARAMS", &[b"Idle2Move"])?;
                let idle_to_move =
                    attr_bool(reader, event, b"Idle2Move", "Idle2Move")?.unwrap_or(false);
                match kind {
                    BlendSpaceXmlKind::BlendSpace => {
                        self.blend_space_mut()?.idle_to_move = idle_to_move
                    }
                    BlendSpaceXmlKind::CombinedBlendSpace => {
                        self.combined_blend_space_mut()?.idle_to_move = idle_to_move;
                    }
                }
                Ok(BlendSpaceElement::VegParams)
            }
            "ExampleList" if matches!(kind, BlendSpaceXmlKind::BlendSpace) => {
                ensure_no_attributes(reader, event, "ExampleList")?;
                Ok(BlendSpaceElement::ExampleList)
            }
            "ExamplePseudo" if matches!(kind, BlendSpaceXmlKind::BlendSpace) => {
                ensure_no_attributes(reader, event, "ExamplePseudo")?;
                Ok(BlendSpaceElement::ExamplePseudo)
            }
            "Blendable" if matches!(kind, BlendSpaceXmlKind::BlendSpace) => {
                ensure_no_attributes(reader, event, "Blendable")?;
                Ok(BlendSpaceElement::Blendable)
            }
            "VGrid" if matches!(kind, BlendSpaceXmlKind::BlendSpace) => {
                ensure_no_attributes(reader, event, "VGrid")?;
                Ok(BlendSpaceElement::VGrid)
            }
            "THRESHOLD" if matches!(kind, BlendSpaceXmlKind::BlendSpace) => {
                ensure_attributes(reader, event, "THRESHOLD", &[b"tz"])?;
                self.blend_space_mut()?.threshold = attr_f32(reader, event, b"tz", "tz")?;
                Ok(BlendSpaceElement::Threshold)
            }
            "BlendSpaces" if matches!(kind, BlendSpaceXmlKind::CombinedBlendSpace) => {
                ensure_no_attributes(reader, event, "BlendSpaces")?;
                Ok(BlendSpaceElement::BlendSpaces)
            }
            _ => Err(BlendSpaceSourceError::UnexpectedElement {
                parent: kind.root_name().to_string(),
                child: name,
            }),
        }
    }

    fn visit_dimension(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "Dimensions", "Param", event)?;
        match self.root_kind.ok_or(BlendSpaceSourceError::MissingRoot)? {
            BlendSpaceXmlKind::BlendSpace => {
                ensure_attributes(
                    reader,
                    event,
                    "Dimensions/Param",
                    &[
                        b"name",
                        b"Name",
                        b"min",
                        b"Min",
                        b"max",
                        b"Max",
                        b"cells",
                        b"Cells",
                        b"scale",
                        b"Scale",
                        b"skey",
                        b"SKey",
                        b"ekey",
                        b"EKey",
                        b"JointName",
                        b"jointName",
                        b"locked",
                        b"Locked",
                    ],
                )?;
                let name = attr_required_string_ci(reader, event, b"name", "name")?;
                let parameter_id = motion_parameter_id(&name);
                self.blend_space_mut()?
                    .dimensions
                    .push(BlendSpaceDimension {
                        unresolved_parameter_reason: unresolved_parameter_reason(
                            &name,
                            parameter_id,
                        ),
                        name,
                        parameter_id,
                        min: attr_f32_ci(reader, event, b"min", "min")?.unwrap_or(0.0),
                        max: attr_f32_ci(reader, event, b"max", "max")?.unwrap_or(1.0),
                        cells: attr_u8_ci(reader, event, b"cells", "cells")?
                            .unwrap_or(8)
                            .max(3),
                        debug_visual_scale: attr_f32_ci(reader, event, b"scale", "scale")?
                            .unwrap_or(1.0)
                            .max(0.01),
                        start_key: attr_f32_ci(reader, event, b"skey", "skey")?.unwrap_or(0.0),
                        end_key: attr_f32_ci(reader, event, b"ekey", "ekey")?.unwrap_or(1.0),
                        joint_name: non_empty(attr_string_ci(reader, event, b"JointName")?),
                        locked: attr_bool_ci(reader, event, b"locked", "locked")?.unwrap_or(false),
                    });
            }
            BlendSpaceXmlKind::CombinedBlendSpace => {
                ensure_attributes(
                    reader,
                    event,
                    "Dimensions/Param",
                    &[
                        b"name",
                        b"Name",
                        b"locked",
                        b"Locked",
                        b"ParaScale",
                        b"paraScale",
                        b"ChooseBlendSpace",
                        b"chooseBlendSpace",
                    ],
                )?;
                let name = attr_required_string_ci(reader, event, b"name", "name")?;
                let parameter_id = motion_parameter_id(&name);
                self.combined_blend_space_mut()?
                    .dimensions
                    .push(CombinedBlendSpaceDimension {
                        unresolved_parameter_reason: unresolved_parameter_reason(
                            &name,
                            parameter_id,
                        ),
                        name,
                        parameter_id,
                        locked: attr_bool_ci(reader, event, b"locked", "locked")?.unwrap_or(false),
                        parameter_scale: attr_f32_ci(reader, event, b"ParaScale", "ParaScale")?
                            .unwrap_or(1.0),
                        choose_blend_space: attr_bool_ci(
                            reader,
                            event,
                            b"ChooseBlendSpace",
                            "ChooseBlendSpace",
                        )?
                        .unwrap_or(false),
                    });
            }
        }
        Ok(BlendSpaceElement::Param)
    }

    fn visit_additional_extraction(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "AdditionalExtraction", "Param", event)?;
        ensure_attributes(
            reader,
            event,
            "AdditionalExtraction/Param",
            &[b"name", b"Name"],
        )?;
        let name = attr_required_string_ci(reader, event, b"name", "name")?;
        let parameter_id = additional_extraction_parameter_id(&name);
        let extraction = BlendSpaceAdditionalExtraction {
            unresolved_parameter_reason: unresolved_parameter_reason(&name, parameter_id),
            name,
            parameter_id,
        };
        match self.root_kind.ok_or(BlendSpaceSourceError::MissingRoot)? {
            BlendSpaceXmlKind::BlendSpace => self
                .blend_space_mut()?
                .additional_extraction
                .push(extraction),
            BlendSpaceXmlKind::CombinedBlendSpace => self
                .combined_blend_space_mut()?
                .additional_extraction
                .push(extraction),
        }
        Ok(BlendSpaceElement::Param)
    }

    fn visit_example(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "ExampleList", "Example", event)?;
        ensure_example_attributes(reader, event)?;
        let animation = attr_required_string_ci(reader, event, b"AName", "AName")?;
        let dimension_names = self
            .blend_space
            .as_ref()
            .ok_or(BlendSpaceSourceError::Structure("Example before ParaGroup"))?
            .dimensions
            .iter()
            .map(|dimension| dimension.name.clone())
            .collect::<Vec<_>>();
        validate_coordinate_attribute_bounds(reader, event, dimension_names.len())?;

        let coordinates = dimension_names
            .into_iter()
            .enumerate()
            .map(|(index, dimension)| {
                Ok(BlendSpaceCoordinate {
                    dimension,
                    value: attr_f32_index(reader, event, b"SetPara", index)?,
                    use_directly_for_delta_motion: attr_bool_index(
                        reader,
                        event,
                        b"UseDirectlyForDeltaMotion",
                        index,
                    )?
                    .unwrap_or(false),
                })
            })
            .collect::<Result<Vec<_>, BlendSpaceSourceError>>()?;

        self.blend_space_mut()?.examples.push(BlendSpaceExample {
            animation: BlendSpaceAnimationRef::new(animation),
            coordinates,
            playback_scale: attr_f32_ci(reader, event, b"PlaybackScale", "PlaybackScale")?
                .unwrap_or(1.0),
        });
        Ok(BlendSpaceElement::Example)
    }

    fn visit_pseudo_example(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "ExamplePseudo", "Pseudo", event)?;
        ensure_attributes(
            reader,
            event,
            "ExamplePseudo/Pseudo",
            &[b"p0", b"p1", b"w0", b"w1"],
        )?;
        self.blend_space_mut()?
            .pseudo_examples
            .push(BlendSpacePseudoExample {
                i0: attr_i32_required(reader, event, b"p0", "p0")?,
                i1: attr_i32_required(reader, event, b"p1", "p1")?,
                w0: attr_f32_required(reader, event, b"w0", "w0")?,
                w1: attr_f32_required(reader, event, b"w1", "w1")?,
            });
        Ok(BlendSpaceElement::Pseudo)
    }

    fn visit_annotation(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "Blendable", "Face", event)?;
        ensure_index_attributes(reader, event, "Blendable/Face", b"p", 8)?;
        let mut indices = Vec::new();
        for index in 0..8 {
            if let Some(value) = attr_i32_index(reader, event, b"p", index)? {
                indices.push(value);
            }
        }
        self.blend_space_mut()?
            .annotations
            .push(BlendSpaceAnnotation { indices });
        Ok(BlendSpaceElement::Face)
    }

    fn visit_virtual_example(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "VGrid", "VExample", event)?;
        let dimension_count = self.blend_space_mut()?.dimensions.len();
        let arity = match dimension_count {
            1 => 2,
            2 => 4,
            3 => 8,
            other => {
                return Err(BlendSpaceSourceError::UnsupportedVirtualExampleDimension(
                    other,
                ));
            }
        };
        ensure_virtual_example_attributes(reader, event, arity)?;
        let mut indices = Vec::with_capacity(arity);
        let mut weights = Vec::with_capacity(arity);
        for index in 0..arity {
            indices.push(attr_i32_index_required(reader, event, b"i", index)?);
            weights.push(attr_f32_index_required(reader, event, b"w", index)?);
        }
        self.blend_space_mut()?
            .virtual_examples
            .push(BlendSpaceVirtualExample { indices, weights });
        Ok(BlendSpaceElement::VExample)
    }

    fn visit_blend_space_reference(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "BlendSpaces", "BlendSpace", event)?;
        ensure_attributes(
            reader,
            event,
            "BlendSpaces/BlendSpace",
            &[b"AName", b"aname"],
        )?;
        let path = attr_required_string_ci(reader, event, b"AName", "AName")?;
        self.combined_blend_space_mut()?
            .blend_spaces
            .push(BlendSpaceReference::new(path));
        Ok(BlendSpaceElement::BlendSpaceRef)
    }

    fn visit_motion_combination(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "MotionCombination", "NewStyle", event)?;
        ensure_attributes(reader, event, "MotionCombination/NewStyle", &[b"Style"])?;
        let combination = BlendSpaceMotionCombination {
            animation: BlendSpaceAnimationRef::new(attr_required_string_ci(
                reader, event, b"Style", "Style",
            )?),
        };
        match self.root_kind.ok_or(BlendSpaceSourceError::MissingRoot)? {
            BlendSpaceXmlKind::BlendSpace => {
                self.blend_space_mut()?
                    .motion_combinations
                    .push(combination);
            }
            BlendSpaceXmlKind::CombinedBlendSpace => {
                self.combined_blend_space_mut()?
                    .motion_combinations
                    .push(combination);
            }
        }
        Ok(BlendSpaceElement::NewStyle)
    }

    fn visit_joint(
        &mut self,
        reader: &Reader<&[u8]>,
        event: &BytesStart<'_>,
    ) -> Result<BlendSpaceElement, BlendSpaceSourceError> {
        ensure_element(reader, "JointList", "Joint", event)?;
        ensure_attributes(reader, event, "JointList/Joint", &[b"Name"])?;
        let joint = BlendSpaceJoint {
            name: attr_required_string_ci(reader, event, b"Name", "Name")?,
        };
        match self.root_kind.ok_or(BlendSpaceSourceError::MissingRoot)? {
            BlendSpaceXmlKind::BlendSpace => self.blend_space_mut()?.joints.push(joint),
            BlendSpaceXmlKind::CombinedBlendSpace => {
                self.combined_blend_space_mut()?.joints.push(joint);
            }
        }
        Ok(BlendSpaceElement::Joint)
    }

    const fn visit_end(&mut self, _element: BlendSpaceElement) {}

    fn finish(self) -> Result<BlendSpaceDocument, BlendSpaceSourceError> {
        match self.root_kind {
            Some(BlendSpaceXmlKind::BlendSpace) => {
                let blend_space = self.blend_space.ok_or(BlendSpaceSourceError::Structure(
                    "missing ParaGroup document",
                ))?;
                if blend_space.dimensions.is_empty() {
                    return Err(BlendSpaceSourceError::MissingElement("Dimensions/Param"));
                }
                if blend_space.examples.is_empty() {
                    return Err(BlendSpaceSourceError::MissingElement("ExampleList/Example"));
                }
                Ok(BlendSpaceDocument::BlendSpace(blend_space))
            }
            Some(BlendSpaceXmlKind::CombinedBlendSpace) => {
                let combined_blend_space =
                    self.combined_blend_space
                        .ok_or(BlendSpaceSourceError::Structure(
                            "missing CombinedBlendSpace document",
                        ))?;
                if combined_blend_space.dimensions.is_empty() {
                    return Err(BlendSpaceSourceError::MissingElement("Dimensions/Param"));
                }
                if combined_blend_space.blend_spaces.is_empty() {
                    return Err(BlendSpaceSourceError::MissingElement(
                        "BlendSpaces/BlendSpace",
                    ));
                }
                Ok(BlendSpaceDocument::CombinedBlendSpace(combined_blend_space))
            }
            None => Err(BlendSpaceSourceError::MissingRoot),
        }
    }

    fn blend_space_mut(&mut self) -> Result<&mut BlendSpace, BlendSpaceSourceError> {
        self.blend_space
            .as_mut()
            .ok_or(BlendSpaceSourceError::Structure("expected ParaGroup"))
    }

    fn combined_blend_space_mut(
        &mut self,
    ) -> Result<&mut CombinedBlendSpace, BlendSpaceSourceError> {
        self.combined_blend_space
            .as_mut()
            .ok_or(BlendSpaceSourceError::Structure(
                "expected CombinedBlendSpace",
            ))
    }
}

#[derive(Debug, Clone, Copy)]
enum BlendSpaceElement {
    Root(BlendSpaceXmlKind),
    Dimensions,
    Param,
    AdditionalExtraction,
    ExampleList,
    Example,
    ExamplePseudo,
    Pseudo,
    Blendable,
    Face,
    VGrid,
    VExample,
    BlendSpaces,
    BlendSpaceRef,
    MotionCombination,
    NewStyle,
    JointList,
    Joint,
    Threshold,
    VegParams,
    Unknown,
}

impl BlendSpaceElement {
    const fn name(self) -> &'static str {
        match self {
            Self::Root(kind) => kind.root_name(),
            Self::Dimensions => "Dimensions",
            Self::Param => "Param",
            Self::AdditionalExtraction => "AdditionalExtraction",
            Self::ExampleList => "ExampleList",
            Self::Example => "Example",
            Self::ExamplePseudo => "ExamplePseudo",
            Self::Pseudo => "Pseudo",
            Self::Blendable => "Blendable",
            Self::Face => "Face",
            Self::VGrid => "VGrid",
            Self::VExample => "VExample",
            Self::BlendSpaces => "BlendSpaces",
            Self::BlendSpaceRef => "BlendSpace",
            Self::MotionCombination => "MotionCombination",
            Self::NewStyle => "NewStyle",
            Self::JointList => "JointList",
            Self::Joint => "Joint",
            Self::Threshold => "THRESHOLD",
            Self::VegParams => "VEGPARAMS",
            Self::Unknown => "unknown",
        }
    }
}

fn unresolved_parameter_reason(name: &str, parameter_id: Option<u8>) -> Option<String> {
    parameter_id.is_none().then(|| {
        format!("`{name}` is not present in the Lumberyard EMotionParamID blend-space mapping")
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

fn ensure_example_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<(), BlendSpaceSourceError> {
    const BASE: &[&[u8]] = &[b"AName", b"PlaybackScale"];
    for attribute in event.attributes() {
        let attribute = attribute?;
        let key = attribute.key.as_ref();
        if BASE.iter().any(|allowed| key.eq_ignore_ascii_case(allowed))
            || indexed_attribute(key, b"SetPara").is_some()
            || indexed_attribute(key, b"UseDirectlyForDeltaMotion").is_some()
        {
            continue;
        }
        return Err(BlendSpaceSourceError::UnexpectedAttribute {
            element: "ExampleList/Example",
            attribute: decode_bytes(reader, key)?,
        });
    }
    Ok(())
}

fn validate_coordinate_attribute_bounds(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    dimension_count: usize,
) -> Result<(), BlendSpaceSourceError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        let key = attribute.key.as_ref();
        for prefix in [
            b"SetPara".as_slice(),
            b"UseDirectlyForDeltaMotion".as_slice(),
        ] {
            if let Some(index) = indexed_attribute(key, prefix)
                && index >= dimension_count
            {
                return Err(BlendSpaceSourceError::CoordinateOutsideDimensions {
                    attribute: decode_bytes(reader, key)?,
                    dimension_count,
                });
            }
        }
    }
    Ok(())
}

fn ensure_index_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
    prefix: &[u8],
    limit: usize,
) -> Result<(), BlendSpaceSourceError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        let key = attribute.key.as_ref();
        let Some(index) = indexed_attribute(key, prefix) else {
            return Err(BlendSpaceSourceError::UnexpectedAttribute {
                element,
                attribute: decode_bytes(reader, key)?,
            });
        };
        if index >= limit {
            return Err(BlendSpaceSourceError::UnexpectedAttribute {
                element,
                attribute: decode_bytes(reader, key)?,
            });
        }
    }
    Ok(())
}

fn ensure_virtual_example_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    arity: usize,
) -> Result<(), BlendSpaceSourceError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        let key = attribute.key.as_ref();
        let allowed = indexed_attribute(key, b"i")
            .or_else(|| indexed_attribute(key, b"w"))
            .is_some_and(|index| index < arity);
        if !allowed {
            return Err(BlendSpaceSourceError::UnexpectedAttribute {
                element: "VGrid/VExample",
                attribute: decode_bytes(reader, key)?,
            });
        }
    }
    Ok(())
}

fn indexed_attribute(key: &[u8], prefix: &[u8]) -> Option<usize> {
    let rest = key.strip_prefix(prefix)?;
    if rest.is_empty() {
        return None;
    }
    str::from_utf8(rest).ok()?.parse().ok()
}

fn ensure_element(
    reader: &Reader<&[u8]>,
    parent: &'static str,
    expected: &'static str,
    event: &BytesStart<'_>,
) -> Result<(), BlendSpaceSourceError> {
    let actual = local_name_string(reader, event)?;
    if actual == expected {
        Ok(())
    } else {
        Err(BlendSpaceSourceError::UnexpectedElement {
            parent: parent.to_string(),
            child: actual,
        })
    }
}

fn ensure_no_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
) -> Result<(), BlendSpaceSourceError> {
    ensure_attributes(reader, event, element, &[])
}

fn ensure_attributes(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    element: &'static str,
    allowed: &[&[u8]],
) -> Result<(), BlendSpaceSourceError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if !allowed
            .iter()
            .any(|allowed| attribute.key.as_ref().eq_ignore_ascii_case(allowed))
        {
            return Err(BlendSpaceSourceError::UnexpectedAttribute {
                element,
                attribute: decode_bytes(reader, attribute.key.as_ref())?,
            });
        }
    }
    Ok(())
}

fn attr_required_string_ci(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<String, BlendSpaceSourceError> {
    attr_string_ci(reader, event, key)?.ok_or(BlendSpaceSourceError::MissingAttribute(name))
}

fn attr_string_ci(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
) -> Result<Option<String>, BlendSpaceSourceError> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if attribute.key.as_ref().eq_ignore_ascii_case(key) {
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

fn attr_i32_required(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<i32, BlendSpaceSourceError> {
    attr_i32(reader, event, key, name)?.ok_or(BlendSpaceSourceError::MissingAttribute(name))
}

fn attr_i32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<i32>, BlendSpaceSourceError> {
    let Some(value) = attr_string_ci(reader, event, key)? else {
        return Ok(None);
    };
    parse_i32(name, value)
}

fn attr_i32_index_required(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    prefix: &[u8],
    index: usize,
) -> Result<i32, BlendSpaceSourceError> {
    attr_i32_index(reader, event, prefix, index)?.ok_or(
        BlendSpaceSourceError::MissingIndexedAttribute {
            prefix: String::from_utf8_lossy(prefix).into_owned(),
            index,
        },
    )
}

fn attr_i32_index(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    prefix: &[u8],
    index: usize,
) -> Result<Option<i32>, BlendSpaceSourceError> {
    let key = format!("{}{}", String::from_utf8_lossy(prefix), index);
    let Some(value) = attr_string_ci(reader, event, key.as_bytes())? else {
        return Ok(None);
    };
    parse_i32_index(&key, value)
}

fn attr_f32_required(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<f32, BlendSpaceSourceError> {
    attr_f32(reader, event, key, name)?.ok_or(BlendSpaceSourceError::MissingAttribute(name))
}

fn attr_f32(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<f32>, BlendSpaceSourceError> {
    let Some(value) = attr_string_ci(reader, event, key)? else {
        return Ok(None);
    };
    parse_f32(name, value)
}

fn attr_f32_ci(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<f32>, BlendSpaceSourceError> {
    attr_f32(reader, event, key, name)
}

fn attr_f32_index_required(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    prefix: &[u8],
    index: usize,
) -> Result<f32, BlendSpaceSourceError> {
    attr_f32_index(reader, event, prefix, index)?.ok_or(
        BlendSpaceSourceError::MissingIndexedAttribute {
            prefix: String::from_utf8_lossy(prefix).into_owned(),
            index,
        },
    )
}

fn attr_f32_index(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    prefix: &[u8],
    index: usize,
) -> Result<Option<f32>, BlendSpaceSourceError> {
    let key = format!("{}{}", String::from_utf8_lossy(prefix), index);
    let Some(value) = attr_string_ci(reader, event, key.as_bytes())? else {
        return Ok(None);
    };
    parse_f32_index(&key, value)
}

fn attr_u8_ci(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<u8>, BlendSpaceSourceError> {
    let Some(value) = attr_string_ci(reader, event, key)? else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| BlendSpaceSourceError::InvalidInteger {
            name: name.to_string(),
            value,
            source,
        })
}

fn attr_bool_ci(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<bool>, BlendSpaceSourceError> {
    attr_bool(reader, event, key, name)
}

fn attr_bool(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    key: &[u8],
    name: &'static str,
) -> Result<Option<bool>, BlendSpaceSourceError> {
    let Some(value) = attr_string_ci(reader, event, key)? else {
        return Ok(None);
    };
    match value.trim() {
        "" => Ok(None),
        "0" | "false" | "False" | "FALSE" => Ok(Some(false)),
        "1" | "true" | "True" | "TRUE" => Ok(Some(true)),
        _ => Err(BlendSpaceSourceError::InvalidBool {
            name: name.to_string(),
            value,
        }),
    }
}

fn attr_bool_index(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    prefix: &[u8],
    index: usize,
) -> Result<Option<bool>, BlendSpaceSourceError> {
    let key = format!("{}{}", String::from_utf8_lossy(prefix), index);
    attr_bool(reader, event, key.as_bytes(), "indexed bool")
}

fn parse_i32(name: &'static str, value: String) -> Result<Option<i32>, BlendSpaceSourceError> {
    parse_i32_index(name, value)
}

fn parse_i32_index(
    name: impl Into<String>,
    value: String,
) -> Result<Option<i32>, BlendSpaceSourceError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| BlendSpaceSourceError::InvalidInteger {
            name: name.into(),
            value,
            source,
        })
}

fn parse_f32(name: &'static str, value: String) -> Result<Option<f32>, BlendSpaceSourceError> {
    parse_f32_index(name, value)
}

fn parse_f32_index(
    name: impl Into<String>,
    value: String,
) -> Result<Option<f32>, BlendSpaceSourceError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|source| BlendSpaceSourceError::InvalidFloat {
            name: name.into(),
            value,
            source,
        })
}

fn local_name_string(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<String, BlendSpaceSourceError> {
    decode_bytes(reader, event.local_name().as_ref())
}

fn decode_bytes(reader: &Reader<&[u8]>, bytes: &[u8]) -> Result<String, BlendSpaceSourceError> {
    Ok(reader
        .decoder()
        .decode(bytes)
        .map_err(quick_xml::Error::from)?
        .into_owned())
}

#[derive(Debug, Error)]
pub enum BlendSpaceSourceTransformError {
    #[error("unsupported blend-space XML path {path}")]
    UnsupportedPath { path: String },
    #[error("parse blend-space XML source: {0}")]
    Parse(#[from] BlendSpaceSourceError),
    #[error("serialize blend-space source RON: {0}")]
    Serialize(#[from] ron::Error),
}

#[derive(Debug, Error)]
pub enum BlendSpaceSourceError {
    #[error("expected ParaGroup or CombinedBlendSpace root")]
    MissingRoot,
    #[error("expected `{expected}` root, found `{actual}`")]
    UnexpectedRoot {
        expected: &'static str,
        actual: String,
    },
    #[error("missing `{0}` element")]
    MissingElement(&'static str),
    #[error("missing `{0}` attribute")]
    MissingAttribute(&'static str),
    #[error("missing `{prefix}{index}` attribute")]
    MissingIndexedAttribute { prefix: String, index: usize },
    #[error("unexpected element `{child}` under `{parent}`")]
    UnexpectedElement { parent: String, child: String },
    #[error("unexpected attribute `{attribute}` on `{element}`")]
    UnexpectedAttribute {
        element: &'static str,
        attribute: String,
    },
    #[error("coordinate attribute `{attribute}` exceeds {dimension_count} dimension(s)")]
    CoordinateOutsideDimensions {
        attribute: String,
        dimension_count: usize,
    },
    #[error("VGrid supports 1D, 2D, or 3D blend spaces, found {0} dimension(s)")]
    UnsupportedVirtualExampleDimension(usize),
    #[error("invalid float `{value}` in `{name}`")]
    InvalidFloat {
        name: String,
        value: String,
        #[source]
        source: ParseFloatError,
    },
    #[error("invalid integer `{value}` in `{name}`")]
    InvalidInteger {
        name: String,
        value: String,
        #[source]
        source: ParseIntError,
    },
    #[error("invalid bool `{value}` in `{name}`")]
    InvalidBool { name: String, value: String },
    #[error("reconstruct blend-space source: {0}")]
    Structure(&'static str),
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
    fn transforms_parametric_blend_space_to_authoring_ron() {
        let artifact = BlendSpaceSourceTransform
            .transform_with_motion_resolver(
                BlendSpaceSourceInput {
                    source_path: "Animations/Gameplay/Character/Npc/Mounts/Grunt/Blendspace/mount_grunt_movement_walk_blend.bspace",
                    bytes: sample_bspace(),
                },
                |name| match name {
                    "mount_grunt_walk_turn_r_downhill" => Some("animations/gameplay/character/npc/mounts/grunt/navigation/mount_grunt_walk_turn_r_downhill.anim.glb".to_string()),
                    "mount_grunt_walk" => Some("animations/gameplay/character/npc/mounts/grunt/navigation/mount_grunt_walk.anim.glb".to_string()),
                    _ => None,
                },
            )
            .unwrap();

        assert_eq!(
            artifact.path,
            "animations/gameplay/character/npc/mounts/grunt/blendspace/mount_grunt_movement_walk_blend.bspace.ron"
        );
        assert_eq!(artifact.schema, BLEND_SPACE_SOURCE_SCHEMA);

        let source = BlendSpaceSource::from_ron_bytes(&artifact.bytes).unwrap();
        assert_eq!(source.blend_space.dimensions.len(), 3);
        assert_eq!(source.blend_space.dimensions[0].name, "MoveSpeed");
        assert_eq!(source.blend_space.dimensions[0].parameter_id, Some(0));
        assert_eq!(source.blend_space.dimensions[1].parameter_id, Some(1));
        assert_eq!(source.blend_space.dimensions[2].parameter_id, Some(3));
        assert_eq!(source.blend_space.examples.len(), 2);
        assert_eq!(
            source.blend_space.examples[0]
                .animation
                .motion_path
                .as_deref(),
            Some(
                "animations/gameplay/character/npc/mounts/grunt/navigation/mount_grunt_walk_turn_r_downhill.anim.glb"
            )
        );
        assert_eq!(
            source.blend_space.examples[0].coordinates[0].value,
            Some(1.5)
        );
        assert_eq!(
            source.blend_space.examples[0].coordinates[1].value,
            Some(-2.5)
        );
        assert_eq!(
            source.blend_space.examples[0].coordinates[2].value,
            Some(-0.8)
        );
        assert_eq!(source.blend_space.annotations[0].indices, [0, 1]);
        assert_eq!(source.blend_space.virtual_examples[0].indices.len(), 8);
    }

    #[test]
    fn transforms_combined_blend_space_to_authoring_ron() {
        let artifact = BlendSpaceSourceTransform
            .transform(BlendSpaceSourceInput {
                source_path: "Animations/Gameplay/Character/Npc/Natural/Prey/Buffalo/Blendspaces/bison_turn_blendcomb.comb",
                bytes: sample_comb(),
            })
            .unwrap();

        assert_eq!(
            artifact.path,
            "animations/gameplay/character/npc/natural/prey/buffalo/blendspaces/bison_turn_blendcomb.comb.ron"
        );
        assert_eq!(artifact.schema, COMBINED_BLEND_SPACE_SOURCE_SCHEMA);

        let source = CombinedBlendSpaceSource::from_ron_bytes(&artifact.bytes).unwrap();
        assert_eq!(source.combined_blend_space.dimensions.len(), 1);
        assert_eq!(
            source.combined_blend_space.dimensions[0].name,
            "DesiredFacing"
        );
        assert_eq!(source.combined_blend_space.dimensions[0].parameter_id, None);
        assert_eq!(
            source.combined_blend_space.additional_extraction[0].parameter_id,
            Some(2)
        );
        assert_eq!(source.combined_blend_space.blend_spaces.len(), 2);
        assert_eq!(
            source.combined_blend_space.blend_spaces[0]
                .authoring_path
                .as_deref(),
            Some(
                "animations/gameplay/character/npc/natural/prey/buffalo/blendspaces/bison_turn_blend_right.bspace.ron"
            )
        );
    }

    #[test]
    fn rejects_coordinates_outside_declared_dimensions() {
        let error = BlendSpaceSource::from_legacy(
            "animations/test.bspace",
            br#"<ParaGroup>
 <Dimensions><Param Name="MoveSpeed"/></Dimensions>
 <ExampleList><Example AName="idle" SetPara1="1"/></ExampleList>
</ParaGroup>"#,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            BlendSpaceSourceError::CoordinateOutsideDimensions {
                attribute,
                dimension_count: 1
            } if attribute == "SetPara1"
        ));
    }

    fn sample_bspace() -> &'static [u8] {
        br#"<ParaGroup>
 <Dimensions>
  <Param Name="MoveSpeed" Min="1.5" Max="3.5" Cells="5"/>
  <Param Name="TurnSpeed" Min="-2.5" Max="2.5" Cells="5"/>
  <Param Name="TravelSlope" Min="-0.80000001" Max="0.80000001" Cells="5"/>
 </Dimensions>
 <ExampleList>
  <Example AName="mount_grunt_walk_turn_r_downhill" SetPara0="1.5" SetPara1="-2.5" SetPara2="-0.80000001" PlaybackScale="2.0590999"/>
  <Example AName="mount_grunt_walk" SetPara0="1.5" SetPara1="0" SetPara2="0" PlaybackScale="0.88"/>
 </ExampleList>
 <Blendable>
  <Face p0="0" p1="1"/>
 </Blendable>
 <VGrid>
  <VExample i0="0" i1="1" i2="0" i3="1" i4="0" i5="1" i6="0" i7="1" w0="0.5" w1="0.5" w2="0" w3="0" w4="0" w5="0" w6="0" w7="0"/>
 </VGrid>
</ParaGroup>"#
    }

    fn sample_comb() -> &'static [u8] {
        br#"<CombinedBlendSpace>
 <Dimensions>
  <Param Name="DesiredFacing" ParaScale="1" ChooseBlendSpace="1" Locked="1"/>
 </Dimensions>
 <AdditionalExtraction>
  <Param name="TravelAngle"/>
  <Param name="TurnSpeed"/>
  <Param name="MoveSpeed"/>
 </AdditionalExtraction>
 <BlendSpaces>
  <BlendSpace AName="animations/Gameplay/Character/Npc/Natural/Prey/Buffalo/Blendspaces/bison_turn_blend_right.bspace"/>
  <BlendSpace AName="animations/Gameplay/Character/Npc/Natural/Prey/Buffalo/BlendSpaces/bison_turn_blend_left.bspace"/>
 </BlendSpaces>
 <MotionCombination/>
 <JointList/>
</CombinedBlendSpace>"#
    }
}
