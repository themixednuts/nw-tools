use super::*;
use crate::model_asset::tests::{ContextSource, EmptySource};

#[test]
fn audio_trigger_resolution_is_catalog_driven_not_name_shaped() {
    let controls = cry_audio::AudioControlsSource::from_xml(
        "libs/gameaudio/wwise/atl_controls.xml",
        r#"<ATLConfig atl_name="main">
             <AudioTriggers>
               <ATLTrigger atl_name="blend_ftsp_alligator">
                 <WwiseEvent wwise_name="blend_ftsp_alligator"/>
               </ATLTrigger>
             </AudioTriggers>
           </ATLConfig>"#,
    )
    .unwrap();
    let mut event_ids = std::collections::BTreeMap::new();
    event_ids.insert("Play_BareEvent".to_owned(), 4242u32);
    let catalogs = AudioCatalogs {
        controls: vec![controls],
        trigger_bank_map: Vec::new(),
        crc_to_bank: std::collections::HashMap::new(),
        preload_groups: Vec::new(),
        event_ids,
        audio_tags: std::collections::BTreeMap::new(),
    };

    let mut banks = BankStore::new(&EmptySource);

    assert!(
        resolve_one_audio_trigger(
            &EmptySource,
            &mut banks,
            &catalogs,
            "blend_ftsp_unlisted",
            false
        )
        .unwrap()
        .is_none()
    );

    let atl = resolve_one_audio_trigger(
        &EmptySource,
        &mut banks,
        &catalogs,
        "blend_ftsp_alligator",
        false,
    )
    .unwrap()
    .expect("ATL trigger resolves");
    assert_eq!(atl.wwise_events.len(), 1);
    assert_eq!(atl.wwise_events[0].name, "blend_ftsp_alligator");

    let bare =
        resolve_one_audio_trigger(&EmptySource, &mut banks, &catalogs, "Play_BareEvent", false)
            .unwrap()
            .expect("bare event name resolves");
    assert_eq!(bare.wwise_events[0].id, Some(4242));
}

#[test]
fn generated_controls_are_collected_regardless_of_phase_or_condition() {
    let dispatch = |enabled, control: &str, condition| nw_model::CryCharacterEventDispatch {
        time: 0.0,
        phase: if enabled {
            nw_model::CryCharacterEventPhase::Enter
        } else {
            nw_model::CryCharacterEventPhase::Exit
        },
        enabled,
        receiver: nw_model::CryCharacterEventReceiverKind::CommonNpcAudio,
        receiver_script_path: "scripts/audio/npcs/commonnpc_audio.lua".to_owned(),
        scene_path: "slices/creature.dynamicslice".to_owned(),
        entity_id: 7,
        entity_name: "creature".to_owned(),
        valid_tag: Some("Alligator".to_owned()),
        valid_tag_crc: Some(cry_audio::az_crc32(b"Alligator")),
        fragment: "Attack".to_owned(),
        proc_layer_ordinal: 0,
        procedural_ordinal: 0,
        character_event: "VOX_Attack".to_owned(),
        joint: String::new(),
        producer: nw_model::CryMannequinAudioProducer::MannequinCharacterEvent,
        operations: vec![nw_model::CryCharacterEventOperation::AudioControl {
            control: control.to_owned(),
            spatial_mode: nw_model::CryAudioSpatialMode::Joint,
            validity_gated: true,
            target_entity: None,
            condition,
        }],
    };
    let mut resolved = empty_resolved();
    resolved.extras.mannequin_audio = vec![nw_model::CryMannequinAnimationAudio {
        animation: "attack".to_owned(),
        clips: vec![nw_model::CryMannequinAudioClip {
            trigger: "VOX_Attack".to_owned(),
            stop_trigger: None,
            character_event: Some("VOX_Attack".to_owned()),
            joint: String::new(),
            start_time: 0.0,
            exit_time: Some(1.0),
            option_on_enter: Some(nw_model::CryCharacterEventOption::Enable),
            option_on_exit: Some(nw_model::CryCharacterEventOption::Disable),
            proc_layer_ordinal: 0,
            procedural_ordinal: 0,
            producer: nw_model::CryMannequinAudioProducer::MannequinCharacterEvent,
            fragment: "Attack".to_owned(),
            tags: String::new(),
            context: None,
            dispatches: vec![
                dispatch(false, "Stop_SFX_Alligator_VOX_Attack", None),
                dispatch(
                    true,
                    "stop_alligator_voice",
                    Some(nw_model::CryCharacterEventCondition::VoicePlaying),
                ),
                dispatch(
                    true,
                    "play_alligator_voice",
                    Some(nw_model::CryCharacterEventCondition::AliveAndVoiceNotPlaying),
                ),
            ],
        }],
    }];

    let mut controls = collect_animation_audio_triggers(&resolved)
        .into_iter()
        .map(|candidate| candidate.parameter)
        .collect::<Vec<_>>();
    controls.sort();

    assert_eq!(
        controls,
        [
            "Stop_SFX_Alligator_VOX_Attack",
            "play_alligator_voice",
            "stop_alligator_voice",
        ]
    );
}

