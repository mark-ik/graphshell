<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workbench Tab Semantics Overlay and Promotion Plan

**Date**: 2026-02-22
**Status**: Implementation-Ready (updated 2026-02-23)
**Relates to**:

- `2026-02-22_workbench_workspace_manifest_persistence_plan.md` — completed manifest migration foundation; `PaneId` and `WorkspaceManifest` types defined there, plus workspace membership/routing context lineage
- `2026-02-22_registry_layer_plan.md` — `WorkbenchSurfaceRegistry` (Phase 3, complete) owns layout and interaction policy for tile containers; `GraphWorkspace`/`AppServices` split (Phase 6) is the future home of `WorkspaceTabSemantics`

---

## Purpose

Add a thin tab-semantics overlay subsystem on top of the workbench (`egui_tiles`) so Graphshell
can preserve tab-specific behavior through structural normalization (including `simplify()`), while
supporting pane rest states and on-demand promotion/demotion back into tab containers.

Stage 8A (design lock) is complete — this document is the design. The remaining stages are
implementation work.

---

## Terminology

- **Visual tabs**: current `egui_tiles::Container::Tabs` shape in the live workbench tree
- **Semantic tabs**: Graphshell tab/group meaning persisted in overlay metadata
- **Pane rest state**: a valid pane-only representation of a semantic tab/group after demotion or `simplify()`
- **Hoist / Unhoist**: structural workbench tree transforms only (egui_tiles operations)
- **Promote / Demote**: semantic + structural lifecycle actions that may trigger hoist/unhoist
- **PaneId**: stable pane identity from the manifest-backed workspace bundle model (defined in `2026-02-22_workbench_workspace_manifest_persistence_plan.md`)

Resolved semantics:

- A semantic tab is any tab/tab-group with saved overlay metadata.
- A single pane with overlay metadata counts for all tab-aware features even when currently unhoisted.
- Graph panes and webview panes are both eligible for semantic tab/group membership.
- A pane belongs to at most one semantic tab group at a time.

---

## Design Principles

- The `egui_tiles` workbench tree is structural state, not semantic truth.
- Semantic metadata is optional and additive.
- Restore prioritizes content availability first, semantic exactness second, exact structural shape third.
- Promotion/demotion between pane and tab-container forms is first-class behavior, not an exception path.
- Graphshell remains compatible with dependency-native transforms (`simplify()`) by persisting semantics explicitly instead of relying on incidental tree shape.

---

## Architecture Integration Points

### WorkbenchSurfaceRegistry (Phase 3 — complete)

The `WorkbenchSurfaceRegistry` layout policy section owns simplification options and tab container
rules. When `egui_tiles::simplify()` runs, it should consult the registry's `SimplificationOptions`
rather than using hardcoded defaults. The overlay's promote/demote operations must remain consistent
with whatever simplification policy is active.

### GraphWorkspace / AppServices (Phase 6 — planned)

`WorkspaceTabSemantics` is pure data and belongs in `GraphWorkspace` after Phase 6. Until then it
lives alongside the workspace manifest. The overlay must not depend on runtime handles (`AppServices`
fields) — it is serializable state only.

### Intent Boundary

- `render/*` captures UI events only; it must not directly mutate overlay semantics.
- `app.rs` is the authority for semantic decisions (promotion/demotion intents, repair policy, warning creation).
- `desktop/*` applies workbench tree mutations and runtime effects in response to app-level intents.
- Promotion/demotion are explicit `GraphIntent` variants, not ad hoc tree rewrites at UI callsites.

---

## Overlay Schema

Minimum planned shape; use `Uuid` for all IDs to be consistent with the codebase identity model.

```rust
pub type TabGroupId = Uuid;

pub struct WorkspaceTabSemantics {
    pub version: u32,                      // schema version for rkyv migration
    pub tab_groups: Vec<TabGroupMetadata>,
}

pub struct TabGroupMetadata {
    pub group_id: TabGroupId,
    pub pane_ids: Vec<PaneId>,             // ordered tab membership
    pub active_pane_id: Option<PaneId>,    // must be a member or repaired to None
}
```

