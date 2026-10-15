"""Offline, reviewed third-party evidence and final-package omission gates."""
import argparse
import hashlib
import io
import json
import os
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


def source_bundle(entries):
    """Canonical ZIP bytes, independent of host metadata, locale and zlib.

    Entry names are deliberately portable ASCII basenames. Source archives keep
    their original bytes (including their own upstream metadata and licenses).
    """
    entries = list(entries)
    names = [name for name, _ in entries]
    if (any(not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name) or name.endswith(".")
            or re.fullmatch(r"(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?", name, re.I)
            for name in names)
            or len({name.lower() for name in names}) != len(names)):
        raise ValueError("Source bundle names must be unique portable basenames")
    archive = io.BytesIO()
    # ZIP_STORED avoids a dependency on the host's compression implementation.
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED, allowZip64=False) as zipped:
        zipped.comment = b""
        for name, data in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.create_version = info.extract_version = 20
            info.compress_type = zipfile.ZIP_STORED
            info.external_attr = 0o100644 << 16
            info.internal_attr = info.flag_bits = info.volume = 0
            info.extra = info.comment = b""
            zipped.writestr(info, data)
    return archive.getvalue()


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
    native = catalog["native_provenance"]
    runtime = json.loads((root / "packaging/appimage-tools.json").read_text())["runtime"]
    if native["appimage"]["runtime_sha256"] != runtime["sha256"]:
        raise ValueError("AppImage version provenance does not match the pinned runtime")
    for name, version in native["appimage"]["versions"].items():
        component = next((c for c in catalog["components"] if c["name"] == name), None)
        if (component is None or component["version"].split()[0] != version
                or not any(s["path"] == component.get("source") for s in catalog["sources"])):
            raise ValueError(f"Missing AppImage version/source evidence: {name}")
    startup = native["appimage"]["compiler_startup"]
    component = next((c for c in catalog["components"] if c["name"] == startup["component"]), None)
    if (component is None or not startup["reference_notices"] or not startup["reference_sources"]
            or any(n not in component["notices"] for n in startup["reference_notices"])
            or any(not any(s["path"] == path for s in catalog["sources"])
                   for path in startup["reference_sources"])):
        raise ValueError("Missing AppImage compiler startup source/license evidence")
    plugin = native["nsis_tauri_utils"]
    if plugin["dll_sha256"] != catalog["windows_installer_plugins"]["$PLUGINSDIR/nsis_tauri_utils.dll"]:
        raise ValueError("Plugin provenance does not match the pinned installer DLL")
    for package in plugin["visible_registry_packages"]:
        if not any(c["name"] == "nsis-tauri-utils embedded: " + package["name"]
                   and c["version"] == package["version"] and c.get("checksum") == package["checksum"]
                   and c["notices"] for c in catalog["components"]):
            raise ValueError(f"Missing observed installer dependency notices: {package['name']}")
    # This is the exact registry source compiled into the application, not an
    # online source offer that can expire or be replaced after distribution.
    mpl = next(p for p in catalog["crates"] if p["name"] == "option-ext")
    source = next((p for p in catalog["sources"] if p["path"] == f"sources/option-ext-{mpl['version']}.crate"), None)
    if source is None or source["sha256"] != mpl["checksum"]:
        raise ValueError("MPL source archive must match the locked option-ext crate")
    # Rust's generated report is not the library's dependency lockfile. Keep
    # that second inventory complete as well, including explicit target exclusions.
    library_lock = evidence / "sources" / f"rust-{catalog['inputs']['rust']}-library-Cargo.lock"
    libraries = [p for p in tomllib.loads(library_lock.read_text())["package"] if p.get("source")]
    identities = lambda packages: {(p["name"], p["version"], p["checksum"]) for p in packages}
    if (identities(libraries) != identities(catalog["rust_library_packages"])
            or len(catalog["rust_library_packages"]) != len(libraries)):
        raise ValueError("Rust standard-library dependency inventory is incomplete")
    covered = {(n["origin"], n["text"]) for c in catalog["components"] for n in c["notices"]}
    if not catalog["rust_source_notices"] or any((n["origin"], n["text"]) not in covered for n in catalog["rust_source_notices"]):
        raise ValueError("Missing Rust in-tree library notices")
    for package in catalog["rust_library_packages"]:
        if package.get("audit_scope"):
            if package["name"] not in {"fortanix-sgx-abi", "vex-sdk", "wasip1"}:
                raise ValueError("Unreviewed Rust standard-library target exclusion")
            continue
        if not any(c["name"] == "Rust std dependency: " + package["name"]
                   and c["version"] == package["version"] and c.get("checksum") == package["checksum"]
                   and c["notices"] for c in catalog["components"]):
            raise ValueError(f"Missing Rust standard-library notice coverage: {package['name']}")
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
    archive = source_bundle([("README.txt", (evidence / "SOURCE-README.txt").read_bytes())] + [
        (item["path"].removeprefix("sources/"), (evidence / item["path"]).read_bytes()) for item in catalog["sources"]])
    return {FILES[0]: "".join(sections).encode(), FILES[1]: (json.dumps(catalog, indent=2, ensure_ascii=False) + "\n").encode(),
            FILES[2]: archive, FILES[3]: (evidence / FILES[3]).read_bytes()}


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
        if os.name == "nt":
            from windows_crt import inspect_installer
            inspect_installer(archive, payload, exe)
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
