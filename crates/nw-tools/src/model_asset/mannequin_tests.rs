use super::*;
use crate::model_asset::tests::ContextSource;
use std::collections::BTreeMap;

const ADB: &[u8] = br#"
        <AnimDB FragDef="animations/mannequin/adb/creature_actions.xml"
                TagDef="animations/mannequin/adb/creature_tags.xml">
          <FragmentList><Attack_Bite><Fragment Tags="Young">
            <AnimLayer><Blend/><Animation name="creature_bite"/></AnimLayer>
            <ProcLayer><Blend ExitTime="0.25"/><Procedural type="CharacterEvent">
              <ProceduralParams><CharacterEventName value="Bite"/><AttachmentJoint value=""/></ProceduralParams>
            </Procedural></ProcLayer>
            <ProcLayer><Blend StartTime="0.1"/><Procedural type="Audio">
              <ProceduralParams><StartTrigger value="play_direct_bite"/><AttachmentJoint value="bind_mouth"/></ProceduralParams>
            </Procedural></ProcLayer>
          </Fragment></Attack_Bite></FragmentList>
        </AnimDB>
    "#;

#[test]
fn character_definition_and_mannequin_family_stay_on_same_entity() {
    const CDF: &str = "objects/characters/npc/example/example.cdf";
    let valid = entity_xml(
        1,
        "valid",
        &format!(
            "{}{}",
            action_component(),
            character_definition_component(CDF)
        ),
    );
    let split_action = entity_xml(2, "action only", &action_component());
    let split_cdf = entity_xml(3, "cdf only", &character_definition_component(CDF));
    let source = source_with_scene(scene(&[valid, split_action, split_cdf], false)).with(
        CDF,
        b"<CharacterDefinition><Model File=\"objects/example.chr\"/></CharacterDefinition>",
    );

    let contexts = character_mannequin_contexts(&source, &[SCENE.to_owned()]).unwrap();

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].cdf_path, CDF);
    assert_eq!(contexts[0].family.adb_paths, [ADB_PATH]);
    assert!(contexts[0].family.controller_paths.is_empty());
}

#[test]
fn catalog_character_definition_asset_selects_same_entity_context() {
    const CDF: &str = "objects/characters/npc/example/catalog.cdf";
    let cdf_guid = uuid!("99999999-bbbb-cccc-dddd-eeeeeeeeeeee");
    let entity = entity_xml(
        4,
        "catalog",
        &format!(
            "{}{}",
            action_component(),
            character_definition_asset_component(cdf_guid, CDF)
        ),
    );
    let source = CatalogSource {
        inner: source_with_scene(scene(&[entity], false)).with(
            CDF,
            b"<CharacterDefinition><Model File=\"objects/example.chr\"/></CharacterDefinition>",
        ),
        by_id: BTreeMap::from([(nw_asset::AssetId::new(cdf_guid, 0), CDF.to_owned())]),
    };

    let contexts = character_mannequin_contexts(&source, &[SCENE.to_owned()]).unwrap();

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].cdf_path, CDF);
}

#[test]
fn only_same_entity_components_form_a_receiver_context() {
    let source = source_with_scene(scene(
        &[
            entity(1, "commonnpc_audio misleading", true, true, None, 11),
            entity(
                2,
                "ActionList misleading",
                true,
                false,
                Some(COMMON_SCRIPT),
                12,
            ),
            entity(3, "valid", true, true, Some(COMMON_SCRIPT), 13),
        ],
        true,
    ));

    let contexts = discover_mannequin_entities(&source, &[SCENE.to_owned()])
        .unwrap()
        .contexts;

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].entity_id, 3);
    assert_eq!(contexts[0].entity_name, "valid");
    assert_eq!(contexts[0].tag_crcs, vec![13]);
}

