"""Build and verify the two narrowly scoped, source-owned NSIS helpers."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import zipfile

import windows_crt as crt

ROOT = Path(__file__).resolve().parent.parent
KIT = ROOT / "third-party/nsis"
OUTPUT = ROOT / "target/packaging/nsis-helpers"
OLD_PLUGINS = {"$PLUGINSDIR/ApplicationID.dll", "$PLUGINSDIR/nsis_tauri_utils.dll"}


def inputs(root=ROOT):
    paths = [p for directory in ("third-party/nsis", "packaging/nsis")
             for p in (root / directory).rglob("*") if p.is_file()]
    paths += [root / p for p in ("packaging/nsis_helpers.py", "packaging/windows_crt.py")]
    return {p.relative_to(root).as_posix(): crt.checksum(p) for p in sorted(paths)}


def locked_packages(kit=KIT):
    source_patch = json.loads((kit / "inputs.json").read_text())["source_patch"]
    packages = []
    for package in tomllib.loads((kit / "Cargo.lock").read_text())["package"]:
        if package["name"] == source_patch["name"]:
            if package["version"] != source_patch["version"] or package.get("source"):
                raise ValueError("Missing locked NSIS source patch")
            packages.append({key: source_patch[key] for key in ("name", "version", "checksum")})
        elif package.get("source"):
            packages.append({key: package[key] for key in ("name", "version", "checksum")})
    return packages


def prepare_source():
    """Apply the single audited stable-Rust compatibility patch to fresh source."""
    spec = json.loads((KIT / "inputs.json").read_text())["source_patch"]
    archive = ROOT / "third-party" / spec["archive"]
    if crt.checksum(archive) != spec["checksum"]:
        raise ValueError("Modified NSIS dependency archive")
    destination = ROOT / "target/packaging/nsis-source"
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    with tarfile.open(archive) as source:
        source.extractall(destination, filter="data")
    original = destination / f"{spec['name']}-{spec['version']}"
    path = original / spec["file"]
    if crt.checksum(path) != spec["before_sha256"]:
        raise ValueError("NSIS source patch preimage changed")
    data = path.read_bytes().replace(b'#![cfg_attr(windows_raw_dylib, feature(raw_dylib))]\n',
        b'// CodeConvoy 2026-10-16: raw_dylib is stable since Rust 1.71; remove obsolete feature gate.\n')
    path.write_bytes(data)
    if crt.checksum(path) != spec["after_sha256"]:
        raise ValueError("NSIS source patch result changed")
    original.rename(destination / spec["name"])


def source_entries(root=ROOT):
    return [(name, (root / name).read_bytes()) for name in inputs(root)]


def source_archive(entries):
    """Deterministic nested source kit; the existing outer bundle stays flat."""
    output = io.BytesIO()
    seen = set()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, data in sorted(entries):
            if (name in seen or name.startswith("/") or any(c in name for c in "\\:\x00")
                    or any(p in ("", ".", "..") for p in name.split("/"))):
                raise ValueError("Invalid NSIS source/evidence archive path")
            seen.add(name)
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    return output.getvalue()


def verify_catalog(catalog, texts, root=ROOT, evidence=None):
    """Source/notice gates also run offline, on platforms that cannot build PE."""
    evidence = evidence or root / "third-party"
    record = catalog["native_provenance"].get("nsis_replacements")
    if not record:
        raise ValueError("Missing reviewed NSIS replacement inventory")
    kit = evidence / "nsis"
    spec = json.loads((kit / "inputs.json").read_text())
    if record["inputs"] != inputs(root) or record["spec"] != spec:
        raise ValueError("NSIS helper sources/build inputs changed: review provenance")
    archive = next((s for s in catalog["sources"] if s["path"] == record["source_archive"]), None)
    if archive is None or (evidence / archive["path"]).read_bytes() != source_archive(source_entries(root)):
        raise ValueError("Incomplete NSIS helper source/build kit")
    packages = json.loads((kit / "dependencies.json").read_text())
    identities = lambda ps: [(p["name"], p["version"], p["checksum"]) for p in ps]
    if identities(packages) != identities(locked_packages(kit)) or record["dependencies"] != packages:
        raise ValueError("Incomplete locked NSIS dependency inventory")
    for package in packages:
        if not package["included"]:
            if not package["name"].startswith("windows_"):
                raise ValueError("Unexpected NSIS dependency exclusion")
            continue
        if not package["notices"] or any(n["text"] not in texts for n in package["notices"]):
            raise ValueError("Missing NSIS dependency notices")
        if not any(s["path"] == package["source"] and s["sha256"] == package["checksum"] for s in catalog["sources"]):
            raise ValueError("Missing locked NSIS dependency source")
        component = next((c for c in catalog["components"] if c["name"] == "CodeConvoy NSIS helper dependency: " + package["name"]), None)
        if (component is None or component["version"] != package["version"] or component.get("checksum") != package["checksum"]
                or component["notices"] != package["notices"]):
            raise ValueError("Missing NSIS dependency inventory/notice coverage")
    if set(record["binaries"]) != set(spec["binaries"]):
        raise ValueError("Missing NSIS replacement binary inventory")
    for name, checksum in record["binaries"].items():
        if catalog["windows_installer_plugins"].get("$PLUGINSDIR/" + name) != checksum:
            raise ValueError("NSIS replacement provenance does not match pinned installer DLL")
    if OLD_PLUGINS & catalog["windows_installer_plugins"].keys():
        raise ValueError("Old NSIS prebuilt plugin restored")
    retained = {entry["path"] for entry in catalog["files"]}
    if record["audit"] not in retained or record["evidence"] not in retained:
        raise ValueError("Missing retained NSIS build evidence")
    audit = json.loads((evidence / record["audit"]).read_text())
    if (audit["spec"] != spec or {n: p["sha256"] for n, p in audit["binaries"].items()} != record["binaries"]
            or any(audit["inputs"].get(name) != checksum for name, checksum in record["inputs"].items()
                   if name.startswith("third-party/nsis/"))):
        raise ValueError("NSIS retained build describes different source/binary inputs")
    with zipfile.ZipFile(evidence / record["evidence"]) as archive:
        if archive.read("provenance.json") != (evidence / record["audit"]).read_bytes():
            raise ValueError("NSIS retained build evidence does not match its index")
        for name, binary in audit["binaries"].items():
            for build in ("first", "second"):
                for filename, checksum in binary["evidence"][build].items():
                    if hashlib.sha256(archive.read(f"{build}/{name}/{filename}")).hexdigest() != checksum:
                        raise ValueError("NSIS retained linker evidence changed")


def pe_exports(text):
    return sorted(re.findall(r"^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\w+)\s*$", text, re.M))


def check_identity(text, expected):
    if not re.search(r"\b14C machine \(x86\)", text, re.I) or pe_exports(text) != expected["exports"]:
        raise ValueError("Unexpected NSIS helper architecture or exports")
    imports = sorted(set(re.findall(r"^\s+([\w.-]+\.dll)\s*$", text, re.I | re.M)))
    if any(re.search(r"(?:msvcp|msvcr|vcruntime|ucrtbase|api-ms-win-crt)", name, re.I) for name in imports):
        raise ValueError("Unexpected dynamic CRT dependency in NSIS helper")
    return imports


def check_link(directory, expected):
    invocation = json.loads((directory / "invocation.json").read_text())
    toolchain = json.loads((directory / "toolchain.json").read_text())
    if (invocation["exit_code"] != 0 or not invocation["command"]
            or invocation["command"][0] != toolchain["linker"]["path"]
            or not all(flag in invocation["command"] for flag in ("/VERBOSE", "/VERBOSE:LIB", "/Brepro"))):
        raise ValueError("Missing successful deterministic helper linker invocation")
    selected = crt.loaded_libraries((directory / "link.log").read_text(encoding="utf-8", errors="replace"),
                                    require_crt=expected["crt"] == "static")
    libraries = json.loads((directory / "selected-libraries.json").read_text())
    if ({p["path"]: p["members"] for p in libraries} != selected or len(libraries) != len(selected)
            or any(not re.fullmatch(r"[0-9a-f]{64}", p["sha256"]) for p in libraries)):
        raise ValueError("Helper linker inventory differs from raw selection evidence")
    if expected["crt"] == "none" and any(PureWindowsPath(p).name.lower() in crt.CRT_NAMES for p in selected):
        raise ValueError("Shortcut helper unexpectedly includes CRT code")
    if expected["crt"] == "none" and any(PureWindowsPath(p).name.lower() not in {"kernel32.lib", "ole32.lib", "uuid.lib"} for p in selected):
        raise ValueError("Unexpected shortcut helper native input")
    if any(PureWindowsPath(p).name.lower().startswith("windows.0.") for p in selected):
        raise ValueError("Unexpected prebuilt Windows import archive")
    for response in invocation.get("response_files", []):
        if not re.fullmatch(r"response-\d+\.rsp", response["name"]) or crt.checksum(directory / response["name"]) != response["sha256"]:
            raise ValueError("Missing helper linker response evidence")
    if [r["original_path"] for r in invocation.get("response_files", [])] != [a[1:] for a in invocation["command"] if a.startswith("@")]:
        raise ValueError("Incomplete helper response evidence")
    return libraries


def verify(directory=OUTPUT):
    spec = json.loads((KIT / "inputs.json").read_text())
    report = json.loads((directory / "provenance.json").read_text())
    if (report.get("schema") != 1 or report.get("inputs") != inputs()
            or report.get("spec") != spec or report.get("packages") != locked_packages()
            or not report.get("rustc", "").startswith(f"rustc {spec['rust']} ")
            or report.get("license_assessment") != "windows-crt-unresolved"):
        raise ValueError("Missing, stale or mismatched NSIS helper provenance")
    if set(report.get("binaries", {})) != set(spec["binaries"]):
        raise ValueError("Incomplete NSIS helper binary evidence")
    for name, record in report["binaries"].items():
        for build in ("first", "second"):
            capture = directory / build / name
            if crt.checksum(capture / name) != record["sha256"]:
                raise ValueError("NSIS helper hash/repeatability mismatch")
            for path, checksum in record["evidence"][build].items():
                if Path(path).name != path or crt.checksum(capture / path) != checksum:
                    raise ValueError("Missing or altered NSIS linker evidence")
            required = {*crt.CAPTURE_FILES, "pe.txt", "build-command.json"}
            if spec["binaries"][name]["crt"] == "static":
                required.add("rustflags.json")
            if not required <= record["evidence"][build].keys():
                raise ValueError("Incomplete NSIS linker evidence")
            check_link(capture, spec["binaries"][name])
            if "rustflags.json" in required:
                flags = json.loads((capture / "rustflags.json").read_text())
                if "windows_raw_dylib" not in flags or "target-feature=+crt-static" not in flags:
                    raise ValueError("Unexpected helper CRT/import-library configuration")
            imports = check_identity((capture / "pe.txt").read_text(), spec["binaries"][name])
            if imports != record["imports"] or record["crt"] != spec["binaries"][name]["crt"]:
                raise ValueError("NSIS helper import/CRT inventory differs from captured binary")
            toolchain = json.loads((capture / "toolchain.json").read_text())
            environment = toolchain["environment"]
            if (environment["VCTOOLSVERSION"].rstrip("\\/") != spec["msvc"]
                    or environment["WINDOWSSDKVERSION"].rstrip("\\/") != spec["windows_sdk"]
                    or not re.fullmatch(r"[0-9a-f]{64}", toolchain.get("compiler", {}).get("sha256", ""))
                    or not re.fullmatch(r"[0-9a-f]{64}", toolchain.get("linker", {}).get("sha256", ""))
                    or not toolchain.get("linker", {}).get("version")):
                raise ValueError("Unexpected helper compiler/SDK")
        if crt.checksum(directory / "plugins" / name) != record["sha256"]:
            raise ValueError("Staged NSIS helper differs from captured binary")
    if set(p.name for p in (directory / "plugins").iterdir()) != set(spec["binaries"]):
        raise ValueError("Unexpected staged NSIS plugin")
    return report


def build():
    if os.name != "nt":
        raise ValueError("NSIS helper builds require native Windows MSVC")
    spec = json.loads((KIT / "inputs.json").read_text())
    prepare_source()
    env, toolchain = crt.msvc_environment(os.environ.copy(), architecture="x64_x86",
                                        toolset=spec["msvc"], sdk=spec["windows_sdk"])
    env["RUSTUP_TOOLCHAIN"] = spec["rust"]
    rustc = crt.output(["rustc", "-vV"], env=env)
    if not rustc.startswith(f"rustc {spec['rust']} "):
        raise ValueError("Unexpected helper Rust compiler")
    compiler = Path(toolchain["linker"]["path"]).with_name("cl.exe")
    toolchain["compiler"] = {"path": str(compiler), "sha256": crt.checksum(compiler)}
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    (OUTPUT / "plugins").mkdir(parents=True)
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False)
    report = {"schema": 1, "spec": spec, "inputs": inputs(), "packages": locked_packages(),
              "rustc": rustc, "cargo": crt.output(["cargo", "--version"], env=env),
              "source_commit": revision.stdout.strip() if revision.returncode == 0 else None,
              "license_assessment": "windows-crt-unresolved", "binaries": {}}
    for build_name in ("first", "second"):
        work = OUTPUT / build_name
        for name, expected in spec["binaries"].items():
            capture = work / name
            capture.mkdir(parents=True)
            crt.write_json(capture / "toolchain.json", toolchain)
            link_env = {**env, "CODECONVOY_REAL_LINKER": toolchain["linker"]["path"],
                        "CODECONVOY_CRT_EVIDENCE": str(capture)}
            if expected["crt"] == "none":
                command = [str(compiler), "/nologo", "/c", "/O1", "/W4", "/WX", "/GS-", "/GR-", "/Zl", "/Oi",
                           "/DUNICODE", "/D_UNICODE", "/DNOMINMAX", "/DWIN32_LEAN_AND_MEAN",
                           str(KIT / "appid.cpp"), f"/Fo{capture / 'appid.obj'}"]
                subprocess.run(command, env=env, check=True)
                arguments = [str(capture / "appid.obj"), "/DLL", "/NOENTRY", "/NODEFAULTLIB", "/MACHINE:X86", "/Brepro",
                             f"/DEF:{KIT / 'appid.def'}", f"/OUT:{capture / name}", "kernel32.lib", "ole32.lib", "uuid.lib"]
                subprocess.run([sys.executable, __file__, "link-no-crt", *arguments], env=link_env, check=True)
            else:
                wrapper = capture / "link.cmd"
                wrapper.write_text(f"@{crt.command_path(sys.executable)} {crt.command_path(Path(crt.__file__))} link %*\n")
                flags = ["--cfg", "windows_raw_dylib", "-C", "target-feature=+crt-static", "-C", "link-arg=/Brepro",
                         f"--remap-path-prefix={Path.home()}=/build-home",
                         f"--remap-path-prefix={env.get('CARGO_HOME', Path.home() / '.cargo')}=/cargo",
                         f"--remap-path-prefix={ROOT}=."]
                link_env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
                crt.write_json(capture / "rustflags.json", flags)
                command = ["cargo", "rustc", "--locked", "--release", "--manifest-path", str(KIT / "Cargo.toml"),
                           "--target", spec["target"], "--target-dir", str(work / "cargo"),
                           "-p", "codeconvoy-utils", "--", "-C", f"linker={wrapper}"]
                subprocess.run(command, env=link_env, check=True)
                shutil.copyfile(work / "cargo" / spec["target"] / "release" / name, capture / name)
            crt.write_json(capture / "build-command.json", command)
            inspection = crt.output([compiler.with_name("dumpbin.exe"), "/headers", "/imports", "/exports", capture / name], env=env)
            (capture / "pe.txt").write_text(inspection, encoding="utf-8")
            imports = check_identity(inspection, expected)
            check_link(capture, expected)
            checksum = crt.checksum(capture / name)
            record = report["binaries"].setdefault(name, {"sha256": checksum, "imports": imports, "crt": expected["crt"], "evidence": {}})
            if record["sha256"] != checksum:
                raise ValueError(f"Independent clean helper builds differ: {name}")
            evidence = [*crt.CAPTURE_FILES, "pe.txt", "build-command.json", *[p.name for p in capture.glob("response-*.rsp")]]
            if expected["crt"] == "static":
                evidence.append("rustflags.json")
            record["evidence"][build_name] = {p: crt.checksum(capture / p) for p in evidence}
            shutil.copyfile(capture / name, OUTPUT / "plugins" / name)
    crt.write_json(OUTPUT / "provenance.json", report)
    verify()
    subprocess.run(["cargo", "test", "--locked", "--manifest-path", str(KIT / "Cargo.toml"),
                    "--target", spec["target"], "--target-dir", str(OUTPUT / "tests"),
                    "-p", "nsis-semvercompare"], env=env, check=True)
    print("Two clean NSIS helper builds match; Windows CRT publication obligation remains unresolved.")


def configure(config, work):
    verify()
    path = (OUTPUT / "plugins").resolve().as_posix()
    if any(c in path for c in '$"\r\n'):
        raise ValueError("Unsupported NSIS helper directory")
    template = (ROOT / "packaging/nsis/installer.nsi").read_text()
    if template.count("@CODECONVOY_HELPERS@") != 1:
        raise ValueError("Missing independent NSIS helper path")
    rendered = work / "installer.nsi"
    rendered.write_text(template.replace("@CODECONVOY_HELPERS@", path), encoding="utf-8")
    config["nsis"]["template"] = str(rendered.resolve())


def native_tests():
    verify()
    spec = json.loads((KIT / "inputs.json").read_text())
    env, toolchain = crt.msvc_environment(os.environ.copy(), architecture="x64_x86",
                                        toolset=spec["msvc"], sdk=spec["windows_sdk"])
    work = ROOT / "target/packaging/nsis-tests"
    work.mkdir(parents=True, exist_ok=True)
    compiler = Path(toolchain["linker"]["path"]).with_name("cl.exe")
    subprocess.run([str(compiler), "/nologo", "/GS-", "/Zl", str(ROOT / "packaging/nsis/test-process.cpp"),
                    f"/Fo{work / 'process.obj'}", f"/Fe{work / 'CodeConvoyNsisFixture.exe'}", "/link",
                    "/ENTRY:start", "/SUBSYSTEM:WINDOWS", "/NODEFAULTLIB", "kernel32.lib"], env=env, check=True)
    template = (ROOT / "packaging/nsis/helper-tests.nsi").read_text(encoding="utf-8")
    for token, path in (("@OUTPUT@", work / "helper-tests.exe"), ("@HELPERS@", OUTPUT / "plugins")):
        value = path.as_posix()
        if any(c in value for c in '$"\r\n'):
            raise ValueError("Unsupported native NSIS test path")
        template = template.replace(token, value)
    script = work / "helper-tests.nsi"
    script.write_text(template, encoding="utf-8-sig")
    makensis = Path(env["LOCALAPPDATA"]) / ".cargo-packager/NSIS/makensis.exe"
    subprocess.run([str(makensis), str(script)], env=env, check=True)
    subprocess.run(["pwsh", "-NoProfile", "-File", str(ROOT / "packaging/nsis/native-tests.ps1")], check=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "link-no-crt":
        sys.exit(crt.capture_link(sys.argv[2:], require_crt=False))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "verify", "test"))
    args = parser.parse_args()
    {"build": build, "verify": verify, "test": native_tests}[args.command]()
