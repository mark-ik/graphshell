# Foundational Reset Architecture Vision

**Date**: 2026-03-06
**Status**: Active architecture direction
**Purpose**: Define the default long-term architecture, vocabulary, and API shape Graphshell should converge toward while the product is still free of legacy pressure.

**Related**:
- `system_architecture_spec.md`
- `register_layer_spec.md`
- `2026-03-06_foundational_reset_migration_governance.md`
- `2026-03-06_foundational_reset_demolition_plan.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `../subsystem_history/edge_traversal_spec.md`
- `../canvas/semantic_tagging_and_knowledge_spec.md`
- `../../TERMINOLOGY.md`

---

## 1. Decision Summary

Graphshell should be treated as a graph-native workspace engine, not as a browser with graph features bolted on.

The default architectural vision is:

- one canonical identity model
- one canonical state model
- one canonical mutation pipeline
- one canonical route-and-identity model
- one canonical meaning for pane, frame, history, and semantic membership

This document is intentionally stricter than the current prototype. It describes the architecture worth moving toward, not the compromises currently tolerated to keep momentum.

---

## 1A. Relationship To Existing System Docs

This reset document introduces a **state-container axis**. It does not replace the existing **subsystem-ownership axis** already defined in active system docs.

Those two axes should be read together:

- subsystem docs answer: "which subsystem owns this behavior or policy?"
- reset docs answer: "which state container should store this data?"

Examples:

- Graph subsystem may own graph-camera interaction policy while view-local camera state lives in `WorkbenchState`.
- Focus subsystem may own focus rules while concrete focus/selection state lives in `WorkbenchState`.
- Register layer may own routing/composition policy while runtime handles live in `RuntimeState`.

This reset also does not invalidate the earlier `GraphWorkspace` / `AppServices` split described in `2026-02-22_registry_layer_plan.md`.

That earlier split should be treated as a precursor:

- `GraphWorkspace` is the prototype-era container that still mixes future `DomainState` and `WorkbenchState`, plus some transitional runtime residue
- `AppServices` is the runtime/service boundary precursor to `RuntimeState` plus registry/runtime service infrastructure

The reset refines that split; it is not a separate competing split.

---

## 2. Product-Level Vision

Graphshell should feel like:

- a durable knowledge workspace
- whose primary data model is a graph of nodes, notes, edges, traversals, and semantic classifications
- whose browser behavior is a projection over that graph
- whose workbench behavior is another projection over that graph
- whose runtime/webview/compositor layer is an implementation detail, not the semantic authority

Graphshell should **not** default to any of these mental models:

- "browser first, graph second"
- "graph editor plus some webviews"
- "window manager whose semantics are whichever subsystem wrote last"

The core truth should be:

- domain meaning is durable
- workbench state is explicit
- runtime state is disposable

---

## 3. Canonical State Model

### 3.1 State Layers

Graphshell should have three explicit state layers:

1. `DomainState`
2. `WorkbenchState`
3. `RuntimeState`

`AppState = DomainState + WorkbenchState + RuntimeState`

### 3.2 DomainState

`DomainState` is durable, replayable, and sync-worthy.

It owns:

- node identities and durable node metadata
- edge identities and durable edge metadata
- notes and note metadata
- traversal history and archive summaries
- semantic tags and semantic index inputs
- saved views and durable authored graph structure
- committed graph positions

`DomainState` must not contain:

- live webview handles
- compositor resources
- frame-local hover state
- ephemeral runtime caches

### 3.3 WorkbenchState

`WorkbenchState` is app-owned but not part of the domain graph.

It owns:

- pane instances
- frame membership
- tile trees and layout state
- focus ownership
- selection state
- route-open policy decisions
- local view preferences

`WorkbenchState` is not the semantic authority for node identity or traversal truth.

### 3.4 RuntimeState

`RuntimeState` is effectful and disposable.

It owns:

- webview ids and live mappings
- compositor bindings
- caches
- async in-flight work
- preview sessions
- replay cursors that are runtime execution details rather than durable domain history

`RuntimeState` must be reconstructible from `DomainState + WorkbenchState + environment`.

---

## 4. Canonical Mutation Pipeline

### 4.1 Replace "everything is a GraphIntent"

The prototype currently overloads `GraphIntent` to represent too many categories of change.

The long-term shape should be:

1. `AppCommand`
2. `AppPlan`
3. `AppTransaction`
4. `AppEffect`

Canonical flow:

`Command -> Plan -> Transaction -> Apply -> Effects`

### 4.2 AppCommand

`AppCommand` describes what the user or system is asking for.

Examples:

- `OpenAddress`
- `CreateNode`
- `AttachPaneToFrame`
- `RecordTraversal`
- `ClassifyNode`
- `EnterHistoryPreview`

`AppCommand` is request-shaped, not storage-shaped.

### 4.3 AppPlan

`AppPlan` is a planner output. It resolves routing, policy, and ownership decisions before state apply.

It may answer questions like:

- which frame should open this pane
- whether this action creates a durable undo boundary
- whether this route targets a graph node, note, tool, or settings surface
- whether an effect is permitted in preview mode

### 4.4 AppTransaction

`AppTransaction` is the canonical pure state change unit.

It should contain:

- `domain_deltas`
- `workbench_deltas`
- `cause`
- `transaction_id`
- `undo_boundary_policy`

This is the preferred truth surface for:

- undo/redo
- persistence
- replay
- diagnostics
- test harness assertions

### 4.5 AppEffect

`AppEffect` is the only place where the runtime layer performs side effects.

Examples:

- create/destroy webview
- persist transaction
- emit diagnostics event
- request compositor update
- schedule async fetch/reconcile work

Effects run after pure state apply.

---

## 5. Identity and Addressing Model

### 5.1 Canonical IDs

Every durable entity should have a first-class identity type.

At minimum:

- `NodeId`
- `EdgeId`
- `NoteId`
- `ViewId`
- `PaneId`
- `FrameId`
- `TransactionId`

Do not use URL strings, webview ids, or pane instance keys as substitutes for durable identity.

### 5.2 Canonical Addresses

Addresses are routes, not identities.

The canonical route namespace for system/workbench surfaces should remain `verso://`.