#[test]
fn same_adb_from_multiple_entities_remains_stable_alternatives() {
    let source = source_with_scene(scene(
        &[
            entity(20, "second", true, true, Some(COMMON_SCRIPT), 200),
            entity(10, "first", true, true, Some(COMMON_SCRIPT), 100),
        ],
        false,
    ));
    let mut resolved = empty_resolved();

    attach_fragment_audio(&source, &[SCENE.to_owned()], &mut resolved).unwrap();

    let entries = resolved
        .extras
        .mannequin_audio
        .iter()
        .filter(|entry| entry.animation == "creature_bite" && entry.clips[0].context.is_some())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].clips[0].context.as_ref().unwrap().entity_id, 10);
    assert_eq!(entries[1].clips[0].context.as_ref().unwrap().entity_id, 20);
    assert_eq!(
        entries[0].clips[0].context.as_ref().unwrap().scene_path,
        SCENE
    );
    assert_eq!(entries[0].clips[0].proc_layer_ordinal, 0);
    assert_eq!(entries[0].clips[0].procedural_ordinal, 0);
    assert_eq!(entries[0].clips[0].exit_time, None);
}

#[test]
fn direct_audio_survives_without_character_event_receiver() {
    let source = source_with_scene(scene(
        &[entity(25, "direct only", false, true, None, 0)],
        false,
    ));
    let mut resolved = empty_resolved();

    attach_fragment_audio(&source, &[SCENE.to_owned()], &mut resolved).unwrap();

    let direct = resolved
        .extras
        .mannequin_audio
        .iter()
        .flat_map(|entry| &entry.clips)
        .find(|clip| clip.trigger == "play_direct_bite")
        .expect("direct Mannequin Audio clip");
    assert!(direct.context.is_none());
    assert_eq!(
        direct.producer,
        nw_model::CryMannequinAudioProducer::MannequinAudio
    );
}

#[test]
fn nw_tag_component_does_not_form_context() {
    let source = source_with_scene(scene(&[entity_with_nw_tag()], false));

    let contexts = discover_mannequin_entities(&source, &[SCENE.to_owned()])
        .unwrap()
        .contexts;

    assert!(contexts.is_empty());
}

#[test]
fn bone_audio_preserves_aligned_bindings_and_spawn_mode() {
    use nw_objectstream::{Element, types};

    let string_array = |name: &str, values: &[&str]| {
        script_property_element(
            name,
            Element::new(Uuid::from_u128(101))
                .with_field("values")
                .with_children(
                    values
                        .iter()
                        .map(|value| {
                            Element::new(types::AZSTD_STRING).with_data(value.as_bytes().to_vec())
                        })
                        .collect::<Vec<_>>(),
                ),
        )
    };
    let entity_array = script_property_element(
        "audioEntity",
        Element::new(Uuid::from_u128(102))
            .with_field("values")
            .with_children(vec![entity_id_element(71), entity_id_element(72)]),
    );
    let spawn = script_property_element(
        "spawnSound",
        Element::new(types::BOOL).with_field("value").with_data([1]),
    );
    let properties = Element::new(Uuid::from_u128(100)).with_children(vec![
        entity_array,
        string_array("characterEventName", &["Bite", "Tail"]),
        string_array("wwiseEvent", &["Play_Bite", "Play_Tail"]),
        spawn,
    ]);

    let (bindings, spawn_sound) = bone_audio_properties(Some(&properties)).unwrap();

    assert!(spawn_sound);
    assert_eq!(
        bindings,
        vec![
            nw_model::CryBoneAudioBinding {
                character_event: "Bite".to_owned(),
                audio_entity: 71,
                wwise_event: "Play_Bite".to_owned(),
            },
            nw_model::CryBoneAudioBinding {
                character_event: "Tail".to_owned(),
                audio_entity: 72,
                wwise_event: "Play_Tail".to_owned(),
            },
        ]
    );
}

#[test]
fn catalog_script_path_precedes_hint_and_hint_is_filesystem_fallback() {
    let script_guid = uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    let xml = scene(
        &[entity_with_script_asset(script_guid, COMMON_SCRIPT)],
        false,
    );
    let fallback = source_with_scene(xml.clone());
    let catalog = CatalogSource {
        inner: source_with_scene(xml),
        by_id: BTreeMap::from([(
            nw_asset::AssetId::new(script_guid, 0),
            MOUNT_SCRIPT.to_owned(),
        )]),
    };

    let fallback_contexts = discover_mannequin_entities(&fallback, &[SCENE.to_owned()])
        .unwrap()
        .contexts;
    let catalog_contexts = discover_mannequin_entities(&catalog, &[SCENE.to_owned()])
        .unwrap()
        .contexts;

    assert!(matches!(
        fallback_contexts[0].receivers[0],
        nw_model::CryCharacterEventReceiver::CommonNpcAudio { .. }
    ));
    assert!(matches!(
        catalog_contexts[0].receivers[0],
        nw_model::CryCharacterEventReceiver::MountAudio { .. }
    ));
}

