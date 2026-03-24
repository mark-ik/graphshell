<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `graphshell-core` Extraction Plan

**Date**: 2026-03-08
**Updated**: 2026-03-23
**Status**: Design / Planning
**Scope**: Extract the identity, authority, and mutation kernel of graphshell into a WASM-clean
crate (`graphshell-core`) that compiles to `wasm32-unknown-unknown` with zero errors and has no
knowledge of egui, wgpu, Servo, or any platform I/O. This crate is the shared foundation for the
desktop app, iOS/Android apps, browser extensions (Firefox/Chrome), and Verse server-side nodes.

**2026-03-23 update**: Added §X (Extension Host Architecture), §Y (Mobile Host Architecture),
binding-framework wrapper crate design (§8a), WAL replay performance acceptance criterion (Step 7),
`GraphSemanticEvent` naming conflict prerequisite (Step 4), and `Address::File` mobile semantics
note (§5.5). See §11 for updated open questions.

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

| Context | How core is used | Binding layer | Sync path |
| --- | --- | --- | --- |
| Desktop app (Linux/macOS/Windows) | Native dependency; host adds egui, wgpu, Servo, iroh | Direct Rust dep | iroh-docs (QUIC) + Nostr relay |
| iOS / Android app | Native dependency; host adds platform UI | `graphshell-core-uniffi` (UniFFI / `cdylib`) | Nostr relay; iroh when available |
| Firefox / Chrome extension | Compiled to WASM; host adds browser DOM APIs | `graphshell-core-wasm` (wasm-bindgen) | Nostr relay only (see §X) |
| Browser tab (WASM) | Compiled to WASM; host adds web UI framework | `graphshell-core-wasm` (wasm-bindgen) | Nostr relay only |
| Verse server-side node | Native or WASM; host adds libp2p/iroh networking | Direct Rust dep or `wasm32-wasi` | iroh / libp2p |
| Headless test harness | Native; no host UI at all | Direct Rust dep | None |

The WASM compilation constraint is the mechanical enforcement mechanism. If `graphshell-core`
compiles to `wasm32-unknown-unknown` with zero errors, it is definitionally free of platform
dependencies. This is better than any code review.

`wasm-bindgen` and UniFFI annotations must **not** appear in `graphshell-core` itself — they live
in thin wrapper crates (`graphshell-core-wasm`, `graphshell-core-uniffi`) that re-export the core
public API with the appropriate binding attributes. See §8a.

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

### 5.5 `Address::File` — mobile sandbox semantics

On iOS and Android, filesystem access is sandboxed and app-specific. `PathBuf` compiles on both
WASM and mobile targets and is valid in WAL entries and snapshots. However, an absolute
`PathBuf` captured on desktop cannot be resolved on mobile: the path does not exist.

**Rule**: Mobile hosts treat `Address::File` and `Address::Directory` as display-only (show path
as label, do not attempt to open). A future `Address::AppSandboxFile { relative: PathBuf }`
variant may be introduced to carry paths portable within an app container — deferred until the
mobile host exists in code.

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

## 8a. Binding-Framework Wrapper Crates

`wasm-bindgen` and UniFFI cannot coexist in the same crate:

- `#[wasm_bindgen]` attributes emit JS glue code that breaks `cdylib` builds targeting iOS/Android.
- `#[uniffi::export]` attributes do not compile on `wasm32` targets.

Both frameworks are therefore excluded from `graphshell-core`. Two thin wrapper crates re-export
the core public API with the appropriate binding attributes:

```
graphshell-core/          ← zero bindgen, zero UniFFI; CI-gated on wasm32-unknown-unknown
graphshell-core-wasm/     ← wasm-bindgen exports; targets browser extensions and browser tabs
graphshell-core-uniffi/   ← UniFFI exports; targets iOS (Swift) and Android (Kotlin)
```

### `graphshell-core-wasm`

```toml
# graphshell-core-wasm/Cargo.toml
[package]
name = "graphshell-core-wasm"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
graphshell-core = { path = "../graphshell-core" }
wasm-bindgen    = "0.2"
serde-wasm-bindgen = "0.6"
```

