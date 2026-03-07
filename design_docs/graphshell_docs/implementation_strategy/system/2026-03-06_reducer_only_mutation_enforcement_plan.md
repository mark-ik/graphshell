# Reducer-Owned Durable Graph Mutation Plan

**Date**: 2026-03-06
**Status**: Active migration slice
**Purpose**: Define a coherent migration from the current trusted-writer model to compiler-enforced reducer ownership of durable graph mutation for live runtime and replay/recovery.

**Related**:
- `system_architecture_spec.md`
- `register_layer_spec.md`
- `../../subsystem_storage/storage_and_persistence_integrity_spec.md`
- `../../subsystem_history/edge_traversal_spec.md`
- `../../../TERMINOLOGY.md`

---

## 1. Decision Summary

Graphshell should move to reducer-owned durable graph mutation.

This plan intentionally does **not** claim that every in-memory change touching `Graph` must immediately flow through the reducer. Current render/layout code still performs transient position and projection updates during interaction and physics ticks. Forcing that whole path through the reducer now would blur durable state, projection state, and per-frame simulation.

The target of this plan is narrower and stronger:

- durable graph mutation is planned by reducer logic
- durable graph mutation is applied through one canonical graph-apply path
- replay/recovery uses the same canonical graph-apply path
- non-reducer runtime modules cannot directly invoke durable graph mutation entry points
- side effects are separated from pure durable graph state application

This is the right next architecture step because it improves determinism, replay fidelity, and auditability without overfitting the reducer to transient render behavior.

---

## 2. Scope and Non-Goals

### 2.1 In scope

This plan covers reducer ownership of **durable graph mutation**, including:

- node add/remove
- edge add/remove
- traversal append and history edge semantics
- durable node metadata updates that belong to persisted graph state
- durable edge metadata updates that belong to persisted graph state
- replay/recovery application of persisted graph deltas

### 2.2 Explicitly out of scope for this slice

This plan does not require immediate reducer ownership of:

- per-frame render synchronization
- physics/layout projection updates
- purely view-local workspace state
- ephemeral UI selection/focus affordances unless they are already reducer-owned for other reasons

### 2.3 Deferred decision

Node position currently straddles two models:

- durable graph state, because it is persisted
- transient projection state, because render/physics mutates it continuously

This plan originally preserved the hybrid reality while reducing other mutation ambiguity first. The first split is now implemented: nodes carry a durable committed position for snapshot/replay truth and a transient projected position for render/physics churn. The remaining work is to continue shrinking the places that read or write the projected lane directly.

---

## 3. Problem Framing

Graphshell documents a single-write-path goal, but the current implementation still exposes multiple mutation paths:

- reducer-driven graph mutation in `GraphBrowserApp`
- persistence replay/recovery graph mutation in `services/persistence`
- public higher-level helper methods that mutate graph state outside reducer-intent entry points
- crate-visible mutable graph accessors that still permit direct graph-state mutation

That current state is workable as an interim trusted-writer architecture, but it is not reducer-enforced ownership.

The central design correction is:

- stop defining the target as "all graph mutation everywhere"
- define the target as "all durable graph mutation uses one canonical reducer-owned apply path"

---

## 4. Architectural Principles

### 4.1 Durable mutation is the enforcement boundary

Compiler-enforced ownership applies to durable graph mutation, not every transient projection write.

### 4.2 Planning, apply, and effects are separate concerns

Reducer logic may decide what should happen, but pure graph state application must be isolated from:

- diagnostics emission
- persistence logging
- webview/runtime orchestration
- UI notifications
- async work dispatch

### 4.3 Replay uses the same graph-apply engine

Replay/recovery must not reconstruct accepted state by calling ad hoc graph mutators directly. It must decode persisted deltas into the same canonical apply engine used by live reducer execution.

### 4.4 Sanctioned construction paths are explicit

