<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workspace Routing and Membership Plan

**Date**: 2026-02-19
**Status**: Refactored (2026-02-22) into current-state maintenance + extension plan
**Persistence direction**: Named-workspace persistence internals are superseded by
`2026-02-22_workbench_workspace_manifest_persistence_plan.md` (workbench/workspace manifest model)

---

## Purpose

This document is no longer a greenfield implementation plan.

Workspace routing and membership are now largely implemented. This doc now serves as:

- the behavioral contract (invariants),
- the architecture boundary reference (what lives where),
- the validation checklist,
- and the prioritized upgrade path for follow-on improvements.

Named-workspace persistence schema evolution (stable UUID panes, manifest-backed membership) is
tracked in the separate workbench/workspace manifest persistence plan. This document remains the
behavioral/routing contract and UI integration reference.

---

## Current State Summary (2026-02-22)

Implemented and in active use:

- UUID-keyed workspace membership index (`node_workspace_membership`)
- Workspace-routed node open intent (`GraphIntent::OpenNodeWorkspaceRouted`)
- Single resolver path (`resolve_workspace_open`)
- Routed double-click / omnibar / radial menu open flows
- Choose-workspace picker for explicit routing
- Unsaved synthesized-workspace tracking and save prompt on workspace switch
- Graph-view membership badge + tooltip + badge click to workspace picker
- Workspace retention actions ("Prune empty", "Keep latest N named")
- Membership-index rebuilds after retention batch operations

Remaining value is now in refinement, resilience, and leverage of existing architecture.

---

## Behavioral Rules (Invariants)

1. Opening a node never creates fanout edges or modifies the graph.
2. Routing is context-preserving: restore an existing workspace when possible.
3. Workspace generation is an explicit fallback only for zero-membership nodes.
4. Generated fallback workspaces are **unsaved** (not auto-persisted); user must save explicitly.
   - If the user applies a graph-mutating action (`AddNode`, `AddEdge`, `RemoveNode`, `ClearGraph`)
     while the workspace is unsaved, set `unsaved_workspace_modified = true`.
   - On the next explicit workspace switch, prompt to save if this flag is set.
   - "Modified" is intentionally narrow: tile re-ordering and zoom do not count.
5. Deleting a workspace removes it from membership and recency candidates immediately.
6. The routing resolver is a single authority function. UI surfaces emit intents; they do not
   perform direct tile mutations for routed-open behavior.
7. If restoring a chosen workspace yields an empty or unusable workbench tree after restore-time
   resolution/pruning, fall back to opening the node in the current workspace (warning log, no panic).
8. Membership is keyed by stable node UUID (`Node.id`), not session-local `NodeKey`.

---

## Architecture Boundaries (How This Uses Current Design)

This feature set fits the project's post-decomposition boundaries well. Preserve these seams.

### `app.rs` (policy/state/reducer authority)

Owns:

- `node_workspace_membership: HashMap<Uuid, BTreeSet<String>>`
- `current_workspace_is_unsaved`
- `unsaved_workspace_modified`
- `pending_node_context_target`
- `resolve_workspace_open(...)`
- `GraphIntent::OpenNodeWorkspaceRouted`
- recency selection policy (`node_last_active_workspace`)

Responsibilities:

- choose routing outcome deterministically
- track unsaved-workspace policy
- maintain membership incrementally for app-layer events (restore, delete, remove-node)
- expose query helpers (`membership_for_node`, `workspaces_for_node_key`, sorted variants)

### `desktop/persistence_ops.rs` (desktop/persistence effects)

Owns (current implementation):

- `build_membership_index_from_layouts(&GraphBrowserApp) -> HashMap<Uuid, BTreeSet<String>>`
- retention batch operations (`prune_empty_named_workspaces`, `keep_latest_named_workspaces`)

Responsibilities (current implementation):

- deserialize named-workspace persistence payloads / workbench layout trees (`Tree<TileKind>`)
- prune stale `NodeKey`s before deriving membership
- rebuild membership index after batch persistence mutations

Preferred future model (tracked separately):

- `build_membership_index_from_workspace_manifests(...)` using named-workspace manifests
- no named-workspace `NodeKey` prune pass required for membership derivation

Reason this stays here:

- `TileKind` and tile-tree deserialization are desktop-layer concerns
- `app.rs` intentionally does not import desktop tile types

### `desktop/gui_frame.rs` (effect orchestration / frame sequencing)

Responsibilities:

- execute pending restore/save actions
- apply fallback behavior after restore failures/empty pruned trees
- trigger membership rebuilds after persistence mutations
- synthesize live workspaces (workbench trees) for "neighbors"/"connected" open modes

