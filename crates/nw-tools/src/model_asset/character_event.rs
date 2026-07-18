//! Pure receiver-owned CharacterEvent mapping.
//!
//! This module translates one authored Mannequin callback into phase-aware,
//! receiver-scoped operations. Catalog lookup is abstract so the branch table can
//! be tested without loading or parsing Wwise banks.

use nw_model::{
    CryAudioSpatialMode, CryCharacterEventCondition, CryCharacterEventDispatch,
    CryCharacterEventOperation, CryCharacterEventOption, CryCharacterEventPhase,
    CryCharacterEventReceiver, CryCharacterEventReceiverKind, CryMannequinAudioClip,
    CryMannequinReceiverContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidAudioTag {
    pub name: String,
    pub crc: u32,
}

pub(super) trait CharacterEventCatalogs {
    fn valid_tags(&self, tag_crcs: &[u32]) -> Vec<ValidAudioTag>;
    fn resolve_control(&self, candidate: &str) -> Option<String>;
}

/// Replace a receiver-owned clip's derived dispatches. Direct Mannequin Audio
/// clips and context-free CharacterEvents remain untouched.
pub(super) fn map_clip(clip: &mut CryMannequinAudioClip, catalogs: &impl CharacterEventCatalogs) {
    let Some(context) = clip.context.clone() else {
        return;
    };
    let Some(character_event) = clip.character_event.clone() else {
        return;
    };

    let mut tags = catalogs.valid_tags(&context.tag_crcs);
    tags.sort_by(|left, right| {
        left.name
            .as_bytes()
            .cmp(right.name.as_bytes())
            .then_with(|| left.crc.cmp(&right.crc))
    });
    tags.dedup();
    let variants = if tags.is_empty() {
        vec![None]
    } else {
        tags.into_iter().map(Some).collect()
    };

    let mut dispatches = Vec::new();
    if let Some(enabled) = option_enabled(clip.option_on_enter) {
        push_phase_dispatches(
            clip,
            &context,
            &character_event,
            &variants,
            clip.start_time.max(0.0),
            CryCharacterEventPhase::Enter,
            enabled,
            catalogs,
            &mut dispatches,
        );
    }
    if let (Some(time), Some(enabled)) = (clip.exit_time, option_enabled(clip.option_on_exit)) {
        push_phase_dispatches(
            clip,
            &context,
            &character_event,
            &variants,
            time.max(0.0),
            CryCharacterEventPhase::Exit,
            enabled,
            catalogs,
            &mut dispatches,
        );
    }

    dispatches.sort_by(dispatch_order);
    clip.dispatches = dispatches;
}

fn option_enabled(option: Option<CryCharacterEventOption>) -> Option<bool> {
    match option? {
        CryCharacterEventOption::Enable => Some(true),
        CryCharacterEventOption::Disable => Some(false),
        CryCharacterEventOption::NoEffect => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_phase_dispatches(
    clip: &CryMannequinAudioClip,
    context: &CryMannequinReceiverContext,
    character_event: &str,
    variants: &[Option<ValidAudioTag>],
    time: f32,
    phase: CryCharacterEventPhase,
    enabled: bool,
    catalogs: &impl CharacterEventCatalogs,
    out: &mut Vec<CryCharacterEventDispatch>,
) {
    for tag in variants {
        for receiver in &context.receivers {
            let (kind, script_path, operations) = receiver_operations(
                receiver,
                context.entity_id,
                character_event,
                enabled,
                tag.as_ref(),
                &clip.joint,
                catalogs,
            );
            out.push(CryCharacterEventDispatch {
                time,
                phase,
                enabled,
                receiver: kind,
                receiver_script_path: script_path.to_owned(),
                scene_path: context.scene_path.clone(),
                entity_id: context.entity_id,
                entity_name: context.entity_name.clone(),
                valid_tag: tag.as_ref().map(|tag| tag.name.clone()),
                valid_tag_crc: tag.as_ref().map(|tag| tag.crc),
                fragment: clip.fragment.clone(),
                proc_layer_ordinal: clip.proc_layer_ordinal,
                procedural_ordinal: clip.procedural_ordinal,
                character_event: character_event.to_owned(),
                joint: clip.joint.clone(),
                producer: clip.producer,
                operations,
            });
        }
    }
}

fn receiver_operations<'a>(
    receiver: &'a CryCharacterEventReceiver,
    entity_id: u64,
    character_event: &str,
    enabled: bool,
    tag: Option<&ValidAudioTag>,
    joint: &str,
    catalogs: &impl CharacterEventCatalogs,
) -> (
    CryCharacterEventReceiverKind,
    &'a str,
    Vec<CryCharacterEventOperation>,
) {
    match receiver {
        CryCharacterEventReceiver::CommonNpcAudio { script_path } => (
            CryCharacterEventReceiverKind::CommonNpcAudio,
            script_path,
            common_npc_operations(entity_id, character_event, enabled, tag, joint, catalogs),
        ),
        CryCharacterEventReceiver::BoneAudio {
            script_path,
            bindings,
            spawn_sound,
        } => (
            CryCharacterEventReceiverKind::BoneAudio,
            script_path,
            bone_audio_operations(character_event, enabled, bindings, *spawn_sound, catalogs),
        ),
        CryCharacterEventReceiver::SubtitleNpcAudio { script_path } => (
            CryCharacterEventReceiverKind::SubtitleNpcAudio,
            script_path,
            enabled
                .then(|| CryCharacterEventOperation::Subtitle {
                    event_name: character_event.to_owned(),
                })
                .into_iter()
                .collect(),
        ),
        CryCharacterEventReceiver::MountAudio { script_path } => (
            CryCharacterEventReceiverKind::MountAudio,
            script_path,
            mount_audio_operations(character_event, enabled, tag, catalogs),
        ),
    }
}

fn common_npc_operations(
    entity_id: u64,
    event: &str,
    enabled: bool,
    tag: Option<&ValidAudioTag>,
    _joint: &str,
    catalogs: &impl CharacterEventCatalogs,
) -> Vec<CryCharacterEventOperation> {
    let Some(tag) = tag.filter(|_| !event.is_empty()) else {
        return Vec::new();
    };
    let prefix = if enabled { "Play_" } else { "Stop_" };
    let mut operations = Vec::new();

    match event {
        event if event.contains("VOX") => {
            if enabled {
                push_audio(
                    &mut operations,
                    catalogs,
                    &format!("stop_{}_voice", tag.name.to_ascii_lowercase()),
                    CryAudioSpatialMode::Entity,
                    true,
                    Some(entity_id),
                    Some(CryCharacterEventCondition::VoicePlaying),
                );
                operations.push(CryCharacterEventOperation::SetVoicePlaying {
                    value: false,
                    condition: Some(CryCharacterEventCondition::VoicePlaying),
                });
            } else {
                operations.push(CryCharacterEventOperation::SetResumeVoiceCountdown);
            }
            if let Some(control) = vox_control(prefix, &tag.name, event) {
                push_audio(
                    &mut operations,
                    catalogs,
                    &control,
                    CryAudioSpatialMode::Joint,
                    true,
                    None,
                    None,
                );
            }
        }
        event if enabled && event.contains("BlockBreaker") => {
            operations.push(CryCharacterEventOperation::KillAllTriggers {
                target_entity: entity_id,
            });
            push_audio(
                &mut operations,
                catalogs,
                "Play_GPUI_BlockBreaker",
                CryAudioSpatialMode::Entity,
                false,
                Some(entity_id),
                None,
            );
        }
        event if enabled && event.contains("Bodyfall") => {
            let control = format!("Play_{event}");
            push_audio(
                &mut operations,
                catalogs,
                &control,
                CryAudioSpatialMode::Entity,
                false,
                Some(entity_id),
                None,
            );
            push_audio(
                &mut operations,
                catalogs,
                &control,
                CryAudioSpatialMode::Joint,
                true,
                None,
                None,
            );
        }
        event if enabled && event.contains("Ground_Slam") => {
            let scoped = format!("Play_SFX_{}_{event}", tag.name);
            push_audio(
                &mut operations,
                catalogs,
                &scoped,
                CryAudioSpatialMode::Entity,
                false,
                Some(entity_id),
                None,
            );
            push_audio(
                &mut operations,
                catalogs,
                "Play_NPC_Ground_Slam",
                CryAudioSpatialMode::Entity,
                false,
                Some(entity_id),
                None,
            );
            push_audio(
                &mut operations,
                catalogs,
                &scoped,
                CryAudioSpatialMode::Joint,
                true,
                None,
                None,
            );
        }
        event if enabled && event.contains("Voice") => {
            push_audio(
                &mut operations,
                catalogs,
                &format!("play_{}_voice", tag.name.to_ascii_lowercase()),
                CryAudioSpatialMode::Entity,
                true,
                Some(entity_id),
                Some(CryCharacterEventCondition::AliveAndVoiceNotPlaying),
            );
        }
        event if enabled && event.contains("PIN_") => {
            let suffix = event.strip_prefix("PIN_").unwrap_or(event);
            let control = format!("Play_PIN_{}_{suffix}", tag.name);
            push_audio(
                &mut operations,
                catalogs,
                &control,
                CryAudioSpatialMode::WorldPosition,
                false,
                Some(entity_id),
                None,
            );
            push_audio(
                &mut operations,
                catalogs,
                &control,
                CryAudioSpatialMode::Joint,
                true,
                None,
                None,
            );
        }
        event => {
            push_audio(
                &mut operations,
                catalogs,
                &format!("{prefix}SFX_{}_{event}", tag.name),
                CryAudioSpatialMode::Joint,
                true,
                None,
                None,
            );
        }
    }
    operations
}

fn vox_control(prefix: &str, tag: &str, event: &str) -> Option<String> {
    let parts = event.split('_').collect::<Vec<_>>();
    let part2 = parts.get(1).copied().filter(|part| !part.is_empty())?;
    let mut control = format!("{prefix}VOX_{tag}_{part2}");
    for part in parts.iter().skip(2).take(2) {
        control.push('_');
        control.push_str(part);
    }
    Some(control)
}

fn bone_audio_operations(
    character_event: &str,
    enabled: bool,
    bindings: &[nw_model::CryBoneAudioBinding],
    spawn_sound: bool,
    catalogs: &impl CharacterEventCatalogs,
) -> Vec<CryCharacterEventOperation> {
    if !enabled {
        return Vec::new();
    }
    let mut operations = Vec::new();
    for binding in bindings
        .iter()
        .filter(|binding| binding.character_event == character_event)
    {
        push_audio(
            &mut operations,
            catalogs,
            &binding.wwise_event,
            if spawn_sound {
                CryAudioSpatialMode::WorldPosition
            } else {
                CryAudioSpatialMode::Entity
            },
            false,
            Some(binding.audio_entity),
            None,
        );
    }
    operations
}

fn mount_audio_operations(
    event: &str,
    enabled: bool,
    tag: Option<&ValidAudioTag>,
    catalogs: &impl CharacterEventCatalogs,
) -> Vec<CryCharacterEventOperation> {
    let Some(tag) = tag.filter(|tag| is_mount_tag(&tag.name)) else {
        return Vec::new();
    };
    let (family, suffix) = if let Some(suffix) = event.strip_prefix("VOX_") {
        ("VOX", suffix)
    } else if let Some(suffix) = event.strip_prefix("SFX_") {
        ("SFX", suffix)
    } else {
        return Vec::new();
    };
    if suffix.is_empty() {
        return Vec::new();
    }
    let prefix = if enabled { "Play" } else { "Stop" };
    let mut operations = Vec::new();
    push_audio(
        &mut operations,
        catalogs,
        &format!("{prefix}_{family}_{}_{suffix}", tag.name),
        CryAudioSpatialMode::Joint,
        true,
        None,
        None,
    );
    operations
}

fn is_mount_tag(tag: &str) -> bool {
    ["Horse", "Wolf", "BigCat", "Bear", "Turkey", "Grunt", "Fox"]
        .into_iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

#[allow(clippy::too_many_arguments)]
fn push_audio(
    operations: &mut Vec<CryCharacterEventOperation>,
    catalogs: &impl CharacterEventCatalogs,
    candidate: &str,
    spatial_mode: CryAudioSpatialMode,
    validity_gated: bool,
    target_entity: Option<u64>,
    condition: Option<CryCharacterEventCondition>,
) {
    let Some(control) = catalogs.resolve_control(candidate) else {
        return;
    };
    operations.push(CryCharacterEventOperation::AudioControl {
        control,
        spatial_mode,
        validity_gated,
        target_entity,
        condition,
    });
}

fn dispatch_order(
    left: &CryCharacterEventDispatch,
    right: &CryCharacterEventDispatch,
) -> std::cmp::Ordering {
    left.time
        .total_cmp(&right.time)
        .then_with(|| phase_order(left.phase).cmp(&phase_order(right.phase)))
        .then_with(|| left.proc_layer_ordinal.cmp(&right.proc_layer_ordinal))
        .then_with(|| left.procedural_ordinal.cmp(&right.procedural_ordinal))
        .then_with(|| {
            left.scene_path
                .to_ascii_lowercase()
                .cmp(&right.scene_path.to_ascii_lowercase())
        })
        .then_with(|| left.entity_id.cmp(&right.entity_id))
        .then_with(|| {
            left.valid_tag
                .as_deref()
                .unwrap_or("")
                .as_bytes()
                .cmp(right.valid_tag.as_deref().unwrap_or("").as_bytes())
        })
        .then_with(|| left.valid_tag_crc.cmp(&right.valid_tag_crc))
        .then_with(|| receiver_order(left.receiver).cmp(&receiver_order(right.receiver)))
        .then_with(|| left.receiver_script_path.cmp(&right.receiver_script_path))
}

fn phase_order(phase: CryCharacterEventPhase) -> u8 {
    match phase {
        CryCharacterEventPhase::Exit => 0,
        CryCharacterEventPhase::Enter => 1,
    }
}

fn receiver_order(receiver: CryCharacterEventReceiverKind) -> u8 {
    match receiver {
        CryCharacterEventReceiverKind::CommonNpcAudio => 0,
        CryCharacterEventReceiverKind::BoneAudio => 1,
        CryCharacterEventReceiverKind::SubtitleNpcAudio => 2,
        CryCharacterEventReceiverKind::MountAudio => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use nw_model::{CryMannequinAudioProducer, CryMannequinReceiverContext};

    #[derive(Default)]
    struct Catalog {
        tags: BTreeMap<u32, Vec<String>>,
        controls: BTreeMap<String, String>,
    }

    impl Catalog {
        fn tag(mut self, crc: u32, name: &str) -> Self {
            self.tags.entry(crc).or_default().push(name.to_owned());
            self
        }

        fn control(mut self, name: &str) -> Self {
            self.controls
                .insert(name.to_ascii_lowercase(), name.to_owned());
            self
        }
    }

    impl CharacterEventCatalogs for Catalog {
        fn valid_tags(&self, tag_crcs: &[u32]) -> Vec<ValidAudioTag> {
            tag_crcs
                .iter()
                .flat_map(|crc| {
                    self.tags
                        .get(crc)
                        .into_iter()
                        .flatten()
                        .map(|name| ValidAudioTag {
                            name: name.clone(),
                            crc: *crc,
                        })
                })
                .collect()
        }

        fn resolve_control(&self, candidate: &str) -> Option<String> {
            self.controls.get(&candidate.to_ascii_lowercase()).cloned()
        }
    }

    #[test]
    fn common_npc_branch_table_preserves_play_and_stop_semantics() {
        let catalogs = Catalog::default()
            .tag(7, "Alligator")
            .control("stop_alligator_voice")
            .control("Play_VOX_Alligator_Attack_Loud_Near")
            .control("Stop_VOX_Alligator_Attack_Loud_Near")
            .control("Play_GPUI_BlockBreaker")
            .control("Stop_SFX_Alligator_BlockBreaker")
            .control("Play_Bodyfall_Heavy")
            .control("Stop_SFX_Alligator_Bodyfall_Heavy")
            .control("Play_SFX_Alligator_Ground_Slam_Heavy")
            .control("Play_NPC_Ground_Slam")
            .control("Stop_SFX_Alligator_Ground_Slam_Heavy")
            .control("play_alligator_voice")
            .control("Stop_SFX_Alligator_Voice")
            .control("Play_PIN_Alligator_Roar")
            .control("Stop_SFX_Alligator_PIN_Roar")
            .control("Play_SFX_Alligator_Bite")
            .control("Stop_SFX_Alligator_Bite");
        let tag = ValidAudioTag {
            name: "Alligator".to_owned(),
            crc: 7,
        };

        let play_vox = common_npc_operations(
            9,
            "VOX_Attack_Loud_Near_Discarded",
            true,
            Some(&tag),
            "",
            &catalogs,
        );
        assert_eq!(play_vox.len(), 3);
        assert!(matches!(
            play_vox[0],
            CryCharacterEventOperation::AudioControl {
                condition: Some(CryCharacterEventCondition::VoicePlaying),
                ..
            }
        ));
        assert!(matches!(
            play_vox[1],
            CryCharacterEventOperation::SetVoicePlaying { value: false, .. }
        ));
        assert_eq!(control(&play_vox[2]), "Play_VOX_Alligator_Attack_Loud_Near");
        let stop_vox = common_npc_operations(
            9,
            "VOX_Attack_Loud_Near_Discarded",
            false,
            Some(&tag),
            "",
            &catalogs,
        );
        assert!(matches!(
            stop_vox[0],
            CryCharacterEventOperation::SetResumeVoiceCountdown
        ));
        assert_eq!(control(&stop_vox[1]), "Stop_VOX_Alligator_Attack_Loud_Near");

        let block = common_npc_operations(9, "BlockBreaker", true, Some(&tag), "", &catalogs);
        assert!(matches!(
            block[0],
            CryCharacterEventOperation::KillAllTriggers { target_entity: 9 }
        ));
        assert_eq!(control(&block[1]), "Play_GPUI_BlockBreaker");
        assert_eq!(
            control(&common_npc_operations(9, "BlockBreaker", false, Some(&tag), "", &catalogs)[0]),
            "Stop_SFX_Alligator_BlockBreaker"
        );

        let body = common_npc_operations(9, "Bodyfall_Heavy", true, Some(&tag), "", &catalogs);
        assert_eq!(body.len(), 2);
        assert_eq!(control(&body[0]), "Play_Bodyfall_Heavy");
        assert_eq!(control(&body[1]), "Play_Bodyfall_Heavy");
        assert_eq!(mode(&body[0]), CryAudioSpatialMode::Entity);
        assert_eq!(mode(&body[1]), CryAudioSpatialMode::Joint);

        let slam = common_npc_operations(9, "Ground_Slam_Heavy", true, Some(&tag), "", &catalogs);
        assert_eq!(
            slam.iter().map(control).collect::<Vec<_>>(),
            [
                "Play_SFX_Alligator_Ground_Slam_Heavy",
                "Play_NPC_Ground_Slam",
                "Play_SFX_Alligator_Ground_Slam_Heavy"
            ]
        );

        let voice = common_npc_operations(9, "Voice", true, Some(&tag), "", &catalogs);
        assert!(matches!(
            voice[0],
            CryCharacterEventOperation::AudioControl {
                condition: Some(CryCharacterEventCondition::AliveAndVoiceNotPlaying),
                ..
            }
        ));

        let pin = common_npc_operations(9, "PIN_Roar", true, Some(&tag), "", &catalogs);
        assert_eq!(pin.len(), 2);
        assert_eq!(mode(&pin[0]), CryAudioSpatialMode::WorldPosition);
        assert_eq!(mode(&pin[1]), CryAudioSpatialMode::Joint);
        assert_eq!(
            control(&common_npc_operations(9, "PIN_Roar", false, Some(&tag), "", &catalogs)[0]),
            "Stop_SFX_Alligator_PIN_Roar"
        );

        let generic = common_npc_operations(9, "Bite", true, Some(&tag), "", &catalogs);
        assert_eq!(control(&generic[0]), "Play_SFX_Alligator_Bite");
        let generic_stop = common_npc_operations(9, "Bite", false, Some(&tag), "", &catalogs);
        assert_eq!(control(&generic_stop[0]), "Stop_SFX_Alligator_Bite");
    }

    #[test]
    fn invalid_block_breaker_control_retains_kill_all_side_effect() {
        let catalogs = Catalog::default().tag(7, "Alligator");
        let tag = ValidAudioTag {
            name: "Alligator".to_owned(),
            crc: 7,
        };
        assert_eq!(
            common_npc_operations(9, "BlockBreaker", true, Some(&tag), "", &catalogs),
            vec![CryCharacterEventOperation::KillAllTriggers { target_entity: 9 }]
        );
    }

    #[test]
    fn tag_cardinality_is_zero_one_or_sorted_mutually_exclusive_variants() {
        let catalogs = Catalog::default()
            .tag(20, "Wolf")
            .tag(10, "Bear")
            .control("Play_SFX_Bear_Bite")
            .control("Play_SFX_Wolf_Bite");
        let mut zero = clip(vec![]);
        map_clip(&mut zero, &catalogs);
        assert_eq!(zero.dispatches.len(), 1);
        assert_eq!(zero.dispatches[0].valid_tag, None);
        assert!(zero.dispatches[0].operations.is_empty());

        let mut one = clip(vec![20]);
        map_clip(&mut one, &catalogs);
        assert_eq!(one.dispatches[0].valid_tag.as_deref(), Some("Wolf"));

        let mut many = clip(vec![20, 10]);
        map_clip(&mut many, &catalogs);
        assert_eq!(
            many.dispatches
                .iter()
                .map(|dispatch| dispatch.valid_tag.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["Bear", "Wolf"]
        );
    }

    #[test]
    fn alligator_tag_never_retries_misspelled_aligator_event() {
        let catalogs = Catalog::default()
            .tag(7, "Alligator")
            .control("Play_SFX_Aligator_Tail_Swipe");
        let mut clip = clip(vec![7]);
        clip.trigger = "Tail_Swipe".to_owned();
        clip.character_event = Some("Tail_Swipe".to_owned());
        map_clip(&mut clip, &catalogs);
        assert!(clip.dispatches[0].operations.is_empty());
    }

    #[test]
    fn atl_authored_controls_do_not_require_an_event_id_csv() {
        let catalogs = Catalog::default()
            .tag(7, "Isabella")
            .control("Play_SFX_Isabella_IsabellaTrial")
            .control("Play_SFX_Isabella_IsabellaWings");
        for event in ["IsabellaTrial", "IsabellaWings"] {
            let mut clip = clip(vec![7]);
            clip.trigger = event.to_owned();
            clip.character_event = Some(event.to_owned());
            map_clip(&mut clip, &catalogs);
            assert_eq!(clip.dispatches[0].operations.len(), 1);
        }
    }

    #[test]
    fn bone_subtitle_and_mount_receivers_keep_their_distinct_rules() {
        let catalogs = Catalog::default()
            .tag(7, "Horse")
            .control("Play_Bone_Bite")
            .control("Play_VOX_Horse_Whinny")
            .control("Stop_VOX_Horse_Whinny");
        let bindings = vec![nw_model::CryBoneAudioBinding {
            character_event: "Bite".to_owned(),
            audio_entity: 71,
            wwise_event: "Play_Bone_Bite".to_owned(),
        }];
        let bone = bone_audio_operations("Bite", true, &bindings, true, &catalogs);
        assert_eq!(bone.len(), 1);
        assert_eq!(mode(&bone[0]), CryAudioSpatialMode::WorldPosition);
        assert!(matches!(
            bone[0],
            CryCharacterEventOperation::AudioControl {
                target_entity: Some(71),
                ..
            }
        ));
        assert!(bone_audio_operations("bite", true, &bindings, true, &catalogs).is_empty());
        assert!(bone_audio_operations("Bite", false, &bindings, true, &catalogs).is_empty());

        let tag = ValidAudioTag {
            name: "Horse".to_owned(),
            crc: 7,
        };
        assert_eq!(
            control(&mount_audio_operations("VOX_Whinny", true, Some(&tag), &catalogs)[0]),
            "Play_VOX_Horse_Whinny"
        );
        assert_eq!(
            control(&mount_audio_operations("VOX_Whinny", false, Some(&tag), &catalogs)[0]),
            "Stop_VOX_Horse_Whinny"
        );
        assert!(mount_audio_operations("Bite", true, Some(&tag), &catalogs).is_empty());

        let subtitle = receiver_operations(
            &CryCharacterEventReceiver::SubtitleNpcAudio {
                script_path: "scripts/subtitlenpc_audio.lua".to_owned(),
            },
            9,
            "VOX_Whinny",
            true,
            Some(&tag),
            "",
            &catalogs,
        )
        .2;
        assert_eq!(
            subtitle,
            vec![CryCharacterEventOperation::Subtitle {
                event_name: "VOX_Whinny".to_owned()
            }]
        );
    }

    #[test]
    fn mapping_is_byte_deterministic_under_permuted_tags_and_receivers() {
        let catalogs = Catalog::default()
            .tag(20, "Wolf")
            .tag(10, "Bear")
            .control("Play_SFX_Bear_Bite")
            .control("Play_SFX_Wolf_Bite")
            .control("Play_VOX_Bear_Bite")
            .control("Play_VOX_Wolf_Bite");
        let mut left = clip(vec![20, 10]);
        left.context
            .as_mut()
            .unwrap()
            .receivers
            .push(CryCharacterEventReceiver::MountAudio {
                script_path: "scripts/mountaudio.lua".to_owned(),
            });
        let mut right = clip(vec![10, 20]);
        right.context.as_mut().unwrap().receivers.insert(
            0,
            CryCharacterEventReceiver::MountAudio {
                script_path: "scripts/mountaudio.lua".to_owned(),
            },
        );
        map_clip(&mut left, &catalogs);
        map_clip(&mut right, &catalogs);
        assert_eq!(
            serde_json::to_vec(&left.dispatches).unwrap(),
            serde_json::to_vec(&right.dispatches).unwrap()
        );
    }

    fn clip(tag_crcs: Vec<u32>) -> CryMannequinAudioClip {
        CryMannequinAudioClip {
            trigger: "Bite".to_owned(),
            stop_trigger: None,
            character_event: Some("Bite".to_owned()),
            joint: String::new(),
            start_time: 0.25,
            exit_time: None,
            option_on_enter: Some(CryCharacterEventOption::Enable),
            option_on_exit: Some(CryCharacterEventOption::Disable),
            proc_layer_ordinal: 0,
            procedural_ordinal: 0,
            producer: CryMannequinAudioProducer::MannequinCharacterEvent,
            fragment: "Attack_Bite".to_owned(),
            tags: String::new(),
            context: Some(CryMannequinReceiverContext {
                scene_path: "slices/creature.dynamicslice".to_owned(),
                entity_id: 9,
                entity_name: "creature".to_owned(),
                tag_crcs,
                adb_paths: Vec::new(),
                controller_paths: Vec::new(),
                receivers: vec![CryCharacterEventReceiver::CommonNpcAudio {
                    script_path: "scripts/commonnpc_audio.lua".to_owned(),
                }],
            }),
            dispatches: Vec::new(),
        }
    }

    fn control(operation: &CryCharacterEventOperation) -> &str {
        let CryCharacterEventOperation::AudioControl { control, .. } = operation else {
            panic!("expected audio control, got {operation:?}");
        };
        control
    }

    fn mode(operation: &CryCharacterEventOperation) -> CryAudioSpatialMode {
        let CryCharacterEventOperation::AudioControl { spatial_mode, .. } = operation else {
            panic!("expected audio control, got {operation:?}");
        };
        *spatial_mode
    }
}