#[test]
fn catalog_miss_does_not_use_serialized_script_hint() {
    let script_guid = uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    let source = CatalogSource {
        inner: source_with_scene(scene(
            &[entity_with_script_asset(script_guid, COMMON_SCRIPT)],
            false,
        )),
        by_id: BTreeMap::new(),
    };

    let contexts = discover_mannequin_entities(&source, &[SCENE.to_owned()])
        .unwrap()
        .contexts;

    assert!(contexts.is_empty());
}

const SCENE: &str = "slices/creature.dynamicslice";
const ADB_PATH: &str = "animations/mannequin/adb/creature_audio.adb";
const COMMON_SCRIPT: &str = "scripts/audio/npcs/commonnpc_audio.lua";
const MOUNT_SCRIPT: &str = "scripts/audio/players/mountaudio.lua";

fn source_with_scene(scene: Vec<u8>) -> ContextSource {
    ContextSource::default()
        .with(SCENE, scene)
        .with(ADB_PATH, ADB)
        .with(COMMON_SCRIPT, b"-- filesystem receiver fixture")
        .with(
            "animations/mannequin/adb/creature_actions.xml",
            b"<TagDefinition version=\"2\"><Tags/></TagDefinition>".to_vec(),
        )
        .with(
            "animations/mannequin/adb/creature_tags.xml",
            b"<TagDefinition version=\"2\"><Tags/></TagDefinition>".to_vec(),
        )
}

fn scene(entities: &[String], misleading_root_components: bool) -> Vec<u8> {
    let root_noise = if misleading_root_components {
        format!(
            "{}{}",
            action_component(),
            script_component(uuid!("11111111-2222-3333-4444-555555555555"), COMMON_SCRIPT)
        )
    } else {
        String::new()
    };
    format!(
            r#"<ObjectStream version="3"><Class name="SliceComponent" type="{{00000000-0000-0000-0000-000000000001}}">{root_noise}{}</Class></ObjectStream>"#,
            entities.join("")
        )
        .into_bytes()
}

fn entity(
    id: u64,
    name: &str,
    ordinary_tag: bool,
    action: bool,
    script: Option<&str>,
    tag: u32,
) -> String {
    let mut components = String::new();
    if ordinary_tag {
        components.push_str(&tag_component(tag));
    }
    if action {
        components.push_str(&action_component());
    }
    if let Some(script) = script {
        components.push_str(&script_component(
            uuid!("11111111-2222-3333-4444-555555555555"),
            script,
        ));
    }
    entity_xml(id, name, &components)
}

fn entity_with_nw_tag() -> String {
    let components = format!(
        r#"<Class name="NWTagComponent" type="{{5B7EC8B0-530E-444F-B10F-CD2F30017188}}"/>{}{}"#,
        action_component(),
        script_component(uuid!("11111111-2222-3333-4444-555555555555"), COMMON_SCRIPT)
    );
    entity_xml(30, "nw only", &components)
}

fn entity_with_script_asset(guid: Uuid, hint: &str) -> String {
    let components = format!(
        "{}{}{}",
        tag_component(77),
        action_component(),
        script_component(guid, hint)
    );
    entity_xml(40, "catalog", &components)
}

fn entity_xml(id: u64, name: &str, components: &str) -> String {
    format!(
        r#"<Class name="AZ::Entity" type="{{75651658-8663-478D-9090-2432DFCAFA44}}"><Class name="AZ::EntityId" field="Id" type="{{6383F1D3-BB27-4E6B-A49A-6409B2059EAA}}"><Class name="AZ::u64" field="id" value="{id}" type="{{D6597933-47CD-4FC8-B911-63F3E2B0993A}}"/></Class><Class name="AZStd::string" field="Name" value="{name}" type="{{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}}"/><Class name="Components" field="Components" type="{{0D23B755-6E8F-5C6C-B7C9-A352A55DC1DF}}">{components}</Class></Class>"#
    )
}

