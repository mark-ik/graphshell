<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `graphshell-core` Extraction Plan

**Date**: 2026-03-08  
**Updated**: 2026-04-11  
**Status**: Design / Active extraction planning  
**Scope**: Extract the identity, authority, and mutation kernel of graphshell
into a WASM-clean crate (`graphshell-core`) that compiles to
`wasm32-unknown-unknown` with zero errors and has no knowledge of egui, wgpu,
Servo, or platform I/O. This crate is the shared foundation for desktop, mobile,
browser extension, browser/PWA, and Verse-side hosts.

**2026-04-11 revision**:

- folds in the useful sequencing from the April execution draft while keeping
  the broader crate boundary intact
- makes Step 4 an explicit multi-phase extraction program instead of one giant
  move
- clarifies that a graph-model-only extraction is **not** sufficient to earn
  the `graphshell-core` name
- introduces a deliberate split between the current host-wide `GraphIntent`
  enum and the portable durable mutation boundary that core must own as
  `CoreIntent`
- adds a host-shim migration pattern, visibility audit, and per-phase WASM
  gates
- reflects recent portable-subsystem work: `graph-tree` is now a sibling crate,
  which clarifies that `graphshell-core` is the kernel beneath portable
  workbench/view crates, not a grab-bag for every portable subsystem

**Related docs**:

- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
  — singular portable web/document engine vs. host-envelope architecture
- [`graph_tree_spec.md`](graph_tree_spec.md)
  — sibling portable workbench-tree crate; clarifies what stays *outside*
  `graphshell-core`
- [`graph_canvas_spec.md`](graph_canvas_spec.md)
  — sibling portable graph-view/canvas subsystem
- [`2026-04-11_core_intent_inventory.md`](2026-04-11_core_intent_inventory.md)
  — first-pass mapping from host `GraphIntent` to `CoreIntent` / host-only buckets
- [`graph/petgraph_algorithm_utilization_spec.md`](../implementation_strategy/graph/petgraph_algorithm_utilization_spec.md)
  — petgraph algorithm surface
- [`graph/2026-02-24_physics_engine_extensibility_plan.md`](../implementation_strategy/graph/2026-02-24_physics_engine_extensibility_plan.md)
  — current physics extensibility architecture
- [`graph/semantic_tagging_and_knowledge_spec.md`](../implementation_strategy/graph/semantic_tagging_and_knowledge_spec.md)
  — UDC semantic tagging and KnowledgeRegistry contract
- [`2026-02-18_universal_node_content_model.md`](2026-02-18_universal_node_content_model.md)
  — node identity / `Address` enum
- [`viewer/clipping_and_dom_extraction_spec.md`](../implementation_strategy/viewer/clipping_and_dom_extraction_spec.md)
  — clip artifact lane and downstream publication boundary
- [`../../verso_docs/implementation_strategy/coop_session_spec.md`](../../verso_docs/implementation_strategy/coop_session_spec.md)
  — Co-op session authority

---

## 1. Purpose and Principle

`graphshell-core` is not a graph library. It is the **identity, authority, and
mutation kernel** of the graphshell system: the minimum portable logic that must
agree across all hosts or the system becomes incoherent.

The test for any candidate component:

> If two hosts disagree about this, can the system still function coherently?

If the answer is no, it belongs in core.

Target host contexts:

| Context | How core is used | Binding layer | Notes |
| --- | --- | --- | --- |
| Desktop app (Windows/macOS/Linux) | Native dependency | direct Rust dep | host adds egui, wgpu, Servo, fjall, iroh |
| Mobile app (iOS/Android) | Native dependency | `graphshell-core-uniffi` | host adds native UI and storage |
| Browser extension | Compiled to WASM | `graphshell-core-wasm` | browser provides JS runtime and WebAssembly host |
| Browser tab / PWA | Compiled to WASM | `graphshell-core-wasm` | browser host may be richer than extension host, but still not a native runtime |
| Verse node / headless service | Native or WASM | direct Rust dep or WASI wrapper | host adds networking and storage |
| Test harness | Native dependency | direct Rust dep | no UI or shell layer required |

