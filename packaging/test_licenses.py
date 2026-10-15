"""License coverage, source identity and final-artifact regression tests."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import io

import licenses
import linux_abi


class LicenseEvidence(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in ("Cargo.lock", "Cargo.toml", "packaging/appimage-tools.json", ".github/workflows/release.yml"):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(licenses.ROOT / name, destination)
        self.evidence = self.root / "third-party"
        shutil.copytree(licenses.EVIDENCE, self.evidence)
        self.catalog = json.loads((self.evidence / "catalog.json").read_text())

    def check(self):
        (self.evidence / "catalog.json").write_text(json.dumps(self.catalog))
        return licenses.checked_catalog(self.root, self.evidence)

    def test_locked_graph_has_coverage_and_explicit_exclusions(self):
        catalog, texts = self.check()
        self.assertTrue(any(not p["targets"] for p in catalog["crates"]))
        for package in catalog["crates"]:
            if package["targets"]:
                self.assertTrue(package["notices"] or package.get("unresolved"))
        combined = "\n".join(texts.values())
        for term in ("SIL OPEN FONT LICENSE", "UBUNTU FONT LICENCE", "Bitstream", "Unicode", "Mozilla Public License"):
            self.assertIn(term, combined)

    def test_new_missing_duplicate_and_changed_crates_fail(self):
        original = copy.deepcopy(self.catalog["crates"])
        changed = copy.deepcopy(original)
        changed[0]["checksum"] = "0" * 64
        for packages in (original[:-1], original + [original[0]], changed):
            with self.subTest(count=len(packages)), self.assertRaisesRegex(ValueError, "Cargo.lock changed"):
                self.catalog["crates"] = packages
                self.check()

    def test_feature_and_tool_changes_require_reaudit(self):
        for name, before, after in (("Cargo.toml", '[features]', '[features]\nlicense_test = []'),
                                    (".github/workflows/release.yml", '1.99.0', '1.100.0'),
                                    (".github/workflows/release.yml", '0.11.8', '0.11.9'),
                                    ("packaging/appimage-tools.json", '20251108', '20251109')):
            path = self.root / name
            original = path.read_text()
            self.assertIn(before, original)
            path.write_text(original.replace(before, after))
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "changed"):
                self.check()
            path.write_text(original)

    def test_notice_removal_or_tamper_cannot_be_silently_accepted(self):
        package = next(p for p in self.catalog["crates"] if p["targets"] and not p.get("unresolved"))
        package["notices"] = []
        with self.assertRaisesRegex(ValueError, "Missing notices"):
            self.check()
        package["unresolved"] = "made-up-waiver"
        with self.assertRaisesRegex(ValueError, "Untracked"):
            self.check()

    def test_notice_hash_and_source_corruption_fail(self):
        path = self.evidence / "texts.json"
        texts = json.loads(path.read_text())
        texts[next(iter(texts))] += "changed"
        path.write_text(json.dumps(texts))
        with self.assertRaisesRegex(ValueError, "altered notice"):
            self.check()
        shutil.copyfile(licenses.EVIDENCE / "texts.json", path)
        source = self.evidence / self.catalog["sources"][0]["path"]
        source.write_bytes(b"not corresponding source")
        with self.assertRaisesRegex(ValueError, "altered license/source"):
            self.check()

    def test_mpl_source_cannot_be_removed_or_replaced_by_another_version(self):
        self.catalog["sources"] = [s for s in self.catalog["sources"] if 'option-ext' not in s["path"]]
        with self.assertRaisesRegex(ValueError, "MPL source archive"):
            self.check()

    def test_no_missing_notice_can_hide_by_dropping_its_blocker(self):
        self.catalog["release_blockers"] = []
        with self.assertRaisesRegex(ValueError, "Untracked"):
            self.check()

    def test_bundle_is_deterministic_and_contains_exact_mpl_source(self):
        first = licenses.materials(self.root, self.evidence)
        self.assertEqual(first, licenses.materials(self.root, self.evidence))
        with zipfile.ZipFile(io.BytesIO(first["THIRD-PARTY-SOURCES.zip"])) as source:
            self.assertIsNone(source.testzip())
            self.assertTrue(all(info.create_system == 3 and info.compress_type == zipfile.ZIP_STORED
                                for info in source.infolist()))
            self.assertEqual(source.read("option-ext-0.2.0.crate"),
                             (self.evidence / "sources/option-ext-0.2.0.crate").read_bytes())
        self.assertIn(b"UNRESOLVED RELEASE OBLIGATIONS", first["THIRD-PARTY-NOTICES.txt"])


class FinalLicensePayload(unittest.TestCase):
    def test_packaging_rejects_a_compiler_with_different_stdlib_notices(self):
        wrong = subprocess.CompletedProcess([], 0, "rustc 1.98.0 (fixture)\n")
        with patch.object(licenses.subprocess, "run", return_value=wrong):
            with self.assertRaisesRegex(ValueError, "compiler differs"):
                licenses.prepare()

    def test_final_appimage_checks_runtime_code_not_just_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "packaging").mkdir()
            original = b"\x7fELF" + bytes(944632 - 4)
            runtime, image = root / "runtime", root / "image"
            runtime.write_bytes(original)
            (root / "packaging/appimage-tools.json").write_text(json.dumps({"runtime": {"sha256": licenses.digest(original)}}))
            packaged = bytearray(original)
            packaged[932096:932112] = b"digest field...."
            image.write_bytes(packaged + b"squashfs")
            with patch.object(licenses, "ROOT", root):
                licenses.verify_appimage_runtime(image, runtime)
                packaged[100] = 1
                image.write_bytes(packaged + b"squashfs")
                with self.assertRaisesRegex(ValueError, "runtime differs"):
                    licenses.verify_appimage_runtime(image, runtime)
                runtime.write_bytes(b"different upstream download")
                with self.assertRaisesRegex(ValueError, "Unreviewed"):
                    licenses.verify_appimage_runtime(image, runtime)

    def test_final_installer_checks_ephemeral_plugins_and_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe = root / "main.exe"
            exe.write_bytes(b"MZfixture")
            plugin = b"MZplugin"
            catalog = {"windows_installer_plugins": {"$PLUGINSDIR/plugin.dll": licenses.digest(plugin)}}
            mode = "valid"
            def extract(args, **kwargs):
                payload = Path(next(a[2:] for a in args if a.startswith('-o')))
                (payload / "$PLUGINSDIR").mkdir()
                (payload / "CodeConvoy.exe").write_bytes(exe.read_bytes())
                (payload / "uninstall.exe").write_bytes(b"MZuninstall")
                (payload / "LICENSE.txt").write_bytes((licenses.ROOT / "LICENSE").read_bytes())
                (payload / "$PLUGINSDIR/plugin.dll").write_bytes(plugin if mode != "changed plugin" else b"MZchanged")
                if mode != "missing notice":
                    (payload / "notice.txt").write_bytes(b"notice")
                if mode == "new library":
                    (payload / "$PLUGINSDIR/extra.dll").write_bytes(b"MZextra")
                return subprocess.CompletedProcess(args, 0)
            with patch.object(licenses, "checked_catalog", return_value=(catalog, {})), \
                 patch.object(licenses, "materials", return_value={"notice.txt": b"notice"}), \
                 patch.object(licenses.subprocess, "run", side_effect=extract):
                licenses.verify_windows_installer(root / "final.exe", root, exe)
                for mode in ("changed plugin", "missing notice", "new library"):
                    with self.subTest(mode=mode), self.assertRaises(ValueError):
                        licenses.verify_windows_installer(root / "final.exe", root, exe)

    def test_every_missing_or_tampered_material_fails_on_final_directory(self):
        data = licenses.materials()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, contents in data.items():
                (root / name).write_bytes(contents)
            licenses.verify_directory(root)
            for name, contents in data.items():
                with self.subTest(name=name):
                    (root / name).unlink()
                    with self.assertRaisesRegex(ValueError, "omits or alters"):
                        licenses.verify_directory(root)
                    (root / name).write_bytes(b"truncated")
                    with self.assertRaisesRegex(ValueError, "omits or alters"):
                        licenses.verify_directory(root)
                    (root / name).write_bytes(contents)

    def test_new_bundled_library_requires_license_review_on_each_platform(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "main").write_bytes(b"\x7fELFapplication")
            licenses.verify_binaries(root, {"main"})
            for magic in (b"\x7fELF", b"MZxx", b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe"):
                with self.subTest(magic=magic):
                    (root / "new-library").write_bytes(magic)
                    with self.assertRaisesRegex(ValueError, "Native payload differs"):
                        licenses.verify_binaries(root, {"main"})
                    (root / "new-library").unlink()

    def test_final_linux_extraction_invokes_notice_gate_and_propagates_failure(self):
        # A valid staging tree must not mask omission from any finished format.
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            artifact = work / "candidate.deb"
            artifact.write_bytes(b"final package bytes")
            def extract(*args, **kwargs):
                if args[0] == "dpkg-deb":
                    Path(args[-1]).mkdir()
                return subprocess.CompletedProcess(args, 0)
            with patch.object(linux_abi, "run", side_effect=extract), patch.object(linux_abi, "inspect_payload", return_value=[("main", (2, 39))]):
                with self.assertRaisesRegex(ValueError, "Final package omits"):
                    linux_abi.verify_artifacts([artifact], work)
            self.assertFalse((work / "glibc-requirements.txt").exists())

    def test_deb_rpm_and_appimage_use_correct_final_notice_location(self):
        with patch.object(licenses, "verify_directory") as notices, patch.object(licenses, "verify_binaries") as native:
            for suffix, path in ((".deb", "usr/share/doc/code-convoy"), (".rpm", "usr/share/licenses/code-convoy"), (".AppImage", "usr/share/doc/code-convoy")):
                licenses.verify_linux_payload(Path("final"), suffix)
                notices.assert_called_with(Path("final") / path)
                native.assert_called_with(Path("final"), {"usr/bin/codeconvoy"})

    def test_release_is_blocked_by_documented_unresolved_obligations(self):
        with self.assertRaisesRegex(ValueError, "license release blockers remain"):
            licenses.require_release_ready()


if __name__ == "__main__":
    unittest.main()
