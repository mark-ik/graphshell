# GL → wgpu Compositor Redesign — Architectural Plan

**Context**: The compositor has two GL-shaped seams that need redesigning to match
the WgpuShared rendering model. This plan was synthesized from three independent
analyses (session analysis, Model A critique, Model B plan) against live code.

---

## Two GL-Shaped Seams (the actual targets)

### Seam 1 — Resource model
`tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>>`
([tile_compositor.rs:598](shell/desktop/workbench/tile_compositor.rs#L598),
[tile_compositor.rs:708](shell/desktop/workbench/tile_compositor.rs#L708))

The GL offscreen context IS the content resource. The wgpu path (`import_to_shared_wgpu_texture`)
already exists and is tried first ([compositor_adapter.rs:565-577](shell/desktop/workbench/compositor_adapter.rs#L565)),
but the map still types the resource as GL-specific.

### Seam 2 — Bridge abstraction
`BackendContentBridge` has a single variant: `ParentRenderCallback`
([render_backend/mod.rs:110-112](shell/desktop/render_backend/mod.rs#L110)).
The wgpu backend stubs (`register_custom_paint_callback`, `custom_pass_from_backend_viewport`)
are no-ops ([wgpu_backend.rs:29-42](shell/desktop/render_backend/wgpu_backend.rs#L29)).
The neutral contract is still callback-shaped even though egui is already on wgpu.

## Key model constraint (PROGRESS.md:108)

`WebView::composite_texture()` returns a **per-webview** `wgpu::Texture`, not a
monolithic multi-tile output. Graphshell composites N textures as image quads
in egui. The correct replacement resource is `NodeKey → wgpu::Texture`, not
`NodeKey → one big texture`.

---

## Phase Plan

### Phase A — `ContentSurfaceHandle` abstraction
Replace `Rc<OffscreenRenderingContext>` as the compositor-facing content resource.

```rust
enum ContentSurfaceHandle {
    ImportedWgpu(egui::TextureId),   // primary wgpu path
    CallbackFallback,                // named compat path (not removed yet)
    Placeholder,                     // degraded / loading
}
```

`tile_rendering_contexts` → `viewer_surfaces: HashMap<NodeKey, ContentSurfaceHandle>`

The existing `upsert_native_content_texture` + `register_content_callback_from_render_context`
becomes the logic that produces a `ContentSurfaceHandle` and stores it. The `OffscreenRenderingContext`
moves to a separate side-channel for GL compat only — not the primary resource map.

**Files**: `tile_compositor.rs`, `compositor_adapter.rs`

### Phase B — `BackendContentBridge` redesign
Add a `SharedWgpuTexture` variant; demote `ParentRenderCallback` to named fallback.

```rust
pub(crate) enum BackendContentBridge {
    SharedWgpuTexture { import: fn(...) -> Option<wgpu::Texture> },  // primary
    ParentRenderCallback(BackendParentRenderCallback),                // fallback
}
```

The wgpu backend stubs (`custom_pass_from_backend_viewport`, `register_custom_paint_callback`)
become deletable once `ParentRenderCallback` is demoted — they're the wrong shape for
wgpu's pre-render texture handoff model.

**Files**: `render_backend/mod.rs`, `render_backend/wgpu_backend.rs`

### Phase C — 3-axis invalidation
Split `CompositedContentSignature { webview_id, rect_px, semantic_generation }` into
three independent axes:

| Axis | What changes | Action |
|------|--------------|--------|
| Content | Servo produces a new frame | Re-import wgpu::Texture from WebRender |
| Placement | Tile rect changes (resize, layout) | Update egui image quad position only |
| Semantic | `semantic_generation` changes | Re-render overlay/affordance pass |

Placement-only changes don't need WebRender re-render — just move the blit.
This is the correct "tile vs document" split: not levels of granularity,
but independent invalidation signals.

**Files**: `tile_compositor.rs` (CompositedContentSignature + differential logic)

### Phase D — Viewer-surface registry keyed by viewer identity
Surface lifecycle follows **GraphTree node membership** (attach → allocate,
detach → drop), not tile tree existence.

```rust
struct ViewerSurfaceRegistry {
    surfaces: HashMap<NodeKey, ViewerSurface>,
}

struct ViewerSurface {
    texture: ContentSurfaceHandle,
    content_generation: u64,  // from Servo frame
    gl_ctx: Option<Rc<OffscreenRenderingContext>>,  // compat fallback only
}
```

`NodeKey` is the authority; `WebViewId` and `PaneId` are lookup keys within,
not owners.

**Files**: `compositor_adapter.rs` (new registry type), `tile_compositor.rs` (consumption)

### Phase E — GraphTree layout authority (link to decoupling plan)
The compositor adapter must target `NodeKey → ContentSurfaceHandle` throughout.
No `TileId` bridging at the adapter layer. This is the Phase E from the
decoupling plan; the wgpu redesign reinforces the same constraint.

### Phase F — GL guardrail retirement
After Phase A-D are stable on WgpuShared:
- Move `capture_gl_state`, `restore_gl_state`, chaos perturbation, scissor
  isolation behind `#[cfg(feature = "gl_compat")]`
- Delete when WgpuShared path is confirmed stable in production builds

**Files**: `compositor_adapter.rs` (guardrail machinery), `render_backend/gl_backend.rs`

### Phase G — Graph rendering (separate track)
Keep explicitly separate from webview composition redesign:
- Instanced wgpu render passes for graph nodes/edges
- Compute shader for Fruchterman-Reingold physics
- Zero-copy thumbnails (GPU texture-to-mappable-buffer, no CPU readback)

---

## What's Explicitly NOT in this Plan

- **Monolithic composite output** — wrong model; per-webview texture is correct
- **Parallel command buffer submission** — premature optimization; sequential is fine
- **Callbacks disappearing** — they graduate to `CallbackFallback` variant, not deleted

---

## Verification

- On WgpuShared path: no `OffscreenRenderingContext` appears in `viewer_surfaces` map
- Placement rect change does NOT trigger WebRender re-render (only blit update)
- `ContentSurfaceHandle::CallbackFallback` path still works for GL fallback builds
- `capture_gl_state` / `restore_gl_state` are unreachable in WgpuShared builds (can assert in debug)

---

---

# graphshell-core Extraction — Execution Plan

**Context**: Extract the portable identity, authority, and mutation kernel from
the Graphshell monolith into `crates/graphshell-core/`. This crate must compile
to `wasm32-unknown-unknown` with zero errors — the mechanical enforcement of
platform independence. It becomes the shared foundation for desktop, mobile,
browser extension, and Verse server-side deployments.

The canonical design spec lives at
`design_docs/graphshell_docs/technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`.
This execution plan implements **Step 4** of that spec (the main extraction),
with prerequisite fixups.

---

## Prerequisites Status

| Step | Status | Notes |
|------|--------|-------|
| 0: Petgraph algorithms | ✅ Done | `hop_distances_from`, `neighbors_undirected`, `weakly_connected_components` exist |
| 1: GraphPos2 | ⏭️ Deferred | Node already uses `euclid::Point2D<f32>`, not `egui::Pos2`. euclid is WASM-clean. GraphPos2 deferred to Step 8 (physics). |
| 2: UUID identity | ✅ Done | `Node.id: Uuid` exists, separate from Address |
| 3: Address enum | ✅ Done | `Address { Http, File, Data, Clip, Directory, Custom }` landed 2026-03-26 |
| 4 prereq: GraphSemanticEvent rename | ❌ Not done | Must rename before extraction (Phase 0 below) |

## Extraction Phases Status

| Phase | Status | Notes |
|-------|--------|-------|
| 0: Rename GraphSemanticEvent | ⏭️ Skipped | Not blocking — extracted types don't reference it |
| 1: Scaffold the crate | ✅ Done | `crates/graphshell-core/` created, in workspace |
| 2: Move leaf types + persistence snapshot | ✅ Done | `types.rs`, `persistence.rs` in core; host re-exports via `pub use` |
| 3: Move Address to core | ✅ Done | `address.rs` in core with all helpers |
| 4: Move Graph, Node, NodeKey, EdgePayload | ✅ Done | `graph/mod.rs`, `graph/apply.rs`, `graph/filter.rs`, `graph/facet_projection.rs` in core |
| 5: Wire host, fix visibility | ✅ Done | 2242 host tests pass (12 pre-existing failures, unrelated) |
| 6: Cleanup and WASM gate hardening | ✅ Done | `cargo check -p graphshell-core --target wasm32-unknown-unknown` passes; `Uuid::new_v4()` and `test_stub` gated behind `cfg(not(wasm32))` |

---

## What Moves to Core

### Portable (moves)
- `model/graph/mod.rs` (5,717 lines) — Graph, Node, NodeKey, EdgePayload, Address, NodeLifecycle, classifications
- `model/graph/apply.rs` (267 lines) — GraphDelta, apply_graph_delta
- `model/graph/filter.rs` (783 lines) — Edge filtering
- `model/graph/badge.rs` (448 lines) — Node badge presentation state
- `model/graph/facet_projection.rs` (337 lines) — Facet grouping
- `services/persistence/types.rs` (subset) — PersistedNode, PersistedEdge, GraphSnapshot, all sub-kind enums

### Stays in Host
- `model/graph/egui_adapter.rs` (2,498 lines) — egui rendering
- `model/graph/edge_style_registry.rs` (664 lines) — pervasive `egui::Color32` usage (60+ refs). Stays until a portable color type is introduced.
- `graph/physics.rs`, `graph/layouts/`, `graph/frame_affinity.rs` — egui_graphs/egui deps
- All shell/, render/, webview lifecycle code
- Intent system (`app/intents.rs`, `app/intent_phases.rs`, `app/graph_mutations.rs`) — deeply coupled to host types. Deferred to a later extraction step.

### Key Decision: Intents Stay in Host (for now)

The 158+ `GraphIntent` variants span 4 dispatch phases and reference host-only types
(`RendererId`, `PaneId`, `GraphViewId`, `FloatingPaneTargetTileContext`, `Instant`,
`HostOpenRequest`, `PendingTileOpenMode`). Only ~30 are pure domain mutations.
Moving all variants would require pulling those types into core or massive splitting
surgery. This extraction focuses on the **data model**. Intent extraction is a
follow-on step that defines a `CoreIntent` subset.

---

## Execution Phases

### Phase 0: Rename GraphSemanticEvent (prerequisite)

Rename `GraphSemanticEvent` / `GraphSemanticEventKind` in the shell to
`WebViewLifecycleEvent` / `WebViewLifecycleEventKind`. This clears the namespace
for the core domain event type defined in the extraction plan.

**Files** (5, all `pub(crate)` scope — mechanical rename):
- `shell/desktop/host/window.rs` — definition
- `shell/desktop/lifecycle/semantic_event_pipeline.rs`
- `shell/desktop/host/window/graph_events.rs`
- `shell/desktop/ui/gui.rs`
- `shell/desktop/ui/gui_tests.rs`
- `shell/desktop/host/running_app_state.rs`
- `shell/desktop/host/embedder.rs`
- `shell/desktop/ui/gui/intent_translation.rs`

### Phase 1: Scaffold the crate

Create `crates/graphshell-core/` with Cargo.toml and empty lib.rs. Add to workspace.

**Cargo.toml deps**:
```toml
petgraph = { version = "0.8.3", features = ["serde-1"] }
uuid = { version = "1", features = ["serde", "v4"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rkyv = { version = "0.8", features = ["std"] }
euclid = "0.22"
url = "2.5"
mime_guess = "2.0"
infer = "0.19"
time = { version = "0.3", features = ["formatting"] }
```

**Verification**: `cargo check -p graphshell-core` passes.

### Phase 2: Move leaf types + persistence snapshot types

Move small, dependency-free types first — these are referenced by both
`model/graph/mod.rs` and `services/persistence/types.rs`, so they must land
in core before either large module can move.

**Types that move** (from `model/graph/mod.rs`):
- `NodeClassification`, `ClassificationScheme`, `ClassificationStatus`, `ClassificationProvenance`
- `NodeImportProvenance`, `ImportRecord`, `ImportRecordMembership`, `NodeImportRecordSummary`
- `NodeTagPresentationState` (from `model/graph/badge.rs`)
- `FrameLayoutHint`, `SplitOrientation`, `DominantEdge`, `FrameLayoutNodeId`

**Types that move** (from `services/persistence/types.rs`):
- `GraphSnapshot`, `PersistedNode`, `PersistedEdge`, `PersistedNodeSessionState`
- `PersistedAddress`, `PersistedAddressKind`
- All `PersistedEdgeFamily` and sub-kind enums
- All edge data structs (`PersistedTraversalEdgeData`, etc.)
- `PersistedTraversalRecord`, `PersistedTraversalMetrics`, `PersistedNavigationTrigger`

**Core module structure**:
- `crates/graphshell-core/src/types.rs` — leaf graph types
- `crates/graphshell-core/src/persistence.rs` — snapshot/persisted types

**Host shims**: `model/graph/mod.rs` and `services/persistence/types.rs` get
`pub use graphshell_core::{types::*, persistence::*};` re-exports so downstream
code compiles unchanged.

### Phase 3: Move Address to core

Move `Address`, `AddressKind`, `address_from_url`, `address_kind_from_url`,
`file_url_uses_directory_syntax`, `cached_host_from_url`, `detect_mime` to
`crates/graphshell-core/src/address.rs`.

These use `url::Url::parse`, `mime_guess`, `infer` — all WASM-clean.

**Host shim**: `pub use graphshell_core::address::*;` in `model/graph/mod.rs`.

### Phase 4: Move Graph, Node, NodeKey, EdgePayload to core

The main extraction. Move the bulk of `model/graph/mod.rs` (Graph struct,
Node struct, EdgePayload, NodeLifecycle, NodeKey type alias, all `impl Graph`
methods, rkyv bridge types, `from_snapshot`/`to_snapshot`) to
`crates/graphshell-core/src/graph/mod.rs`.

Sub-modules that also move:
- `model/graph/apply.rs` → `crates/graphshell-core/src/graph/apply.rs`
- `model/graph/filter.rs` → `crates/graphshell-core/src/graph/filter.rs`
- `model/graph/facet_projection.rs` → `crates/graphshell-core/src/graph/facet_projection.rs`

**What stays**: `model/graph/egui_adapter.rs`, `model/graph/edge_style_registry.rs`

**Host shim**: `model/graph/mod.rs` becomes:
```rust
pub use graphshell_core::graph::*;
pub mod egui_adapter;
pub mod edge_style_registry;
```

**Visibility concern**: Many `Graph` mutation methods are `pub(crate)`. When moved
to graphshell-core, `pub(crate)` means within core, not within the host. Items
the host needs for persistence replay become `pub` with doc comments explaining
the trust boundary. Items only core needs stay `pub(crate)`.

**Test migration**: Tests at the bottom of `model/graph/mod.rs` (lines 4670+)
that construct `GraphSnapshot` objects move to `crates/graphshell-core/tests/`.

### Phase 5: Wire host, fix visibility, verify compilation

Fix all import path breakages. The re-export shim chain means most downstream
code compiles unchanged, but some `pub(crate)` items need visibility adjustments.

**Verification**:
- `cargo check` (full workspace)
- `cargo check -p graphshell-core --target wasm32-unknown-unknown`
- `cargo test` (full test suite)

### Phase 6: Cleanup and WASM gate hardening ✅

- `Graph::add_node()` and `Node::test_stub()` gated with `#[cfg(not(target_arch = "wasm32"))]`
- `GraphDelta::AddNode { id: None }` panics on WASM (hosts must supply IDs)
- `uuid` `v4` feature is target-gated: only enabled on non-WASM via `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
- No `std::time::Instant` in core (verified)
- `cargo check -p graphshell-core --target wasm32-unknown-unknown` passes with 0 errors
- 97 core tests pass, 2242 host tests pass

---

## Deferred Work (later extraction steps)

| What | When | Why deferred |
|------|------|--------------|
| Intent system (`GraphIntent` + `apply_intents`) | Step 4+ (separate plan) | 158 variants referencing host-only types; needs `CoreIntent` subset design |
| Edge style registry | After portable color type | 60+ `egui::Color32` references |
| Physics engine (`step()`, `cold_start_positions`) | Step 8 | Requires `GraphPos2` or equivalent |
| Coop authority | Step 5 | After graph data model is stable in core |
| NIP-84 publication | Step 6 | After core stabilizes |
| Persistence WAL types | Step 7 | `LogEntry` enum has host coupling |
| `GraphWorkspace` container | After intents move | Needs intent dispatch to be in core first |

---

## Verification Plan

After each phase:
1. `cargo check` — full workspace compiles
2. `cargo test` — all tests pass
3. After Phase 5+: `cargo check -p graphshell-core --target wasm32-unknown-unknown` — WASM gate

End-to-end:
- The graphshell desktop binary builds and runs identically
- graph-tree crate still compiles (no dependency on graphshell-core — they're siblings)
- No rkyv deserialization breakage (structural matching, not path-based)
- No serde breakage (field-name-based, not module-path-based)

---

## Critical Files

| File | Lines | Role in extraction |
|------|-------|--------------------|
| `model/graph/mod.rs` | 5,717 | Primary extraction source |
| `services/persistence/types.rs` | ~800 | Snapshot types, move to core |
| `model/graph/apply.rs` | 267 | GraphDelta, moves with Graph |
| `model/graph/filter.rs` | 783 | Edge filter, moves with Graph |
| `model/graph/badge.rs` | 448 | Badge state, leaf type moves |
| `model/graph/facet_projection.rs` | 337 | Facet grouping, moves |
| `model/graph/edge_style_registry.rs` | 664 | Stays (egui::Color32 dep) |
| `model/graph/egui_adapter.rs` | 2,498 | Stays (egui rendering) |
| `shell/desktop/host/window.rs` | — | GraphSemanticEvent rename |
| `Cargo.toml` (workspace root) | — | Add workspace member + dep |
