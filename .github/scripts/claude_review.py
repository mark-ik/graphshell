#!/usr/bin/env python3
"""
Claude Code Review Script
=========================
Reads PR/issue context, loads relevant design docs, calls Claude, and posts
a structured review comment.

Invoked by .github/workflows/claude-review.yml.

Environment variables required:
  ANTHROPIC_API_KEY   - API key for Claude
  GH_TOKEN            - GitHub token for reading/writing PR comments
  GITHUB_REPOSITORY   - "owner/repo" (set automatically by GitHub Actions)

Optional:
  REVIEW_MODE         - "scan" (find all labeled PRs) or "explicit"
  REVIEW_TARGETS      - JSON list of "pr:N" or "issue:N" strings
  FORCE_REVIEW        - "true" to bypass recency check
  REVIEW_LABELS       - comma-separated label names that trigger review
                        (default: "claude_review,review")
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import textwrap
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import anthropic
import requests

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

REPO = os.environ.get("GITHUB_REPOSITORY", "")
GH_TOKEN = os.environ.get("GH_TOKEN", "")
ANTHROPIC_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
FORCE_REVIEW = os.environ.get("FORCE_REVIEW", "false").lower() == "true"
REVIEW_LABELS = {
    l.strip().lower()
    for l in os.environ.get("REVIEW_LABELS", "claude_review,review").split(",")
    if l.strip()
}

# Marker embedded in every review comment so we can detect prior reviews.
REVIEW_MARKER = "<!-- claude-review -->"

# Claude model to use
CLAUDE_MODEL = "claude-opus-4-6"

# Max tokens for the review response
MAX_REVIEW_TOKENS = 2048

# Design doc root relative to the repo checkout
DESIGN_DOCS_ROOT = Path("design_docs")

# Files always included in the review context
ALWAYS_INCLUDE_DOCS = [
    DESIGN_DOCS_ROOT / "TERMINOLOGY.md",
    DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "IMPLEMENTATION_ROADMAP.md",
    DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-24_immediate_priorities.md",
]

# Keyword → strategy doc mapping for context-sensitive loading
KEYWORD_DOC_MAP: list[tuple[list[str], Path]] = [
    (["physics", "reheat", "simulation", "force"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-24_physics_engine_extensibility_plan.md"),
    (["registry", "lens", "layout", "theme", "canvas"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-22_registry_layer_plan.md"),
    (["traversal", "history", "webview", "url"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-20_edge_traversal_impl_plan.md"),
    (["render", "radial", "palette", "command", "action"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-23_graph_interaction_consistency_plan.md"),
    (["diagnostic", "channel", "severity", "observability"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "research" / "2026-02-24_diagnostics_research.md"),
    (["pane", "tile", "workbench", "split", "tab"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md"),
    (["graph", "node", "edge", "lasso", "select", "multi"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "research" / "2026-02-18_graph_ux_research_report.md"),
    (["multi.view", "view.state", "view.id", "graphviewid"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-22_multi_graph_pane_plan.md"),
    (["embedder", "wry", "verso", "webview"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-23_wry_integration_strategy.md"),
    (["badge", "tag", "udc", "semantic"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-23_udc_semantic_tagging_plan.md"),
    (["accessibility", "screen.reader", "graph.reader", "a11y"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-24_spatial_accessibility_plan.md"),
    (["export", "html", "interactive", "artifact"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-25_interactive_html_export_plan.md"),
    (["viewport", "culling", "lod", "zoom", "label"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-24_performance_tuning_plan.md"),
    (["verse", "sync", "peer", "identity", "trust"],
     DESIGN_DOCS_ROOT / "verse_docs" / "implementation_strategy" / "2026-02-18_verse_integration_plan.md"),
    (["settings", "config", "profile"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-20_settings_architecture_plan.md"),
    (["backlog", "ticket", "stub"],
     DESIGN_DOCS_ROOT / "graphshell_docs" / "implementation_strategy" / "2026-02-25_backlog_ticket_stubs.md"),
]

# ---------------------------------------------------------------------------
# GitHub API helpers
# ---------------------------------------------------------------------------

def gh_headers() -> dict:
    return {
        "Authorization": f"Bearer {GH_TOKEN}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }


def gh_get(path: str, params: dict | None = None) -> dict | list:
    url = f"https://api.github.com/repos/{REPO}{path}"
    r = requests.get(url, headers=gh_headers(), params=params or {})
    r.raise_for_status()
    return r.json()


def gh_post(path: str, body: dict) -> dict:
    url = f"https://api.github.com/repos/{REPO}{path}"
    r = requests.post(url, headers=gh_headers(), json=body)
    r.raise_for_status()
    return r.json()


def get_pr(pr_number: int) -> dict:
    return gh_get(f"/pulls/{pr_number}")


def get_issue(issue_number: int) -> dict:
    return gh_get(f"/issues/{issue_number}")


def get_pr_diff(pr_number: int) -> str:
    """Returns the unified diff for a PR (truncated to ~300 lines if large)."""
    url = f"https://api.github.com/repos/{REPO}/pulls/{pr_number}"
    r = requests.get(url, headers={**gh_headers(), "Accept": "application/vnd.github.diff"})
    if r.status_code != 200:
        return "(diff not available)"
    lines = r.text.splitlines()
    if len(lines) > 300:
        lines = lines[:300] + [f"\n... ({len(lines) - 300} more lines truncated)"]
    return "\n".join(lines)


def get_pr_files(pr_number: int) -> list[str]:
    files = gh_get(f"/pulls/{pr_number}/files", {"per_page": "100"})
    return [f["filename"] for f in files] if isinstance(files, list) else []


def get_issue_comments(issue_number: int) -> list[dict]:
    comments = gh_get(f"/issues/{issue_number}/comments", {"per_page": "100"})
    return comments if isinstance(comments, list) else []


def get_pr_comments(pr_number: int) -> list[dict]:
    # PR comments live on the issues endpoint
    return get_issue_comments(pr_number)


def get_last_claude_review(comments: list[dict]) -> Optional[dict]:
    """Returns the most recent comment that contains the claude-review marker."""
    for c in reversed(comments):
        if REVIEW_MARKER in c.get("body", ""):
            return c
    return None


def get_pr_latest_commit_time(pr_number: int) -> Optional[datetime]:
    commits = gh_get(f"/pulls/{pr_number}/commits", {"per_page": "100"})
    if not isinstance(commits, list) or not commits:
        return None
    times = []
    for c in commits:
        ts = c.get("commit", {}).get("committer", {}).get("date") or \
             c.get("commit", {}).get("author", {}).get("date")
        if ts:
            times.append(datetime.fromisoformat(ts.replace("Z", "+00:00")))
    return max(times) if times else None


def post_comment(issue_number: int, body: str) -> None:
    gh_post(f"/issues/{issue_number}/comments", {"body": body})
    print(f"Posted review comment to #{issue_number}")


def list_labeled_prs() -> list[dict]:
    """Return open PRs that carry any of the review labels."""
    result = []
    for label in REVIEW_LABELS:
        page = 1
        while True:
            prs = gh_get("/pulls", {"state": "open", "per_page": "100", "page": str(page)})
            if not isinstance(prs, list) or not prs:
                break
            for pr in prs:
                pr_labels = {lbl["name"].lower() for lbl in pr.get("labels", [])}
                if pr_labels & REVIEW_LABELS:
                    if not any(r["number"] == pr["number"] for r in result):
                        result.append(pr)
            if len(prs) < 100:
                break
            page += 1
    return result


def list_labeled_issues() -> list[dict]:
    """Return open issues (not PRs) that carry any of the review labels."""
    result = []
    for label in REVIEW_LABELS:
        page = 1
        while True:
            items = gh_get("/issues", {
                "state": "open", "per_page": "100",
                "page": str(page), "labels": label,
            })
            if not isinstance(items, list) or not items:
                break
            for item in items:
                if "pull_request" not in item:  # exclude PRs
                    if not any(r["number"] == item["number"] for r in result):
                        result.append(item)
            if len(items) < 100:
                break
            page += 1
    return result


# ---------------------------------------------------------------------------
# Design doc loading
# ---------------------------------------------------------------------------

def read_doc(path: Path) -> str:
    if path.exists():
        return path.read_text(encoding="utf-8")
    return f"(file not found: {path})"


def select_docs(text: str) -> list[Path]:
    """Return design docs relevant to the given text (title + description)."""
    text_lower = text.lower()
    selected: list[Path] = list(ALWAYS_INCLUDE_DOCS)
    for keywords, doc_path in KEYWORD_DOC_MAP:
        if any(re.search(kw.replace(".", r"\W*"), text_lower) for kw in keywords):
            if doc_path not in selected and doc_path.exists():
                selected.append(doc_path)
    return selected


def load_docs(paths: list[Path]) -> str:
    parts = []
    for p in paths:
        content = read_doc(p)
        # Truncate very large docs to save context (keep first ~150 lines)
        lines = content.splitlines()
        if len(lines) > 150:
            content = "\n".join(lines[:150]) + f"\n\n... ({len(lines) - 150} more lines — read full file if needed)"
        parts.append(f"### {p}\n\n{content}")
    return "\n\n---\n\n".join(parts)


# ---------------------------------------------------------------------------
# Prompt construction
# ---------------------------------------------------------------------------

def build_pr_prompt(pr: dict, diff: str, files: list[str], docs: str) -> str:
    number = pr["number"]
    title = pr.get("title", "")
    body = pr.get("body") or "(no description)"
    base = pr.get("base", {}).get("ref", "main")
    head = pr.get("head", {}).get("ref", "")
    labels = [l["name"] for l in pr.get("labels", [])]

    return textwrap.dedent(f"""
    You are performing a structured code review for the Graphshell repository.
    Your role is described in CLAUDE.md (already read). Follow the output format
    exactly.

    ## PR #{number}: {title}

    **Base → head:** {base} ← {head}
    **Labels:** {', '.join(labels) or 'none'}

    ### PR Description
    {body}

    ### Changed Files
    {chr(10).join(f'  - {f}' for f in files) or '  (none)'}

    ### Diff (first 300 lines)
    ```diff
    {diff}
    ```

    ---

    ## Design Documents (excerpt)

    {docs}

    ---

    ## Instructions

    1. Identify which documented plan step / roadmap item this PR implements.
       Cite the exact strategy doc and section.
    2. List the acceptance criteria from that doc and check each one.
    3. Note any gaps, missing tests, or terminology issues.
    4. Give an honest recommendation.

    Use the exact output template from CLAUDE.md. Begin your response with
    `{REVIEW_MARKER}`.
    """).strip()


def build_issue_prompt(issue: dict, docs: str) -> str:
    number = issue["number"]
    title = issue.get("title", "")
    body = issue.get("body") or "(no description)"
    labels = [l["name"] for l in issue.get("labels", [])]

    return textwrap.dedent(f"""
    You are performing a readiness review for a GitHub issue in the Graphshell
    repository. Your role is described in CLAUDE.md. Follow the output format
    exactly.

    ## Issue #{number}: {title}

    **Labels:** {', '.join(labels) or 'none'}

    ### Issue Description
    {body}

    ---

    ## Design Documents (excerpt)

    {docs}

    ---

    ## Instructions

    Review this issue for *implementation readiness*:
    1. Is there a matching strategy/research doc that backs this work?
    2. Are the acceptance criteria clear and testable?
    3. Does the description use canonical terminology (check TERMINOLOGY.md)?
    4. Are blocking dependencies identified?
    5. Is this properly scoped for the current roadmap phase?

    Use the exact output template from CLAUDE.md (adapted for issues).
    Recommendation must be one of: Ready to implement / Needs scoping /
    Blocked by dependency / Needs design doc first.
    Begin your response with `{REVIEW_MARKER}`.
    """).strip()


# ---------------------------------------------------------------------------
# Claude API call
# ---------------------------------------------------------------------------

def call_claude(prompt: str) -> str:
    client = anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)
    message = client.messages.create(
        model=CLAUDE_MODEL,
        max_tokens=MAX_REVIEW_TOKENS,
        system=(
            "You are Claude Code, operating as a code reviewer for the Graphshell "
            "open-source project. You have deep familiarity with the project's design "
            "documents, implementation strategy, and canonical terminology. "
            "Your reviews are concise, grounded in the documented acceptance criteria, "
            "and always cite specific design docs by path and section. "
            "Never invent design requirements not present in the docs."
        ),
        messages=[{"role": "user", "content": prompt}],
    )
    return message.content[0].text


# ---------------------------------------------------------------------------
# Review orchestration
# ---------------------------------------------------------------------------

def should_review_pr(pr_number: int, force: bool = False) -> bool:
    """Return True if a fresh review is warranted for this PR."""
    if force:
        return True

    # Skip if last commit is the "Initial plan" only (no file changes)
    files = get_pr_files(pr_number)
    if not files:
        print(f"PR #{pr_number}: no file changes — skipping (Initial plan only)")
        return False

    comments = get_pr_comments(pr_number)
    last_review = get_last_claude_review(comments)
    if last_review is None:
        return True  # Never reviewed

    review_time = datetime.fromisoformat(
        last_review["updated_at"].replace("Z", "+00:00")
    )
    latest_commit = get_pr_latest_commit_time(pr_number)
    if latest_commit is None:
        return False

    return latest_commit > review_time


def should_review_issue(issue_number: int, force: bool = False) -> bool:
    if force:
        return True
    comments = get_issue_comments(issue_number)
    return get_last_claude_review(comments) is None


def review_pr(pr_number: int) -> None:
    print(f"Reviewing PR #{pr_number}…")
    pr = get_pr(pr_number)
    diff = get_pr_diff(pr_number)
    files = get_pr_files(pr_number)

    search_text = f"{pr.get('title', '')} {pr.get('body', '')} {' '.join(files)}"
    doc_paths = select_docs(search_text)
    docs = load_docs(doc_paths)

    prompt = build_pr_prompt(pr, diff, files, docs)
    review = call_claude(prompt)

    if not review.startswith(REVIEW_MARKER):
        review = REVIEW_MARKER + "\n" + review

    post_comment(pr_number, review)


def review_issue(issue_number: int) -> None:
    print(f"Reviewing issue #{issue_number}…")
    issue = get_issue(issue_number)

    search_text = f"{issue.get('title', '')} {issue.get('body', '')}"
    doc_paths = select_docs(search_text)
    docs = load_docs(doc_paths)

    prompt = build_issue_prompt(issue, docs)
    review = call_claude(prompt)

    if not review.startswith(REVIEW_MARKER):
        review = REVIEW_MARKER + "\n" + review

    post_comment(issue_number, review)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    if not ANTHROPIC_API_KEY:
        print("ERROR: ANTHROPIC_API_KEY is not set", file=sys.stderr)
        sys.exit(1)
    if not GH_TOKEN:
        print("ERROR: GH_TOKEN is not set", file=sys.stderr)
        sys.exit(1)
    if not REPO:
        print("ERROR: GITHUB_REPOSITORY is not set", file=sys.stderr)
        sys.exit(1)

    mode = os.environ.get("REVIEW_MODE", "scan")
    raw_targets = os.environ.get("REVIEW_TARGETS", "[]")
    try:
        targets: list[str] = json.loads(raw_targets)
    except json.JSONDecodeError:
        targets = []

    reviewed = 0

    if targets:
        for t in targets:
            if t.startswith("pr:"):
                n = int(t[3:])
                if should_review_pr(n, FORCE_REVIEW):
                    review_pr(n)
                    reviewed += 1
                else:
                    print(f"PR #{n}: already up-to-date, skipping")
            elif t.startswith("issue:"):
                n = int(t[6:])
                if should_review_issue(n, FORCE_REVIEW):
                    review_issue(n)
                    reviewed += 1
                else:
                    print(f"Issue #{n}: already reviewed, skipping")
    else:
        # Scan mode: find all labeled items
        print("Scan mode: finding all labeled PRs and issues…")
        for pr in list_labeled_prs():
            n = pr["number"]
            if should_review_pr(n, FORCE_REVIEW):
                review_pr(n)
                reviewed += 1
            else:
                print(f"PR #{n}: already up-to-date, skipping")
        for issue in list_labeled_issues():
            n = issue["number"]
            if should_review_issue(n, FORCE_REVIEW):
                review_issue(n)
                reviewed += 1
            else:
                print(f"Issue #{n}: already reviewed, skipping")

    print(f"Done. {reviewed} review(s) posted.")


if __name__ == "__main__":
    main()
