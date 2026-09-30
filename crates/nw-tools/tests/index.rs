//! Content-index tracers through the public `ContentIndex` seam (ticket 006).
//!
//! Membership, original casing, reconstructed locations, resume, and
//! non-blocking start — never numeric CRC values.

use std::fs::File;
use std::time::{Duration, Instant};

use nw_pak::crypak::{Entry, Method, Writer};
use nw_tools::grep::adapter::{FactDraft, FactKind};
use nw_tools::index::{ContentIndex, EntryFormat, Lookup, start_alongside_at};

fn write_pak(dir: &std::path::Path, filename: &str, entries: Vec<Entry>) -> std::path::PathBuf {
    let pak = dir.join(filename);
    let file = File::create(&pak).unwrap();
    Writer::new(file).write(entries).unwrap();
    dir.to_path_buf()
}

/// Minimal valid `.datasheet`: version 0x11, one string column `Name`, one
/// row holding `IronSword`. Same layout as `tests/grep.rs`.
fn minimal_datasheet() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x70];
    bytes[0x00..0x04].copy_from_slice(&0x11u32.to_le_bytes());
    bytes[0x08..0x0c].copy_from_slice(&0u32.to_le_bytes());
    bytes[0x10..0x14].copy_from_slice(&10u32.to_le_bytes());
    bytes[0x38..0x3c].copy_from_slice(&52u32.to_le_bytes());
    bytes[0x44..0x48].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x48..0x4c].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x5c..0x60].copy_from_slice(&0u32.to_le_bytes());
    bytes[0x60..0x64].copy_from_slice(&19i32.to_le_bytes());
    bytes[0x64..0x68].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x68..0x6c].copy_from_slice(&0u32.to_le_bytes());
    bytes[0x6c..0x70].copy_from_slice(&24u32.to_le_bytes());
    bytes.extend_from_slice(b"TestSheet\0TestType\0Name\0IronSword\0");
    bytes
}

fn adb_bytes(group: &str, tags: &str, animation: &str) -> Vec<u8> {
    format!(
        r#"<AnimDB FragDef="actions.xml" TagDef="tags.xml"><FragmentList><{group}><Fragment BlendOutDuration="0.2" Tags="{tags}"><AnimLayer><Blend ExitTime="0" StartTime="0" Duration="0.2" CurveType="0"/><Animation name="{animation}"/></AnimLayer></Fragment></{group}></FragmentList></AnimDB>"#
    )
    .into_bytes()
}

#[test]
fn interned_values_are_found_by_lowercase_crc_with_original_casing() {
    let mut index = ContentIndex::open_in_memory().unwrap();
    index
        .ingest(
            "test.pak",
            "data/Items.datasheet",
            EntryFormat::Datasheet,
            &[FactDraft {
                field: "ItemID".into(),
                value: "IronSword".into(),
                kind: FactKind::Cell,
                loc_a: 0,
                loc_b: 0,
            }],
        )
        .unwrap();

    let hits = index.lookup(Lookup::value("ironsword")).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].value, "IronSword");
    assert_eq!(hits[0].field, "ItemID");
    assert_eq!(hits[0].entry, "data/Items.datasheet");
    assert_eq!(hits[0].location, "row 0 · column ItemID");

    let by_field = index.lookup(Lookup::field("ItemID")).unwrap();
    assert_eq!(by_field.len(), 1);
    assert_eq!(by_field[0].field, "ItemID");

    let by_name = index.lookup(Lookup::name("data/items.datasheet")).unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].entry, "data/Items.datasheet");

    let by_stem = index.lookup(Lookup::name("items")).unwrap();
    assert!(
        by_stem.iter().any(|hit| hit.value == "Items"),
        "{by_stem:?}"
    );
    let by_base = index.lookup(Lookup::name("items.datasheet")).unwrap();
    assert!(
        by_base.iter().any(|hit| hit.value == "Items.datasheet"),
        "{by_base:?}"
    );
}

#[test]
fn file_paths_are_crc_indexed_even_when_longer_than_content_tokens() {
    let path = format!("lyshineui/long/{}/sheet.datasheet", "a".repeat(600));
    let mut index = ContentIndex::open_in_memory().unwrap();
    index
        .ingest("test.pak", &path, EntryFormat::Datasheet, &[])
        .unwrap();

    let hits = index.lookup(Lookup::name(&path)).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entry, path);
    assert_eq!(hits[0].location, "name");
}