Exports are thin `#[wasm_bindgen]` wrappers that accept/return JS-compatible types and delegate
entirely to `graphshell-core`. No logic lives here. Example:

```rust
#[wasm_bindgen]
pub fn apply_intent(state: &mut WasmWorkspace, intent_json: &str) -> Result<(), JsValue> {
    let intent: GraphIntent = serde_json::from_str(intent_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    state.inner.apply_intent(intent);
    Ok(())
}
```

Intents cross the WASM boundary as JSON strings — the same serialization format used by the WAL.
This means the extension JS host and the desktop Rust host share the same wire format for all
mutations, at no extra design cost.

CI gate for this crate:

```yaml
- name: wasm-bindgen build
  run: wasm-pack build graphshell-core-wasm --target bundler
```

### `graphshell-core-uniffi`

```toml
# graphshell-core-uniffi/Cargo.toml
[package]
name = "graphshell-core-uniffi"
edition = "2021"

[lib]
crate-type = ["cdylib", "staticlib"]

[dependencies]
graphshell-core = { path = "../graphshell-core" }
uniffi          = { version = "0.28", features = ["build"] }
```

UniFFI UDL or proc-macro annotations live here, not in `graphshell-core`. The binding surface is
designed for the Swift/Kotlin API ergonomics of the mobile host, not for the Rust API ergonomics
of `graphshell-core` itself. These two concerns are kept separate by design.

CI gate: a `cargo build --target aarch64-apple-ios` check is added when the iOS host crate exists.

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

**Prerequisite — `GraphSemanticEvent` naming conflict**: The current `GraphSemanticEvent` in
`shell/desktop/host/window.rs` is a Servo/webview lifecycle event bus (`UrlChanged`,
`WebViewCrashed`, `HistoryChanged`, `HostOpenRequest`). The plan's `GraphSemanticEvent` is a
domain event type that crosses from core to host. These are different types with the same name.
Before Step 4 lands, the desktop-shell type must be renamed (e.g. `ServoLifecycleEvent` or
`WebViewEvent`) to clear the namespace for the core domain event type. Doing this during Step 4
would create a conflict that blocks the host crate from compiling during the migration.

**Effort**: Large. **Gate**: After Steps 1–3 and the `GraphSemanticEvent` rename.

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

**Additional acceptance criterion (extension host)**: WAL replay — reconstructing a
`GraphWorkspace` by replaying a WAL log entry-by-entry — must complete in under 200ms for a
1,000-entry log on a reference device. This is a correctness requirement for the Chrome MV3
extension host, which must replay WAL from IndexedDB on every service worker activation before
handling any intent. A replay that exceeds the activation window produces a lost-update. A
benchmark test must be added alongside the schema migration.

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

## 10. Extension Host Architecture

This section describes how a Firefox/Chrome browser extension host consumes `graphshell-core-wasm`
and what constraints shape the extension-specific parts of the design. These constraints must be
reflected in the sequencing and acceptance criteria of Steps 4–8.

### 10.1 Execution contexts

A browser extension has two distinct JS execution contexts with different capabilities:

| Context | WASM | DOM access | Persistence | Notes |
| --- | --- | --- | --- | --- |
| Background service worker (MV3) / background page (MV2) | Yes | No | IndexedDB, `chrome.storage` | WASM core lives here |
| Content script (injected into pages) | No | Yes | None directly | Sends messages to background |

The WASM core (`graphshell-core-wasm`) loads and runs exclusively in the background worker.
Content scripts interact with the core only via the extension messaging API
(`chrome.runtime.sendMessage` / `browser.runtime.sendMessage`).

### 10.2 Clip operation message flow

```
Content script
  │  Captures: URL, title, selected text (DOM access)
  │  chrome.runtime.sendMessage({ type: "clip", url, title, selection, nostr_pubkey })
  ▼
Background worker
  │  Receives message
  │  Generates NodeId = crypto.randomUUID()  ← host generates UUID, never core
  │  Calls: apply_intent(workspace, JSON.stringify({
  │    AddNode: { id: nodeId, address: { Http: url }, title }
  │  }))
  │  Constructs NIP-84 kind 9802 event from clip schema (core serializes; host signs)
  │  Signs via window.nostr (NIP-07) or injected signer
  │  Publishes to Nostr relay via fetch()
  │  Writes WAL entry to IndexedDB
  ▼
Response sent back to content script (confirmation / node ID)
```