#[test]
fn audio_event_kind_distinguishes_footstep_from_direct_and_ignores_others() {
    let footstep = cry_animation::AnimationEvent {
        name: "footstep".into(),
        name_lowercase_crc32: 0,
        normalized_time: 0.5,
        normalized_end_time: 0.5,
        parameter: "blend_ftsp_alligator".into(),
        bone: String::new(),
        second_bone: String::new(),
        offset: [0.0; 3],
        direction: [0.0; 3],
        model: String::new(),
        source: cry_xml::XmlElement {
            name: "event".into(),
            attributes: Default::default(),
            children: Vec::new(),
            text: String::new(),
        },
    };
    assert_eq!(audio_event_kind(&footstep), Some(true));
    let direct = cry_animation::AnimationEvent {
        name: "sound".into(),
        ..footstep.clone()
    };
    assert_eq!(audio_event_kind(&direct), Some(false));
    let unrelated = cry_animation::AnimationEvent {
        name: "hit".into(),
        parameter: String::new(),
        ..footstep.clone()
    };
    assert_eq!(audio_event_kind(&unrelated), None);
}

fn empty_catalogs() -> AudioCatalogs {
    AudioCatalogs {
        controls: Vec::new(),
        trigger_bank_map: Vec::new(),
        crc_to_bank: std::collections::HashMap::new(),
        preload_groups: Vec::new(),
        event_ids: std::collections::BTreeMap::new(),
        audio_tags: std::collections::BTreeMap::new(),
    }
}

fn metal_fxlib() -> cry_audio::MaterialEffectsLibrary {
    cry_audio::MaterialEffectsLibrary::from_xml(
        "libs/materialeffects/fxlibs/blend_ftsp_test.xml",
        r#"<FXLib type="playerfootstep">
             <Effect name="metal">
               <Audio trigger="t"><Switch name="SurfaceType" state="metal"/></Audio>
             </Effect>
           </FXLib>"#,
    )
    .unwrap()
}

#[test]
fn surface_name_resolves_only_when_the_hash_validates() {
    let library = metal_fxlib();
    let catalogs = empty_catalogs();
    let metal_id = cry_audio::WwiseNameId::from_name("metal").0;
    assert_eq!(
        resolve_surface_name(metal_id, Some(&library), &catalogs).as_deref(),
        Some("metal")
    );
    assert_eq!(
        resolve_surface_name(0xDEAD_BEEF, Some(&library), &catalogs),
        None
    );
}

