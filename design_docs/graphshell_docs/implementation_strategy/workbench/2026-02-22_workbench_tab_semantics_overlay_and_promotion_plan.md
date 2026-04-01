<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workbench Semantic Tab Overlay and Pane-Rest Execution Note

**Date**: 2026-02-22
**Status**: Active execution note — planned feature, not yet implemented in runtime code

**Canonical authority chain**:

- `../graph/multi_view_pane_spec.md §7` — canonical contract for `FrameTabSemantics`, hoist/unhoist, pane-rest semantics, and simplify-safe invariants
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` — canonical authority for `PaneOpeningMode`, `SimplificationSuppressed`, and graph-enrollment promotion semantics
- `pane_chrome_and_promotion_spec.md` — pane chrome and opening semantics cross-reference only; not the canonical owner of `FrameTabSemantics`

**Relates to**:

- `../../archive_docs/checkpoint_2026-02-22/2026-02-22_workbench_workspace_manifest_persistence_plan.md` — completed manifest migration foundation (archived 2026-02-24); `PaneId` and `FrameManifest` types defined there, plus frame membership/routing context lineage
- `frame_persistence_format_spec.md` — canonical current persisted frame-bundle shape; semantic tab overlay remains a planned additive extension there
- `../system/register/workbench_surface_registry_spec.md` — `WorkbenchSurfaceRegistry` owns layout/interaction policy for tile containers, including simplification options; overlay restore/collapse logic must read those options (the former `2026-02-22_registry_layer_plan.md` Phase 3 is now canonicalized here)
- `../system/2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md` — current `GraphWorkspace`/`AppServices` field-ownership status; `FrameTabSemantics` target home after state-ownership migration

---

## Purpose

This note keeps the `FrameTabSemantics` direction active as an implementation guide without
pretending the feature has already landed.

The target remains the same:

- preserve semantic tab membership through structural normalization such as `simplify()`
- allow pane-rest states to retain tab-aware meaning
- restore tab containers on demand without conflating hoist/unhoist with graph-enrollment promotion

Current reality:

- runtime code does **not** yet contain a `FrameTabSemantics` carrier
- runtime code does **not** yet contain the planned restore/collapse intents in this note
- some tab-aware behavior still infers semantics directly from live tile-tree shape
- the concept remains intended architecture and is still referenced by active graph/workbench docs

---

## Terminology

- **Visual tabs**: current `egui_tiles::Container::Tabs` shape in the live workbench tree
- **Semantic tabs**: Graphshell tab/group meaning persisted in overlay metadata
- **Pane rest state**: a valid pane-only representation of a semantic tab/group after collapse or `simplify()`
- **Hoist / Unhoist**: structural workbench tree transforms only (egui_tiles operations)
- **Restore Tabs / Collapse to Pane Rest**: semantic + structural lifecycle actions in this plan that may trigger hoist/unhoist
- **PaneId**: stable pane identity from the manifest-backed frame bundle model (defined in `2026-02-22_workbench_workspace_manifest_persistence_plan.md`)

Terminology guardrail:

- **Promotion** remains reserved for graph-enrollment / graph-citizenship semantics elsewhere in Graphshell and is intentionally not used as the canonical lifecycle term in this plan.

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
- Restore/collapse transitions between pane and tab-container forms are first-class behavior, not an exception path.
- Graphshell remains compatible with dependency-native transforms (`simplify()`) by persisting semantics explicitly instead of relying on incidental tree shape.

---

## Architecture Integration Points

### WorkbenchSurfaceRegistry

The `WorkbenchSurfaceRegistry` layout policy section (see `../system/register/workbench_surface_registry_spec.md`) owns simplification options and tab container rules. When `egui_tiles::simplify()` runs, it should consult the registry's `SimplificationOptions` rather than using hardcoded defaults. The overlay's restore/collapse operations must remain consistent with whatever simplification policy is active.

### GraphWorkspace / AppServices state ownership

`FrameTabSemantics` is pure data and belongs in `GraphWorkspace`. Until the state-ownership migration in `../system/2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md` is complete, it lives alongside the frame manifest. The overlay must not depend on runtime handles (`AppServices` fields) — it is serializable state only.

### Intent Boundary

- UI/render code captures events only; it must not directly mutate overlay semantics.
- The reducer / app layer is the authority for semantic decisions (restore/collapse intents, repair policy, warning creation).
- The desktop/workbench apply layer mutates tree/runtime state in response to reducer-owned intents.
- Restore/collapse actions are explicit `GraphIntent` variants, not ad hoc tree rewrites at UI callsites.

---

## Overlay Schema

Minimum planned shape; use `Uuid` for all IDs to be consistent with the codebase identity model.

```rust
pub type TabGroupId = Uuid;