#[test]
fn build_indexes_datasheet_and_mannequin_from_a_pak() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new("data/items.datasheet", minimal_datasheet(), Method::Store),
            Entry::new(
                "animations/player_rapier.adb",
                adb_bytes(
                    "Ability_Rapier_Riposte_Kickout",
                    "1H_Melee+Rapier",
                    "rapier_ability_riposte_cancel",
                ),
                Method::Store,
            ),
        ],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    let status = index.build(&root).unwrap();
    assert!(status.is_complete());

    let cells = index.lookup(Lookup::value("ironsword")).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].location, "row 0 · column Name");

    let fragments = index.search(&["kickout".to_string()], false).unwrap();
    let locations: Vec<&str> = fragments.iter().map(|hit| hit.location.as_str()).collect();
    assert!(
        locations.contains(&"fragment Ability_Rapier_Riposte_Kickout"),
        "{locations:?}"
    );
    assert_eq!(
        index
            .lookup(Lookup::value("Ability_Rapier_Riposte_Kickout"))
            .unwrap()
            .len(),
        1
    );

    let names = index
        .lookup(Lookup::name("animations/player_rapier.adb"))
        .unwrap();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].location, "name");
}

#[test]
fn complete_paks_are_not_reindexed() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![Entry::new(
            "data/items.datasheet",
            minimal_datasheet(),
            Method::Store,
        )],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(&root).unwrap().is_complete());
    assert!(index.is_complete(&root));
    assert!(index.build(&root).unwrap().is_complete());
    assert_eq!(index.lookup(Lookup::value("ironsword")).unwrap().len(), 1);
}

#[test]
fn start_alongside_returns_before_the_build_joins() {
    let cache_dir = tempfile::tempdir().unwrap();
    let cache_path = cache_dir.path().join("index.sqlite");
    let pak_dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        pak_dir.path(),
        "test.pak",
        vec![Entry::new(
            "data/items.datasheet",
            minimal_datasheet(),
            Method::Store,
        )],
    );
    let started = Instant::now();
    let handle = start_alongside_at(cache_path.clone(), root);
    assert!(handle.is_detached());
    assert!(started.elapsed() < Duration::from_millis(200));
    handle.join();
    let index = ContentIndex::open(&cache_path).unwrap();
    assert_eq!(index.lookup(Lookup::value("ironsword")).unwrap().len(), 1);
}

#[test]
fn search_finds_a_needle_among_many_unrelated_tokens() {
    let mut index = ContentIndex::open_in_memory().unwrap();
    for n in 0..2_000 {
        index
            .ingest(
                "noise.pak",
                &format!("data/row_{n}.datasheet"),
                EntryFormat::Datasheet,
                &[FactDraft {
                    field: "ItemID".into(),
                    value: format!("NoiseToken{n:04}"),
                    kind: FactKind::Cell,
                    loc_a: 0,
                    loc_b: 0,
                }],
            )
            .unwrap();
    }
    index
        .ingest(
            "test.pak",
            "animations/player_rapier.adb",
            EntryFormat::Mannequin,
            &[FactDraft {
                field: String::new(),
                value: "Ability_Rapier_Riposte_Kickout".into(),
                kind: FactKind::Fragment,
                loc_a: 0,
                loc_b: 0,
            }],
        )
        .unwrap();

    let started = Instant::now();
    let hits = index.search(&["kickout".to_string()], false).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "search took {:?}",
        started.elapsed()
    );
    assert!(
        hits.iter()
            .any(|hit| hit.value == "Ability_Rapier_Riposte_Kickout"),
        "{hits:?}"
    );
    assert!(
        index
            .lookup(Lookup::value("no-such-token-xyz"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn build_indexes_every_searchable_text_format() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new(
                "slices/player.slice",
                objectstream_json().into_bytes(),
                Method::Store,
            ),
            Entry::new(
                "sounds/wwise/test_events.csv",
                b"Name,Id\nPlay_TestHorn,1\n".to_vec(),
                Method::Store,
            ),
            Entry::new(
                "scripts/method_string.luac",
                include_bytes!("../../nw-lua/tests/fixtures/linear/method_string.luac").to_vec(),
                Method::Store,
            ),
            Entry::new(
                "animations/player.animevents",
                br#"<anim_event_list><animation name="animations/player/run.caf"><event name="Footstep" time="0.5"/></animation></anim_event_list>"#.to_vec(),
                Method::Store,
            ),
        ],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(&root).unwrap().is_complete());

    let slice = index.lookup(Lookup::value("player_character")).unwrap();
    assert_eq!(slice.len(), 1);
    assert_eq!(slice[0].entry, "slices/player.slice");
    assert_eq!(slice[0].location, "field Name");

    let audio = index.lookup(Lookup::value("play_testhorn")).unwrap();
    assert_eq!(audio.len(), 1);
    assert_eq!(audio[0].location, "row 0 · column Name");

    let lua = index.search(&["upper".to_string()], false).unwrap();
    assert!(
        lua.iter()
            .any(|hit| hit.entry == "scripts/method_string.luac"),
        "{lua:?}"
    );

    let events = index.lookup(Lookup::value("footstep")).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].location, "event Footstep");
}