Persistence: serialized with rkyv and stored in the workspace bundle (redb). This is workspace
state, not graph WAL data — it must not appear in fjall `LogEntry` variants.

Optional future additions (schema-additive, not breaking):

- Group-level UI metadata: label, color, collapsed/rest preference
- Pane-level metadata: title override, pin state
- Timestamps: `last_promoted_at`, `last_demoted_at`
- Diagnostics: `last_repair_at`, `last_repair_reason`

---

## Source-of-Truth Split

| Layer | Type | Contents |
| ----- | ---- | -------- |
| Manifest layout | `WorkspaceLayout` | Structural egui_tiles tree |
| Manifest identity | `WorkspaceManifest` | Pane identity and node membership |
| Semantic overlay | `WorkspaceTabSemantics` | Tab group order, active pane |

These three must not overlap. The overlay queries shape only as a fallback when overlay metadata is
absent.

---

## Overlay Query Precedence

For all tab-aware behavior (omnibar saved tabs, pin UI, tab affordances):

1. Semantic overlay metadata
2. Live workbench tree shape inference
3. Pane-only fallback behavior

Implement through shared helper APIs — not per-feature ad hoc logic.

Proposed helper surface (exact module TBD, likely `desktop/workbench_semantics.rs`):

```rust
fn semantic_tab_groups_for_workspace(semantics: &WorkspaceTabSemantics) -> &[TabGroupMetadata];
fn saved_tab_nodes_for_workspace(semantics: Option<&WorkspaceTabSemantics>, tree: &Tree<TileKind>) -> Vec<NodeKey>;
fn pane_semantic_tab_state(pane_id: PaneId, semantics: Option<&WorkspaceTabSemantics>) -> PaneSemanticState;
```

---

## Promotion / Demotion Semantics

### Demote: semantic tab/group → pane rest state

- Persist/refresh overlay metadata before structural change.
- Allow tabs container to be simplified/unhoisted into pane rest state.
- Preserve content visibility (last paint / thumbnail fallback when runtime content suspends).
- Keep pane fully usable while tab chrome is hidden.

### Promote: pane rest state → semantic tab/group

- Re-wrap pane into tabs container (egui_tiles structural operation).
- Reload/reapply overlay metadata: group membership, order, active pane.
- Restore normal tab interactions and affordances.

Ordering invariant (must not be violated):

1. Semantic decision — `GraphIntent` applied by reducer
2. Workbench tree mutation — desktop layer applies structural rewrap
3. Lifecycle/runtime dispatch — resume/suspend webviews as needed
4. UI readiness update — on runtime confirmation

Both promote and demote must be idempotent. Pane rest state is a valid intermediate state while
runtime content is suspended or resuming.

### Intent Variants

```rust
GraphIntent::PromotePaneToSemanticTabGroup {
    pane_id: PaneId,
    group_id: TabGroupId,   // existing group to rejoin, or new
}
GraphIntent::DemoteSemanticTabGroupToPaneRest {
    group_id: TabGroupId,
}
GraphIntent::RepairWorkspaceTabSemantics {
    workspace_name: String,
}
```

The desktop apply layer handles tree rewrites, lifecycle dispatch, and UI state updates in response
to these intents.

---

## Single-Pane On-Demand Affordance

When a semantic tab group has been demoted to pane rest state, expose a small inverted-tab control:

- Anchored near the pane viewport margin (upper-left, configurable via `WorkbenchSurfaceRegistry`
  interaction policy).
- Hidden by default; revealed when cursor approaches the tab region.
- Click dispatches `PromotePaneToSemanticTabGroup` intent.
- Keyboard-accessible equivalent command must exist (not mouse-only); registered in `InputRegistry`.

---

## Validation / Repair Policy

### Invariants

- Every `pane_id` in overlay metadata exists in the workspace manifest.
- No pane belongs to more than one semantic tab group at once.
- `active_pane_id` is either `None` or a member of the same group.
- Group `pane_ids` ordering is deterministic and free of duplicates.

### Repair behavior