Hydration and construction are allowed, but they must be clearly named and narrowly scoped. They are not general runtime mutation escape hatches.

### 4.5 Public runtime escape hatches must close

It is not enough to restrict raw `Graph` mutators if runtime code can still call higher-level mutating helpers on `GraphBrowserApp`.

---

## 5. Target Model

### 5.1 Reducer result shape

Do not model the reducer as `GraphIntent -> Vec<GraphMutation>` for every intent. The reducer already processes graph intents, workspace-only intents, and mixed intents.

Use a result shape closer to:

1. `graph_deltas`: pure durable graph mutations
2. `workspace_updates`: reducer-owned non-graph state changes
3. `effects`: side effects to execute after state apply

The exact type names are flexible. The important design point is that graph-delta planning is explicit without pretending every reducer intent is only a graph mutation.

### 5.2 Canonical graph delta layer

Introduce a pure delta type, for example `GraphDelta`, representing durable graph-state changes only.

Rules:

- no side effects in `GraphDelta`
- replayable and deterministic
- expressive enough for current reducer-owned durable graph writes
- expressive enough for persisted replay/recovery

### 5.3 Canonical graph apply layer

Introduce a dedicated graph apply module, for example `model::graph::apply`, responsible for:

- applying `GraphDelta` to `Graph`
- owning the restricted access path to raw graph mutators
- serving both live reducer execution and replay/recovery

This avoids forcing module-private graph mutators to be callable directly from `graph_app.rs`.

### 5.4 Reducer-owned undo boundary model

Undo capture in the current codebase is not a pure graph concern. The snapshot already spans:

- graph state
- selection state
- highlighted edge state
- serialized workspace/frame layout

That means reducer-owned mutation enforcement should not treat undo capture as a raw graph helper to be exposed to UI/runtime code indefinitely.

The target model is:

- reducer/app layer owns undo-boundary policy
- UI/workbench code may supply `layout_before` when it is the only layer that can serialize the live tile tree before mutation
- raw checkpoint insertion is not a public runtime escape hatch

Recommended shape:

1. `apply_reducer_intents_with_context(intents, ctx)`
2. `record_workspace_undo_boundary(layout_before, reason)` for layout-only operations that do not naturally map to graph intents

Where `ctx` carries:

- `workspace_layout_before: Option<String>`
- `force_undo_boundary: bool`
- `reason: UndoBoundaryReason`

And `UndoBoundaryReason` is an explicit audit label such as:

- `ReducerIntents`
- `OpenNodePane`
- `RestoreFrameSnapshot`
- `DetachNodeToSplit`

This keeps undo-boundary ownership in the reducer/app layer without pretending every workspace-layout mutation is a graph intent.

---

## 6. Sanctioned Exceptions

The following remain valid with explicit naming and narrow scope:

### 6.1 Construction and hydration

- `Graph::new`
- snapshot/materialized graph construction
- test-only builders

These are constructors, not runtime mutation APIs.

### 6.2 Full-state replacement

Whole-graph replacement during load/undo/redo/recovery may remain a sanctioned path temporarily, but it must be documented as state restoration, not as a general-purpose runtime mutation mechanism.

### 6.3 Transitional render/layout behavior

Direct transient position updates during render/physics remain temporarily valid until position state is split or otherwise normalized.

This exception must be treated as temporary technical debt, not as proof that reducer ownership is unnecessary.

---

## 7. Canonical Migration Strategy

### Stage 0 - Narrow and document the enforcement target

Before code changes:

- rename the architecture target to reducer-owned durable graph mutation
- document explicit non-goals
- document sanctioned construction/hydration exceptions
- document transient render/layout exception status

### Stage A - Introduce `GraphDelta` and graph-apply module

Implementation status as of 2026-03-06:

- Implemented: `model::graph::apply` exists and is used for add/remove node, add/remove edge, traversal append, replay add/remove operations, and durable node metadata writes (`title`, `url`, `thumbnail`, `favicon`, `mime_hint`, `address_kind`, `is_pinned`).
- Remaining: durable edge metadata beyond traversal append still needs the same normalization if/when introduced.

