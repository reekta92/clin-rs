#!/usr/bin/env python3
"""Render repository documentation into an existing GitHub wiki checkout."""

import argparse
import os
from pathlib import Path
import posixpath
import re
from urllib.parse import quote, unquote, urlsplit, urlunsplit

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ".clin-generated-pages"
ALIASES = {
    "README.md": "Overview",
    "docs/INDEX.md": "Documentation-Index",
    "docs/CONFIG_REFERENCE.md": "Configuration-Reference",
}


def page_names(root):
    sources = sorted([*root.glob("*.md"), *(root / "docs").rglob("*.md")])
    names = {}
    for source in sources:
        path = source.relative_to(root).as_posix()
        names[path] = ALIASES.get(path, source.stem.replace("_", "-").title())
    if len(set(names.values())) != len(names) or {"Home", "_Sidebar", "_Footer"} & set(names.values()):
        raise ValueError("Documentation filenames produce duplicate or reserved wiki titles")
    return names


def rewrite_links(text, source, names, root, repository, revision):
    def replace(match):
        target = match.group(0)
        url = urlsplit(target)
        if url.scheme or url.netloc or not url.path:
            return target
        path = posixpath.normpath(posixpath.join(posixpath.dirname(source), unquote(url.path)))
        if path in names:
            return urlunsplit(("", "", names[path], url.query, url.fragment))
        if path.startswith("../") or not (root / path).exists():
            raise ValueError(f"Broken repository link in {source}: {target}")
        kind = "tree" if (root / path).is_dir() else "blob"
        absolute = f"https://github.com/{repository}/{kind}/{quote(revision, safe='')}/{quote(path)}"
        return urlunsplit(("https", "github.com", urlsplit(absolute).path, url.query, url.fragment))

    # Code examples describe local files, not wiki navigation.
    lines = []
    fence = None
    for line in text.splitlines(keepends=True):
        marker = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
        if marker:
            run, rest = marker.groups()
            if fence is None:
                fence = run
            elif run[0] == fence[0] and len(run) >= len(fence) and not rest.strip():
                fence = None
            lines.append(line)
        elif fence is not None:
            lines.append(line)
        else:
            lines.append(re.sub(r"(?<=\]\()[^\s)]+", replace, line))
    return "".join(lines)


def build_pages(root, repository, revision):
    names = page_names(root)
    pages = {}
    for source, name in names.items():
        text = (root / source).read_text(encoding="utf-8")
        source_url = f"https://github.com/{repository}/blob/{quote(revision, safe='')}/{quote(source)}"
        notice = f"> Synced from [{source}]({source_url}). Edit the source file, not this wiki page.\n\n"
        pages[f"{name}.md"] = notice + rewrite_links(text, source, names, root, repository, revision)

    def links(paths):
        return "".join(f"- [{names[path].replace('-', ' ')}]({names[path]})\n" for path in paths)

    docs = sorted((path for path in names if path.startswith("docs/")), key=lambda path: names[path])
    project = sorted(path for path in names if not path.startswith("docs/") and path != "README.md")
    navigation = (
        "## Getting started\n\n"
        "- [Overview](Overview)\n"
        "- [Installation](Overview#installation)\n"
        "- [Quick start](Overview#quick-start)\n"
        "- [CLI commands](Overview#cli-commands)\n\n"
        "## Documentation\n\n" + links(docs) + "\n## Project\n\n" + links(project)
    )
    pages["Home.md"] = (
        "# clin documentation\n\n"
        "clin is a terminal note manager inspired by Obsidian. "
        "This wiki contains all public Markdown documentation from the repository.\n\n"
        "Pages are generated from `main`. To change documentation, edit the source "
        "file linked at the top of each page and submit a pull request.\n\n" + navigation
    )
    pages["_Sidebar.md"] = "[Home](Home)\n\n" + navigation
    pages["_Footer.md"] = (
        f"[Home](Home) · [Repository](https://github.com/{repository}) · "
        f"[Releases](https://github.com/{repository}/releases) · "
        f"[Report an issue](https://github.com/{repository}/issues)\n\n"
        "Generated from repository documentation. Source files are authoritative; "
        "direct edits to generated pages are overwritten by the next sync.\n"
    )
    return pages


def write_pages(output, pages):
    manifest = output / MANIFEST
    if manifest.is_symlink():
        raise ValueError("Refusing to overwrite wiki symlink: manifest")
    previous = set(manifest.read_text(encoding="utf-8").splitlines()) if manifest.exists() else set()
    # Never let a damaged manifest remove paths outside this checkout.
    for name in previous | pages.keys():
        if not re.fullmatch(r"[A-Za-z0-9_-]+\.md", name):
            raise ValueError(f"Invalid generated wiki filename: {name}")
        if (output / name).is_symlink():
            raise ValueError(f"Refusing to overwrite wiki symlink: {name}")
    for name, text in pages.items():
        (output / name).write_text(text, encoding="utf-8")
    for name in previous - pages.keys():
        (output / name).unlink(missing_ok=True)
    manifest.write_text("".join(f"{name}\n" for name in sorted(pages)), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wiki", type=Path, help="Existing clone of the wiki Git repository")
    parser.add_argument("--revision", default="main", help="Source revision for attribution links")
    args = parser.parse_args()
    output = args.wiki.resolve()
    if not (output / ".git").is_dir() or output == ROOT or output in ROOT.parents or output.is_relative_to(ROOT):
        parser.error("wiki must be a separate Git checkout outside the source repository")
    repository = os.environ.get("GITHUB_REPOSITORY", "reekta92/clin-rs")
    pages = build_pages(ROOT, repository, args.revision)
    write_pages(output, pages)
    print(f"Rendered {len(pages) - 2} wiki pages plus sidebar and footer into {output}")


if __name__ == "__main__":
    main()