- Auto-repair invalid metadata when safe.
- Preserve visible panes even if some semantic metadata must be dropped or corrected.
- Reapply remaining valid semantics.
- Repair must not depend on runtime webview availability.
- Dispatch `RepairWorkspaceTabSemantics` intent; reducer applies repair; desktop layer updates UI.

### Warning policy

- Aggregate all repairs per workspace restore/load operation.
- Emit detailed repair events to debug logs.
- Emit at most one user-facing warning per restore/load by default.
- Warning text must include: workspace name, affected group/pane IDs, exact repair action, what was
  preserved, what changed.

Example: `Workspace 'research-1': repaired tab group g42 (missing active pane p9). Preserved panes [p3,p7]; active tab reset to p3.`

---

## Stage 8 Execution Plan

Stage 8A (design lock) is complete. The following stages are implementation work.

### Stage 8B: Overlay Persistence + Validation

Goal: persist optional tab semantics metadata in the workspace bundle and validate/repair it on load.

- Extend bundle schema with optional `WorkspaceTabSemantics` field (rkyv, additive — bundle load
  works with or without overlay present).
- Implement validation helpers: check each invariant in order, collect all violations before repairing.
- Implement repair helpers: drop/correct invalid entries; preserve valid entries; return repair log.
- Implement `RepairWorkspaceTabSemantics` intent handling in reducer.
- Add roundtrip tests: bundle with overlay, bundle without overlay, bundle with invalid overlay.
- Add repair invariant tests: duplicate pane, invalid active pane, missing pane.

Done gate: bundle load/save roundtrip passes with and without overlay. Invalid overlay repairs are
deterministic and test-covered. Repair emits structured log entries.

### Stage 8C: Overlay-First Query APIs + Consumer Migration

Goal: route all tab-aware features through shared semantic queries; eliminate direct tree-shape
inference from consumers.

- Add overlay-first helper APIs in `desktop/workbench_semantics.rs`.
- Update omnibar saved-tab discovery to use `saved_tab_nodes_for_workspace()`.
- Update pin UI tab-aware queries to use `pane_semantic_tab_state()`.
- Preserve tree-shape fallback in the helpers during rollout; do not require overlay presence.
- No new direct tree-shape inference paths in migrated consumers.

Done gate: tab-aware feature behavior is stable under tree-shape normalization. `cargo grep` for
direct `Tree<TileKind>` access outside `desktop/workbench_semantics.rs` returns no new callsites
for tab-semantic queries.

### Stage 8D: Promotion / Demotion + Pane Rest State

Goal: implement semantic lifecycle transitions and on-demand rewrap affordance.

- Add `PromotePaneToSemanticTabGroup`, `DemoteSemanticTabGroupToPaneRest` intent variants and
  reducer handling.
- Desktop apply layer: implement tree rewrap (hoist/unhoist), runtime lifecycle dispatch
  (resume/suspend), UI readiness update on confirmation.
- Implement idempotency: applying promote/demote twice has same effect as once.
- Implement single-pane inverted-tab affordance in `desktop/ui/toolbar/` or pane chrome render
  (exact location TBD by render pass structure). Register keyboard equivalent in `InputRegistry`.
- Connect lifecycle ordering: promote waits for runtime confirmation before updating UI readiness.

Done gate: demoted semantic tabs can be promoted back with metadata fully restored. Pane rest state
is usable while runtime content is suspended or resuming. Promote/demote are idempotent under
repeated application.

### Stage 8E: Simplify-Safe Restore Integration

Goal: allow `egui_tiles::simplify()` to run without losing Graphshell tab semantics.

- Define the structural simplify + semantic reapply pipeline:
  1. Run `simplify()`.
  2. Detect which pane IDs changed container membership.
  3. For each affected group in overlay, check if members still exist.
  4. If a group collapsed to a single pane, keep overlay metadata (pane rest state).
  5. If pane IDs were remapped by simplify (currently not the case with stable PaneId, but guard
     against it), update overlay membership accordingly.
- Add cross-transform roundtrip tests: `save → restore → simplify → reapply → save` preserves
  overlay semantics.
