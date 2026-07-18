//! Typed access to Mannequin fragment audio.
//!
//! Creature vocal/action sounds (bite, hurt, tail whip, body falls) are authored
//! as `ProcLayer` procedural clips inside Mannequin fragments — not as
//! CryAnimation animevents. Two procedural types carry audio:
//!
//! - `type="Audio"` clips name an ATL `StartTrigger`, an optional `StopTrigger`,
//!   the `AttachmentJoint` the sound rides, and inherit the owning `ProcLayer`
//!   `<Blend StartTime>` timing (footsteps, hurt, tail whip, body falls).
//! - `type="CharacterEvent"` clips carry an authored `CharacterEventName` (`Bite`,
//!   `VOX_Attack1`) for a coattached receiver script plus the authored
//!   `AttachmentJoint`. Each owning `ProcLayer` schedules its clips
//!   independently: the first `<Blend ExitTime>` is clamped to zero and each later
//!   value is a transition delay accumulated onto the prior entry. The parser
//!   preserves event names; dispatch naming remains receiver-owned.
//!
//! A single logical fragment (e.g. `r_Death`) is assembled from several ADBs:
//! the animation database contributes the `AnimLayer` animation while a sibling
//! audio database contributes the `ProcLayer` audio for the same fragment name.
//! Callers merge [`MannequinFragmentAudio`] across databases by fragment name to
//! pair each animation with the sounds it fires.

use serde::{Deserialize, Serialize};

use crate::source_transform::{
    MannequinAnimationDatabase, MannequinFragment, MannequinProcedural, MannequinProceduralLayer,
};

/// The Mannequin procedural `type` that fires an ATL audio trigger.
pub const AUDIO_PROCEDURAL_TYPE: &str = "Audio";

/// The Mannequin procedural `type` that fires a named character event for a
/// coattached receiver script to interpret.
pub const CHARACTER_EVENT_PROCEDURAL_TYPE: &str = "CharacterEvent";

/// Sentinel `StopTrigger` value meaning "do not stop" — never a real ATL trigger,
/// so it is dropped from [`MannequinAudioClip::stop_trigger`].
pub const DO_NOTHING_TRIGGER: &str = "do_nothing";

const START_TRIGGER_PARAM: &str = "StartTrigger";
const STOP_TRIGGER_PARAM: &str = "StopTrigger";
const ATTACHMENT_JOINT_PARAM: &str = "AttachmentJoint";
const CHARACTER_EVENT_NAME_PARAM: &str = "CharacterEventName";
const OPTION_ON_ENTER_PARAM: &str = "OptionOnEnter";
const OPTION_ON_EXIT_PARAM: &str = "OptionOnExit";

/// Which procedural type authored an audio clip, so a caller can resolve each the
/// right way: an [`AtlTrigger`](MannequinAudioKind::AtlTrigger) `trigger` is an ATL
/// control name, while a [`CharacterEvent`](MannequinAudioKind::CharacterEvent)
/// `trigger` is the authored `CharacterEventName` interpreted by the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MannequinAudioKind {
    /// `type="Audio"` clip firing an ATL `StartTrigger` directly.
    #[default]
    AtlTrigger,
    /// `type="CharacterEvent"` clip carrying a short `CharacterEventName`.
    CharacterEvent,
}

/// Dispatch behavior authored by a CharacterEvent procedural option.
///
/// Unknown serialized values normalize to [`NoEffect`](Self::NoEffect), matching
/// the runtime's drop-not-guess behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MannequinCharacterEventOption {
    /// Dispatch the CharacterEvent with `shouldPlay=true`.
    Enable,
    /// Dispatch the CharacterEvent with `shouldPlay=false`.
    Disable,
    /// Do not dispatch a CharacterEvent for this phase.
    NoEffect,
}

impl MannequinCharacterEventOption {
    fn from_serialized(value: &str) -> Self {
        // Shipped ADBs author the symbolic enum names (`Enable`/`Disable`);
        // the numeric spellings are the same enum's serialized values
        // (Enable=0, Disable=1, NoEffect=2). Anything else dispatches nothing.
        match value.trim() {
            "Enable" | "0" => Self::Enable,
            "Disable" | "1" => Self::Disable,
            _ => Self::NoEffect,
        }
    }
}