Policy note:

- `MAX_CONNECTED_SPLIT_PANES` is the effective cap for synthesized opens.
- Current value is `4` (as of 2026-02-22). Avoid hardcoding a numeric cap in plan text.

### `render/mod.rs` (UI event capture and presentation)

Responsibilities:

- capture double-click / context / radial / command-palette input
- emit workspace-routed intents
- render choose-workspace picker and unsaved-workspace prompt
- pass membership metadata into graph rendering adapter

Constraint:

- render code should not bypass the routing resolver for "Open in Workspace" behavior.

### `graph/egui_adapter.rs` (graph rendering details)

Responsibilities:

- membership badge rendering on nodes
- badge hit-test support
- store display-only membership metadata (`count`, `names`)
- adapter construction via membership-aware path (`from_graph_with_memberships(...)`)

---

## Architectural Leverage (Use What Already Exists)

When extending this system, build on the existing hooks instead of adding parallel flows:

- `GraphIntent::OpenNodeWorkspaceRouted` for all workspace-routed opens
- `resolve_workspace_open(...)` as the single decision function
- `build_membership_index_from_layouts(...)` for correctness after batch persistence operations
- choose-workspace picker UI (`render_choose_workspace_picker(...)`) for explicit workspace selection
- membership badge metadata injection (`from_graph_with_memberships(...)`) for graph-view affordances
- retention ops in `desktop/persistence_ops.rs` instead of ad hoc workspace deletion loops

Anti-patterns to avoid:

- duplicate resolver logic in UI code
- direct tile mutation in render paths for routed opens
- membership scans in `app.rs` that deserialize `TileKind`
- maintaining parallel membership caches with different invalidation rules

---

## Known Constraints and Rationale

### NodeKey Instability (Current Named-Workspace Format)

`NodeKey` (`petgraph::NodeIndex`) is stable only within a session. The current named-workspace
format persists `NodeKey`s, so stale keys must be pruned before membership derivation.
Membership therefore uses `Node.id` (UUID) as the stable key.

This constraint is the primary reason for the manifest-backed workbench/workspace persistence plan,
which removes `NodeKey` from named-workspace persistence entirely.

### Layer Constraint: `TileKind` Is Desktop-Only

Workspace parsing and runtime tile conversion remain desktop-layer concerns. `GraphBrowserApp`
receives rebuilt membership via `init_membership_index(...)` rather than parsing workbench
persistence directly.

### Recency Is Session-Local Today

`node_last_active_workspace` is currently keyed by `NodeKey`, which means recency does not survive
restart. This is acceptable for current behavior but is the highest-value follow-on improvement.

### Right-Click Detection in `egui_graphs`

There is still no direct right-click node event in the graph widget path used here; right-click
targeting depends on pointer secondary-click + hovered node state.

---

## Validation Checklist (Contract-Level)

1. **Node in 1 workspace**: default open restores that workspace; no fallback workspace created.
2. **Node in N workspaces**: default open picks highest-recency workspace; explicit picker opens a specific one.
3. **Node in 0 workspaces**: default open falls back to current workspace unsaved open; no named workspace auto-persist.
4. **Open with Neighbors/Connected**: synthesized workspace contains intended traversal set, capped by `MAX_CONNECTED_SPLIT_PANES`.
5. **Workspace restore empty after resolve/prune**: falls back to current-workspace open and logs warning.
6. **Workspace delete**: removed from membership and recency candidates immediately.
7. **Node URL change**: membership index unchanged (UUID stable).
8. **Node removed**: UUID entry removed from membership index.
9. **Startup membership init**: membership index available before first graph render path relies on it.
10. **Batch retention prune**: membership index rebuilt after completion; no stale entries remain.
11. **Resolver determinism**: identical inputs produce identical `WorkspaceOpenAction`.
12. **Unsaved modification semantics**: graph-mutating actions while unsaved set prompt flag; non-graph UI actions do not.

### Automated Coverage Present

- `app::tests::test_set_node_url_preserves_workspace_membership`
- `app::tests::test_resolve_workspace_open_deterministic_fallback_without_recency_match`
- resolver preference tests in `app::tests::test_resolve_workspace_open_*`
- `desktop::persistence_ops::tests::test_build_membership_index_from_layouts_skips_reserved_and_stale_nodes`
- `desktop::persistence_ops::tests::test_prune_empty_named_workspaces_rebuilds_membership_index`
- `desktop::persistence_ops::tests::test_keep_latest_named_workspaces_rebuilds_membership_index`
- graph membership badge adapter tests in `graph::egui_adapter::tests::*membership_badge*`

### Headed Manual Tracking

