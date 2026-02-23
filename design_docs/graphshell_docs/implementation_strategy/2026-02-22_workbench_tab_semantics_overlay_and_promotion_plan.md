<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workbench Tab Semantics Overlay and Promotion Plan

**Date**: 2026-02-22
**Status**: Draft (design/prep before implementation)
**Relates to**:
- `2026-02-22_workbench_workspace_manifest_persistence_plan.md` (completed manifest migration foundation)
- `2026-02-19_workspace_routing_and_membership_plan.md`

---

## Purpose

Add a thin tab-semantics overlay subsystem on top of the workbench (`egui_tiles`) so Graphshell
can preserve tab-specific behavior through structural normalization (including `simplify()`),
while supporting pane rest states and on-demand promotion/demotion back into tab containers.

This plan intentionally treats tab semantics as a follow-on architecture layer built on the
manifest-based workspace persistence model.

## Semantic Model

- **Workbench**: The container (IDE Window).
- **Graph**: The navigation engine (File Tree).
- **Tabs**: Open documents/nodes.

---

## Design Principles

- Graphshell semantics must survive dependency-initiated structural normalization.
- The `egui_tiles` workbench tree is structural state, not semantic truth.
- Semantic metadata is optional and additive for tabs and panes.
- Restore prioritizes content availability first, semantic exactness second, exact structural shape third.
- Promotion/demotion between pane and tab-container forms is first-class behavior, not an exception path.
- Graphshell should remain compatible with dependency-native transforms (including `simplify()`) by
  persisting app semantics explicitly instead of relying on incidental tree shape.

---

## Scope

In scope:

- tab semantics overlay metadata (tab groups / ordering / active pane)
- promotion/demotion semantics (tab/group <-> pane rest state)
- simplify-safe restore and reapplication of tab semantics
- overlay-first semantic query APIs for tab-aware features
- repair/validation policy and user-facing warning aggregation
- single-pane on-demand rewrap affordance ("inverted tab" control)

Out of scope:

- workbench/workspace manifest core migration (already completed)
- changing `egui_tiles` internals/forking the dependency
- graph edge traversal/history semantics (integration only, no redesign)

---

## Terminology

- **Visual tabs**: current `egui_tiles::Container::Tabs` shape in the live workbench tree
- **Semantic tabs**: Graphshell tab/group meaning persisted in overlay metadata
- **Pane rest state**: a valid pane-only representation of a semantic tab/group after demotion/unhoist/simplify
- **Hoist / Unhoist**: structural workbench tree transforms only
- **Promote / Demote**: semantic + structural lifecycle actions (may trigger hoist/unhoist)

Resolved meaning:

- A **semantic tab** is any tab/tab-group with saved tab metadata.
- A single pane with semantic metadata counts for all tab-aware features even when currently unhoisted.
- Graph panes and webview panes are both eligible for semantic tab/group membership.
- A pane belongs to at most one semantic tab group at a time (historical reassignment is allowed).

---

## Problem Statement

Current tab-aware behavior in Graphshell often infers semantics from workbench tree shape. This is
fragile because dependency-native structural transforms (such as `egui_tiles::simplify()`) can
collapse single-tab containers into panes and erase tab-specific shape.

Consequences without an overlay:

- saved-tab discovery and tab-aware queries can regress when shape changes
- tab-specific UI affordances become tightly coupled to `egui_tiles` containers
- Graphshell must choose between preserving behavior and allowing dependency normalization
- repair/restore logic remains implicit and hard to validate

---

## Target Model (Overlay with a Little Guts)

### Core idea

Keep `egui_tiles` as the structural workbench engine and add a thin semantic overlay that:

- stores tab/group semantics using stable IDs
- survives simplify/unhoist operations
- supports promote/demote lifecycle actions
- re-applies semantics when needed

This is not a fork of `egui_tiles`; it is a compatible semantic subsystem layered on top.

### Overlay metadata (minimum planned shape, not final schema)

