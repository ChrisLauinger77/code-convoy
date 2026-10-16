"""Cheap source/provenance gates. Full offline builds are a separate check."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import types
import unittest
from unittest.mock import patch

import appimage_runtime
import licenses


KIT = licenses.EVIDENCE / "runtime"
builder = types.ModuleType("runtime_recipe_test")
builder.__file__ = str(KIT / "build-runtime.py")
exec(compile((KIT / "build-runtime.py").read_bytes(), builder.__file__, "exec"), builder.__dict__)


class RuntimeInputs(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.kit = self.root / "runtime"
        shutil.copytree(KIT, self.kit)
        self.spec = json.loads((self.kit / "runtime-inputs.json").read_text())

    def verify(self):
        (self.kit / "runtime-inputs.json").write_text(json.dumps(self.spec))
        return builder.verify(licenses.EVIDENCE / "sources", self.kit)

    def test_every_source_version_and_patch_is_identified(self):
        spec = self.verify()
        self.assertEqual(spec["sources"]["type2-runtime"]["revision"],
                         "dd6cebedcbddde9c82f89b011e8e1d40b6e43868")
        self.assertEqual({n: s["version"] for n, s in spec["sources"].items() if n != "type2-runtime"},
                         {"libfuse": "3.15.0", "squashfuse": "0.5.2", "musl": "1.2.5",
                          "zstd": "1.5.6", "zlib": "1.3.1", "mimalloc": "2.1.7"})
        self.assertTrue(all(p["commit"] and p["sha256"] for p in spec["packages"]))

    def test_missing_source_component_and_altered_versions_fail(self):
        original = copy.deepcopy(self.spec)
        for name in original["sources"]:
            with self.subTest(name=name):
                self.spec = copy.deepcopy(original)
                del self.spec["sources"][name]
                with self.assertRaisesRegex(ValueError, "Incomplete"):
                    self.verify()
                self.spec = copy.deepcopy(original)
                key = "revision" if name == "type2-runtime" else "version"
                self.spec["sources"][name][key] = "0" * 40 if key == "revision" else "99.0.0"
                with self.assertRaisesRegex(ValueError, "identity mismatch"):
                    self.verify()

    def test_missing_modified_source_and_patch_checksums_fail(self):
        original = copy.deepcopy(self.spec)
        for name in original["sources"]:
            self.spec = copy.deepcopy(original)
            self.spec["sources"][name]["sha256"] = "0" * 64
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "modified runtime input"):
                self.verify()
        self.spec = copy.deepcopy(original)
        self.spec["upstream_patch_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "upstream libfuse patch"):
            self.verify()

    def test_every_recipe_file_is_required_and_checksum_verified(self):
        for item in self.spec["recipe"]:
            path = self.kit / item["file"]
            original = path.read_bytes()
            path.unlink()
            with self.subTest(file=path.name), self.assertRaisesRegex(ValueError, "Missing or modified"):
                self.verify()
            path.write_bytes(original + b"tampered")
            with self.assertRaisesRegex(ValueError, "Missing or modified"):
                self.verify()
            path.write_bytes(original)

    def test_missing_toolchain_and_startup_evidence_fail(self):
        original = copy.deepcopy(self.spec)
        for key, value in (("packages", []), ("compiler_sources", []), ("toolchain_objects", {})):
            self.spec = copy.deepcopy(original)
            self.spec[key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "Incomplete"):
                self.verify()

    def test_kernel_header_source_must_match_the_pinned_package(self):
        self.spec["linux_headers"]["version"] = "different"
        with self.assertRaisesRegex(ValueError, "header/package identity"):
            self.verify()

    def test_missing_or_altered_cached_input_never_starts_a_build(self):
        cache = self.root / "cache"
        cache.mkdir()
        work = self.root / "work"
        with patch.object(builder, "KIT", self.kit), patch.object(builder, "verify", return_value=self.spec), \
                patch.object(builder.subprocess, "run") as run:
            for content in (None, b"wrong rootfs"):
                if content:
                    (cache / self.spec["image"]["file"]).write_bytes(content)
                with self.assertRaisesRegex(ValueError, "Missing or modified"):
                    builder.build(licenses.EVIDENCE / "sources", cache, work)
                self.assertFalse(work.exists())
            run.assert_not_called()

    def test_pin_change_requires_provenance_review_before_loading_recipe(self):
        (self.root / "packaging").mkdir()
        (self.root / "packaging/appimage-tools.json").write_text(json.dumps({"runtime": {"manifest_sha256": "0" * 64}}))
        with self.assertRaisesRegex(ValueError, "inputs changed"):
            appimage_runtime.checked_kit(self.root, self.root)

    def test_distribution_contains_every_relink_input(self):
        import io
        import zipfile
        spec = self.verify()
        with zipfile.ZipFile(io.BytesIO(licenses.materials()["THIRD-PARTY-SOURCES.zip"])) as archive:
            for item in [*spec["sources"].values(), *spec["compiler_sources"], spec["linux_headers"], *spec["recipe"]]:
                self.assertEqual(hashlib.sha256(archive.read(item["file"])).hexdigest(), item["sha256"])
            self.assertEqual(archive.read("runtime-inputs.json"), (KIT / "runtime-inputs.json").read_bytes())

    def test_accepted_risk_does_not_clear_runtime_obligations(self):
        catalog, texts = licenses.checked_catalog()
        blocked = copy.deepcopy(catalog)
        blocked["release_blockers"].append({"id": "fixture-relink-gap", "detail": "Missing relink input"})
        with patch.object(licenses, "checked_catalog", return_value=(blocked, texts)):
            with self.assertRaisesRegex(ValueError, "fixture-relink-gap"):
                licenses.require_release_ready()
        self.assertEqual(catalog["release_blockers"], [])
        self.assertEqual([r["id"] for r in catalog["accepted_licensing_risks"]], ["windows-crt"])


class Repeatability(unittest.TestCase):
    def test_changed_output_or_evidence_cannot_pass_comparison(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "packaging").mkdir()
            pin = {"manifest_sha256": "1" * 64, "sha256": hashlib.sha256(b"runtime").hexdigest()}
            (root / "packaging/appimage-tools.json").write_text(json.dumps({"runtime": pin}))
            first, second = root / "first", root / "second"
            audit = {"manifest_sha256": pin["manifest_sha256"], "runtime_sha256": pin["sha256"],
                     "recipient_patch_sha256": None, "full_sha256": hashlib.sha256(b"full").hexdigest(),
                     "map_sha256": hashlib.sha256(b"map").hexdigest()}
            for folder in (first, second):
                folder.mkdir()
                for name, data in (("runtime", b"runtime"), ("runtime.full", b"full"), ("runtime.map", b"map"),
                                   ("audit.json", json.dumps(audit).encode())):
                    (folder / name).write_bytes(data)
            with patch.object(appimage_runtime, "ROOT", root), patch.object(appimage_runtime, "checked_kit"):
                with self.assertRaisesRegex(ValueError, "two separate"):
                    appimage_runtime.compare_builds(first, first)
                appimage_runtime.compare_builds(first, second)
                for name in ("runtime", "runtime.full", "runtime.map"):
                    original = (second / name).read_bytes()
                    (second / name).write_bytes(b"changed")
                    with self.subTest(name=name), self.assertRaises(ValueError):
                        appimage_runtime.compare_builds(first, second)
                    (second / name).write_bytes(original)
                audit["recipient_patch_sha256"] = "2" * 64
                (second / "audit.json").write_text(json.dumps(audit))
                with self.assertRaisesRegex(ValueError, "Stale, modified"):
                    appimage_runtime.compare_builds(first, second)


if __name__ == "__main__":
    unittest.main()
