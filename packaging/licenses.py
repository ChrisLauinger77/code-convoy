"""Offline, reviewed third-party evidence and final-package omission gates."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "third-party"
BUNDLE = ROOT / "target/packaging/licenses"
FILES = ("THIRD-PARTY-NOTICES.txt", "THIRD-PARTY-INVENTORY.json",
         "THIRD-PARTY-SOURCES.zip", "RUST-STDLIB-LICENSES.html")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def dependency_inputs(manifest):
    return {key: manifest[key] for key in ("dependencies", "dev-dependencies", "build-dependencies", "target", "features")
            if key in manifest}


def checked_catalog(root=ROOT, evidence=EVIDENCE):
    catalog = json.loads((evidence / "catalog.json").read_text(encoding="utf-8"))
    texts = json.loads((evidence / "texts.json").read_text(encoding="utf-8"))
    if catalog["schema"] != 1:
        raise ValueError("Unsupported license inventory schema")
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    expected = {(p["name"], p["version"], p.get("checksum")) for p in lock["package"] if p.get("source")}
    actual = [(p["name"], p["version"], p["checksum"]) for p in catalog["crates"]]
    if len(actual) != len(set(actual)) or set(actual) != expected:
        raise ValueError("Cargo.lock changed: review the third-party license inventory")
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    if digest(json.dumps(dependency_inputs(manifest), sort_keys=True).encode()) != catalog["inputs"]["dependencies_sha256"]:
        raise ValueError("Cargo dependencies/features changed: review license target coverage")
    if digest((root / "packaging/appimage-tools.json").read_bytes()) != catalog["inputs"]["appimage_tools_sha256"]:
        raise ValueError("AppImage tools changed: review bundled licenses and sources")
    workflow = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
    if (set(re.findall(r'toolchain: "([^"]+)"', workflow)) != {catalog["inputs"]["rust"]}
            or re.findall(r'PACKAGER_VERSION: "([^"]+)"', workflow) != [catalog["inputs"]["packager"]]):
        raise ValueError("Release toolchain changed: review runtime/installer notices")
    blockers = catalog["release_blockers"]
    blocker_ids = {item["id"] for item in blockers}
    if len(blocker_ids) != len(blockers) or any(not item["detail"].strip() for item in blockers):
        raise ValueError("Malformed license blockers")
    for key, text in texts.items():
        if not text.strip() or digest(text.encode()) != key:
            raise ValueError(f"Missing or altered notice text: {key}")
    for component in catalog["crates"] + catalog["components"]:
        if not component["license"] or not component["version"]:
            raise ValueError(f"Incomplete license inventory: {component['name']}")
        if component.get("unresolved") and component["unresolved"] not in blocker_ids:
            raise ValueError(f"Untracked license obligation: {component['name']}")
        required = bool(component.get("targets")) if "targets" in component else component["scope"] != "build only"
        if required and not component["notices"] and not component.get("unresolved"):
            raise ValueError(f"Missing notices: {component['name']}")
        for notice in component["notices"]:
            if notice["text"] not in texts or not notice["origin"]:
                raise ValueError(f"Missing notice/provenance: {component['name']}")
    for item in catalog["sources"] + catalog["files"]:
        path = evidence / item["path"]
        if (path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(evidence.resolve())
                or digest(path.read_bytes()) != item["sha256"]):
            raise ValueError(f"Missing or altered license/source evidence: {item['path']}")
    # This is the exact registry source compiled into the application, not an
    # online source offer that can expire or be replaced after distribution.
    mpl = next(p for p in catalog["crates"] if p["name"] == "option-ext")
    source = next((p for p in catalog["sources"] if p["path"] == f"sources/option-ext-{mpl['version']}.crate"), None)
    if source is None or source["sha256"] != mpl["checksum"]:
        raise ValueError("MPL source archive must match the locked option-ext crate")
    return catalog, texts


def materials(root=ROOT, evidence=EVIDENCE):
    catalog, texts = checked_catalog(root, evidence)
    introduction = (evidence / "NOTICE-INTRO.txt").read_text(encoding="utf-8")
    sections = [introduction, "\nCOMPONENT INVENTORY\n"]
    for component in catalog["crates"] + catalog["components"]:
        if "targets" in component and not component["targets"]:
            continue
        sections.append(f"\n{component['name']} {component['version']}\nLicense: {component['license']}\n")
        if component.get("authors"):
            sections.append("Upstream authors (Cargo metadata): " + ", ".join(component["authors"]) + "\n")
        if "targets" in component:
            sections.append("Targets: " + ", ".join(component["targets"]) + "\n")
        else:
            sections.append("Scope: " + component["scope"] + "\n")
        if component.get("unresolved"):
            sections.append("UNRESOLVED: " + component["unresolved"] + "\n")
        for notice in component["notices"]:
            sections.append(f"Notice {notice['text']}\nSource: {notice['origin']}\n")
    sections.append("\nUNRESOLVED RELEASE OBLIGATIONS\n")
    sections.extend(f"{item['id']}: {item['detail']}\n" for item in catalog["release_blockers"])
    sections.append("\nFULL NOTICE TEXTS (identified by SHA-256 above)\n")
    for key, text in sorted(texts.items()):
        sections.append(f"\n{'=' * 72}\nNotice {key}\n{'=' * 72}\n{text}\n")
    archive = io.BytesIO()
    # Source archives are already compressed. Store them without a second
    # compression pass, so verifier output is independent of the host zlib.
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED) as zipped:
        for name, data in [("README.txt", (evidence / "SOURCE-README.txt").read_bytes())] + [
                (item["path"].removeprefix("sources/"), (evidence / item["path"]).read_bytes()) for item in catalog["sources"]]:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3  # Identical ZIP metadata on Windows and Unix.
            info.compress_type = zipfile.ZIP_STORED
            info.external_attr = 0o100644 << 16
            zipped.writestr(info, data)
    return {FILES[0]: "".join(sections).encode(), FILES[1]: (json.dumps(catalog, indent=2, ensure_ascii=False) + "\n").encode(),
            FILES[2]: archive.getvalue(), FILES[3]: (evidence / FILES[3]).read_bytes()}


def prepare():
    catalog, _ = checked_catalog()
    compiler = subprocess.run(["rustc", "--version"], check=True, capture_output=True, text=True).stdout.split()
    if len(compiler) < 2 or compiler[:2] != ["rustc", catalog["inputs"]["rust"]]:
        raise ValueError("Packaging compiler differs from the audited Rust standard-library version")
    BUNDLE.mkdir(parents=True, exist_ok=True)
    for name, data in materials().items():
        (BUNDLE / name).write_bytes(data)
    return BUNDLE


def verify_directory(directory):
    for name, expected in materials().items():
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.read_bytes() != expected:
            raise ValueError(f"Final package omits or alters third-party material: {path}")


def verify_binaries(directory, expected):
    """Reject newly bundled native code until its inventory/notices are reviewed.

    Host-loaded system libraries are not redistributed. Symlink aliases to an
    inspected file are permitted; escaping/broken links are never skipped.
    """
    found = set()
    root = directory.resolve()
    for path in directory.rglob("*"):
        if path.is_symlink():
            resolved = path.resolve(strict=True)
            if not resolved.is_relative_to(root):
                raise ValueError(f"Package link escapes the audited payload: {path}")
            continue
        if not path.is_file():
            continue
        with path.open("rb") as source:
            magic = source.read(4)
        if magic == b"\x7fELF" or magic[:2] == b"MZ" or magic in (
                b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
                b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca", b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca"):
            found.add(path.relative_to(directory).as_posix())
    if found != set(expected):
        raise ValueError(f"Native payload differs from license audit: expected={sorted(expected)}, found={sorted(found)}")


def verify_linux_payload(payload, suffix):
    location = "usr/share/licenses/code-convoy" if suffix == ".rpm" else "usr/share/doc/code-convoy"
    verify_directory(payload / location)
    verify_binaries(payload, {"usr/bin/codeconvoy"})
    print(f"Final {suffix} payload: third-party notices, sources and native inventory verified", flush=True)


def verify_appimage_runtime(image, runtime):
    spec = json.loads((ROOT / "packaging/appimage-tools.json").read_text())["runtime"]
    original = runtime.read_bytes()
    if digest(original) != spec["sha256"]:
        raise ValueError("Unreviewed AppImage runtime")
    with image.open("rb") as source:
        actual = source.read(len(original))
    # The pinned 20251108 ELF reserves .digest_md5 at this offset (16 bytes).
    # appimagetool fills this field after assembling the final image. Every
    # other runtime byte must be the audited, checksum-pinned upstream binary.
    offset, size = 932096, 16
    if (len(original) != 944632 or original[offset:offset + size] != bytes(size)
            or actual[:offset] != original[:offset] or actual[offset + size:] != original[offset + size:]):
        raise ValueError("Final AppImage runtime differs from the reviewed binary")


def verify_windows_installer(archive, work, exe):
    catalog, _ = checked_catalog()
    with tempfile.TemporaryDirectory(prefix="final-installer-", dir=work) as temporary:
        payload = Path(temporary)
        subprocess.run(["7z", "x", "-y", f"-o{payload}", str(archive)], check=True, stdout=subprocess.PIPE)
        verify_directory(payload)
        if ((payload / "CodeConvoy.exe").read_bytes() != exe.read_bytes()
                or (payload / "LICENSE.txt").read_bytes() != (ROOT / "LICENSE").read_bytes()):
            raise ValueError("Final installer application/license differs from the staged files")
        plugins = catalog["windows_installer_plugins"]
        verify_binaries(payload, {"CodeConvoy.exe", "uninstall.exe", *plugins})
        for name, checksum in plugins.items():
            if digest((payload / name).read_bytes()) != checksum:
                raise ValueError(f"Final installer plugin differs from the license audit: {name}")
        print("Final Windows installer: notices, sources, application and embedded plugins verified", flush=True)


def require_release_ready():
    catalog, _ = checked_catalog()
    if catalog["release_blockers"]:
        raise ValueError("Third-party license release blockers remain: " + ", ".join(p["id"] for p in catalog["release_blockers"]))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "prepare", "release-ready", "verify"))
    parser.add_argument("directory", nargs="?", type=Path)
    args = parser.parse_args()
    if args.command == "check":
        inventory, _ = checked_catalog()
        print(f"License inventory checked: {len(inventory['crates'])} locked crates; {len(inventory['release_blockers'])} unresolved release obligations")
    elif args.command == "prepare":
        print(prepare())
    elif args.command == "release-ready":
        require_release_ready()
    elif args.directory is None:
        parser.error("verify requires the installed/extracted notice directory")
    else:
        verify_directory(args.directory)
