<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `graphshell-core` Extraction Plan

**Date**: 2026-03-08
**Status**: Design / Planning
**Scope**: Extract the identity, authority, and mutation kernel of graphshell into a WASM-clean
crate (`graphshell-core`) that compiles to `wasm32-unknown-unknown` with zero errors and has no
knowledge of egui, wgpu, Servo, or any platform I/O. This crate is the shared foundation for the
desktop app, iOS/Android apps, browser extensions (Firefox/Chrome), and Verse server-side nodes.

**Related docs**:

- [`canvas/petgraph_algorithm_utilization_spec.md`](../implementation_strategy/canvas/petgraph_algorithm_utilization_spec.md) — petgraph algorithm surface
- [`canvas/2026-02-24_physics_engine_extensibility_plan.md`](../implementation_strategy/canvas/2026-02-24_physics_engine_extensibility_plan.md) — current physics extensibility architecture
- [`canvas/2026-02-23_udc_semantic_tagging_plan.md`](../implementation_strategy/canvas/2026-02-23_udc_semantic_tagging_plan.md) — UDC semantic tagging
- [`2026-02-18_universal_node_content_model.md`](2026-02-18_universal_node_content_model.md) — node identity / `Address` enum
- [`2026-03-18_event_log_fact_store_query_architecture.md`](../../archive_docs/checkpoint_2026-03-18/2026-03-18_event_log_fact_store_query_architecture.md) — portable event-log / fact-store / query split (archived: implemented 2026-03-18)
- [`system/coop_session_spec.md`](../implementation_strategy/system/coop_session_spec.md) — Coop session authority (§3, §6, §15, §16)
- [`viewer/2026-02-11_clipping_dom_extraction_plan.md`](../implementation_strategy/viewer/2026-02-11_clipping_dom_extraction_plan.md) — clip publication (NIP-84)

---

## 1. Purpose and Principle

`graphshell-core` is not a graph library. It is the **identity, authority, and mutation kernel**
of the graphshell system — the minimal set of logic that must be identical across all deployments
or the system is incoherent.

The test for any candidate component: *if two platforms disagree about this, can the system still
function coherently?* If the answer is no, it belongs in core.

**Target deployment contexts**:

| Context | How core is used |
| --- | --- |
| Desktop app (Linux/macOS/Windows) | Native dependency; host adds egui, wgpu, Servo, iroh |
| iOS / Android app | Native dependency via UniFFI or `cdylib`; host adds platform UI |
| Firefox / Chrome extension | Compiled to WASM; host adds browser DOM APIs |
| Browser tab (WASM) | Compiled to WASM; host adds web UI framework |
| Verse server-side node | Native or WASM; host adds libp2p/iroh networking |
| Headless test harness | Native; no host UI at all |

The WASM compilation constraint is the mechanical enforcement mechanism. If `graphshell-core`
compiles to `wasm32-unknown-unknown` with zero errors, it is definitionally free of platform
dependencies. This is better than any code review.

---

## 2. What Belongs in Core

### 2.1 Graph Domain State and Mutations

The graph is the primary shared state. Every platform that touches the graph must use the same
types, the same identity system, and the same reducer. Divergence here means sync is impossible.

| Component | Current location | Notes |
| --- | --- | --- |
| `Graph` (petgraph-backed topology) | `model/graph/mod.rs` | All petgraph algorithm accessors live here (§5) |
| `NodeKey` / `EdgeKey` UUID identity | `model/graph/mod.rs` | `uuid` crate is WASM-clean |
| `Node`, `EdgePayload` data types | `model/graph/mod.rs` | Pure data; no render state |
| `GraphWorkspace` | `graph_app.rs` | State container; no egui types (see §2.7) |
| `GraphIntent` enum + `apply_intents()` | intent system | The single authority for all durable mutations |
| `GraphSemanticEvent` | event boundary | The only type that crosses from core to host |

### 2.2 Node Identity and Address

A browser extension clipping a page must store `Address::Http(url)` in exactly the same format
the desktop app reads from its snapshot. A mobile app receiving a synced node must interpret its
address correctly. These types must be identical across platforms.

