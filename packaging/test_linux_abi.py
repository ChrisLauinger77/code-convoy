"""Final-package ABI gates, including malformed GNU readelf output."""
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import linux
import linux_abi as abi


def needs(*names):
    return ("Version needs section '.gnu.version_r' contains 1 entry:\n"
            " Addr: 0x100 Offset: 0x100 Link: 5 (.dynstr)\n"
            f" 000000: Version: 1 File: libc.so.6 Cnt: {len(names)}\n"
            + "".join(f" 0x{16*(i+1):04x}: Name: {name} Flags: none Version: {i+2}\n"
                      for i, name in enumerate(names)))


def elf(root, name):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"\x7fELF fixture")
    return path


def readelf_result(args, version):
    if args[1] == "--dynamic":
        output = "Dynamic section at offset 0x80 contains 3 entries:\n"
        if version:
            output += " 0x6ffffffe (VERNEED) 0x100\n 0x6fffffff (VERNEEDNUM) 1\n"
        output += " 0x0 (NULL) 0x0\n"
    else:
        output = needs("GLIBC_" + version) if version else "No version information found in this file."
    return subprocess.CompletedProcess(args, 0, output, "")


class LinuxAbi(unittest.TestCase):
    def test_glibc_reads_numeric_requirements_not_exported_definitions(self):
        definitions = ("Version definition section '.gnu.version_d' contains 1 entry:\n"
                       " Addr: 0x80 Offset: 0x80 Link: 5 (.dynstr)\n"
                       " 000000: Rev: 1 Flags: none Index: 2 Cnt: 1 Name: GLIBC_2.99\n")
        self.assertEqual(abi.required_glibc(definitions + needs(
            "GLIBC_2.9", "GLIBC_2.43", "GLIBC_2.3.4", "GCC_3.0", "GLIBCXX_3.4.30")), (2, 43))
        self.assertIsNone(abi.required_glibc(definitions))
        self.assertIsNone(abi.required_glibc("No version information found in this file.\n"))

    def test_unknown_private_and_malformed_glibc_names_fail(self):
        for name in ("GLIBC_PRIVATE", "GLIBC_ABI_DT_RELR", "GLIBC_", "GLIBC", "GLIBC_2", "GLIBC_2.x"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Unrecognized glibc ABI"):
                abi.required_glibc(needs(name))

    def test_malformed_or_incomplete_version_information_fails(self):
        valid = needs("GLIBC_2.39", "GLIBC_2.40")
        for output in ("", "unrecognized output", "Version needs section", "No version information found",
                       "Version symbols section '.gnu.version' contains 1 entry:\n",
                       valid.replace("contains 1 entry", "contains 2 entries"),
                       valid.replace("Cnt: 2", "Cnt: 3"), valid.replace("Cnt: 2", "Cnt: 1"),
                       valid[:valid.index(" 0x0020:")], valid.replace(" Name: GLIBC_2.40", " Name:"),
                       valid.replace(" Version: 3", " Version:"),
                       valid.replace("Version: 1 File:", "Version: 2 File:"),
                       valid.replace(" Addr: 0x100 Offset: 0x100 Link: 5 (.dynstr)\n", ""),
                       valid + valid):
            with self.subTest(output=output), self.assertRaises(ValueError):
                abi.required_glibc(output)

    def test_readelf_diagnostics_and_command_errors_fail(self):
        for stdout, stderr in ((needs("GLIBC_2.39"), "readelf: Warning: truncated table"), ("", "")):
            with patch.object(abi, "run", return_value=subprocess.CompletedProcess([], 0, stdout, stderr)) as run:
                with self.assertRaises(ValueError):
                    abi.inspect_glibc(Path("fixture"))
                self.assertEqual(run.call_args.kwargs["env"]["LC_ALL"], "C")
        with patch.object(abi, "run", side_effect=subprocess.CalledProcessError(1, ["readelf"])):
            with self.assertRaises(subprocess.CalledProcessError):
                abi.inspect_glibc(Path("fixture"))

    def test_missing_version_section_cannot_hide_dynamic_requirements(self):
        for output in ("No version information found in this file.", needs("GLIBC_2.39")):
            for dynamic in ("", "Dynamic section at offset 0x80 contains 1 entry:\n 0x0 (VERNEEDNUM) 2\n"):
                responses = [subprocess.CompletedProcess([], 0, output, ""),
                             subprocess.CompletedProcess([], 0, dynamic, "")]
                with self.subTest(output=output, dynamic=dynamic), patch.object(abi, "run", side_effect=responses):
                    with self.assertRaises(ValueError):
                        abi.inspect_glibc(Path("fixture"))

    def test_final_deb_accepts_239_rejects_240_and_243(self):
        for version in ("2.39", "2.40", "2.43"):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary).resolve()
                artifact = work / "final.deb"
                artifact.write_bytes(b"packaged bytes")

                def run(*args, **kwargs):
                    if args[0] == "dpkg-deb":
                        self.assertEqual(args[2], artifact)
                        elf(args[3], "usr/bin/codeconvoy")
                        return subprocess.CompletedProcess(args, 0)
                    self.assertEqual(args[0], "readelf")
                    return readelf_result(args, version)

                with patch.object(abi, "run", side_effect=run):
                    if version == "2.39":
                        report = abi.inspect_artifact(artifact, work)
                        self.assertIn("maximum required GLIBC 2.39", report)
                        self.assertIn(abi.checksum(artifact), report)
                    else:
                        with self.assertRaisesRegex(ValueError, "exceeds GLIBC 2.39") as rejected:
                            abi.inspect_artifact(artifact, work)
                        self.assertIn("ubuntu-24.04", str(rejected.exception))
                        self.assertIn("do not raise MAX_GLIBC", str(rejected.exception))

    @unittest.skipIf(sys.platform == "win32", "Linux payload symlinks require native symlink support")
    def test_payload_symlinks_are_resolved_or_rejected_with_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            root = work / "payload"
            main = elf(root, "usr/bin/codeconvoy")
            alias = root / "alias"
            outside = elf(work, "outside")
            with patch.object(abi, "inspect_glibc", return_value=(2, 39)) as inspect:
                alias.symlink_to(main.relative_to(root))
                self.assertEqual(abi.inspect_payload(root), [("usr/bin/codeconvoy", (2, 39))])
                inspect.assert_called_once_with(main)
                for target, message in (("missing", "Cannot resolve package symlink"),
                                        ("alias", "Cannot resolve package symlink"),
                                        (outside, "Package symlink escapes its payload")):
                    alias.unlink()
                    alias.symlink_to(target)
                    with self.subTest(target=target), self.assertRaisesRegex(ValueError, message) as rejected:
                        abi.inspect_payload(root)
                    self.assertIn(str(alias), str(rejected.exception))

    def test_every_binary_and_dependency_contributes_to_final_maximum(self):
        for high_name in ("usr/bin/helper", "usr/lib/libdependency.so"):
            with self.subTest(high_name=high_name), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary).resolve()
                artifact = work / "final.deb"
                artifact.write_bytes(b"final bytes")

                def run(*args, **kwargs):
                    if args[0] == "dpkg-deb":
                        for name in ("usr/bin/codeconvoy", "usr/bin/helper", "usr/lib/libdependency.so"):
                            elf(args[3], name)
                        return subprocess.CompletedProcess(args, 0)
                    version = "2.40" if args[-1].as_posix().endswith(high_name) else "2.34"
                    return readelf_result(args, version)

                with patch.object(abi, "run", side_effect=run), self.assertRaisesRegex(ValueError, high_name):
                    abi.inspect_artifact(artifact, work)

    def test_final_rpm_extraction_and_failure_propagate(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            artifact = work / "final.rpm"
            artifact.write_bytes(b"final RPM")

            def run(*args, **kwargs):
                if args[0] == "rpm2cpio":
                    self.assertEqual(args[1], artifact)
                    kwargs["stdout"].write(b"final RPM payload")
                elif args[0] == "cpio":
                    self.assertEqual(kwargs["stdin"].read(), b"final RPM payload")
                    elf(args[-1], "usr/bin/codeconvoy")
                else:
                    return readelf_result(args, "2.39")
                return subprocess.CompletedProcess(args, 0)

            with patch.object(abi, "run", side_effect=run):
                self.assertIn("GLIBC 2.39", abi.inspect_artifact(artifact, work))
            with patch.object(abi, "run", side_effect=subprocess.CalledProcessError(1, ["rpm2cpio"])):
                with self.assertRaises(subprocess.CalledProcessError):
                    abi.verify_artifacts([artifact], work)
            self.assertFalse((work / "glibc-requirements.txt").exists())

    def test_appimage_runtime_and_final_payload_are_both_inspected(self):
        for runtime, payload in ((None, "2.39"), (None, "2.43"), ("2.40", "2.39")):
            with self.subTest(runtime=runtime, payload=payload), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary).resolve()
                artifact = elf(work, "final.AppImage")
                inspected = []

                def run(*args, **kwargs):
                    if args[0] == "readelf":
                        if args[1] == "--version-info":
                            inspected.append(args[-1])
                        version = runtime if args[-1] == artifact else payload
                        return readelf_result(args, version)
                    if args[0] == artifact:
                        self.assertEqual(args[1], "--appimage-offset")
                        return subprocess.CompletedProcess(args, 0, "944632\n", "")
                    self.assertEqual(args[0], "unsquashfs")
                    self.assertEqual(args[-1], artifact)
                    self.assertEqual(args[-2], "944632")
                    elf(args[3], "usr/bin/codeconvoy")
                    return subprocess.CompletedProcess(args, 0)

                with patch.object(abi, "run", side_effect=run):
                    if runtime or payload != "2.39":
                        with self.assertRaisesRegex(ValueError, "GLIBC"):
                            abi.inspect_artifact(artifact, work)
                    else:
                        self.assertIn("AppImage runtime", abi.inspect_artifact(artifact, work))
                        self.assertEqual(len(inspected), 2)
                self.assertEqual(inspected[0], artifact)

    def test_missing_final_executable_and_bad_extraction_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            artifact = elf(work, "final.AppImage")
            with patch.object(abi, "inspect_glibc", return_value=None), patch.object(
                    abi, "run", return_value=subprocess.CompletedProcess([], 0, "not an offset", "")):
                with self.assertRaisesRegex(ValueError, "offset"):
                    abi.inspect_artifact(artifact, work)
            with self.assertRaisesRegex(ValueError, "missing its ELF"):
                abi.inspect_payload(work)

    def test_package_assembly_propagates_final_gate_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            output = work / "packages"
            output.mkdir()
            (output / "built.deb").write_bytes(b"deb")
            rpm = work / "rpm/RPMS/x86_64"
            rpm.mkdir(parents=True)
            (rpm / "built.rpm").write_bytes(b"rpm")
            appdir = work / "CodeConvoy.AppDir"
            appdir.mkdir()
            (work / f"CodeConvoy-{linux.version()}-x86_64.AppImage").write_bytes(b"image")

            def run(*args, **kwargs):
                output = ""
                if args[0:2] == ("dpkg-deb", "--field"):
                    output = ("libc6 (>= 2.39)" if args[-1] == "Depends" else
                              f"Package: code-convoy\nVersion: {linux.version()}\nArchitecture: amd64\n")
                if args[0:2] == ("rpm", "-qp"):
                    output = f"code-convoy {linux.version()} 1 x86_64"
                return subprocess.CompletedProcess(args, 0, output, "")

            with patch.object(linux, "fresh_work", return_value=work), patch.object(linux, "DIST", work), \
                 patch.object(linux, "inspect_glibc", return_value=(2, 39)), \
                 patch.object(linux, "package", return_value=output), patch.object(linux, "run", side_effect=run), \
                 patch.object(linux, "verify_linux_payload"), patch.object(linux, "download_tools", return_value={
                     "linuxdeploy": "linuxdeploy", "appimagetool": "appimagetool", "runtime": "runtime"}), \
                 patch.object(linux.licenses, "verify_appimage_runtime"), \
                 patch.object(linux, "verify_artifacts", side_effect=ValueError("final ELF rejected")) as gate:
                with self.assertRaisesRegex(ValueError, "final ELF rejected"):
                    linux.package_linux()
                self.assertEqual([p.suffix for p in gate.call_args.args[0]], [".deb", ".rpm", ".AppImage"])

    @unittest.skipUnless(sys.platform == "linux" and all(shutil.which(tool) for tool in ("cc", "readelf", "dpkg-deb")),
                         "requires Linux ELF and Debian packaging tools")
    def test_real_final_deb_and_versioned_elf_dependency(self):
        # Build a tiny ELF fixture with a controlled imported symbol version.
        # No host glibc version, network, application build or execution needed.
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            root = work / "root"
            (root / "DEBIAN").mkdir(parents=True)
            (root / "DEBIAN/control").write_text("Package: fixture\nVersion: 1\nArchitecture: amd64\nMaintainer: Test <test@example.invalid>\nDescription: ABI fixture\n")
            (root / "usr/bin").mkdir(parents=True)
            (work / "dep.c").write_text("int dependency(void) { return 0; }\n")
            (work / "main.c").write_text("extern int dependency(void); int probe(void) { return dependency(); }\n")
            for version in ("2.39", "2.40", "2.43"):
                (work / "versions").write_text(f"GLIBC_{version} {{ global: dependency; }};\n")
                subprocess.run(["cc", "-shared", "-nostdlib", "-fPIC", str(work / "dep.c"),
                                "-Wl,--version-script=" + str(work / "versions"), "-o", str(work / "libdep.so")], check=True)
                subprocess.run(["cc", "-shared", "-nostdlib", "-fPIC", str(work / "main.c"),
                                "-L" + str(work), "-ldep", "-o", str(root / "usr/bin/codeconvoy")], check=True)
                artifact = work / "final.deb"
                subprocess.run(["dpkg-deb", "--build", "--root-owner-group", str(root), str(artifact)], check=True, stdout=subprocess.DEVNULL)
                # The stage is deliberately changed after packaging: only the
                # final archive may decide whether the candidate is accepted.
                (root / "usr/bin/codeconvoy").write_bytes(b"not the packaged binary")
                if version == "2.39":
                    self.assertIn("GLIBC 2.39", abi.inspect_artifact(artifact, work))
                else:
                    with self.assertRaisesRegex(ValueError, "exceeds GLIBC 2.39"):
                        abi.inspect_artifact(artifact, work)
            # Remove section headers from the real dependency fixture while
            # leaving the loader's dynamic table intact. readelf -V alone says
            # "No version information"; that must fail instead of lowering ABI.
            dependency = work / "libdep.so"
            # Use the imported-version binary from the final DEB, not the
            # dependency's exported definitions.
            extracted = work / "extracted"
            subprocess.run(["dpkg-deb", "--extract", str(artifact), str(extracted)], check=True)
            data = bytearray((extracted / "usr/bin/codeconvoy").read_bytes())
            struct.pack_into("<Q", data, 40, 0)  # ELF64 e_shoff
            struct.pack_into("<HHH", data, 58, 0, 0, 0)
            dependency.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "version needs information"):
                abi.inspect_glibc(dependency)


if __name__ == "__main__":
    unittest.main()
