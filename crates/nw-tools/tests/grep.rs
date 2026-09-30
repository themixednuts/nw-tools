//! Global `grep` tracers through the public lib seam (ticket 004).
//!
//! One vertical slice at a time: each test drives `grep::run` over a tiny
//! synthetic pak and asserts membership and order — never numeric scores.

use std::fs::File;

use nw_pak::crypak::{Entry, Method, Writer};
use nw_tools::grep::{GrepRequest, run};

fn write_pak(dir: &std::path::Path, filename: &str, entries: Vec<Entry>) -> std::path::PathBuf {
    let pak = dir.join(filename);
    let file = File::create(&pak).unwrap();
    Writer::new(file).write(entries).unwrap();
    dir.to_path_buf()
}

fn adb_bytes(group: &str, tags: &str, animation: &str) -> Vec<u8> {
    format!(
        r#"<AnimDB FragDef="actions.xml" TagDef="tags.xml"><FragmentList><{group}><Fragment BlendOutDuration="0.2" Tags="{tags}"><AnimLayer><Blend ExitTime="0" StartTime="0" Duration="0.2" CurveType="0"/><Animation name="{animation}"/></AnimLayer></Fragment></{group}></FragmentList></AnimDB>"#
    )
    .into_bytes()
}

/// Minimal valid `.datasheet`: version 0x11, one string column `Name`, one
/// row holding `IronSword`. Layout mirrors `nw_datasheet::Layout`:
/// header (..0x5c), one 12-byte column record, one 8-byte cell record,
/// null-terminated string table at 0x70.
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

#[test]
fn finds_entries_by_fuzzy_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new(
                "animations/player_rapier.adb",
                adb_bytes("Locomotion_Idle", "Idle", "idle_pose"),
                Method::Store,
            ),
            Entry::new("textures/rock.dds", vec![0u8; 64], Method::Store),
        ],
    );
    let report = run(&GrepRequest {
        root,
        queries: vec!["rapier".to_string()],
        exact: false,
    })
    .unwrap();
    let names: Vec<&str> = report.hits.iter().map(|hit| hit.entry.as_str()).collect();
    assert_eq!(names, ["animations/player_rapier.adb"]);
}

#[test]
fn finds_datasheet_cell_content() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new("data/items.datasheet", minimal_datasheet(), Method::Store),
            Entry::new(
                "data/broken.datasheet",
                b"not a datasheet".to_vec(),
                Method::Store,
            ),
        ],
    );
    let report = run(&GrepRequest {
        root,
        queries: vec!["ironsword".to_string()],
        exact: false,
    })
    .unwrap();
    assert_eq!(report.hits.len(), 1);
    let hit = &report.hits[0];
    assert_eq!(hit.entry, "data/items.datasheet");
    assert_eq!(hit.location, "row 0 · column Name");
}

#[test]
fn undecodable_entries_are_skipped_not_aborted() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![
            Entry::new("data/items.datasheet", minimal_datasheet(), Method::Store),
            Entry::new(
                "data/broken.datasheet",
                b"not a datasheet".to_vec(),
                Method::Store,
            ),
        ],
    );
    let report = run(&GrepRequest {
        root,
        queries: vec!["ironsword".to_string()],
        exact: false,
    })
    .unwrap();
    assert_eq!(report.hits.len(), 1);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].entry, "data/broken.datasheet");
    assert_eq!(report.skipped[0].reason, "undecodable");
}

#[test]
fn finds_mannequin_fragment_content() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_pak(
        dir.path(),
        "test.pak",
        vec![Entry::new(
            "animations/player_rapier.adb",
            adb_bytes(
                "Ability_Rapier_Riposte_Kickout",
                "1H_Melee+Rapier",
                "rapier_ability_riposte_cancel",
            ),
            Method::Store,
        )],
    );
    let report = run(&GrepRequest {
        root,
        queries: vec!["kickout".to_string()],
        exact: false,
    })
    .unwrap();
    assert_eq!(report.hits.len(), 1);
    let hit = &report.hits[0];
    assert_eq!(hit.entry, "animations/player_rapier.adb");
    assert_eq!(hit.location, "fragment Ability_Rapier_Riposte_Kickout");
}

#[test]
fn crc_lookup_works_live_without_an_index() {
    use nw_tools::grep::find;
    use nw_tools::index::Lookup;
    use nw_tools::resources::SessionOverlay;

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
    let hits = find::lookup(
        find::Source::Live(&root),
        &SessionOverlay::default(),
        Lookup::value("IronSword"),
    )
    .unwrap();
    assert!(
        hits.iter().any(|hit| hit.value == "IronSword"),
        "{hits:?}"
    );
    assert!(
        find::lookup(
            find::Source::Auto {
                index: None,
                root: &root,
            },
            &SessionOverlay::default(),
            Lookup::value("IronSword"),
        )
        .unwrap()
        .iter()
        .any(|hit| hit.value == "IronSword")
    );
}