| Component | Notes |
| --- | --- |
| `NodeId` (UUID) | Stable across sync, Coop sessions, NIP-84 publication |
| `Address` enum | `Http(Url)`, `File(PathBuf)`, `Onion`, `Ipfs(Cid)`, `Gemini`, `Custom` |
| `HistoryEntry` | Append-only navigation log per node; WAL-replayed across platforms |
| `RendererKind` hint enum | Names which renderer a node prefers; not the renderer itself |
| URL normalization | Shared by NIP-84 `r` tag, node deduplication, Coop contribution routing |

`Address::File` and `Address::Directory` compile on WASM (PathBuf is in std) but are never
resolved inside core. Resolution is a host-only concern. See §7.2.

### 2.3 Session Authority (Coop)

A mobile Coop guest must enforce the same contribution approval rules as the desktop host. A
browser extension participating in a session must produce the same snapshot format. If authority
logic is platform-specific, security guarantees are meaningless.

| Component | Notes |
| --- | --- |
| `CoopSessionId(Uuid)` | Session identity independent of `GraphViewId` |
| Role enum | `Host`, `Guest(ViewOnly)`, `Guest(Contributor)`, `Guest(Editor)` |
| Authority rules | Host owns domain policy; approval workflow logic |
| `CoopContribution` type | Proposed mutation from guest; includes approval state machine |
| Snapshot contract | What `TakeCoopSnapshot` produces; cross-platform serialization |
| Ephemeral signal types | `CursorPosition`, `PresenceHeartbeat` — defined in core as data, never persisted |

What stays in the host: cursor rendering, presence UI, command palette role-filtering, approval
dialog UX. Core defines the authority rules; platforms define how to surface them.

### 2.4 Publication Schema (NIP-84 / Clips)

A browser extension clipping a page and the desktop app must produce the same NIP-84 `kind 9802`
event structure. The wire format is the shared contract. Signing and network I/O are platform
concerns.

| Component | Notes |
| --- | --- |
| NIP-84 `kind 9802` event struct | Content, `r` tag (canonical URL), `context` tag |
| Clip node type | DOM-extracted content node; same schema on all platforms |
| `nostr_event_id` metadata field | Set after publication; stored for deduplication and link-back |
| URL normalization (canonical form) | Strips UTM/tracking params; shared with `Address::Http` deduplication |

What stays in the host: Nostr signing (keypair is platform-specific — NIP-46 on desktop, browser
`window.nostr`, iOS Secure Enclave), relay pool, network I/O.

### 2.5 Persistence Schema

Every platform that reads or writes graph state must use the same WAL log entry types and snapshot
serialization format. If these are host-crate types, cross-platform sync is impossible.

| Component | Notes |
| --- | --- |
| WAL log entry types | `AddNode`, `NavigateNode`, `AddEdge`, `TagNode`, `TakeCoopSnapshot`, etc. |
| Snapshot serialization format | The format that travels over iroh-docs for Device Sync |
| `GraphDelta` / batch mutation type | Used for atomic snapshot application and WAL replay |

What stays in the host: fjall storage (OS filesystem), iroh-docs sync transport, platform-specific
snapshot storage locations.

### 2.6 UDC Semantic Tagging (Partial)

| Component | Core? | Notes |
| --- | --- | --- |
| `TagNode` / `UntagNode` intent variants | Yes | Graph mutations; must flow through reducer |
| `semantic_tags: HashMap<NodeKey, HashSet<String>>` | Yes | Graph state; moves onto `GraphWorkspace` |
| `semantic_index_dirty: bool` | Yes | Graph state flag; drives host reconciliation |
| `CompactCode` (parsed UDC representation) | Yes | Mobile + extension must agree on encoding |
| `KnowledgeRegistry`, `reconcile_semantics` | No | `nucleo` uses threads; host-only |
| UDC dataset, fuzzy search | No | Heavy; host-only |

`semantic_tags` is currently held on `GraphBrowserApp`. It moves to `GraphWorkspace` in core
when this crate is extracted. The host holds the `GraphWorkspace` and passes it to
`reconcile_semantics` as before.

