#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
from pathlib import Path


def wipe_wiki_content(wiki_dir: Path) -> None:
    for child in wiki_dir.iterdir():
        if child.name == ".git":
            continue
        if child.is_dir():
            shutil.rmtree(child)
        else:
            child.unlink()


def copy_tree(source_dir: Path, wiki_dir: Path) -> None:
    for source_path in source_dir.rglob("*"):
        relative = source_path.relative_to(source_dir.parent)
        target = wiki_dir / relative
        if source_path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, target)


def page_target_from_markdown(relative_path: Path) -> str:
    return relative_path.as_posix().removesuffix(".md")


def build_sidebar(source_dir: Path) -> str:
    root = source_dir.parent
    lines: list[str] = ["- [Home](Home)"]

    def walk(dir_path: Path, depth: int) -> None:
        indent = "  " * depth
        entries = sorted(dir_path.iterdir(), key=lambda p: (p.is_file(), p.name.lower()))
        for entry in entries:
            rel = entry.relative_to(root)
            if entry.is_dir():
                lines.append(f"{indent}- **{entry.name}**")
                walk(entry, depth + 1)
                continue
            if entry.suffix.lower() != ".md":
                continue
            title = entry.stem
            target = page_target_from_markdown(rel)
            lines.append(f"{indent}- [{title}]({target})")

    lines.append("- **design_docs**")
    walk(source_dir, 1)
    return "\n".join(lines) + "\n"


def build_home(source_dir: Path) -> str:
    root = source_dir.parent
    top_level_docs = sorted(
        [p for p in source_dir.iterdir() if p.is_file() and p.suffix.lower() == ".md"],
        key=lambda p: p.name.lower(),
    )
    top_level_dirs = sorted([p for p in source_dir.iterdir() if p.is_dir()], key=lambda p: p.name.lower())

    lines = [
        "# Graphshell Design Docs",
        "",
        "This wiki is auto-synced from the repository `design_docs/` directory.",
        "",
        "## Top-level Docs",
    ]

    if top_level_docs:
        for doc in top_level_docs:
            rel = doc.relative_to(root)
            target = page_target_from_markdown(rel)
            lines.append(f"- [{doc.stem}]({target})")
    else:
        lines.append("- (none)")

    lines.append("")
    lines.append("## Sections")
    for directory in top_level_dirs:
        lines.append(f"- **{directory.name}**")

    lines.extend(
        [
            "",
            "Use `_Sidebar` for full directory navigation.",
            "",
            "_Last sync source: `design_docs/`_",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Sync repository design docs into a GitHub wiki checkout.")
    parser.add_argument("--source", type=Path, default=Path("design_docs"), help="Source docs directory")
    parser.add_argument("--wiki-dir", type=Path, required=True, help="Checked-out wiki repository path")
    args = parser.parse_args()

    source_dir = args.source.resolve()
    wiki_dir = args.wiki_dir.resolve()

    if not source_dir.exists() or not source_dir.is_dir():
        raise SystemExit(f"Source directory not found: {source_dir}")
    if not wiki_dir.exists() or not wiki_dir.is_dir():
        raise SystemExit(f"Wiki directory not found: {wiki_dir}")

    wipe_wiki_content(wiki_dir)
    copy_tree(source_dir, wiki_dir)

    home_md = build_home(source_dir)
    sidebar_md = build_sidebar(source_dir)

    (wiki_dir / "Home.md").write_text(home_md, encoding="utf-8")
    (wiki_dir / "_Sidebar.md").write_text(sidebar_md, encoding="utf-8")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
