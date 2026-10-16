"""Dynamic import policy, prerequisite bootstrap and no-bundling regressions."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import windows_crt as crt
import windows_runtime as runtime
import nsis_helpers
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

    def test_release_configurations_are_dynamic_and_shortcut_helper_is_crt_free(self):
        build = (runtime.ROOT / "packaging/build.py").read_text()
        helpers = (runtime.ROOT / "packaging/nsis_helpers.py").read_text()
        for text in (build, helpers):
            self.assertIn("target-feature=-crt-static", text)
            self.assertNotIn("target-feature=+crt-static", text)
        spec = json.loads((nsis_helpers.KIT / "inputs.json").read_text())
        self.assertEqual(spec["binaries"]["codeconvoy_utils.dll"]["crt"], "dynamic")
        self.assertEqual(spec["binaries"]["CodeConvoyAppId.dll"]["crt"], "none")
        self.assertIn('"/NOENTRY", "/NODEFAULTLIB"', helpers)

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

    def test_bootstrap_covers_all_call_sites_and_removal_requires_process_runtime(self):
        text = (runtime.ROOT / "packaging/nsis/installer.nsi").read_text()
        init = text.split("Function .onInit\n")[1].split("FunctionEnd")[0]
        self.assertLess(init.index("Call RequireWindowsRuntime"), init.index("MUI_LANGDLL_DISPLAY"))
        self.assertIn('Call RequireWindowsRuntime\n  codeconvoy_utils::SemverCompare', text)
        self.assertIn('Call ${PREFIX}RequireWindowsRuntime\n  codeconvoy_utils::FindProcess', text)
        self.assertIn('Call ${PREFIX}RequireWindowsRuntime\n        codeconvoy_utils::KillProcess', text)
        uninstall = text.split("Function un.onInit\n")[1].split("FunctionEnd")[0]
        self.assertIn('Call un.RequireWindowsRuntime', uninstall)
        self.assertIn('!insertmacro CheckIfAppIsRunning "un."', text)
        self.assertEqual(text.count('StrCpy $R0 "invalid"'), 3)
        version_page = text.split('Function PageReinstall\n')[1].split('FunctionEnd')[0]
        self.assertIn('SetErrorLevel 1603\n    Quit', version_page)
        install = text.split('Section Install\n')[1].split('SectionEnd')[0]
        self.assertLess(install.index('CheckIfAppIsRunning'), install.index('SetOutPath'))

    def test_rendered_check_uses_file_versions_architectures_and_no_runtime_override(self):
        text = runtime.render_prerequisites()
        self.assertNotIn("@CHECKS_", text)
        self.assertIn('GetDLLVersion "$CrtFile"', text)
        self.assertIn('IntCmpU', text)
        self.assertIn('LoadLibraryExW', text)
        self.assertIn('$WINDIR\\Sysnative', text)
        self.assertIn('$WINDIR\\SysWOW64', text)
        self.assertIn('SetErrorLevel 1603', text)
        self.assertIn('IfSilent no_dialog', text)
        self.assertIn('!insertmacro CodeConvoyRuntimeFunctions "un." 0', text)
        for disallowed in ('ReadReg', 'ReadEnvStr', 'ExecWait', 'Download::', 'nsis_tauri_utils::Download'):
            self.assertNotIn(disallowed, text)
        with tempfile.TemporaryDirectory() as temp, patch.object(nsis_helpers, "verify"):
            config = {"nsis": {}}
            nsis_helpers.configure(config, Path(temp))
            production = Path(config["nsis"]["template"]).read_text()
            self.assertNotIn('!define CODECONVOY_CRT_FIXTURE', production)
            self.assertNotIn('prerequisite-tests', production)


if __name__ == "__main__":
    unittest.main()