### 2.7 Layout: Position Type and Physics Engine

| Component | Notes |
| --- | --- |
| `GraphPos2 { x: f32, y: f32 }` | WASM-clean position newtype; replaces `egui::Pos2` everywhere in core |
| `LayoutHint` enum | Topology-driven layout selection hint |
| Topology classifier | Uses petgraph; returns `LayoutHint`; lives alongside `Graph` |
| MST warm seed pre-pass | petgraph MST + radial layout → initial `GraphPos2` positions |
| Component locus pre-pass | Kosaraju → per-component gravity loci; inputs for `ComponentGravityLoci` force |
| **Headless physics engine** | `step(nodes, edges, params, dt) -> f32`; pure math (§6) |

`From<GraphPos2> for egui::Pos2` and the inverse live in the host crate only. Core never imports
egui.

`PhysicsProfile.apply_to_state()` stays in the host — it depends on `egui_graphs` state types.
The physics computation (`step()`) moves to core; the rendering adapter (`egui_graphs`) stays in
the host.

### 2.8 Petgraph Algorithm Surface

All algorithm accessors on `Graph` (specified in `petgraph_algorithm_utilization_spec.md §4.1`)
live in core. petgraph itself is WASM-clean. Summary:

```rust
impl Graph {
    pub fn neighbors_undirected(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_;
    pub fn hop_distances_from(&self, source: NodeKey) -> HashMap<NodeKey, usize>;
    pub fn orphan_node_keys(&self) -> Vec<NodeKey>;
    pub fn shortest_path(&self, from: NodeKey, to: NodeKey) -> Option<Vec<NodeKey>>;
    pub fn is_reachable(&self, from: NodeKey, to: NodeKey) -> bool;
    pub fn weakly_connected_components(&self) -> Vec<Vec<NodeKey>>;
    pub fn strongly_connected_components(&self) -> Vec<Vec<NodeKey>>;
    pub fn condensation_dag(&self) -> petgraph::Graph<Vec<NodeKey>, ()>;
    pub fn toposort(&self) -> Result<Vec<NodeKey>, NodeKey>;
    pub fn min_spanning_tree_positions(&self) -> Vec<(NodeKey, GraphPos2)>;
    pub fn classify_topology(&self) -> LayoutHint;
}
```

`hop_distance_cache` and `component_membership_cache` (with `GraphPos2` loci) live on
`GraphWorkspace` in core, invalidated on structural graph changes.

---

## 3. What Does Not Belong in Core

| Component | Reason |
| --- | --- |
| Any `egui::*` type | Breaks WASM gate |
| `egui_graphs` physics state/layout types | Not WASM-clean |
| `PhysicsProfile.apply_to_state()` | Depends on egui_graphs |
| `ContentRenderer`, `ProtocolResolver` traits | Reference `egui::Ui`, OS filesystem, platform I/O |
| `TileRenderMode`, `CompositorAdapter` | Render-pipeline concerns; egui/GL specific |
| Tile compositor, workbench layout | egui-specific; each platform has its own layout system |
| `KnowledgeRegistry`, `reconcile_semantics` | `nucleo` uses threads; heavy; host-only |
| iroh, libp2p, Nostr transport crates | Network I/O; each platform uses its own transport stack |
| Nostr signing (`nsec`/`npub` operations) | Platform-specific (NIP-46, browser extension, Secure Enclave) |
| fjall storage | OS filesystem |
| Servo / webview lifecycle | Host crate; not present on mobile or extension |
| `GraphBrowserApp` application state | UI state, webview maps — host only |

---

## 4. Crate Boundary Summary

