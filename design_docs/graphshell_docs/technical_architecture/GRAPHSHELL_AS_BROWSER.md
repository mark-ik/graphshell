# GRAPHSHELL AS A WEB BROWSER

**Document Type**: Thin behavioral summary
**Status**: Summary-only; canonical runtime behavior lives in implementation-strategy specs
**Purpose**: Explain the browser/workbench model at a user-visible level and point to the authority docs for exact contracts.

---

## 1. Browser Model in One Page

Graphshell is a browser where:

- pages and resources are graph-backed nodes,
- navigation creates or enriches traversal-aware edges,
- panes are workbench placements over graph truth,
- viewers are runtime attachments reconciled from graph/workbench state,
- history is a temporal system, not just a back/forward list.

User-visible consequences:

- opening content can create a node, activate an existing node, or focus an existing pane depending on route policy,
- navigation history is preserved both within a node and across node-to-node traversal,
- closing or deactivating a pane is not the same thing as deleting graph identity,
- graph, workbench, and viewer layers must stay synchronized through explicit intent/reconcile boundaries.

---

## 2. Canonical Behavioral Sources

| Behavior family | Canonical doc |
|---|---|
| Traversal recording, edge payloads, History Manager | `../implementation_strategy/subsystem_history/edge_traversal_spec.md` |
| History subsystem policy, replay isolation, health summary | `../implementation_strategy/subsystem_history/SUBSYSTEM_HISTORY.md` |
| Node lifecycle and ghost/tombstone behavior | `../implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| Viewer routing, fallback, presentation modes | `../implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md` |
| Workbench tile/frame semantics | `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md` |
| Graph-first frame semantics and graph citizenship | `../implementation_strategy/workbench/graph_first_frame_semantics_spec.md` |
| Pane opening mode and enrollment/promotion semantics | `../implementation_strategy/workbench/2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` |
| Input ownership and modal/focus return behavior | `../implementation_strategy/aspect_input/input_interaction_spec.md` |
| Focus restoration and region navigation | `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md` |
| Internal address/runtime route naming | `../implementation_strategy/system/2026-03-03_graphshell_address_scheme_implementation_plan.md` |

---

## 3. Browser Guarantees

These are stable high-level guarantees. Exact mechanisms belong to the docs above.

1. Node identity is not the current URL.
2. Traversal truth is reducer-owned and replayable.
3. Pane arrangement is workbench-owned, not viewer-owned.
4. Viewer lifecycle is reconcile-driven, not directly mutated by surface code.
5. Temporal preview/replay must not mutate live truth.
6. Runtime namespace is `verso://`; `graphshell://` is legacy/compatibility wording only where explicitly noted.

---

## 4. Current Closure Snapshot

- Core browsing, workbench routing, and traversal capture are active.
- History Manager is active.
- Four-state lifecycle is the canonical lifecycle.
- Faceted node filtering and facet-pane routing are specified but not closed in runtime.
- Temporal preview/replay remains backlog.
- Native-overlay/Wry integration remains scaffold-level.

For detailed status, use
`../implementation_strategy/2026-03-01_complete_feature_inventory.md`.

---

## 5. What This Doc Must Not Do

This file must not restate:

- Rust type definitions,
- transition matrices,
- traversal append skip rules,
- diagnostics channel inventories,
- CI/test gate details.

If a PR changes those concerns, update the canonical subsystem/spec doc first and
keep this file as a summary.