pub struct FrameTabSemantics {
    pub version: u32,                      // schema version for rkyv migration
    pub tab_groups: Vec<TabGroupMetadata>,
}

pub struct TabGroupMetadata {
    pub group_id: TabGroupId,
    pub pane_ids: Vec<PaneId>,             // ordered tab membership
    pub active_pane_id: Option<PaneId>,    // must be a member or repaired to None
}
```

Planned persistence target: serialized with rkyv and stored in the frame bundle as additive frame
state. This is frame state, not graph WAL data — it must not appear in fjall `LogEntry` variants.

Optional future additions (schema-additive, not breaking):

- Group-level UI metadata: label, color, collapsed/rest preference
- Pane-level metadata: title override, pin state
- Timestamps: `last_restored_at`, `last_collapsed_at`
- Diagnostics: `last_repair_at`, `last_repair_reason`

---

## Source-of-Truth Split

| Layer | Type | Contents |
| ----- | ---- | -------- |
| Manifest layout | `FrameLayout` | Structural egui_tiles tree |
| Manifest identity | `FrameManifest` | Pane identity and node membership |
| Semantic overlay | `FrameTabSemantics` | Tab group order, active pane |

These three must not overlap. The overlay queries shape only as a fallback when overlay metadata is
absent.

---

## Overlay Query Precedence

For all tab-aware behavior (omnibar saved tabs, pin UI, tab affordances):

1. Semantic overlay metadata
2. Live workbench tree shape inference
3. Pane-only fallback behavior

Implement through shared helper APIs — not per-feature ad hoc logic.

Current gap this work is meant to close:

- omnibar saved-tab discovery still reads tab membership from tile-tree shape directly instead of an overlay-first semantic helper path

Proposed helper surface (exact module TBD; name/location should be chosen to fit the current desktop/workbench module layout rather than the older monolithic owner-file era structure):

```rust
fn semantic_tab_groups_for_frame(semantics: &FrameTabSemantics) -> &[TabGroupMetadata];
fn saved_tab_nodes_for_frame(semantics: Option<&FrameTabSemantics>, tree: &Tree<TileKind>) -> Vec<NodeKey>;
fn pane_semantic_tab_state(pane_id: PaneId, semantics: Option<&FrameTabSemantics>) -> PaneSemanticState;
```

---

## Restore / Collapse Semantics

### Collapse to pane rest state: semantic tab/group -> pane rest state

- Persist/refresh overlay metadata before structural change.
- Allow tabs container to be simplified/unhoisted into pane rest state.
- Preserve content visibility (last paint / thumbnail fallback when runtime content suspends).
- Keep pane fully usable while tab chrome is hidden.

### Restore tabs: pane rest state -> semantic tab/group

- Re-wrap pane into tabs container (egui_tiles structural operation).
- Reload/reapply overlay metadata: group membership, order, active pane.
- Restore normal tab interactions and affordances.

Ordering invariant (must not be violated):

1. Semantic decision — `GraphIntent` applied by reducer
2. Workbench tree mutation — desktop layer applies structural rewrap
3. Lifecycle/runtime dispatch — resume/suspend webviews as needed
4. UI readiness update — on runtime confirmation

Both restore and collapse operations must be idempotent. Pane rest state is a valid intermediate state while
runtime content is suspended or resuming.

### Intent Variants

```rust
GraphIntent::RestorePaneToSemanticTabGroup {
    pane_id: PaneId,
    group_id: TabGroupId,   // existing group to rejoin, or new
}
GraphIntent::CollapseSemanticTabGroupToPaneRest {
    group_id: TabGroupId,
}
GraphIntent::RepairFrameTabSemantics {
  frame_name: String,
}
```

These intent variants do not exist in runtime code today; they are the proposed reducer-owned
carrier for this feature.

The desktop apply layer handles tree rewrites, lifecycle dispatch, and UI state updates in response
to these intents.

---

## Single-Pane On-Demand Affordance

When a semantic tab group has been collapsed to pane rest state, expose a small inverted-tab control:

- Anchored near the pane viewport margin (upper-left, configurable via `WorkbenchSurfaceRegistry`
  interaction policy).
- Hidden by default; revealed when cursor approaches the tab region.
- Click dispatches `RestorePaneToSemanticTabGroup` intent.
- Keyboard-accessible equivalent command must exist (not mouse-only); registered in `InputRegistry`.

---

## Validation / Repair Policy

### Invariants

- Every `pane_id` in overlay metadata exists in the frame manifest.
- No pane belongs to more than one semantic tab group at once.
- `active_pane_id` is either `None` or a member of the same group.
- Group `pane_ids` ordering is deterministic and free of duplicates.

### Repair behavior

- Auto-repair invalid metadata when safe.
- Preserve visible panes even if some semantic metadata must be dropped or corrected.
- Reapply remaining valid semantics.
- Repair must not depend on runtime webview availability.
- Dispatch `RepairFrameTabSemantics` intent; reducer applies repair; desktop layer updates UI.

### Warning policy

- Aggregate all repairs per frame restore/load operation.
- Emit detailed repair events to debug logs.
- Emit at most one user-facing warning per restore/load by default.
- Warning text must include: frame name, affected group/pane IDs, exact repair action, what was
  preserved, what changed.

Example: `Frame 'research-1': repaired tab group g42 (missing active pane p9). Preserved panes [p3,p7]; active tab reset to p3.`

---

## Stage 8 Execution Plan

Stage 8A (design lock) is complete as documentation only. Stages 8B–8E remain implementation work,
and no runtime slice beyond design lock has landed.

### Stage 8B: Overlay Persistence + Validation

**Status**: Not started

Goal: persist optional tab semantics metadata in the frame bundle and validate/repair it on load.

- Extend bundle schema with optional `FrameTabSemantics` field (rkyv, additive — bundle load
  works with or without overlay present).
- Implement validation helpers: check each invariant in order, collect all violations before repairing.
- Implement repair helpers: drop/correct invalid entries; preserve valid entries; return repair log.
- Implement `RepairFrameTabSemantics` intent handling in reducer.
- Add roundtrip tests: bundle with overlay, bundle without overlay, bundle with invalid overlay.
- Add repair invariant tests: duplicate pane, invalid active pane, missing pane.

Done gate: bundle load/save roundtrip passes with and without overlay. Invalid overlay repairs are
deterministic and test-covered. Repair emits structured log entries.

### Stage 8C: Overlay-First Query APIs + Consumer Migration

**Status**: Not started — blocked on Stage 8B done gate

Goal: route all tab-aware features through shared semantic queries; eliminate direct tree-shape
inference from consumers.

- Add overlay-first helper APIs in a current desktop/workbench semantic-query module.
- Update omnibar saved-tab discovery to use `saved_tab_nodes_for_frame()`.
- Update pin UI tab-aware queries to use `pane_semantic_tab_state()`.
- Preserve tree-shape fallback in the helpers during rollout; do not require overlay presence.
- No new direct tree-shape inference paths in migrated consumers.

Done gate: tab-aware feature behavior is stable under tree-shape normalization. `cargo grep` for
direct `Tree<TileKind>` access outside `desktop/workbench_semantics.rs` returns no new callsites
for tab-semantic queries.

### Stage 8D: Restore / Collapse + Pane Rest State

**Status**: Not started — blocked on Stage 8B done gate

Goal: implement semantic lifecycle transitions and on-demand rewrap affordance.

- Add `RestorePaneToSemanticTabGroup`, `CollapseSemanticTabGroupToPaneRest` intent variants and
  reducer handling.
- Desktop apply layer: implement tree rewrap (hoist/unhoist), runtime lifecycle dispatch
  (resume/suspend), UI readiness update on confirmation.
- Implement idempotency: applying restore/collapse twice has same effect as once.
- Implement single-pane inverted-tab affordance in `desktop/ui/toolbar/` or pane chrome render
  (exact location TBD by render pass structure). Register keyboard equivalent in `InputRegistry`.
- Connect lifecycle ordering: restore waits for runtime confirmation before updating UI readiness.

Done gate: collapsed semantic tabs can be restored with metadata fully restored. Pane rest state
is usable while runtime content is suspended or resuming. Restore/collapse operations are idempotent under
repeated application.

### Stage 8E: Simplify-Safe Restore Integration

**Status**: Not started — blocked on Stage 8D done gate

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
- Restore/collapse idempotency tests
- Simplify/reapply equivalence tests: semantic invariants preserved across simplify
- Warning aggregation tests: at most one user-facing warning per restore operation
- Lifecycle ordering tests: restore while runtime is suspended or resuming

---

## Risks and Mitigations

Overlay becoming a second layout engine: keep overlay semantic-only (groups/order/active), not a
structural replacement. Clear ownership split with `FrameLayout`.

Feature code bypassing helpers: shared overlay-first query APIs are the only permitted path for
tab-aware queries. Migrate high-risk consumers first (omnibar, pin UI).

Repair warnings creating user confusion: exact aggregated messages with content-preservation-first
repair policy. Debug logs for detail.

Async races during restore/collapse: explicit intent ordering, idempotent operations, pane rest state
as valid intermediate state.

`WorkbenchSurfaceRegistry` diverging from overlay assumptions: simplification options live in the
registry; overlay restore/collapse logic reads those options. If simplify policy changes, the
simplify-reapply pipeline (Stage 8E) must be re-validated.

---

## Frame Routing Polish Addendum (Absorbed 2026-02-24)

This section absorbs and replaces the 2026-02-24 frame-routing polish plan.

### Resolver explainability

- Emit structured resolver traces from `resolve_frame_open`: candidates, recency scores, selected frame, and decision reason (`MostRecent`, `ExplicitTarget`, `Fallback`).
- Surface traces through diagnostics/logging for headed validation and bug triage.
- Optional preference strategy remains constrained to resolver policy selection, not alternate UI-side routing logic.

### Membership-aware UI affordances

- Badge tooltips should list memberships in recency order and highlight current frame.
- Frame-target actions in command palette should include membership hints.
- Hide badge noise for nodes that belong only to the current frame; keep stronger visual treatment for external membership.

### Batch operation intent boundary

- Route prune/retention operations through intent/request paths (`GraphIntent::PruneEmptyFrames`, `GraphIntent::RetentionSweep { max_snapshots }`) rather than direct maintenance shortcuts.
- Keep persistence effects in desktop helper layers, but preserve intent-layer observability.

### Validation additions

- [ ] Multi-home open emits resolver trace with ranking and decision reason.
- [ ] Membership badges follow local-only vs external-membership visibility rules.
- [ ] Command palette frame-target entries expose membership hints.
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
- `GraphWorkspace`/`AppServices` split (Phase 6) noted as future home of `FrameTabSemantics`.
- Persistence path made explicit: rkyv/redb frame bundle, not fjall WAL.
- `GraphIntent` variants made concrete with field signatures.
- Stage 8A declared complete (design document is the output); stages 8B–8E are now the
  implementation work items with explicit done gates.
- Source-of-truth split table added.
- `WorkbenchSurfaceRegistry` risk added to Risks section.
- Helper API surface and module location (`desktop/workbench_semantics.rs`) made concrete.

### 2026-04-01 (reconciliation revision)

- Retitled from the older overlay-and-promotion wording to keep `promotion` reserved for graph enrollment only.
- Status corrected: `FrameTabSemantics` remains planned architecture and is not yet implemented in runtime code.
- Canonical authority chain updated to point at `../graph/multi_view_pane_spec.md` for the semantic contract.
- Stale ownership wording replaced with current reducer/app-layer and desktop/workbench apply-layer language.
- Current open coupling called out explicitly: omnibar saved-tab discovery still reads semantic tab membership from tile-tree shape.
