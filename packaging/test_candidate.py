"""Candidate approval and tagging must preserve tested bytes and immutable tags."""
import contextlib
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import candidate
import publish
import release


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


class CandidateTagging(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.checkout = self.root / "checkout"
        self.origin = self.root / "origin.git"
        self.dist = self.root / "dist"
        self.checkout.mkdir()
        self.dist.mkdir()
        self.version = release.version()
        self.tag = "v" + self.version
        self.ref = f"refs/tags/{self.tag}"
        self.output = self.root / "output"
        self.summary = self.root / "summary"
        env = dict(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                   GIT_AUTHOR_NAME="Candidate test", GIT_AUTHOR_EMAIL="test@example.invalid",
                   GIT_COMMITTER_NAME="Candidate test", GIT_COMMITTER_EMAIL="test@example.invalid",
                   VERSION=self.version, RELEASE_TAG=self.tag, CANDIDATE_RUN="123",
                   GITHUB_OUTPUT=str(self.output), GITHUB_STEP_SUMMARY=str(self.summary))
        patcher = patch.dict(os.environ, env)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.git("init", "--initial-branch=main")
        self.git("init", "--bare", "--initial-branch=main", str(self.origin))
        (self.checkout / "Cargo.toml").write_text(f'[package]\nversion = "{self.version}"\n')
        self.git("add", "Cargo.toml")
        self.git("commit", "-m", "Temporary candidate fixture")
        self.commit = self.git("rev-parse", "HEAD")
        os.environ["SOURCE_SHA"] = self.commit
        self.git("remote", "add", "origin", str(self.origin))
        self.git("push", "origin", "main")
        for name in release.artifact_names(self.version):
            (self.dist / name).write_bytes(name.encode())
        release.aggregate(self.dist)
        for patcher in (patch.object(release, "ROOT", self.checkout),
                        patch.object(publish, "DIST", self.dist),
                        patch.object(publish, "require_release_ready")):
            patcher.start()
            self.addCleanup(patcher.stop)

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.checkout, check=True,
                              capture_output=True, text=True).stdout.strip()

    def remote_tag(self):
        return self.git("ls-remote", "--refs", "origin", self.ref)

    def create_tag(self, message="Candidate-Run: 123", commit=None, *, push=False):
        self.git("tag", "-a", self.tag, commit or self.commit, "-m", message)
        if push:
            self.git("push", "origin", self.ref)

    def assert_no_tag(self):
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertEqual(self.remote_tag(), "")
        self.assertFalse(self.output.exists())

    def test_new_candidate_pushes_an_annotated_tag_and_outputs_it(self):
        candidate.tag_candidate()
        self.assertEqual(self.git("cat-file", "-t", self.ref), "tag")
        self.assertEqual(self.git("rev-parse", f"{self.ref}^{{commit}}"), self.commit)
        self.assertEqual(self.git("for-each-ref", "--format=%(contents)", self.ref),
                         f"CodeConvoy {self.version}\n\nCandidate-Run: 123")
        self.assertEqual(self.remote_tag().split()[0], self.git("rev-parse", self.ref))
        self.assertEqual(self.output.read_text(), f"tag={self.tag}\n")

    def test_retry_after_summary_failure_reuses_remote_tag_without_local_tag(self):
        with patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(self.root)}), self.assertRaises(OSError):
            candidate.tag_candidate()
        original = self.remote_tag()
        # A fresh runner need not have a local copy of the tag being recovered.
        retry = self.root / "retry"
        self.git("clone", "--no-tags", str(self.origin), str(retry))
        with patch.object(release, "ROOT", retry), \
                patch.dict(os.environ, {"GITHUB_RUN_ATTEMPT": "2"}):
            candidate.tag_candidate()
        self.assertEqual(self.remote_tag(), original)
        self.assertIn("Reused annotated tag", self.summary.read_text())

    def test_retry_pushes_existing_local_candidate_tag_without_recreating_it(self):
        self.create_tag()
        original = self.git("rev-parse", self.ref)
        candidate.tag_candidate()
        self.assertEqual(self.remote_tag().split()[0], original)
        self.assertEqual(self.git("rev-parse", self.ref), original)

    def test_retry_with_both_local_and_remote_candidate_tag_preserves_the_object(self):
        self.create_tag(push=True)
        original = self.remote_tag()
        candidate.tag_candidate()
        self.assertEqual(self.remote_tag(), original)
        self.assertEqual(self.git("rev-parse", self.ref), original.split()[0])
        self.assertIn("Reused annotated tag", self.summary.read_text())

    def test_unavailable_or_damaged_assets_never_create_a_tag(self):
        asset = self.dist / release.artifact_names(self.version)[0]
        original = asset.read_bytes()
        asset.unlink()
        with self.assertRaisesRegex(ValueError, "Release assets differ"):
            candidate.tag_candidate()
        self.assert_no_tag()
        asset.write_bytes(b"damaged candidate")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            candidate.tag_candidate()
        self.assert_no_tag()
        asset.write_bytes(original)
        manifest = self.dist / "SHA256SUMS"
        manifest.write_text(manifest.read_text().splitlines()[0] + "\n")
        with self.assertRaisesRegex(ValueError, "every release asset"):
            candidate.tag_candidate()
        self.assert_no_tag()

    def test_release_blocker_never_creates_a_tag(self):
        with patch.object(publish, "require_release_ready", side_effect=ValueError("release blocker")), \
                self.assertRaisesRegex(ValueError, "release blocker"):
            candidate.tag_candidate()
        self.assert_no_tag()

    def test_source_or_version_mismatch_never_creates_a_tag(self):
        for name, value in (("SOURCE_SHA", "0" * 40), ("VERSION", "999.0.0"),
                            ("RELEASE_TAG", "v999.0.0"), ("CANDIDATE_RUN", "123\n456")):
            with self.subTest(name=name), patch.dict(os.environ, {name: value}), self.assertRaises(ValueError):
                candidate.tag_candidate()
            self.assert_no_tag()

    def test_other_candidate_local_tag_is_rejected_without_pushing(self):
        self.create_tag("Candidate-Run: 456")
        original = self.git("rev-parse", self.ref)
        with self.assertRaisesRegex(ValueError, "does not belong to this candidate"):
            candidate.tag_candidate()
        self.assertEqual(self.git("rev-parse", self.ref), original)
        self.assertEqual(self.remote_tag(), "")

    def reject_remote_tag(self):
        original = self.remote_tag()
        # The object must be checked from origin, even if checkout fetched no tags.
        self.git("tag", "-d", self.tag)
        with self.assertRaisesRegex(ValueError, "does not belong to this candidate"):
            candidate.tag_candidate()
        self.assertEqual(self.remote_tag(), original)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertFalse(self.output.exists())

    def test_remote_tag_on_another_commit_is_rejected(self):
        other = self.git("commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "Other candidate")
        self.create_tag(commit=other, push=True)
        self.reject_remote_tag()

    def test_remote_tag_from_another_candidate_is_rejected(self):
        self.create_tag("Candidate-Run: 456", push=True)
        self.reject_remote_tag()

    def test_remote_tag_with_duplicate_provenance_is_rejected(self):
        self.create_tag("Candidate-Run: 123\nCandidate-Run: 123", push=True)
        self.reject_remote_tag()

    def test_lightweight_remote_tag_is_rejected(self):
        self.git("tag", self.tag)
        self.git("push", "origin", self.ref)
        self.reject_remote_tag()

    def test_nested_remote_tag_is_rejected_even_when_it_peels_to_the_commit(self):
        self.git("tag", "-a", "inner", self.commit, "-m", "Inner tag")
        self.create_tag(commit="inner", push=True)
        self.assertEqual(self.git("rev-parse", f"{self.ref}^{{commit}}"), self.commit)
        original = self.remote_tag()
        with self.assertRaisesRegex(ValueError, "does not belong to this candidate"):
            candidate.tag_candidate()
        self.assertEqual(self.remote_tag(), original)

    def test_conflicting_tag_created_just_before_push_is_not_overwritten(self):
        actual_run = candidate.run

        def race(*args, **kwargs):
            if args[:2] == ("git", "push"):
                self.git("--git-dir", str(self.origin), "update-ref", self.ref, self.commit)
            return actual_run(*args, **kwargs)

        with patch.object(candidate, "run", side_effect=race), self.assertRaises(subprocess.CalledProcessError):
            candidate.tag_candidate()
        self.assertEqual(self.remote_tag().split()[0], self.commit)
        self.assertFalse(self.output.exists())

    def test_unreachable_origin_never_creates_a_tag(self):
        self.git("remote", "set-url", "origin", str(self.root / "missing.git"))
        with self.assertRaises(subprocess.CalledProcessError):
            candidate.tag_candidate()
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
