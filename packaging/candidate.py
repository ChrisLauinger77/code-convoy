"""Describe and safely tag the complete candidate after release approval."""
import argparse
import os
from pathlib import Path
import re
import subprocess

from publish import preflight
from release import run, version


def capture(*args):
    return run(*args, stdout=subprocess.PIPE, text=True).stdout.strip()


def verify_tag(ref, tag, commit, run_id):
    """Only the same candidate's direct annotated commit tag may be reused."""
    error = f"Existing tag {tag} does not belong to this candidate; it will not be changed"
    if capture("git", "cat-file", "-t", ref) != "tag":
        raise ValueError(error)
    header, separator, message = capture("git", "cat-file", "tag", ref).partition("\n\n")
    fields = header.splitlines()
    trailers = [line for line in message.splitlines() if line.startswith("Candidate-Run:")]
    if (not separator or fields[:3] != [f"object {commit}", "type commit", f"tag {tag}"]
            or trailers != [f"Candidate-Run: {run_id}"]):
        raise ValueError(error)


def ensure_tag(tag, commit, run_id):
    ref = f"refs/tags/{tag}"
    try:
        capture("git", "show-ref", "--verify", "--quiet", ref)
        local_exists = True
    except subprocess.CalledProcessError as error:
        if error.returncode != 1:
            raise
        local_exists = False
    if local_exists:
        verify_tag(ref, tag, commit, run_id)
    try:
        remote = capture("git", "ls-remote", "--exit-code", "--refs", "origin", ref)
    except subprocess.CalledProcessError as error:
        if error.returncode != 2:
            raise
        remote = None
    if remote is not None:
        fields = remote.split()
        if len(fields) != 2 or fields[1] != ref:
            raise ValueError(f"Cannot identify origin's tag {tag}")
        # FETCH_HEAD inspects the remote object without replacing any local tag.
        run("git", "fetch", "--no-tags", "origin", ref)
        if capture("git", "rev-parse", "FETCH_HEAD") != fields[0]:
            raise ValueError(f"Origin's tag {tag} changed during verification")
        verify_tag("FETCH_HEAD", tag, commit, run_id)
        return "Reused"
    if not local_exists:
        run("git", "tag", "-a", tag, commit, "-m", f"CodeConvoy {version()}",
            "-m", f"Candidate-Run: {run_id}")
    # No force: a conflicting tag created during verification must reject this push.
    run("git", "push", "origin", ref)
    return "Pushed"


def tag_candidate():
    tag = os.environ["RELEASE_TAG"]
    commit = os.environ["SOURCE_SHA"]
    run_id = os.environ["CANDIDATE_RUN"]
    if os.environ["VERSION"] != version() or capture("git", "rev-parse", "HEAD") != commit:
        raise ValueError("Checkout does not match the validated version and commit")
    if not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise ValueError("Candidate run ID must be a positive integer")
    # Download/expiry checks happen in the workflow; all publication gates must
    # pass on those bytes before creating even a local release tag.
    preflight(tag)
    action = ensure_tag(tag, commit, run_id)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"tag={tag}\n")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as summary_file:
        summary_file.write(f"{action} annotated tag `{tag}` on `{commit}` "
                           f"with `Candidate-Run: {run_id}`.\n")


def summary():
    # A rerun can replace artifacts under the same run ID. Require a fresh
    # manual run so the Candidate-Run annotation identifies the tested packages.
    if os.environ["GITHUB_RUN_ATTEMPT"] != "1":
        raise ValueError("Start a new manual workflow run instead of rerunning a candidate")
    tag = f"v{version()}"
    commit = capture("git", "rev-parse", "HEAD")
    run_id = os.environ["GITHUB_RUN_ID"]
    text = (
        "Download and test this run's complete release-assets artifact.\n"
        "When satisfied, approve the `release` environment in **Review deployments**. Approval "
        f"creates the annotated tag `{tag}` on `{commit}` with `Candidate-Run: {run_id}`, "
        "then publishes these existing packages without rebuilding.\n\n"
        "To abandon this candidate, reject the deployment. "
        "Approve before the artifact expires (retention: 30 days, subject to repository limits). "
        "For another candidate, start a new manual run; do not rerun this run.\n"
    )
    print(text, end="")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as summary_file:
        summary_file.write(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("summary", "tag"), nargs="?", default="summary")
    arguments = parser.parse_args()
    if arguments.command == "tag":
        tag_candidate()
    else:
        summary()