The WASM compilation constraint is the mechanical enforcement mechanism. If
`graphshell-core` compiles to `wasm32-unknown-unknown` with zero errors, it is
definitionally free of platform dependencies. That rule is stronger than code
review and must be used continuously during extraction, not only at the end.

`wasm-bindgen` and UniFFI annotations must **not** appear inside
`graphshell-core`. Those belong in thin wrapper crates:

- `graphshell-core-wasm`
- `graphshell-core-uniffi`

---

## 2. What Belongs in Core

### 2.1 Kernel Boundary Summary

| Category | Core owns | Host owns |
| --- | --- | --- |
| Identity | `NodeId`, `Address`, URL normalization, canonical persisted identity | browser/webview/session handles, filesystem resolution |
| Graph model | `Graph`, `Node`, `EdgePayload`, graph-side leaf types, algorithm accessors | egui adapters, render styles, shell-only derived caches |
| Durable mutation authority | portable durable intent enum and reducer | UI/shell/workbench/runtime orchestration intents that translate into core mutations |
| Domain events | portable domain event surface emitted from core | host lifecycle buses and UI notifications |
| State container | `GraphWorkspace` and intentionally portable session state | `GraphBrowserApp`, shell state, render state, compositor state |
| Persistence schema | snapshots, WAL entries, replay helpers, schema versioning | fjall/IndexedDB/SQLite adapters, transport/storage locations |
| Session authority | Coop types and approval rules | UX for approval, presence rendering, command gating UI |
| Publication schema | portable event payload structs and normalization | signing, relay I/O, credential handling |
| Layout / physics | portable position type, topology classifier, headless layout/physics math | egui_graphs adapter, view renderer, camera/input semantics |

### 2.2 Graph Domain State and Algorithms

Every host that touches graph truth must share the same graph model and the same
algorithm surface.

Core owns:

- `Graph`
- `Node`
- `NodeKey` / `EdgeKey`
- `EdgePayload`
- graph-side classification/import/frame-layout leaf types
- graph filtering / facet projection helpers that are pure and portable
- graph algorithm accessors (neighbors, reachability, connected components,
  topology classification, etc.)
- `GraphDelta` or equivalent batch mutation type used by persistence replay and
  reducer internals

### 2.3 Durable Mutation Boundary

The current host-wide `GraphIntent` enum is too broad and too entangled with
shell/workbench/runtime concerns to move into core unchanged. That does **not**
mean mutation authority stays in the host. It means the mutation boundary must
be **split deliberately** during extraction.

Core owns:

- a portable durable intent enum, `CoreIntent`
- `apply_core_intents()` (or equivalent reducer API)
- the serialization format for durable mutations
- reducer-side validation and postconditions

The host keeps:

- UI-only intents
- shell lifecycle intents
- workbench/layout/compositor intents
- runtime/webview bookkeeping intents
- any orchestration layer that translates host actions into one or more
  `CoreIntent` values

Notes:

- `CoreIntent` is intentional, not provisional. It marks the portable mutation
  contract clearly and avoids conflating the core reducer boundary with the
  broader host orchestration enum.
- The important invariant is that **all durable graph mutations pass through the
  portable reducer in core**.

### 2.3a Draft `CoreIntent` Surface

The first extraction target should define `CoreIntent` as a deliberately smaller
and cleaner enum than the current host `GraphIntent`.

Recommended top-level shape:

```rust
pub enum CoreIntent {
    Graph(CoreGraphIntent),
    View(CoreViewIntent),
    Session(CoreSessionIntent),
    Sync(CoreSyncIntent),
}
```

Recommended initial contents:

```rust
pub enum CoreGraphIntent {
    AddNode { id: NodeId, address: Address, position: Point2D<f32> },
    RemoveNode { key: NodeKey },
    RestoreGhostNode { key: NodeKey },
    SetNodeAddress { key: NodeKey, address: Address },
    SetNodeTitle { key: NodeKey, title: String },
    SetNodePinned { key: NodeKey, is_pinned: bool },
    SetNodePosition { key: NodeKey, position: Point2D<f32> },
    AddRelation { from: NodeKey, to: NodeKey, selector: RelationSelector, label: Option<String> },
    RemoveRelation { from: NodeKey, to: NodeKey, selector: RelationSelector },
    TagNode { key: NodeKey, tag: String },
    UntagNode { key: NodeKey, tag: String },
    AssignClassification { key: NodeKey, classification: NodeClassification },
    UnassignClassification { key: NodeKey, scheme: ClassificationScheme, value: String },
    AcceptClassification { key: NodeKey, scheme: ClassificationScheme, value: String },
    RejectClassification { key: NodeKey, scheme: ClassificationScheme, value: String },
    SetPrimaryClassification { key: NodeKey, scheme: ClassificationScheme, value: String },
}

pub enum CoreViewIntent {
    SetViewLensId { view_id: GraphViewId, lens_id: String },
    SetViewLayoutAlgorithm { view_id: GraphViewId, algorithm_id: String },
    SetViewPhysicsProfile { view_id: GraphViewId, profile_id: String },
    SetViewFilter { view_id: GraphViewId, expr: Option<FacetExpr> },
    ClearViewFilter { view_id: GraphViewId },
    SetViewDimension { view_id: GraphViewId, dimension: ViewDimension },
    SetViewEdgeProjectionOverride { view_id: GraphViewId, selectors: Option<Vec<RelationSelector>> },
}

pub enum CoreSessionIntent {
    Undo,
    Redo,
    SetSelectedFrame { frame_name: Option<String> },
    PromoteNodeToActive { key: NodeKey, cause: LifecycleCause },
    DemoteNodeToWarm { key: NodeKey, cause: LifecycleCause },
    DemoteNodeToCold { key: NodeKey, cause: LifecycleCause },
    SetWorkbenchEdgeProjection { selectors: Vec<RelationSelector> },
}

pub enum CoreSyncIntent {
    ApplyRemoteDelta { entries: Vec<u8> },
    TrustPeer { peer_id: String, display_name: String },
    GrantWorkspaceAccess { peer_id: String, workspace_id: String },
    ForgetDevice { peer_id: String },
    RevokeWorkspaceAccess { peer_id: String, workspace_id: String },
}
```

This is intentionally a **starting surface**, not a frozen final enum. The
guiding rule is that `CoreIntent` owns durable graph/workspace mutations and
portable session mutations, but excludes host-only orchestration.

### 2.3b What Stays Out of `CoreIntent`

The following families remain host-side and translate into `CoreIntent` or
other host actions:

- shell and chrome toggles:
  - `ToggleHelpPanel`
  - `ToggleCommandPalette`
  - `ToggleRadialMenu`
- graph-canvas runtime/view commands:
  - `TogglePhysics`
  - `ToggleGhostNodes`
  - `ToggleCameraPositionFitLock`
  - `ToggleCameraZoomFitLock`
  - `RequestFitToScreen`
  - `RequestZoomIn`
  - `RequestZoomOut`
  - `RequestZoomReset`
  - `RequestZoomToSelected`
  - `RequestZoomToGraphlet`
  - `ReheatPhysics`
- workbench / pane / graph-tree orchestration:
  - `CreateGraphViewSlot`
  - `RouteGraphViewToWorkbench`
  - `OpenNodeFrameRouted`
  - `OpenNodeWorkspaceRouted`
  - `SetPanePresentationMode`
  - `PromoteEphemeralPane`
  - `RestorePaneToSemanticTabGroup`
  - `CollapseSemanticTabGroupToPaneRest`
- browser/webview runtime bookkeeping:
  - `AcceptHostOpenRequest`
  - `MapWebviewToNode`
  - `UnmapWebview`
  - `WebViewCreated`
  - `WebViewUrlChanged`
  - `WebViewHistoryChanged`
  - `WebViewScrollChanged`
  - `WebViewTitleChanged`
  - `WebViewCrashed`
- history-preview UI/runtime controls:
  - `EnterHistoryTimelinePreview`
  - `ExitHistoryTimelinePreview`
  - replay progress/reporting intents
- host service controls:
  - capsule server start/stop intents
  - mod activation/load reporting
  - memory pressure reporting

These are real application intents, but they are not the portable durable kernel
surface.

### 2.3c Translation Rule

