import tempfile
from pathlib import Path
import re
import unittest

from sync_wiki import MANIFEST, ROOT, build_pages, page_names, rewrite_links, write_pages


class WikiTests(unittest.TestCase):
    def test_all_public_docs_have_pages_and_navigation(self):
        names = page_names(ROOT)
        pages = build_pages(ROOT, "reekta92/clin-rs", "main")
        self.assertEqual(len(pages), len(names) + 3)
        for source, name in names.items():
            with self.subTest(source=source):
                self.assertIn(f"{name}.md", pages)
                self.assertIn(f"]({name})", pages["Home.md"])
                self.assertIn(f"]({name})", pages["_Sidebar.md"])
                self.assertIn(f"/blob/main/{source}", pages[f"{name}.md"])
        for name, text in pages.items():
            for target in re.findall(r"\]\(([^\s)]+)", text):
                if ":" not in target and not target.startswith("#"):
                    with self.subTest(page=name, target=target):
                        self.assertIn(f"{target.split('#')[0]}.md", pages)

    def test_links_and_anchors_rewritten_but_examples_unchanged(self):
        text = (
            "[Editor](EDITOR.md#find-popup) [Readme](../README.md?x=1#installation)\n"
            "[License](../LICENSE) [Sources](../src/)\n"
            "[Web](https://example.com) [Here](#here)\n"
            "```markdown\n[Example](EDITOR.md)\n```\n"
            "~~~markdown\n[Example](EDITOR.md)\n~~~\n"
        )
        actual = rewrite_links(text, "docs/INDEX.md", page_names(ROOT), ROOT, "reekta92/clin-rs", "abc123")
        self.assertIn("[Editor](Editor#find-popup)", actual)
        self.assertIn("[Readme](Overview?x=1#installation)", actual)
        self.assertIn("[License](https://github.com/reekta92/clin-rs/blob/abc123/LICENSE)", actual)
        self.assertIn("[Sources](https://github.com/reekta92/clin-rs/tree/abc123/src)", actual)
        self.assertIn("[Web](https://example.com) [Here](#here)", actual)
        self.assertEqual(actual.count("[Example](EDITOR.md)"), 2)

    def test_broken_repository_link_fails(self):
        with self.assertRaisesRegex(ValueError, "Broken repository link"):
            rewrite_links("[Missing](MISSING.md)", "docs/INDEX.md", {}, ROOT, "owner/repo", "main")
        with self.assertRaisesRegex(ValueError, "Broken repository link"):
            rewrite_links("[Escape](../../outside)", "docs/INDEX.md", {}, ROOT, "owner/repo", "main")

    def test_new_docs_are_discovered_and_duplicate_titles_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "docs" / "NEW_FEATURE.md").write_text("# New feature\n")
            pages = build_pages(root, "owner/repo", "main")
            self.assertIn("New-Feature.md", pages)
            self.assertIn("](New-Feature)", pages["Home.md"])
            (root / "docs" / "new_feature.md").write_text("# Duplicate\n")
            with self.assertRaisesRegex(ValueError, "duplicate or reserved"):
                page_names(root)

    def test_sync_is_idempotent_removes_only_stale_generated_pages(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "Custom.md").write_text("Handwritten page\n")
            write_pages(output, {"Home.md": "Home\n", "Old.md": "Old\n"})
            write_pages(output, {"Home.md": "Updated\n"})
            self.assertFalse((output / "Old.md").exists())
            self.assertEqual((output / "Custom.md").read_text(), "Handwritten page\n")
            before = {p.name: p.read_bytes() for p in output.iterdir()}
            write_pages(output, {"Home.md": "Updated\n"})
            self.assertEqual(before, {p.name: p.read_bytes() for p in output.iterdir()})

    def test_unsafe_manifest_fails_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / MANIFEST).write_text("../outside.md\n")
            with self.assertRaisesRegex(ValueError, "Invalid generated wiki filename"):
                write_pages(output, {"Home.md": "New content\n"})
            self.assertFalse((output / "Home.md").exists())

    def test_symlinks_are_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            target = output / "Custom.md"
            target.write_text("Keep\n")
            (output / "Home.md").symlink_to(target)
            with self.assertRaisesRegex(ValueError, "wiki symlink"):
                write_pages(output, {"Home.md": "New content\n"})
            self.assertEqual(target.read_text(), "Keep\n")
            (output / "Home.md").unlink()
            (output / MANIFEST).symlink_to(target)
            with self.assertRaisesRegex(ValueError, "wiki symlink: manifest"):
                write_pages(output, {"Home.md": "New content\n"})
            self.assertFalse((output / "Home.md").exists())
            self.assertEqual(target.read_text(), "Keep\n")


if __name__ == "__main__":
    unittest.main()
