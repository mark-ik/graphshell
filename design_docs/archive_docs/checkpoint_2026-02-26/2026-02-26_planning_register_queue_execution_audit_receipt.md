# 2026-02-26 Planning Register Queue Execution Audit Receipt

Date window: 2026-02-26 (queue execution audit + targeted patch)

## Purpose

Document the results of executing the current canonical issue queue from
`PLANNING_REGISTER.md` and record which queued issues were already implemented
in-tree vs. which still required a patch.

This receipt is an execution/status audit only. It does not rewrite issue
priority or lane ordering.

## Canonical sequence audited

Source: `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`

- `lane:p6 pane/workbench architecture`: `#76 -> #77`
- `lane:p7 viewer/content/registry alignment (phase 1)`: `#78 -> #68 -> #69 -> #70`
- `lane:p6 graph multi-view stack`: `#63 -> #64 -> #65 -> #66 -> #67`
- `lane:p6 storage/schema follow-up`: `#79`
- `lane:p10 baseline completion`: `#74 -> #75 -> #73`

## Code changes made during this audit

### Patched (`#70`)

Issue: `#70` "Universal content (P7.c): lifecycle integration for ViewerRegistry::select_for"

Observed gap:
- The lifecycle prewarm path in `shell/desktop/ui/gui_frame.rs` still assumed
  `Active selected node => ensure webview runtime`, bypassing viewer policy.

Patch:
- Prewarm webview creation now checks whether the selected node's default
  `NodePaneState` resolves to a webview-hosting viewer using
  `tile_runtime::node_pane_hosts_webview_runtime(...)` before calling
  `ensure_webview_for_node(...)`.

Result:
- Lifecycle/prewarm behavior now respects the same viewer-policy semantics used
  in tile runtime and node-pane opening paths.

Validation:
- `cargo check -q` passed (warnings only).

## Issues audited as already implemented in-tree

### `#77` (`lane:p6`)

Status from audit: implemented

Evidence highlights:
- Tool-pane render/title/focus dispatch keyed by `ToolPaneState`
- Diagnostics focus routing and targeted tests already present

### `#78` (`lane:p7`)

Status from audit: partially refined during this session; core semantics already present

Changes made in worktree (uncommitted at audit time):
- Removed stale alias helper names that implied generic node panes are webview hosts
- Added explicit `node_pane_hosts_webview_runtime(...)` predicate
- Fixed `toggle_tile_view(...)` to avoid unconditionally creating webview runtime
  for every opened node pane
- Terminology cleanup in invariants/post-render helpers

### `#68` (`lane:p7`)

Status from audit: implemented

Evidence highlights:
- Node `mime_hint` / `address_kind`
- reducer + intents + persistence/WAL handling
- tests present

### `#69` (`lane:p7`)

Status from audit: implemented

Evidence highlights:
- MIME detection helpers (magic bytes + extension fallback)
- reducer avoids redundant MIME writes
- tests present for detection behavior

### `#63`-`#67` (`lane:p6` graph multi-view stack)

Status from audit: implemented

Evidence highlights:
- `GraphViewId` + per-view `GraphViewState` map + focused view tracking
- graph pane open/split/focus preserves `GraphViewId`
- render path keyed by `GraphViewId`
- per-pane lens UI and split graph pane flow
- Canonical/Divergent controls + explicit commit stub
- reducer semantics and tests for layout mode transitions

### `#79` (`lane:p6` storage/schema follow-up)

Status from audit: implemented

Evidence highlights:
- Persisted pane schema aligned to pane model (`NodePane`, `Tool { kind }`)
- compat support for legacy `WebViewNode` alias and `Diagnostic` tile
- compatibility + schema-term tests in `persistence_ops`

### `#74` (`lane:p10` WebView a11y bridge graft)

Status from audit: implemented (issue body appears stale relative to code)

Evidence highlights:
- WebView accessibility updates are queued and injected into egui AccessKit tree
- stable anchor IDs, graft planning, degraded role conversion fallback logging
- bridge-focused unit tests

### `#75` (`lane:p10` a11y validation harness/manual checks)

Status from audit: implemented (documentation/testing collateral)

Evidence highlights:
- `design_docs/graphshell_docs/testing/VALIDATION_TESTING.md` contains:
  - automated P10.d harness commands
  - repeatable manual screen-reader checklist
  - handoff evidence template

### `#73` (`lane:p10` culling validation + benchmark instrumentation)

Status from audit: implemented (tests + validation doc)

Evidence highlights:
- viewport culling metrics/benchmark tests in `render/mod.rs`
- P10.b validation checklist + capture template in
  `design_docs/graphshell_docs/testing/VALIDATION_TESTING.md`

## Outcome / queue state implication

The audited canonical sequence is materially ahead of the GitHub issue status
queue. Multiple issues remain open but appear implemented in-tree.

Operational implication:
- Next execution passes should prefer "acceptance audit + close/update issue"
  before coding, to reduce duplicate work and queue drift.

## Recommended follow-up (documentation / tracker hygiene)

1. Update issue comments/status for audited issues with file-level evidence.
2. Use hub issue `#86` to track stale-open issue reconciliation and sequencing
   status refresh.
3. Refresh `PLANNING_REGISTER.md` queue notes (or add a new receipt) after issue
   status reconciliation lands.
