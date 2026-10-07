"""Connect a release tag to the complete, previously tested candidate packages."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

from release import check_tag, run, version


def capture(*args):
    return run(*args, stdout=subprocess.PIPE, text=True).stdout.strip()


def candidate_run(message):
    trailers = [line for line in message.splitlines() if line.startswith("Candidate-Run:")]
    if len(trailers) != 1 or not re.fullmatch(r"Candidate-Run: [1-9][0-9]*", trailers[0]):
        raise ValueError("Tag must contain exactly one Candidate-Run: <positive run ID> line")
    return trailers[0].split(" ")[1]


def verify_run(candidate, repository, commit):
    expected = {
        "event": "workflow_dispatch",
        "status": "completed",
        "conclusion": "success",
        "head_sha": commit,
        "path": ".github/workflows/release.yml",
        # A rerun can replace artifacts under the same run ID. Require a fresh
        # manual run so the annotation unambiguously identifies tested packages.
        "run_attempt": 1,
    }
    for key, value in expected.items():
        if candidate.get(key) != value:
            raise ValueError(f"Candidate run requires {key}={value!r}; found {candidate.get(key)!r}")
    for key in ("repository", "head_repository"):
        if candidate.get(key, {}).get("full_name") != repository:
            raise ValueError("Candidate run must originate in this repository")


def select_artifact(artifacts):
    matches = [artifact for artifact in artifacts if artifact["name"] == "release-assets"]
    if len(matches) != 1:
        raise ValueError("Candidate must contain exactly one release-assets artifact")
    artifact = matches[0]
    if artifact["expired"] or artifact["size_in_bytes"] <= 0:
        raise ValueError("Candidate release-assets is expired or empty; build and test a new candidate")
    return artifact["id"]


def resolve():
    tag = os.environ["RELEASE_TAG"]
    repository = os.environ["GH_REPO"]
    check_tag(tag)
    ref = f"refs/tags/{tag}"
    if capture("git", "cat-file", "-t", ref) != "tag":
        raise ValueError("Release requires an annotated tag with Candidate-Run metadata")
    tag_object = capture("git", "cat-file", "tag", ref)
    message = tag_object.split("\n\n", 1)[1]
    run_id = candidate_run(message)
    commit = capture("git", "rev-parse", f"{ref}^{{commit}}")
    candidate = json.loads(capture("gh", "api", f"repos/{repository}/actions/runs/{run_id}"))
    verify_run(candidate, repository, commit)
    pages = json.loads(capture(
        "gh", "api", "--paginate", "--slurp",
        f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100",
    ))
    artifact_id = select_artifact([artifact for page in pages for artifact in page["artifacts"]])
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"run_id={run_id}\nartifact_id={artifact_id}\n")


def commands():
    if os.environ["GITHUB_RUN_ATTEMPT"] != "1":
        raise ValueError("Start a new manual workflow run instead of rerunning a candidate")
    tag = f"v{version()}"
    commit = capture("git", "rev-parse", "HEAD")
    run_id = os.environ["GITHUB_RUN_ID"]
    text = (
        "After this run succeeds, download and test its complete release-assets artifact.\n"
        "When satisfied, run these commands in your local checkout:\n\n"
        "```sh\n"
        f"git tag -a {tag} {commit} -m \"CodeConvoy {version()}\" "
        f"-m \"Candidate-Run: {run_id}\"\n"
        f"git push origin {tag}\n"
        "```\n\n"
        "The tag publishes these existing packages without rebuilding. "
        "Publish before the artifact expires (retention: 30 days, subject to repository limits). "
        "For another candidate, start a new manual run; do not rerun this run.\n"
    )
    print(text, end="")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as summary:
        summary.write(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("resolve", "commands"))
    arguments = parser.parse_args()
    if arguments.command == "resolve":
        resolve()
    else:
        commands()
