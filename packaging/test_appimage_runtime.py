"""Official download identity, delivered relinking inputs and cleanup observation."""
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import licenses
import linux


class OfficialRuntime(unittest.TestCase):
    def test_download_rejects_corrupt_bytes_before_making_them_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "packaging").mkdir()
            (root / "packaging/appimage-tools.json").write_text(json.dumps({"runtime": {
                "url": "https://example.invalid/runtime", "sha256": hashlib.sha256(b"reviewed").hexdigest()}}))
            with patch.object(linux, "ROOT", root), \
                    patch.object(linux.urllib.request, "urlopen", return_value=io.BytesIO(b"changed")), \
                    patch.object(Path, "chmod") as chmod:
                with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
                    linux.download_tools(root)
                chmod.assert_not_called()
            with patch.object(linux, "ROOT", root), \
                    patch.object(linux.urllib.request, "urlopen", return_value=io.BytesIO(b"reviewed")):
                self.assertEqual(linux.download_tools(root)["runtime"].read_bytes(), b"reviewed")

    def test_distributed_kit_contains_demonstrated_inputs_and_no_private_builder(self):
        catalog, _ = licenses.checked_catalog()
        record = json.loads((licenses.EVIDENCE / catalog["native_provenance"]["appimage"]["relink_evidence"]).read_text())
        with zipfile.ZipFile(io.BytesIO(licenses.materials()["THIRD-PARTY-SOURCES.zip"])) as archive:
            for name, sha in record["recipient_inputs"].items():
                self.assertEqual(hashlib.sha256(archive.read(name)).hexdigest(), sha)
            for obsolete in ("build-runtime.py", "build-runtime.sh", "runtime-inputs.json", "gcc-14.2.0-alpine-r4.tar.gz"):
                self.assertNotIn(obsolete, archive.namelist())

    def test_accepted_risk_does_not_clear_runtime_obligations(self):
        catalog, texts = licenses.checked_catalog()
        blocked = copy.deepcopy(catalog)
        blocked["release_blockers"].append({"id": "fixture-relink-gap", "detail": "Missing relink input"})
        with patch.object(licenses, "checked_catalog", return_value=(blocked, texts)):
            with self.assertRaisesRegex(ValueError, "fixture-relink-gap"):
                licenses.require_release_ready()
        self.assertEqual(catalog["release_blockers"], [])
        self.assertEqual([r["id"] for r in catalog["accepted_licensing_risks"]], ["windows-crt"])


class ExtractionObservation(unittest.TestCase):
    def exercise(self, retain=False, accumulate=False, fail=False):
        launches = []

        def launch(*args, env):
            launches.append(args)
            self.assertNotIn("NO_CLEANUP", env)
            if len(launches) == 2:
                self.assertEqual(env["APPIMAGE_EXTRACT_AND_RUN"], "1")
            else:
                self.assertNotIn("APPIMAGE_EXTRACT_AND_RUN", env)
                self.assertEqual(args[-1], "--appimage-extract-and-run")
            if fail:
                raise subprocess.CalledProcessError(17, args)
            if retain:
                name = str(len(launches) if accumulate else 1) * 32
                payload = Path(env["TMPDIR"]) / f"appimage_extracted_{name}" / "usr/bin"
                payload.mkdir(parents=True, exist_ok=True)
                (payload / "codeconvoy").write_bytes(b"fixture")

        with patch.object(licenses, "verify_appimage_runtime") as verify, \
                patch.object(linux, "run", side_effect=launch), patch("builtins.print") as output:
            linux.check_extraction_cleanup(Path("fixture.AppImage"), {"NO_CLEANUP": "1", "APPIMAGE_EXTRACT_AND_RUN": "0"})
            verify.assert_called_once()
        self.assertEqual(len(launches), 3)
        return "\n".join(str(call.args[0]) for call in output.call_args_list)

    def test_known_retention_is_detected_and_reported_without_claiming_cleanup_passed(self):
        output = self.exercise(retain=True)
        self.assertEqual(output.count("KNOWN UPSTREAM DEFECT"), 3)
        self.assertNotIn("PASS extraction cleanup", output)

    def test_clean_behavior_is_observed(self):
        self.assertEqual(self.exercise().count("PASS extraction cleanup"), 3)

    def test_additional_retained_directories_are_a_regression(self):
        with self.assertRaisesRegex(ValueError, "Unexpected extraction cleanup regression"):
            self.exercise(retain=True, accumulate=True)

    def test_application_failure_cannot_be_waived_as_cleanup(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.exercise(fail=True)

    def test_changed_pin_cannot_inherit_the_exception(self):
        pin = {"runtime": {"sha256": "0" * 64}}
        with patch.object(linux.json, "loads", return_value=pin):
            with self.assertRaisesRegex(ValueError, "Unexpected extraction cleanup regression"):
                self.exercise(retain=True)


if __name__ == "__main__":
    unittest.main()
