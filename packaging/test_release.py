"""Tests for gates that must fail before any public release is created."""
import hashlib
from pathlib import Path
import tempfile
import unittest

import release


class ReleaseGates(unittest.TestCase):
    def test_tag_must_match_cargo_exactly(self):
        release.check_tag("v" + release.version())
        for tag in (None, release.version(), "v999.0.0", "v" + release.version() + "-alpha"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.check_tag(tag)

    def test_universal_requires_both_architectures(self):
        release.verify_architectures("x86_64 arm64\n")
        for architectures in ("arm64", "x86_64", "", "arm64 x86_64 i386"):
            with self.subTest(architectures=architectures), self.assertRaises(ValueError):
                release.verify_architectures(architectures)

    def test_complete_assets_produce_exact_final_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in release.artifact_names(release.version()):
                (directory / name).write_bytes(name.encode())
            release.aggregate(directory)
            lines = (directory / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(lines), 6)
            for line in lines:
                digest, name = line.split("  ")
                self.assertEqual(digest, hashlib.sha256(name.encode()).hexdigest())
            release.aggregate(directory)  # A previous checksum file is not an extra asset.

    def test_missing_empty_and_extra_assets_fail_closed(self):
        names = release.artifact_names(release.version())
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in names:
                (directory / name).write_bytes(b"package")
            (directory / names[0]).unlink()
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            (directory / names[0]).touch()
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            (directory / names[0]).write_bytes(b"package")
            (directory / "debug.log").write_text("not distributable")
            with self.assertRaises(ValueError):
                release.aggregate(directory)
            self.assertFalse((directory / "SHA256SUMS").exists())


if __name__ == "__main__":
    unittest.main()