/// One `ProcLayer` audio clip: the trigger/short name it fires, the joint it
/// attaches to, and the layer-blend time (seconds, relative to the fragment's
/// animation start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAudioClip {
    /// For an [`AtlTrigger`](MannequinAudioKind::AtlTrigger) clip, the ATL
    /// `StartTrigger` control name (e.g. `play_vox_alligator_hurt`). For a
    /// [`CharacterEvent`](MannequinAudioKind::CharacterEvent) clip, the authored
    /// `CharacterEventName` (e.g. `Bite`, `VOX_Attack1`) interpreted by the
    /// coattached receiver script.
    pub trigger: String,
    /// ATL `StopTrigger`, when it is a real trigger (the `do_nothing` sentinel is
    /// dropped). Never set for a `CharacterEvent` clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_trigger: Option<String>,
    /// Authored `AttachmentJoint` string, with surrounding whitespace trimmed.
    pub joint: String,
    /// Absolute entry time in seconds relative to the fragment animation.
    ///
    /// An `Audio` clip keeps its authored `StartTime`. A `CharacterEvent` clip
    /// receives the cumulative entry time of its owning `ProcLayer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f32>,
    /// Absolute CharacterEvent exit time, equal to the next procedural entry in
    /// the same `ProcLayer`. The final clip has no exit time. Always absent for
    /// direct `Audio` clips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_time: Option<f32>,
    /// CharacterEvent OnEnter behavior. Direct `Audio` clips have no value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_on_enter: Option<MannequinCharacterEventOption>,
    /// CharacterEvent OnExit behavior. Direct `Audio` clips have no value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_on_exit: Option<MannequinCharacterEventOption>,
    /// Zero-based owning `ProcLayer` ordinal within the fragment variant.
    #[serde(default)]
    pub proc_layer_ordinal: usize,
    /// Zero-based procedural ordinal within the owning `ProcLayer`.
    #[serde(default)]
    pub procedural_ordinal: usize,
    /// Which procedural type authored this clip.
    #[serde(default)]
    pub kind: MannequinAudioKind,
}

/// The AnimLayer animations and ProcLayer audio clips of one Mannequin fragment
/// group, gathered for pairing across animation/audio databases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinFragmentAudio {
    /// Fragment group name (e.g. `Attack_Bite`, `r_Death`).
    pub fragment: String,
    /// The first non-empty fragment `Tags` string, when authored.
    pub tags: String,
    /// AnimLayer `<Animation name>` entries this fragment plays.
    pub animations: Vec<String>,
    /// ProcLayer `type="Audio"` and `type="CharacterEvent"` clips this fragment
    /// fires.
    pub clips: Vec<MannequinAudioClip>,
}

impl MannequinAnimationDatabase {
    /// Typed access to every fragment's AnimLayer animations paired with its
    /// ProcLayer audio clips.
    ///
    /// One entry is produced per `FragmentList` group that carries at least one
    /// animation or one audio clip. Fragments in a group are merged (a group may
    /// hold several tag variants), so a group that only supplies audio (a sibling
    /// audio database) still appears — its animation comes from the matching group
    /// in the animation database, resolved by the caller.
    #[must_use]
    pub fn fragment_audio(&self) -> Vec<MannequinFragmentAudio> {
        let mut out = Vec::new();
        for group in &self.fragment_groups {
            let mut tags = String::new();
            let mut animations = Vec::new();
            let mut clips = Vec::new();
            for fragment in &group.fragments {
                if tags.is_empty()
                    && let Some(fragment_tags) =
                        fragment.tags.as_deref().filter(|value| !value.is_empty())
                {
                    tags = fragment_tags.to_owned();
                }
                collect_fragment_animations(fragment, &mut animations);
                collect_fragment_audio(fragment, &mut clips);
            }
            if animations.is_empty() && clips.is_empty() {
                continue;
            }
            out.push(MannequinFragmentAudio {
                fragment: group.name.clone(),
                tags,
                animations,
                clips,
            });
        }
        out
    }
}

fn collect_fragment_animations(fragment: &MannequinFragment, out: &mut Vec<String>) {
    for layer in &fragment.animation_layers {
        for animation in &layer.animations {
            let name = animation.name.trim();
            if !name.is_empty()
                && !out
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(name))
            {
                out.push(name.to_owned());
            }
        }
    }
}

