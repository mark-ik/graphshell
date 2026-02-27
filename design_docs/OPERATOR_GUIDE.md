# Graphshell Operator Guide

This is a practical, reusable playbook for day-to-day repo operations.

## 0) Cargo-first default policy

- Prefer direct cargo commands as the primary workflow.
- Treat `scripts/dev/*` as optional convenience wrappers, not the default path.
- Assume debug profile unless `--release` is explicitly needed.

Default local validation loop:

- `cargo check`
- `cargo test`
- `cargo run -- <url>`

Versioning/release cadence policy:

- `design_docs/graphshell_docs/implementation_strategy/VERSIONING_POLICY.md`

## 1) Safe PR review workflow

1. Start clean:
   - `git status --short`
2. Check out PR branch:
   - `gh pr checkout <PR_NUMBER> --repo mark-ik/graphshell`
3. Run targeted acceptance tests from PR description.
4. Run compile check:
   - `cargo check`
5. Decide:
   - Mergeable now
   - Mergeable with non-blocking nits
   - Blocked (list exact failures)

## 2) Diff PR against main (without copy/paste)

Use Git diffs and merges, not manual file copying.

- Changed files only:
  - `git diff --name-only origin/main...HEAD`
- Full patch:
  - `git diff origin/main...HEAD`
- PR metadata:
  - `gh pr view <PR_NUMBER> --repo mark-ik/graphshell --json mergeable,mergeStateStatus`

## 3) Resolve merge conflicts correctly

On the PR branch:

1. `git fetch origin`
2. `git merge origin/main`
3. If conflicts:
   - `git diff --name-only --diff-filter=U`
4. Open each conflicted file and resolve conflict markers:
   - keep needed lines from both sides
   - delete `<<<<<<<`, `=======`, `>>>>>>>`
5. `git add <resolved_file>`
6. Run targeted tests + `cargo check`
7. `git commit` (merge commit)
8. `git push`

Debug-focused test commands (common):

- `cargo test`
- `cargo test <name> --lib -- --nocapture`
- `cargo test --test <integration_test_name>`
- `cargo test -- --test-threads=1`

## 4) Fast conflict-resolution checklist

- Keep both feature registrations when both are valid (typical in `mod.rs` files).
- Do not drop diagnostics channel constants that were added by earlier merged PRs.
- Re-run scenario tests that correspond to both sides of the merge.
- Confirm no unresolved conflict markers remain:
  - `git grep -n "<<<<<<<\|=======\|>>>>>>>"`

## 5) Batched commits pattern

For large work, split into logical commits:

1. Architecture / moves / module reshaping
2. Runtime logic + tests
3. Docs / plans / roadmap sync

Helpful checks:

- What is staged:
  - `git diff --cached --name-status`
- What is left:
  - `git status --short`

## 6) Wiki sync behavior (already configured)

Workflow file:

- `.github/workflows/wiki-sync.yml`

Script:

- `.github/scripts/sync_wiki.py`

Auto-triggers on push to `main` when these paths change:

- `design_docs/**`
- `.github/scripts/sync_wiki.py`
- `.github/workflows/wiki-sync.yml`

Manual trigger is also available from GitHub Actions (`workflow_dispatch`).

## 7) Recommended merge gate for Verse phase PRs

Before merging any Verse closure PR:

1. Run the new scenario target from that PR (for example `verse_delta_sync_basic` or `verse_access_control`).
2. Run `cargo check`.
3. Confirm diagnostics assertions are deterministic (prefer pre/post event deltas over global `> 0` checks).

---

If this guide drifts from your actual workflow, update it immediately after finishing a task so it stays truthful.