All `GraphIntent` variants cross the JS↔WASM boundary as JSON strings, matching the WAL
serialization format. No Rust types are exposed directly to JS.

### 10.3 Chrome MV3 service worker ephemerality

Chrome MV3 service workers are terminated after approximately 30 seconds of inactivity. The
in-memory `GraphWorkspace` is lost on termination. On the next activation, the worker must
reconstruct state from the persisted WAL before handling any intent.

**Consequences for Step 7**:

1. Every WAL entry must be written to IndexedDB before the intent response is returned to the
   content script. Fire-and-forget WAL writes are not safe — a worker termination between intent
   application and WAL write produces a lost update.
2. WAL replay must complete inside the service worker activation window. See Step 7 acceptance
   criterion (200ms / 1,000 entries).
3. WAL compaction (snapshotting) is required to bound replay time. The `GraphSnapshot` type in
   core must be snapshotable at any point, not only at explicit user save actions.

Firefox WebExtensions use a persistent background page by default (no ephemerality), but should
be designed to tolerate the same constraint for MV3 compatibility.

### 10.4 Sync: relay-only constraint

Browser extensions cannot open UDP sockets. iroh uses native QUIC over UDP and is therefore
unavailable in the extension context. The extension sync path is **Nostr-relay-only**:

- Mutations are published as NIP-84 `kind 9802` events or equivalent WAL-replay events.
- The extension subscribes to the relay for events matching its pubkey to receive updates from
  the desktop app.
- There is no direct P2P channel between the extension and desktop — the relay is the broker.

This is a permanent architectural constraint for the extension host, not a temporary limitation.
The desktop app's iroh sync path and the extension's relay sync path converge at the Nostr relay:
both publish and consume the same event types.

**Implication for Step 6**: The NIP-84 publication schema must be sufficient for full WAL replay,
not just clip publication. The relay event format is the extension's only durable sync channel.

### 10.5 UUID generation asymmetry

