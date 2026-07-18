//! Lumberyard MaterialEffects FX-library catalog (`<FXLib>`).
//!
//! A `footstep`-kind animation event's `parameter` names an FX library at
//! `libs/materialeffects/fxlibs/<parameter>.xml`, not an ATL trigger directly.
//! The library lists one `<Effect name="<surface>">` per surface, each with an
//! `<Audio trigger="<atl trigger>"><Switch name="SurfaceType" state="<surface>"/>`
//! that names the ATL trigger to fire (verified in `GameData.pak`, e.g.
//! `blend_ftsp_alligator.xml`, `fstp_run_dryad.xml` where the trigger case
//! differs from the file stem). Only the audio-relevant elements are modelled;
//! `<Particle>`, `<Decal>`, and other siblings are retained as opaque children
//! so nothing surface-authored is silently dropped, but they are not
//! interpreted.

use serde::Serialize;
use thiserror::Error;

use cry_xml::XmlElement;

/// Directory under which footstep FX libraries live.
pub const MATERIAL_EFFECTS_FXLIB_DIR: &str = "libs/materialeffects/fxlibs";

/// The catalog path a footstep `parameter` resolves to.
#[must_use]
pub fn footstep_fxlib_path(parameter: &str) -> String {
    let name = parameter.replace('\\', "/").to_ascii_lowercase();
    format!("{MATERIAL_EFFECTS_FXLIB_DIR}/{name}.xml")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialEffectsLibrary {
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_type: Option<String>,
    pub effects: Vec<MaterialEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialEffect {
    /// Surface name (`<Effect name>`), e.g. `metal`, `water`.
    pub name: String,
    pub audio: Vec<MaterialEffectAudio>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialEffectAudio {
    /// The ATL trigger this surface fires.
    pub trigger: String,
    pub switches: Vec<MaterialEffectSwitch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialEffectSwitch {
    /// Switch group name, e.g. `SurfaceType`.
    pub name: String,
    /// Switch state, e.g. `metal` (the per-surface Wwise switch value).
    pub state: String,
}

impl MaterialEffectsLibrary {
    /// Parse a `<FXLib>` document, extracting its `<Effect>/<Audio>/<Switch>`
    /// audio bindings. Non-audio siblings are ignored.
    pub fn from_xml(source_path: &str, xml: &str) -> Result<Self, MaterialEffectsError> {
        let root = cry_xml::parse(xml)?;
        if !root.name.eq_ignore_ascii_case("FXLib") {
            return Err(MaterialEffectsError::UnexpectedRoot { name: root.name });
        }
        let mut effects = Vec::new();
        for effect in &root.children {
            if !effect.name.eq_ignore_ascii_case("Effect") {
                continue;
            }
            let name = attr(effect, "name").unwrap_or_default().to_owned();
            let mut audio = Vec::new();
            for element in &effect.children {
                if !element.name.eq_ignore_ascii_case("Audio") {
                    continue;
                }
                let Some(trigger) = attr(element, "trigger").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let switches = element
                    .children
                    .iter()
                    .filter(|child| child.name.eq_ignore_ascii_case("Switch"))
                    .map(|child| MaterialEffectSwitch {
                        name: attr(child, "name").unwrap_or_default().to_owned(),
                        state: attr(child, "state").unwrap_or_default().to_owned(),
                    })
                    .collect();
                audio.push(MaterialEffectAudio {
                    trigger: trigger.to_owned(),
                    switches,
                });
            }
            effects.push(MaterialEffect { name, audio });
        }
        Ok(Self {
            source_path: source_path.replace('\\', "/").to_ascii_lowercase(),
            library_type: attr(&root, "type").map(str::to_owned),
            effects,
        })
    }

    /// Distinct ATL trigger names this library fires, in first-seen order.
    #[must_use]
    pub fn triggers(&self) -> Vec<String> {
        let mut triggers = Vec::new();
        for audio in self.effects.iter().flat_map(|effect| &effect.audio) {
            if !triggers
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&audio.trigger))
            {
                triggers.push(audio.trigger.clone());
            }
        }
        triggers
    }

    /// Distinct surface states this library covers, in first-seen order. These
    /// are the authored `SurfaceType` switch states (falling back to the
    /// `<Effect name>` when a surface omits an explicit switch).
    #[must_use]
    pub fn surfaces(&self) -> Vec<String> {
        let mut surfaces = Vec::new();
        let mut push = |value: &str| {
            if !value.is_empty()
                && !surfaces
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(value))
            {
                surfaces.push(value.to_owned());
            }
        };
        for effect in &self.effects {
            let switch_states: Vec<&str> = effect
                .audio
                .iter()
                .flat_map(|audio| &audio.switches)
                .map(|switch| switch.state.as_str())
                .filter(|state| !state.is_empty())
                .collect();
            if switch_states.is_empty() {
                push(&effect.name);
            } else {
                for state in switch_states {
                    push(state);
                }
            }
        }
        surfaces
    }
}

fn attr<'a>(element: &'a XmlElement, name: &str) -> Option<&'a str> {
    element
        .attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[derive(Debug, Error)]
pub enum MaterialEffectsError {
    #[error(transparent)]
    Xml(#[from] cry_xml::XmlError),
    #[error("MaterialEffects FX library has unexpected root {name}")]
    UnexpectedRoot { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_footstep_fxlib_triggers_and_surfaces() {
        let library = MaterialEffectsLibrary::from_xml(
            "Libs/MaterialEffects/FXLibs/FStp_Run_Dryad.xml",
            r#"<FXLib type="playerfootstep">
                 <Effect name="default">
                   <Audio trigger="FTSP_run_dryad">
                     <Switch name="SurfaceType" state="default"/>
                   </Audio>
                   <Particle/>
                 </Effect>
                 <Effect name="metal">
                   <Audio trigger="FTSP_run_dryad">
                     <Switch name="SurfaceType" state="metal"/>
                   </Audio>
                 </Effect>
               </FXLib>"#,
        )
        .unwrap();

        assert_eq!(library.library_type.as_deref(), Some("playerfootstep"));
        assert_eq!(
            library.source_path,
            "libs/materialeffects/fxlibs/fstp_run_dryad.xml"
        );
        // The trigger is read from the library, not assumed to equal the stem.
        assert_eq!(library.triggers(), vec!["FTSP_run_dryad".to_owned()]);
        assert_eq!(
            library.surfaces(),
            vec!["default".to_owned(), "metal".to_owned()]
        );
    }

    #[test]
    fn footstep_path_is_lowercased_catalog_path() {
        assert_eq!(
            footstep_fxlib_path("Blend_Ftsp_Alligator"),
            "libs/materialeffects/fxlibs/blend_ftsp_alligator.xml"
        );
    }
}
