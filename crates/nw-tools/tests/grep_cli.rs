use std::fs::File;
use std::process::Command;

use nw_datasheet::game_system::Crc32;
use nw_pak::crypak::{Entry, Method, Writer};
use nw_tools::index::ContentIndex;

#[test]
fn crc_cli_matches_live_and_index_and_accepts_decimal_and_hex() {
    let root = tempfile::tempdir().unwrap();
    Writer::new(File::create(root.path().join("test.pak")).unwrap())
        .write(vec![
            Entry::new(
                "scripts/Test.txt",
                b"CaseSensitiveToken\nCaseSensitiveToken\n".to_vec(),
                Method::Store,
            ),
            Entry::new(
                "metadata.xml",
                br#"<Node FieldName="FieldOnlyValue"/>"#.to_vec(),
                Method::Store,
            ),
        ])
        .unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    let cache = cache_dir.path().join("index.sqlite");
    {
        let mut index = ContentIndex::open(&cache).unwrap();
        index.build(root.path()).unwrap();
    }
    for (target, case, crc, expected) in [
        (
            "value",
            Some("original"),
            Crc32::from_str("CaseSensitiveToken"),
            2,
        ),
        (
            "value",
            Some("lower"),
            Crc32::from_str_lower("CaseSensitiveToken"),
            2,
        ),
        ("name", Some("lower"), Crc32::from_str_lower("test"), 1),
        ("field", None, Crc32::from_str("Node/@FieldName"), 1),
    ] {
        let mut previous = None;
        for (live, literal) in [
            (true, crc.value().to_string()),
            (false, format!("0x{:08X}", crc.value())),
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_nw-tools"));
            command
                .args(["--format", "json", "grep", "--root"])
                .arg(root.path())
                .args(["--index"])
                .arg(&cache)
                .args(["--crc", &literal, "--target", target]);
            if let Some(case) = case {
                command.args(["--case", case]);
            }
            if live {
                command.arg("--live");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["hits"].as_array().unwrap().len(), expected);
            assert_eq!(document["source"], if live { "live" } else { "index" });
            if let Some(previous) = previous {
                assert_eq!(document["hits"], previous);
            }
            previous = Some(document["hits"].clone());
        }
    }
}

#[test]
fn crc_cli_rejects_invalid_or_overflowing_values() {
    for crc in ["-1", "4294967296", "0x100000000", "0x", "garbage"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nw-tools"))
            .args(["grep", "--crc", crc])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("CRC"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn querying_does_not_create_an_index() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing.sqlite");
    let output = Command::new(env!("CARGO_BIN_EXE_nw-tools"))
        .args(["grep", "--root"])
        .arg(root.path())
        .arg("--index")
        .arg(&missing)
        .args(["--crc", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!missing.exists());
}

#[test]
fn text_cli_combines_queries_and_can_limit_hits_to_content() {
    let root = tempfile::tempdir().unwrap();
    Writer::new(File::create(root.path().join("test.pak")).unwrap())
        .write(vec![Entry::new(
            "ToolingNeedle.txt",
            b"ToolingNeedleValue\nSecondUniqueValue\n".to_vec(),
            Method::Store,
        )])
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nw-tools"))
        .args(["-v", "--format", "json", "grep", "--root"])
        .arg(root.path())
        .args([
            "--live",
            "--exact",
            "--content-only",
            "ToolingNeedleValue",
            "SecondUniqueValue",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let hits = document["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|hit| hit["location"] != "name"));
}