- Document compatibility guarantees in this plan's Findings section once confirmed.

Done gate: `egui_tiles::simplify()` no longer conflicts with Graphshell tab semantics. No pane loss
or tab metadata loss under supported transforms.

---

## Test and Validation Plan

- Roundtrip tests with/without overlay metadata
- Repair invariant tests: duplicate pane, invalid active pane, missing pane
- Query precedence tests: overlay first, shape fallback, pane-only fallback
- Promote/demote idempotency tests
- Simplify/reapply equivalence tests: semantic invariants preserved across simplify
- Warning aggregation tests: at most one user-facing warning per restore operation
- Lifecycle ordering tests: promote while runtime is suspended or resuming

---

## Risks and Mitigations

Overlay becoming a second layout engine: keep overlay semantic-only (groups/order/active), not a
structural replacement. Clear ownership split with `WorkspaceLayout`.

Feature code bypassing helpers: shared overlay-first query APIs are the only permitted path for
tab-aware queries. Migrate high-risk consumers first (omnibar, pin UI).

Repair warnings creating user confusion: exact aggregated messages with content-preservation-first
repair policy. Debug logs for detail.

Async races during promote/demote: explicit intent ordering, idempotent operations, pane rest state
as valid intermediate state.

`WorkbenchSurfaceRegistry` diverging from overlay assumptions: simplification options live in the
registry; overlay promote/demote reads those options. If simplify policy changes, the
simplify-reapply pipeline (Stage 8E) must be re-validated.

---

## Workspace Routing Polish Addendum (Absorbed 2026-02-24)

This section absorbs and replaces `2026-02-24_workspace_routing_polish_plan.md`.

### Resolver explainability

- Emit structured resolver traces from `resolve_workspace_open`: candidates, recency scores, selected workspace, and decision reason (`MostRecent`, `ExplicitTarget`, `Fallback`).
- Surface traces through diagnostics/logging for headed validation and bug triage.
- Optional preference strategy remains constrained to resolver policy selection, not alternate UI-side routing logic.

### Membership-aware UI affordances

- Badge tooltips should list memberships in recency order and highlight current workspace.
- Workspace-target actions in command palette should include membership hints.
- Hide badge noise for nodes that belong only to the current workspace; keep stronger visual treatment for external membership.

### Batch operation intent boundary

- Route prune/retention operations through intent/request paths (`GraphIntent::PruneEmptyWorkspaces`, `GraphIntent::RetentionSweep { max_snapshots }`) rather than direct maintenance shortcuts.
- Keep persistence effects in desktop helper layers, but preserve intent-layer observability.

### Validation additions

- [ ] Multi-home open emits resolver trace with ranking and decision reason.
- [ ] Membership badges follow local-only vs external-membership visibility rules.
- [ ] Command palette workspace-target entries expose membership hints.
- [ ] Batch prune/retention operations are visible in intent/request diagnostics.

---

## Findings

To be populated during and after Stage 8B–8E implementation. Document compatibility guarantees from
Stage 8E here once confirmed.

---

## Progress

### 2026-02-22

- Plan created. Scope, design principles, and target model established.
- Stage 8A (design lock) declared complete.

### 2026-02-23 (implementation-ready revision)

- Aligned to registry layer: `WorkbenchSurfaceRegistry` (Phase 3, complete) noted as owner of
  simplification options; overlay must be consistent with active simplification policy.
- `TabGroupId` changed from `u64` to `Uuid` for consistency with the codebase identity model.
- `GraphWorkspace`/`AppServices` split (Phase 6) noted as future home of `WorkspaceTabSemantics`.
- Persistence path made explicit: rkyv/redb workspace bundle, not fjall WAL.
- `GraphIntent` variants made concrete with field signatures.
- Stage 8A declared complete (design document is the output); stages 8B–8E are now the
  implementation work items with explicit done gates.
- Source-of-truth split table added.
- `WorkbenchSurfaceRegistry` risk added to Risks section.
- Helper API surface and module location (`desktop/workbench_semantics.rs`) made concrete.
