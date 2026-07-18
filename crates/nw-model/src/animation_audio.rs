//! Phase-aware Mannequin audio representation carried in glTF animation extras.

use serde::{Deserialize, Serialize};

/// CharacterEvent producer whose authored joint representation is carried by a
/// dispatch record. Timeline producers use a lowercase CRC instead; this model
/// currently emits only the two Mannequin producers below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryMannequinAudioProducer {
    MannequinAudio,
    MannequinCharacterEvent,
}

/// Phase option authored on a Mannequin CharacterEvent procedural.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryCharacterEventOption {
    Enable,
    Disable,
    NoEffect,
}

/// Callback phase for one receiver-owned CharacterEvent dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryCharacterEventPhase {
    Enter,
    Exit,
}

/// Receiver implementation that independently observes the entity-addressed
/// CharacterEvent callback. Vector order never represents subscriber order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryCharacterEventReceiverKind {
    CommonNpcAudio,
    BoneAudio,
    SubtitleNpcAudio,
    MountAudio,
}

/// Spatial execution mode used by a receiver-generated audio control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryAudioSpatialMode {
    Entity,
    Joint,
    WorldPosition,
}

/// Runtime state condition retained as metadata. Preview consumers must not turn
/// a conditional control into an unconditional audible strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CryCharacterEventCondition {
    VoicePlaying,
    AliveAndVoiceNotPlaying,
}

/// One receiver operation. The vector containing these values is ordered within
/// that receiver callback; receiver vectors themselves do not encode EBus
/// subscriber order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CryCharacterEventOperation {
    AudioControl {
        control: String,
        spatial_mode: CryAudioSpatialMode,
        validity_gated: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_entity: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        condition: Option<CryCharacterEventCondition>,
    },
    KillAllTriggers {
        target_entity: u64,
    },
    SetVoicePlaying {
        value: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        condition: Option<CryCharacterEventCondition>,
    },
    SetResumeVoiceCountdown,
    Subtitle {
        event_name: String,
    },
}

/// One phase-aware CharacterEvent callback delivered to one directly coattached
/// receiver. `operations` preserves only that receiver's established order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryCharacterEventDispatch {
    pub time: f32,
    pub phase: CryCharacterEventPhase,
    pub enabled: bool,
    pub receiver: CryCharacterEventReceiverKind,
    pub receiver_script_path: String,
    pub scene_path: String,
    pub entity_id: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub entity_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_tag_crc: Option<u32>,
    pub fragment: String,
    pub proc_layer_ordinal: usize,
    pub procedural_ordinal: usize,
    pub character_event: String,
    /// Authored AttachmentJoint. Empty and unknown values remain empty.
    pub joint: String,
    pub producer: CryMannequinAudioProducer,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<CryCharacterEventOperation>,
}

/// One exact BoneAudio event/entity/control alignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryBoneAudioBinding {
    pub character_event: String,
    pub audio_entity: u64,
    pub wwise_event: String,
}

/// A CharacterEvent receiver script directly co-owned by the context entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CryCharacterEventReceiver {
    CommonNpcAudio {
        script_path: String,
    },
    BoneAudio {
        script_path: String,
        bindings: Vec<CryBoneAudioBinding>,
        spawn_sound: bool,
    },
    SubtitleNpcAudio {
        script_path: String,
    },
    MountAudio {
        script_path: String,
    },
}

/// Same-entity provenance required to interpret CharacterEvent procedurals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryMannequinReceiverContext {
    pub scene_path: String,
    pub entity_id: u64,
    pub entity_name: String,
    pub tag_crcs: Vec<u32>,
    pub adb_paths: Vec<String>,
    pub controller_paths: Vec<String>,
    pub receivers: Vec<CryCharacterEventReceiver>,
}

