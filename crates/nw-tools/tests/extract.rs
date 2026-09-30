//! Extract seam: exclusive claim, leftover XML complete extract (ADR-0001).

use nw_tools::extract::{self, SkipReason};
use nw_tools::grep::adapter::FactKind;
use nw_tools::index::EntryFormat;

fn adb_bytes() -> Vec<u8> {
    br#"<AnimDB FragDef="actions.xml" TagDef="tags.xml"><FragmentList><Locomotion_Idle><Fragment BlendOutDuration="0.2" Tags="Idle"><AnimLayer><Blend ExitTime="0" StartTime="0" Duration="0.2" CurveType="0"/><Animation name="idle_pose"/></AnimLayer></Fragment></Locomotion_Idle></FragmentList></AnimDB>"#
        .to_vec()
}

#[test]
fn leftover_xml_emits_element_attribute_and_text() {
    let extracted = extract::bytes(
        "libs/config/region.xml",
        br#"<Region name="Brightwood">west</Region>"#,
    );
    assert_eq!(extracted.format, EntryFormat::Xml);
    assert_eq!(extracted.skip, None);
    let values: Vec<&str> = extracted
        .facts
        .iter()
        .map(|fact| fact.value.as_str())
        .collect();
    assert!(values.contains(&"Region"), "{values:?}");
    assert!(values.contains(&"name"), "{values:?}");
    assert!(values.contains(&"Brightwood"), "{values:?}");
    assert!(values.contains(&"west"), "{values:?}");
    let bright = extracted
        .facts
        .iter()
        .find(|fact| fact.value == "Brightwood")
        .expect("attribute value");
    assert_eq!(bright.field, "Region/@name");
    assert_eq!(bright.kind, FactKind::Xml);
}

#[test]
fn leftover_xml_does_not_claim_mannequin() {
    let extracted = extract::bytes("animations/player.adb", &adb_bytes());
    assert_eq!(extracted.format, EntryFormat::Mannequin);
    assert_eq!(extracted.skip, None);
    assert!(
        extracted
            .facts
            .iter()
            .any(|fact| fact.value == "Locomotion_Idle")
    );
}

#[test]
fn failed_mannequin_does_not_fall_through_to_leftover_xml() {
    let extracted = extract::bytes("animations/player.adb", b"not xml at all");
    assert_eq!(extracted.format, EntryFormat::Mannequin);
    assert_eq!(extracted.skip, Some(SkipReason::Undecodable));
    assert!(
        extracted
            .facts
            .iter()
            .all(|fact| fact.kind == FactKind::FileName || fact.kind != FactKind::Xml)
    );
}

#[test]
fn broken_xml_still_recovers_strings() {
    let extracted = extract::bytes("libs/config/region.xml", br#"<Region name="Brightwood""#);
    assert_eq!(extracted.format, EntryFormat::Xml);
    assert_eq!(extracted.skip, None);
    assert!(
        extracted
            .facts
            .iter()
            .any(|fact| fact.value == "Brightwood"),
        "{:?}",
        extracted.facts
    );
}

#[test]
fn name_only_entries_do_not_pull_bytes() {
    let mut pulled = false;
    let extracted = extract::entry("textures/rock.dds", || {
        pulled = true;
        Some(vec![0u8; 8])
    });
    assert!(!pulled);
    assert_eq!(extracted.format, EntryFormat::Unknown);
    assert_eq!(extracted.skip, None);
    let names: Vec<&str> = extracted
        .facts
        .iter()
        .filter(|fact| fact.kind == FactKind::FileName)
        .map(|fact| fact.value.as_str())
        .collect();
    assert_eq!(names, ["textures/rock.dds", "rock.dds", "rock"]);
}

#[test]
fn path_facts_include_basename_and_stem() {
    let facts = extract::path_name_facts("data/Items.datasheet");
    let values: Vec<&str> = facts.iter().map(|fact| fact.value.as_str()).collect();
    assert_eq!(values, ["data/Items.datasheet", "Items.datasheet", "Items"]);
}
