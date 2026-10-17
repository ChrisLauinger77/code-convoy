"""Tests for gates that must fail before any public release is created."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import zipfile

import release
import licenses


class ReleaseGates(unittest.TestCase):
    def test_cargo_lock_and_release_notes_match_package_version(self):
        with (release.ROOT / "Cargo.lock").open("rb") as source:
            packages = tomllib.load(source)["package"]
        versions = [p["version"] for p in packages if p["name"] == "codeconvoy"]
        self.assertEqual(versions, [release.version()])
        notes = (release.ROOT / "docs/release-notes.md").read_text(encoding="utf-8")
        self.assertTrue(notes.startswith(f"# CodeConvoy {release.version()} — "))

    def test_deb_uses_measured_requirement_and_refuses_missing_measurement(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(release, "run"), patch.object(licenses, "prepare", return_value=Path("notices")):
            work = Path(temporary)
            with self.assertRaisesRegex(ValueError, "measured glibc"):
                release.package(work, work, "codeconvoy", "deb")
            release.package(work, work, "codeconvoy", "deb", glibc="2.43")
            dependencies = json.loads((work / "packager.json").read_text())["deb"]["depends"]
            self.assertIn("libc6 (>= 2.43)", dependencies)
            self.assertNotIn("libc6", dependencies)
            self.assertIn("libxkbcommon-x11-0", dependencies)

    def test_portable_zip_requires_exact_binary_license_and_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            exe = directory / "CodeConvoy.exe"
            exe.write_bytes(b"fixture executable")
            archive = directory / "portable.zip"
            license_text = (release.ROOT / "LICENSE").read_bytes()
            valid = {"CodeConvoy.exe": exe.read_bytes(), "LICENSE.txt": license_text, **licenses.materials()}
            microsoft_notice = (licenses.EVIDENCE / licenses.MICROSOFT_NOTICE).read_bytes()
            notices = valid["THIRD-PARTY-NOTICES.txt"]
            self.assertEqual(notices.count(microsoft_notice), 1)
            for contents in (
                {"CodeConvoy.exe": exe.read_bytes()},
                {**valid, "CodeConvoy.exe": b"wrong binary"},
                {**valid, "LICENSE.txt": b"wrong license"},
                {**valid, "debug.log": b"log"},
                {k: v for k, v in valid.items() if k != "THIRD-PARTY-NOTICES.txt"},
                {**valid, "THIRD-PARTY-NOTICES.txt": notices.replace(microsoft_notice, b"")},
                {**valid, "THIRD-PARTY-NOTICES.txt": notices.replace(microsoft_notice, microsoft_notice.replace(
                    b"Microsoft Corporation", b"Altered Corporation", 1))},
                {**valid, "THIRD-PARTY-SOURCES.zip": b"missing source"},
                valid,
            ):
                with self.subTest(files=list(contents)):
                    with zipfile.ZipFile(archive, "w") as zipped:
                        for name, data in contents.items():
                            zipped.writestr(name, data)
                    if contents == valid:
                        release.verify_windows_zip(archive, exe)
                    else:
                        with self.assertRaisesRegex(ValueError, "ZIP failed"):
                            release.verify_windows_zip(archive, exe)

    def test_installer_and_app_ship_license_as_resource(self):
        rust = licenses.checked_catalog()[0]["inputs"]["rust"]
        compiler = subprocess.CompletedProcess([], 0, f"rustc {rust} (fixture)\n")
        with tempfile.TemporaryDirectory() as temporary, patch.object(release, "run"), \
                patch("windows_crt.configure_installer"), patch.object(licenses, "BUNDLE", Path(temporary) / "notices"), \
                patch.object(licenses.subprocess, "run", return_value=compiler):
            work = Path(temporary)
            microsoft_notice = (licenses.EVIDENCE / licenses.MICROSOFT_NOTICE).read_bytes()
            for format_name in ("app", "nsis"):
                release.package(work, work, "codeconvoy", format_name)
                config = json.loads((work / "packager.json").read_text())
                self.assertIn({"src": "LICENSE", "target": "LICENSE.txt"}, config["resources"])
                for name in licenses.FILES:
                    self.assertIn({"src": str(licenses.BUNDLE / name), "target": name}, config["resources"])
                notice_resource = next(r for r in config["resources"] if r["target"] == "THIRD-PARTY-NOTICES.txt")
                self.assertEqual(Path(notice_resource["src"]).read_bytes().count(microsoft_notice), 1)

    def test_tag_must_match_cargo_exactly(self):
        release.check_tag("v" + release.version())
        for tag in (None, release.version(), "v999.0.0", "v" + release.version() + "-alpha"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.check_tag(tag)

    def test_universal_requires_both_architectures(self):
        release.verify_architectures("x86_64 arm64\n")
        for architectures in ("arm64", "x86_64", "", "arm64 x86_64 i386"):
            with self.subTest(architectures=architectures), self.assertRaises(ValueError):
                release.verify_architectures(architectures)

    def test_complete_assets_produce_exact_final_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in release.artifact_names(release.version()):
                (directory / name).write_bytes(name.encode())
            release.aggregate(directory)
            lines = (directory / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(lines), 6)
            for line in lines:
                digest, name = line.split("  ")
                self.assertEqual(digest, hashlib.sha256(name.encode()).hexdigest())
            release.aggregate(directory)  # A previous checksum file is not an extra asset.
            release.verify_checksums(directory)

    def test_checksum_manifest_cannot_omit_duplicate_or_rehash_corrupt_assets(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in release.artifact_names(release.version()):
                (directory / name).write_bytes(name.encode())
            release.aggregate(directory)
            manifest = directory / "SHA256SUMS"
            original = manifest.read_text()
            lines = original.splitlines(keepends=True)
            for invalid in ("", "".join(lines[1:]), original + lines[0],
                            original.replace(lines[0][:64], "0" * 64),
                            original + "0" * 64 + "  extra.zip\n"):
                manifest.write_text(invalid)
                with self.assertRaises(ValueError):
                    release.verify_checksums(directory)
                self.assertEqual(manifest.read_text(), invalid)
            manifest.write_text(original)
            (directory / release.artifact_names(release.version())[-1]).write_bytes(b"corrupted")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                release.verify_checksums(directory)
            self.assertEqual(manifest.read_text(), original)

    def test_missing_empty_and_extra_assets_fail_closed(self):
        names = release.artifact_names(release.version())
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in names:
                (directory / name).write_bytes(b"package")
            (directory / names[0]).unlink()
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            (directory / names[0]).touch()
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            (directory / names[0]).write_bytes(b"package")
            (directory / "debug.log").write_text("not distributable")
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            self.assertFalse((directory / "SHA256SUMS").exists())


if __name__ == "__main__":
    unittest.main()
