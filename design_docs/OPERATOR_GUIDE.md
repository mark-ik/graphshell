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
- `cargo test --features test-utils --test scenarios`
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

## 8) Windows Servo build reliability (mozjs_sys / mozmake)

Symptoms:

- `mozjs_sys` fails with `Failed to run "mozmake": program not found`.
- Build logs may also show `Failed to unpack ... js_static.lib` before falling back to source build.

Reliable recovery path:

1. Run bootstrap installer mode:
  - `pwsh -NoProfile -File scripts/dev/bootstrap-dev-env.ps1 --install`
2. Ensure `make` and `mozmake` resolve:
  - `where make`
  - `where mozmake`
  - first `mozmake` hit should be `C:\mozilla-build\bin\mozmake.exe`
3. Persist Windows env vars (once):
  - `setx MOZILLABUILD C:\mozilla-build`
  - `setx MOZTOOLS_PATH C:\mozilla-build`
  - `setx CARGO_TARGET_DIR C:\t\graphshell-target`
4. Open a new shell and run:
  - `cargo check -p graphshell --all-targets`
  - `cargo test -p graphshell --lib`

Use `scripts/dev/smoke-matrix.ps1` only when you specifically want lane-isolated target routing, a one-command smoke check, or WSL fallback behavior. The normal contributor path is direct cargo.

Notes:

- Keeping `CARGO_TARGET_DIR` outside OneDrive reduces archive/unpack fragility during Servo static lib extraction.
- The smoke helper now defaults Windows lane output to `C:\t\graphshell-target\windows_target` when `CARGO_TARGET_DIR` is unset.
- `bootstrap-dev-env.ps1 --install` now installs real `make.exe` and `mozmake.exe` into `C:\mozilla-build\bin` and removes any stale `~/.cargo/bin/mozmake.cmd` shim.
- If `where mozmake` resolves to `~/.cargo/bin/mozmake.cmd`, remove it and rerun bootstrap so the MozillaBuild copy wins.
- The validated Windows verification environment is:
  - `MOZILLABUILD=C:\mozilla-build`
  - `MOZTOOLS_PATH=C:\mozilla-build`
  - `CARGO_TARGET_DIR=C:\t\graphshell-target`
  - `PATH` preferring `C:\mozilla-build\bin`

If you previously used repo-local `target/windows_target`, `target`, or `target-clat` trees, they are disposable build outputs and safe to delete when reclaiming disk.

Camera/navigation semantic guardrails (for incident prevention and regression triage):

- `design_docs/graphshell_docs/implementation_strategy/graph/graph_node_edge_interaction_spec.md` §4.0

---

If this guide drifts from your actual workflow, update it immediately after finishing a task so it stays truthful.
