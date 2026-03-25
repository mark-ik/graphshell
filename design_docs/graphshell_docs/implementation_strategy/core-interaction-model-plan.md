<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Core Interaction Model Plan

**Date**: 2026-03-18
**Status**: Planning / Active
**Purpose**: Establish the shared interaction model that the graph, workbench,
and navigator must agree on — so that node behavior is understandable and
consistent across every surface.

**Related docs**:

- [PLANNING_REGISTER.md](PLANNING_REGISTER.md) — execution control-plane and Interaction Decisions Receipt (2026-03-16)
- [canvas/graph_backlog_pack.md](canvas/graph_backlog_pack.md) — graph backlog G01–G50
- [workbench/workbench_backlog_pack.md](workbench/workbench_backlog_pack.md) — workbench backlog WB01–WB25
- [workbench/navigator_backlog_pack.md](workbench/navigator_backlog_pack.md) — navigator backlog NV01–NV25
- [system/register/SYSTEM_REGISTER.md](system/register/SYSTEM_REGISTER.md) — routing decision rules and two-authority model
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical term definitions
- [../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md](../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md) — `graphshell-core` crate extraction plan

---

## Overview

This plan defines six areas of work required to bring the graph, workbench,
and navigator into agreement:

1. **Core Invariants** — the non-negotiable rules every surface must honour.
2. **Legacy Cleanup** — old presentation logic, stale carriers, and authority
   violations that must be cleared before new interaction work can land safely.
3. **Diagnostics Refinement** — making UI/UX problems observable and
   diagnosable through structured diagnostics.
4. **Cross-Surface Wiring** — threading / signal routing changes between
   subsystems so that state changes propagate correctly.
5. **Surface Agreement Checklist** — the specific cross-pack backlog items that
   must converge before the interaction model can be claimed as settled.
6. **Core Extraction Alignment** — ensuring the portable `graphshell-core`
   boundary stays clean as the interaction model solidifies.

---

## 1. Core Invariants

These are the foundational rules. Every new feature, surface, diagnostic, and
test must satisfy all seven. If a surface violates an invariant, it is a bug —
not a UI preference.

### INV-1 — Identity

One node, one identity, consistent across graph / workbench / navigator.

A node's `NodeKey`, lifecycle state, `Address`, content kind, and membership
are the same regardless of which surface displays it. The graph canvas, the
navigator row, the workbench tab, and the command palette all project the same
underlying node. If they disagree, one of them is wrong.

- Title, lifecycle, address, and tags are read from graph truth — never cached
  locally by a surface with independent freshness.
- Badge and status rendering derive from the same lifecycle state and tag set
  across all surfaces.
- Graph mutations flow exclusively through `apply_reducer_intents()`. No
  surface maintains a parallel truth store.

**Backlog anchors**: G03 (glossary), G05 (lifecycle), G11 (canonical node
shape), G19 (node lifecycle transitions), G21 (graph-view identity contract),
WB03 (workbench glossary), NV02 (navigator glossary).
**Plans**: [graphshell_core_extraction_plan §1](../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md)
(NodeKey / Address / Graph in portable core).

### INV-2 — State Separation

Existence, visibility, activation, and selection are four distinct states.
None implies another except by explicit intent.

| State | Meaning | Owner |
| --- | --- | --- |
| **Existence** | The node has a `NodeKey` in the graph and is not tombstoned | Graph truth |
| **Visibility** | The node is rendered on at least one surface | Workbench + canvas view |
| **Activation** | The node's tile/row holds focus | Workbench focus model |
| **Selection** | The node is in the selected target set | Graph selection model per `GraphViewId` |

- A node can exist without being visible. It can be visible without being
  active. It can be selected without being the focused tile.
- Hidden or non-present surfaces may retain return-target memory, but not live
  focus or live selection (Interaction Decisions Receipt §4).
- Ghost nodes (tombstoned) exist in the graph but are filtered from default
  queries; they are visible only when "Show Deleted" is enabled.

**Backlog anchors**: G19 (lifecycle transitions), G23 (dismiss/hide),
G30 (selection reveal), G31 (selection lifecycle), G34 (tombstone visibility),
WB21 (focus model contract).
**Plans**: [PLANNING_REGISTER §Interaction Decisions Receipt](PLANNING_REGISTER.md)
(§3, §4 — selection reveal and lifecycle rules).