fn collect_fragment_audio(fragment: &MannequinFragment, out: &mut Vec<MannequinAudioClip>) {
    for (proc_layer_ordinal, layer) in fragment.procedural_layers.iter().enumerate() {
        let entry_times = procedural_entry_times(layer);
        // The XML alternates `<Blend/>` then `<Procedural/>`, so the procedural at
        // index `i` inherits the blend at index `i`.
        for (procedural_ordinal, (procedural, &entry_time)) in
            layer.procedurals.iter().zip(&entry_times).enumerate()
        {
            let blend = layer.blends.get(procedural_ordinal);
            if procedural.ty.eq_ignore_ascii_case(AUDIO_PROCEDURAL_TYPE) {
                let Some(trigger) = param(procedural, START_TRIGGER_PARAM)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let stop_trigger = param(procedural, STOP_TRIGGER_PARAM)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| {
                        !value.is_empty() && !value.eq_ignore_ascii_case(DO_NOTHING_TRIGGER)
                    });
                let joint = param(procedural, ATTACHMENT_JOINT_PARAM)
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_default();
                // Direct Audio timing remains the authored blend StartTime.
                let start_time = blend.and_then(|blend| blend.start_time);
                out.push(MannequinAudioClip {
                    trigger,
                    stop_trigger,
                    joint,
                    start_time,
                    exit_time: None,
                    option_on_enter: None,
                    option_on_exit: None,
                    proc_layer_ordinal,
                    procedural_ordinal,
                    kind: MannequinAudioKind::AtlTrigger,
                });
            } else if procedural
                .ty
                .eq_ignore_ascii_case(CHARACTER_EVENT_PROCEDURAL_TYPE)
            {
                let Some(event_name) = param(procedural, CHARACTER_EVENT_NAME_PARAM)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let joint = param(procedural, ATTACHMENT_JOINT_PARAM)
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_default();
                out.push(MannequinAudioClip {
                    trigger: event_name,
                    stop_trigger: None,
                    joint,
                    start_time: Some(entry_time),
                    exit_time: entry_times.get(procedural_ordinal + 1).copied(),
                    option_on_enter: Some(character_event_option(
                        procedural,
                        OPTION_ON_ENTER_PARAM,
                        MannequinCharacterEventOption::Enable,
                    )),
                    option_on_exit: Some(character_event_option(
                        procedural,
                        OPTION_ON_EXIT_PARAM,
                        MannequinCharacterEventOption::Disable,
                    )),
                    proc_layer_ordinal,
                    procedural_ordinal,
                    kind: MannequinAudioKind::CharacterEvent,
                });
            }
        }
    }
}

fn procedural_entry_times(layer: &MannequinProceduralLayer) -> Vec<f32> {
    let mut entry_time = 0.0;
    layer
        .procedurals
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let delay = layer
                .blends
                .get(index)
                .and_then(|blend| blend.exit_time)
                .unwrap_or_default();
            entry_time = if index == 0 {
                delay.max(0.0)
            } else {
                entry_time + delay
            };
            entry_time
        })
        .collect()
}

fn character_event_option(
    procedural: &MannequinProcedural,
    name: &str,
    default: MannequinCharacterEventOption,
) -> MannequinCharacterEventOption {
    param(procedural, name)
        .map(MannequinCharacterEventOption::from_serialized)
        .unwrap_or(default)
}

fn param<'a>(procedural: &'a MannequinProcedural, name: &str) -> Option<&'a str> {
    procedural
        .parameters
        .iter()
        .find(|parameter| parameter.name.eq_ignore_ascii_case(name))
        .and_then(|parameter| parameter.value.as_deref())
}

#[cfg(test)]
mod tests {
    use crate::source_transform::MannequinAnimationDatabaseSource;

    // Synthetic direct-Audio fixture; `bind_mouth_web` is intentional here and
    // does not describe the authored alligator CharacterEvent Bite clip.
    const SYNTHETIC_AUDIO_SAMPLE: &[u8] = br#"
        <AnimDB FragDef="actions.xml" TagDef="tags.xml">
          <FragmentList>
            <r_R0_B>
              <Fragment BlendOutDuration="0.2" Tags="">
                <AnimLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Animation name="alligator_r_r0_b"/>
                </AnimLayer>
                <ProcLayer>
                  <Blend ExitTime="9" StartTime="0.25" Duration="0"/>
                  <Procedural type="Audio" contextType="AudioContext">
                    <ProceduralParams>
                      <StartTrigger value="play_vox_alligator_hurt"/>
                      <StopTrigger value="do_nothing"/>
                      <AttachmentJoint value="bind_mouth_web"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Procedural type="Audio" contextType="AudioContext">
                    <ProceduralParams>
                      <StartTrigger value="play_sfx_aligator_tail_whip_fast"/>
                      <StopTrigger value="do_nothing"/>
                      <AttachmentJoint value="bind_tail_05"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </r_R0_B>
            <r_Death>
              <Fragment BlendOutDuration="0.2" Tags="">
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Procedural type="Audio" contextType="AudioContext">
                    <ProceduralParams>
                      <StartTrigger value="play_bodyfall_big"/>
                      <StopTrigger value="do_nothing"/>
                      <AttachmentJoint value="bind_pelvis"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </r_Death>
            <Idle>
              <Fragment BlendOutDuration="0.2" Tags="">
                <AnimLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0.4"/>
                  <Animation name="alligator_idle" flags="Loop"/>
                </AnimLayer>
              </Fragment>
            </Idle>
          </FragmentList>
        </AnimDB>
    "#;