Remaining manual validations are tracked in:

- `ports/graphshell/design_docs/graphshell_docs/tests/VALIDATION_TESTING.md`
  (`Workspace Routing and Membership (Headed Manual)`)

---

## Prioritized Extension Workstreams (Next Iteration)

These are the recommended follow-ons that best exploit the current architecture.

### Workstream A: Persist Recency by UUID (Highest Value)

Problem:

- resolver recency is session-local because `node_last_active_workspace` is keyed by `NodeKey`

Upgrade:

- move recency tracking key from `NodeKey` to `Uuid` (or maintain a compatibility migration path)

Benefits:

- routing preference survives restarts
- removes `uuid -> NodeKey` translation in resolver recency lookup
- improves determinism for repeated workflows across sessions

Design notes:

- keep resolver API unchanged initially; migrate internals first
- preserve tie-breaker fallback to alphabetical order for deterministic behavior

Status note:

- This workstream should be implemented alongside the workbench/workspace manifest persistence
  migration so recency persistence aligns with stable UUID-based named-workspace storage.

### Workstream B: Resolver Strategy and Explainability

Problem:

- resolver policy is fixed and opaque during debugging

Upgrade:

- add a small strategy layer (e.g. `RecentThenAlpha`, `Alphabetical`, `ExplicitOnly`)
- add debug logging/tracing payload for "why this workspace was chosen"

Benefits:

- easier behavior tuning without UI rewrites
- simpler regression triage for routing surprises

Design notes:

- keep `resolve_workspace_open(...)` as single authority
- do not expose multiple codepaths that bypass it

### Workstream C: Centralize Membership Rebuild Triggers

Problem:

- rebuild calls are correct but spread across multiple effect sites

Upgrade:

- add a small helper in the desktop effect layer for "rebuild membership index now"
- optionally standardize post-batch mutation flow in one function

Benefits:

- lowers risk of future retention/persistence features forgetting to rebuild
- makes batch operations easier to audit

Design notes:

- this is a refactor for consistency, not behavior change

Status note:

- In the manifest model, this becomes "centralize membership cache rebuild/update triggers from
  workspace manifests"; the routing requirements here remain the same.

### Workstream D: Membership-Aware UI Enhancements (Low Risk)

Examples:

- richer badge tooltip ordering (recency-sorted names)
- small badge visual distinction for "current routed target" or "recent workspace"
- command-palette affordances that surface workspace membership count directly

Benefits:

- better discoverability with minimal architectural impact

Design notes:

- use existing `from_graph_with_memberships(...)` injection path
- keep graph adapter display-only; no policy decisions in render layer

### Workstream E: Batch Workspace Operations via Intents (Optional Discipline Tightening)

Problem:

- some persistence-hub actions may be invoked directly from UI/effect orchestration paths

Upgrade:

- represent batch retention actions as explicit intents/requests where useful

Benefits:

- tighter consistency with reducer-first architecture
- easier testability of request state and prompt interactions

Non-goal:

- do not force every file I/O operation through the reducer if it harms simplicity

---

## Suggested Refactor Rules for Future Changes

1. Add new open modes by extending the existing routed-open intent path, not by creating new direct tile mutations.
2. Treat membership index correctness as desktop-layer persistence read/update + app-layer cache.
   In the current implementation this is layout-derived; in the manifest model it is manifest-derived.
3. Keep rendering modules display-oriented: UI may request actions, but resolver and unsaved-workspace policy stay in `app.rs`.
4. Prefer constants and policy names over numeric values in docs (example: use `MAX_CONNECTED_SPLIT_PANES` instead of hardcoding `12`).
5. When adding batch workspace features, include membership-index rebuild behavior in the same change and tests.

---

## Out of Scope (This Doc)

1. Full multi-window architecture changes.
2. Non-workspace graph semantics (edge taxonomy changes).
3. Command palette redesign beyond routing and workspace-selection integration.
4. Bookmarks, node versioning/history.
5. Large Persistence Hub redesign unrelated to routing/membership correctness.

---

## Historical Notes (Original Plan Context)

This document began as a draft implementation plan on 2026-02-19 and was revised multiple times
to address:

- NodeKey instability and UUID-keyed membership indexing
- desktop-layer `TileKind` constraints
- unsaved synthesized-workspace semantics
- resolver determinism and fallback behavior
- right-click targeting limitations in `egui_graphs`

As of 2026-02-22, the core plan has been implemented and the doc has been restructured to serve as
an operational reference and extension roadmap.

As of 2026-02-22, named-workspace persistence redesign is split into the dedicated workbench/workspace
manifest persistence plan to reduce coupling between routing behavior and persistence schema work.
