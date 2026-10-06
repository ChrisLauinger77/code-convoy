"""Publisher regression tests using local assets and a simulated GitHub API."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import publish
import release


class PublisherGates(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.tag = "v" + release.version()
        self.repository = "owner/code-convoy"
        self.release_id = 42
        self.endpoint = f"repos/{self.repository}/releases/{self.release_id}"
        self.published = False
        for name in release.artifact_names(release.version()):
            (self.directory / name).write_bytes(name.encode())
        release.aggregate(self.directory)
        self.remote = {
            "id": self.release_id,
            "tag_name": self.tag,
            "draft": True,
            "assets": [{"name": path.name, "size": path.stat().st_size,
                        "digest": "sha256:" + release.checksum(path)}
                       for path in self.directory.iterdir()],
        }
        for patcher in (patch.object(publish, "DIST", self.directory),
                        patch.dict(publish.os.environ, {"RELEASE_TAG": self.tag, "GH_REPO": self.repository})):
            patcher.start()
            self.addCleanup(patcher.stop)
        command_patcher = patch.object(publish, "run", side_effect=self.github)
        self.commands = command_patcher.start()
        self.addCleanup(command_patcher.stop)
        checksum_patcher = patch.object(publish.subprocess, "run")
        self.checksum_command = checksum_patcher.start()
        self.addCleanup(checksum_patcher.stop)

    def github(self, *args, **kwargs):
        if args[:3] == ("gh", "release", "create"):
            self.assertIn("--draft", args)
            return subprocess.CompletedProcess(args, 0)
        if args[:3] == ("gh", "release", "view"):
            return subprocess.CompletedProcess(args, 0, json.dumps({"databaseId": self.release_id}))
        if args[:2] == ("gh", "api"):
            # Reproduce GitHub's published-only tag endpoint: the old publisher
            # would receive 404 here despite the complete draft existing.
            if "/releases/tags/" in args[2]:
                raise subprocess.CalledProcessError(1, args, stderr="gh: Not Found (HTTP 404)")
            self.assertEqual(args[2], self.endpoint)
            if "--method" in args:
                self.assertEqual(args[3:], ("--method", "PATCH", "-F", "draft=false", "-f", "make_latest=true"))
                self.published = True
            return subprocess.CompletedProcess(args, 0, json.dumps(self.remote))
        self.fail(f"Unexpected GitHub command: {args}")

    def test_complete_draft_is_verified_and_published_by_id(self):
        publish.main()
        self.assertTrue(self.published)
        self.checksum_command.assert_called_once_with(
            ["sha256sum", "--check", "SHA256SUMS"], cwd=self.directory, check=True)

    def test_wrong_release_or_non_draft_is_never_published(self):
        for key, value in (("id", 99), ("tag_name", "v999.0.0"), ("draft", False)):
            with self.subTest(key=key), patch.dict(self.remote, {key: value}), self.assertRaises(ValueError):
                publish.main()
            self.assertFalse(self.published)

    def test_incomplete_or_mismatched_uploads_are_never_published(self):
        original = copy.deepcopy(self.remote["assets"])
        wrong_size = copy.deepcopy(original)
        wrong_size[0]["size"] += 1
        wrong_digest = copy.deepcopy(original)
        wrong_digest[0]["digest"] = "sha256:" + "0" * 64
        for assets in (original[:-1], original + [{"name": "unexpected.log"}], wrong_size, wrong_digest):
            with self.subTest(assets=assets), patch.dict(self.remote, {"assets": assets}), self.assertRaises(ValueError):
                publish.main()
            self.assertFalse(self.published)

    def test_invalid_release_id_is_never_published(self):
        for value in (None, True, 0, -1, "42"):
            with self.subTest(value=value):
                self.release_id = value
                with self.assertRaises(ValueError):
                    publish.main()
                self.assertFalse(self.published)

    def test_failed_checksum_or_upload_prevents_publication(self):
        self.checksum_command.side_effect = subprocess.CalledProcessError(1, ["sha256sum"])
        with self.assertRaises(subprocess.CalledProcessError):
            publish.main()
        self.commands.assert_not_called()
        self.checksum_command.side_effect = None
        self.commands.side_effect = subprocess.CalledProcessError(1, ["gh", "release", "create"])
        with self.assertRaises(subprocess.CalledProcessError):
            publish.main()
        self.assertEqual(self.commands.call_count, 1)
        self.assertFalse(self.published)


if __name__ == "__main__":
    unittest.main()