### INV-3 — Selection Propagation

Selecting a node on any surface sets graph selection truth. Surfaces project
that truth — they do not own their own copy.

- There is one selection model per `GraphViewId`, authoritative for all
  surfaces rendering that view.
- When a surface selects a node, it writes to graph selection truth. All other
  surfaces observe the change via `SelectionChanged` signal.
- Navigator, canvas, and command surfaces never maintain independent selection
  state — they read and render.
- Mixed selection (nodes, edges, frames, tiles, arrangement objects) coexists
  in one selected target set (Interaction Decisions Receipt §5).

**Backlog anchors**: G28 (mixed selection model), G30 (selection reveal),
G31 (selection lifecycle), NV07 (navigator selection contract),
NV12 (navigator selection projection), WB22 (selection-to-workbench targeting).
**Plans**: [SYSTEM_REGISTER — routing decision rules](system/register/SYSTEM_REGISTER.md)
(SelectionChanged signal path).

### INV-4 — Click Grammar

Single-click = select / focus. Double-click = activate / open. This holds
across all three surfaces.

| Surface | Single-click | Double-click |
| --- | --- | --- |
| **Graph canvas — node** | Select node | Open / focus node in tile |
| **Graph canvas — edge** | Select edge | Open relevant family in History Manager |
| **Graph canvas — frame object** | Select frame arrangement | Expand / focus frame |
| **Navigator — node row** | Select node | Navigate (residency-aware) |
| **Navigator — frame/tile row** | Expand / collapse | (same) |
| **Workbench — tab** | Activate tab (focus) | — |

Surface-specific detail (navigator row-type clicks, edge clicks) may refine
the grammar, but deviations from the default must be explicitly documented —
silent fallback to an implicit primary target is forbidden
(Interaction Decisions Receipt §6).

- Pointer gesture ownership is exclusive once resolved — mid-gesture
  reclassification is forbidden (Input Interaction Spec §2.2).

**Backlog anchors**: NV06 (navigator click grammar lock), NV08
(residency-aware navigation), NV09 (structural row focus), G32 (edge selection
semantics).
**Plans**: [PLANNING_REGISTER §Interaction Decisions Receipt](PLANNING_REGISTER.md)
(§1 — row-type-specific navigator clicks, §2 — residency-aware navigation).
**Research**: Input Interaction Spec §2.2 (gesture ownership).

### INV-5 — Reveal

Reveal is conditional and side-effect-free. It does not change selection,
activation, or existence.

Reveal scrolls or pans a surface so that a target object becomes visible. It
fires only when the graph canvas is visible and the node is offscreen. It never
mutates selection truth, focus state, or graph existence. A reveal that cannot
be performed (surface hidden, node tombstoned) is silently skipped — it does
not fall back to selecting or activating the target.

**Backlog anchors**: G30 (selection reveal — conditional, only when graph is
visible and node is offscreen), NV11 (navigator selection reveal).
**Plans**: [PLANNING_REGISTER §Interaction Decisions Receipt](PLANNING_REGISTER.md)
(§3 — reveal-on-select rule).

### INV-6 — Dismiss / Delete

Dismiss removes from surface context. Delete removes from graph truth. Dismiss
is always recoverable. Delete is not.

| Action | Effect | Recovery | Authority |
| --- | --- | --- | --- |
| **Dismiss** | Removes node from current surface view (hides from canvas, collapses in navigator, closes tile) | Always recoverable — node still exists in graph; re-open, un-hide, or restore | Workbench authority or view-local filter |
| **Delete** | Tombstones node in graph truth; removes from all surfaces | Not recoverable (except via undo while WAL checkpoint still available) | Graph reducer only |

- No surface may conflate dismiss and delete in a single ambiguous action.
- "Close tab" is dismiss. "Delete node" is delete. The UI must never present
  one as the other.
- Dismissed nodes remain in recents, search, and navigator (collapsed or
  filtered) — they are not erased from the user's mental model.

**Backlog anchors**: G19 (node lifecycle transitions), G23 (dismiss/hide
contracts), G34 (tombstone visibility filtering).
**Plans**: [PLANNING_REGISTER §Interaction Decisions Receipt](PLANNING_REGISTER.md)
(§12 — node dismiss lifecycle).

### INV-7 — Command Applicability