Create a pure graph delta type plus a dedicated graph apply module.

Rules:

- graph apply is the canonical durable graph mutation engine
- raw graph mutators remain hidden behind graph apply
- side effects remain outside graph apply

### Stage B - Split reducer planning from graph apply

Implementation status as of 2026-03-06:

- Partially implemented: reducer-owned undo-boundary dispatch now exists via `apply_reducer_intents_with_context(...)`, `record_workspace_undo_boundary(...)`, and `UndoBoundaryReason`.
- Implemented: startup/UI callers were moved away from direct public undo checkpoint helpers and direct undo/redo helper calls.
- Remaining: the reducer still mixes graph apply, workspace mutation, and effect execution inside `GraphBrowserApp`; a first-class reducer outcome type has not been extracted yet.

Reducer flow becomes:

1. reducer intent planning
2. graph delta apply
3. workspace update apply
4. effect execution

Rules:

- graph deltas are planned, then applied
- workspace-only intents do not need fake graph deltas
- effect emission is post-apply and separate

### Stage C - Route replay/recovery through graph apply

Persistence replay decodes persisted entries into canonical graph deltas and invokes the same graph apply path used by live reducer execution.

Rules:

- replay does not call raw graph mutators directly
- replay may suppress or adapt effects
- replay and live execution share the same durable graph mutation semantics

### Stage D - Close higher-level runtime escape hatches

Implementation status as of 2026-03-06:

- Partially implemented: non-reducer runtime callers have been moved off direct `add_node_and_sync` / `add_edge_and_sync` and public undo helpers in the main UI/workbench paths.
- Partially implemented: the trusted-writer contract test now also guards persistence runtime code against raw durable mutation escape hatches such as direct `get_node_mut`, `get_edge_mut`, and `update_node_url` access.
- Remaining: `GraphBrowserApp` still exposes crate-visible helpers used by the reducer itself and some transient/runtime mutation paths are intentionally not yet normalized.

After graph apply exists:

- migrate runtime callers away from direct `add_node_and_sync` / `add_edge_and_sync` style helpers
- replace direct public undo checkpoint helper usage with reducer-owned undo-boundary APIs
- route startup graph creation through reducer intents or reducer-owned bootstrapping wrappers
- route UI undo/redo through reducer-intent entry points rather than direct helper calls
- reduce public mutating `GraphBrowserApp` surface area

### Stage E - Tighten raw graph visibility

Implementation status as of 2026-03-06:

- Started: `get_edge_mut` is no longer needed by non-test runtime code after replay moved onto graph-apply for dissolved traversal recovery and pin-state replay.
- Updated: `get_node_mut` is now test-only. Runtime transient/session/lifecycle writes use dedicated graph setters, and projected node movement is beginning to move onto an explicit projected-position lane.

Restrict direct graph mutation APIs so non-graph-apply modules cannot compile if they attempt durable graph mutation.

Preferred order:

1. dedicated apply module ownership
2. tighter visibility for raw graph mutators and mutable graph accessors
3. capability token only if visibility alone is still too weak
4. crate split only if later proven necessary

### Stage F - Normalize remaining mutable graph accessors

Audit and restrict crate-visible mutable accessors such as:

- mutable node access
- mutable edge payload access
- replay-only helper mutators

The reducer-owned boundary is incomplete if raw topology calls are blocked but arbitrary durable metadata mutation remains available elsewhere.

### Stage G - Revisit position/projection ownership

Once durable graph apply is stable for non-transient mutation, decide whether to:

- split durable committed position from transient projected position
- keep position reducer-owned only at commit points
- or keep a documented exception if that model remains clearly bounded

This stage is now in progress. The first slices keep snapshot/replay truth on a durable committed position, route render/physics mutation through a projected-position lane, move runtime placement reads onto explicit projected-position helpers, and hide the raw position fields behind named node accessors.

