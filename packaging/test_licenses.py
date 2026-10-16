"""License coverage, source identity and final-artifact regression tests."""
import copy
import json
import os
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
        for name in ("Cargo.lock", "Cargo.toml", "packaging/appimage-tools.json", ".github/workflows/release.yml",
                     "packaging/nsis_helpers.py", "packaging/windows_crt.py",
                     "packaging/windows_runtime.py", "packaging/windows-runtime.json"):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(licenses.ROOT / name, destination)
        shutil.copytree(licenses.ROOT / "packaging/nsis", self.root / "packaging/nsis")
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

    def test_microsoft_notice_is_delivered_verbatim_with_blocker_active(self):
        catalog, texts = self.check()
        component = next(c for c in catalog["components"] if c["name"] == licenses.MICROSOFT_COMPONENT)
        notice = next(n for n in component["notices"] if n["path"] == licenses.MICROSOFT_NOTICE)
        original = (self.evidence / licenses.MICROSOFT_NOTICE).read_bytes()
        self.assertEqual(original, texts[notice["text"]].encode("utf-8"))
        material = licenses.materials(self.root, self.evidence)
        self.assertEqual(material["THIRD-PARTY-NOTICES.txt"].count(original), 1)
        self.assertEqual(set(material), set(licenses.FILES))  # Existing four-file delivery.
        self.assertIn(b"CodeConvoy's original code and artwork remain under the MIT license", original)
        self.assertIn(b"CodeConvoy Windows releases", original)
        self.assertEqual(component["unresolved"], "windows-crt")
        self.assertEqual([b["id"] for b in catalog["release_blockers"]], ["windows-crt"])

    def test_microsoft_notice_cannot_lose_its_component_or_source_binding(self):
        original = copy.deepcopy(self.catalog)
        for mode in ("component", "notice", "file", "other valid text"):
            self.catalog = copy.deepcopy(original)
            component = next(c for c in self.catalog["components"] if c["name"] == licenses.MICROSOFT_COMPONENT)
            if mode == "component":
                self.catalog["components"].remove(component)
            elif mode == "notice":
                component["notices"] = []
            elif mode == "file":
                self.catalog["files"] = [f for f in self.catalog["files"] if f["path"] != licenses.MICROSOFT_NOTICE]
            else:
                component["notices"][0]["text"] = self.catalog["crates"][0]["notices"][0]["text"]
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "Microsoft Distributable Code notice binding"):
                self.check()
        self.catalog = original
        source = self.evidence / licenses.MICROSOFT_NOTICE
        source.write_bytes(source.read_bytes() + b"altered")
        with self.assertRaisesRegex(ValueError, "altered license/source evidence"):
            self.check()
        # Updating just the source-file pin must not replace the delivered text.
        next(f for f in self.catalog["files"] if f["path"] == licenses.MICROSOFT_NOTICE)["sha256"] = licenses.digest(source.read_bytes())
        with self.assertRaisesRegex(ValueError, "Microsoft Distributable Code notice binding"):
            self.check()

    def test_mpl_source_cannot_be_removed_or_replaced_by_another_version(self):
        self.catalog["sources"] = [s for s in self.catalog["sources"] if 'option-ext' not in s["path"]]
        with self.assertRaisesRegex(ValueError, "MPL source archive"):
            self.check()

    def test_runtime_build_and_relink_evidence_must_remain_bound(self):
        records = self.catalog["native_provenance"]["appimage"]
        original = copy.deepcopy(self.catalog["files"])
        for key in ("build_audit", "relink_audit"):
            self.catalog["files"] = [s for s in original if s["path"] != records[key]]
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "build/relink evidence"):
                self.check()

    def test_stdlib_report_does_not_replace_its_own_lockfile_coverage(self):
        self.catalog["components"] = [c for c in self.catalog["components"]
                                      if c["name"] != "Rust std dependency: addr2line"]
        with self.assertRaisesRegex(ValueError, "Missing Rust standard-library notice"):
            self.check()

    def test_stdlib_dependency_cannot_disappear_from_inventory(self):
        self.catalog["rust_library_packages"].pop()
        with self.assertRaisesRegex(ValueError, "standard-library dependency inventory"):
            self.check()

    def test_compiler_builtins_and_nested_source_notices_cannot_be_omitted(self):
        self.catalog["components"] = [c for c in self.catalog["components"]
                                      if not c["name"].startswith("Rust in-tree library dependencies")]
        with self.assertRaisesRegex(ValueError, "Missing Rust in-tree library notices"):
            self.check()

    def test_locked_plugin_dependencies_cannot_lose_notice_coverage(self):
        self.catalog["components"] = [c for c in self.catalog["components"]
                                      if c["name"] != "CodeConvoy NSIS helper dependency: semver"]
        with self.assertRaisesRegex(ValueError, "Missing NSIS dependency inventory/notice coverage"):
            self.check()

    def test_native_provenance_cannot_describe_another_binary(self):
        for component, key in (("appimage", "runtime_sha256"), ("appimage", "source_revision")):
            record = self.catalog["native_provenance"][component]
            original = record[key]
            record[key] = "0" * 64
            with self.subTest(component=component), self.assertRaisesRegex(ValueError, "provenance does not match"):
                self.check()
            record[key] = original
        self.catalog["native_provenance"]["nsis_replacements"]["binaries"]["CodeConvoyAppId.dll"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "provenance does not match"):
            self.check()

    def test_nsis_source_and_locked_dependency_delivery_are_required(self):
        record = self.catalog["native_provenance"]["nsis_replacements"]
        for source in (record["source_archive"], "sources/windows-sys-0.48.0.crate"):
            original = copy.deepcopy(self.catalog["sources"])
            self.catalog["sources"] = [p for p in original if p["path"] != source]
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, "NSIS"):
                self.check()
            self.catalog["sources"] = original
        record["dependencies"].pop()
        with self.assertRaisesRegex(ValueError, "locked NSIS dependency inventory"):
            self.check()

    def test_restoring_old_cached_plugins_is_rejected(self):
        self.catalog["windows_installer_plugins"]["$PLUGINSDIR/ApplicationID.dll"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "Old NSIS prebuilt plugin"):
            self.check()

    def test_nsis_source_changes_require_a_new_reviewed_build_kit(self):
        path = self.root / "third-party/nsis/appid.cpp"
        path.write_bytes(path.read_bytes() + b"\n// changed\n")
        with self.assertRaisesRegex(ValueError, "NSIS helper sources/build inputs changed"):
            self.check()

    def test_identified_appimage_source_cannot_be_omitted(self):
        self.catalog["sources"] = [s for s in self.catalog["sources"] if 'mimalloc' not in s["path"]]
        with self.assertRaisesRegex(ValueError, "Missing AppImage version/source"):
            self.check()

    def test_compiler_startup_evidence_cannot_be_omitted(self):
        startup = self.catalog["native_provenance"]["appimage"]["compiler_startup"]
        component = next(c for c in self.catalog["components"] if c["name"] == startup["component"])
        for notice in startup["reference_notices"]:
            with self.subTest(notice=notice["origin"]):
                component["notices"].remove(notice)
                with self.assertRaisesRegex(ValueError, "Missing AppImage compiler startup"):
                    self.check()
                component["notices"].append(notice)
        for path in startup["reference_sources"]:
            with self.subTest(source=path):
                source = next(s for s in self.catalog["sources"] if s["path"] == path)
                self.catalog["sources"].remove(source)
                with self.assertRaisesRegex(ValueError, "Missing AppImage compiler startup"):
                    self.check()
                self.catalog["sources"].append(source)
        self.catalog["components"].remove(component)
        with self.assertRaisesRegex(ValueError, "Missing AppImage compiler startup"):
            self.check()

    def test_no_missing_notice_can_hide_by_dropping_its_blocker(self):
        self.catalog["crates"][0]["unresolved"] = "fixture-obligation"
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

    def test_source_zip_ignores_inventory_order_and_filesystem_metadata(self):
        first = licenses.materials(self.root, self.evidence)["THIRD-PARTY-SOURCES.zip"]
        self.catalog["sources"].reverse()
        self.check()
        for item in self.catalog["sources"]:
            path = self.evidence / item["path"]
            os.utime(path, (1700000000, 1700000000))
            path.chmod(0o755)
        self.assertEqual(first, licenses.materials(self.root, self.evidence)["THIRD-PARTY-SOURCES.zip"])
        with zipfile.ZipFile(io.BytesIO(first)) as archive:
            self.assertEqual(archive.namelist(), sorted(archive.namelist()))
            self.assertEqual(archive.comment, b"")
            for info in archive.infolist():
                self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
                self.assertEqual((info.create_system, info.create_version, info.extract_version), (3, 20, 20))
                self.assertEqual(info.external_attr, 0o100644 << 16)
                self.assertEqual((info.internal_attr, info.flag_bits, info.volume), (0, 0, 0))
                self.assertEqual((info.extra, info.comment), (b"", b""))
                if info.filename != "README.txt":
                    item = next(s for s in self.catalog["sources"] if Path(s["path"]).name == info.filename)
                    self.assertEqual(archive.read(info), (self.evidence / item["path"]).read_bytes())

    def test_source_zip_portable_golden_bytes_preserve_binary_and_line_endings(self):
        # This fixed digest runs on Windows, Linux and macOS. A same-host repeat
        # alone would miss platform-dependent ZIP headers or text conversion.
        entries = [("z.bin", bytes(range(256))), ("README.txt", b"line one\r\nline two\n\x00\xff")]
        expected = "92c32ed875aaba4578f986262a5f4ba279769c75b07262e0679c148b8c229be8"
        for ordered in (entries, list(reversed(entries))):
            data = licenses.source_bundle(ordered)
            self.assertEqual(licenses.digest(data), expected)
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                for name, contents in entries:
                    self.assertEqual(archive.read(name), contents)

    def test_source_zip_rejects_ambiguous_and_nonportable_names(self):
        for names in (("a", "a"), ("README.txt", "readme.txt"), ("../source",),
                      ("nested/source",), ("nested\\source",), ("C:source",),
                      ("source\x00hidden",), ("é.txt",), ("source.",), ("CON.txt",)):
            with self.subTest(names=names), self.assertRaisesRegex(ValueError, "portable basenames"):
                licenses.source_bundle((name, b"source") for name in names)


