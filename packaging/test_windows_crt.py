"""Fail-closed provenance checks; these fixtures make no legal assessment."""
import copy
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release
import windows_crt as crt


class WindowsCrtTests(unittest.TestCase):
    def test_only_loaded_archive_members_are_selected(self):
        path = r"C:\Program Files\Microsoft Visual Studio\VC\lib\libcmt.lib"
        log = (r"Searching C:\SDK\libucrt.lib:" + "\n Found _symbol\n"
               f" Loaded {path}(first.obj)\n Loaded {path}(second.obj)\n"
               f" Loaded {path}(first.obj)\n")
        self.assertEqual(crt.loaded_libraries(log), {path: ["first.obj", "second.obj"]})

    def test_missing_truncated_or_relative_selection_fails(self):
        for log in ("Searching C:\\VC\\libcmt.lib:\n Found _symbol", "",
                    "Loaded libcmt.lib(x.obj)", "Loaded C:\\VC\\libcmt.lib(",
                    "Loaded C:\\VC\\libcmt.lib", "Loaded C:\\other.lib(x.obj)"):
            with self.subTest(log=log), self.assertRaises(ValueError):
                crt.loaded_libraries(log)

    def test_command_file_path_cannot_inject_shell_code(self):
        self.assertEqual(crt.command_path(r"C:\Program Files\link.exe"), '"C:\\Program Files\\link.exe"')
        for char in '\r\n"%!^&|<>':
            with self.subTest(char=char), self.assertRaises(ValueError):
                crt.command_path("C:\\path" + char)

    def test_generated_archive_is_hashed_before_rustc_removes_it(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive, response = root / "temporary.lib", root / "link.rsp"
            archive.write_bytes(b"ephemeral import archive")
            response.write_bytes(b"link arguments")
            expected = crt.checksum(archive)
            real_checksum = crt.checksum
            def link(command, **kwargs):
                kwargs["stdout"].write(b"Loaded C:\\VC\\libcmt.lib(startup.obj)\n")
                return SimpleNamespace(returncode=0)
            def checksum(path):
                return real_checksum(archive if str(path) == r"C:\VC\libcmt.lib" else path)
            with patch.dict(crt.os.environ, {"CODECONVOY_CRT_EVIDENCE": str(root),
                                            "CODECONVOY_REAL_LINKER": r"C:\VC\link.exe"}), \
                 patch.object(crt.subprocess, "run", side_effect=link), \
                 patch.object(crt, "checksum", side_effect=checksum), \
                 patch.object(crt.sys, "stdout", SimpleNamespace(buffer=io.BytesIO())):
                self.assertEqual(crt.capture_link(["@" + str(response)]), 0)
            archive.unlink()  # rustc can remove this as soon as its linker exits.
            captured = json.loads((root / "selected-libraries.json").read_text())
            self.assertEqual(captured[0]["sha256"], expected)
            self.assertEqual((root / "response-0.rsp").read_bytes(), response.read_bytes())

    def fixture(self, directory):
        binary = directory / "CodeConvoy.exe"
        binary.write_bytes(b"MZfixture")
        toolchain = {"linker": {"path": r"C:\VC\link.exe", "sha256": "a" * 64, "version": "14.50 fixture"},
                     "visual_studio": {"installationVersion": "18 fixture"},
                     "environment": {k: "fixture" for k in
                                     ("VCTOOLSVERSION", "VCTOOLSINSTALLDIR", "WINDOWSSDKVERSION", "WINDOWSSDKDIR", "LIB")}}
        (directory / "link.log").write_text("Loaded C:\\VC\\libcmt.lib(startup.obj)\n", encoding="utf-8")
        (directory / "response-0.rsp").write_bytes(b"object.obj /OUT:CodeConvoy.exe")
        crt.write_json(directory / "invocation.json", {"exit_code": 0,
                       "command": [toolchain["linker"]["path"], "@C:\\link.rsp", "/VERBOSE:LIB"],
                       "response_files": [{"original_path": r"C:\link.rsp", "name": "response-0.rsp",
                                           "sha256": crt.checksum(directory / "response-0.rsp")}]})
        crt.write_json(directory / "toolchain.json", toolchain)
        report = {"schema": 1, "status": "captured", "license_assessment": "unresolved", "source_commit": "fixture",
                  "target": "x86_64-pc-windows-msvc", "binary_sha256": crt.checksum(binary),
                  "inputs": {name: crt.checksum(crt.ROOT / name) for name in crt.INPUTS},
                  "rustc": "rustc fixture", "cargo": "cargo fixture", "toolchain": toolchain,
                  "libraries": [{"path": r"C:\VC\libcmt.lib", "sha256": "b" * 64, "members": ["startup.obj"]}]}
        self.save_report(directory, report)
        return binary, report

    def save_report(self, directory, report):
        crt.write_json(directory / "selected-libraries.json", report["libraries"])
        report["evidence"] = {name: crt.checksum(directory / name)
                              for name in crt.CAPTURE_FILES}
        crt.write_json(directory / "provenance.json", report)

    def test_complete_capture_is_not_legal_approval(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            binary, _ = self.fixture(directory)
            self.assertEqual(crt.verify(binary, directory, source_commit="fixture")["license_assessment"], "unresolved")

    def test_missing_or_stale_evidence_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            binary, original = self.fixture(directory)
            for key, value in (("source_commit", "old"), ("binary_sha256", "0" * 64), ("inputs", {}),
                               ("libraries", []), ("toolchain", {}), ("rustc", ""),
                               ("license_assessment", "approved")):
                with self.subTest(key=key):
                    report = copy.deepcopy(original)
                    report[key] = value
                    self.save_report(directory, report)
                    with self.assertRaises(ValueError):
                        crt.verify(binary, directory, source_commit="fixture")
            self.save_report(directory, original)
            (directory / "link.log").write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "altered"):
                crt.verify(binary, directory, source_commit="fixture")
            (directory / "provenance.json").unlink()
            with self.assertRaises(FileNotFoundError):
                crt.verify(binary, directory, source_commit="fixture")

    def test_failed_link_or_missing_response_cannot_pass(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for mode in ("failed", "omitted", "altered", "missing member"):
                binary, report = self.fixture(directory)
                invocation = json.loads((directory / "invocation.json").read_text())
                if mode == "failed":
                    invocation["exit_code"] = 1
                elif mode == "omitted":
                    invocation["response_files"] = []
                elif mode == "altered":
                    (directory / "response-0.rsp").write_bytes(b"changed")
                else:
                    report["libraries"][0]["members"] = []
                crt.write_json(directory / "invocation.json", invocation)
                self.save_report(directory, report)
                with self.subTest(mode=mode), self.assertRaises(ValueError):
                    crt.verify(binary, directory, source_commit="fixture")

    def test_no_packaging_without_application_evidence(self):
        with patch.object(crt, "verify", side_effect=ValueError("missing provenance")), \
             patch.object(release, "fresh_work") as stage:
            with self.assertRaisesRegex(ValueError, "missing provenance"):
                release.windows()
            stage.assert_not_called()

    def test_diagnostic_uses_the_existing_release_toolchain_pins(self):
        catalog = json.loads((crt.ROOT / "third-party/catalog.json").read_text(encoding="utf-8"))
        workflow = (crt.ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn(f'rustup toolchain install {catalog["inputs"]["rust"]} --profile minimal', workflow)
        self.assertIn(f'cargo +{catalog["inputs"]["rust"]} install --locked --version '
                      f'{catalog["inputs"]["packager"]} cargo-packager', workflow)

    def test_final_package_hashes_and_helper_inspection_are_bound(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary, report = self.fixture(root)
            installer, archive, log = root / "setup.exe", root / "portable.zip", root / "pe-0.txt"
            installer.write_bytes(b"MZinstaller")
            archive.write_bytes(b"ZIPfixture")
            log.write_text("dumpbin fixture")
            crt.write_json(root / "packaged-binaries.json", {"application_sha256": crt.checksum(binary),
                           "license_assessment": "unresolved", "binaries": [{"name": "installer",
                           "sha256": crt.checksum(installer), "inspection": log.name,
                           "inspection_sha256": crt.checksum(log)}]})
            with patch.object(crt, "EVIDENCE", root), patch.object(crt, "verify", return_value=report):
                crt.record_packages(installer, archive, binary)
                manifest = json.loads((root / "packages.json").read_text())
                self.assertEqual(manifest["packages"], {p.name: crt.checksum(p) for p in (installer, archive)})
                installer.write_bytes(b"different installer")
                with self.assertRaisesRegex(ValueError, "differs"):
                    crt.record_packages(installer, archive, binary)


if __name__ == "__main__":
    unittest.main()
