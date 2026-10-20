"""Keep embedded-font attribution in collected release evidence."""
from pathlib import Path
import tempfile
import unittest

from collect_licenses import notice_files


class FontAttribution(unittest.TestCase):
    def test_collects_nested_font_attribution_and_excludes_unrelated_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fonts = root / "fonts"
            fonts.mkdir()
            expected = {
                "fonts/Hack-Regular.txt": "Hack attribution\n",
                "fonts/egui-icons.txt": "Derived from emoji-icon-font; preserve its MIT notice.\n",
                "fonts/emoji-icon-font-mit-license.txt": "Original MIT attribution\n",
                "fonts/OFL.txt": "Font license\n",
            }
            for name, text in expected.items():
                (root / name).write_text(text)
            (fonts / "egui-icons.ttf").write_bytes(b"font bytes")
            (fonts / "README.txt").write_text("Build instructions\n")
            (root / "LICENSE.rs").write_text("// source code\n")
            self.assertEqual(dict(notice_files(root)),
                             {str(Path(name)): text for name, text in expected.items()})
