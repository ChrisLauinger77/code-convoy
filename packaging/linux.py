"""Linux package assembly; invoked by release.py after the locked native build."""
import json
import os
import shutil
import urllib.request

from release import ROOT, DIST, checksum, fresh_work, one_file, package, run, version


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
    output = package(work, ROOT / "target/release", "codeconvoy", "deb")
    deb = DIST / f"code-convoy_{version()}_amd64.deb"
    shutil.copy2(one_file(output, "*.deb"), deb)
    control = run("dpkg-deb", "--field", deb, "Package", "Version", "Architecture",
                  capture_output=True, text=True).stdout
    if control.splitlines() != ["Package: code-convoy", f"Version: {version()}", "Architecture: amd64"]:
        raise ValueError(f"Unexpected Debian identity: {control}")
    run("dpkg-deb", "--contents", deb)
    appdir = work / "CodeConvoy.AppDir"
    run("dpkg-deb", "--extract", deb, appdir)
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
