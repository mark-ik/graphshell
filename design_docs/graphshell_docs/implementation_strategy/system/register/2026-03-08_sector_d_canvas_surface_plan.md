<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector D — Canvas Surface Registry Development Plan

**Doc role:** Implementation plan for the canvas surface registry sector
**Status:** Active / partially implemented
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `CanvasRegistry`, `LayoutRegistry`, `PhysicsProfileRegistry`, `LayoutDomainRegistry`, `PresentationDomainRegistry`
**Specs:** [canvas_registry_spec.md](canvas_registry_spec.md), [layout_registry_spec.md](layout_registry_spec.md), [physics_profile_registry_spec.md](physics_profile_registry_spec.md), [layout_domain_registry_spec.md](layout_domain_registry_spec.md), [presentation_domain_registry_spec.md](presentation_domain_registry_spec.md)
**Also depends on:** `../2026-03-08_graph_app_decomposition_plan.md`

---

## Purpose

The canvas surface is how the spatial browser looks and moves. Every visual property of the
graph — node layout algorithm, force simulation parameters, label rendering, animation curves,
zoom behaviour — currently lives as hardcoded constants scattered across `render/mod.rs` and
`graph_app.rs`. None of these five registries have Rust implementations.

These registries are tightly coupled. The `layout-first` principle from `layout_domain_registry_spec.md`
requires that layout resolves before presentation, and `CanvasRegistry` is the graph-domain surface
authority that coordinates topology, layout, and interaction policy. All five must be developed
together in this sector.

```
CanvasRegistry
 ├── topology policy   ──► node culling, edge routing, selection geometry
 ├── layout policy     ──► LayoutRegistry (named algorithms)  ──► LayoutDomainRegistry
 └── interaction policy ──► PhysicsProfileRegistry (force presets)

LayoutDomainRegistry  ──► coordinates layout-first across graph + workbench + viewer surfaces
PresentationDomainRegistry ──► appearance + motion semantics after layout resolves
                             ──► ThemeRegistry (Sector G)
```

---

## Current State

Implementation update (2026-03-10):
- `PhysicsProfileRegistry` is implemented as a runtime-owned active-profile authority in
  `shell/desktop/runtime/registries/physics_profile.rs` and wired through `RegistryRuntime`.
- `CanvasRegistry` is implemented as a runtime-owned active-profile authority in
  `shell/desktop/runtime/registries/canvas.rs` and now drives live keyboard-pan and lasso policy.
- `LayoutDomainRegistry` exists as a domain coordinator and is now owned by `RegistryRuntime`
  for active viewer-surface resolution.
- `PresentationDomainRegistry` now resolves concrete presentation tokens used by
  `render/mod.rs` and `tile_compositor.rs` instead of leaving those colors hardcoded.

Remaining gap:
- The dedicated `LayoutRegistry` / `LayoutAlgorithm` abstraction from Phase D2 is still not
  implemented as its own runtime authority. Layout execution remains on the current
  `egui_graphs` Fruchterman-Reingold path in `render/mod.rs`, with canvas profiles carrying the
  algorithm id as policy metadata only.

The `graph_app_decomposition_plan.md` (dated 2026-03-08) is the parallel structural work; these
registry implementations are the policy surface that decomposed app code will call into.

---

## Phase D1 — PhysicsProfileRegistry: Named force presets

**Start here.** Physics profiles are the simplest atomic registry and unlock immediate
user-visible behaviour changes (graph "feel" modes).

The `physics_profile_registry_spec.md` documents the Fruchterman-Reingold 1991 algorithm
and three canonical preset families: `Liquid`, `Gas`, `Solid`.

### D1.1 — Define `PhysicsProfile` and `PhysicsProfileRegistry`

```rust
pub struct PhysicsProfile {
    pub id: PhysicsProfileId,
    pub display_name: String,
    pub repulsion_strength: f32,     // k² / distance coefficient
    pub attraction_strength: f32,    // spring constant for edges
    pub gravity_center: f32,         // pull toward canvas centre
    pub damping: f32,                // velocity damping per tick
    pub max_displacement: f32,       // clamp per step
    pub cooling_factor: f32,         // temperature decay (0 < f < 1)
    pub iterations_per_frame: u8,
}

pub const PHYSICS_PROFILE_LIQUID: PhysicsProfileId = PhysicsProfileId("physics:liquid");
pub const PHYSICS_PROFILE_GAS: PhysicsProfileId = PhysicsProfileId("physics:gas");
pub const PHYSICS_PROFILE_SOLID: PhysicsProfileId = PhysicsProfileId("physics:solid");

pub struct PhysicsProfileRegistry {
    profiles: HashMap<PhysicsProfileId, PhysicsProfile>,
    active: PhysicsProfileId,
}
```