#[test]
fn build_indexes_embedded_serialize_modules_and_behavior_context() {
    let mut index = ContentIndex::open_in_memory().unwrap();
    index.ingest_embedded().unwrap();

    let serialize = index.lookup(Lookup::value("FEPlatformExecution")).unwrap();
    assert!(
        serialize.iter().any(|hit| {
            hit.pak == "nw-resources"
                && hit.entry == "serialize.json"
                && hit.value == "FEPlatformExecution"
        }),
        "{serialize:?}"
    );

    let module = index.lookup(Lookup::value("CameraComponent")).unwrap();
    assert!(
        module.iter().any(|hit| {
            hit.pak == "nw-resources"
                && hit.entry == "modules/camera-module.json"
                && hit.value == "CameraComponent"
        }),
        "{module:?}"
    );

    let behavior = index.lookup(Lookup::name("behavior-context.json")).unwrap();
    assert_eq!(behavior.len(), 1);
    assert_eq!(behavior[0].pak, "nw-resources");
    let named = index.lookup(Lookup::field("m_name")).unwrap();
    assert!(
        named
            .iter()
            .any(|hit| hit.pak == "nw-resources" && hit.entry == "behavior-context.json"),
        "{named:?}"
    );
}

#[test]
fn build_indexes_txt_and_cfg_from_a_pak() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new(
                "libs/config/game.cfg",
                b"-- test config\nsys_gameName = Ironwood\n".to_vec(),
                Method::Store,
            ),
            Entry::new(
                "libs/notes/readme.txt",
                b"welcome to Ironwood\n".to_vec(),
                Method::Store,
            ),
            Entry::new(
                "libs/config/region.xml",
                b"<Region name=\"Brightwood\"/>\n".to_vec(),
                Method::Store,
            ),
        ],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(&root).unwrap().is_complete());

    let cfg = index
        .lookup(Lookup::value("sys_gameName = Ironwood"))
        .unwrap();
    assert_eq!(cfg.len(), 1);
    assert_eq!(cfg[0].entry, "libs/config/game.cfg");
    assert_eq!(cfg[0].location, "line 2");

    let txt = index.lookup(Lookup::value("welcome to Ironwood")).unwrap();
    assert_eq!(txt.len(), 1);
    assert_eq!(txt[0].entry, "libs/notes/readme.txt");
    assert_eq!(txt[0].location, "line 1");

    let xml = index.lookup(Lookup::value("Brightwood")).unwrap();
    assert_eq!(xml.len(), 1);
    assert_eq!(xml[0].entry, "libs/config/region.xml");
    assert_eq!(xml[0].field, "Region/@name");
    assert_eq!(xml[0].location, "Region/@name");
}

#[test]
fn serialize_override_is_a_session_view_and_does_not_replace_the_index() {
    use nw_tools::grep::find;
    use nw_tools::resources::{ResourceView, SessionOverlay};

    let dir = tempfile::tempdir().unwrap();
    let serialize = dir.path().join("serialize.json");
    std::fs::write(&serialize, br#"{"name":"SessionOnlyType"}"#).unwrap();
    let view = ResourceView::from_overrides(Some(&serialize), None, None).unwrap();
    assert!(view.has_overrides());
    assert!(view.overrides_path("serialize.json"));
    assert!(!view.overrides_path("modules/camera-module.json"));
    assert!(
        std::str::from_utf8(view.serialize())
            .unwrap()
            .contains("SessionOnlyType")
    );

    let mut index = ContentIndex::open_in_memory().unwrap();
    index.ingest_embedded().unwrap();
    assert!(
        index
            .lookup(Lookup::value("SessionOnlyType"))
            .unwrap()
            .is_empty()
    );
    assert!(
        !index
            .lookup(Lookup::value("FEPlatformExecution"))
            .unwrap()
            .is_empty()
    );

    let overlay = SessionOverlay::extract(&view);
    let source = find::Source::Indexed(&index);
    let hits = find::lookup(source, &overlay, Lookup::value("SessionOnlyType")).unwrap();
    assert!(
        hits.iter().any(|hit| {
            hit.pak == "nw-resources"
                && hit.entry == "serialize.json"
                && hit.value == "SessionOnlyType"
        }),
        "{hits:?}"
    );
    assert!(
        find::lookup(source, &overlay, Lookup::value("FEPlatformExecution"))
            .unwrap()
            .is_empty(),
        "overridden serialize hides persisted serialize.json hits"
    );
    assert!(
        !find::lookup(source, &overlay, Lookup::value("CameraComponent"))
            .unwrap()
            .is_empty()
    );
}

fn objectstream_json() -> String {
    r#"{
  "name": "ObjectStream",
  "version": 3,
  "Objects": [
    {
      "typeId": "{75651658-8663-478D-9090-2432DFCAFA44}",
      "typeName": "AZ::Entity",
      "version": 2,
      "Objects": [
        {
          "field": "Name",
          "typeId": "{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}",
          "typeName": "AZStd::string",
          "value": "player_character"
        }
      ]
    }
  ]
}"#
    .to_owned()
}
