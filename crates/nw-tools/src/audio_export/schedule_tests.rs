use super::*;
use std::fs;

#[test]
fn lowest_media_wav_is_deterministic_by_media_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for id in [500u32, 100u32] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }

    // Out of two decoded variations, the lowest media id wins — regardless
    // of array order.
    let media = vec![
        serde_json::json!({ "mediaId": 500 }),
        serde_json::json!({ "mediaId": 100 }),
    ];
    let chosen = lowest_media_wav(&media, root).unwrap();
    assert!(chosen.ends_with("100.wav"), "chose {chosen:?}");

    // A media id whose WAV is absent is ignored; the lowest *present* wins.
    let media = vec![
        serde_json::json!({ "mediaId": 1 }),
        serde_json::json!({ "mediaId": 500 }),
    ];
    let chosen = lowest_media_wav(&media, root).unwrap();
    assert!(chosen.ends_with("500.wav"), "chose {chosen:?}");
}

#[test]
fn blend_schedule_rotates_default_branch_variations_per_event() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Two default-branch variations (10, 20) and one non-default sample (5),
    // all decoded on disk.
    for id in [10u32, 20u32, 5u32] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }

    // Media array is deliberately out of order and mixes in the non-default
    // sample; only the default-branch variations rotate, ordered by media id.
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [{
                "trigger": "step",
                "media": [
                    { "mediaId": 20, "defaultBranch": true },
                    { "mediaId": 5 },
                    { "mediaId": 10, "defaultBranch": true },
                ],
            }],
        },
        "animations": [{
            "name": "walk",
            "extras": {
                "cryDuration": 1.0,
                "crySampleRate": 30,
                "cryEvents": [
                    { "parameter": "step", "normalizedTime": 0.1 },
                    { "parameter": "step", "normalizedTime": 0.2 },
                    { "parameter": "step", "normalizedTime": 0.3 },
                ],
            },
        }],
    });

    let schedule = build_blend_schedule(&document, root).unwrap();
    let sounds = &schedule.clips[0].sounds;
    assert_eq!(sounds.len(), 3);
    // Round-robin over [10, 20]: adjacent overlapping events differ.
    assert!(sounds[0].wav.ends_with("10.wav"), "{:?}", sounds[0].wav);
    assert!(sounds[1].wav.ends_with("20.wav"), "{:?}", sounds[1].wav);
    assert!(sounds[2].wav.ends_with("10.wav"), "{:?}", sounds[2].wav);
    // The non-default sample never appears.
    assert!(sounds.iter().all(|sound| !sound.wav.ends_with("5.wav")));
}

#[test]
fn blend_schedule_follows_default_surface_weighted_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Three decoded variations on disk.
    for id in [10u32, 20u32, 30u32] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }

    // The default surface's weighted sequence [30, 10, 30, 20] is the engine
    // order; the blend must assign consecutive footsteps straight down it, not
    // by media-id order and not the non-default branch.
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [{
                "trigger": "step",
                "media": [
                    { "mediaId": 10, "defaultBranch": true },
                    { "mediaId": 20, "defaultBranch": true },
                    { "mediaId": 30, "defaultBranch": true },
                ],
                "surfaceMedia": [
                    { "switchId": 7, "default": true, "media": [10, 20, 30],
                      "sequence": [30, 10, 30, 20] },
                    { "switchId": 9, "media": [30] },
                ],
            }],
        },
        "animations": [{
            "name": "walk",
            "extras": {
                "cryDuration": 1.0,
                "crySampleRate": 30,
                "cryEvents": [
                    { "parameter": "step", "normalizedTime": 0.1 },
                    { "parameter": "step", "normalizedTime": 0.2 },
                    { "parameter": "step", "normalizedTime": 0.3 },
                    { "parameter": "step", "normalizedTime": 0.4 },
                    { "parameter": "step", "normalizedTime": 0.5 },
                ],
            },
        }],
    });

    let schedule = build_blend_schedule(&document, root).unwrap();
    let sounds = &schedule.clips[0].sounds;
    assert_eq!(sounds.len(), 5);
    // Follows the weighted sequence, cycling once past its length of 4.
    let expected = ["30.wav", "10.wav", "30.wav", "20.wav", "30.wav"];
    for (sound, want) in sounds.iter().zip(expected) {
        assert!(sound.wav.ends_with(want), "want {want}, got {}", sound.wav);
    }
}