A command is available only if it validly applies to the full selection set.
No silent narrowing to a subset.

- `ActionRegistry::list_actions_for_context(...)` evaluates every selected
  object. Commands that are invalid for any member of the selection are not
  offered.
- Blocked execution is explicit and diagnosable (emits
  `command:blocked_execution` diagnostic); silent no-op is forbidden.
- This rule applies uniformly: keyboard, command palette, radial menu, omnibar,
  graph-scoped Navigator hosts, and workbench-scoped Navigator hosts all use
  the same applicability check.

**Backlog anchors**: G29 (command applicability rule), NV13 (navigator command
applicability), G28 (mixed selection — the input to applicability).
**Plans**: [PLANNING_REGISTER §Interaction Decisions Receipt](PLANNING_REGISTER.md)
(§6 — command applicability, §5 — mixed selection targeting, §16 — command
target focus rule).
**Research**: Command Surface Interaction Spec (ActionRegistry as single
authority).

---

## 2. Legacy Cleanup Inventory

Old presentation logic, stale carriers, and authority violations that conflict
with the core invariants. These must be cleared or explicitly bridged before
new interaction features land safely.

### 2.1 Graph-Side Cleanup (INV-1, INV-3 violations)

| Item | Current state | Target | Backlog ref |
| --- | --- | --- | --- |
| Direct graph mutation callsites outside reducer | Inventoried but not fully retired | All durable graph mutations flow through `apply_reducer_intents()` or are explicitly marked non-durable | G02, G07, G08, G10 |
| `GraphIntent` variants that are actually workbench bridges | Classified via G06, bridge seam exists | Each bridge intent has explicit documentation; long-term target is typed `WorkbenchIntent` routing | G06, G44 |
| `workspace.selected_nodes` compatibility mirror | Legacy field shadowing per-view selection truth | Remove; selection truth is per-`GraphViewId` (INV-3) | G28 |
| Ambiguous edge/relation types | Multiple overlapping kinds exist | Relation family vocabulary (G13) unifies them | G12, G13, G14 |

### 2.2 Workbench-Side Cleanup (INV-2, INV-6 violations)

| Item | Current state | Target | Backlog ref |
| --- | --- | --- | --- |
| Legacy panel booleans on `GraphBrowserApp` | Removed for most panels | Pane-open/close state lives in tile tree only; no ad hoc booleans | WB02, WB06, WB07 |
| Frame membership as Vec, not graph edges | `FrameTabSemantics` holds membership locally | Frame membership expressed as `ArrangementRelation` / `frame-member` graph edges | G18, WB05 |
| Ad hoc helper functions for tile open/close | Scattered across several modules | Every pane open/close/focus/split path has one canonical carrier | WB08, WB10 |
| Tool pane authority confusion | Settings/command palette use mixed workbench/reducer paths | All tool panes route through workbench authority | WB17 |
| Dismiss/close conflation | Some close paths tombstone instead of dismiss | Close = dismiss (INV-6); delete is a separate explicit action | G23, WB07 |

### 2.3 Navigator-Side Cleanup (INV-1, INV-3 violations)

| Item | Current state | Target | Backlog ref |
| --- | --- | --- | --- |
| `ToolPaneState::FileTree` and `FileTreeContainmentRelationSource` | Kept alive as legacy placeholder | Retire once Navigator sections ship (replaced by containment-family projection) | NV04 |
| `SavedViewCollections` sidebar | Legacy pre-navigator saved-view UI | Replaced by arrangement-family navigator section | NV15 |
| Ad hoc navigator refresh on graph change | Direct observer coupling | Signal-driven refresh via `SignalBus` | NV10 |

### 2.4 Render / Canvas Cleanup

| Item | Current state | Target | Backlog ref |
| --- | --- | --- | --- |
| Hardcoded action lists in `command_palette.rs`, `radial_menu.rs` | Replicated local lists | All surfaces query `ActionRegistry::list_actions_for_context()` | UX Integration Research G-IS-1 |
| Command-surface context assembly inline in `render/mod.rs` | Partially extracted | Canvas requests palette/menu render; does not assemble command-surface policy | Render Mod Decomposition Plan |
| `render/mod.rs` monolith (3,764 lines) | Stage 1-2 extraction done | Continue decomposition: isolate camera, selection, lasso, search, physics interaction helpers by concern | Render Mod Decomposition Plan |