```rust
pub type TabGroupId = u64; // or Uuid

pub struct WorkspaceTabSemantics {
    pub version: u32,
    pub tab_groups: Vec<TabGroupMetadata>,
}

pub struct TabGroupMetadata {
    pub group_id: TabGroupId,
    pub pane_ids: Vec<PaneId>,          // ordered tab membership
    pub active_pane_id: Option<PaneId>, // must be member or repaired
}
```

Notes:

- `PaneId` comes from the manifest-backed workspace bundle model.
- Container IDs are not required for v1; add them later only if split/container-local semantics become first-class.
- Metadata remains optional: absence falls back to shape inference and pane-only behavior.

### Optional future additions (schema-friendly)

- group-level UI metadata (labels, color, collapsed/rest preference)
- pane-level tab metadata (title overrides, pin state)
- timestamps (`last_promoted_at`, `last_demoted_at`)
- diagnostics (`last_repair_at`, `last_repair_reason`)

---

## Semantics vs Shape Contract

### Source-of-truth split

- `WorkspaceLayout` (manifest plan): structural workbench tree
- `WorkspaceManifest` (manifest plan): pane identity + membership
- `WorkspaceTabSemantics` (this plan): tab semantics only

### Overlay query precedence (required)

For tab-aware behavior (omnibar saved tabs, pin UI, tab affordances, future tab queries):

1. semantic tab overlay metadata
2. live workbench tree shape inference
3. pane-only fallback behavior

This precedence must be implemented through shared helper APIs, not per-feature ad hoc logic.

---

## Promotion / Demotion Semantics

### Demote (semantic tab/group -> pane rest state)

Demotion is a semantic lifecycle transition that may include structural simplification/unhoisting.

Behavior:

- persist/refresh tab semantics metadata
- allow tabs container to be simplified/unhoisted into pane rest state
- preserve content visibility (including last paint / thumbnail-like fallback when runtime content suspends)
- keep pane fully usable even when tab chrome is hidden

### Promote (pane rest state -> semantic tab/group)

Promotion restores tab/group semantics and tab chrome on demand.

Behavior:

- re-wrap pane into tabs container
- reload/reapply tab semantics metadata (group membership, order, active pane)
- restore normal tab interactions and affordances

### Single-pane on-demand affordance (planned UX)

It is acceptable for a tab/group to rest as a pane until tab-specific handling is needed.

Planned affordance:

- a small inverted-tab control anchored near the pane viewport margin (e.g., upper-left)
- hidden by default and revealed when cursor approaches the tab region
- click promotes/rewraps the pane and restores tab metadata/chrome
- keyboard-accessible equivalent command must exist (not mouse-only)

---

## Validation / Repair / Warning Policy

### Validation invariants (minimum)

- every `pane_id` referenced by tab metadata exists in the workspace manifest
- no pane belongs to more than one semantic tab group at once
- `active_pane_id` is either `None` or a member of the same group
- group order is deterministic and free of duplicates

### Repair behavior (release)

Repair should preserve user-visible content and usability first.

Rules:

- auto-repair invalid metadata when safe
- preserve visible panes even if some semantic metadata must be dropped/corrected
- reapply remaining valid semantics
- do not require synchronous runtime/webview availability to repair metadata

### User-facing warning policy

Repair warnings must be exact and not noisy.

Rules:

- aggregate repairs per workspace restore/load operation
- emit detailed repair events to debug logs
- emit at most one user-facing warning per restore/load by default
- warning text must include:
  - workspace name
  - affected group/pane IDs
  - exact repair action
  - what was preserved
  - what changed behaviorally/visually

Example (illustrative):

- `Workspace 'research-1': repaired tab group g42 (missing active pane p9). Preserved panes [p3,p7]; active tab reset to p3.`

---

## Integration Constraints (Carry-Forward Guardrails)

### 1. Intent boundary discipline

