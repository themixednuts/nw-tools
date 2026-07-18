//! Typed access to Mannequin fragment audio.
//!
//! Creature vocal/action sounds (bite, hurt, tail whip, body falls) are authored
//! as `ProcLayer` procedural clips inside Mannequin fragments — not as
//! CryAnimation animevents. Two procedural types carry audio:
//!
//! - `type="Audio"` clips name an ATL `StartTrigger`, an optional `StopTrigger`,
//!   the `AttachmentJoint` the sound rides, and inherit the owning `ProcLayer`
//!   `<Blend StartTime>` timing (footsteps, hurt, tail whip, body falls).
//! - `type="CharacterEvent"` clips carry a short `CharacterEventName` (`Bite`,
//!   `VOX_Attack1`) the character audio handler expands to a full Wwise event,
//!   the `AttachmentJoint`, and inherit the owning `<Blend ExitTime>` timing
//!   (Ghidra-confirmed NewWorld 3-26: the bite/attack vocals, animation-bound at
//!   authored clip-relative times). Their short names are resolved to Wwise
//!   events by the caller against the character's event-id catalog.
//!
//! A single logical fragment (e.g. `r_Death`) is assembled from several ADBs:
//! the animation database contributes the `AnimLayer` animation while a sibling
//! audio database contributes the `ProcLayer` audio for the same fragment name.
//! Callers merge [`MannequinFragmentAudio`] across databases by fragment name to
//! pair each animation with the sounds it fires.

use serde::{Deserialize, Serialize};

use crate::source_transform::{
    MannequinAnimationDatabase, MannequinFragment, MannequinProcedural,
};

/// The Mannequin procedural `type` that fires an ATL audio trigger.
pub const AUDIO_PROCEDURAL_TYPE: &str = "Audio";

/// The Mannequin procedural `type` that fires a named character event — the
/// bite/attack vocals whose short `CharacterEventName` the character audio
/// handler expands to a Wwise event.
pub const CHARACTER_EVENT_PROCEDURAL_TYPE: &str = "CharacterEvent";

/// Sentinel `StopTrigger` value meaning "do not stop" — never a real ATL trigger,
/// so it is dropped from [`MannequinAudioClip::stop_trigger`].
pub const DO_NOTHING_TRIGGER: &str = "do_nothing";

const START_TRIGGER_PARAM: &str = "StartTrigger";
const STOP_TRIGGER_PARAM: &str = "StopTrigger";
const ATTACHMENT_JOINT_PARAM: &str = "AttachmentJoint";
const CHARACTER_EVENT_NAME_PARAM: &str = "CharacterEventName";

/// Which procedural type authored an audio clip, so a caller can resolve each the
/// right way: an [`AtlTrigger`](MannequinAudioKind::AtlTrigger) `trigger` is an ATL
/// control name, while a [`CharacterEvent`](MannequinAudioKind::CharacterEvent)
/// `trigger` is a short `CharacterEventName` that must be expanded to a Wwise
/// event through the character's event catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MannequinAudioKind {
    /// `type="Audio"` clip firing an ATL `StartTrigger` directly.
    #[default]
    AtlTrigger,
    /// `type="CharacterEvent"` clip carrying a short `CharacterEventName`.
    CharacterEvent,
}

/// One `ProcLayer` audio clip: the trigger/short name it fires, the joint it
/// attaches to, and the layer-blend time (seconds, relative to the fragment's
/// animation start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannequinAudioClip {
    /// For an [`AtlTrigger`](MannequinAudioKind::AtlTrigger) clip, the ATL
    /// `StartTrigger` control name (e.g. `play_vox_alligator_hurt`). For a
    /// [`CharacterEvent`](MannequinAudioKind::CharacterEvent) clip, the short
    /// `CharacterEventName` (e.g. `Bite`, `VOX_Attack1`) the caller resolves to a
    /// Wwise event.
    pub trigger: String,
    /// ATL `StopTrigger`, when it is a real trigger (the `do_nothing` sentinel is
    /// dropped). Never set for a `CharacterEvent` clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_trigger: Option<String>,
    /// Skeleton joint the sound is attached to (e.g. `bind_mouth_web`).
    pub joint: String,
    /// The owning `ProcLayer` `<Blend>` time in seconds, when authored: an
    /// `Audio` clip inherits `StartTime`, a `CharacterEvent` clip inherits
    /// `ExitTime` (the authored clip-relative fire time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f32>,
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
            if !name.is_empty() && !out.iter().any(|existing| existing.eq_ignore_ascii_case(name)) {
                out.push(name.to_owned());
            }
        }
    }
}

