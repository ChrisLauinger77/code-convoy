"""Linux package assembly; invoked by release.py after the locked native build."""
import json
import os
import re
import shutil
import urllib.request

from release import ROOT, DIST, checksum, fresh_work, one_file, package, run, version


def required_glibc(output):
    # Definitions exported by a bundled library are not its runtime requirements.
    needs = output.partition("Version needs section")[2]
    names = set(re.findall(r"Name: (GLIBC_\S+)", needs))
    if any(not re.fullmatch(r"GLIBC_[0-9]+(?:\.[0-9]+)+", name) for name in names):
        raise ValueError(f"Unrecognized glibc ABI requirement; inspect before packaging: {sorted(names)}")
    versions = [tuple(map(int, name.removeprefix("GLIBC_").split("."))) for name in names]
    return max(versions) if versions else None


def inspect_glibc(path):
    output = run("readelf", "--version-info", "--wide", path, capture_output=True,
                 text=True, env=dict(os.environ, LC_ALL="C")).stdout
    return required_glibc(output)


def glibc_label(value):
    return ".".join(map(str, value))


def verify_linux_payload(appdir):
    for target, source in {
        "usr/bin/codeconvoy": "target/release/codeconvoy",
        "usr/share/applications/codeconvoy.desktop": "packaging/codeconvoy.desktop",
        "usr/share/icons/hicolor/256x256/apps/codeconvoy.png": "assets/codeconvoy-256.png",
        "usr/share/doc/code-convoy/copyright": "LICENSE",
    }.items():
        if (appdir / target).read_bytes() != (ROOT / source).read_bytes():
            raise ValueError(f"Unexpected package payload: {target}")


def report_appimage_glibc(appdir, work):
    requirements = []
    for path in sorted(appdir.rglob("*")):
        if path.is_symlink() or not path.is_file():
            continue
        with path.open("rb") as source:
            if source.read(4) != b"\x7fELF":
                continue
        required = inspect_glibc(path)
        if required:
            requirements.append((required, path.relative_to(appdir)))
    if not requirements:
        raise ValueError("No glibc requirements found in the deployed AppImage payload")
    minimum = max(value for value, _ in requirements)
    report = f"AppImage payload requires glibc >= {glibc_label(minimum)}\n"
    report += "".join(f"{glibc_label(value)}  {path}\n" for value, path in requirements)
    (work / "glibc-requirements.txt").write_text(report, encoding="utf-8")
    print(report, flush=True)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as output:
            output.write(f"\n### Linux runtime ABI inspection\n\n```text\n{report}```\n")


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
    report_appimage_glibc(appdir, work)
    # Replace any generated AppRun, including a symlink, after dependency deployment.
    launcher = appdir / "AppRun"
    launcher.unlink(missing_ok=True)
    shutil.copy2(ROOT / "packaging/AppRun", launcher)
    launcher.chmod(0o755)
    env = dict(os.environ, ARCH="x86_64")
    image = DIST / f"CodeConvoy-{version()}-x86_64.AppImage"
    run(tools["appimagetool"], "--appimage-extract-and-run", "--runtime-file", tools["runtime"], appdir, image, env=env)
    image.chmod(0o755)
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
    run(probe_image, env=probe_env)
