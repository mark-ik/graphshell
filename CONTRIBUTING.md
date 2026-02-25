# Contributing

## PR Workflow

The main source of merge churn in this repo is multiple concurrent PRs touching the same integration files (`app.rs`, `render/mod.rs`) and historical planning docs. Use the rules below to reduce rebases and superseded PRs.

### 1. Work in lanes, not large parallel batches

Group changes by lane and merge within the lane before starting another PR that touches the same files.

Examples:
- `lane:p6` multi-view / pane state
- `lane:p7` content model + viewer registry
- `lane:p10` perf + accessibility baselines
- `lane:docs` planning/adoption docs

Only keep one active mergeable PR per lane if it touches shared integration files.

### 2. Split by ownership boundary

Prefer small PRs that each do one thing:
- foundation/model changes
- UI wiring/integration
- tests/validation

If a PR touches `app.rs` or `render/mod.rs`, scope it to a single child issue (for example `#63`, `#64`, `#72`, etc.) rather than a full epic.

### 3. Use stacked PRs for dependent work

If PR B depends on PR A, base PR B on PR A's branch instead of `main`.

Merge order:
1. bottom of stack
2. middle
3. top

This avoids rebasing every open PR whenever `main` moves.

### 4. Treat historical planning docs as snapshots

These files are historical source-register artifacts and should generally not be rewritten for status churn:
- `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-25_backlog_ticket_stubs.md`

Preferred pattern:
- add a new dated strategy doc
- link to the historical doc
- track status in GitHub issues/projects/comments

### 5. Refresh before marking ready

Before changing a PR from draft to ready:
1. update from `main` (`gh pr update-branch` or local merge/rebase)
2. resolve conflicts
3. rerun relevant checks/tests

This reduces "base branch was modified" merge failures.

### 6. Merge frequently

Do not let many ready PRs queue up while `main` is moving. Merge small, clean PRs quickly, especially in high-churn areas.

### 7. Call out hotspots in the PR description

List:
- issue(s) addressed
- lane (`p6`, `p7`, `p10`, `docs`, etc.)
- hot files touched (`app.rs`, `render/mod.rs`, shared docs)
- whether the PR is stack-dependent

This makes review and merge sequencing much easier.