fn tag_component(tag: u32) -> String {
    format!(
        r#"<Class name="TagComponent" type="{{0F16A377-EAA0-47D2-8472-9EAAA680B169}}"><Class name="Tags" field="Tags" type="{{93BBB90E-EBB4-507D-89B6-E4921FE44AFF}}"><Class name="AZ::Crc32" type="{{9F4E062E-06A0-46D4-85DF-E0DA96467D3A}}"><Class name="unsigned int" field="value" value="{tag}" type="{{43DA906B-7DEF-4CA8-9790-854106D3F983}}"/></Class></Class></Class>"#
    )
}

fn action_component() -> String {
    format!(
        r#"<Class name="ActionListComponent" type="{{30ED0ACE-51DD-48B9-BA41-2FA6775CD106}}"><Class name="AZStd::string" field="m_animationDatabase" value="{ADB_PATH}" type="{{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}}"/></Class>"#
    )
}

fn character_definition_component(cdf: &str) -> String {
    format!(
        r#"<Class name="CharacterComponent" type="{{15407CAA-4970-4D06-8B5C-612FBA11BB45}}"><Class name="AzFramework::SimpleAssetReference&lt;CharacterDefinitionAsset&gt;" field="m_cdfPath" type="{{A1342558-687A-406A-8BE4-784D6546589D}}"><Class name="SimpleAssetReferenceBase" field="BaseClass1" type="{{E16CA6C5-5C78-4AD9-8E9B-F8C1FB4D1DB8}}"><Class name="AZStd::string" field="AssetPath" value="{cdf}" type="{{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}}"/></Class></Class></Class>"#
    )
}

fn character_definition_asset_component(guid: Uuid, hint: &str) -> String {
    format!(
        r#"<Class name="CharacterComponent" type="{{15407CAA-4970-4D06-8B5C-612FBA11BB45}}"><Class name="Asset" field="m_characterDefinition" value="id={{{guid}}}:0,type={{82557326-4AE3-416C-95D6-C70635AB7588}},hint={{{hint}}}" type="{{77A19D40-8731-4D3C-9041-1B43047366A4}}"/></Class>"#
    )
}

fn script_component(guid: Uuid, hint: &str) -> String {
    format!(
        r#"<Class name="AzFramework::ScriptComponent" type="{{8D1BC97E-C55D-4D34-A460-E63C57CD0D4B}}"><Class name="Asset" field="Script" value="id={{{guid}}}:0,type={{82557326-4AE3-416C-95D6-C70635AB7588}},hint={{{hint}}}" type="{{77A19D40-8731-4D3C-9041-1B43047366A4}}"/><Class name="Properties" field="Properties" type="{{79682522-2F81-4B36-9FC2-A091C7504F7F}}"/></Class>"#
    )
}

fn script_property_element(
    name: &str,
    value: nw_objectstream::Element,
) -> nw_objectstream::Element {
    use nw_objectstream::{Element, types};

    Element::new(Uuid::from_u128(103)).with_children(vec![
        Element::new(SCRIPT_PROPERTY_ID)
            .with_field("BaseClass1")
            .with_children(vec![
                Element::new(types::AZSTD_STRING)
                    .with_field("name")
                    .with_data(name.as_bytes().to_vec()),
            ]),
        value,
    ])
}

fn entity_id_element(id: u64) -> nw_objectstream::Element {
    use nw_objectstream::{Element, types};

    Element::new(types::ENTITY_ID).with_children(vec![
        Element::new(types::AZ_U64)
            .with_field("id")
            .with_data(id.to_be_bytes()),
    ])
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
        parsed_animation_assets: std::collections::HashSet::new(),
    }
}

struct CatalogSource {
    inner: ContextSource,
    by_id: BTreeMap<nw_asset::AssetId, String>,
}

impl nw_asset_graph::AssetSource for CatalogSource {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.inner.read(path)
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>> {
        self.inner.matching_paths(pattern)
    }

    fn path_by_id(&self, asset_id: nw_asset::AssetId) -> Option<String> {
        self.by_id.get(&asset_id).cloned()
    }
}

impl AssetSource for CatalogSource {
    fn materials(&self, cgf: &[u8], mesh: &MeshRef) -> Option<nw_model::MaterialSet> {
        self.inner.materials(cgf, mesh)
    }
}