The host keeps its broad orchestration enum, but mutation-bearing paths must
become explicit translations:

- one host `GraphIntent` may map to zero, one, or many `CoreIntent` values
- pure host/UI intents may map to no core intents at all
- no host phase may mutate graph truth directly once the reducer split lands
- persistence/WAL serialization is defined over `CoreIntent` and other
  core-owned persisted types, not over the entire host orchestration enum

### 2.4 `GraphWorkspace`

`GraphWorkspace` belongs in core. It is the pure graph/session state container
that the host owns, not a host state bag.

Core-owned `GraphWorkspace` contents include:

- graph truth
- semantic tag state that is intentionally portable
- graph-side caches derived from graph truth
- graph-view session state that is deliberately snapshotable and portable
- reducer-local dirty flags and replay state

The host owns:

- shell panes, tabs, window state, browser/webview maps
- render adapters
- workbench / `graph-tree` projection state
- graph canvas / `graph-canvas` runtime state
- command palette, panels, ephemeral UI toggles

### 2.5 Domain Event Boundary

Core emits portable domain events. The host reacts to them. The current shell
`GraphSemanticEvent` name collision must be resolved before the core event type
lands.

Core owns:

- domain `GraphSemanticEvent` (or the final chosen name)

The host owns:

- webview/browser lifecycle event buses
- UX notifications and effect execution

### 2.6 Node Identity and Address

These types must be identical across platforms:

- `NodeId` (`Uuid`)
- `Address`
- `AddressKind`
- `HistoryEntry`
- `viewer_override: Option<ViewerId or String>`
- URL normalization used for identity, deduplication, and publication

`Address::File` and `Address::Directory` are valid data inside core, but are
never resolved by core. Opening files is always a host concern.

### 2.7 Persistence Schema

Every host that reads or writes graph state must use the same persisted types.

Core owns:

- `GraphSnapshot`
- persisted node / edge types
- WAL log entry types
- schema versioning and compatibility helpers
- replay helpers used to rebuild `GraphWorkspace`

The host owns:

- concrete storage adapters
- file/database locations
- sync transport

### 2.8 Session Authority (Coop)

This remains in the plan as a follow-on step after the base kernel lands, but it
still belongs in `graphshell-core`:

- `CoopSessionId`
- role enum
- contribution / approval types
- Coop snapshot contract
- ephemeral presence/cursor signal payloads

### 2.9 Publication Schema

Also a follow-on core lane:

- NIP-84 payload structs
- clip-content publication schema
- normalization rules shared with identity

### 2.10 UDC Semantic Tagging (Partial)

Core owns only the portable semantic substrate:

- parsed/portable semantic code representation
- graph-owned semantic tag state
- durable tag/untag reducer inputs
- dirty flags and compact graph-side semantic facts

Host-only:

- `KnowledgeRegistry`
- threaded fuzzy-search machinery
- heavy datasets and reconciliation workers

### 2.11 Layout and Physics

Core will eventually own:

- a portable position type (or retain `euclid::Point2D<f32>` if that remains the
  best portable representation)
- topology classification
- headless layout/physics math

This is a later step, not a prerequisite for the initial kernel extraction.

---

## 3. What Does Not Belong in Core

| Component | Why it stays out |
| --- | --- |
| `egui::*`, `wgpu::*`, Servo, webview host types | platform/UI/render dependencies |
| `egui_graphs` state and adapters | framework-specific render/layout bridge |
| workbench layout and pane management | belongs to host or sibling portable crate (`graph-tree`) |
| graph view / canvas runtime and rendering | belongs to host or sibling portable crate (`graph-canvas`) |
| compositor adapters and shell layout passes | host composition concerns |
| `GraphBrowserApp` | mixes UI state, shell state, registries, runtime maps |
| Nostr signing, relay pool, iroh/libp2p transport | host/network concerns |
| fjall / IndexedDB / SQLite implementations | storage adapters, not schema |
| JS/Swift/Kotlin binding annotations | belong in wrapper crates |
| shell lifecycle event buses | not the same as domain mutation events |
| heavy semantic reconciliation systems (`KnowledgeRegistry`, `nucleo`) | host threading / dataset concerns |

Portable sibling crates:

- `graph-tree`: portable workbench/navigator tree projection over graph truth
- `graph-canvas`: portable graph-view/canvas subsystem and render packet layer

These crates may consume `graphshell-core`, but they do not belong inside it.

---

## 4. Target Crate Layout

The precise module split may evolve, but the target shape is:

```text
graphshell-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── types.rs
    ├── address.rs
    ├── event.rs
    ├── intent.rs
    ├── workspace.rs
    ├── graph/
    │   ├── mod.rs
    │   ├── apply.rs
    │   ├── filter.rs
    │   └── facet_projection.rs
    ├── persistence/
    │   ├── mod.rs
    │   ├── snapshot.rs
    │   ├── wal.rs
    │   └── compat.rs
    ├── coop/              # Step 5
    ├── publication/       # Step 6
    └── physics/           # Step 7+
```

Wrapper crates:

```text
graphshell-core-wasm/
graphshell-core-uniffi/
```

### 4.1 Host-Shim Migration Pattern

The extraction will use **temporary host re-export shims** to keep the host
compiling while consumers are moved.

Allowed temporary pattern:

- move the canonical type/module into `graphshell-core`
- re-export it from the old host path
- migrate downstream imports incrementally
- remove shims before Step 4 is marked complete

Not allowed:

- long-lived duplicated truth
- copying logic into both host and core
- leaving the reducer or workspace owned by both crates

### 4.2 Visibility Discipline

Crossing a crate boundary changes the meaning of `pub(crate)`. Extraction must
include an explicit visibility audit:

- promote only the API the host truly needs
- keep reducer internals crate-private
- prefer narrow public façades over exposing arbitrary mutation helpers
- document trust boundaries where replay or persistence reconstruction requires
  privileged entry points

---

## 5. Portability Rules and Mechanical Gates

### 5.1 Per-Phase WASM Gate

After `graphshell-core` exists, every extraction subphase must pass:

```text
cargo check -p graphshell-core --target wasm32-unknown-unknown
```

This is not an end-of-project check. It is a continuous gate.

### 5.2 Host Generates IDs

Core never calls `Uuid::new_v4()`. Hosts generate IDs and pass them into core.
This applies to:

- `NodeId`
- `CoopSessionId`
- any future durable portable ID

### 5.3 No Platform I/O in Core

Core may parse and normalize addresses, but it never:

- opens files
- reads browser APIs
- talks to the network
- reaches into host storage

### 5.4 No Binding Annotations in Core

No `#[wasm_bindgen]`, no UniFFI annotations, no JS- or mobile-binding
attributes inside `graphshell-core`.

### 5.5 Single-Writer / No Thread Assumptions

Core must not require multi-threading. Hosts may choose multi-threaded
execution, but the core contract is single-writer safe and portable.

### 5.6 `Address::File` and `Address::Directory`

These are valid persisted data, but not portable open operations:

- desktop hosts may resolve them
- mobile hosts treat them as display-only unless a future sandboxed variant is
  introduced
- browser hosts do not resolve them

### 5.7 Dependency Discipline

Dependencies are allowed only if they are both:

1. portable to the target WASM gate
2. appropriate to the kernel boundary

Expected core dependencies include:

- `petgraph`
- `uuid`
- `serde`
- `serde_json`
- `rkyv`
- `url`
- `euclid`
- `time`

Dependencies such as `mime_guess` and `infer` are acceptable only if they stay
within pure metadata/address classification logic and remain WASM-clean.

---

## 6. Execution Strategy

### 6.1 Prerequisites and Current Status

| Item | Status | Plan impact |
| --- | --- | --- |
| Petgraph algorithm foundation | Done enough to proceed | graph algorithm surface no longer blocks extraction |
| UUID node identity migration | Landed | core can treat `NodeId` as stable identity |
| `Address` introduction | Landed enough to proceed | remaining mechanical cleanup can fold into Step 4 subphases |
| `GraphPos2` introduction | No longer a prerequisite | graph model already uses portable `euclid::Point2D<f32>`; position-type cleanup moves to physics step |
| shell `GraphSemanticEvent` rename | Required | must land before core event boundary lands |
| `graph-tree` extraction | Landed | validates sibling portable crates and sharpens `graphshell-core` boundary |