| Category | In core | In host |
| --- | --- | --- |
| Graph topology + identity | `Graph`, `NodeKey`, `Node`, `EdgePayload` | — |
| Mutations | `GraphIntent`, `apply_intents()` | — |
| Events | `GraphSemanticEvent` | — |
| State container | `GraphWorkspace` | `GraphBrowserApp` |
| Address / history | `Address`, `HistoryEntry`, `RendererKind` hint | `ContentRenderer`, `ProtocolResolver` |
| Node identity | `NodeId` (UUID), URL normalization | — |
| Session authority | `CoopSessionId`, role enum, approval state machine, snapshot contract | Cursor rendering, presence UX, command filtering |
| Publication schema | NIP-84 event struct, clip node type, `nostr_event_id` | Nostr signing, relay pool, network I/O |
| Persistence schema | WAL log entry types, snapshot serialization | fjall storage, iroh-docs transport |
| UDC | `CompactCode`, `semantic_tags`, `semantic_index_dirty`, `TagNode`/`UntagNode` | `KnowledgeRegistry`, `reconcile_semantics`, dataset |
| Layout | `GraphPos2`, `LayoutHint`, topology classifier, MST seed, physics `step()` | `PhysicsProfile.apply_to_state()`, egui_graphs |
| Algorithms | All petgraph accessors on `Graph` | — |

---

## 5. WASM Portability: Known Constraints

### 5.1 `Uuid::new_v4()` — never called inside core

UUID generation requires an RNG. On browser WASM this works via `crypto.getRandomValues()` (the
`"js"` feature on `getrandom`). On `wasm32-unknown-unknown` without a JS runtime (server-side
WASM, test harnesses), there is no RNG.

**Rule**: Core never calls `Uuid::new_v4()`. All `NodeId` and `CoopSessionId` values are
generated by the host and passed into `apply_intents()` as parameters. Core only stores and
compares UUIDs — never generates them.

### 5.2 `Address::File` — compiles, not resolved

`std::path::PathBuf` is available on `wasm32-unknown-unknown`. The variant compiles and is valid
data in snapshots and WAL entries. Resolving a `File` address (reading filesystem bytes) is a
host-only concern. Core never opens files.

### 5.3 Thread safety

`wasm32-unknown-unknown` is single-threaded by default. Core must not use `std::thread`,
`std::sync::Mutex` (prefer `RefCell` where interior mutability is needed in WASM contexts),
or any type that requires `Send + Sync` across threads. The physics `step()` function is
single-threaded and pure; no atomics or locks required.

### 5.4 Dependencies — WASM status

| Crate | WASM-clean? | Notes |
| --- | --- | --- |
| `petgraph` | Yes | No platform deps; confirmed |
| `uuid` | Yes with `"js"` feature | `"js"` for browser; `"getrandom"` with WASI for server |
| `url` | Yes | IDNA normalization is pure Rust |
| `serde` + `serde_json` | Yes | Used for snapshot/event serialization |
| `indexmap` | Yes | If used in graph internals |

---

## 6. Headless Physics Engine

### 6.1 Design Principles

- Pure math: no I/O, no platform callbacks, no egui dependency.
- Deterministic: same inputs → same outputs. No thread-local RNG inside `step()`.
- Topology-aware: `LayoutHint` from the classifier pre-pass drives cold-start positioning.
- Decoupled from petgraph: topology analysis is a pre-pass in the `Graph` layer; the physics
  engine receives positions and force params, outputs positions.

### 6.2 Core Interface

```rust
/// One node as seen by the physics engine.
/// The engine does not know about NodeKey, URLs, or graph semantics.
pub struct PhysicsNode {
    pub pos: GraphPos2,
    pub mass: f32,    // default 1.0; heavier nodes displace less
    pub pinned: bool, // displacement suppressed; forces still computed (for neighbors)
}

pub struct PhysicsParams {
    pub k: f32,                              // optimal edge-length constant (FR)
    pub temperature: f32,                    // annealing temperature (decreases each step)
    pub gravity: GravityMode,
    pub semantic_forces: Vec<SemanticForce>, // pre-computed UDC similarity pairs
}

pub enum GravityMode {
    Center { strength: f32 },
    ComponentLoci { loci: Vec<(usize, GraphPos2)>, strength: f32 },
    None,
}

/// UDC semantic attraction between two node indices (pre-computed by host KnowledgeRegistry).
pub struct SemanticForce {
    pub a: usize,
    pub b: usize,
    pub similarity: f32,  // [0.0, 1.0]
}

/// Step the simulation forward by `dt`.
/// Updates `nodes[*].pos` in place.
/// `edges` are index pairs into `nodes`.
/// Returns maximum node displacement (convergence signal for the host).
pub fn step(
    nodes: &mut [PhysicsNode],
    edges: &[(usize, usize)],
    params: &PhysicsParams,
    dt: f32,
) -> f32;

/// Compute topology-aware initial positions before the first step().
/// Only call when all nodes have pos == GraphPos2::ZERO (cold start or imported graph).
pub fn cold_start_positions(
    node_count: usize,
    edges: &[(usize, usize)],
    hint: LayoutHint,
    area: f32,
) -> Vec<GraphPos2>;
```