Built-in presets tuned for the existing graph feel (Gas ≈ current default).

**Done gates:**
- [x] `PhysicsProfileRegistry` struct in `shell/desktop/runtime/registries/physics_profile.rs`.
- [x] `LIQUID`, `GAS`, `SOLID` presets registered with calibrated values matching current graph behaviour.
- [x] `set_active_profile()` + `active_profile()` API.
- [x] Added to `RegistryRuntime`.
- [x] `DIAG_PHYSICS_PROFILE` channel (Info) emits on profile switch.
- [x] Unit test: each preset resolves to distinct parameter values.

### D1.2 — Replace hardcoded force constants in `render/mod.rs`

All `FORCE_*` constants in `render/mod.rs` are replaced by calls to the active physics profile:

```rust
let profile = registries.physics_profile.active_profile();
let repulsion = profile.repulsion_strength;
// ...
```

This is the key decomposition step that removes hardcoded physics from the render path.

**Done gates:**
- [x] All `FORCE_*` constants removed from `render/mod.rs`.
- [x] Physics simulation reads from `PhysicsProfileRegistry::active_profile()`.
- [x] Visual regression check: default Gas profile produces identical graph layout to before.

### D1.3 — Profile switching via action

Register `graph:set_physics_profile { profile_id }` in `ActionRegistry` (Sector B). Switching
profile emits `GraphIntent::SetPhysicsProfile { profile_id }` through the reducer; the reducer
updates the active profile and triggers a physics reheat.

**Done gates:**
- [x] `GraphIntent::SetPhysicsProfile` variant defined and handled.
- [x] Physics reheats (temperature reset) on profile switch.
- [x] Profile switch persists to workspace state.

---

## Phase D2 — LayoutRegistry: Named layout algorithms

**Unlocks:** Layout algorithm selection; graph layout experiments.

The `layout_registry_spec.md`'s `algorithm-contract` policy: every layout algorithm must define
its input graph constraints, output coordinate contract, and determinism guarantee.

### D2.1 — Define `LayoutAlgorithm` trait and `LayoutRegistry`

```rust
pub trait LayoutAlgorithm: Send + Sync {
    fn id(&self) -> LayoutAlgorithmId;
    fn display_name(&self) -> &str;
    fn is_deterministic(&self) -> bool;

    /// Execute one step of the layout. Returns true if stable (converged).
    fn step(
        &self,
        nodes: &mut [(NodeKey, Vec2)],
        edges: &[(NodeKey, NodeKey)],
        profile: &PhysicsProfile,
        canvas_size: Vec2,
    ) -> bool;
}

pub struct LayoutRegistry {
    algorithms: HashMap<LayoutAlgorithmId, Box<dyn LayoutAlgorithm>>,
    active: LayoutAlgorithmId,
}
```

Built-in algorithms:
- `layout:fruchterman_reingold` — current default; extracted from `render/mod.rs`.
- `layout:force_atlas_2` — prospective (stub for now, returns `unimplemented!`).
- `layout:hierarchical` — prospective (stub).

The `no-hidden-mutation` policy: layout steps only mutate the node coordinate buffer; they must
not touch `GraphBrowserApp` fields directly.

**Done gates:**
- [ ] `LayoutAlgorithm` trait defined.
- [ ] `FruchtermanReingold` struct implementing the trait, extracted from current render code.
- [ ] `LayoutRegistry` struct with active algorithm selection.
- [ ] `LayoutRegistry` added to `RegistryRuntime`.
- [ ] Unit test: `FruchtermanReingold::step()` moves nodes; converges to stable on complete graph.

Implementation note (2026-03-10):
- This is the remaining structural gap in Sector D. Current runtime-owned canvas/physics/presentation
  work does not satisfy D2 by itself.

### D2.2 — Extract Fruchterman-Reingold from `render/mod.rs`

Move the existing force-directed algorithm implementation into a dedicated
`app/graph_layout.rs` module that implements `LayoutAlgorithm`. This is the structural
companion to the `graph_app_decomposition_plan.md`.