Durable content identity must remain in content-domain authorities such as:

- `notes://`
- `graph://`
- `node://`

`verso://view/...` routes may consume durable content ids, but they must not become the durable identity authority for those records.

Address resolution should return a typed open target while preserving the distinction between route namespace and durable identity authority, such as:

- `NodeTarget`
- `NoteTarget`
- `GraphTarget`
- `ViewTarget`
- `ToolTarget`
- `SettingsTarget`

### 5.3 Identity Rule

A node may have a URL.
A pane may display a node.
A route may open a pane.
A webview may currently realize that pane.

Those are four different things and must never share one overloaded identifier.

---

## 6. Durable vs Projected Position

This is a foundational split worth keeping.

Nodes should have:

- `committed_position`
- `projected_position`

Rules:

- `committed_position` lives in `DomainState`
- `projected_position` lives in runtime/projection state
- reducer/domain transactions may commit authored position changes
- render/physics may update projected position freely
- render/physics must not silently redefine durable authored layout

This split removes one of the largest sources of ambiguity in the prototype.

---

## 7. Semantic Model

### 7.1 Semantic Truth Is Plural

The canonical semantic model should be set-based, not scalar.

A node may legitimately belong to multiple semantic classes.

Canonical representation:

- `semantic_tags: Set<Tag>`
- `semantic_classes: Set<SemanticClass>`
- optional `primary_class` only for compatibility or display reduction

### 7.2 Reduction Rule

If a single semantic class is needed for display or compatibility, derive it from the plural set deterministically.

The plural set remains canonical.

### 7.3 Why This Matters

The product vision already wants:

- bridge nodes
- cross-cluster membership
- faceted filtering across multiple classes
- semantic grouping that is not forced into one bucket

The architecture should match that intent directly.

---

## 8. History Model

History should be modeled as a first-class temporal event stream, not as an awkward byproduct of browser callbacks.

Canonical distinction:

- `TraversalEvent` is the directed temporal event
- `EdgeRecord` is the durable relationship summary enriched by traversal reduction

History preview/replay should operate over transactions and temporal events, not by mutating live runtime state directly.

Preview-mode rules should remain strict:

- no live runtime side effects
- no live persistence writes
- no silent fallback into normal mode

---

## 9. Workbench Model

### 9.1 Pane and Frame Semantics

Panes, frames, and routing should be modeled as workbench semantics, not mixed into graph semantics.

A pane is:

- a view instance over some target

A frame is:

- a workbench container with layout/focus behavior

### 9.2 Open Modes

Open-mode policy should be explicit:

- reuse current pane
- open tab in current frame
- split current frame
- route to named/preferred frame
- open detached surface if such a concept exists

This should be planner logic, not incidental UI branching.

---

## 10. Terms To Retire or Restrict

The prototype still carries a few overloaded words that should be narrowed now.

### 10.1 Promotion

Do not use `promotion` to mean more than one thing.

Allowed meaning:

- transition from ephemeral/non-graph-backed browsing context into graph-backed durable domain participation

Disallowed meanings:

