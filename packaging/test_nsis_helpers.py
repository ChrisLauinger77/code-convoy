"""NSIS helper identity, dependency and provenance regression tests."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import nsis_helpers as helpers
import windows_crt as crt


class NsisHelpers(unittest.TestCase):
    def fixture(self, root):
        spec = json.loads((helpers.KIT / "inputs.json").read_text())
        report = {"schema": 1, "spec": spec, "inputs": helpers.inputs(),
                  "packages": helpers.locked_packages(), "rustc": f"rustc {spec['rust']} (fixture)",
                  "license_assessment": "windows-crt-unresolved", "binaries": {}}
        (root / "plugins").mkdir()
        for name, expected in spec["binaries"].items():
            binary = b"MZfixture" + name.encode()
            (root / "plugins" / name).write_bytes(binary)
            record = {"sha256": helpers.crt.checksum(root / "plugins" / name), "evidence": {},
                      "imports": (["KERNEL32.dll", "VCRUNTIME140.dll", "api-ms-win-crt-runtime-l1-1-0.dll", "api-ms-win-crt-string-l1-1-0.dll"]
                                  if expected["crt"] == "dynamic" else ["KERNEL32.dll"]), "crt": expected["crt"]}
            for build in ("first", "second"):
                directory = root / build / name
                directory.mkdir(parents=True)
                (directory / name).write_bytes(binary)
                library = r"C:\VC\msvcrt.lib" if expected["crt"] == "dynamic" else r"C:\SDK\ole32.lib"
                (directory / "link.log").write_text(f"Loaded {library}(fixture.obj)\n")
                (directory / "link.map").write_text("retained symbols")
                crt.write_json(directory / "invocation.json", {"exit_code": 0, "command": [r"C:\VC\link.exe", "/VERBOSE", "/VERBOSE:LIB", "/Brepro"]})
                crt.write_json(directory / "selected-libraries.json", [{"path": library, "members": ["fixture.obj"], "sha256": "a" * 64}])
                crt.write_json(directory / "build-command.json", ["fixture-build"])
                crt.write_json(directory / "toolchain.json", {"linker": {"path": r"C:\VC\link.exe", "sha256": "c" * 64, "version": "fixture"},
                               "compiler": {"sha256": "b" * 64}, "environment": {
                                   "VCTOOLSVERSION": spec["msvc"], "WINDOWSSDKVERSION": spec["windows_sdk"]}})
                crt.write_json(directory / "rustflags.json", ["--cfg", "windows_raw_dylib", "-C", "target-feature=-crt-static"])
                (directory / "pe.txt").write_text("14C machine (x86)\n" + "".join(" " + name + "\n" for name in record["imports"]) + "".join(
                    f" {n} {n:X} 00001000 {symbol}\n" for n, symbol in enumerate(expected["exports"], 1)))
                record["evidence"][build] = {p.name: crt.checksum(p) for p in directory.iterdir() if p.name != name}
            report["binaries"][name] = record
        crt.write_json(root / "provenance.json", report)
        return report

    def test_two_builds_staged_binary_and_all_evidence_are_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            helpers.verify(root)
            for relative in ("plugins/codeconvoy_utils.dll", "first/codeconvoy_utils.dll/codeconvoy_utils.dll",
                             "second/CodeConvoyAppId.dll/CodeConvoyAppId.dll", "first/CodeConvoyAppId.dll/link.log",
                             "second/codeconvoy_utils.dll/selected-libraries.json", "first/codeconvoy_utils.dll/rustflags.json"):
                path = root / relative
                original = path.read_bytes()
                path.write_bytes(original + b"changed")
                with self.subTest(path=relative), self.assertRaises(ValueError):
                    helpers.verify(root)
                path.write_bytes(original)
            (root / "plugins/ApplicationID.dll").write_bytes(b"MZold cached plugin")
            with self.assertRaisesRegex(ValueError, "Unexpected staged"):
                helpers.verify(root)

    def test_missing_dependency_source_or_linker_capture_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = self.fixture(root)
            changed = []
            report = copy.deepcopy(original); report["packages"].pop(); changed.append(report)
            report = copy.deepcopy(original); report["inputs"].popitem(); changed.append(report)
            report = copy.deepcopy(original); report["spec"]["upstream"]["revision"] = "0" * 40; changed.append(report)
            report = copy.deepcopy(original); report["binaries"]["CodeConvoyAppId.dll"]["evidence"]["first"].pop("link.log"); changed.append(report)
            report = copy.deepcopy(original); report["binaries"]["codeconvoy_utils.dll"]["crt"] = "none"; changed.append(report)
            report = copy.deepcopy(original); report["binaries"]["codeconvoy_utils.dll"]["imports"] = []; changed.append(report)
            for report in changed:
                crt.write_json(root / "provenance.json", report)
                with self.assertRaises(ValueError):
                    helpers.verify(root)

    def test_unexpected_export_architecture_or_dynamic_crt_fails(self):
        expected = {"exports": ["Set"], "crt": "none"}
        valid = "14C machine (x86)\n 1 1 00001000 Set\n KERNEL32.dll\n"
        self.assertEqual(helpers.check_identity(valid, expected), ["KERNEL32.dll"])
        for text in (valid.replace("14C", "8664"), valid.replace(" Set", " Download"), valid + " VCRUNTIME140.dll\n"):
            with self.assertRaises(ValueError):
                helpers.check_identity(text, expected)

    def test_custom_template_cannot_use_packager_cached_plugins(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(helpers, "verify"):
            root = Path(temporary)
            config = {"nsis": {"installMode": "currentUser"}}
            helpers.configure(config, root)
            template = Path(config["nsis"]["template"]).read_text()
            self.assertIn('!addplugindir /x86-unicode "', template)
            self.assertIn("CodeConvoyAppId::Set", template)
            self.assertIn("codeconvoy_utils::SemverCompare", template)
            self.assertNotIn("ApplicationID::", template)
            self.assertNotIn("nsis_tauri_utils::", template)
            self.assertEqual(config["nsis"]["installMode"], "currentUser")

    def test_upstream_runtime_modules_and_exact_stack_safety_patch(self):
        import difflib
        import tarfile
        spec = json.loads((helpers.KIT / "inputs.json").read_text())
        with tarfile.open(helpers.ROOT / "third-party/sources/nsis-tauri-utils-0.2.1.tar.gz") as upstream:
            for name in ("nsis-process", "nsis-semvercompare", "pluginapi"):
                relative = f"crates/{name}/src/lib.rs"
                original = upstream.extractfile("nsis-tauri-utils-nsis_tauri_utils-v0.2.1/" + relative).read()
                current = (helpers.KIT / relative).read_bytes()
                if name != "pluginapi":
                    self.assertEqual(current, original)
                else:
                    patch = spec["upstream"]["patches"][0]
                    self.assertEqual(patch["file"], relative)
                    self.assertEqual(hashlib.sha256(original).hexdigest(), patch["before_sha256"])
                    self.assertEqual(hashlib.sha256(current).hexdigest(), patch["after_sha256"])
                    expected = "".join(difflib.unified_diff(original.decode().splitlines(True),
                        current.decode().splitlines(True), fromfile="a/" + relative, tofile="b/" + relative))
                    self.assertEqual((helpers.KIT / patch["path"]).read_text(), expected)

    def test_template_preserves_every_other_upstream_instruction(self):
        directory = helpers.ROOT / "packaging/nsis"
        text = (directory / "installer.nsi").read_text()
        header = '; Derived from cargo-packager 0.11.8; see UPSTREAM.json and LICENSE files.\n!addplugindir /x86-unicode "@CODECONVOY_HELPERS@"\n'
        self.assertTrue(text.startswith(header))
        text = text.removeprefix(header)
        text = text.replace('!include "@CODECONVOY_PREREQUISITES@"\n', '')
        for indent in ('  ', '        '):
            text = text.replace(indent + 'ClearErrors\n' + indent + 'Pop $R0\n' + indent + '${If} ${Errors}\n' +
                                indent + '  StrCpy $R0 "invalid"\n' + indent + '${EndIf}\n', indent + 'Pop $R0\n')
        text = text.replace('          SetErrorLevel 1603\n          IfSilent silent ui\n', '          IfSilent silent ui\n')
        text = text.replace('Section Install\n  !insertmacro CheckIfAppIsRunning ""\n\n  SetOutPath $INSTDIR\n',
                            'Section Install\n  SetOutPath $INSTDIR\n\n  !insertmacro CheckIfAppIsRunning ""\n')
        text = text.replace('Function un.onInit\n  ${GetOptions} $CMDLINE "/P" $PassiveMode\n  IfErrors +2 0\n    StrCpy $PassiveMode 1\n  Call un.RequireWindowsRuntime\n', 'Function un.onInit\n')
        text = text.replace('\n  Call RequireWindowsRuntime\n\n', '\n')
        text = text.replace('  Call RequireWindowsRuntime\n', '')
        text = text.replace('        Call ${PREFIX}RequireWindowsRuntime\n', '')
        text = text.replace('  Call ${PREFIX}RequireWindowsRuntime\n', '')
        text = text.replace('!macro CheckIfAppIsRunning PREFIX', '!macro CheckIfAppIsRunning')
        for suffix in (' ""', ' "un."'):
            text = text.replace('!insertmacro CheckIfAppIsRunning' + suffix, '!insertmacro CheckIfAppIsRunning')
        text = text.replace('    ; Abort in a custom page would skip the page and continue installation.\n' +
            '    ${IfNot} ${Silent}\n    ${AndIf} $PassiveMode != 1\n' +
            '      MessageBox MB_OK|MB_ICONSTOP "Could not validate the installed version. Repair the x86 Visual C++ Runtime and retry."\n' +
            '    ${EndIf}\n    SetErrorLevel 1603\n    Quit\n',
            '    SetErrorLevel 1603\n    Abort "Could not validate the installed version. Repair the x86 Visual C++ Runtime and retry."\n')
        for message, extra in [('validate the installed version', '  ${AndIf} $R0 != "-1"\n'), ('check running processes', '')]:
            text = text.replace('  ${If} $R0 != "0"\n  ${AndIf} $R0 != "1"\n' + extra +
                '    SetErrorLevel 1603\n    Abort "Could not ' + message + '. Repair the x86 Visual C++ Runtime and retry."\n  ${EndIf}\n', '')
        text = text.replace('${If} $R0 == "0"', '${If} $R0 = 0')
        status = '  Pop $R0\n  ${If} $R0 != 0\n    Abort "Could not assign application shortcut identity."\n  ${EndIf}\n'
        self.assertEqual(text.count(status), 2)
        text = text.replace(status, "").replace("CodeConvoyAppId::Set", "ApplicationID::Set").replace("codeconvoy_utils::", "nsis_tauri_utils::")
        upstream = json.loads((directory / "UPSTREAM.json").read_text())
        self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), upstream["upstream_sha256"])

    def test_nested_source_archive_is_portable_deterministic_and_rejects_escapes(self):
        entries = [("third-party/nsis/a", b"a"), ("packaging/b", b"b")]
        self.assertEqual(helpers.source_archive(entries), helpers.source_archive(list(reversed(entries))))
        for name in ("../outside", "/absolute", "a/../b", "C:/outside", "a\\b"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                helpers.source_archive([(name, b"data")])


if __name__ == "__main__":
    unittest.main()