fn collect_fragment_audio(fragment: &MannequinFragment, out: &mut Vec<MannequinAudioClip>) {
    for layer in &fragment.procedural_layers {
        // The XML alternates `<Blend/>` then `<Procedural/>`, so the procedural at
        // index `i` inherits the blend at index `i`.
        for (index, procedural) in layer.procedurals.iter().enumerate() {
            let blend = layer.blends.get(index);
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
                // An `Audio` clip fires at the blend's `StartTime`.
                let start_time = blend.and_then(|blend| blend.start_time);
                out.push(MannequinAudioClip {
                    trigger,
                    stop_trigger,
                    joint,
                    start_time,
                    kind: MannequinAudioKind::AtlTrigger,
                });
            } else if procedural.ty.eq_ignore_ascii_case(CHARACTER_EVENT_PROCEDURAL_TYPE) {
                let Some(event_name) = param(procedural, CHARACTER_EVENT_NAME_PARAM)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let joint = param(procedural, ATTACHMENT_JOINT_PARAM)
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_default();
                // A `CharacterEvent` clip fires at the blend's `ExitTime` — the
                // authored clip-relative time the vocal plays.
                let start_time = blend.and_then(|blend| blend.exit_time);
                out.push(MannequinAudioClip {
                    trigger: event_name,
                    stop_trigger: None,
                    joint,
                    start_time,
                    kind: MannequinAudioKind::CharacterEvent,
                });
            }
        }
    }
}

fn param(procedural: &MannequinProcedural, name: &str) -> Option<String> {
    procedural
        .parameters
        .iter()
        .find(|parameter| parameter.name.eq_ignore_ascii_case(name))
        .and_then(|parameter| parameter.value.clone())
}

#[cfg(test)]
mod tests {
    use crate::source_transform::MannequinAnimationDatabaseSource;

    const SAMPLE: &[u8] = br#"
        <AnimDB FragDef="actions.xml" TagDef="tags.xml">
          <FragmentList>
            <r_R0_B>
              <Fragment BlendOutDuration="0.2" Tags="">
                <AnimLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Animation name="alligator_r_r0_b"/>
                </AnimLayer>
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0.25" Duration="0"/>
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
        let database = MannequinAnimationDatabaseSource::from_legacy("test.adb", SAMPLE)
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
        assert_eq!(bite.clips[1].trigger, "play_sfx_aligator_tail_whip_fast");
        assert_eq!(bite.clips[1].joint, "bind_tail_05");

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

    // The real `npc_alligator_audio.adb` shape: `Attack_Bite` in the Audio scope
    // is authored entirely as `type="CharacterEvent"` clips whose `<Blend
    // ExitTime>` (not StartTime) is the fire time. VOX_Chatters (0), VOX_Attack1
    // (0.7) share the first ProcLayer; Bite (0.6) rides a second ProcLayer.
    const CHARACTER_EVENT_SAMPLE: &[u8] = br#"
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
                      <AttachmentJoint value="bind_mouth_web"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </Attack_Bite>
          </FragmentList>
        </AnimDB>
    "#;

    #[test]
    fn extracts_character_event_clips_with_exit_time_timing() {
        use super::MannequinAudioKind;

        let database = MannequinAnimationDatabaseSource::from_legacy("audio.adb", CHARACTER_EVENT_SAMPLE)
            .unwrap()
            .database;
        let fragments = database.fragment_audio();

        let bite = fragments.iter().find(|f| f.fragment == "Attack_Bite").unwrap();
        assert_eq!(bite.clips.len(), 3);

        // The short `CharacterEventName` is carried verbatim in `trigger` for the
        // caller to resolve; every clip is tagged `CharacterEvent`.
        assert!(
            bite.clips
                .iter()
                .all(|clip| clip.kind == MannequinAudioKind::CharacterEvent)
        );
        assert!(bite.clips.iter().all(|clip| clip.stop_trigger.is_none()));

        let chatters = &bite.clips[0];
        assert_eq!(chatters.trigger, "VOX_Chatters");
        assert_eq!(chatters.start_time, Some(0.0));

        let attack = &bite.clips[1];
        assert_eq!(attack.trigger, "VOX_Attack1");
        // Timing rides the owning `<Blend ExitTime>`, not `StartTime`.
        assert_eq!(attack.start_time, Some(0.69999999));

        let hit = &bite.clips[2];
        assert_eq!(hit.trigger, "Bite");
        assert_eq!(hit.start_time, Some(0.60000002));
        assert_eq!(hit.joint, "bind_mouth_web");
    }
}
