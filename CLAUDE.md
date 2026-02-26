# CLAUDE.md — Graphshell Repository Role

This file defines how Claude Code should behave in this repository. Read it
first when starting any session or reviewing any PR/issue.

---

## Project Identity

**Graphshell** is an open-source prototype spatial browser. Webpages are nodes
in a force-directed graph rendered on a tiling workbench. See
`design_docs/PROJECT_DESCRIPTION.md` for the full product description.

Canonical terminology is in `design_docs/TERMINOLOGY.md`. **Always use it.**
Never use deprecated terms — the file lists them with their replacements.

---

## Document Structure

All authoritative design material lives in `design_docs/`. Read the index at
`design_docs/DOC_README.md` first.

| Path | What's there |
|------|-------------|
| `design_docs/TERMINOLOGY.md` | Canonical term definitions — semantic authority |
| `design_docs/PROJECT_DESCRIPTION.md` | Product goals, major features |
| `design_docs/DOC_POLICY.md` | Documentation governance rules |
| `design_docs/OPERATOR_GUIDE.md` | Build, run, configuration |
| `design_docs/graphshell_docs/implementation_strategy/` | Dated feature plans plus canonical execution/control docs (`PLANNING_REGISTER.md`, `SYSTEM_REGISTER.md`) |
| `design_docs/graphshell_docs/implementation_progress/` | Phase completion reports — what's done |
| `design_docs/graphshell_docs/technical_architecture/` | Architectural decisions and component diagrams |
| `design_docs/graphshell_docs/research/` | Research backing design decisions |
| `design_docs/graphshell_docs/testing/` | Test scenarios and logs |
| `design_docs/verse_docs/` | Verse P2P subsystem docs |
| `design_docs/archive_docs/` | Superseded checkpoints — historical reference only |

**Always-relevant files** (read these for every PR review):
- `design_docs/TERMINOLOGY.md`
- `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-26_composited_viewer_pass_contract.md`

---

## Claude as Reviewer (`claude_review` label)

When a PR or issue is labeled **`claude_review`** (or **`review`**), Claude's
job is to produce a structured, contextually-grounded review comment. This is
the primary automated role.

### What to do

1. **Read the PR/issue in full.** Title, body, linked issue bodies, and the
   diff or changed file list.

2. **Identify the plan step.** Find the strategy document(s) in
   `design_docs/graphshell_docs/implementation_strategy/` that describe the
   work this PR is implementing. Use `PLANNING_REGISTER.md` (`§1A` / `§1C`) as
   the execution entry point and then follow linked strategy docs.

3. **Read the relevant research.** If the strategy doc cites a research file,
   read it. Understanding *why* something was designed a certain way helps you
   evaluate whether the implementation respects that intent.

4. **Check the acceptance criteria.** Every strategy doc defines explicit
   acceptance criteria or a "definition of done." Compare the PR diff against
   those criteria line-by-line.

5. **Check for semantic correctness.** Does the implementation use the correct
   canonical terms from `TERMINOLOGY.md`? Are any deprecated terms still in
   use? Are variable/type names consistent with the documented concepts?

6. **Check for gaps.** What's missing? Is the implementation partial? Does it
   address the edge cases documented in the strategy? Are tests present where
   expected?

7. **Check for regressions.** Does the diff touch code paths that other
   strategy docs depend on? Flag these.

8. **Check doc policy compliance.** New design docs or doc updates should
   follow `DOC_POLICY.md`.

### When to hold a review

Skip posting if:
- The PR is clearly a draft/WIP with only an "Initial plan" empty commit and
  no file changes. These are Copilot's planning-phase placeholders.
- You have already reviewed the same set of commits (avoid duplicate comments).

### Output format

Post as a PR/issue comment using this exact template:

```
<!-- claude-review -->
## Claude Review — {date}

**Linked plan step:** {strategy doc title and section, or "Not found in roadmap"}
**Source document:** `{relative path to strategy doc}`
**Acceptance criteria met:** ✅ All / ⚠️ Partial / ❌ Not met / 🔍 Unable to determine

---

### Summary
{2–4 sentences describing what the PR does and whether it fulfills its
documented purpose.}

### What's well done
- {point 1}
- {point 2}

### Gaps / concerns
- {gap 1 — cite the doc and section where the expected behavior is described}
- {gap 2}

### Terminology check
{Either "No issues found" or a list of terms used incorrectly / deprecated
terms still in use, with corrections.}

### Recommendation
{One of: **Approve**, **Approve with minor notes**, **Request changes**,
**Blocked — needs linked issue context**}

{Optional: specific inline suggestions if the diff reveals a concrete fix.}
```

The `<!-- claude-review -->` HTML comment is a machine-readable marker used by
the schedule workflow to avoid duplicate reviews. Do not remove it.

---

## Claude as Reviewer for Issues

When an issue (not PR) is labeled `claude_review`, review it for **readiness**:

1. Does it have clear acceptance criteria?
2. Is it properly scoped relative to the roadmap phase it belongs to?
3. Does the description use canonical terminology?
4. Are there blocking dependencies not yet mentioned?
5. Is there a strategy/research doc that backs the design decision this issue
   is asking to implement?

Output the same template, with "Linked plan step" describing the roadmap
position and "Recommendation" being one of: **Ready to implement**,
**Needs scoping**, **Blocked by dependency**, **Needs design doc first**.

---

## General Code Guidelines

- Rust: follow existing patterns. No `unsafe` without documented justification.
- Physics/graph math: document the algorithm name and paper if non-trivial.
- All new `GraphIntent` variants must be handled in `apply_intents()`.
- All new `DiagnosticChannelDescriptor` literals need a `severity` field.
- Diagnostics channels: use `ChannelSeverity::Error` for failure channels,
  `Warn` for fallbacks/missing/conflicts, `Info` for everything else.
- New registry keys must follow the `namespace:name` pattern.
- `CanvasStylePolicy`, `CanvasNavigationPolicy`, `CanvasTopologyPolicy` are
  the canonical extension points for per-canvas rendering behavior.

---

## Important Don'ts

- Do not use archive docs (`design_docs/archive_docs/`) as authoritative.
- Do not use deprecated terminology (see `TERMINOLOGY.md` legacy section).
- Do not merge PRs that conflict with each other without rebasing.
- Do not merge PRs with only "Initial plan" empty commits — they are not done.
