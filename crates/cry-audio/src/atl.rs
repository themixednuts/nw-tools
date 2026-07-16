//! Lossless ATL control projection.
//!
//! Ownership and element rules follow Lumberyard AudioSystem's ATL loader and
//! the New World `nw-audio-assets` legacy source transform. Known engine
//! fields are typed; every additional attribute is retained.

use serde::Serialize;
use thiserror::Error;

use cry_xml::XmlElement;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioControlsSource {
    pub source_path: String,
    pub name: Option<String>,
    pub root_attributes: Vec<AudioControlAttribute>,
    pub triggers: Vec<AudioTrigger>,
    pub rtpcs: Vec<AudioRtpc>,
    pub switches: Vec<AudioSwitch>,
    pub states: Vec<AudioSwitch>,
    pub environments: Vec<AudioEnvironment>,
    pub preloads: Vec<AudioPreload>,
}

impl AudioControlsSource {
    /// Parse a Lumberyard/New World `ATLConfig` document.
    ///
    /// Unknown structural elements fail closed. Unknown attributes are kept in
    /// the owning node so game-specific extensions are never discarded.
    pub fn from_xml(source_path: &str, xml: &str) -> Result<Self, AudioControlsError> {
        let root = cry_xml::parse(xml)?;
        if !tag_eq(&root.name, "ATLConfig") {
            return Err(AudioControlsError::UnexpectedRoot { name: root.name });
        }
        let mut source = Self {
            source_path: normalize_path(source_path),
            name: attr(&root, "atl_name").map(str::to_owned),
            root_attributes: extra_attrs(&root, &["atl_name"]),
            triggers: Vec::new(),
            rtpcs: Vec::new(),
            switches: Vec::new(),
            states: Vec::new(),
            environments: Vec::new(),
            preloads: Vec::new(),
        };
        for child in &root.children {
            if tag_eq(&child.name, "AudioTriggers") {
                source.triggers.extend(parse_named_section(
                    child,
                    "ATLTrigger",
                    AudioTrigger::from_element,
                )?);
            } else if tag_eq(&child.name, "AudioRtpcs") || tag_eq(&child.name, "AudioRTPCs") {
                source.rtpcs.extend(parse_named_section(
                    child,
                    "ATLRtpc",
                    AudioRtpc::from_element,
                )?);
            } else if tag_eq(&child.name, "AudioSwitches") {
                source.switches.extend(parse_named_section(
                    child,
                    "ATLSwitch",
                    AudioSwitch::from_element,
                )?);
            } else if tag_eq(&child.name, "AudioStates") {
                source.states.extend(parse_named_section(
                    child,
                    "ATLSwitch",
                    AudioSwitch::from_element,
                )?);
            } else if tag_eq(&child.name, "AudioEnvironments") {
                source.environments.extend(parse_named_section(
                    child,
                    "ATLEnvironment",
                    AudioEnvironment::from_element,
                )?);
            } else if tag_eq(&child.name, "AudioPreloads") {
                source.preloads.extend(parse_named_section(
                    child,
                    "ATLPreloadRequest",
                    AudioPreload::from_element,
                )?);
            } else {
                return Err(unexpected(&root.name, &child.name));
            }
        }
        Ok(source)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioTrigger {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub playback_info: Option<AudioTriggerPlaybackInfo>,
    pub requests: Vec<AudioBackendReference>,
}

impl AudioTrigger {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        let mut playback_info = None;
        let mut requests = Vec::new();
        for child in &element.children {
            if tag_eq(&child.name, "TriggerPlaybackInfo") {
                if playback_info
                    .replace(AudioTriggerPlaybackInfo::from_element(child)?)
                    .is_some()
                {
                    return Err(AudioControlsError::DuplicateElement {
                        parent: element.name.clone(),
                        name: child.name.clone(),
                    });
                }
            } else {
                requests.push(AudioBackendReference::from_element(child)?);
            }
        }
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            playback_info,
            requests,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioTriggerPlaybackInfo {
    pub max_radius: f64,
    pub max_duration: f64,
    pub attributes: Vec<AudioControlAttribute>,
}

impl AudioTriggerPlaybackInfo {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        ensure_no_children(element)?;
        Ok(Self {
            max_radius: required_f64_attr(element, "MaxRadius")?,
            max_duration: required_f64_attr(element, "MaxDuration")?,
            attributes: extra_attrs(element, &["MaxRadius", "MaxDuration"]),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioRtpc {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub requests: Vec<AudioBackendReference>,
}

impl AudioRtpc {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            requests: parse_backend_refs(element)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioSwitch {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub states: Vec<AudioSwitchState>,
}

impl AudioSwitch {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            states: parse_named_section(element, "ATLSwitchState", AudioSwitchState::from_element)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioSwitchState {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub requests: Vec<AudioBackendReference>,
}

impl AudioSwitchState {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            requests: parse_backend_refs(element)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioEnvironment {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub requests: Vec<AudioBackendReference>,
}

impl AudioEnvironment {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            requests: parse_backend_refs(element)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioPreload {
    pub name: Option<String>,
    pub auto_load: bool,
    pub attributes: Vec<AudioControlAttribute>,
    pub platforms: Vec<AudioPreloadPlatform>,
    pub config_groups: Vec<AudioPreloadConfigGroup>,
    pub files: Vec<AudioPreloadFile>,
}

impl AudioPreload {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        let mut platforms = Vec::new();
        let mut config_groups = Vec::new();
        let mut files = Vec::new();
        for child in &element.children {
            if tag_eq(&child.name, "ATLPlatforms") {
                platforms.extend(parse_named_section(
                    child,
                    "Platform",
                    AudioPreloadPlatform::from_element,
                )?);
            } else if tag_eq(&child.name, "ATLConfigGroup") {
                config_groups.push(AudioPreloadConfigGroup::from_element(child)?);
            } else if tag_eq(&child.name, "WwiseFile") {
                files.push(AudioPreloadFile::from_element(child)?);
            } else {
                return Err(unexpected(&element.name, &child.name));
            }
        }
        Ok(Self {
            name: attr(element, "atl_name").map(str::to_owned),
            auto_load: attr(element, "atl_type").is_some_and(|value| value == "AutoLoad"),
            attributes: extra_attrs(element, &["atl_name", "atl_type"]),
            platforms,
            config_groups,
            files,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioPreloadPlatform {
    pub name: String,
    pub config_group_name: Option<String>,
    pub attributes: Vec<AudioControlAttribute>,
}

impl AudioPreloadPlatform {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        ensure_no_children(element)?;
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            config_group_name: attr(element, "atl_config_group_name").map(str::to_owned),
            attributes: extra_attrs(element, &["atl_name", "atl_config_group_name"]),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioPreloadConfigGroup {
    pub name: String,
    pub attributes: Vec<AudioControlAttribute>,
    pub files: Vec<AudioPreloadFile>,
}

impl AudioPreloadConfigGroup {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        Ok(Self {
            name: required_attr(element, "atl_name")?.to_owned(),
            attributes: extra_attrs(element, &["atl_name"]),
            files: parse_named_section(element, "WwiseFile", AudioPreloadFile::from_element)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioPreloadFile {
    pub wwise_name: String,
    pub localized: Option<String>,
    pub attributes: Vec<AudioControlAttribute>,
}

impl AudioPreloadFile {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        ensure_no_children(element)?;
        Ok(Self {
            wwise_name: required_attr(element, "wwise_name")?.to_owned(),
            localized: attr(element, "wwise_localised").map(str::to_owned),
            attributes: extra_attrs(element, &["wwise_name", "wwise_localised"]),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioBackendReference {
    pub kind: AudioBackendReferenceKind,
    pub atl_name: Option<String>,
    pub wwise_name: Option<String>,
    pub attributes: Vec<AudioControlAttribute>,
    pub children: Vec<AudioBackendReference>,
}

impl AudioBackendReference {
    fn from_element(element: &XmlElement) -> Result<Self, AudioControlsError> {
        let kind = AudioBackendReferenceKind::from_tag(&element.name)
            .ok_or_else(|| unexpected("audio backend reference", &element.name))?;
        Ok(Self {
            kind,
            atl_name: attr(element, "atl_name").map(str::to_owned),
            wwise_name: attr(element, "wwise_name").map(str::to_owned),
            attributes: extra_attrs(element, &["atl_name", "wwise_name"]),
            children: element
                .children
                .iter()
                .map(Self::from_element)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioBackendReferenceKind {
    WwiseEvent,
    WwiseRtpc,
    WwiseSwitch,
    WwiseState,
    WwiseValue,
    WwiseAuxBus,
    AtlSwitchRequest,
    AtlValue,
}

impl AudioBackendReferenceKind {
    fn from_tag(tag: &str) -> Option<Self> {
        [
            ("WwiseEvent", Self::WwiseEvent),
            ("WwiseRtpc", Self::WwiseRtpc),
            ("WwiseSwitch", Self::WwiseSwitch),
            ("WwiseState", Self::WwiseState),
            ("WwiseValue", Self::WwiseValue),
            ("WwiseAuxBus", Self::WwiseAuxBus),
            ("ATLSwitchRequest", Self::AtlSwitchRequest),
            ("ATLValue", Self::AtlValue),
        ]
        .into_iter()
        .find_map(|(name, value)| tag_eq(tag, name).then_some(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioControlAttribute {
    pub name: String,
    pub value: String,
}

fn parse_named_section<T>(
    element: &XmlElement,
    expected: &str,
    mut parser: impl FnMut(&XmlElement) -> Result<T, AudioControlsError>,
) -> Result<Vec<T>, AudioControlsError> {
    element
        .children
        .iter()
        .map(|child| {
            if tag_eq(&child.name, expected) {
                parser(child)
            } else {
                Err(unexpected(&element.name, &child.name))
            }
        })
        .collect()
}

fn parse_backend_refs(
    element: &XmlElement,
) -> Result<Vec<AudioBackendReference>, AudioControlsError> {
    element
        .children
        .iter()
        .map(AudioBackendReference::from_element)
        .collect()
}

fn ensure_no_children(element: &XmlElement) -> Result<(), AudioControlsError> {
    if let Some(child) = element.children.first() {
        Err(unexpected(&element.name, &child.name))
    } else {
        Ok(())
    }
}

fn attr<'a>(element: &'a XmlElement, name: &str) -> Option<&'a str> {
    element
        .attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn required_attr<'a>(
    element: &'a XmlElement,
    name: &'static str,
) -> Result<&'a str, AudioControlsError> {
    attr(element, name).ok_or_else(|| AudioControlsError::MissingAttribute {
        element: element.name.clone(),
        name,
    })
}

fn required_f64_attr(element: &XmlElement, name: &'static str) -> Result<f64, AudioControlsError> {
    let value = required_attr(element, name)?;
    let parsed = value
        .parse::<f64>()
        .map_err(|_| AudioControlsError::InvalidNumber {
            element: element.name.clone(),
            name,
            value: value.to_owned(),
        })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(AudioControlsError::InvalidNumber {
            element: element.name.clone(),
            name,
            value: value.to_owned(),
        })
    }
}

fn extra_attrs(element: &XmlElement, known: &[&str]) -> Vec<AudioControlAttribute> {
    element
        .attributes
        .iter()
        .filter(|(name, _)| !known.iter().any(|known| name.eq_ignore_ascii_case(known)))
        .map(|(name, value)| AudioControlAttribute {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

fn tag_eq(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

fn unexpected(parent: &str, name: &str) -> AudioControlsError {
    AudioControlsError::UnexpectedElement {
        parent: parent.to_owned(),
        name: name.to_owned(),
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[derive(Debug, Error)]
pub enum AudioControlsError {
    #[error(transparent)]
    Xml(#[from] cry_xml::XmlError),
    #[error("ATL control document has unexpected root {name}")]
    UnexpectedRoot { name: String },
    #[error("unexpected ATL element {name} under {parent}")]
    UnexpectedElement { parent: String, name: String },
    #[error("ATL element {element} is missing attribute {name}")]
    MissingAttribute { element: String, name: &'static str },
    #[error("ATL element {element} has invalid number {name}={value}")]
    InvalidNumber {
        element: String,
        name: &'static str,
        value: String,
    },
    #[error("ATL element {parent} contains duplicate {name}")]
    DuplicateElement { parent: String, name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_atl_sections_and_retains_extension_attributes() {
        let source = AudioControlsSource::from_xml(
            "Libs/GameAudio/Wwise/atl_controls.xml",
            r#"<ATLConfig atl_name="main" game="nw">
                <AudioTriggers><ATLTrigger atl_name="play" custom="yes">
                  <TriggerPlaybackInfo MaxRadius="10" MaxDuration="2.5" extra="x"/>
                  <WwiseEvent wwise_name="Play_Event"/>
                </ATLTrigger></AudioTriggers>
                <AudioRtpcs><ATLRtpc atl_name="volume"><WwiseRtpc wwise_name="Volume"/></ATLRtpc></AudioRtpcs>
                <AudioSwitches><ATLSwitch atl_name="surface"><ATLSwitchState atl_name="snow"><WwiseSwitch wwise_name="Surface"><WwiseValue wwise_name="Snow"/></WwiseSwitch></ATLSwitchState></ATLSwitch></AudioSwitches>
                <AudioStates><ATLSwitch atl_name="music"><ATLSwitchState atl_name="combat"><WwiseState wwise_name="Music"><WwiseValue wwise_name="Combat"/></WwiseState></ATLSwitchState></ATLSwitch></AudioStates>
                <AudioEnvironments><ATLEnvironment atl_name="cave"><WwiseAuxBus wwise_name="Cave"/></ATLEnvironment></AudioEnvironments>
                <AudioPreloads><ATLPreloadRequest atl_name="boot" atl_type="AutoLoad"><ATLConfigGroup atl_name="pc"><WwiseFile wwise_name="init.bnk" wwise_localised="false"/></ATLConfigGroup></ATLPreloadRequest></AudioPreloads>
              </ATLConfig>"#,
        )
        .unwrap();

        assert_eq!(source.source_path, "libs/gameaudio/wwise/atl_controls.xml");
        assert_eq!(source.root_attributes[0].name, "game");
        assert_eq!(
            source.triggers[0].requests[0].kind,
            AudioBackendReferenceKind::WwiseEvent
        );
        assert_eq!(
            source.switches[0].states[0].requests[0].children[0]
                .wwise_name
                .as_deref(),
            Some("Snow")
        );
        assert!(source.preloads[0].auto_load);
        assert_eq!(
            source.preloads[0].config_groups[0].files[0].wwise_name,
            "init.bnk"
        );
    }
}
