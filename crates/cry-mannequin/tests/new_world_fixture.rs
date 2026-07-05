use std::fs;
use std::path::PathBuf;

use cry_mannequin::{
    BlendSpaceSource, CombinedBlendSpaceSource, MannequinAnimationDatabaseSource,
    MannequinControllerDefinitionSource, MannequinTagDefinitionEntry, MannequinTagDefinitionSource,
};

const FIXTURE_ROOT: &str = "E:/Projects/new-world/tmp/mannequin_e1c_raw";
const BLENDSPACE_FIXTURE_ROOT: &str = "E:/Projects/new-world/tmp/blendspace_e1d_raw";
const LOST_COMMANDER_ANIMS: &str = "animations/mannequin/adb/isleofnight/lostcommander_anims.adb";
const LOST_COMMANDER_SWORD_ANIMS: &str =
    "animations/mannequin/adb/isleofnight/lostcommander_swordanims.adb";
const LOST_COMMANDER_ACTIONS: &str = "animations/mannequin/adb/isleofnight/npc_monster/lostcommander_family/lostcommander_actions.xml";
const LOST_COMMANDER_TAGS: &str =
    "animations/mannequin/adb/isleofnight/npc_monster/lostcommander_family/lostcommandertags.xml";
const LOST_COMMANDER_CONTROLLER: &str = "animations/mannequin/adb/isleofnight/npc_monster/lostcommander_family/lostcommander_controllerdefs.xml";
const MOUNT_GRUNT_WALK_BLEND: &str = "animations/gameplay/character/npc/mounts/grunt/blendspace/mount_grunt_movement_walk_blend.bspace";
const BISON_TURN_BLEND_COMB: &str =
    "animations/gameplay/character/npc/natural/prey/buffalo/blendspaces/bison_turn_blendcomb.comb";

#[test]
fn decodes_lostcommander_anims_adb_with_real_animation_options() {
    let bytes = read_fixture(LOST_COMMANDER_ANIMS);
    let source = MannequinAnimationDatabaseSource::from_legacy_with_motion_resolver(
        LOST_COMMANDER_ANIMS,
        &bytes,
        |name| match name {
            "champ_sword_backhit_to_onfront" => Some("animations/gameplay/character/npc/supernatural/champion/sword/combat/reactions/champ_sword_backhit_to_onfront.anim.glb".to_string()),
            "champ_sword_alertpoint" => Some("animations/gameplay/character/npc/supernatural/champion/sword/combat/navigation/champ_sword_alertpoint.anim.glb".to_string()),
            "thorpe_cleavecombo_fail" => Some("animations/gameplay/character/npc/supernatural/thorpe/combat/thorpe_cleavecombo_fail.anim.glb".to_string()),
            _ => None,
        },
    )
    .expect("decode real lostcommander_anims.adb fixture");

    assert_eq!(source.database.fragment_groups.len(), 72);
    assert_eq!(fragment_count(&source), 155);
    assert_eq!(animation_count(&source), 204);
    assert!(fragments(&source).all(|fragment| {
        fragment
            .animation_layers
            .iter()
            .flat_map(|layer| &layer.animations)
            .any(|animation| !animation.name.is_empty())
    }));

    let first_group = &source.database.fragment_groups[0];
    assert_eq!(first_group.name, "r_Death");
    let first_animation = &first_group.fragments[0].animation_layers[0].animations[0];
    assert_eq!(first_animation.name, "champ_sword_backhit_to_onfront");
    assert_eq!(
        first_animation.motion_path.as_deref(),
        Some(
            "animations/gameplay/character/npc/supernatural/champion/sword/combat/reactions/champ_sword_backhit_to_onfront.anim.glb"
        )
    );
    assert_eq!(first_animation.unresolved_motion_reason, None);
}

