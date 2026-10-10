#!/usr/bin/env python3
"""Stage the static homepage using only the Python standard library.

Keep the existing application artwork as the single source of truth. Only the
explicit public files below are copied, never repository or runtime data.
"""

from pathlib import Path
import shutil


def main() -> None:
    source = Path(__file__).resolve().parent
    root = source.parent
    output = root / "dist" / "homepage"
    (output / "assets").mkdir(parents=True, exist_ok=True)
    for name in ("index.html", "styles.css"):
        shutil.copyfile(source / name, output / name)
    for name in ("codeconvoy-128.png", "screenshot.png"):
        shutil.copyfile(root / "assets" / name, output / "assets" / name)
    (output / ".nojekyll").touch()
    print(f"Homepage ready: {output}")


if __name__ == "__main__":
    main()