---

## 3. Diagnostics Refinement

Diagnostics are the shared observability contract. When the interaction model is
violated, the violation must be observable — not silent. This section defines the
diagnostic channels and invariant assertions needed to make UI/UX problems
diagnosable.

### 3.1 Existing Diagnostics Infrastructure

- `DiagnosticsRegistry` with `ChannelRegistry` and `AnalyzerRegistry`
- Per-channel `ChannelSeverity` (Error / Warn / Info)
- Diagnostics Inspector tool pane for runtime visualization
- Invariant test harness for contract assertions

### 3.2 Required Interaction-Model Diagnostic Channels

| Channel | Severity | Emits when | Invariant ref |
| --- | --- | --- | --- |
| `graph:mutation_outside_reducer` | Error | A graph mutation occurs outside `apply_reducer_intents()` | INV-1 |
| `graph:invalid_relation_payload` | Error | An edge is created with impossible endpoints or invalid relation family | INV-1, G20 |
| `graph:impossible_selection_state` | Error | Selection set contains a tombstoned node or non-existent key | INV-2, INV-3, G43 |
| `workbench:authority_violation` | Warn | A tile-tree mutation routes through the graph reducer instead of workbench authority | INV-6, G44 |
| `workbench:focus_failure` | Warn | Focus handoff fails after pane close (no valid successor found) | INV-2, WB24 |
| `workbench:blocked_close` | Info | A pane close is blocked by lock state | INV-2, WB24 |
| `workbench:restore_failure` | Warn | A pane restore fails (target node tombstoned or arrangement missing) | INV-6, WB24 |
| `workbench:dismiss_delete_conflation` | Error | A dismiss path triggers a tombstone or vice versa | INV-6, G23 |
| `navigator:stale_projection` | Warn | Navigator row references a node/frame that no longer exists in graph or workbench | INV-1, NV21 |
| `navigator:routing_failure` | Warn | Navigator action (click, expand, navigate) fails to resolve to a valid carrier | INV-4, NV21 |
| `input:binding_conflict` | Warn | Two bindings claim the same chord in the same context | INV-4 |
| `input:context_leak` | Error | An input context is popped that was never pushed, or not popped on surface dismiss | INV-4 |
| `command:blocked_execution` | Info | A command is invoked but blocked because it is not valid for every selected object | INV-7 |
| `command:surface_divergence` | Error | Same `ActionId` produces different behavior on different surfaces | INV-7 |
| `ux:selection_divergence` | Warn | Two surfaces disagree about the current selection state for the same `GraphViewId` | INV-3 |
| `ux:reveal_side_effect` | Warn | A reveal operation mutated selection, focus, or existence | INV-5 |

### 3.3 Invariant Assertions (Runtime)

Beyond diagnostic channels, the following runtime assertions should exist in
debug builds to catch interaction-model violations early:

- **Graph truth assertion**: After `apply_reducer_intents()`, no node in the
  selection set is tombstoned. (G47)
- **Single-write-path assertion**: Graph edge count and node count change only
  inside the reducer call boundary. (G09, G10)
- **Focus-selection coherence**: After workbench focus changes, the selected
  node set is updated to reflect the newly focused tile's content. (G30, G31)
- **Navigator projection freshness**: After a graph mutation signal, no
  navigator row references a stale `NodeKey`. (NV21)
- **Command parity**: `ActionRegistry::list_actions_for_context()` returns the
  same action set regardless of which surface calls it. (Command Surface Spec)

### 3.4 UX Telemetry (Local-Only)

For diagnosing interaction-model confusion (not violations, but user-experience
friction):

- Command abandonment rate (user opens palette, dismisses without executing)
- Focus confusion events (rapid focus-cycle without action)
- Undo-after-action rate (immediate undo suggests unintended action)

All telemetry is local-only, collected through `DiagnosticsRegistry`, never
transmitted. See UX Integration Research Deliverable 5.

---

## 4. Cross-Surface Wiring

State changes must propagate correctly between subsystems. This section
identifies the threading and signal routing changes needed.

### 4.1 Authority Boundaries (Two-Authority Model)

The system has two mutation authorities (SYSTEM_REGISTER.md):

1. **Graph Reducer** — `apply_reducer_intents()`: graph data model, node/edge
   lifecycle, traversal history, WAL, undo/redo.