- `render/*` captures UI events and renders state only; it must not directly mutate tab overlay semantics.
- `app.rs` remains the authority for semantic decisions (promotion/demotion intents, repair policy, warning intent creation).
- `desktop/*` applies workbench tree mutations and runtime effects (rewrap, restore, lifecycle coordination) in response to app-level intents.
- Promotion/demotion must be explicit operations/intents, not ad hoc tree rewrites scattered across UI callsites.

### 2. Avoid duplicate sources of truth

- Use shared overlay-first semantic query helpers.
- Do not mix direct tree inspection and overlay reads outside the defined fallback sequence.
- Keep layout, manifest, and overlay semantics responsibilities separate.

### 3. Warning aggregation (avoid UX noise)

- Aggregate repair events per workspace restore/load.
- Detailed logs are fine; user-facing warnings should be summarized and exact.

### 4. Async / lifecycle ordering (avoid races)

Required ordering model:

1. semantic decision (app intent)
2. workbench tree mutation (desktop apply)
3. lifecycle/runtime request dispatch (resume/suspend as needed)
4. readiness/UI state update on runtime confirmation

Rules:

- promotion/demotion must be idempotent
- pane rest states are valid while runtime content is suspended/resuming
- semantic repair must not depend on runtime availability

### 5. Dependency compatibility

- Preserve compatibility with dependency-native structural transforms (`egui_tiles::simplify()`).
- Restore/reapply Graphshell semantics through overlay metadata + promotion logic, not by globally disabling dependency behavior.

---

## API / Intent Design Prep (Pre-Implementation)

### Semantic query helpers (overlay-first)

Proposed helper surface (exact module TBD):

```rust
fn workspace_tab_semantics_for_name(...) -> Option<WorkspaceTabSemantics>;
fn semantic_tab_groups_for_workspace(...) -> Vec<TabGroupMetadata>;
fn saved_tab_nodes_for_workspace(...) -> Vec<NodeKey>; // overlay first, shape fallback
fn pane_semantic_tab_state(...) -> PaneSemanticState;
```

Goals:

- centralize precedence logic
- prevent feature-specific shape inference duplication
- make Stage 8 migrations incremental and testable

### Promotion / demotion operations (semantic lifecycle)

Proposed operation/intents (names illustrative):

```rust
GraphIntent::PromotePaneToSemanticTabGroup { ... }
GraphIntent::DemoteSemanticTabGroupToPaneRest { ... }
GraphIntent::RepairWorkspaceTabSemantics { workspace_name: String }
```

Desktop-layer apply operations should handle:

- workbench tree rewrites (rewrap/hoist/unhoist)
- runtime lifecycle dispatch (resume/suspend)
- UI readiness updates

---

## Save / Restore Transform Walkthroughs (Design Validation)

These should be documented and tested before full implementation:

1. Single semantic tab -> simplified pane -> promote back
- preserve pane content and semantic tab identity
- active pane restored on promote

2. Multi-tab group with active tab
- preserve tab order and active tab across roundtrip

3. Graph pane in semantic tab group
- confirm pane-type-agnostic overlay behavior

4. Missing pane in metadata
- repair deterministically
- preserve remaining panes
- emit exact aggregated warning

5. Overlay absent / partial
- fallback to shape inference, then pane-only behavior

---

## Stage 8 Execution Plan

### Stage 8A: Design Lock + Schema Draft

Goal:

- finalize overlay schema, terminology, invariants, and warning policy before coding

Tasks:

- finalize `WorkspaceTabSemantics` + `TabGroupMetadata` schema
- finalize validation/repair invariants
- finalize warning aggregation/message format
- finalize helper/API/intent names

Acceptance:

- schema and behaviors are specific enough to implement without semantic ambiguity

### Stage 8B: Overlay Persistence + Validation

Goal:

- persist optional tab semantics metadata in the workspace bundle and validate/repair it

Tasks:

- extend bundle schema with optional tab semantics overlay
- implement validation/repair helpers
- add roundtrip and invalid-metadata repair tests

Acceptance:

- bundle load/save works with and without overlay metadata
- invalid overlay repairs are deterministic and test-covered

### Stage 8C: Overlay-First Query APIs + Consumer Migration

Goal:

- route tab-aware features to shared semantic queries

Tasks:

- add overlay-first helper APIs
- update omnibar saved-tab discovery
- update pin UI tab-aware comparisons/queries (if applicable)
- preserve tree-shape fallback during rollout

Acceptance:

- tab-aware feature behavior is stable under tree-shape normalization
- no new direct tree-shape inference paths introduced in migrated consumers

### Stage 8D: Promotion / Demotion Semantics + Pane Rest State

Goal:

- implement semantic lifecycle transitions and on-demand rewrap

Tasks:

- add promotion/demotion intents/operations
- implement pane rest state handling
- implement single-pane inverted-tab affordance + keyboard equivalent
- connect lifecycle ordering (resume/suspend) with idempotent operations

Acceptance:

- demoted semantic tabs/groups can be promoted back with metadata restored
- pane rest state remains usable while runtime content is suspended/resuming

### Stage 8E: Simplify-Safe Restore Integration

Goal:

- allow dependency normalization while preserving Graphshell tab semantics

Tasks:

- define/implement structural simplify + semantic reapply pipeline
- add cross-transform roundtrip tests (`save -> restore -> simplify -> reapply -> save`)
- document compatibility guarantees

Acceptance:

- `egui_tiles::simplify()` no longer conflicts with Graphshell tab semantics
- no pane loss / tab metadata loss under supported transforms

---

## Test and Validation Plan (Pre-commit Checklist)

- roundtrip tests with/without overlay metadata
- repair invariants tests (duplicate pane, invalid active pane, missing pane)
- query precedence tests (overlay first, shape fallback)
- promotion/demotion idempotency tests
- simplify/reapply equivalence tests (semantic invariants preserved)
- warning aggregation tests (single user-facing warning per restore op)
- lifecycle ordering tests (promotion while runtime suspended/resuming)

---

## Risks and Mitigations

### Risk: Overlay becomes a second ad hoc layout engine

Mitigation:

- keep overlay semantic-only (groups/order/active), not structural layout replacement
- maintain clear ownership split with `WorkspaceLayout`

### Risk: Feature code bypasses helpers and reintroduces shape-coupling

Mitigation:

- shared overlay-first query APIs
- integration constraints enforced in code review
- migrate high-risk consumers first (omnibar, pin UI)

### Risk: Repair warnings create user confusion/noise

Mitigation:

- exact aggregated messages
- debug logs for detail
- content-preservation-first repair policy

### Risk: Async races during promote/demote

Mitigation:

- explicit intent ordering
- idempotent operations
- pane rest state as valid intermediate state

---

## Future: Verse Integration & Registry Layer

As Graphshell evolves toward the "Verse" vision (P2P sharing), the workspace persistence layer will integrate with the **Registry Ecosystem** (`2026-02-22_registry_layer_plan.md`) and **Verse Strategy** (`2026-02-22_verse_implementation_strategy.md`).

### Identity Registry (Signing)
Named workspace bundles will eventually include a cryptographic signature from the `IdentityRegistry`. This allows workspaces to be shared as "Tokenized Reports" with provenance.

### Protocol Registry (Storage Abstraction)
While currently coupled to `redb`/`fjall` on the local filesystem, the save/load operations will eventually route through the `ProtocolRegistry`. This enables saving a workspace directly to IPFS (`ipfs://...`) or a P2P peer, treating the storage backend as a pluggable transport.

---

## Suggested Follow-On Doc Updates

After implementation begins:

- update `2026-02-22_workbench_workspace_manifest_persistence_plan.md` to mark Stage 8 follow-on plan active/completed
- update `2026-02-22_multi_graph_pane_plan.md` to reference pane rest state / promotion semantics where relevant
- update tab-aware feature docs (omnibar/pin/workbench UX) to use overlay-first query terminology
