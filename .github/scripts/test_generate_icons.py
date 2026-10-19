from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

import generate_icons


class IconTests(unittest.TestCase):
    def test_generated_icons_preserve_proportions_and_square_sizes(self):
        for width, height, margin in ((200, 100, (128, 32)), (100, 200, (32, 128)), (100, 100, None)):
            with self.subTest(width=width, height=height), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "assets").mkdir()
                source = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">'
                          f'<rect width="{width}" height="{height}" fill="red" /></svg>')
                (root / "assets/icon.svg").write_text(source)
                with patch.object(generate_icons, "ROOT", root):
                    generate_icons.main()
                self.assertEqual((root / "assets/icon.svg").read_text(), source)
                with Image.open(root / "assets/clin.png") as png:
                    self.assertEqual(png.size, (256, 256))
                    self.assertEqual(png.getpixel((128, 128)), (255, 0, 0, 255))
                    if margin:
                        self.assertEqual(png.getpixel(margin)[3], 0)
                with Image.open(root / "clin.ico") as ico:
                    self.assertEqual(ico.ico.sizes(), {(size, size) for size in (16, 24, 32, 48, 64, 128, 256)})
                    ico.load()
                with Image.open(root / "assets/clin.icns") as icns:
                    icns.load()
                    self.assertEqual(icns.size, (1024, 1024))