#[test]
fn build_surface_media_marks_default_and_keeps_unresolved_branches_by_id() {
    let branches = vec![
        SurfaceBranch {
            switch_id: cry_audio::WwiseNameId::from_name("metal").0,
            is_default: true,
            media: [10u32, 20].into_iter().collect(),
            sequence: vec![20, 10, 20],
        },
        SurfaceBranch {
            switch_id: 999,
            is_default: false,
            media: [30u32].into_iter().collect(),
            sequence: Vec::new(),
        },
    ];
    let library = metal_fxlib();
    let catalogs = empty_catalogs();
    let default_media: std::collections::BTreeSet<u32> = [10, 20].into_iter().collect();

    let out = build_surface_media(
        branches,
        &default_media,
        Some(&library),
        &catalogs,
        "blend_ftsp_test",
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].surface.as_deref(), Some("metal"));
    assert!(out[0].default);
    assert_eq!(out[0].media, vec![10, 20]);
    assert_eq!(out[0].sequence, vec![20, 10, 20]);
    assert_eq!(out[1].surface, None);
    assert!(!out[1].default);
    assert_eq!(out[1].switch_id, 999);
    assert!(out[1].sequence.is_empty());
}

#[test]
fn audio_catalogs_loads_canonical_tags_by_case_folded_crc() {
    let source = ContextSource::default().with(
        AUDIO_TAG_DATA_PATH,
        b"ValidTag,CombatMusicStart,CombatMusicStop\nAlligator,Start,Stop\n",
    );
    let catalogs = AudioCatalogs::load(&source, &empty_resolved()).unwrap();
    let tags = character_event::CharacterEventCatalogs::valid_tags(
        &catalogs,
        &[cry_audio::az_crc32(b"ALLIGATOR")],
    );
    assert_eq!(
        tags,
        vec![character_event::ValidAudioTag {
            name: "Alligator".to_owned(),
            crc: cry_audio::az_crc32(b"alligator"),
        }]
    );
}

#[test]
fn audio_catalogs_builds_event_ids_from_typed_mapping() {
    let path = "sounds/wwise/npc_alligator_events.csv";
    let source =
        ContextSource::default().with(path, b"Name,Id\nPlay_Alligator,7\nStop_Alligator,9\n");
    let mut resolved = empty_resolved();
    resolved
        .extras
        .source_assets
        .push(nw_model::CrySourceAsset {
            path: path.to_owned(),
            kind: nw_model::CrySourceAssetKind::AudioMapping,
            document: serde_json::Value::Null,
        });

    let catalogs = AudioCatalogs::load(&source, &resolved).unwrap();
    assert_eq!(catalogs.event_id("Play_Alligator"), 7);
    assert_eq!(catalogs.event_id("Stop_Alligator"), 9);
    assert_eq!(
        catalogs.canonical_event_name("play_alligator").as_deref(),
        Some("Play_Alligator")
    );
}

#[test]
fn bank_store_parses_each_path_at_most_once_per_resolution() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingSource(AtomicUsize);

    impl nw_asset_graph::AssetSource for CountingSource {
        fn read(&self, _path: &str) -> Option<Vec<u8>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(vec![0; 8])
        }

        fn matching_paths(&self, _pattern: &str) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
    }

    impl AssetSource for CountingSource {
        fn materials(&self, _cgf: &[u8], _mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
            None
        }
    }

    let source = CountingSource(AtomicUsize::new(0));
    let mut store = BankStore::new(&source);
    assert!(store.parsed("sounds/wwise/Creature.bnk").is_none());
    assert!(store.parsed("SOUNDS/WWISE/CREATURE.BNK").is_none());
    assert_eq!(source.0.load(Ordering::Relaxed), 1);
}

fn empty_resolved() -> ResolvedAsset {
    ResolvedAsset {
        model: nw_model::Model {
            meshes: Vec::new(),
            skeletons: Vec::new(),
            auxiliary_nodes: Vec::new(),
        },
        materials: None,
        animations: Vec::new(),
        extras: nw_model::CryAssetExtras::default(),
        physics: nw_model::PhysicsScene::default(),
        animation_asset_evaluations: std::collections::HashMap::new(),
    }
}