#[test]
fn blend_schedule_places_mannequin_audio_strips_at_start_time_frames() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for id in [10u32, 20u32] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }
    // A Mannequin audio clip fires an ATL trigger at an absolute startTime
    // (seconds), unlike a normalized-time animevent.
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [{
                "trigger": "play_vox_alligator_hurt",
                "media": [
                    { "mediaId": 10, "defaultBranch": true },
                    { "mediaId": 20, "defaultBranch": true },
                ],
            }],
        },
        "animations": [{
            "name": "alligator_r_r0_b",
            "extras": {
                "cryDuration": 2.0,
                "crySampleRate": 30,
                "cryMannequinAudio": [
                    { "trigger": "play_vox_alligator_hurt", "joint": "bind_mouth_web",
                      "startTime": 0.5, "fragment": "r_R0_B" },
                    { "trigger": "play_vox_alligator_hurt", "joint": "bind_mouth_web",
                      "startTime": 0.0, "fragment": "r_R0_B" },
                ],
            },
        }],
    });
    let schedule = build_blend_schedule(&document, root).unwrap();
    let sounds = &schedule.clips[0].sounds;
    assert_eq!(sounds.len(), 2);
    // frame = 1 + round(startTime * fps); fps = 30.
    assert_eq!(sounds[0].frame, 1 + 15);
    assert_eq!(sounds[1].frame, 1);
    // Weighted-sequence rotation applies across the two placements.
    assert!(sounds[0].wav.ends_with("10.wav"), "{:?}", sounds[0].wav);
    assert!(sounds[1].wav.ends_with("20.wav"), "{:?}", sounds[1].wav);
}

#[test]
fn phase_schedule_orders_exit_first_trims_play_and_keeps_conditions_as_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for id in [42u32, 43, 44, 99] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }
    let a_enter = dispatch(
        "Alligator",
        "Bite",
        0.1,
        "enter",
        true,
        0,
        0,
        Some("Play_A"),
        None,
    );
    let a_exit = dispatch(
        "Alligator",
        "Bite",
        0.5,
        "exit",
        false,
        0,
        0,
        Some("Stop_A"),
        None,
    );
    let b_enter = dispatch(
        "Alligator",
        "Roar",
        0.5,
        "enter",
        true,
        0,
        1,
        Some("Play_B"),
        None,
    );
    let conditional = dispatch(
        "Alligator",
        "Voice",
        0.2,
        "enter",
        true,
        1,
        0,
        Some("Play_Breathing"),
        Some("aliveAndVoiceNotPlaying"),
    );
    // Same-frame Enter deliberately precedes Exit in the input. The schedule
    // must reorder them by phase, using ordinals only after phase.
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [
                { "trigger": "Play_A", "media": [{ "mediaId": 42, "defaultBranch": true }] },
                { "trigger": "Play_B", "media": [{ "mediaId": 43, "defaultBranch": true }] },
                { "trigger": "Play_Breathing", "media": [{ "mediaId": 44, "defaultBranch": true }] },
                { "trigger": "Stop_A", "media": [{ "mediaId": 99, "defaultBranch": true }] }
            ]
        },
        "animations": [{
            "name": "alligator_bite",
            "extras": {
                "cryDuration": 2.0,
                "crySampleRate": 30,
                "cryMannequinAudio": [{
                    "trigger": "Bite",
                    "characterEvent": "Bite",
                    "joint": "",
                    "startTime": 0.1,
                    "fragment": "Attack_Bite",
                    "dispatches": [b_enter, a_exit, conditional, a_enter]
                }]
            }
        }]
    });

    let schedule = build_blend_schedule(&document, root).unwrap();
    let clip = &schedule.clips[0];
    assert_eq!(clip.name, "alligator_bite");
    assert_eq!(clip.sounds.len(), 2);
    assert!(clip.sounds[0].wav.ends_with("42.wav"));
    assert_eq!(clip.sounds[0].frame, absolute_frame(0.1, 30));
    assert_eq!(clip.sounds[0].end_frame, Some(absolute_frame(0.5, 30)));
    assert!(clip.sounds[1].wav.ends_with("43.wav"));
    assert_eq!(clip.sounds[1].frame, absolute_frame(0.5, 30));
    assert_eq!(clip.sounds[1].end_frame, None, "no fabricated final stop");
    assert!(
        clip.sounds
            .iter()
            .all(|sound| !sound.wav.ends_with("99.wav"))
    );
    assert!(
        clip.sounds
            .iter()
            .all(|sound| !sound.wav.ends_with("44.wav"))
    );
    assert_eq!(
        clip.dispatches[2]
            .get("phase")
            .and_then(serde_json::Value::as_str),
        Some("exit")
    );
    assert_eq!(
        clip.dispatches[3]
            .get("phase")
            .and_then(serde_json::Value::as_str),
        Some("enter")
    );
    assert_eq!(
        clip.dispatches.len(),
        4,
        "conditional breathing remains metadata"
    );
}

