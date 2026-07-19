from __future__ import annotations

import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest


AZOTH = Path(__file__).parents[1] / "azoth"
sys.path.insert(0, str(AZOTH))
import metadata  # noqa: E402


class MetadataTests(unittest.TestCase):
    def test_loads_gltf_and_glb(self) -> None:
        document = {"asset": {"version": "2.0", "generator": "nw-tools"}, "extras": {}}
        encoded = json.dumps(document).encode()
        padded = encoded + b" " * ((4 - len(encoded) % 4) % 4)
        glb = struct.pack("<4sII", b"glTF", 2, 20 + len(padded))
        glb += struct.pack("<II", len(padded), 0x4E4F534A) + padded
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            gltf_path = root / "sample.gltf"
            glb_path = root / "sample.glb"
            gltf_path.write_text(json.dumps(document), encoding="utf-8")
            glb_path.write_bytes(glb)
            self.assertEqual(metadata.load_document(gltf_path), document)
            self.assertEqual(metadata.load_document(glb_path), document)

    def test_indexes_every_extended_category_and_variants(self) -> None:
        document = {
            "asset": {"version": "2.0", "generator": "nw-tools"},
            "extras": {
                "sourceAssets": [
                    {"kind": "nvClothFabric", "path": "cape.cloth"},
                    {"kind": "wwiseSoundBank", "path": "sound.bnk"},
                    {"kind": "mannequinAnimationDatabase", "path": "motion.adb"},
                    {"kind": "particleLibrary", "path": "particles.xml"},
                    {"kind": "terrainHeightmap", "path": "terrain.heightmap"},
                    {"kind": "vegetationDistribution", "path": "plants.distribution"},
                ],
                "physics": {"hitVolumes": [{"context": {"alternateSourcePaths": ["alt.slice"]}}]},
            },
        }
        categories = {item.category for item in metadata.resource_records(document)}
        self.assertTrue(
            {"Cloth", "Audio", "Mannequin", "Particles", "Terrain", "Vegetation", "Variants"}
            <= categories
        )

    def test_reports_missing_external_resources(self) -> None:
        document = {
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": 4, "uri": "shared/missing.bin"}],
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "asset.gltf"
            issues = metadata.diagnostics(document, root, manifest)
            self.assertEqual(issues, ["missing external resource: shared/missing.bin"])


if __name__ == "__main__":
    unittest.main()