The desktop Rust host generates `NodeId` values via `Uuid::new_v4()`. The extension JS host
generates them via `crypto.randomUUID()`. Both produce UUID v4 strings in the same canonical
format (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`, lowercase, hyphenated).

**Required acceptance test (Step 2)**: A roundtrip test — JS-generated UUID string → WASM core
`NodeId` → WAL entry serialized as JSON → deserialized in a native Rust test → same `Uuid` value.
This must be an explicit test in the Step 2 PR, not assumed.

---

## 11. Mobile Host Architecture

This section describes how iOS and Android thin clients consume `graphshell-core-uniffi` and what
constraints are specific to the mobile deployment context.

### 11.1 Binding layer

iOS and Android apps consume `graphshell-core` via the `graphshell-core-uniffi` wrapper crate
(see §8a). UniFFI generates Swift bindings for iOS and Kotlin bindings for Android from a shared
Rust implementation. The `graphshell-core` API is designed for Rust ergonomics first; the
UniFFI UDL or proc-macro layer in `graphshell-core-uniffi` adapts it to Swift/Kotlin ergonomics.

No UniFFI attributes appear in `graphshell-core` itself.

### 11.2 Threading model

`wasm32-unknown-unknown` is single-threaded. iOS and Android are multi-threaded. The physics
`step()` function is pure and stateless, which means it can be called from any thread on mobile.
`GraphWorkspace` mutations must be serialized to a single thread (the "core thread") to preserve
the single-writer contract.

The mobile host is responsible for this serialization. Core makes no threading guarantees beyond
"safe to call from one thread at a time."

### 11.3 `Address::File` sandbox constraint

See §5.5. Mobile hosts must treat `Address::File` and `Address::Directory` as display-only.
Attempting to resolve these addresses on mobile will fail or access incorrect paths. The mobile
renderer for a `File` node must show the path as a label and offer an "unavailable on this device"
affordance rather than attempting to open it.

### 11.4 Sync path

Mobile hosts can use the Nostr relay sync path (same as the extension) and, where native
networking is available, the iroh-docs sync path. iroh compiles for iOS and Android as a native
dependency — it does not require WASM. The mobile host may therefore offer both relay and direct
P2P sync, unlike the extension host.

### 11.5 WAL persistence

Mobile hosts use platform-native storage (CoreData, SQLite, or a bundled key-value store) for WAL
persistence. The WAL entry types from `graphshell-core/src/persistence/wal.rs` are the canonical
serialization format — the mobile storage layer is an adapter that reads and writes these types.
No mobile-specific WAL schema variant is introduced.

---

## 12. Acceptance Criteria

### Core compilation (Steps 4+)

1. `cargo build -p graphshell-core --target wasm32-unknown-unknown` passes with zero errors.
2. No import of `egui`, `wgpu`, `servo`, `iroh`, `libp2p`, or any OS-dependent crate in core.
3. `apply_intents()` is in core; every `GraphIntent` variant is handled.
4. The core domain `GraphSemanticEvent` is the only type crossing from core to host at the domain
   event boundary. The desktop Servo lifecycle event type has been renamed (see Step 4 prerequisite)
   and does not conflict.
5. `Uuid::new_v4()` is never called inside core — all IDs are passed in by the host.
6. `graphshell` (host crate) compiles and all existing tests pass after extraction.
7. `wasm-pack build graphshell-core-wasm --target bundler` succeeds (after §8a wrapper crate
   is created).
8. No `#[wasm_bindgen]` or `#[uniffi::export]` attributes appear anywhere in `graphshell-core`.

### UUID roundtrip (Step 2)

1. A JS-generated UUID string (`crypto.randomUUID()` format) round-trips through the WASM core:
   WASM `NodeId` → WAL entry JSON → deserialized in native Rust → byte-identical `Uuid` value.
   (See §10.5.)

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

## 13. Open Questions

1. **WASM target variant for Verse server-side**: `wasm32-unknown-unknown` (most restrictive) vs.
   `wasm32-wasi` (allows OS-like syscalls). The CI gate uses `wasm32-unknown-unknown`; Verse
   server nodes may target WASI for access to clocks and file I/O. Consider a second gate for
   `wasm32-wasi` once the Verse deployment target is confirmed.

2. **`GraphWorkspace` split**: `GraphBrowserApp` currently holds both graph state and UI state.
   The extraction must cleanly separate them: `GraphWorkspace` in core owns pure graph state;
   `GraphBrowserApp` in the host owns everything else and holds a `GraphWorkspace`. Circular
   dependency risk must be reviewed at Step 4.

3. **Snapshot versioning and cross-platform forward compatibility**: WAL log entry types in core
   must be versioned for forward/backward compatibility between app versions and across platforms.
   The extension will have a different release cadence than the desktop app. A `schema_version`
   field on `GraphSnapshot` is the minimum. `#[serde(default)]` on new fields handles additive
   changes; removed fields need explicit migration. Design deferred to Step 7.

4. **`serde_json` vs. `postcard` for snapshot serialization**: `serde_json` is human-readable,
   debuggable, and required for the Nostr relay sync path (Nostr event content is always JSON).
   `postcard` is compact and fast but incompatible with the relay path. A likely outcome is
   `serde_json` for WAL entries that travel via relay, and optionally `postcard` for iroh-docs
   binary blobs. Decision deferred to Step 7 but constrained by §10.4.

5. **Relay event format for full WAL replay**: §X.4 states the relay event format must be
   sufficient for full WAL replay, not just clip publication. This requires deciding how non-clip
   mutations (edge creation, node rename, tag operations) are encoded as Nostr events. A
   dedicated event kind (e.g. `kind 30078` application-specific data, or a custom kind) is likely
   needed for generic WAL entries. Design required before Step 6.

6. **`graphshell-core-wasm` JS API surface**: The `apply_intent` JSON-string interface described
   in §8a is the minimal design. As the extension matures, higher-level query methods
   (`get_node`, `neighbors_of`, `snapshot_json`) will be needed. The JS API surface of
   `graphshell-core-wasm` should be specified before the extension host is built, not discovered
   incrementally. Track as a pre-extension design review.

---

*This document is the authoritative design reference for `graphshell-core` extraction.
Update it as steps complete, prerequisites change, or deployment targets are confirmed.*
