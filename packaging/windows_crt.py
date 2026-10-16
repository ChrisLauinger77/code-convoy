"""Windows linker evidence, not a redistribution approval or a license scanner."""
import hashlib
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import subprocess
import sys
import uuid
import windows_runtime

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "target/packaging/windows-crt"
CRT_NAMES = {"libcmt.lib", "libvcruntime.lib", "libucrt.lib", "libcpmt.lib",
             "msvcrt.lib", "vcruntime.lib", "ucrt.lib", "msvcprt.lib",
             "oldnames.lib", "legacy_stdio_definitions.lib"}
INPUTS = ("Cargo.toml", "Cargo.lock", "packaging/build.py", "packaging/windows_crt.py",
          "packaging/windows_runtime.py", "packaging/windows-runtime.json")
CAPTURE_FILES = ("link.log", "link.map", "invocation.json", "toolchain.json", "selected-libraries.json")


def pe_imports(text):
    return sorted(set(re.findall(r"^\s+([\w.-]+\.dll)\s*$", text, re.I | re.M)))


def check_dynamic_imports(imports, architecture=None):
    names = {name.lower() for name in imports}
    if "vcruntime140.dll" not in names or not any(
            n == "ucrtbase.dll" or n.startswith("api-ms-win-crt-") for n in names):
        raise ValueError("Missing dynamic VC/UCRT imports")
    if any(n.startswith(("msvcp", "msvcr")) for n in names):
        raise ValueError("Unexpected additional Microsoft C/C++ runtime import")
    if architecture:
        actual = sorted(n for n in names if re.search(r"msvcp|msvcr|vcruntime|ucrtbase|api-ms-win-crt", n))
        if actual != windows_runtime.policy()["crt_imports"][architecture]:
            raise ValueError("Dynamic CRT imports differ from the reviewed runtime prerequisite policy")


def check_dynamic_libraries(selected):
    names = {PureWindowsPath(p).name.lower() for p in selected}
    if names & {"libcmt.lib", "libvcruntime.lib", "libucrt.lib", "libcpmt.lib"}:
        raise ValueError("Unexpected static CRT implementation library")
    if "msvcrt.lib" not in names:
        raise ValueError("Missing dynamic CRT startup evidence")