### 6.1a Primary Source Files for Step 4

The base extraction work will center on these existing files/modules:

- `model/graph/mod.rs`
- `model/graph/apply.rs`
- `model/graph/filter.rs`
- `model/graph/facet_projection.rs`
- `model/graph/badge.rs` (partial carve-out only, not a whole-file move)
- `services/persistence/types.rs`
- `app/intents.rs`
- `app/intent_phases.rs`
- `app/workspace_state.rs`
- `shell/desktop/host/window.rs` and related shell lifecycle files for the
  `GraphSemanticEvent` rename

### 6.2 Step 4 Done-Definition

Step 4 is complete **only when all of the following are true**:

1. `crates/graphshell-core/` exists and passes the WASM gate.
2. The core graph model and graph-side leaf types live in `graphshell-core`.
3. Snapshot and WAL types live in `graphshell-core`.
4. Core owns the durable mutation boundary through a portable reducer API.
5. `GraphWorkspace` lives in `graphshell-core`.
6. The domain event boundary lives in `graphshell-core`.
7. The host no longer owns duplicate graph truth, reducer authority, or
   workspace state.
8. Temporary re-export shims are either removed or clearly transitional and
   scheduled for removal immediately after extraction.

A graph-model-only move does **not** satisfy Step 4.

### 6.3 Step 4a — Namespace Cleanup and Crate Scaffold

**Goal**: Clear naming conflicts and create the extraction target with CI gates
before moving behavior.

Key work:

- rename the shell lifecycle/event type currently named `GraphSemanticEvent`
  to a host-specific name such as `WebViewLifecycleEvent`
- create `crates/graphshell-core/`
- add the new crate to the workspace
- add the WASM gate to CI immediately, even if the crate is initially sparse
- add an import/dependency audit note for forbidden crates in core

Done gate:

- the shell no longer defines a host type called `GraphSemanticEvent`
- `cargo check -p graphshell-core`
- `cargo check -p graphshell-core --target wasm32-unknown-unknown`

### 6.4 Step 4b — Dependency Untangling and Leaf Carve-Outs

**Goal**: Remove the small but important tangles that would otherwise turn the
main move into a circular-import mess.

Key work:

- split portable leaf types out of oversized host modules before moving them
- specifically carve `NodeTagPresentationState` out of `model/graph/badge.rs`
  instead of treating all of `badge.rs` as a portable module
- extract graph-side leaf enums/structs such as:
  - classification types
  - import record types
  - frame layout hint types
- make persistence leaf types depend on core-owned leaf types rather than
  importing through host `crate::graph::*`

Done gate:

- persistence leaf modules no longer import graph leaf types from host paths
- no file is moved wholesale merely because part of it is portable
- the host compiles via temporary re-exports where needed

### 6.5 Step 4c — Address and Identity Boundary

**Goal**: Land the portable identity/address substrate early and cleanly.

Key work:

- move `Address`, `AddressKind`, normalization helpers, MIME/address-class
  helpers, and portable viewer-override support into core
- keep file resolution and host-specific open behavior out of core
- move portable history/address persistence helpers that naturally belong with
  identity

Done gate:

- all address parsing and normalization for graph truth comes from core
- no file/network/browser access appears in the address module
- WASM gate passes after the move

### 6.6 Step 4d — Graph Model, Algorithms, and Snapshot Types

**Goal**: Move the graph model, algorithm surface, and snapshot-facing graph
types into core as one coherent unit.

Key work:

- move the canonical graph model:
  - `Graph`
  - `Node`
  - `NodeKey`
  - `EdgePayload`
  - graph-side lifecycle/state enums
- move graph helpers and submodules that are already pure:
  - `apply.rs`
  - `filter.rs`
  - `facet_projection.rs`
- move `GraphDelta` / batch mutation helpers used by replay and reducer internals
- move snapshot-facing persisted graph types together with the graph model so
  `from_snapshot` / `to_snapshot` do not remain split-brained across crates
- migrate graph tests into `graphshell-core/tests/`

Done gate:

