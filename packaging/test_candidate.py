"""The candidate summary must pin the approved tag to the exact tested commit and run."""
import contextlib
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import candidate


class CandidateGates(unittest.TestCase):
    def test_summary_and_log_pin_approval_to_exact_candidate_commit_and_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            summary = Path(temporary) / "summary.md"
            env = dict(GITHUB_RUN_ID="123", GITHUB_RUN_ATTEMPT="1", GITHUB_STEP_SUMMARY=str(summary))
            with patch.dict(os.environ, env), patch.object(candidate, "capture", return_value="abc") as capture, \
                    patch.object(candidate, "version", return_value="0.2.0"), \
                    contextlib.redirect_stdout(io.StringIO()) as log:
                candidate.summary()
            text = summary.read_text()
            self.assertEqual(text, log.getvalue())
            capture.assert_called_once_with("git", "rev-parse", "HEAD")
            self.assertIn("approve the `release` environment", text)
            self.assertIn("annotated tag `v0.2.0` on `abc` with `Candidate-Run: 123`", text)
            self.assertNotIn("git push", text)

    def test_rerun_does_not_offer_approval(self):
        with tempfile.TemporaryDirectory() as temporary:
            summary = Path(temporary) / "summary.md"
            env = dict(GITHUB_RUN_ID="123", GITHUB_RUN_ATTEMPT="2", GITHUB_STEP_SUMMARY=str(summary))
            with patch.dict(os.environ, env), self.assertRaises(ValueError):
                candidate.summary()
            self.assertFalse(summary.exists())


if __name__ == "__main__":
    unittest.main()