/// One Mannequin fragment audio clip attached to a glTF animation. Direct Audio
/// clips retain their scalar trigger fields; receiver-owned CharacterEvents add
/// phase-aware `dispatches` while preserving the authored procedural fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryMannequinAudioClip {
    /// A direct Audio ATL trigger, or the original receiver-owned
    /// CharacterEventName.
    pub trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_trigger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_event: Option<String>,
    /// Authored AttachmentJoint. Empty and unknown values remain empty.
    pub joint: String,
    pub start_time: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_on_enter: Option<CryCharacterEventOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_on_exit: Option<CryCharacterEventOption>,
    pub proc_layer_ordinal: usize,
    pub procedural_ordinal: usize,
    pub producer: CryMannequinAudioProducer,
    /// Owning Mannequin fragment name (e.g. `Attack_Bite`).
    pub fragment: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tags: String,
    /// Present only for CharacterEvent clips. Direct Mannequin Audio ATL clips
    /// remain receiver-independent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CryMannequinReceiverContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dispatches: Vec<CryCharacterEventDispatch>,
}

/// Mannequin fragment audio grouped by the glTF animation it attaches to. Carried
/// on `CryAssetExtras` (not serialized there) so the exporter can distribute each
/// animation's clips into its per-animation `cryMannequinAudio` extras.
#[derive(Debug, Clone)]
pub struct CryMannequinAnimationAudio {
    /// Animation name that matches a fragment's AnimLayer `<Animation name>`.
    pub animation: String,
    pub clips: Vec<CryMannequinAudioClip>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_event_phase_records_round_trip_without_a_raw_joint_id() {
        let clip = CryMannequinAudioClip {
            trigger: "Bite".to_owned(),
            stop_trigger: None,
            character_event: Some("Bite".to_owned()),
            joint: String::new(),
            start_time: 0.25,
            exit_time: Some(0.75),
            option_on_enter: Some(CryCharacterEventOption::Enable),
            option_on_exit: Some(CryCharacterEventOption::Disable),
            proc_layer_ordinal: 2,
            procedural_ordinal: 3,
            producer: CryMannequinAudioProducer::MannequinCharacterEvent,
            fragment: "Attack_Bite".to_owned(),
            tags: "Young".to_owned(),
            context: None,
            dispatches: vec![CryCharacterEventDispatch {
                time: 0.25,
                phase: CryCharacterEventPhase::Enter,
                enabled: true,
                receiver: CryCharacterEventReceiverKind::CommonNpcAudio,
                receiver_script_path: "scripts/audio/npcs/commonnpc_audio.lua".to_owned(),
                scene_path: "slices/alligator.dynamicslice".to_owned(),
                entity_id: 41,
                entity_name: "alligator".to_owned(),
                valid_tag: Some("Alligator".to_owned()),
                valid_tag_crc: Some(cry_audio_crc_for_test(b"Alligator")),
                fragment: "Attack_Bite".to_owned(),
                proc_layer_ordinal: 2,
                procedural_ordinal: 3,
                character_event: "Bite".to_owned(),
                joint: String::new(),
                producer: CryMannequinAudioProducer::MannequinCharacterEvent,
                operations: vec![CryCharacterEventOperation::AudioControl {
                    control: "Play_SFX_Alligator_Bite".to_owned(),
                    spatial_mode: CryAudioSpatialMode::Joint,
                    validity_gated: true,
                    target_entity: None,
                    condition: None,
                }],
            }],
        };

        let json = serde_json::to_vec(&clip).unwrap();
        assert!(!String::from_utf8_lossy(&json).contains("jointId"));
        assert_eq!(
            serde_json::from_slice::<CryMannequinAudioClip>(&json).unwrap(),
            clip
        );
    }

    fn cry_audio_crc_for_test(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for byte in bytes {
            let mut current = byte.to_ascii_lowercase();
            for _ in 0..8 {
                let mix = (crc ^ u32::from(current)) & 1;
                crc >>= 1;
                if mix != 0 {
                    crc ^= 0xedb8_8320;
                }
                current >>= 1;
            }
        }
        !crc
    }
}