### 6.3 Algorithm Selection by `LayoutHint`

| `LayoutHint` | `cold_start_positions` | `step` force profile |
| --- | --- | --- |
| `ForceGeneral` | Random scatter in area circle | Standard FR: repulsion + attraction + gravity |
| `ForceTree` | BFS-level radial (root = highest-degree node) | FR with reduced repulsion, stronger edge attraction |
| `ForceBus` | Horizontal line; branches above/below | Spine-constraint force |
| `ForceRing` | Equally spaced on a circle | Circular constraint force |
| `ForceClique` | Small random cluster | Charge repulsion dominant; edge attraction suppressed |
| `ExplicitSeed` | No-op (caller sets positions) | Standard FR relaxation |

### 6.4 Topology Classifier

Lives on `Graph` in core (has petgraph access). Returns `LayoutHint` before the first physics
tick for a workspace.

```rust
impl Graph {
    pub fn classify_topology(&self) -> LayoutHint {
        let n = self.node_count();
        if n == 0 { return LayoutHint::ForceGeneral; }

        let max_degree = self.inner.node_indices()
            .map(|v| self.inner.edges(v).count()
                   + self.inner.edges_directed(v, Direction::Incoming).count())
            .max().unwrap_or(0);

        match petgraph::algo::toposort(&self.inner, None) {
            Ok(_) if max_degree <= 2 => LayoutHint::ForceBus,
            Ok(_)                    => LayoutHint::ForceTree,
            Err(_) => {
                let e = self.inner.edge_count();
                let clique_threshold = n * (n - 1) / 2;
                if e >= clique_threshold * 3 / 4 { LayoutHint::ForceClique }
                else if max_degree == 2           { LayoutHint::ForceRing   }
                else                              { LayoutHint::ForceGeneral }
            }
        }
    }
}
```

### 6.5 MST Warm Seed

When all node positions are zero (first load or imported graph with no committed positions),
the host calls `graph.min_spanning_tree_positions()` before the first `step()`:

1. petgraph Kruskal MST (edge weights = `1 / (1 + navigations)` — frequently co-visited nodes
   start close).
2. Radial tree layout on the MST: root = highest-degree node, children at equal angular intervals
   per depth level. Orphan nodes placed on an outer ring.
3. Positions written to `PhysicsNode::pos`.
4. `step()` called with `LayoutHint::ExplicitSeed` — relaxes from the MST seed, not random.

Guard: if any node has a non-zero committed position (user's spatial memory from a previous
session), skip the MST seed entirely.

### 6.6 Semantic Forces

`KnowledgeRegistry` (host) computes UDC prefix similarity after each `reconcile_semantics` call
and produces `Vec<SemanticForce>`. This is passed into `PhysicsParams` each frame. The engine
applies `F = similarity * (pos_B - pos_A) * k_semantic`. No UDC knowledge inside the engine —
similarity scores arrive as pre-computed `f32` values.

---

## 7. Module Layout in `graphshell-core`