- pane hoist/unhoist
- docked vs tiled transitions
- generic focus elevation

If structural pane movement needs a name, use terms like:

- `hoist`
- `dock`
- `tile`
- `detach`

### 10.2 Frame Affinity vs Zone

Do not keep both unless they mean different things.

Preferred resolution:

- `frame membership` is workbench truth
- if "zone" survives, it must be a derived visual/layout grouping, not a second competing membership model

### 10.3 Focus vs Hover vs Preferred Input

These must not collapse into one fuzzy notion.

Use explicit terms:

- `semantic_focus_owner`
- `pointer_hover_target`
- `preferred_input_target`

Hover should not silently become semantic focus unless the policy says so explicitly.

### 10.4 Lifecycle Terms

If `Active/Warm/Cold/Tombstone` remain, they should describe one specific lifecycle model only.

Do not reuse them to describe pane state, frame state, preview state, or semantic availability.

---

## 11. Proposed API Shape

The long-term API should read semantically, not procedurally.

Examples of good default command shapes:

```rust
enum AppCommand {
    OpenAddress {
        address: VersoAddress,
        target_policy: OpenTargetPolicy,
    },
    CreateNode {
        address: Option<NodeAddress>,
        committed_position: Point2D<f32>,
    },
    RecordTraversal {
        from: NodeId,
        to: NodeId,
        trigger: NavigationTrigger,
        timestamp_ms: u64,
    },
    ClassifyNode {
        node_id: NodeId,
        tags: Vec<String>,
    },
    CommitNodePosition {
        node_id: NodeId,
        position: Point2D<f32>,
    },
    EnterHistoryPreview {
        cursor: HistoryCursor,
    },
}
```

Examples of good delta shapes:

```rust
enum DomainDelta {
    NodeCreated { ... },
    NodeRemoved { ... },
    NodeMetadataUpdated { ... },
    EdgeUpserted { ... },
    TraversalRecorded { ... },
    SemanticTagsUpdated { ... },
    CommittedPositionChanged { ... },
}

enum WorkbenchDelta {
    PaneOpened { ... },
    PaneClosed { ... },
    FrameLayoutChanged { ... },
    FocusChanged { ... },
    SelectionChanged { ... },
}
```

The point is not the exact names. The point is that the API should communicate meaning clearly.

---

## 12. Contradictions Worth Eliminating Now

These are the contradictions the prototype should stop tolerating:

1. A single intent type representing graph, workbench, and runtime policy changes interchangeably.
2. A single position field serving both authored persistence and transient simulation.
3. Semantic plurality desired at the UX level but singularity enforced in storage.
4. Promotion meaning both graph enrollment and structural pane movement.
5. Frame/zone semantics competing as if both are canonical membership truth.
6. Focus/hover/preferred-input semantics crossing subsystem boundaries without one explicit authority.
7. History being treated partly as browser integration trivia and partly as first-class temporal state.

---

## 13. Recommended Migration Sequence

### Stage 1 - Vocabulary reset

- ban overloaded meanings in active specs
- declare canonical meanings for promotion, frame membership, focus ownership, and history preview

### Stage 2 - State split

- formalize `DomainState`, `WorkbenchState`, `RuntimeState`
- move obvious runtime-only fields out of domain/app blobs

### Stage 3 - Transaction model

- introduce `AppTransaction`
- make undo/redo and persistence operate on transactions rather than ad hoc mixed snapshots

### Stage 4 - Command/planner separation

- replace graph-shaped universal intents with `AppCommand`
- introduce planner output for routing/open-mode/policy decisions

### Stage 5 - Full delta normalization

- all durable domain writes route through canonical domain deltas
- all workbench writes route through canonical workbench deltas

### Stage 6 - Runtime isolation

- runtime/webview/compositor code consumes effects and projections only
- runtime code is no longer a backdoor semantic authority

---

## 14. Immediate Defaults To Instantiate

If only a few foundational defaults are instantiated now, they should be these:

1. Keep the durable vs projected position split and extend it consistently.
2. Continue moving durable graph mutation into canonical delta/apply paths.
3. Introduce the state-layer split in documentation before code fully matches it.
4. Treat semantic plurality as canonical going forward.
5. Retire overloaded terminology in active specs as soon as touched.
6. Treat transactions as the eventual authority surface for undo, replay, persistence, and diagnostics.

---

## 15. Guiding Principle

The default vision of Graphshell should be:

> A durable, inspectable, replayable knowledge-work engine whose browser and workbench surfaces are projections over a canonical graph-centered domain model.

That vision is stricter than the prototype, but it is a better default. The prototype should move toward it deliberately rather than normalize its current shortcuts into permanent architecture.