### Stage H - Remove trusted-writer wording

After live reducer execution and replay both use graph apply, and non-reducer runtime escape hatches are closed:

- remove trusted-writer exception language
- update architecture docs to reducer-owned durable graph mutation language
- retain enforcement tests

---

## 8. Side-Effect Isolation Contract

Reducer-owned durable graph mutation requires a strict split:

- graph apply: pure, deterministic, replayable durable state mutation
- workspace apply: reducer-owned non-graph state updates
- effects: diagnostics, orchestration, UI notifications, async work

Undo-boundary capture belongs with reducer/app transaction coordination, not inside pure graph apply.
It is allowed to depend on a caller-provided `workspace_layout_before` snapshot because the live tile tree is currently owned outside the reducer.

Replay mode behavior:

- apply graph deltas
- apply any required deterministic workspace restoration
- suppress or adapt live-only effects
- preserve deterministic state reconstruction

---

## 9. Enforcement and Testing

### 9.1 Compile boundary goal

Non-reducer runtime modules should fail to compile if they attempt to invoke durable graph mutation entry points directly.

### 9.2 Contract test expansion

Existing boundary tests should remain, but they must expand beyond raw topology calls. Guard against:

- direct raw graph mutator calls
- direct mutable graph accessor use for durable metadata writes
- direct high-level app mutation helper calls from non-reducer runtime code
- direct undo/redo helper calls from runtime/UI layers after migration
- direct raw checkpoint insertion from runtime/UI layers after reducer-owned undo-boundary APIs exist

### 9.3 Parity tests

Add parity tests proving:

- live reducer execution and replay produce equivalent durable graph state
- side effects are not embedded in graph apply
- replay suppresses or adapts live-only effects correctly
- sanctioned hydration/restoration paths do not regress graph invariants

---

## 10. Acceptance Criteria

1. Durable graph mutation has one canonical apply path shared by live reducer execution and replay/recovery.
2. Non-reducer runtime modules cannot directly invoke durable graph mutation entry points.
3. Raw mutable graph access used for durable metadata mutation is restricted to sanctioned apply/hydration paths.
4. Public higher-level runtime mutation escape hatches are removed, reduced, or explicitly reducer-owned.
5. Side effects are not embedded in pure graph apply.
6. Replay of persisted logs produces durable graph state equivalent to live-constructed state for the same delta sequence.
7. Undo-boundary capture is owned by reducer/app transaction APIs rather than public raw checkpoint helpers.
8. Transitional exceptions are documented explicitly and do not silently expand.

---

## 11. Recommended Implementation Choice

Implement this plan with:

- a dedicated graph-apply module
- a pure `GraphDelta` layer
- a reducer outcome that separates graph deltas, workspace updates, and effects
- reducer-owned undo-boundary APIs that accept caller-supplied `layout_before` when needed
- tighter visibility on raw graph mutators and mutable accessors
- migration of startup/undo/runtime callers away from direct mutating helpers

Do **not** start with a crate split.

Do **not** require immediate reducer ownership of transient render/physics position writes.

Do **not** claim reducer-only enforcement for all graph-adjacent state until the current position/projection ambiguity is resolved.

Do **not** force pure workspace-layout operations into fake graph intents just to capture undo. Use explicit reducer/app transaction APIs for those cases.

---

## 12. Interim Policy

Until Stages D through H complete, the repo remains in a temporary trusted-writer state with explicit constraints:

- reducer-owned runtime flow is the primary durable graph writer
- persistence replay/recovery is a sanctioned reconstruction path until migrated to graph apply
- sanctioned hydration/restoration paths remain allowed
- transient render/layout mutation remains a documented exception
- non-reducer runtime code must continue shrinking its direct mutation surface rather than expanding it

This interim state must be described as transitional architecture, not as the final boundary model.
