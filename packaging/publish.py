"""Final tag-only CI step. A partial upload can leave a draft, never a public release."""
import json
import os
import subprocess

from release import DIST, ROOT, aggregate, artifact_names, check_tag, checksum, run, version

tag = os.environ["RELEASE_TAG"]
check_tag(tag)
# Verify downloaded checksums before regenerating/validating the inventory.
subprocess.run(["sha256sum", "--check", "SHA256SUMS"], cwd=DIST, check=True)
aggregate(DIST)
names = artifact_names(version()) + ["SHA256SUMS"]
run("gh", "release", "create", tag, "--verify-tag", "--draft", "--title", f"CodeConvoy {tag}",
    "--notes-file", ROOT / "docs/release-notes.md", *[DIST / name for name in names])
response = run("gh", "api", f"repos/{os.environ['GH_REPO']}/releases/tags/{tag}", capture_output=True, text=True)
release = json.loads(response.stdout)
assets = {asset["name"]: asset for asset in release["assets"]}
if set(assets) != set(names) or not release["draft"]:
    raise ValueError("Draft release does not contain the exact expected asset set")
for name in names:
    asset = assets[name]
    if asset["size"] != (DIST / name).stat().st_size:
        raise ValueError(f"Uploaded asset has an incorrect size: {name}")
    digest = asset.get("digest")
    if digest and digest != "sha256:" + checksum(DIST / name):
        raise ValueError(f"Uploaded asset checksum mismatch: {name}")
run("gh", "release", "edit", tag, "--draft=false", "--latest")
