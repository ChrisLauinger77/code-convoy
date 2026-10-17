"""Linux package assembly; invoked by release.py after the locked native build."""
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import urllib.request

from release import ROOT, DIST, checksum, fresh_work, one_file, package, run, version
from linux_abi import glibc_label, inspect_glibc, verify_artifacts
import licenses


def check_extraction_cleanup(image, env):
    """Observe the accepted defect before deleting this test's private TMPDIR.

    Repeated identical images may retain one tree, never one per launch. Only
    the reviewed upstream pin gets this exception; execution failures propagate.
    """
    licenses.verify_appimage_runtime(image)
    pin = json.loads((ROOT / "packaging/appimage-tools.json").read_text())["runtime"]
    known = "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
    with tempfile.TemporaryDirectory(prefix="codeconvoy-extraction-check-") as directory:
        temporary = Path(directory)
        clean_env = {k: v for k, v in env.items()
                     if k not in ("APPIMAGE_EXTRACT_AND_RUN", "NO_CLEANUP")}
        clean_env["TMPDIR"] = directory
        previous = None
        for mode in ("flag", "environment", "flag"):
            if mode == "flag":
                run(image, "--appimage-extract-and-run", env=clean_env)
            else:
                run(image, env=dict(clean_env, APPIMAGE_EXTRACT_AND_RUN="1"))
            entries = list(temporary.iterdir())
            if not entries:
                current = {}
                print("PASS extraction cleanup: no retained files", flush=True)
            else:
                if (pin["sha256"] != known or len(entries) != 1 or entries[0].is_symlink()
                        or not entries[0].is_dir()
                        or not re.fullmatch(r"appimage_extracted_[0-9a-f]{32}", entries[0].name)):
                    raise ValueError("Unexpected extraction cleanup regression")
                current = {str(p.relative_to(temporary)): p.lstat().st_size
                           for p in entries[0].rglob("*") if p.is_file() and not p.is_symlink()}
                if not current:
                    raise ValueError("Retained extraction tree has no payload")
                print(f"KNOWN UPSTREAM DEFECT: extraction cleanup retained one tree, "
                      f"{len(current)} files / {sum(current.values())} bytes ({mode}); "
                      "see docs/appimage-upstream-runtime-validation.md", flush=True)
            if previous is not None and previous != current:
                raise ValueError("Repeated extraction changed the retained payload")
            previous = current


def verify_linux_payload(appdir):
    for target, source in {
        "usr/bin/codeconvoy": "target/release/codeconvoy",
        "usr/share/applications/codeconvoy.desktop": "packaging/codeconvoy.desktop",
        "usr/share/icons/hicolor/256x256/apps/codeconvoy.png": "assets/codeconvoy-256.png",
        "usr/share/doc/code-convoy/copyright": "LICENSE",
    }.items():
        if (appdir / target).read_bytes() != (ROOT / source).read_bytes():
            raise ValueError(f"Unexpected package payload: {target}")


def download_tools(work):
    tools = {}
    for name, spec in json.loads((ROOT / "packaging/appimage-tools.json").read_text()).items():
        path = work / name
        with urllib.request.urlopen(spec["url"], timeout=120) as source, path.open("wb") as target:
            shutil.copyfileobj(source, target)
        if checksum(path) != spec["sha256"]:
            raise ValueError(f"SHA256 mismatch for {name}; refusing to execute download")
        path.chmod(0o755)
        tools[name] = path
    return tools