#[test]
fn multiple_tag_variants_get_separate_scenes_and_never_mix() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for id in [10u32, 20] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }
    let bear = dispatch(
        "Bear",
        "Bite",
        0.1,
        "enter",
        true,
        0,
        0,
        Some("Play_Bear"),
        None,
    );
    let wolf = dispatch(
        "Wolf",
        "Bite",
        0.1,
        "enter",
        true,
        0,
        0,
        Some("Play_Wolf"),
        None,
    );
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [
                { "trigger": "Play_Bear", "media": [{ "mediaId": 10, "defaultBranch": true }] },
                { "trigger": "Play_Wolf", "media": [{ "mediaId": 20, "defaultBranch": true }] }
            ]
        },
        "animations": [{
            "name": "bite",
            "extras": {
                "cryDuration": 1.0,
                "crySampleRate": 30,
                "cryMannequinAudio": [{
                    "trigger": "Bite", "characterEvent": "Bite", "joint": "",
                    "startTime": 0.1, "fragment": "Attack_Bite",
                    "dispatches": [wolf, bear]
                }]
            }
        }]
    });

    let schedule = build_blend_schedule(&document, root).unwrap();
    assert_eq!(schedule.clips.len(), 2);
    assert_eq!(schedule.clips[0].name, "bite [audio:Bear]");
    assert_eq!(schedule.clips[1].name, "bite [audio:Wolf]");
    assert!(schedule.clips[0].sounds[0].wav.ends_with("10.wav"));
    assert!(schedule.clips[1].sounds[0].wav.ends_with("20.wav"));
    assert_eq!(schedule.clips[0].dispatches.len(), 1);
    assert_eq!(schedule.clips[1].dispatches.len(), 1);
}

#[test]
fn blend_schedule_falls_back_to_lowest_when_no_default_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for id in [7u32, 3u32] {
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(id),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
    }
    let document = serde_json::json!({
        "extras": {
            "audioTriggers": [{
                "trigger": "step",
                "media": [{ "mediaId": 7 }, { "mediaId": 3 }],
            }],
        },
        "animations": [{
            "name": "walk",
            "extras": {
                "cryDuration": 1.0,
                "crySampleRate": 30,
                "cryEvents": [
                    { "parameter": "step", "normalizedTime": 0.1 },
                    { "parameter": "step", "normalizedTime": 0.2 },
                ],
            },
        }],
    });
    let schedule = build_blend_schedule(&document, root).unwrap();
    let sounds = &schedule.clips[0].sounds;
    // No default branch → the single lowest media id repeats (old behavior).
    assert_eq!(sounds.len(), 2);
    assert!(sounds.iter().all(|sound| sound.wav.ends_with("3.wav")));
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    tag: &str,
    event: &str,
    time: f64,
    phase: &str,
    enabled: bool,
    proc_layer_ordinal: u64,
    procedural_ordinal: u64,
    control: Option<&str>,
    condition: Option<&str>,
) -> serde_json::Value {
    let mut operations = Vec::new();
    if let Some(control) = control {
        let mut operation = serde_json::json!({
            "kind": "audioControl",
            "control": control,
            "spatialMode": "joint",
            "validityGated": true
        });
        if let Some(condition) = condition {
            operation
                .as_object_mut()
                .unwrap()
                .insert("condition".to_owned(), condition.into());
        }
        operations.push(operation);
    }
    serde_json::json!({
        "time": time,
        "phase": phase,
        "enabled": enabled,
        "receiver": "commonNpcAudio",
        "receiverScriptPath": "scripts/audio/npcs/commonnpc_audio.lua",
        "scenePath": "slices/creature.dynamicslice",
        "entityId": 9,
        "entityName": "creature",
        "validTag": tag,
        "validTagCrc": cry_audio::az_crc32(tag.as_bytes()),
        "fragment": "Attack_Bite",
        "procLayerOrdinal": proc_layer_ordinal,
        "proceduralOrdinal": procedural_ordinal,
        "characterEvent": event,
        "joint": "",
        "producer": "mannequinCharacterEvent",
        "operations": operations
    })
}
