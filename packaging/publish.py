"""Final tag-only CI step. A partial upload can leave a draft, never a public release."""
import json
import os
import subprocess

from release import DIST, ROOT, aggregate, artifact_names, check_tag, checksum, run, version

def main():
    tag = os.environ["RELEASE_TAG"]
    repository = os.environ["GH_REPO"]
    check_tag(tag)
    # Verify downloaded checksums before regenerating/validating the inventory.
    subprocess.run(["sha256sum", "--check", "SHA256SUMS"], cwd=DIST, check=True)
    aggregate(DIST)
    names = artifact_names(version()) + ["SHA256SUMS"]
    run("gh", "release", "create", tag, "--verify-tag", "--draft", "--title", f"CodeConvoy {tag}",
        "--notes-file", ROOT / "docs/release-notes.md", *[DIST / name for name in names])
    # The REST tag endpoint only returns published releases. gh also resolves
    # drafts by their pending tag, yielding the numeric ID needed by REST.
    response = run("gh", "release", "view", tag, "--repo", repository, "--json", "databaseId",
                   stdout=subprocess.PIPE, text=True)
    release_id = json.loads(response.stdout)["databaseId"]
    if type(release_id) is not int or release_id <= 0:
        raise ValueError("Draft release lookup did not return a valid release ID")
    endpoint = f"repos/{repository}/releases/{release_id}"
    response = run("gh", "api", endpoint, stdout=subprocess.PIPE, text=True)
    release = json.loads(response.stdout)
    if release["id"] != release_id or release["tag_name"] != tag or release["draft"] is not True:
        raise ValueError("Release lookup did not return the expected draft")
    assets = {asset["name"]: asset for asset in release["assets"]}
    if set(assets) != set(names):
        raise ValueError("Draft release does not contain the exact expected asset set")
    for name in names:
        asset = assets[name]
        if asset["size"] != (DIST / name).stat().st_size:
            raise ValueError(f"Uploaded asset has an incorrect size: {name}")
        digest = asset.get("digest")
        if digest and digest != "sha256:" + checksum(DIST / name):
            raise ValueError(f"Uploaded asset checksum mismatch: {name}")
    # Publish the same release that passed verification, without another tag lookup.
    run("gh", "api", endpoint, "--method", "PATCH", "-F", "draft=false", "-f", "make_latest=true")


if __name__ == "__main__":
    main()