- the host graph module becomes a thin compatibility façade plus host-only
  modules such as egui adapters and style registries
- graph algorithm tests pass from the core crate
- snapshot round-trip tests pass from the core crate

### 6.7 Step 4e — Durable Mutation Boundary Split and Reducer Extraction

**Goal**: Move mutation authority into core without dragging host-only intent
types across the boundary.

Key work:

- inventory the current host `GraphIntent` variants into categories:
  - durable portable graph mutations
  - portable graph-view/session mutations
  - host orchestration / workbench / shell / runtime actions
  - pure ephemeral UI actions
- define `CoreIntent` as the portable durable mutation surface
- move reducer logic into core as `apply_core_intents()` or equivalent
- align core intent serialization with WAL serialization
- add translation from host-level intents into one or more core intents
- keep host orchestration enums in the host crate

Important rule:

- do **not** pull host-only types into core just to preserve the current enum
  name

Done gate:

- every durable mutation path goes through the core reducer
- the core intent enum contains no `PaneId`, renderer IDs, shell tiles, `Instant`,
  host open requests, or similar host types
- host-only intent phases become translation/orchestration layers rather than
  the mutation authority

### 6.8 Step 4f — `GraphWorkspace` and Domain Event Extraction

**Goal**: Move the pure state container and domain event surface into core.

Key work:

- extract `GraphWorkspace` from host application state
- move portable graph/session caches and semantic state into `GraphWorkspace`
- define the core domain event surface
- keep shell lifecycle buses and UX effects out of core

Done gate:

- the host owns a `GraphWorkspace` value from core instead of defining it
  locally
- the host app state contains only host concerns plus a core workspace field
- the only domain event crossing from core to host is the core event type

### 6.9 Step 4g — WAL Schema, Replay, and Persistence Boundary

**Goal**: Move the durable persistence schema into core and prove replay works
for non-desktop hosts.

Key work:

- move WAL entry types into `graphshell-core`
- move schema versioning and compatibility helpers into core
- add replay helpers that rebuild `GraphWorkspace` from WAL + snapshots
- add benchmarks or regression tests for replay performance
- add UUID round-trip tests across native and WASM serialization paths

Extension-host acceptance requirement:

- replaying a reference WAL into a usable workspace must fit within a
  service-worker-friendly budget; the old 200ms / 1,000 entries target remains
  the working guardrail until a real benchmark suite replaces it

Done gate:

- the host storage layer writes and reads core-owned persisted types
- replay tests pass against core APIs
- WAL schema no longer lives in the host crate

### 6.10 Step 4h — Host Integration, Shim Collapse, and API Hardening

**Goal**: Finish the authority migration instead of leaving core as a mirrored
parallel model.

Key work:

- fix downstream imports and direct consumers
- remove or sharply reduce compatibility shims
- complete the visibility audit (`pub(crate)` vs. `pub`)
- document trust-boundary APIs used by replay or restoration
- verify full workspace compilation and runtime parity

Done gate:

- no duplicate reducer authority remains in the host
- no duplicate `GraphWorkspace` ownership remains in the host
- core owns the canonical graph model, reducer, workspace, and persistence
  schema
- full workspace tests pass
- Step 4 done-definition is satisfied in full

### 6.11 Step 5 — Coop Authority in Core

After the base kernel lands, move Coop types and approval rules into core:

- `CoopSessionId`
- role enum
- contribution / approval types
- Coop snapshot contract
- ephemeral presence payload types

### 6.12 Step 6 — Publication Schema in Core

Move portable publication schema into core:

- NIP-84 payload structs
- clip publication schema
- publication-side normalization helpers

Signing, keys, relay I/O, and publication scheduling remain host concerns.

### 6.13 Step 7 — Headless Physics and Position-Type Cleanup

This step now explicitly sits **after** the base extraction. The graph model is
already portable enough to extract with `euclid::Point2D<f32>`.

Use Step 7 to decide whether to:

- keep `euclid::Point2D<f32>` as the core position type, or
- introduce `GraphPos2` as a semantic newtype when physics/layout math moves

Move only the portable math:

- topology classifier
- warm-start positioning
- headless `step()` function

Keep view adapters in host or sibling crates.