#[test]
fn decodes_lostcommander_swordanims_as_procedural_mannequin_database() {
    let bytes = read_fixture(LOST_COMMANDER_SWORD_ANIMS);
    let source = MannequinAnimationDatabaseSource::from_legacy(LOST_COMMANDER_SWORD_ANIMS, &bytes)
        .expect("decode real lostcommander_swordanims.adb fixture");

    assert_eq!(source.database.fragment_groups.len(), 9);
    assert_eq!(fragment_count(&source), 15);
    assert_eq!(animation_count(&source), 0);
    assert_eq!(procedural_count(&source), 50);
}

#[test]
fn decodes_lostcommander_tag_and_controller_definitions() {
    let actions = MannequinTagDefinitionSource::from_legacy(
        LOST_COMMANDER_ACTIONS,
        &read_fixture(LOST_COMMANDER_ACTIONS),
    )
    .expect("decode real LostCommander_Actions.xml fixture");
    assert_eq!(actions.version.as_deref(), Some("2"));
    assert_eq!(flattened_tag_count(&actions), 77);
    assert_eq!(actions.entries.len(), 77);

    let tags = MannequinTagDefinitionSource::from_legacy(
        LOST_COMMANDER_TAGS,
        &read_fixture(LOST_COMMANDER_TAGS),
    )
    .expect("decode real LostCommanderTags.xml fixture");
    assert_eq!(tags.version.as_deref(), Some("2"));
    assert_eq!(tags.entries.len(), 23);
    assert_eq!(group_count(&tags), 9);
    assert_eq!(flattened_tag_count(&tags), 57);

    let controller = MannequinControllerDefinitionSource::from_legacy(
        LOST_COMMANDER_CONTROLLER,
        &read_fixture(LOST_COMMANDER_CONTROLLER),
    )
    .expect("decode real LostCommander controllerdefs.xml fixture");
    assert_eq!(controller.fragment_definitions.len(), 77);
    assert_eq!(controller.scope_contexts.len(), 4);
    assert_eq!(controller.scopes.len(), 8);
    assert_eq!(
        controller.tags.filename,
        "Animations/Mannequin/ADB/IsleOfNight/NPC_Monster/LostCommander_Family/LostCommanderTags.xml"
    );
    assert_eq!(
        controller.fragments.filename,
        "Animations/Mannequin/ADB/IsleOfNight/NPC_Monster/LostCommander_Family/LostCommander_Actions.xml"
    );
    assert_eq!(
        controller
            .fragment_definitions
            .iter()
            .map(|fragment| fragment.overrides.len())
            .sum::<usize>(),
        24
    );
}

#[test]
fn decodes_real_mount_grunt_movement_walk_bspace_fixture() {
    let bytes = read_blend_fixture(MOUNT_GRUNT_WALK_BLEND);
    let source = BlendSpaceSource::from_legacy_with_motion_resolver(
        MOUNT_GRUNT_WALK_BLEND,
        &bytes,
        |name| {
            Some(format!(
                "animations/gameplay/character/npc/mounts/grunt/navigation/{name}.anim.glb"
            ))
        },
    )
    .expect("decode real mount_grunt_movement_walk_blend.bspace fixture");

    assert_eq!(source.blend_space.dimensions.len(), 3);
    assert_eq!(source.blend_space.dimensions[0].name, "MoveSpeed");
    assert_eq!(source.blend_space.dimensions[0].parameter_id, Some(0));
    assert_eq!(source.blend_space.dimensions[1].name, "TurnSpeed");
    assert_eq!(source.blend_space.dimensions[1].parameter_id, Some(1));
    assert_eq!(source.blend_space.dimensions[2].name, "TravelSlope");
    assert_eq!(source.blend_space.dimensions[2].parameter_id, Some(3));
    assert_eq!(source.blend_space.examples.len(), 18);
    assert!(source.blend_space.examples.iter().all(|example| {
        example.animation.motion_path.is_some()
            && example.animation.unresolved_motion_reason.is_none()
            && example.coordinates.len() == source.blend_space.dimensions.len()
            && example
                .coordinates
                .iter()
                .all(|coordinate| coordinate.value.is_some())
    }));
    let first = &source.blend_space.examples[0];
    assert_eq!(first.animation.name, "mount_grunt_walk_turn_r_downhill");
    assert_eq!(
        first.animation.motion_path.as_deref(),
        Some(
            "animations/gameplay/character/npc/mounts/grunt/navigation/mount_grunt_walk_turn_r_downhill.anim.glb"
        )
    );
    assert_eq!(first.coordinates[0].value, Some(3.5));
    assert_eq!(first.coordinates[1].value, Some(-2.5));
    assert_eq!(first.coordinates[2].value, Some(-0.8));
    assert!(!source.blend_space.annotations.is_empty());
    assert!(!source.blend_space.virtual_examples.is_empty());
}