class FinalLicensePayload(unittest.TestCase):
    def test_final_source_zip_repacking_cannot_hide_changed_metadata(self):
        expected = licenses.source_bundle([("source.txt", b"same source bytes")])
        # Both ZIPs extract to identical source. Only the final archive metadata
        # differs; comparing staging files or extracted contents would miss it.
        changed = io.BytesIO()
        with zipfile.ZipFile(changed, "w") as archive:
            archive.writestr("source.txt", b"same source bytes")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "THIRD-PARTY-SOURCES.zip"
            with patch.object(licenses, "materials", return_value={path.name: expected}):
                path.write_bytes(expected)
                licenses.verify_directory(root)
                path.write_bytes(changed.getvalue())
                with self.assertRaisesRegex(ValueError, "omits or alters"):
                    licenses.verify_directory(root)

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
            (root / "packaging/appimage-tools.json").write_text(json.dumps({"runtime": {
                "sha256": licenses.digest(original), "size": len(original), "md5_offset": 932096}}))
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
        materials = licenses.materials()
        microsoft_notice = (licenses.EVIDENCE / licenses.MICROSOFT_NOTICE).read_bytes()
        self.assertIn(microsoft_notice, materials["THIRD-PARTY-NOTICES.txt"])
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
                for name, data in materials.items():
                    if name == "THIRD-PARTY-NOTICES.txt":
                        if mode == "missing notice":
                            continue
                        if mode == "missing Microsoft notice":
                            data = data.replace(microsoft_notice, b"")
                        elif mode == "altered Microsoft notice":
                            data = data.replace(microsoft_notice, microsoft_notice.replace(
                                b"Microsoft Corporation", b"Altered Corporation", 1))
                    (payload / name).write_bytes(data)
                if mode == "new library":
                    (payload / "$PLUGINSDIR/extra.dll").write_bytes(b"MZextra")
                return subprocess.CompletedProcess(args, 0)
            with patch.object(licenses, "checked_catalog", return_value=(catalog, {})), \
                 patch.object(licenses, "materials", return_value=materials), \
                 patch("windows_crt.inspect_installer") as inspect_crt, \
                 patch.object(licenses.subprocess, "run", side_effect=extract):
                licenses.verify_windows_installer(root / "final.exe", root, exe)
                for mode in ("changed plugin", "missing notice", "missing Microsoft notice",
                             "altered Microsoft notice", "new library"):
                    with self.subTest(mode=mode), self.assertRaises(ValueError):
                        licenses.verify_windows_installer(root / "final.exe", root, exe)
                if licenses.os.name == "nt":
                    mode = "valid"
                    inspect_crt.side_effect = ValueError("CRT inspection failed")
                    with self.assertRaisesRegex(ValueError, "CRT inspection failed"):
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
        with patch.object(licenses, "checked_catalog", return_value=({"release_blockers": [{"id": "fixture-source-gap"}]}, {})):
            with self.assertRaisesRegex(ValueError, "license release blockers remain"):
                licenses.require_release_ready()
        with patch.object(licenses, "checked_catalog", return_value=({"release_blockers": []}, {})):
            licenses.require_release_ready()


if __name__ == "__main__":
    unittest.main()
