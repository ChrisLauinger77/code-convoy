#!/usr/bin/env python3
"""Build the CodeConvoy Type 2 runtime from an extracted source kit (Linux x86-64).

Only `fetch` accesses the network. `build` requires bubblewrap and creates a new
offline root filesystem; it never installs packages on the host. See
runtime-README.txt. This script is also shipped inside THIRD-PARTY-SOURCES.zip.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import urllib.request

KIT = Path(__file__).resolve().parent
COMPONENTS = {"type2-runtime", "libfuse", "squashfuse", "musl", "zstd", "zlib", "mimalloc"}
LINK_INPUTS = {"rcrt1.o", "crti.o", "crtbeginS.o", "runtime.o", "libsquashfuse.a",
               "libsquashfuse_ll.a", "libzstd.a", "libz.a", "libfuse3.a", "libmimalloc.a",
               "libssp_nonshared.a", "libgcc.a", "libgcc_eh.a", "libc.a", "crtendS.o", "crtn.o"}
SYSTEM_ARCHIVES = {"libssp_nonshared.a", "libgcc.a", "libgcc_eh.a"}


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checked_file(root, spec):
    name = spec["file"]
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", name):
        raise ValueError(f"Unsafe input filename: {name}")
    path = root / name
    if (not re.fullmatch(r"[a-f0-9]{64}", spec["sha256"])
            or path.is_symlink() or not path.is_file() or sha(path) != spec["sha256"]):
        raise ValueError(f"Missing or modified runtime input: {name}")
    return path


def manifest(kit=KIT):
    spec = json.loads((kit / "runtime-inputs.json").read_text())
    if (spec["schema"] != 1 or spec["architecture"] != "x86_64"
            or set(spec["sources"]) != COMPONENTS
            or not re.fullmatch(r"[a-f0-9]{40}", spec["sources"]["type2-runtime"]["revision"])
            or spec["source_date_epoch"] != 1762560000
            or set(spec["toolchain_objects"]) != {"crtbeginS.o", "crtendS.o"}):
        raise ValueError("Incomplete runtime build-input manifest")
    packages = spec["packages"]
    names = [p["pkgname"] for p in packages]
    if len(names) != len(set(names)) or not {"gcc", "binutils", "musl-dev", "linux-headers", "meson", "cmake"} <= set(names):
        raise ValueError("Incomplete runtime toolchain closure")
    for p in packages:
        if (p["file"] != f"{p['pkgname']}-{p['pkgver']}.apk"
                or not re.fullmatch(r"[a-f0-9]{40}", p["commit"])
                or not p["license"] or not p["origin"]):
            raise ValueError("Unidentified build-tool package")
    for item in [spec["image"], *packages]:
        if not item["url"].startswith("https://") or not re.fullmatch(r"[a-f0-9]{64}", item["sha256"]):
            raise ValueError("Unpinned runtime build image/package")
    return spec


def verify(sources, kit=KIT):
    spec = manifest(kit)
    required = {"build-runtime.py", "build-runtime.sh", "runtime-README.txt",
                "libfuse-notice.patch", "libfuse-demo.patch", "gcc-14.2.0-alpine-r4.tar.gz",
                "runtime-build-LICENSE.txt"}
    if {r["file"] for r in spec["recipe"]} != required or len(spec["recipe"]) != len(required):
        raise ValueError("Incomplete runtime relinking recipe")
    for item in spec["recipe"]:
        checked_file(kit, item)
    if {s["file"] for s in spec["compiler_sources"]} != {
            "gcc-14.2.0-crtstuff.c", "gcc-14.2.0-COPYING3", "gcc-14.2.0-COPYING.RUNTIME"}:
        raise ValueError("Incomplete compiler startup source/notices")
    for item in spec["compiler_sources"]:
        checked_file(sources, item)
    headers = spec["linux_headers"]
    if headers["version"] != next(p["pkgver"] for p in spec["packages"] if p["pkgname"] == "linux-headers"):
        raise ValueError("Linux UAPI header/package identity mismatch")
    checked_file(sources, headers)
    for name, item in spec["sources"].items():
        path = checked_file(sources, item)
        if not item["version"] or not item["license"] or not item["origin"] or not item["directory"]:
            raise ValueError(f"Unidentified static-library source: {name}")
        identity = item["revision"] if name == "type2-runtime" else item["version"]
        prefix = "fuse" if name == "libfuse" else name
        if item["directory"] != prefix + "-" + identity:
            raise ValueError(f"Static-library version/source identity mismatch: {name}")
        with tarfile.open(path) as archive:
            if not all(archive.getmember(item["directory"] + "/" + notice).isfile()
                       for notice in item["notices"]):
                raise ValueError(f"Missing runtime source notices: {name}")
            if not item["notices"]:
                raise ValueError(f"Missing runtime source notices: {name}")
            if name == "type2-runtime":
                patch = archive.extractfile(item["directory"] + "/patches/libfuse/mount.c.diff").read()
                if hashlib.sha256(patch).hexdigest() != spec["upstream_patch_sha256"]:
                    raise ValueError("Modified upstream libfuse patch")
    return spec


def fetch(cache, spec):
    cache.mkdir(parents=True, exist_ok=True)
    for item in [spec["image"], *spec["packages"]]:
        path = cache / item["file"]
        if not path.exists():
            temporary = path.with_suffix(path.suffix + ".partial")
            print(f"Fetch {item['file']}", flush=True)
            with urllib.request.urlopen(item["url"], timeout=120) as source, temporary.open("wb") as output:
                shutil.copyfileobj(source, output)
            if sha(temporary) != item["sha256"]:
                raise ValueError(f"Downloaded runtime input checksum mismatch: {item['file']}")
            temporary.replace(path)
        checked_file(cache, item)


def build(sources, cache, work, recipient_patch=None):
    spec = verify(sources)
    for item in [spec["image"], *spec["packages"]]:
        checked_file(cache, item)
    if os.uname().machine != "x86_64" or not shutil.which("bwrap"):
        raise ValueError("Runtime builds require x86-64 Linux and bubblewrap (user namespaces enabled)")
    work.mkdir()  # Never reuse or remove an existing build directory.
    root = work / "root"
    root.mkdir()
    with tarfile.open(cache / spec["image"]["file"]) as image:
        # Verified Alpine rootfs contains absolute internal symlinks. This is the
        # only trusted image extraction; source extraction uses --no-same-owner.
        image.extractall(root, filter="tar")
    for name in ("inputs", "sources", "kit", "out", "build"):
        (root / name).mkdir(exist_ok=True)
    recipe = work / "recipe"
    recipe.mkdir()
    for item in spec["recipe"]:
        shutil.copyfile(KIT / item["file"], recipe / item["file"])
    shutil.copyfile(KIT / "runtime-inputs.json", recipe / "runtime-inputs.json")
    if recipient_patch:
        shutil.copyfile(recipient_patch, recipe / "recipient.patch")
    output = work / "out"
    output.mkdir()
    command = ["bwrap", "--unshare-all", "--die-with-parent", "--uid", "0", "--gid", "0",
               "--clearenv", "--setenv", "PATH", "/usr/sbin:/usr/bin:/sbin:/bin",
               "--setenv", "HOME", "/root", "--setenv", "LC_ALL", "C", "--setenv", "TZ", "UTC",
               "--bind", str(root), "/", "--dev", "/dev", "--proc", "/proc",
               "--ro-bind", str(cache), "/inputs", "--ro-bind", str(sources), "/sources",
               "--ro-bind", str(recipe), "/kit", "--bind", str(output), "/out", "--chdir", "/"]
    with (work / "build.log").open("w") as log:
        subprocess.run(command + ["/sbin/apk", "add", "--no-network", "--no-chown",
                                  *["/inputs/" + p["file"] for p in spec["packages"]]],
                       check=True, stdout=log, stderr=subprocess.STDOUT)
        subprocess.run(command + ["/bin/sh", "/kit/build-runtime.sh"], check=True,
                       stdout=log, stderr=subprocess.STDOUT)
    print(f"Built {output / 'runtime'}: {sha(output / 'runtime')}")


def capture(output):
    """Inside the offline build: identify every linker input and selected member."""
    spec = manifest()
    inputs = {}
    for line in (output / "link-inputs.txt").read_text().splitlines():
        path = Path(line).resolve(strict=True)
        if path.name in inputs and inputs[path.name]["sha256"] != sha(path):
            raise ValueError("Ambiguous runtime linker input")
        inputs[path.name] = {"path": str(path), "sha256": sha(path)}
    if set(inputs) != LINK_INPUTS:
        raise ValueError(f"Unexpected runtime link graph: {set(inputs) ^ LINK_INPUTS}")
    for name, expected in spec["toolchain_objects"].items():
        if inputs[name]["sha256"] != expected:
            raise ValueError(f"Unreviewed compiler startup object: {name}")
    map_text = (output / "runtime.map").read_text()
    members = sorted(set(re.findall(r"^(/\S+\.a\([^)]+\))", map_text, re.M)))
    # These compiler/system archives are searched but not selected in this C
    # runtime. New member selections require source/notice review, not a waiver.
    if any(Path(m.split("(")[0]).name in SYSTEM_ARCHIVES for m in members):
        raise ValueError("New compiler/system archive code requires provenance review")
    dynamic = subprocess.check_output(["readelf", "-dW", output / "runtime"], text=True)
    headers = subprocess.check_output(["readelf", "-lW", output / "runtime"], text=True)
    if "(NEEDED)" in dynamic or "INTERP" in headers:
        raise ValueError("Replacement runtime must remain entirely static")
    tools = {}
    for tool in ("gcc", "ld", "as", "ar", "strip", "make", "meson", "cmake", "python3"):
        path = Path(shutil.which(tool)).resolve()
        tools[tool] = {"path": str(path), "sha256": sha(path),
                       "version": subprocess.check_output([tool, "--version"], text=True).splitlines()[0]}
    result = {"schema": 1, "manifest_sha256": sha(KIT / "runtime-inputs.json"),
              "runtime_sha256": sha(output / "runtime"), "full_sha256": sha(output / "runtime.full"),
              "map_sha256": sha(output / "runtime.map"), "link_inputs": inputs,
              "archive_members": members, "tools": tools,
              "recipient_patch_sha256": sha(KIT / "recipient.patch") if (KIT / "recipient.patch").exists() else None,
              "compiler_process": "GCC 14.2.0-r4, C source, no plugins or LTO; GNU binutils 2.43.1-r3"}
    (output / "audit.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("verify", "fetch", "build", "capture"))
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument("--sources", type=Path, default=KIT)
    parser.add_argument("--cache", type=Path, default=Path("runtime-input-cache"))
    parser.add_argument("--work", type=Path)
    parser.add_argument("--recipient-patch", type=Path)
    args = parser.parse_args()
    if args.command == "capture":
        capture(args.output)
        return
    spec = verify(args.sources.resolve(strict=True))
    if args.command == "fetch":
        fetch(args.cache.resolve(), spec)
    elif args.command == "build":
        if args.work is None:
            parser.error("build requires --work pointing to a new directory")
        build(args.sources.resolve(strict=True), args.cache.resolve(strict=True), args.work.resolve(), args.recipient_patch)
    else:
        print("Runtime source identities, patches and relinking recipe verified")


if __name__ == "__main__":
    main()
