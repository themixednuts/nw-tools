//! CryAnimation-owned animation assets.
//!
//! CAF controller bytes remain owned by `cry-chunk`; this crate adds clip
//! identity and the engine-owned `.animevents` model. The latter cannot use the
//! reflected `AnimationEvent` projection because Cry's native string fields are
//! emitted as pointer-shaped bytes by SerializeContext codegen.

pub mod reflected;

use std::collections::BTreeMap;
use std::path::Path;

pub use cry_chunk::{
    CafAnimation, CafAnimationHeader, CafController, CafDecodeError, CafRootMotion, DbaArchive,
    DbaDecodeError, PositionKey, RotationKey, ScaleKey,
};
pub use cry_xml::XmlElement;
use nw_asset::{AssetDependencies, AssetDependency, AssetDependencyTarget};

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub source_path: String,
    pub name: String,
    pub caf: CafAnimation,
    pub events: Vec<AnimationEvent>,
}

impl AnimationClip {
    /// Decode a compiled or intermediate CAF. Both formats use the Cry chunk
    /// graph; their extension only identifies the compiler stage.
    ///
    /// # Errors
    ///
    /// Returns the underlying fail-closed CAF decoder error.
    pub fn parse(source_path: impl Into<String>, bytes: &[u8]) -> Result<Self, CafDecodeError> {
        let source_path = source_path.into();
        let name = animation_name(&source_path);
        Ok(Self {
            source_path,
            name,
            caf: CafAnimation::parse(bytes)?,
            events: Vec::new(),
        })
    }

    /// Decode every CAF stored in a compiled CryAnimation tracks database.
    /// Clip identity comes from the embedded CAF path, as it does in
    /// `LoaderDBA.cpp::ReadController905`.
    ///
    /// # Errors
    ///
    /// Returns the fail-closed DBA decoder error when the shared track tables,
    /// animation headers, controller references, or compressed values are invalid.
    pub fn parse_dba(bytes: &[u8]) -> Result<Vec<Self>, DbaDecodeError> {
        Ok(DbaArchive::parse(bytes)?
            .animations
            .into_iter()
            .map(|entry| Self {
                name: animation_name(&entry.file_path),
                source_path: entry.file_path,
                caf: entry.animation,
                events: Vec::new(),
            })
            .collect())
    }

    /// Attach every event whose animation path matches this clip.
    #[must_use]
    pub fn with_event_database(mut self, database: &AnimationEventDatabase) -> Self {
        self.events = database.events_for(&self.source_path).cloned().collect();
        self
    }
}

fn animation_name(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = file
        .strip_suffix(".i_caf")
        .or_else(|| file.strip_suffix(".caf"))
        .unwrap_or(file);
    stem.to_owned()
}

/// Exact `CAnimEventData` field model from CryAnimation.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEvent {
    pub normalized_time: f32,
    pub normalized_end_time: f32,
    pub name: String,
    pub name_lowercase_crc32: u32,
    pub parameter: String,
    pub bone: String,
    pub second_bone: String,
    pub offset: [f32; 3],
    pub direction: [f32; 3],
    pub model: String,
    /// Complete source event, including game-specific extension attributes.
    pub source: XmlElement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEventList {
    pub animation_path: String,
    pub attributes: BTreeMap<String, String>,
    pub events: Vec<AnimationEvent>,
    pub source: XmlElement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEventDatabase {
    pub source: XmlElement,
    pub animations: Vec<AnimationEventList>,
}

impl AnimationEventDatabase {
    /// Parse the `.animevents` XML consumed by
    /// `AnimEventLoader::LoadAnimationEventDatabase`.
    ///
    /// # Errors
    ///
    /// Fails closed on malformed numeric event fields rather than silently
    /// substituting a different event.
    pub fn from_xml(xml: &str) -> Result<Self, AnimationEventError> {
        let source = cry_xml::parse(xml)?;
        let mut animations = Vec::new();
        for animation in source.children_named("animation") {
            let animation_path = animation.attribute("name").unwrap_or_default().to_owned();
            let events = animation
                .children_named("event")
                .map(AnimationEvent::from_element)
                .collect::<Result<Vec<_>, _>>()?;
            animations.push(AnimationEventList {
                animation_path,
                attributes: animation.attributes.clone(),
                events,
                source: animation.clone(),
            });
        }
        Ok(Self { source, animations })
    }

    pub fn events_for<'a>(
        &'a self,
        animation_path: &str,
    ) -> impl Iterator<Item = &'a AnimationEvent> {
        let wanted = normalize_path(animation_path);
        self.animations
            .iter()
            .filter(move |list| normalize_path(&list.animation_path) == wanted)
            .flat_map(|list| &list.events)
    }
}