**Done gates:**
- [ ] `FruchtermanReingoldLayout` struct in `app/graph_layout.rs`.
- [ ] `render/mod.rs` calls `registries.layout.active_algorithm().step()`.
- [ ] No layout logic remains inline in `render/mod.rs`.

---

## Phase D3 — CanvasRegistry: Graph-domain surface authority

**Unlocks:** Per-canvas topology / layout / interaction policy; `CanvasStylePolicy`,
`CanvasNavigationPolicy`, `CanvasTopologyPolicy` canonical extension points (CLAUDE.md).

### D3.1 — Define `CanvasProfile` and `CanvasRegistry`

The `canvas_registry_spec.md` separates three concerns: topology policy, layout policy, and
interaction/rendering policy. These must not be conflated.

```rust
pub struct CanvasProfile {
    pub id: CanvasProfileId,
    pub topology: CanvasTopologyPolicy,
    pub layout: CanvasLayoutPolicy,
    pub interaction: CanvasInteractionPolicy,
}

pub struct CanvasTopologyPolicy {
    pub edge_routing: EdgeRouting,        // Straight | Curved | Orthogonal
    pub culling_enabled: bool,
    pub lod_levels: Vec<LodLevel>,        // label/edge detail at zoom thresholds
    pub selection_geometry: SelectionGeometry,  // Single | Lasso | Box
}

pub struct CanvasLayoutPolicy {
    pub algorithm_id: LayoutAlgorithmId,
    pub physics_profile_id: PhysicsProfileId,
    pub initial_placement: InitialPlacement, // Random | Radial | Grid
}

pub struct CanvasInteractionPolicy {
    pub zoom_range: (f32, f32),
    pub pan_enabled: bool,
    pub node_drag_enabled: bool,
    pub edge_create_gesture: EdgeCreateGesture,
}

pub struct CanvasRegistry {
    profiles: HashMap<CanvasProfileId, CanvasProfile>,
    active: CanvasProfileId,
}
```

`CanvasStylePolicy`, `CanvasNavigationPolicy`, `CanvasTopologyPolicy` are the canonical
extension points per CLAUDE.md — they are the fields on `CanvasProfile`.

**Done gates:**
- [x] `CanvasRegistry` struct in `shell/desktop/runtime/registries/canvas.rs`.
- [x] `CANVAS_PROFILE_DEFAULT` registered with values matching current hardcoded behaviour.
- [x] Added to `RegistryRuntime`.
- [x] `resolve_topology()`, `resolve_layout()`, `resolve_interaction_policy()` APIs exposed.
- [x] `DIAG_CANVAS_PROFILE` channel (Info severity).

### D3.2 — Wire canvas profile into render path

`render/mod.rs` and `render/panels.rs` call into `CanvasRegistry` for topology, layout algorithm,
and interaction policy instead of hardcoded values.

**Done gates:**
- [x] Culling and LOD toggles from `CanvasTopologyPolicy` (PLANNING_REGISTER §3 quick-win #10).
- [x] Edge routing mode read from `CanvasTopologyPolicy`.
- [x] Zoom range enforced from `CanvasInteractionPolicy`.
- [ ] No hardcoded canvas constants remain in `render/`.

Implementation note (2026-03-10):
- Render-time navigation/culling/lasso policy now resolves through the active canvas profile, but
  the final “no hardcoded canvas constants remain” claim still depends on D2’s extracted layout
  algorithm path.

### D3.3 — Zoom-adaptive label LOD

PLANNING_REGISTER §3 quick-win #8: "Add zoom-adaptive label LOD." Wire label detail levels
from `CanvasTopologyPolicy::lod_levels` into the label rendering path.

**Done gates:**
- [x] `LodLevel` threshold table governs label visibility at each zoom level.
- [x] Label rendering uses `CanvasRegistry::active_profile().topology.lod_levels`.

---

## Phase D4 — LayoutDomainRegistry and PresentationDomainRegistry

**Unlocks:** Cross-surface layout coordination; themed presentation; full `layout-first` policy.

### D4.1 — `LayoutDomainRegistry`: coordinate layout-first across surfaces

The `layout_domain_registry_spec.md`'s `layout-first` policy: layout resolves before presentation
across graph, workbench, and viewer surfaces. The domain registry is the coordinator.

```rust
pub struct LayoutDomainRegistry {
    canvas_registry: Arc<CanvasRegistry>,
    workbench_surface: Arc<WorkbenchSurfaceRegistry>,  // Sector E
    viewer_surface: Arc<ViewerSurfaceRegistry>,         // Sector A
}

impl LayoutDomainRegistry {
    /// Resolve the complete layout intent for a frame tick.
    pub fn resolve_layout_frame(&self, context: &LayoutContext) -> LayoutFrameResult
}
```

The `surface-sovereignty` policy: each surface owns its own layout state; the domain registry
coordinates sequencing, not ownership.

**Done gates:**
- [x] `LayoutDomainRegistry` struct in `registries/domain/layout/mod.rs`.
- [ ] `resolve_layout_frame()` sequences: canvas layout step → workbench tile layout → viewer viewport.
- [x] Added to `RegistryRuntime`.
- [x] No layout step for surface X modifies surface Y's layout state.

Implementation note (2026-03-10):
- The domain registry is now runtime-owned for active surface resolution, but the stronger
  `resolve_layout_frame()` sequencing abstraction remains blocked on D2’s missing standalone layout authority.

### D4.2 — `PresentationDomainRegistry`: post-layout appearance

The `presentation_domain_registry_spec.md`'s `post-layout` policy: presentation (colour, motion,
typography, animation) applies only after layout resolves.