2. **Workbench Authority** — frame-loop intercept, `tile_behavior.rs`,
   `tile_view_ops.rs`: tile-tree shape, pane open/close/focus.

Every interaction must be classifiable to one of these authorities. The routing
decision table (SYSTEM_REGISTER.md) is normative:

| Mechanism | When |
| --- | --- |
| Direct call | Same module / same struct, co-owned state |
| `GraphReducerIntent` | Mutation of graph data model |
| `WorkbenchIntent` | Mutation of tile-tree shape |
| Signal / `SignalBus` | Decoupled cross-registry notification |

**Anti-patterns to eliminate**:

- Calling `apply_intents` for tile-tree mutations (violates authority boundary)
- Direct calls across registry boundaries (use Signal instead)
- Accumulating workbench state in `GraphBrowserApp` workspace fields
- Background intent producers bypassing `ControlPanel`
- Dismiss paths that tombstone (INV-6 violation)
- Surfaces maintaining independent selection copies (INV-3 violation)

### 4.2 Signal Routing for Cross-Surface Agreement

The following state-change propagation paths must exist via `SignalBus`:

| Signal source | Signal | Observers |
| --- | --- | --- |
| Graph reducer (after `apply_intents`) | `GraphMutationCompleted` | Navigator (refresh projection), diagnostics (invariant check), canvas (re-render) |
| Workbench authority (after tile-tree mutation) | `WorkbenchLayoutChanged` | Navigator (refresh arrangement sections), diagnostics, graph-scoped Navigator host (frame chip update) |
| Selection model (after selection change) | `SelectionChanged(GraphViewId)` | Navigator (highlight row), canvas (re-render selection), command surfaces (re-evaluate applicability) |
| Focus model (after focus handoff) | `FocusChanged(PaneId)` | Selection model (update selection truth), navigator (active row), graph-scoped Navigator host (title update) |
| Lens / view config change | `ViewConfigChanged(GraphViewId)` | Canvas (re-render), physics (re-parameterize), navigator (optional section visibility) |

These signals do not introduce new routing mechanisms — they use the existing
`SignalBus` / `SignalRoutingLayer` contract (SR2/SR3 in SYSTEM_REGISTER.md).
The goal is to replace ad hoc observer coupling (NV10) with shared reusable
signal paths.

### 4.3 Navigator Chrome Model

**Updated 2026-03-22**: The two-surface chrome split (Graph Bar + Workbench
Sidebar as fixed separate surfaces) described in
`subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` is superseded
by the unified Navigator model. See `navigator/NAVIGATOR.md §12` for the
canonical model.

Summary: there is one Navigator surface with two orthogonal settings — form
factor (Sidebar panel or Toolbar bar) and scope (Both / GraphOnly /
WorkbenchOnly / Auto). The controls previously attributed to "Graph Bar" and
"Workbench Sidebar" are now scope-sections within the unified Navigator.

The goal this section was enforcing remains unchanged:

- Graph-scope controls (undo/redo, new node/edge, omnibar, physics, lens,
  tag filters, sync) must always be available regardless of which tiles are open
- Arrangement controls (frame management, tile layout, back/forward) are
  contextual to workbench state

These goals are achieved through Navigator scope configuration rather than
fixed surface separation.

**Wiring implications** (unchanged):

- Graph-scope Navigator sections read from graph truth + selection model;
  write through `ActionRegistry` → graph reducer.
- Workbench-scope Navigator sections read from tile tree + workbench session
  state; write through `ActionRegistry` → workbench authority.
- Both scope sections must receive `SelectionChanged` and `FocusChanged`
  signals to stay current.

### 4.4 ActionRegistry as Single Command Authority

All command surfaces (keyboard, command palette, radial menu, omnibar,
graph-scoped Navigator hosts, workbench-scoped Navigator hosts) must route
through `ActionRegistry`:

- `ActionRegistry::list_actions_for_context(...)` defines visibility.
- `ActionRegistry::execute(...)` defines execution.
- Context includes the full resolved selection set.
- Commands are available only if valid for every selected object (INV-7).
- Blocked execution is explicit and diagnosable; silent no-op is forbidden.

**Current gaps** (from UX Integration Research):

- `command_palette.rs` and `radial_menu.rs` maintain hardcoded action lists
  that must be replaced with `ActionRegistry` queries.
