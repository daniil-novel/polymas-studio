"""Portable SBOM evidence must keep its dependency graph and omit machine paths."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "polymas_sbom", Path(__file__).resolve().parents[2] / "scripts" / "sbom.py"
)
sbom = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sbom)


def fixture():
    root = "path+file:///C:/Users/fixture/build/desktop#desktop@0.1.0"
    local = "path+file:///C:/Users/fixture/build/core#0.1.0"
    return {
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "bom-ref": root,
            "name": "polymas-desktop", "version": "0.1.0",
            "purl": "pkg:cargo/polymas-desktop@0.1.0?download_url=file://.#src/main.rs"}},
        "components": [
            {"type": "library", "bom-ref": local, "name": "polymas-core", "version": "0.1.0",
             "purl": "pkg:cargo/polymas-core@0.1.0?download_url=file://..\\core",
             "licenses": [{"expression": "LicenseRef-Proprietary"}]},
            {"type": "library", "bom-ref": "registry+test#serde@1.0", "name": "serde", "version": "1.0",
             "purl": "pkg:cargo/serde@1.0", "licenses": [{"expression": "MIT OR Apache-2.0"}],
             "hashes": [{"alg": "SHA-256", "content": "a" * 64}]},
        ],
        "dependencies": [{"ref": root, "dependsOn": [local]},
            {"ref": local, "dependsOn": ["registry+test#serde@1.0"]},
            {"ref": "registry+test#serde@1.0", "dependsOn": []}],
    }


class PortableSbomTests(unittest.TestCase):
    def test_cli_cannot_overwrite_its_input_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "evidence.json"
            original = json.dumps(fixture()).encode()
            for arguments in [
                ["normalize", str(path), str(path), "--name", "polymas-desktop", "--version", "0.1.0"],
                ["licenses", str(path), str(path)],
            ]:
                path.write_bytes(original)
                result = subprocess.run(
                    [sys.executable, SPEC.origin, *arguments], capture_output=True, check=False
                )
                with self.subTest(command=arguments[0]):
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(path.read_bytes(), original)

    def test_npm_directory_label_is_normalized_only_when_package_url_matches(self):
        data = fixture()
        root = data["metadata"]["component"]
        old = root["bom-ref"]
        root.update(name="build-directory", purl="pkg:npm/polymas-studio@0.1.0", **{"bom-ref": "polymas-studio@0.1.0"})
        data["dependencies"][0]["ref"] = root["bom-ref"]
        self.assertNotEqual(old, root["bom-ref"])
        result = sbom.portable(data, "polymas-studio", "0.1.0")
        self.assertEqual(result["metadata"]["component"]["name"], "polymas-studio")
        root["purl"] = "pkg:npm/unrelated-package@0.1.0"
        with self.assertRaises(ValueError):
            sbom.portable(data, "polymas-studio", "0.1.0")

    def test_local_references_are_rewritten_consistently_without_losing_evidence(self):
        original = fixture()
        before = copy.deepcopy(original)
        result = sbom.portable(original, "polymas-desktop", "0.1.0")
        self.assertEqual(result["version"], original["version"] + 1)
        self.assertEqual(sbom.portable(result, "polymas-desktop", "0.1.0"), result)
        self.assertEqual(original, before, "Input evidence must remain untouched")
        serialized = json.dumps(result)
        self.assertNotIn("file://", serialized)
        self.assertNotIn("Users", serialized)
        self.assertEqual(result["metadata"]["component"]["purl"], "pkg:cargo/polymas-desktop@0.1.0")
        root_ref = result["metadata"]["component"]["bom-ref"]
        core_ref = result["components"][0]["bom-ref"]
        self.assertEqual(result["dependencies"][0], {"ref": root_ref, "dependsOn": [core_ref]})
        self.assertEqual(result["dependencies"][1]["dependsOn"], ["registry+test#serde@1.0"])
        self.assertEqual(result["components"][1], before["components"][1])

    def test_mismatched_version_empty_or_broken_inventory_cannot_be_packaged(self):
        for mutate in [
            lambda d: d.update(serialNumber="urn:uuid:***"),
            lambda d: d.update(signature={"algorithm": "RS256", "value": "signed-input"}),
            lambda d: d.update(specVersion="2.0"),
            lambda d: d.update(components=[]),
            lambda d: d["metadata"]["component"].update(version="9.0.0"),
            lambda d: d["dependencies"][0]["dependsOn"].append("missing-component"),
            lambda d: d["components"].append(copy.deepcopy(d["components"][0])),
        ]:
            data = fixture()
            mutate(data)
            with self.subTest(data=data), self.assertRaises(ValueError):
                sbom.portable(data, "polymas-desktop", "0.1.0")

    def test_unexpected_machine_paths_are_rejected_instead_of_silently_shipped(self):
        for path in ["C:\\Users\\fixture\\private", "file:///private/file", "/Users/fixture/private", "/home/fixture/private"]:
            data = fixture()
            data["components"][1]["properties"] = [{"name": "source", "value": path}]
            with self.subTest(path=path), self.assertRaises(ValueError):
                sbom.portable(data, "polymas-desktop", "0.1.0")

    def test_license_inventory_reports_missing_evidence_and_keeps_distinct_versions(self):
        data = sbom.portable(fixture(), "polymas-desktop", "0.1.0")
        other = copy.deepcopy(data)
        other["components"][1].update(version="2.0", purl="pkg:cargo/serde@2.0", licenses=[])
        rows = sbom.license_inventory([data, other])
        serde = [row for row in rows if row["name"] == "serde"]
        self.assertEqual(len(serde), 2)
        self.assertEqual(serde[0]["licenses"], ["MIT OR Apache-2.0"])
        self.assertEqual(serde[1]["licenses"], [])
        self.assertTrue(serde[1]["reviewRequired"])


if __name__ == "__main__":
    unittest.main()
