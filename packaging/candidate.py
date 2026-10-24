"""Describe the approval that tags and publishes this complete, tested candidate."""
import os
from pathlib import Path
import subprocess

from release import run, version


def capture(*args):
    return run(*args, stdout=subprocess.PIPE, text=True).stdout.strip()


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
    summary()
