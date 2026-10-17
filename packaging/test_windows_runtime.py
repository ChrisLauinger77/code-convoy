"""Dynamic import policy, prerequisite bootstrap and no-bundling regressions."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import windows_crt as crt
import windows_runtime as runtime
import release


class WindowsRuntime(unittest.TestCase):
    def test_dynamic_inputs_require_runtime_and_reject_static_implementations(self):
        valid = ["KERNEL32.dll", "VCRUNTIME140.dll", "api-ms-win-crt-runtime-l1-1-0.dll"]
        crt.check_dynamic_imports(valid)
        for imports in (valid[:1], valid[:-1], valid + ["MSVCP140.dll"], valid + ["MSVCR120.dll"]):
            with self.assertRaises(ValueError):
                crt.check_dynamic_imports(imports)
        crt.check_dynamic_libraries([r"C:\VC\msvcrt.lib", r"C:\VC\vcruntime.lib", r"C:\SDK\ucrt.lib"])
        for name in ("libcmt.lib", "libvcruntime.lib", "libucrt.lib", "libcpmt.lib"):
            with self.assertRaises(ValueError):
                crt.check_dynamic_libraries([r"C:\VC\msvcrt.lib", "C:\\VC\\" + name])

    def test_application_still_links_dynamic_crt(self):
        build = (runtime.ROOT / "packaging/build.py").read_text()
        self.assertIn("target-feature=-crt-static", build)
        self.assertNotIn("target-feature=+crt-static", build)
        self.assertEqual(set(runtime.policy()["runtime_files"]), {"x64"})

    def test_prohibited_runtime_files_are_rejected_case_insensitively_at_any_depth(self):
        for filename in ("VCRUNTIME140.dll", "vcruntime140_1.dll", "MSVCP140.dll", "msvcr120.dll",
                         "ucrtbase.dll", "api-ms-win-crt-runtime-l1-1-0.dll", "vc_redist.x64.exe", "vcredist_x86.exe"):
            for prefix in ("", "folder/", "$PLUGINSDIR\\"):
                with self.subTest(filename=filename, prefix=prefix), self.assertRaises(ValueError):
                    runtime.reject_bundled_runtimes([prefix + filename])
        runtime.reject_bundled_runtimes(["CodeConvoy.exe", "uninstall.exe", "$PLUGINSDIR/System.dll"])

    def test_portable_archive_rejects_runtime_bundling(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(release.licenses, "materials", return_value={}):
            root = Path(temp)
            exe = root / "CodeConvoy.exe"
            exe.write_bytes(b"fixture")
            archive = root / "portable.zip"
            with zipfile.ZipFile(archive, "w") as z:
                z.writestr("CodeConvoy.exe", exe.read_bytes())
                z.writestr("LICENSE.txt", (release.ROOT / "LICENSE").read_bytes())
                z.writestr("VCRUNTIME140.dll", b"prohibited")
            with self.assertRaisesRegex(ValueError, "redistribution is prohibited"):
                release.verify_windows_zip(archive, exe)

    def test_supported_config_preserves_per_user_identity_and_downgrades(self):
        config = json.loads((runtime.ROOT / "packaging/packager.json").read_text())
        self.assertEqual(config["identifier"], "io.github.chrislauinger77.code-convoy")
        self.assertEqual(config["nsis"]["installMode"], "currentUser")
        self.assertTrue(config["windows"]["allowDowngrades"])
        self.assertNotIn("appdataPaths", config["nsis"])

    def test_rendered_check_uses_file_versions_architectures_and_no_runtime_override(self):
        text = runtime.render_prerequisites()
        self.assertNotIn("@CHECKS_", text)
        self.assertIn('GetDLLVersion "$CrtFile"', text)
        self.assertIn('IntCmpU', text)
        self.assertIn('$WINDIR\\Sysnative', text)
        self.assertIn('SetErrorLevel 1603', text)
        self.assertIn('IfSilent no_dialog', text)
        for disallowed in ('ReadReg', 'ReadEnvStr', 'ExecWait', 'Download::', 'nsis_tauri_utils::Download'):
            self.assertNotIn(disallowed, text)
        with tempfile.TemporaryDirectory() as temp:
            config = {"nsis": {}}
            crt.configure_installer(config, Path(temp))
            production = Path(config["nsis"]["template"]).read_text()
            self.assertNotIn('!define CODECONVOY_CRT_FIXTURE', production)
            self.assertNotIn('prerequisite-tests', production)


if __name__ == "__main__":
    unittest.main()