    #[test]
    fn extracts_audio_clips_and_animations_per_fragment() {
        let database =
            MannequinAnimationDatabaseSource::from_legacy("test.adb", SYNTHETIC_AUDIO_SAMPLE)
                .unwrap()
                .database;
        let fragments = database.fragment_audio();

        let bite = fragments.iter().find(|f| f.fragment == "r_R0_B").unwrap();
        assert_eq!(bite.animations, ["alligator_r_r0_b"]);
        assert_eq!(bite.clips.len(), 2);
        assert_eq!(bite.clips[0].trigger, "play_vox_alligator_hurt");
        // The `do_nothing` sentinel StopTrigger is dropped.
        assert_eq!(bite.clips[0].stop_trigger, None);
        assert_eq!(bite.clips[0].joint, "bind_mouth_web");
        assert_eq!(bite.clips[0].start_time, Some(0.25));
        assert_eq!(bite.clips[0].exit_time, None);
        assert_eq!(bite.clips[0].option_on_enter, None);
        assert_eq!(bite.clips[0].option_on_exit, None);
        assert_eq!(bite.clips[0].proc_layer_ordinal, 0);
        assert_eq!(bite.clips[0].procedural_ordinal, 0);
        assert_eq!(bite.clips[1].trigger, "play_sfx_aligator_tail_whip_fast");
        assert_eq!(bite.clips[1].joint, "bind_tail_05");
        assert_eq!(bite.clips[1].start_time, Some(0.0));
        assert_eq!(bite.clips[1].proc_layer_ordinal, 1);
        assert_eq!(bite.clips[1].procedural_ordinal, 0);

        // An audio-only fragment (no AnimLayer) still surfaces so the animation
        // database's matching fragment can supply the animation by name.
        let death = fragments.iter().find(|f| f.fragment == "r_Death").unwrap();
        assert!(death.animations.is_empty());
        assert_eq!(death.clips.len(), 1);
        assert_eq!(death.clips[0].trigger, "play_bodyfall_big");

        // A pure-animation fragment surfaces too (with no clips): the merge keys
        // on fragment name, so the animation must be visible even when the audio
        // lives in a sibling database.
        let idle = fragments.iter().find(|f| f.fragment == "Idle").unwrap();
        assert_eq!(idle.animations, ["alligator_idle"]);
        assert!(idle.clips.is_empty());

        // Every collected clip is tagged as an ATL trigger.
        assert!(
            bite.clips
                .iter()
                .all(|clip| clip.kind == super::MannequinAudioKind::AtlTrigger)
        );
    }

    // The real `npc_alligator_audio.adb` Attack_Bite shape. ExitTime is a
    // transition delay within each independent ProcLayer, and the Bite clip's
    // authored AttachmentJoint is empty.
    const ALLIGATOR_CHARACTER_EVENT_SAMPLE: &[u8] = br#"
        <AnimDB FragDef="actions.xml" TagDef="tags.xml">
          <FragmentList>
            <Attack_Bite>
              <Fragment BlendOutDuration="0.2" Tags="">
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0" CurveType="0"/>
                  <Procedural type="CharacterEvent" contextType="CharacterEventContext">
                    <ProceduralParams>
                      <CharacterEventName value="VOX_Chatters"/>
                      <AttachmentJoint value=""/>
                      <SoundObstructionType value="UseLinkedProxy"/>
                      <OptionOnEnter value="Enable"/>
                      <OptionOnExit value="Disable"/>
                    </ProceduralParams>
                  </Procedural>
                  <Blend ExitTime="0.69999999" StartTime="0" Duration="0" CurveType="0"/>
                  <Procedural type="CharacterEvent" contextType="CharacterEventContext">
                    <ProceduralParams>
                      <CharacterEventName value="VOX_Attack1"/>
                      <AttachmentJoint value=""/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
                <ProcLayer>
                  <Blend ExitTime="0.60000002" StartTime="0" Duration="0" CurveType="0"/>
                  <Procedural type="CharacterEvent" contextType="CharacterEventContext">
                    <ProceduralParams>
                      <CharacterEventName value="Bite"/>
                      <AttachmentJoint value=""/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </Attack_Bite>
          </FragmentList>
        </AnimDB>
    "#;