def package_linux():
    work = fresh_work("linux")
    minimum = inspect_glibc(ROOT / "target/release/codeconvoy")
    if minimum is None:
        raise ValueError("No glibc requirements found in the Linux executable")
    output = package(work, ROOT / "target/release", "codeconvoy", "deb", glibc=glibc_label(minimum))
    deb = DIST / f"code-convoy_{version()}_amd64.deb"
    shutil.copy2(one_file(output, "*.deb"), deb)
    control = run("dpkg-deb", "--field", deb, "Package", "Version", "Architecture",
                  capture_output=True, text=True).stdout
    if control.splitlines() != ["Package: code-convoy", f"Version: {version()}", "Architecture: amd64"]:
        raise ValueError(f"Unexpected Debian identity: {control}")
    run("dpkg-deb", "--contents", deb)
    depends = run("dpkg-deb", "--field", deb, "Depends", capture_output=True, text=True).stdout
    if f"libc6 (>= {glibc_label(minimum)})" not in [d.strip() for d in depends.split(",")]:
        raise ValueError(f"DEB omits its measured glibc requirement: {depends}")
    appdir = work / "CodeConvoy.AppDir"
    run("dpkg-deb", "--extract", deb, appdir)
    verify_linux_payload(appdir)
    desktop = appdir / "usr/share/applications/codeconvoy.desktop"
    run("desktop-file-validate", desktop)
    run("rpmbuild", "-bb", ROOT / "packaging/code-convoy.spec",
        "--define", f"_topdir {work / 'rpm'}", "--define", f"project_root {ROOT}",
        "--define", f"app_version {version()}", "--target", "x86_64")
    rpm = DIST / f"code-convoy-{version()}-1.x86_64.rpm"
    shutil.copy2(one_file(work / "rpm/RPMS/x86_64", "*.rpm"), rpm)
    identity = run("rpm", "-qp", "--queryformat", "%{NAME} %{VERSION} %{RELEASE} %{ARCH}", rpm,
                   capture_output=True, text=True).stdout
    if identity != f"code-convoy {version()} 1 x86_64":
        raise ValueError(f"Unexpected RPM identity: {identity}")
    run("rpm", "-qpl", rpm)
    run("rpm", "-qpR", rpm)
    tools = download_tools(work)
    run(tools["linuxdeploy"], "--appimage-extract-and-run", "--appdir", appdir,
        "--desktop-file", desktop, "--icon-file", appdir / "usr/share/icons/hicolor/256x256/apps/codeconvoy.png")
    # Replace any generated AppRun, including a symlink, after dependency deployment.
    launcher = appdir / "AppRun"
    launcher.unlink(missing_ok=True)
    shutil.copy2(ROOT / "packaging/AppRun", launcher)
    launcher.chmod(0o755)
    env = dict(os.environ, ARCH="x86_64")
    image = DIST / f"CodeConvoy-{version()}-x86_64.AppImage"
    run(tools["appimagetool"], "--appimage-extract-and-run", "--runtime-file", tools["runtime"], appdir, image, env=env)
    image.chmod(0o755)
    licenses.verify_appimage_runtime(image, tools["runtime"])
    verify_artifacts([deb, rpm, image], work)
    # Exercise the same AppRun/runtime with the project's real process/Git layer.
    # This probe image is an intermediate test artifact, never a public download.
    probe_dir = work / "probe.AppDir"
    shutil.copytree(appdir, probe_dir, symlinks=True)
    shutil.copy2(ROOT / "target/release/examples/package_probe", probe_dir / "usr/bin/codeconvoy")
    host_bin = work / "host-bin"
    host_bin.mkdir()
    for name in ("codex", "copilot", "opencode", "claude"):
        executable = host_bin / name
        executable.write_text(f"#!/bin/sh\nprintf 'package-host-{name}\\n'\n")
        executable.chmod(0o755)
    probe_image = work / "probe.AppImage"
    run(tools["appimagetool"], "--appimage-extract-and-run", "--runtime-file", tools["runtime"], probe_dir, probe_image, env=env)
    host_path = str(host_bin) + os.pathsep + os.environ["PATH"]
    probe_env = dict(os.environ, PATH=host_path, CODECONVOY_EXPECT_PATH=host_path,
                     CODECONVOY_EXPECT_LD_LIBRARY_PATH=os.environ.get("LD_LIBRARY_PATH", ""),
                     APPIMAGE_EXTRACT_AND_RUN="1")
    check_extraction_cleanup(probe_image, probe_env)