#[test]
fn decodes_real_bison_turn_comb_fixture() {
    let bytes = read_blend_fixture(BISON_TURN_BLEND_COMB);
    let source = CombinedBlendSpaceSource::from_legacy(BISON_TURN_BLEND_COMB, &bytes)
        .expect("decode real bison_turn_blendcomb.comb fixture");

    assert_eq!(source.combined_blend_space.dimensions.len(), 1);
    assert_eq!(
        source.combined_blend_space.dimensions[0].name,
        "DesiredFacing"
    );
    assert_eq!(source.combined_blend_space.dimensions[0].parameter_id, None);
    assert_eq!(
        source
            .combined_blend_space
            .additional_extraction
            .iter()
            .map(|parameter| parameter.parameter_id)
            .collect::<Vec<_>>(),
        [Some(2), Some(1), Some(0)]
    );
    assert_eq!(source.combined_blend_space.blend_spaces.len(), 2);
    assert!(
        source
            .combined_blend_space
            .blend_spaces
            .iter()
            .all(|blend_space| blend_space.authoring_path.is_some()
                && blend_space.unresolved_reference_reason.is_none())
    );
    assert_eq!(
        source.combined_blend_space.blend_spaces[0]
            .authoring_path
            .as_deref(),
        Some(
            "animations/gameplay/character/npc/natural/prey/buffalo/blendspaces/bison_turn_blend_right.bspace.ron"
        )
    );
}

fn read_fixture(relative: &str) -> Vec<u8> {
    let path = PathBuf::from(FIXTURE_ROOT).join(relative);
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "read real New World Mannequin fixture {}: {error}; regenerate it from Steam PAKs with nw-extract assets extract",
            path.display()
        )
    })
}

fn read_blend_fixture(relative: &str) -> Vec<u8> {
    let path = PathBuf::from(BLENDSPACE_FIXTURE_ROOT).join(relative);
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "read real New World blend-space fixture {}: {error}; regenerate it from Steam PAKs with nw-extract assets extract",
            path.display()
        )
    })
}

fn fragments(
    source: &MannequinAnimationDatabaseSource,
) -> impl Iterator<Item = &cry_mannequin::MannequinFragment> {
    source
        .database
        .fragment_groups
        .iter()
        .flat_map(|group| group.fragments.iter())
}

fn fragment_count(source: &MannequinAnimationDatabaseSource) -> usize {
    fragments(source).count()
}

fn animation_count(source: &MannequinAnimationDatabaseSource) -> usize {
    fragments(source)
        .flat_map(|fragment| &fragment.animation_layers)
        .flat_map(|layer| &layer.animations)
        .count()
}

fn procedural_count(source: &MannequinAnimationDatabaseSource) -> usize {
    fragments(source)
        .flat_map(|fragment| &fragment.procedural_layers)
        .flat_map(|layer| &layer.procedurals)
        .count()
}

fn flattened_tag_count(source: &MannequinTagDefinitionSource) -> usize {
    source
        .entries
        .iter()
        .map(|entry| match entry {
            MannequinTagDefinitionEntry::Tag(_) => 1,
            MannequinTagDefinitionEntry::Group(group) => group.tags.len(),
        })
        .sum()
}

fn group_count(source: &MannequinTagDefinitionSource) -> usize {
    source
        .entries
        .iter()
        .filter(|entry| matches!(entry, MannequinTagDefinitionEntry::Group(_)))
        .count()
}