### 6.14 Step 8 — Wrapper Crates and Host Envelope Hardening

Once the core API stabilizes:

- add `graphshell-core-wasm`
- add `graphshell-core-uniffi`
- specify the JS API surface for browser hosts
- specify the Swift/Kotlin surface for mobile hosts

These wrappers are adapters, not the kernel itself.

---

## 7. Host Envelope Notes

### 7.1 Browser Extension / Browser Tab / PWA Hosts

Browser-hosted Graphshell has two important truths:

1. `graphshell-core` can be consumed as WASM.
2. the browser itself is the execution substrate for browser-side JS and
   WebAssembly capabilities.

Implications:

- browser hosts use `graphshell-core-wasm`
- the background/service-worker host owns persistence and message routing
- content scripts do DOM capture and message the background host
- browser hosts can run guest WebAssembly through the browser runtime even where
  native-style runtimes such as Wasmtime are unavailable
- core mutation payloads should have a clean JSON serialization surface because
  that naturally crosses the JS/WASM boundary and aligns with WAL/event formats

### 7.2 Extension Service-Worker Constraint

The MV3-style worker model still matters:

- the in-memory workspace may be ephemeral
- WAL replay must be fast enough to restore state before handling durable work
- writes must be durable before acknowledging mutation completion

This is why replay helpers and persistence schema belong in core, not only in
the desktop host.

### 7.3 Mobile Hosts

Mobile hosts consume the same kernel through `graphshell-core-uniffi`.

Constraints:

- single-writer core contract still applies
- mobile storage is an adapter over the same WAL/snapshot schema
- `Address::File` / `Address::Directory` are display-only unless a future
  sandboxed relative-path variant is introduced

---

## 8. Acceptance Criteria

### 8.1 Step 4 Completion

1. `cargo check -p graphshell-core --target wasm32-unknown-unknown` passes.
2. No `egui`, `wgpu`, Servo, fjall, iroh, libp2p, or host binding annotations
   appear in `graphshell-core`.
3. Core owns the canonical graph model.
4. Core owns the durable mutation boundary and reducer.
5. Core owns `GraphWorkspace`.
6. Core owns the domain event boundary.
7. Core owns snapshot and WAL schema types.
8. The host compiles and runs by depending on core rather than duplicating the
   same logic.

### 8.2 Intent / Reducer Boundary

1. Every durable mutation path flows through the core reducer.
2. Core reducer inputs serialize without host-only types.
3. Host orchestration enums translate into core intents instead of mutating the
   graph directly.

### 8.3 Persistence and Replay

1. Snapshot round-trip tests pass from `graphshell-core`.
2. WAL round-trip tests pass from `graphshell-core`.
3. UUID values round-trip across JS/WASM/native serialization paths.
4. Replay into a fresh workspace is benchmarked and kept within the extension
   host budget.

### 8.4 Coop Authority (Step 5)

1. Coop approval rules are enforced from core types/state machines.
2. Ephemeral presence/cursor signals remain outside WAL persistence.

### 8.5 Publication Schema (Step 6)

1. A clip/publication payload can be constructed from core data alone.
2. URL normalization behaves identically across native and WASM builds.

### 8.6 CI Gates

At minimum:

- `cargo check`
- `cargo test`
- `cargo check -p graphshell-core --target wasm32-unknown-unknown`

Later:

- `wasm-pack build graphshell-core-wasm --target bundler`
- mobile wrapper build gates once those crates exist

---

## 9. Open Questions

1. **Position type**: keep `euclid::Point2D<f32>` in core, or introduce a
   semantic `GraphPos2` newtype during the physics step?
2. **Snapshot/WAL versioning policy**: how explicit should compatibility
   migrations be across staggered desktop / extension / mobile release cadences?
3. **Relay event format for non-clip WAL replay**: what application-level Nostr
   event shape should carry generic durable mutations?
4. **Wrapper-crate API shape**: what is the minimum stable JS / Swift / Kotlin
   surface we want before extension/mobile hosts land?

---

*This document is the authoritative design and execution reference for
`graphshell-core` extraction. Update it as subphases land, prerequisites shift,
or the durable mutation boundary becomes better understood in code.*
