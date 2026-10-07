"""Candidate provenance must be verified before publishing existing binaries."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import candidate


class CandidateGates(unittest.TestCase):
    def test_annotation_requires_one_positive_run_id(self):
        self.assertEqual(candidate.candidate_run("CodeConvoy\n\nCandidate-Run: 123\n"), "123")
        for message in ("", "Candidate-Run: 0", "Candidate-Run: abc",
                        "Candidate-Run: 123\nCandidate-Run: 456", "Candidate-Run: 123 extra"):
            with self.subTest(message=message), self.assertRaises(ValueError):
                candidate.candidate_run(message)

    def test_candidate_requires_successful_original_manual_run_of_tagged_commit(self):
        data = dict(event="workflow_dispatch", status="completed", conclusion="success",
                    head_sha="abc", path=".github/workflows/release.yml", run_attempt=1,
                    repository={"full_name": "owner/repo"},
                    head_repository={"full_name": "owner/repo"})
        candidate.verify_run(data, "owner/repo", "abc")
        for key, value in dict(event="push", status="in_progress", conclusion="failure",
                               head_sha="different", path=".github/workflows/ci.yml", run_attempt=2,
                               repository={"full_name": "other/repo"},
                               head_repository={"full_name": "fork/repo"}).items():
            with self.subTest(key=key), self.assertRaises(ValueError):
                candidate.verify_run({**data, key: value}, "owner/repo", "abc")

    def test_artifact_requires_unique_nonexpired_complete_collection(self):
        artifact = dict(name="release-assets", id=42, expired=False, size_in_bytes=100)
        self.assertEqual(candidate.select_artifact([
            dict(name="packages-linux"), artifact,
        ]), 42)
        for artifacts in ([], [artifact, artifact], [{**artifact, "expired": True}],
                          [{**artifact, "size_in_bytes": 0}]):
            with self.subTest(artifacts=artifacts), self.assertRaises(ValueError):
                candidate.select_artifact(artifacts)

    def test_summary_and_log_pin_tag_to_exact_candidate_commit_and_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            summary = Path(temporary) / "summary.md"
            env = dict(GITHUB_RUN_ID="123", GITHUB_RUN_ATTEMPT="1", GITHUB_STEP_SUMMARY=str(summary))
            with patch.dict(os.environ, env), patch.object(candidate, "capture", return_value="abc"), \
                    patch.object(candidate, "version", return_value="0.2.0"), \
                    contextlib.redirect_stdout(io.StringIO()) as log:
                candidate.commands()
            text = summary.read_text()
            self.assertEqual(text, log.getvalue())
            self.assertIn('git tag -a v0.2.0 abc -m "CodeConvoy 0.2.0" -m "Candidate-Run: 123"', text)
            self.assertIn("git push origin v0.2.0", text)

    def test_rerun_does_not_print_a_publish_command(self):
        with patch.dict(os.environ, {"GITHUB_RUN_ATTEMPT": "2"}), self.assertRaises(ValueError):
            candidate.commands()

    def test_resolver_writes_verified_cross_run_download_outputs(self):
        data = dict(event="workflow_dispatch", status="completed", conclusion="success",
                    head_sha="abc", path=".github/workflows/release.yml", run_attempt=1,
                    repository={"full_name": "owner/repo"},
                    head_repository={"full_name": "owner/repo"})
        pages = [{"artifacts": [dict(name="packages-linux")]}, {"artifacts": [
            dict(name="release-assets", id=42, expired=False, size_in_bytes=100),
        ]}]
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            env = dict(RELEASE_TAG="v0.2.0", GH_REPO="owner/repo", GITHUB_OUTPUT=str(output))
            with patch.dict(os.environ, env), patch.object(candidate, "check_tag"), \
                    patch.object(candidate, "capture", side_effect=[
                        "tag", "object abc\ntype commit\n\nCodeConvoy\n\nCandidate-Run: 123",
                        "abc", json.dumps(data), json.dumps(pages),
                    ]) as capture:
                candidate.resolve()
            self.assertEqual(output.read_text(), "run_id=123\nartifact_id=42\n")
            self.assertIn(("gh", "api", "repos/owner/repo/actions/runs/123"),
                          [call.args for call in capture.call_args_list])

    def test_lightweight_tag_is_rejected_before_api_access(self):
        with patch.dict(os.environ, {"RELEASE_TAG": "v0.2.0", "GH_REPO": "owner/repo"}), \
                patch.object(candidate, "check_tag"), \
                patch.object(candidate, "capture", return_value="commit") as capture, \
                self.assertRaises(ValueError):
            candidate.resolve()
        capture.assert_called_once_with("git", "cat-file", "-t", "refs/tags/v0.2.0")


if __name__ == "__main__":
    unittest.main()