impl AssetDependencies for AnimationEventDatabase {
    fn asset_dependencies(&self) -> Vec<AssetDependency> {
        let mut dependencies = Vec::new();
        for list in &self.animations {
            if !list.animation_path.trim().is_empty() {
                dependencies.push(AssetDependency::optional_path(
                    "animation_event.clip",
                    &list.animation_path,
                ));
            }
            for event in &list.events {
                if !event.model.trim().is_empty() {
                    dependencies.push(AssetDependency::required_path(
                        "animation_event.model",
                        &event.model,
                    ));
                }
                if !event.parameter.trim().is_empty() {
                    dependencies.push(AssetDependency::optional(
                        format!("animation_event.{}", event.name.to_ascii_lowercase()),
                        AssetDependencyTarget::symbol(&event.parameter),
                    ));
                }
            }
        }
        dependencies
    }
}

impl AnimationEvent {
    fn from_element(element: &XmlElement) -> Result<Self, AnimationEventError> {
        let name = element
            .attribute("name")
            .unwrap_or("__unnamed__")
            .to_owned();
        let normalized_time = parse_optional_f32(element, "time")?
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let normalized_end_time = parse_optional_f32(element, "endTime")?
            .unwrap_or(normalized_time)
            .clamp(normalized_time, 1.0);
        Ok(Self {
            normalized_time,
            normalized_end_time,
            name_lowercase_crc32: crc32fast::hash(name.to_ascii_lowercase().as_bytes()),
            name,
            parameter: element
                .attribute("parameter")
                .unwrap_or_default()
                .to_owned(),
            bone: element.attribute("bone").unwrap_or_default().to_owned(),
            second_bone: element
                .attribute("secondBone")
                .unwrap_or_default()
                .to_owned(),
            offset: parse_optional_vec3(element, "offset")?.unwrap_or([0.0; 3]),
            direction: parse_optional_vec3(element, "dir")?.unwrap_or([0.0; 3]),
            model: element.attribute("model").unwrap_or_default().to_owned(),
            source: element.clone(),
        })
    }
}

fn parse_optional_f32(
    element: &XmlElement,
    attribute: &'static str,
) -> Result<Option<f32>, AnimationEventError> {
    let Some(value) = element.attribute(attribute) else {
        return Ok(None);
    };
    let parsed = value
        .parse::<f32>()
        .map_err(|_| AnimationEventError::InvalidNumber {
            attribute,
            value: value.to_owned(),
        })?;
    if !parsed.is_finite() {
        return Err(AnimationEventError::InvalidNumber {
            attribute,
            value: value.to_owned(),
        });
    }
    Ok(Some(parsed))
}

fn parse_optional_vec3(
    element: &XmlElement,
    attribute: &'static str,
) -> Result<Option<[f32; 3]>, AnimationEventError> {
    let Some(value) = element.attribute(attribute) else {
        return Ok(None);
    };
    let values = value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|component| !component.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AnimationEventError::InvalidVector {
            attribute,
            value: value.to_owned(),
        })?;
    let vector: [f32; 3] = values
        .try_into()
        .map_err(|_| AnimationEventError::InvalidVector {
            attribute,
            value: value.to_owned(),
        })?;
    if !vector.iter().all(|component| component.is_finite()) {
        return Err(AnimationEventError::InvalidVector {
            attribute,
            value: value.to_owned(),
        });
    }
    Ok(Some(vector))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[must_use]
pub fn is_animation_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let lowercase = name.to_ascii_lowercase();
            lowercase.ends_with(".caf") || lowercase.ends_with(".i_caf")
        })
}

#[derive(Debug, thiserror::Error)]
pub enum AnimationEventError {
    #[error(transparent)]
    Xml(#[from] cry_xml::XmlError),
    #[error("animation event attribute `{attribute}` is not a finite number: `{value}`")]
    InvalidNumber {
        attribute: &'static str,
        value: String,
    },
    #[error("animation event attribute `{attribute}` is not a finite Vec3: `{value}`")]
    InvalidVector {
        attribute: &'static str,
        value: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authoritative_event_fields_and_retains_extensions() {
        let database = AnimationEventDatabase::from_xml(
            r#"<anim_event_list custom="keep"><animation name="animations/hero/idle.caf">
                <event name="sound" time="0.25" endTime="0.5" parameter="play_foot"
                    bone="Bip01" secondBone="Bip02" offset="1, 2, 3" dir="0 1 0"
                    model="objects/fx.cgf" gameExtension="kept"/>
            </animation></anim_event_list>"#,
        )
        .unwrap();
        let event = database
            .events_for("ANIMATIONS\\HERO\\IDLE.CAF")
            .next()
            .unwrap();
        assert_eq!(event.normalized_time, 0.25);
        assert_eq!(event.normalized_end_time, 0.5);
        assert_eq!(event.offset, [1.0, 2.0, 3.0]);
        assert_eq!(event.source.attributes["gameExtension"], "kept");
        assert_ne!(event.name_lowercase_crc32, 0);
    }
}
