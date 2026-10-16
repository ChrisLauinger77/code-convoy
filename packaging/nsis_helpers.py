"""Build and verify the two narrowly scoped, source-owned NSIS helpers."""
import argparse
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import subprocess
import sys
import tomllib

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
    return [{key: p[key] for key in ("name", "version", "checksum")}
            for p in tomllib.loads((kit / "Cargo.lock").read_text())["package"] if p.get("source")]


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
    if invocation["exit_code"] != 0 or not all(flag in invocation["command"] for flag in ("/VERBOSE", "/VERBOSE:LIB", "/Brepro")):
        raise ValueError("Missing successful deterministic helper linker invocation")
    selected = crt.loaded_libraries((directory / "link.log").read_text(encoding="utf-8", errors="replace"),
                                    require_crt=expected["crt"] == "static")
    libraries = json.loads((directory / "selected-libraries.json").read_text())
    if ({p["path"]: p["members"] for p in libraries} != selected or len(libraries) != len(selected)
            or any(not re.fullmatch(r"[0-9a-f]{64}", p["sha256"]) for p in libraries)):
        raise ValueError("Helper linker inventory differs from raw selection evidence")
    if expected["crt"] == "none" and any(PureWindowsPath(p).name.lower() in crt.CRT_NAMES for p in selected):
        raise ValueError("Shortcut helper unexpectedly includes CRT code")
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
            required = {*crt.CAPTURE_FILES, "pe.txt"}
            if not required <= record["evidence"][build].keys():
                raise ValueError("Incomplete NSIS linker evidence")
            check_link(capture, spec["binaries"][name])
            check_identity((capture / "pe.txt").read_text(), spec["binaries"][name])
            toolchain = json.loads((capture / "toolchain.json").read_text())
            environment = toolchain["environment"]
            if (environment["VCTOOLSVERSION"].rstrip("\\/") != spec["msvc"]
                    or environment["WINDOWSSDKVERSION"].rstrip("\\/") != spec["windows_sdk"]
                    or not toolchain.get("compiler", {}).get("sha256")):
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
    report = {"schema": 1, "spec": spec, "inputs": inputs(), "packages": locked_packages(),
              "rustc": rustc, "cargo": crt.output(["cargo", "--version"], env=env),
              "source_commit": crt.output(["git", "rev-parse", "HEAD"]).strip(),
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
                flags = ["-C", "target-feature=+crt-static", "-C", "link-arg=/Brepro",
                         f"--remap-path-prefix={Path.home()}=/build-home",
                         f"--remap-path-prefix={env.get('CARGO_HOME', Path.home() / '.cargo')}=/cargo",
                         f"--remap-path-prefix={ROOT}=."]
                link_env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
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


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "link-no-crt":
        sys.exit(crt.capture_link(sys.argv[2:], require_crt=False))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "verify"))
    args = parser.parse_args()
    build() if args.command == "build" else verify()