```text
graphshell-core/
  src/
    lib.rs
    graph/
      mod.rs          — Graph, NodeKey, EdgeKey, Node, EdgePayload
      algorithms.rs   — petgraph accessor impls (hop_distances_from, shortest_path, etc.)
      topology.rs     — classify_topology(), LayoutHint
    intent.rs         — GraphIntent enum, apply_intents()
    event.rs          — GraphSemanticEvent
    workspace.rs      — GraphWorkspace (semantic_tags, caches, component_loci)
    address.rs        — Address, HistoryEntry, RendererKind, NodeId
    pos.rs            — GraphPos2
    url_normalize.rs  — canonical URL normalization (shared: NIP-84, deduplication)
    coop/
      mod.rs          — CoopSessionId, role enum, CoopContribution
      authority.rs    — approval state machine, policy enforcement
      snapshot.rs     — CoopSnapshot serialization contract
    publication/
      nip84.rs        — kind 9802 event struct, clip node type, nostr_event_id field
    persistence/
      wal.rs          — WAL log entry types
      snapshot.rs     — GraphSnapshot serialization
      delta.rs        — GraphDelta / batch mutation type
    udc/
      mod.rs          — CompactCode, semantic_tags types
    physics/
      mod.rs
      step.rs         — step(), cold_start_positions()
      node.rs         — PhysicsNode, PhysicsParams, GravityMode, SemanticForce
```

---

## 8. `Cargo.toml` and CI Gate

```toml
# graphshell-core/Cargo.toml
[package]
name = "graphshell-core"
edition = "2021"

[dependencies]
petgraph  = { version = "0.8", features = ["serde-1"] }
uuid      = { version = "1",   features = ["v4", "serde"] }
serde     = { version = "1",   features = ["derive"] }
serde_json = "1"
url       = "2"

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["js"] }   # browser WASM RNG
```

CI gate (required on every PR touching `graphshell-core`):

```yaml
- name: WASM compilation gate
  run: cargo build -p graphshell-core --target wasm32-unknown-unknown
```

Any import of egui, wgpu, Servo, iroh, libp2p, or any OS-dependent crate causes a compile error.
The gate is self-enforcing.

---

## 9. Sequencing and Prerequisites

Steps must be completed in order. Each is a prerequisite for the next.

### Step 0 — Petgraph algorithm PR sequence (active)

Complete petgraph spec PRs 1–5: hop-distance cache, `neighbors_undirected`, depth-2 connected
candidates, `Graph` accessor foundation, component membership cache + `ComponentGravityLoci`.
This cleans the `Graph` API boundary before extraction.

**Status**: Active. **Effort**: Medium total across 5 PRs.

### Step 1 — Introduce `GraphPos2`

Add `GraphPos2` as a newtype in the host crate. Replace `egui::Pos2` in `ComponentGravityParams`
and all locus-position fields. Add `From<>` conversions at the render boundary in the host.
No behavior change.

**Effort**: Small. **Risk**: Low.

### Step 2 — UUID node identity migration

Migrate `Node` identity from URL-based to UUID-based (`NodeId: Uuid`). Extend the fjall log with
`NavigateNode` replacing `UpdateNodeUrl`. Persistence migration required.

**Effort**: Large. **Risk**: High. **Gate**: After Step 0.

### Step 3 — `Address` enum introduction

Add `Address` enum in the host crate. Wire `Node::address: Address` replacing the current URL
field. `ContentRenderer::can_render()` hook is not required at this step.

**Effort**: Medium. **Gate**: After Step 2.

### Step 4 — Extract `graphshell-core` crate

Create the crate. Move all components from §2 into the module layout defined in §7. Host crate
depends on `graphshell-core` as a path dependency initially.

**Effort**: Large. **Gate**: After Steps 1–3.

### Step 5 — Coop authority and snapshot in core

Move `CoopSessionId`, role enum, approval state machine, and `CoopSnapshot` serialization into
`graphshell-core/src/coop/`. Verify that no host-side UI types leak into these modules.

**Effort**: Medium. **Gate**: After Step 4.

### Step 6 — Publication schema in core

Move NIP-84 event struct, clip node type, `nostr_event_id` field, and URL normalization into
`graphshell-core/src/publication/`. Signing and relay I/O remain in the host.

**Effort**: Small. **Gate**: After Step 4.

### Step 7 — Persistence schema in core

Move WAL log entry types, `GraphSnapshot`, and `GraphDelta` into
`graphshell-core/src/persistence/`. fjall storage and iroh-docs transport remain in the host.

**Effort**: Medium. **Gate**: After Step 4.

