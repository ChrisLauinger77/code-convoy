"""Native package assembly and release gates. Requires Python 3.11+; no pip packages."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
IDENTIFIER = "io.github.chrislauinger77.code-convoy"


def metadata():
    with (ROOT / "Cargo.toml").open("rb") as source:
        return tomllib.load(source)["package"]


def version():
    value = metadata()["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError(f"Expected a normal release version, found {value!r}")
    return value


def check_tag(tag):
    expected = f"v{version()}"
    if tag != expected:
        raise ValueError(f"Tag {tag!r} does not match Cargo.toml: expected {expected!r}")


def artifact_names(value):
    return [f"code-convoy_{value}_amd64.deb", f"code-convoy-{value}-1.x86_64.rpm",
            f"CodeConvoy-{value}-x86_64.AppImage", f"CodeConvoy-{value}-windows-x86_64-setup.exe",
            f"CodeConvoy-{value}-windows-x86_64.zip", f"CodeConvoy-{value}-macos-universal.dmg"]


def checksum(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def aggregate(directory):
    expected = set(artifact_names(version()))
    actual = {p.name for p in directory.iterdir() if p.name != "SHA256SUMS"}
    if actual != expected:
        raise ValueError(f"Release assets differ: missing={sorted(expected-actual)}, extra={sorted(actual-expected)}")
    for name in expected:
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Release asset is not a nonempty regular file: {name}")
    (directory / "SHA256SUMS").write_text(
        "".join(f"{checksum(directory / name)}  {name}\n" for name in sorted(expected)), encoding="utf-8")


def run(*args, **kwargs):
    print("+", " ".join(str(a) for a in args), flush=True)
    return subprocess.run([str(a) for a in args], cwd=ROOT, check=True, **kwargs)


def fresh_work(platform):
    work = ROOT / "target" / "packaging" / platform
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    DIST.mkdir(exist_ok=True)
    return work


def package(work, binary_dir, binary, format_name):
    config = json.loads((ROOT / "packaging/packager.json").read_text())
    config.update(version=version(), description=metadata()["description"], homepage=metadata()["homepage"],
                  binaries=[{"path": binary, "main": True}], binariesDir=str(binary_dir),
                  outDir=str(work / "packages"))
    (work / "packager.json").write_text(json.dumps(config, indent=2) + "\n")
    # Raw JSON keeps relative source paths rooted in the checkout, not target/.
    run("cargo", "packager", "--config", json.dumps(config), "--formats", format_name)
    return work / "packages"


def one_file(directory, pattern):
    matches = list(directory.glob(pattern))
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one {pattern} in {directory}, found {matches}")
    return matches[0]


def verify_architectures(output):
    if set(output.split()) != {"arm64", "x86_64"}:
        raise ValueError(f"Expected both arm64 and x86_64, found {output.strip()!r}")


def verify_mac_app(app):
    with (app / "Contents/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    for key, expected in {"CFBundleIdentifier": IDENTIFIER, "CFBundleName": "CodeConvoy",
                          "CFBundleShortVersionString": version(), "CFBundleVersion": version()}.items():
        if info.get(key) != expected:
            raise ValueError(f"Invalid bundle {key}: {info.get(key)!r}")
    binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
    verify_architectures(run("/usr/bin/lipo", "-archs", binary, capture_output=True, text=True).stdout)
    run("/usr/bin/codesign", "--verify", "--deep", "--strict", "--all-architectures", "--verbose=2", app)
    signature = run("/usr/bin/codesign", "--display", "--verbose=4", app, capture_output=True, text=True).stderr
    print(signature)
    if "Signature=adhoc" not in signature or f"Identifier={IDENTIFIER}" not in signature:
        raise ValueError("Expected an ad-hoc signature with the CodeConvoy bundle identifier")


def sign_mac_app(app):
    # Future Developer ID signing/notarization belongs at this explicit boundary,
    # after assembly and before DMG creation. v0.1 needs no Apple credentials.
    run("/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", app)
    verify_mac_app(app)


def macos():
    work = fresh_work("macos")
    binaries = work / "bin"
    binaries.mkdir()
    run("/usr/bin/lipo", "-create", ROOT / "target/aarch64-apple-darwin/release/codeconvoy",
        ROOT / "target/x86_64-apple-darwin/release/codeconvoy", "-output", binaries / "codeconvoy")
    verify_architectures(run("/usr/bin/lipo", "-archs", binaries / "codeconvoy", capture_output=True, text=True).stdout)
    output = package(work, binaries, "codeconvoy", "app")
    app = one_file(output, "*.app")
    plist_path = app / "Contents/Info.plist"
    with plist_path.open("rb") as source:
        info = plistlib.load(source)
    # cargo-packager otherwise uses a timestamp for CFBundleVersion.
    info["CFBundleVersion"] = version()
    with plist_path.open("wb") as destination:
        plistlib.dump(info, destination)
    sign_mac_app(app)
    image_root = work / "dmg"
    image_root.mkdir()
    shutil.copytree(app, image_root / "CodeConvoy.app", symlinks=True)
    (image_root / "Applications").symlink_to("/Applications", target_is_directory=True)
    dmg = DIST / f"CodeConvoy-{version()}-macos-universal.dmg"
    run("/usr/bin/hdiutil", "create", "-ov", "-volname", "CodeConvoy", "-srcfolder", image_root,
        "-format", "UDZO", "-fs", "HFS+", dmg)
    run("/usr/bin/hdiutil", "verify", dmg)
    mount = work / "mounted"
    mount.mkdir()
    run("/usr/bin/hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg)
    try:
        verify_mac_app(mount / "CodeConvoy.app")
        if os.readlink(mount / "Applications") != "/Applications":
            raise ValueError("DMG is missing its Applications link")
    finally:
        run("/usr/bin/hdiutil", "detach", mount)


def verify_windows_exe(path):
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3c)[0]
    if data[pe:pe+4] != b"PE\0\0" or struct.unpack_from("<H", data, pe+4)[0] != 0x8664:
        raise ValueError("Expected an x86_64 PE executable")
    if struct.unpack_from("<H", data, pe+24+68)[0] != 2:
        raise ValueError("Release executable must use the Windows GUI subsystem")


def windows():
    work = fresh_work("windows")
    binaries = work / "bin"
    binaries.mkdir()
    exe = binaries / "CodeConvoy.exe"
    shutil.copy2(ROOT / "target/release/codeconvoy.exe", exe)
    verify_windows_exe(exe)
    output = package(work, binaries, "CodeConvoy", "nsis")
    shutil.copy2(one_file(output, "*.exe"), DIST / f"CodeConvoy-{version()}-windows-x86_64-setup.exe")
    archive = DIST / f"CodeConvoy-{version()}-windows-x86_64.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as zipped:
        zipped.write(exe, "CodeConvoy.exe")
    with zipfile.ZipFile(archive) as zipped:
        if zipped.namelist() != ["CodeConvoy.exe"] or zipped.testzip() is not None:
            raise ValueError("Portable ZIP failed verification")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["version", "check-tag", "aggregate", "macos", "windows", "linux"])
    parser.add_argument("value", nargs="?")
    args = parser.parse_args()
    if args.command == "version":
        print(version())
    elif args.command == "check-tag":
        check_tag(args.value)
    elif args.command == "aggregate":
        aggregate(Path(args.value) if args.value else DIST)
    elif args.command == "linux":
        from linux import package_linux
        package_linux()
    else:
        globals()[args.command]()


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"Release preparation failed: {error}")
