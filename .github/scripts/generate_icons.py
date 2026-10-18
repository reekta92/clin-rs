#!/usr/bin/env python3
"""Regenerate package icons from assets/icon.svg (requires librsvg and Pillow)."""

from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[2]


def main():
    with TemporaryDirectory() as temporary:
        master = Path(temporary) / "icon.png"
        subprocess.run(
            ["rsvg-convert", "--keep-aspect-ratio", "--width", "1024", "--height", "1024", "--output", str(master),
             str(ROOT / "assets/icon.svg")],
            check=True,
        )
        with Image.open(master) as rendered:
            icon = ImageOps.pad(rendered.convert("RGBA"), (1024, 1024), color=(0, 0, 0, 0))
            assert icon.size == (1024, 1024) and icon.mode == "RGBA"
            icon.resize((256, 256), Image.Resampling.LANCZOS).save(ROOT / "assets/clin.png")
            icon.save(ROOT / "clin.ico", sizes=[(size, size) for size in (16, 24, 32, 48, 64, 128, 256)])
            icon.save(ROOT / "assets/clin.icns")

    for path in (ROOT / "assets/clin.png", ROOT / "clin.ico", ROOT / "assets/clin.icns"):
        with Image.open(path) as generated:
            generated.verify()
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