### Step 8 — Headless physics engine in core

Add `graphshell-core/src/physics/`. Wire host: replace `egui_graphs` FR computation with
`core::physics::step()`. `egui_graphs` remains as the rendering adapter; only the math moves.

**Effort**: Medium. **Gate**: After Step 4.

### Step 9 — Semantic forces wiring

After UDC Phase 2 (`SemanticGravity` force), replace the O(N²) pair loop with the pre-computed
`Vec<SemanticForce>` from `KnowledgeRegistry`, passed into `PhysicsParams` each frame.

**Effort**: Small. **Gate**: After Step 8 and UDC Phase 2.

---

## 10. Acceptance Criteria

### Core compilation (Steps 4+)

1. `cargo build -p graphshell-core --target wasm32-unknown-unknown` passes with zero errors.
2. No import of `egui`, `wgpu`, `servo`, `iroh`, `libp2p`, or any OS-dependent crate in core.
3. `apply_intents()` is in core; every `GraphIntent` variant is handled.
4. `GraphSemanticEvent` is the only type crossing from core to host at the domain event boundary.
5. `Uuid::new_v4()` is never called inside core — all IDs are passed in by the host.
6. `graphshell` (host crate) compiles and all existing tests pass after extraction.

### Physics engine (Step 8)

1. `step()` is deterministic: same inputs → same outputs; no thread-local RNG.
2. `classify_topology()` correctly identifies tree, ring, bus, clique, and general topologies
   against a suite of synthetic test graphs.
3. MST warm seed produces non-overlapping initial positions for a 20-node disconnected graph.

### Coop authority (Step 5)

1. `coop_workbench_intents_are_intercepted_before_reducer` — existing invariant test passes with
   authority types now in core.
2. `coop_contributor_mutation_requires_host_approval` — passes with approval state machine in core.
3. `coop_cursor_stream_does_not_touch_undo_or_wal` — ephemeral signal types defined in core
   have no WAL entry variant.

### Publication schema (Step 6)

1. A NIP-84 `kind 9802` event constructed from a clip node in core serializes to valid JSON
   matching the expected wire format.
2. URL normalization strips UTM parameters and normalizes trailing slashes consistently.

### CI gate

1. A CI job `cargo build -p graphshell-core --target wasm32-unknown-unknown` is present and
   required to pass on every PR touching `graphshell-core`.

---

## 11. Open Questions

1. **WASM target variant for Verse server-side**: `wasm32-unknown-unknown` (most restrictive) vs.
   `wasm32-wasi` (allows OS-like syscalls). The CI gate uses `wasm32-unknown-unknown`; Verse
   server nodes may target WASI for access to clocks and file I/O. Consider a second gate for
   `wasm32-wasi` once the Verse deployment target is confirmed.

2. **UniFFI / `cdylib` for iOS/Android**: iOS and Android apps will consume `graphshell-core`
   via a C ABI (`cdylib`) or UniFFI bindings. The module layout in §7 should be designed with
   UniFFI attribute placement in mind (`#[uniffi::export]` on public API surfaces). No action
   required before Step 4; track as a pre-mobile design review.

3. **`GraphWorkspace` split**: `GraphBrowserApp` currently holds both graph state and UI state.
   The extraction must cleanly separate them: `GraphWorkspace` in core owns pure graph state;
   `GraphBrowserApp` in the host owns everything else and holds a `GraphWorkspace`. Circular
   dependency risk must be reviewed at Step 4.

4. **Snapshot versioning**: WAL log entry types in core must be versioned for forward/backward
   compatibility between app versions and across platforms. A schema version field on
   `GraphSnapshot` is the minimum requirement. Design deferred to Step 7.

5. **`serde_json` vs. `postcard` for snapshot serialization**: `serde_json` is human-readable and
   debuggable but larger over the wire. `postcard` is compact and fast but not human-readable.
   For iroh-docs sync, compactness matters. Decision deferred to Step 7.

---

*This document is the authoritative design reference for `graphshell-core` extraction.
Update it as steps complete, prerequisites change, or deployment targets are confirmed.*