- Toolbar settings has a local action list that must be unified.

---

## 5. Surface Agreement Checklist

The specific cross-pack backlog items that must converge. These are the items
where graph, workbench, and navigator directly depend on each other.

### 5.1 Selection Agreement (Gravity Center)

Selection is the interaction model's gravity center — it determines what
commands are available and what surfaces display.

| Item | Packs | Status | Done gate |
| --- | --- | --- | --- |
| Mixed selection model | G28 | Specified | Nodes, edges, frames, tiles, arrangement objects coexist in one target set |
| Command applicability | G29, NV13 | Specified | Commands valid only for every selected object; no implicit fallback |
| Selection reveal | G30, NV11 | Specified | Reveal-on-select only when graph is visible and node is offscreen |
| Selection lifecycle | G31, NV07 | Specified | Hidden surfaces retain memory but not live selection |
| Selection-to-workbench targeting | WB22 | Not started | Selected objects that map to workbench actions become explicit targets |
| Navigator selection contract | NV07, NV12 | Not started | Navigator selection maps onto global mixed-selection model |

### 5.2 Click Grammar Agreement

| Item | Packs | Status | Done gate |
| --- | --- | --- | --- |
| Navigator click grammar lock | NV06 | Not started | Row-type single/double-click is canonical |
| Navigator residency-aware navigation | NV08 | Not started | Double-click routes differently for live vs cold nodes |
| Edge selection semantics | G32 | Not started | Edge click behavior is distinct from node selection |
| Navigator structural row focus | NV09 | Not started | Frame/tile rows expand/collapse and get command focus |

### 5.3 Authority Boundary Agreement

| Item | Packs | Status | Done gate |
| --- | --- | --- | --- |
| Reducer/workbench boundary cleanup | G44, WB06 | Partial | Graph and tile mutations no longer share misleading carriers |
| Arrangement relation contract | G18, WB05 | Partial | Frame membership is graph-backed relation families |
| Workbench intent inventory | WB06 | Not started | Every workbench action mapped to `WorkbenchIntent`, graph bridge, or legacy |
| Graph truth vs presentation contract | G04 | Not started | One doc lists durable truth vs per-view projection |

### 5.4 Focus Model Agreement

| Item | Packs | Status | Done gate |
| --- | --- | --- | --- |
| Focus model contract | WB21 | Not started | Pane focus, tab focus, frame focus are distinguished and routed consistently |
| Focus return path algorithm | UX Integration Research | Not started | Focus return after modal/palette dismiss is computable from documented algorithm |
| Focus-selection coherence | G30, G31, WB21 | Not started | Focus changes update selection truth |

### 5.5 Navigator–Graph–Workbench Sync

| Item | Packs | Status | Done gate |
| --- | --- | --- | --- |
| Navigator projection refresh | NV10 | Not started | Signal-driven, not ad hoc observer |
| Navigator arrangement projection | NV15 | Not started | Frames/tiles project as expandable arrangement objects |
| Workbench–navigator contract sync | NV23, WB25 | Not started (hard dependency) | Pane/focus and projection semantics no longer contradict |
| Graph–navigator contract sync | NV24, G13, G15, G21 | Not started | Navigator sections and graph relation/view semantics align |

---

## 6. Core Extraction Alignment

The `graphshell-core` crate extraction (see
`technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`) defines
the portable kernel that must be identical across all deployment contexts. The
core interaction model must respect this boundary.

### 6.1 What Belongs in `graphshell-core`

These interaction-model components are part of the portable core:

| Component | Location today | Core rationale |
| --- | --- | --- |
| `Graph`, `NodeKey`, `Node`, `EdgePayload` | `model/graph/mod.rs` | If platforms disagree about graph identity, sync is impossible |
| `GraphIntent` + `apply_intents()` | intent system | Single mutation authority must be identical everywhere |
| Relation family vocabulary | Design docs (not yet in code) | Navigator, arrangement, history all depend on it |
| Selection model (per-`GraphViewId`) | `app/selection.rs` | Commands must see the same selection everywhere |
| `GraphWorkspace` / `DomainState` | `graph_app.rs` / `app/workspace_state.rs` | State container for all durable graph truth |
| `GraphPos2`, physics `step()` | (extraction target) | Layout positions must be identical for sync |
| `Address`, `HistoryEntry`, URL normalization | `model/graph/mod.rs` | Wire format for node identity |
| WAL log entry types, snapshot serialization | `services/persistence/` | Cross-platform sync contract |
| UDC `CompactCode`, `semantic_tags` | `registries/domain/` | Tagging must agree across platforms |
| Coop session authority rules | (not yet in code) | Security guarantees are meaningless if platform-specific |