    #[test]
    #[allow(clippy::excessive_precision)]
    fn extracts_authored_alligator_character_events() {
        use super::{MannequinAudioKind, MannequinCharacterEventOption};

        let database = MannequinAnimationDatabaseSource::from_legacy(
            "audio.adb",
            ALLIGATOR_CHARACTER_EVENT_SAMPLE,
        )
        .unwrap()
        .database;
        let fragments = database.fragment_audio();

        let bite = fragments
            .iter()
            .find(|fragment| fragment.fragment == "Attack_Bite")
            .unwrap();
        assert_eq!(bite.clips.len(), 3);
        assert!(
            bite.clips
                .iter()
                .all(|clip| clip.kind == MannequinAudioKind::CharacterEvent)
        );
        assert!(bite.clips.iter().all(|clip| clip.stop_trigger.is_none()));

        let chatters = &bite.clips[0];
        assert_eq!(chatters.trigger, "VOX_Chatters");
        assert_eq!(chatters.start_time, Some(0.0));
        assert_eq!(chatters.exit_time, Some(0.69999999));
        // Shipped ADBs author the symbolic enum labels.
        assert_eq!(
            chatters.option_on_enter,
            Some(MannequinCharacterEventOption::Enable)
        );
        assert_eq!(
            chatters.option_on_exit,
            Some(MannequinCharacterEventOption::Disable)
        );
        assert_eq!(chatters.proc_layer_ordinal, 0);
        assert_eq!(chatters.procedural_ordinal, 0);

        let attack = &bite.clips[1];
        assert_eq!(attack.trigger, "VOX_Attack1");
        assert_eq!(attack.start_time, Some(0.69999999));
        assert_eq!(attack.exit_time, None);
        assert_eq!(
            attack.option_on_enter,
            Some(MannequinCharacterEventOption::Enable)
        );
        assert_eq!(
            attack.option_on_exit,
            Some(MannequinCharacterEventOption::Disable)
        );
        assert_eq!(attack.proc_layer_ordinal, 0);
        assert_eq!(attack.procedural_ordinal, 1);

        let hit = &bite.clips[2];
        assert_eq!(hit.trigger, "Bite");
        assert_eq!(hit.start_time, Some(0.60000002));
        assert_eq!(hit.exit_time, None);
        assert_eq!(hit.joint, "");
        assert_eq!(hit.proc_layer_ordinal, 1);
        assert_eq!(hit.procedural_ordinal, 0);
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn schedules_attack_running_bite_delays_cumulatively() {
        let database = parse_audio(
            br#"
                <ProcLayer>
                  <Blend ExitTime="0.010827571" StartTime="99"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="VOX_Chatters"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.81917238" StartTime="99"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="VOX_Attack1"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.47000003" StartTime="99"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Ground_Slam"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.66000009" StartTime="99"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Bite"/>
                  </ProceduralParams></Procedural>
                </ProcLayer>
            "#,
        );
        let clips = &database.fragment_audio()[0].clips;

        assert_eq!(
            clips
                .iter()
                .map(|clip| clip.start_time.unwrap())
                .collect::<Vec<_>>(),
            [0.010827571, 0.829999951, 1.299999981, 1.960000071]
        );
        assert_eq!(clips[0].exit_time, clips[1].start_time);
        assert_eq!(clips[1].exit_time, clips[2].start_time);
        assert_eq!(clips[2].exit_time, clips[3].start_time);
        assert_eq!(clips[3].exit_time, None);
    }

    #[test]
    fn parses_character_event_options_and_defaults() {
        use super::MannequinCharacterEventOption::{Disable, Enable, NoEffect};

        // Synthetic fixture: bind_mouth_web verifies that an authored nonempty
        // joint remains intact without implying that the real alligator Bite uses it.
        let database = parse_audio(
            br#"
                <ProcLayer>
                  <Blend ExitTime="0"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Explicit"/>
                    <AttachmentJoint value="bind_mouth_web"/>
                    <OptionOnEnter value="0"/>
                    <OptionOnExit value="1"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="NoEffect"/>
                    <OptionOnEnter value="NoEffect"/>
                    <OptionOnExit value="2"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Missing"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="AuthoredSymbolic"/>
                    <OptionOnEnter value="Enable"/>
                    <OptionOnExit value="Disable"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="UnsupportedSymbolic"/>
                    <OptionOnEnter value="enable"/>
                    <OptionOnExit value="Reset"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Numeric"/>
                    <OptionOnEnter value="0"/>
                    <OptionOnExit value="1"/>
                  </ProceduralParams></Procedural>
                </ProcLayer>
            "#,
        );
        let clips = &database.fragment_audio()[0].clips;

        assert_eq!(clips[0].joint, "bind_mouth_web");
        assert_eq!(clips[0].option_on_enter, Some(Enable));
        assert_eq!(clips[0].option_on_exit, Some(Disable));
        assert_eq!(clips[1].option_on_enter, Some(NoEffect));
        assert_eq!(clips[1].option_on_exit, Some(NoEffect));
        assert_eq!(clips[2].option_on_enter, Some(Enable));
        assert_eq!(clips[2].option_on_exit, Some(Disable));
        // Authored symbolic labels dispatch; unknown spellings do not.
        assert_eq!(clips[3].option_on_enter, Some(Enable));
        assert_eq!(clips[3].option_on_exit, Some(Disable));
        assert_eq!(clips[4].option_on_enter, Some(NoEffect));
        assert_eq!(clips[4].option_on_exit, Some(NoEffect));
        assert_eq!(clips[5].option_on_enter, Some(Enable));
        assert_eq!(clips[5].option_on_exit, Some(Disable));
    }

    #[test]
    fn clamps_first_delay_and_schedules_proc_layers_independently() {
        let database = parse_audio(
            br#"
                <ProcLayer>
                  <Blend ExitTime="-0.25"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="NegativeFirst"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.5"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="AfterNegative"/>
                  </ProceduralParams></Procedural>
                </ProcLayer>
                <ProcLayer>
                  <Blend ExitTime="0"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="ZeroFirst"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="0.25"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="Independent"/>
                  </ProceduralParams></Procedural>
                  <Blend ExitTime="-0.1"/>
                  <Procedural type="CharacterEvent"><ProceduralParams>
                    <CharacterEventName value="RawNegativeTransition"/>
                  </ProceduralParams></Procedural>
                </ProcLayer>
            "#,
        );
        let clips = &database.fragment_audio()[0].clips;

        assert_eq!(
            clips
                .iter()
                .map(|clip| (clip.start_time, clip.proc_layer_ordinal))
                .collect::<Vec<_>>(),
            [
                (Some(0.0), 0),
                (Some(0.5), 0),
                (Some(0.0), 1),
                (Some(0.25), 1),
                (Some(0.15), 1),
            ]
        );
        assert_eq!(clips[0].exit_time, Some(0.5));
        assert_eq!(clips[1].exit_time, None);
        assert_eq!(clips[2].exit_time, Some(0.25));
        assert_eq!(clips[3].exit_time, Some(0.15));
        assert_eq!(clips[4].exit_time, None);
        assert_eq!(clips[0].procedural_ordinal, 0);
        assert_eq!(clips[1].procedural_ordinal, 1);
        assert_eq!(clips[2].procedural_ordinal, 0);
        assert_eq!(clips[3].procedural_ordinal, 1);
        assert_eq!(clips[4].procedural_ordinal, 2);
    }

    fn parse_audio(proc_layers: &[u8]) -> crate::MannequinAnimationDatabase {
        let mut fixture = br#"
            <AnimDB FragDef="actions.xml" TagDef="tags.xml">
              <FragmentList>
                <Test>
                  <Fragment>
        "#
        .to_vec();
        fixture.extend_from_slice(proc_layers);
        fixture.extend_from_slice(
            br#"
                  </Fragment>
                </Test>
              </FragmentList>
            </AnimDB>
            "#,
        );
        MannequinAnimationDatabaseSource::from_legacy("test.adb", &fixture)
            .unwrap()
            .database
    }
}
