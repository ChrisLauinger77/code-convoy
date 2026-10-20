"""Collect notice evidence for review, not a license approval tool.

Usage: python packaging/collect_licenses.py METADATA_JSON OUTPUT_JSON
Generate metadata with cargo metadata --locked --format-version 1 after fetching
all target dependencies. Review nested assets and fill missing upstream notices
at exact revisions before updating third-party/catalog.json and texts.json.
"""
import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET


def notice_files(root):
    names = re.compile(r"^(?:LICEN[CS]E|COPYING|NOTICE|AUTHORS|COPYRIGHT|OFL|UFL|UNLICENSE)(?:[._-]|$)", re.I)
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix in (".rs", ".toml"):
            continue
        # Retain nested notices conservatively, including font and protocol data.
        if names.match(path.name) or path.name in ("Hack-Regular.txt", "egui-icons.txt", "emoji-icon-font-mit-license.txt"):
            yield str(path.relative_to(root)), path.read_text(encoding="utf-8-sig")
        elif path.suffix == ".xml":
            try:
                document = ET.parse(path)
            except ET.ParseError:
                continue
            for number, element in enumerate(document.iter()):
                if element.tag in ("copyright", "comment"):
                    text = "".join(element.itertext()).strip()
                    if "copyright" in text.lower():
                        yield f"{path.relative_to(root)}#{number}", text + "\n"


def collect(metadata):
    packages = []
    for package in metadata["packages"]:
        if package["source"] is None:
            continue
        root = Path(package["manifest_path"]).parent
        notices = [{"path": path, "sha256": hashlib.sha256(text.encode()).hexdigest(), "text": text}
                   for path, text in notice_files(root)]
        vcs_path = root / ".cargo_vcs_info.json"
        packages.append({"name": package["name"], "version": package["version"],
                         "license": package["license"], "repository": package["repository"],
                         "authors": package["authors"], "source": package["source"],
                         "vcs": json.loads(vcs_path.read_text()) if vcs_path.exists() else None,
                         "notices": notices})
    return packages


if __name__ == "__main__":
    Path(sys.argv[2]).write_text(json.dumps(collect(json.loads(Path(sys.argv[1]).read_text())),
                                          ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
