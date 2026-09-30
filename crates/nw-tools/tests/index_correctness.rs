use std::fs::File;

use nw_pak::crypak::{Entry, Method, Writer};
use nw_tools::grep::adapter::{FactDraft, FactKind};
use nw_tools::grep::find::{self, Source};
use nw_tools::index::{ContentIndex, CrcCase, EntryFormat, Lookup, LookupTarget};
use nw_tools::resources::SessionOverlay;

fn write_pak(root: &std::path::Path, name: &str, entries: Vec<Entry>) {
    Writer::new(File::create(root.join(name)).unwrap())
        .write(entries)
        .unwrap();
}

#[test]
fn long_content_and_field_crcs_match_live_extraction() {
    let root = tempfile::tempdir().unwrap();
    let field = "LongField".repeat(80);
    let value = "LongValue".repeat(80);
    write_pak(
        root.path(),
        "test.pak",
        vec![Entry::new(
            "long.xml",
            format!("<Root {field}=\"{value}\"/>").into_bytes(),
            Method::Store,
        )],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(root.path()).unwrap().is_complete());
    let overlay = SessionOverlay::default();
    for lookup in [
        Lookup::value(&value),
        Lookup::field(&format!("Root/@{field}")),
    ] {
        let live = find::lookup(Source::Live(root.path()), &overlay, lookup).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(
            find::lookup(Source::Indexed(&index), &overlay, lookup).unwrap(),
            live
        );
    }
}

#[test]
fn fuzzy_search_returns_all_matches_for_every_query() {
    let mut index = ContentIndex::open_in_memory().unwrap();
    let mut facts: Vec<_> = (0..300)
        .map(|n| FactDraft {
            field: "Name".into(),
            value: format!("RepeatedNeedle{n}"),
            kind: FactKind::Cell,
            loc_a: n,
            loc_b: 0,
        })
        .collect();
    facts.push(FactDraft {
        field: "Name".into(),
        value: "SecondQueryOnly".into(),
        kind: FactKind::Cell,
        loc_a: 300,
        loc_b: 0,
    });
    index
        .ingest(
            "test.pak",
            "items.datasheet",
            EntryFormat::Datasheet,
            &facts,
        )
        .unwrap();
    let hits = index
        .search(&["RepeatedNeedle".into(), "SecondQueryOnly".into()], false)
        .unwrap();
    assert_eq!(hits.len(), 301);
    assert!(hits.iter().any(|hit| hit.value == "SecondQueryOnly"));
}

#[test]
fn unreadable_pak_is_never_marked_complete() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("broken.pak"), b"not a pak").unwrap();
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(
        !index
            .build(root.path())
            .is_ok_and(|status| status.is_complete())
    );
    assert!(!index.is_complete(root.path()));
}

