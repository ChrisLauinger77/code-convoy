"""Integrate the standalone, distributed runtime recipe with release packaging."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parent.parent


def checked_kit(root=ROOT, evidence=None):
    evidence = evidence or root / "third-party"
    kit = evidence / "runtime"
    pin = json.loads((root / "packaging/appimage-tools.json").read_text())["runtime"]
    data = (kit / "runtime-inputs.json").read_bytes()
    if hashlib.sha256(data).hexdigest() != pin["manifest_sha256"]:
        raise ValueError("Runtime build inputs changed: review provenance and relinking evidence")
    script = kit / "build-runtime.py"
    expected = next(s["sha256"] for s in json.loads(data)["recipe"] if s["file"] == script.name)
    source = script.read_bytes()
    if script.is_symlink() or hashlib.sha256(source).hexdigest() != expected:
        raise ValueError("Missing or modified runtime build script")
    # Execute only the verified source, never a timestamp-based cached .pyc.
    builder = types.ModuleType("codeconvoy_runtime_build")
    builder.__file__ = str(script)
    exec(compile(source, str(script), "exec"), builder.__dict__)
    checked = builder.verify(evidence / "sources", kit)
    source = checked["sources"]["type2-runtime"]
    if pin["source_revision"] != source["revision"] or pin["upstream_version"] != source["version"]:
        raise ValueError("Runtime source identity differs from its reviewed pin")
    return checked


def build_runtime(work):
    checked_kit()
    recipe = ROOT / "third-party/runtime/build-runtime.py"
    cache = ROOT / "target/appimage-runtime-inputs"
    args = [sys.executable, str(recipe), "--sources", str(ROOT / "third-party/sources"),
            "--cache", str(cache)]
    subprocess.run([*args, "fetch"], check=True)
    build = work / "runtime-build"
    subprocess.run([*args, "build", "--work", str(build)], check=True)
    runtime = build / "out/runtime"
    from release import checksum
    pin = json.loads((ROOT / "packaging/appimage-tools.json").read_text())["runtime"]
    if checksum(runtime) != pin["sha256"]:
        raise ValueError("Source-built AppImage runtime differs from the reviewed output")
    return runtime


def compare_builds(first, second):
    """Expensive builds are explicit; comparing their outputs is inexpensive."""
    if first.resolve() == second.resolve():
        raise ValueError("Reproducibility requires two separate clean build directories")
    checked_kit()
    pin = json.loads((ROOT / "packaging/appimage-tools.json").read_text())["runtime"]
    from release import checksum
    for directory in (first, second):
        audit = json.loads((directory / "audit.json").read_text())
        if (audit["manifest_sha256"] != pin["manifest_sha256"]
                or audit["runtime_sha256"] != pin["sha256"]
                or audit["recipient_patch_sha256"] is not None
                or checksum(directory / "runtime") != pin["sha256"]):
            raise ValueError("Stale, modified or unreviewed reproducibility evidence")
        for name, key in (("runtime.full", "full_sha256"), ("runtime.map", "map_sha256")):
            if checksum(directory / name) != audit[key]:
                raise ValueError("Modified runtime build evidence")
    for name in ("runtime", "runtime.full", "runtime.map", "audit.json"):
        if (first / name).read_bytes() != (second / name).read_bytes():
            raise ValueError(f"Runtime clean builds differ: {name}")
    print("Two clean runtime outputs and link audits are byte-identical")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    args = parser.parse_args()
    compare_builds(args.first, args.second)