### 6.2 What Stays in the Desktop Shell

These interaction-model components are shell-specific:

| Component | Reason |
| --- | --- |
| `egui_tiles` tree, tile compositor, pane chrome | Platform-specific layout system |
| `render/mod.rs` canvas orchestration | egui-dependent rendering |
| Navigator UI rendering | Platform-specific presentation |
| `KnowledgeRegistry`, `reconcile_semantics` | Thread-dependent, heavy |
| Servo / Wry viewer lifecycle | Platform-specific viewer backends |
| `GraphBrowserApp` application state | UI state, webview maps |
| `CompositorAdapter`, `TileRenderMode` | GL/render pipeline concerns |

### 6.3 Boundary Discipline

The interaction model introduces several cross-boundary contracts. To keep the
core extraction clean:

- **Selection model** is core (the data structure and rules), but selection
  **rendering** (highlight, lasso) is shell.
- **Click grammar rules** are core (what intent a click produces), but click
  **detection** (pointer events, gesture recognition) is shell.
- **Command applicability rules** are core (`ActionRegistry` query logic), but
  command **surface rendering** (palette, radial menu, omnibar UI) is shell.
- **Focus model rules** are core (what focus means), but focus **visual
  affordances** (ring, highlight, tab underline) are shell.
- **Relation family vocabulary** is core, but **navigator section rendering**
  is shell.
- **`GraphSemanticEvent`** is the only type that crosses from core to host. It
  replaces direct coupling between core mutations and shell rendering.

### 6.4 WASM Portability Constraints on Interaction Model

- Core never calls `Uuid::new_v4()` — all IDs are generated by the host.
- Core never imports `egui::*` — positions use `GraphPos2`.
- Core never uses `std::thread` — physics `step()` is single-threaded and pure.
- Core never opens files — `Address::File` compiles but is resolved by the host.
- Core never does network I/O — sync transport is host-only.

---

## 7. Execution Priorities

### 7.1 Phase 1 — Authority Boundaries and Cleanup

**Goal**: Graph, workbench, and navigator agree on who owns what.

1. Complete G01 (Graph Core Boundary) and WB01 (Workbench Core Boundary) —
   one canonical doc each.
2. Complete G02 (Mutation Entry Audit) and WB02 (Tile Tree Ownership Audit) —
   inventory every mutation path and tag its owner.
3. Complete G06 (Intent Carrier Classification) and WB06 (Workbench Intent
   Inventory) — classify every intent/action to its authority.
4. Wire G07/WB07 legacy mutation diagnostics — violations emit warnings.
5. Complete NV01 (Navigator Projection Boundary) — navigator reads only,
   never owns truth.

**Exit criteria**: Every mutation path in the system is tagged to an authority
with violations surfaced in diagnostics.

### 7.2 Phase 2 — Shared Vocabulary and Identity Contracts

**Goal**: Terms mean the same thing everywhere.

1. Lock glossaries: G03, WB03, NV02 — every entity has one canonical
   definition.
2. Define relation family vocabulary: G13 — one shared vocabulary for
   navigator sections, arrangement, history, and copy provenance.
3. Define node/edge canonical shapes: G11, G12 — payload contracts cover all
   required fields.
4. Define graph truth vs presentation: G04 — one doc lists durable truth vs
   per-view projection.
5. Define navigator section mapping: NV04 — each section is mapped to its
   authority source.

**Exit criteria**: No two documents define the same term differently. Every
data contract names its authority and persistence tier.

### 7.3 Phase 3 — Selection and Click Grammar

**Goal**: Users can predict what clicking does, and commands always target the
right set.

1. Implement mixed selection model: G28.
2. Implement command applicability rule: G29, NV13.
3. Lock navigator click grammar: NV06.
4. Implement residency-aware navigation: NV08.
5. Implement selection reveal: G30, NV11.
6. Implement selection lifecycle: G31.
7. Wire selection-to-workbench targeting: WB22.
8. Wire `ActionRegistry` as single command authority — unify
   `command_palette.rs` and `radial_menu.rs` hardcoded lists.