```rust
pub struct PresentationProfile {
    pub theme_id: ThemeId,              // resolved from ThemeRegistry (Sector G)
    pub motion: MotionPolicy,           // Reduced | Standard | Expressive
    pub node_label_font: FontSpec,
    pub edge_label_font: FontSpec,
    pub selection_highlight: Color,
    pub focus_ring: FocusRingSpec,
}

pub struct PresentationDomainRegistry {
    profiles: HashMap<PresentationProfileId, PresentationProfile>,
    active: PresentationProfileId,
}
```

`ThemeRegistry` (Sector G) provides the token set; `PresentationDomainRegistry` resolves the
per-canvas profile that selects from those tokens.

**Done gates:**
- [x] `PresentationDomainRegistry` struct defined.
- [x] `PRESENTATION_PROFILE_DEFAULT` registered.
- [x] Added to `RegistryRuntime`.
- [x] `resolve_presentation_profile()` called from graph/workbench render paths instead of hardcoded colours.
- [x] Deferred: full theme token resolution remains owned by Sector G `ThemeRegistry` completion.

---

## Acceptance Criteria (Sector D complete)

- [ ] No hardcoded force constants in `render/mod.rs`; all resolved from `PhysicsProfileRegistry`.
- [ ] No hardcoded canvas constants in `render/panels.rs`; all resolved from `CanvasRegistry`.
- [ ] Layout algorithm is `FruchtermanReingold` struct implementing `LayoutAlgorithm` trait.
- [ ] `CanvasTopologyPolicy`, `CanvasNavigationPolicy`, `CanvasStylePolicy` are the documented
  extension points per CLAUDE.md.
- [ ] LOD-based label visibility works at zoom-in and zoom-out thresholds.
- [ ] `LayoutDomainRegistry` sequences layout-first across canvas, workbench, and viewer.
- [x] `PresentationDomainRegistry` applies presentation after layout; no hardcoded colours remain in the graph/workbench presentation paths touched in D4.
- [ ] Physics profile can be switched at runtime via `graph:set_physics_profile` action.
- [ ] All five registries are in `RegistryRuntime` and covered by diagnostics channels.

Sector D implementation-state summary (2026-03-10):
- D1, D3, and D4 runtime/profile work are implemented.
- D2 (`LayoutRegistry` / extracted `LayoutAlgorithm`) remains the honest blocker for calling the
  whole sector complete.

---

## Related Documents

- [canvas_registry_spec.md](canvas_registry_spec.md)
- [layout_registry_spec.md](layout_registry_spec.md)
- [physics_profile_registry_spec.md](physics_profile_registry_spec.md)
- [layout_domain_registry_spec.md](layout_domain_registry_spec.md)
- [presentation_domain_registry_spec.md](presentation_domain_registry_spec.md)
- [../2026-03-08_graph_app_decomposition_plan.md](../2026-03-08_graph_app_decomposition_plan.md)
- [2026-03-08_sector_g_mod_agent_plan.md](2026-03-08_sector_g_mod_agent_plan.md) — ThemeRegistry dependency
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
