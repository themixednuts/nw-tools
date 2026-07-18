//! Golden writer fixture: the exact native-source + sidecar bytes that lock the
//! `azoth.source-meta/v1` contract.
//!
//! `emit_fixture` (ignored) writes the fixture into a temp directory so the
//! exact sidecar bytes can be copied into this crate's `tests/golden/` and into
//! the engine reader's fixtures. `golden_sidecars_are_stable` re-writes and
//! asserts byte-for-byte equality with the committed goldens, locking the
//! writer to the contract the engine's asset processor reads.

use std::path::Path;

use nw_tools::native_port::{
    AssetId, DecodedClass, MappedSource, Mapper, SourceInput,
    writer::{sidecar_path, write_native_sources},
};
use uuid::uuid;

/// Deterministic native payload for a fixture source.
fn payload(native_path: &str) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    bytes.extend_from_slice(native_path.as_bytes());
    bytes
}

fn fixture_inputs() -> Vec<SourceInput> {
    vec![
        // Uncataloged: sidecar omits preserved_asset_id (engine mints it).
        SourceInput::new(
            "Textures/Items/Weapons/Sword_Diff.png",
            DecodedClass::Image,
            None,
        ),
        // Cataloged: sidecar preserves the New World catalog id.
        SourceInput::new(
            "textures/terrain/rock_diff.png",
            DecodedClass::Image,
            Some(AssetId::new(
                uuid!("11112222-3333-4444-5555-666677778888"),
                0,
            )),
        ),
    ]
}

fn write_fixture(out: &Path) {
    let inv = Mapper::pinned().unwrap().map_all(fixture_inputs());
    assert!(!inv.has_blocking_errors(), "fixture must map cleanly");
    let reader = |source: &MappedSource| Ok(payload(&source.native_path));
    write_native_sources(&inv.sources, &reader, out).unwrap();
}

#[test]
#[ignore = "regenerates the committed golden sidecars; run with --ignored"]
fn emit_fixture() {
    let out = std::env::temp_dir().join("nw-tools-native-port-fixture");
    if out.exists() {
        std::fs::remove_dir_all(&out).unwrap();
    }
    write_fixture(&out);
    eprintln!("fixture written to: {}", out.display());
}

#[test]
fn golden_sidecars_are_stable() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("port-output");
    write_fixture(&out);

    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for rel in [
        "assets/rendering/textures/items/weapons/sword_diff.png.azmeta.json",
        "assets/rendering/textures/terrain/rock_diff.png.azmeta.json",
    ] {
        let produced = std::fs::read(out.join(rel)).unwrap();
        let expected = std::fs::read(golden.join(rel))
            .unwrap_or_else(|_| panic!("missing golden `{rel}`; run `emit_fixture` and copy"));
        assert_eq!(
            produced, expected,
            "golden mismatch for `{rel}` (regenerate with the ignored emit_fixture test)"
        );
    }

    // The native source payloads are written next to their sidecars.
    let sword = out.join("assets/rendering/textures/items/weapons/sword_diff.png");
    assert!(sword.exists());
    assert!(sidecar_path(&sword).exists());
    // No import manifest is emitted anymore.
    assert!(!out.join("azoth-import-manifest.json").exists());
}