**Exit criteria**: Click behavior is deterministic and documented for every
surface. Command applicability is enforced from the selection set. No silent
fallback to hidden targets.

### 7.4 Phase 4 — Focus, Arrangement, and Projection Agreement

**Goal**: Focus is coherent, frames are graph-backed, and navigator projects
correctly.

1. Define focus model contract: WB21.
2. Implement arrangement relation contract: G18, WB05 — frame membership as
   graph edges.
3. Wire navigator projection refresh via signals: NV10.
4. Implement navigator arrangement projection: NV15.
5. Implement cross-surface diagnostics: G43, WB24, NV21.
6. Implement unified Navigator chrome with scope/form-factor configuration
   per `navigator/NAVIGATOR.md §12`. The chrome scope split plan
   (`2026-03-13_chrome_scope_split_plan.md`) remains a valid execution
   reference for the controls involved, but the target surface model is the
   unified Navigator, not two fixed bars.

**Exit criteria**: Focus changes propagate to selection and navigator via
signals. Frames are graph-backed arrangement relations. Navigator refreshes on
signal, not ad hoc coupling. Diagnostics cover focus failure, stale projection,
and authority violations.

### 7.5 Phase 5 — Convergence and Hardening

**Goal**: The interaction model is internally consistent and resilient.

1. Run workbench–navigator contract sync: NV23.
2. Run graph–navigator contract sync: NV24.
3. Run graph scenario test matrix: G46.
4. Run navigator scenario test matrix: NV22.
5. Add runtime invariant assertions: G47.
6. Run hardening slice for highest-risk graph paths: G49.
7. Run inter-plan audit checkpoint.

**Exit criteria**: Scenario tests cover mixed selection, dismiss/delete, copy
provenance, relation visibility, click grammar, reveal, and recents. Runtime
assertions catch interaction-model violations in debug builds. Audit receipt
published.

---

## 8. Binding Interaction Decisions (Receipted)

The following decisions from the Interaction Decisions Receipt (2026-03-16) are
binding inputs to this plan. They are not re-decided here — they are adopted
as constraints.

1. Navigator click grammar is row-type specific (§1 → INV-4)
2. Residency-aware node navigation (§2 → INV-4)
3. Selection reveal rule (§3 → INV-5)
4. Selection lifecycle (§4 → INV-2, INV-3)
5. Mixed selection targeting (§5 → INV-3, INV-7)
6. Command applicability rule (§6 → INV-7)
7. Tile terminology (§7 → INV-1)
8. Cross-context reuse model: Move/Associate/Copy (§8 → INV-1, INV-6)
9. Copy provenance (§9 → INV-1)
10. Edge presentation model (§10 → INV-1)
11. Graph-view copy semantics (§11 → INV-1, INV-6)
12. Node dismiss lifecycle (§12 → INV-6)
13. Recent semantics (§13 → INV-2)
14. Switch Surface semantics (§14 → INV-2)
15. Arrangement object semantics on graph (§15 → INV-1, INV-3)
16. Command target focus rule (§16 → INV-7)

---

## 9. Success Criteria

The core interaction model is settled when:

- [ ] All seven core invariants have runtime diagnostic coverage.
- [ ] Every mutation path is tagged to graph or workbench authority with no
      unclassified legacy paths remaining.
- [ ] Selection propagation is single-source (INV-3); no surface maintains an
      independent selection copy.
- [ ] Click grammar (INV-4) and command applicability (INV-7) are consistent
      across graph canvas, navigator, and command surfaces.
- [ ] Reveal is side-effect-free (INV-5) with diagnostic coverage.
- [ ] Dismiss and delete are cleanly separated (INV-6) across all surfaces.
- [ ] Navigator is purely projection-driven with signal-based refresh.
- [ ] Frame membership is expressed as graph-backed arrangement relations.
- [ ] Focus model is explicit with deterministic handoff and documented
      return paths.
- [ ] `graphshell-core` boundary is respected — no egui types in core
      interaction contracts.
- [ ] Cross-surface scenario tests exist and pass.
- [ ] Inter-plan audit receipt is published.

Reaching these criteria is sufficient to claim a version-number milestone for
the interaction model.