#[test]
fn skipped_content_stays_partial_and_retry_does_not_duplicate_names() {
    let root = tempfile::tempdir().unwrap();
    write_pak(
        root.path(),
        "test.pak",
        vec![Entry::new("broken.datasheet", vec![0; 64], Method::Store)],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    for _ in 0..2 {
        assert!(!index.build(root.path()).unwrap().is_complete());
        assert!(!index.is_complete(root.path()));
        assert_eq!(
            index
                .lookup(Lookup::name("broken.datasheet"))
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn removing_a_pak_invalidates_coverage_and_removes_its_facts() {
    let root = tempfile::tempdir().unwrap();
    write_pak(
        root.path(),
        "removed.pak",
        vec![Entry::new(
            "removed.txt",
            b"RemovedToken".to_vec(),
            Method::Store,
        )],
    );
    write_pak(
        root.path(),
        "kept.pak",
        vec![Entry::new("kept.txt", b"KeptToken".to_vec(), Method::Store)],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(root.path()).unwrap().is_complete());
    std::fs::remove_file(root.path().join("removed.pak")).unwrap();
    assert!(!index.is_complete(root.path()));
    assert!(index.build(root.path()).unwrap().is_complete());
    assert!(
        index
            .lookup(Lookup::value("RemovedToken"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(index.lookup(Lookup::value("KeptToken")).unwrap().len(), 1);
}

#[test]
fn completeness_is_scoped_to_the_pak_root() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    write_pak(
        first.path(),
        "test.pak",
        vec![Entry::new(
            "value.txt",
            b"FirstValue".to_vec(),
            Method::Store,
        )],
    );
    std::fs::copy(
        first.path().join("test.pak"),
        second.path().join("test.pak"),
    )
    .unwrap();
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(first.path()).unwrap().is_complete());
    assert!(!index.is_complete(second.path()));
}

#[test]
fn live_and_pak_only_index_include_bundled_resources() {
    let root = tempfile::tempdir().unwrap();
    let mut index = ContentIndex::open_in_memory().unwrap();
    assert!(index.build(root.path()).unwrap().is_complete());
    let overlay = SessionOverlay::default();
    let lookup = Lookup::name("behavior-context.json");
    let live = find::lookup(Source::Live(root.path()), &overlay, lookup).unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].pak, "nw-resources");
    assert_eq!(
        find::lookup(Source::Indexed(&index), &overlay, lookup).unwrap(),
        live
    );
    assert_eq!(
        find::lookup(
            Source::Auto {
                index: Some(&index),
                root: root.path()
            },
            &overlay,
            lookup
        )
        .unwrap(),
        live
    );
}

#[test]
fn raw_crc_preserves_collision_candidates_and_repeated_occurrences() {
    use nw_datasheet::game_system::Crc32;
    let candidates = [
        "Candidate6283367772731384346",
        "Candidate10614864037769514338",
    ];
    let mut index = ContentIndex::open_in_memory().unwrap();
    let facts: Vec<_> = [candidates[0], candidates[0], candidates[1]]
        .into_iter()
        .enumerate()
        .map(|(n, value)| FactDraft {
            field: "Name".into(),
            value: value.into(),
            kind: FactKind::Cell,
            loc_a: n as u32,
            loc_b: 0,
        })
        .collect();
    index
        .ingest(
            "test.pak",
            "items.datasheet",
            EntryFormat::Datasheet,
            &facts,
        )
        .unwrap();
    let hits = index
        .lookup(Lookup {
            target: LookupTarget::Value,
            case: CrcCase::Original,
            crc: Crc32::new(0xf835a8f0),
        })
        .unwrap();
    assert_eq!(hits.len(), 3);
    for candidate in candidates {
        assert!(hits.iter().any(|hit| hit.value == candidate));
    }
}

#[test]
fn text_search_retains_repeated_xml_occurrences() {
    let root = tempfile::tempdir().unwrap();
    write_pak(
        root.path(),
        "test.pak",
        vec![Entry::new(
            "repeated.xml",
            b"<Root><Item name=\"RepeatedValue\"/><Item name=\"RepeatedValue\"/></Root>".to_vec(),
            Method::Store,
        )],
    );
    let mut index = ContentIndex::open_in_memory().unwrap();
    index.build(root.path()).unwrap();
    assert_eq!(
        index.search(&["RepeatedValue".into()], true).unwrap().len(),
        2
    );
}

#[test]
fn reflection_keeps_long_values_and_repeated_fields() {
    let value = "ReflectionValue".repeat(60);
    let json = serde_json::json!({"name": value, "description": value}).to_string();
    let extracted = nw_tools::extract::bytes("modules/test.json", json.as_bytes());
    let matching: Vec<_> = extracted
        .facts
        .iter()
        .filter(|fact| fact.value == value)
        .collect();
    assert_eq!(matching.len(), 2);
    assert!(matching.iter().any(|fact| fact.field == "name"));
    assert!(matching.iter().any(|fact| fact.field == "description"));
}

#[test]
fn resource_stamps_notice_content_changes_with_the_same_size_and_prefix() {
    use nw_tools::resources::ResourceView;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("serialize.json");
    std::fs::write(&path, br#"{"name":"FirstValue"}"#).unwrap();
    let first = ResourceView::from_overrides(Some(&path), None, None).unwrap();
    std::fs::write(&path, br#"{"name":"OtherValue"}"#).unwrap();
    let second = ResourceView::from_overrides(Some(&path), None, None).unwrap();
    assert_eq!(first.stamp().0, second.stamp().0);
    assert_ne!(first.stamp().1, second.stamp().1);
}

#[test]
fn lowercase_crc_changes_only_ascii_bytes() {
    use nw_datasheet::game_system::Crc32;
    let mut index = ContentIndex::open_in_memory().unwrap();
    index
        .ingest(
            "test.pak",
            "items.datasheet",
            EntryFormat::Datasheet,
            &[FactDraft {
                field: "Name".into(),
                value: "ÄABC".into(),
                kind: FactKind::Cell,
                loc_a: 0,
                loc_b: 0,
            }],
        )
        .unwrap();
    let lookup = Lookup {
        target: LookupTarget::Value,
        case: CrcCase::Lower,
        crc: Crc32::from_str("Äabc"),
    };
    assert_eq!(index.lookup(lookup).unwrap().len(), 1);
    assert!(index.lookup(Lookup::value("äabc")).unwrap().is_empty());
}