def checksum(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def output(command, *, env=None):
    return subprocess.run(command if isinstance(command, str) else [str(a) for a in command],
                          check=True, cwd=ROOT, env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout.decode("utf-8", errors="replace")


def loaded_libraries(log, *, require_crt=True):
    """Only Loaded records prove selection; Searching/Found lines do not.

    VSLANG=1033 fixes diagnostic language. Keep archive-member spelling and full
    paths, and fail if MSVC changes this diagnostic format rather than guessing.
    """
    libraries = {}
    searched = None
    for line in log.splitlines():
        line = line.strip()
        if line.startswith("Searching "):
            match = re.fullmatch(r"Searching (.+\.(?:lib|rlib)):", line, re.I)
            searched = match.group(1) if match and PureWindowsPath(match.group(1)).is_absolute() else None
            continue
        if line == "Finished searching libraries" or line.startswith("Starting pass "):
            searched = None
        if not line.startswith("Loaded "):
            continue
        match = re.fullmatch(r"Loaded (.+\.(?:lib|rlib))\(([^\r\n]+)\)", line, re.I)
        if not match:
            raise ValueError(f"Unrecognized linker Loaded record: {line}")
        path, member = match.groups()
        if not PureWindowsPath(path).is_absolute():
            # MSVC 14.51 prints a basename here. Only its immediately active
            # absolute search record can disambiguate it, never LIB or PATH.
            if (not searched or PureWindowsPath(path).name != path
                    or PureWindowsPath(searched).name.lower() != path.lower()):
                raise ValueError(f"Linker did not identify a full library path: {path}")
            path = searched
        libraries.setdefault(path, set()).add(member)
    if not libraries or (require_crt and not any(PureWindowsPath(p).name.lower() in CRT_NAMES for p in libraries)):
        raise ValueError("No selected CRT library evidence in linker diagnostics")
    return {p: sorted(members) for p, members in sorted(libraries.items())}


def command_path(path):
    # These paths become literal .cmd source, never user-provided shell code.
    if any(c in str(path) for c in '\r\n"%!^&|<>'):
        raise ValueError("Unsupported command-file path")
    return f'"{path}"'


def linker_version(linker, env):
    # MSVC LINK returns 1100 for /? even when it successfully prints help.
    # This exception is limited to the probe; an actual link must return zero.
    probe = subprocess.run([str(linker), "/?"], env=env, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, check=False)
    text = probe.stdout.decode("utf-8", errors="replace")
    match = re.search(r"^Microsoft \(R\) Incremental Linker Version [0-9]+(?:\.[0-9]+)+\s*$", text, re.M)
    if probe.returncode not in (0, 1100) or not match:
        raise ValueError(f"Cannot identify MSVC linker (probe exit {probe.returncode}): {text}")
    return match.group().strip()


def msvc_environment(env, *, architecture="x64", toolset=None, sdk=None):
    if architecture not in ("x64", "x64_x86") or any(
            value is not None and not re.fullmatch(r"[0-9.]+", value) for value in (toolset, sdk)):
        raise ValueError("Unsupported MSVC environment selection")
    env = {key.upper(): value for key, value in env.items()}
    vswhere = Path(env["PROGRAMFILES(X86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
    instances = json.loads(output([vswhere, "-latest", "-products", "*", "-requires",
                                   "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-format", "json", "-utf8"]))
    if len(instances) != 1 or instances[0].get("isPrerelease"):
        raise ValueError("Expected one installed stable MSVC instance")
    instance = instances[0]
    vcvars = Path(instance["installationPath"]) / "VC/Auxiliary/Build/vcvarsall.bat"
    # Import the documented developer environment internally. Never print or
    # persist its complete output: the inherited environment can contain secrets.
    # cmd.exe parses command text, not C argv quoting (which escapes quotes with
    # backslashes). Quote these validated installation paths as cmd literals.
    arguments = architecture + (f" {sdk}" if sdk else "") + (f" -vcvars_ver={toolset}" if toolset else "")
    result = output(f'{command_path(env.get("COMSPEC", "cmd.exe"))} /d /s /c '
                    f'"call {command_path(vcvars)} {arguments} >nul && set"', env=env)
    selected = {key.upper(): value for key, value in env.items()}
    for line in result.splitlines():
        key, separator, value = line.partition("=")
        if separator and key:
            selected[key.upper()] = value
    selected["VSLANG"] = "1033"
    required = ("VCTOOLSVERSION", "VCTOOLSINSTALLDIR", "WINDOWSSDKVERSION", "WINDOWSSDKDIR", "LIB")
    if any(not selected.get(key) for key in required):
        raise ValueError("MSVC developer environment omitted toolset/SDK identity")
    machine = "x86" if architecture == "x64_x86" else "x64"
    linker = Path(selected["VCTOOLSINSTALLDIR"]) / f"bin/Hostx64/{machine}/link.exe"
    record = {
        "visual_studio": {key: instance[key] for key in
                          ("instanceId", "installationPath", "installationVersion", "productId", "isPrerelease")},
        "environment": {key: selected[key] for key in required},
        "linker": {"path": str(linker), "sha256": checksum(linker),
                   "version": linker_version(linker, selected)},
    }
    return selected, record


def capture_link(arguments, *, require_crt=True):
    """Called by rustc's final-binary linker wrapper, not by dependency builds."""
    directory = Path(os.environ["CODECONVOY_CRT_EVIDENCE"])
    response_files = []
    for argument in arguments:
        if argument.startswith("@"):
            source = Path(argument[1:])
            name = f"response-{len(response_files)}.rsp"
            shutil.copyfile(source, directory / name)
            response_files.append({"name": name, "original_path": str(source), "sha256": checksum(source)})
    # MSVC 14.51's :LIB alone reports searches without selected members.
    # Retain it, request full detail too, and keep a map of retained symbols.
    command = [os.environ["CODECONVOY_REAL_LINKER"], *arguments, "/VERBOSE:LIB", "/VERBOSE",
               f"/MAP:{directory / 'link.map'}"]
    # Exclusive creation rejects an unexpected second invocation. A failed or
    # skipped link must never reuse an earlier successful report.
    with (directory / "link.log").open("xb") as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    write_json(directory / "invocation.json", {"command": command, "exit_code": result.returncode,
                                               "response_files": response_files})
    if result.returncode == 0:
        # Rust may remove generated import archives immediately after this
        # wrapper exits. Hash every selected archive while it still exists.
        selected = loaded_libraries((directory / "link.log").read_text(encoding="utf-8", errors="replace"),
                                    require_crt=require_crt)
        write_json(directory / "selected-libraries.json", [
            {"path": path, "sha256": checksum(path), "members": members,
             "crt_family": PureWindowsPath(path).name.lower() in CRT_NAMES}
            for path, members in selected.items()])
    sys.stdout.buffer.write((directory / "link.log").read_bytes())
    return result.returncode


def build(command, env, target=None):
    if target not in (None, "x86_64-pc-windows-msvc"):
        raise ValueError("CRT provenance currently covers the Windows x86_64 release target")
    if EVIDENCE.exists():
        shutil.rmtree(EVIDENCE)
    EVIDENCE.mkdir(parents=True)
    runtime = windows_runtime.policy()
    env, toolchain = msvc_environment(env, toolset=runtime["msvc"], sdk=runtime["windows_sdk"])
    write_json(EVIDENCE / "toolchain.json", toolchain)
    env["CODECONVOY_REAL_LINKER"] = toolchain["linker"]["path"]
    env["CODECONVOY_CRT_EVIDENCE"] = str(EVIDENCE)
    # A unique linker argument forces the final executable to relink even with
    # a warm Cargo cache. Dependency compilation still uses the existing flags.
    wrapper = EVIDENCE / f"link-{uuid.uuid4().hex}.cmd"
    wrapper.write_text(f"@{command_path(sys.executable)} {command_path(Path(__file__).resolve())} link %*\n",
                       encoding="utf-8")
    command = [command[0], "rustc", *command[2:], "--", "-C", f"linker={wrapper}"]
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    invocation = json.loads((EVIDENCE / "invocation.json").read_text())
    if invocation["exit_code"] != 0:
        raise ValueError("CRT evidence records a failed linker invocation")
    libraries = json.loads((EVIDENCE / "selected-libraries.json").read_text(encoding="utf-8"))
    binary = ROOT / "target" / (target or "") / "release/codeconvoy.exe"
    inspection = output([Path(toolchain["linker"]["path"]).with_name("dumpbin.exe"), "/headers", "/imports", binary], env=env)
    (EVIDENCE / "application-pe.txt").write_text(inspection, encoding="utf-8")
    imports = pe_imports(inspection)
    check_dynamic_imports(imports, "x64")
    check_dynamic_libraries(p["path"] for p in libraries)
    windows_runtime.capture_system(EVIDENCE)
    report = {
        "imports": imports, "crt": "dynamic",
        "pe_sha256": checksum(EVIDENCE / "application-pe.txt"),
        "schema": 1, "status": "captured", "license_assessment": "unresolved",
        "source_commit": output(["git", "rev-parse", "HEAD"]).strip(),
        "target": "x86_64-pc-windows-msvc", "binary_sha256": checksum(binary),
        "rustc": output(["rustc", "-vV"], env=env), "cargo": output(["cargo", "--version"], env=env).strip(),
        "runner": {key: env.get(key.upper(), "") for key in ("ImageOS", "ImageVersion", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT")},
        "inputs": {name: checksum(ROOT / name) for name in INPUTS},
        "toolchain": toolchain, "libraries": libraries,
        "evidence": {name: checksum(EVIDENCE / name) for name in CAPTURE_FILES},
    }
    write_json(EVIDENCE / "provenance.json", report)
    verify(binary)
    print("Windows linker inputs captured; redistribution approval remains unresolved.", flush=True)


def verify(binary, directory=EVIDENCE, *, source_commit=None):
    report = json.loads((directory / "provenance.json").read_text(encoding="utf-8"))
    source_commit = source_commit or output(["git", "rev-parse", "HEAD"]).strip()
    if (report.get("schema") != 1 or report.get("status") != "captured"
            or report.get("license_assessment") != "unresolved"
            or report.get("source_commit") != source_commit
            or report.get("target") != "x86_64-pc-windows-msvc"
            or report.get("binary_sha256") != checksum(binary)):
        raise ValueError("Missing, stale or mismatched Windows CRT provenance")
    if report.get("inputs") != {name: checksum(ROOT / name) for name in INPUTS}:
        raise ValueError("Windows CRT provenance describes different build inputs")
    toolchain = report.get("toolchain", {})
    linker = toolchain.get("linker", {})
    if (not report.get("rustc") or not report.get("cargo") or not linker.get("path")
            or not linker.get("version") or not re.fullmatch(r"[0-9a-f]{64}", linker.get("sha256", ""))
            or not toolchain.get("visual_studio", {}).get("installationVersion")
            or any(not toolchain.get("environment", {}).get(key) for key in
                   ("VCTOOLSVERSION", "VCTOOLSINSTALLDIR", "WINDOWSSDKVERSION", "WINDOWSSDKDIR", "LIB"))):
        raise ValueError("Incomplete Windows CRT toolchain identity")
    for name in CAPTURE_FILES:
        if report.get("evidence", {}).get(name) != checksum(directory / name):
            raise ValueError(f"Missing or altered Windows CRT evidence: {name}")
    invocation = json.loads((directory / "invocation.json").read_text())
    if (invocation.get("exit_code") != 0 or not invocation.get("command")
            or invocation["command"][0] != linker["path"] or "/VERBOSE:LIB" not in invocation["command"]
            or "/VERBOSE" not in invocation["command"]
            or not any(a.startswith("/MAP:") for a in invocation["command"])
            or json.loads((directory / "toolchain.json").read_text()) != toolchain):
        raise ValueError("Incomplete or failed Windows linker capture")
    for response in invocation.get("response_files", []):
        if (not re.fullmatch(r"response-[0-9]+\.rsp", response["name"])
                or checksum(directory / response["name"]) != response["sha256"]):
            raise ValueError("Missing or altered linker response file")
    if [r["original_path"] for r in invocation.get("response_files", [])] != [
            a[1:] for a in invocation["command"] if a.startswith("@")]:
        raise ValueError("Incomplete linker response file capture")
    selected = loaded_libraries((directory / "link.log").read_text(encoding="utf-8", errors="replace"))
    libraries = report.get("libraries", [])
    if (libraries != json.loads((directory / "selected-libraries.json").read_text(encoding="utf-8"))
            or len(libraries) != len(selected) or {p["path"]: p["members"] for p in libraries} != selected
            or any(not re.fullmatch(r"[0-9a-f]{64}", p.get("sha256", "")) for p in libraries)):
        raise ValueError("Selected Windows linker inputs differ from raw diagnostics")
    inspection = directory / "application-pe.txt"
    if (report.get("crt") != "dynamic" or report.get("pe_sha256") != checksum(inspection)
            or report.get("imports") != pe_imports(inspection.read_text())):
        raise ValueError("Missing or altered application dynamic CRT inspection")
    check_dynamic_imports(report["imports"], "x64")
    check_dynamic_libraries(selected)
    return report


def record_packages(installer, archive, binary):
    """Bind the already byte-verified final packages to the linker/helper evidence."""
    report = verify(binary)
    helpers = json.loads((EVIDENCE / "packaged-binaries.json").read_text(encoding="utf-8"))
    if (helpers.get("application_sha256") != checksum(binary)
            or helpers.get("license_assessment") != "unresolved"
            or not any(p["name"] == "installer" and p["sha256"] == checksum(installer)
                       for p in helpers.get("binaries", []))):
        raise ValueError("Final installer differs from inspected Windows binaries")
    for record in helpers["binaries"]:
        if (not re.fullmatch(r"pe-[0-9]+\.txt", record["inspection"])
                or checksum(EVIDENCE / record["inspection"]) != record["inspection_sha256"]):
            raise ValueError("Missing or altered Windows PE inspection")
    evidence_files = ["provenance.json", "packaged-binaries.json"]
    if "nsis_provenance_sha256" in helpers:
        if checksum(EVIDENCE / "nsis-provenance.json") != helpers["nsis_provenance_sha256"]:
            raise ValueError("Missing or altered NSIS helper CRT provenance")
        evidence_files.append("nsis-provenance.json")
    write_json(EVIDENCE / "packages.json", {
        "schema": 1, "source_commit": report["source_commit"], "license_assessment": "unresolved",
        "application_sha256": checksum(binary),
        "evidence": {name: checksum(EVIDENCE / name) for name in evidence_files},
        "packages": {p.name: checksum(p) for p in (installer, archive)},
    })


def inspect_installer(installer, payload, binary):
    """Record rebuilt helpers and remaining prebuilt NSIS components separately."""
    import nsis_helpers
    report = verify(binary)
    helper_report = nsis_helpers.verify()
    shutil.copyfile(nsis_helpers.OUTPUT / "provenance.json", EVIDENCE / "nsis-provenance.json")
    dumpbin = Path(report["toolchain"]["linker"]["path"]).with_name("dumpbin.exe")
    records = []
    paths = [("installer", installer)] + [(p.relative_to(payload).as_posix(), p)
             for p in sorted(payload.rglob("*")) if p.is_file() and p.suffix.lower() in (".dll", ".exe")]
    for index, (name, path) in enumerate(paths):
        text = output([dumpbin, "/headers", "/imports", path], env={**os.environ, "VSLANG": "1033"})
        log = EVIDENCE / f"pe-{index}.txt"
        log.write_text(text, encoding="utf-8")
        helper = helper_report["binaries"].get(path.name) if name.startswith("$PLUGINSDIR/") else None
        if helper and helper["sha256"] != checksum(path):
            raise ValueError("Final installer helper differs from captured linker inputs")
        records.append({"name": name, "sha256": checksum(path), "inspection": log.name,
                        "inspection_sha256": checksum(log),
                        "imports": sorted(set(re.findall(r"^\s+([\w.-]+\.dll)\s*$", text, re.I | re.M))),
                        "static_crt_provenance": "captured above" if name == "CodeConvoy.exe" else
                        (f"nsis-provenance.json: {path.name}; CRT={helper['crt']}; redistribution assessment remains unresolved"
                         if helper else "unresolved: prebuilt component; imports do not identify static library inputs")})
    write_json(EVIDENCE / "packaged-binaries.json", {"schema": 1, "application_sha256": checksum(binary),
               "nsis_provenance_sha256": checksum(EVIDENCE / "nsis-provenance.json"),
               "license_assessment": "unresolved", "dumpbin_sha256": checksum(dumpbin), "binaries": records})


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "link":
        sys.exit(capture_link(sys.argv[2:]))
    sys.exit("Use packaging/build.py and packaging/release.py windows to capture and verify evidence")
