"""Routine dependency updates should trigger obligation review, not audit churn."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import licenses


class DependencyPolicy(unittest.TestCase):
    def setUp(self):
        self.crate = {"id": "registry+fixture#dependency@2.0.0", "name": "dependency", "version": "2.0.0",
                      "source": "registry+fixture", "license": "MIT OR Apache-2.0"}
        self.metadata = {"packages": [{"id": "root"}, self.crate],
                         "resolve": {"root": "root", "nodes": [{"id": "root"}, {"id": self.crate["id"]}]}}
        self.locked = {key: self.crate[key] for key in ("name", "version", "source")}
        self.locked["checksum"] = "a" * 64
        self.lock = {"package": [self.locked]}
        self.catalog = {"crates": []}

    def check(self):
        licenses.check_dependencies(self.metadata, self.catalog, self.lock)

    def test_new_and_updated_permissive_crates_need_no_catalog_edits(self):
        self.check()  # Entirely new crate, with no inventory entry.
        self.catalog["crates"] = [{**self.locked, "version": "1.0.0", "checksum": "b" * 64,
                                   "license": self.crate["license"]}]
        self.check()  # Routine version/checksum drift needs no review.

    def test_new_or_changed_obligations_fail_with_affected_crate(self):
        self.catalog["crates"] = [{**self.locked, "license": self.crate["license"]}]
        for expression in ("GPL-3.0-only", "LGPL-2.1-or-later", "MPL-2.0", "AGPL-3.0-only",
                           "BUSL-1.1", "LicenseRef-Custom", "MIT AND LicenseRef-Restricted",
                           "(MIT OR Apache-2.0) AND OFL-1.1", None, ""):
            self.crate["license"] = expression
            with self.subTest(expression=expression), self.assertRaisesRegex(ValueError, "dependency 2.0.0.*obligations"):
                self.check()

    def test_copyleft_review_is_bound_to_license_source_version_and_checksum(self):
        self.crate["license"] = "MPL-2.0"
        self.catalog["crates"] = [{**self.locked, "license": "MPL-2.0"}]
        self.check()
        original = copy.deepcopy(self.catalog["crates"][0])
        for key, value in (("version", "1.0.0"), ("checksum", "b" * 64),
                           ("source", "registry+another"), ("license", "MIT")):
            self.catalog["crates"][0] = {**original, key: value}
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "obligations"):
                self.check()

    def test_git_and_local_dependencies_cannot_bypass_review_with_permissive_metadata(self):
        for source in ("git+fixture#revision", None):
            self.crate["source"] = source
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, "checksum"):
                self.check()

    def test_filtered_or_missing_dependency_metadata_cannot_silently_pass(self):
        self.metadata["packages"].pop()
        with self.assertRaisesRegex(ValueError, "complete resolved"):
            self.check()
        self.metadata["resolve"] = None
        with self.assertRaisesRegex(ValueError, "including dependencies"):
            self.check()

    def test_spdx_choices_conjunctions_exceptions_and_legacy_slashes(self):
        for expression in ("MIT/Apache-2.0", "Apache-2.0 / MIT", "Unlicense",
                           "(MIT OR Apache-2.0) AND Unicode-3.0", "Apache-2.0 WITH LLVM-exception",
                           "Apache-2.0 OR GPL-2.0-only", "MIT OR Apache-2.0 OR LGPL-2.1-or-later",
                           "(GPL-3.0-only AND LicenseRef-Custom) OR MIT"):
            with self.subTest(expression=expression):
                self.assertTrue(licenses.notice_only(expression))
        for expression in ("", "MIT OR", "MIT AND", "(MIT", "MIT)", "MIT + Apache-2.0", "MIT AND OR MIT",
                           "MIT WITH", "MIT WITH ( OR MIT", "MIT WITH Unknown-exception", "MIT MIT",
                           "MIT AND (Apache-2.0 OR MPL-2.0) AND LGPL-2.1-only"):
            with self.subTest(expression=expression):
                self.assertFalse(licenses.notice_only(expression))

    def test_ci_reads_current_all_target_all_feature_metadata_without_modifying_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text('[[package]]\nname="dependency"\nversion="2.0.0"\n'
                                            'source="registry+fixture"\nchecksum="' + "a" * 64 + '"\n')
            result = subprocess.CompletedProcess([], 0, json.dumps(self.metadata))
            with patch.object(licenses, "checked_catalog", return_value=(self.catalog, {})), \
                    patch.object(licenses.subprocess, "run", return_value=result) as run:
                licenses.current_dependencies(root, root)
            self.assertEqual(run.call_args.args[0],
                             ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"])
            self.assertEqual(list(root.iterdir()), [root / "Cargo.lock"])


if __name__ == "__main__":
    unittest.main()
