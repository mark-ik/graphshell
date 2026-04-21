<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Layouts as Pluggable Mods Plan (2026-04-19)

**Status**: First-pass scope landed (graph-canvas registry). Host integration
and third-party registration API still pending.
**Scope**: Treat every graph-canvas layout — built-in or third-party — as
an entry in a common mod registry, so users get a flat "layouts I can
pick from" surface combining Graphshell's bundled defaults with
third-party additions. Distinct from the WASM-guest-specific lane
(which covers the sandboxing + ABI for external layouts) because this
plan also applies to native Rust layouts shipped by mods inside the
Graphshell process.

**Parent**:
[2026-04-19_step5_spatial_pattern_layouts_plan.md §1.1](2026-04-19_step5_spatial_pattern_layouts_plan.md)
enumerates the built-in set that needs to flow through this registry.

**Related**:

- [2026-04-03_wasm_layout_runtime_plan.md](2026-04-03_wasm_layout_runtime_plan.md) — WASM guest ABI for sandboxed external layouts.
- [2026-04-03_layout_variant_follow_on_plan.md](2026-04-03_layout_variant_follow_on_plan.md) — the built-in variant portfolio.
- [2026-04-03_layout_backend_state_ownership_plan.md](2026-04-03_layout_backend_state_ownership_plan.md) — the shared state carrier the registry hands out.

---

## 1. Framing

Per the project's configurability / modularity principle (captured in
agent-memory as `feedback_configurability_over_opinionated_defaults.md`),
every major extension surface in Graphshell is modeled as a registry of
interchangeable providers. Layouts are one such surface.

The bundled layouts that currently live at `graph_canvas::layout::*` are
not special except in that they ship in the Graphshell binary. A
third-party mod should be able to register a `Layout<N>` impl on the
same footing and have it appear in the user's layout picker next to FR
and Phyllotaxis.

### 1.1 Why this is distinct from the WASM runtime plan

The WASM runtime plan handles sandboxed layouts authored in WASM
guests — a separate security/ABI concern. This plan covers the
higher-level registration + discovery model that *both* native and WASM
layouts share. A WASM-hosted layout registers through this registry;
the registry just happens to back it with a guest-ABI adapter.

---

## 2. Current reality

- `graph_canvas::layout` exports eleven concrete `Layout<N>` impls plus
  configs.
- Graphshell's `app::graph_layout` has a `LayoutAlgorithm` trait with
  four concrete impls (`ForceDirectedLayout`, `ForceDirectedBarnesHutLayout`,
  `GridLayout`, `TreeLayout`) registered in a `LayoutRegistry`.
- There's no unified user-facing "layout picker" that shows every
  registered layout. The two trait systems coexist and don't see each
  other.

The immediate structural gap: `LayoutAlgorithm` is a one-shot
mutate-the-graph interface, `Layout<N>` is an iterative
return-deltas interface. They serve different lifecycles (instant-apply
vs continuous tick). Both are legitimate; the registry should surface
both under one user-facing catalog without forcing either into the other
shape.

---

## 3. Design decisions

### 3.1 Two trait categories, one catalog

The registry recognizes two layout categories:

- **Analytic / static layouts**: implement `LayoutAlgorithm` (one-shot
  apply, mutate graph positions directly). Examples: Grid, Tree, Radial,
  Phyllotaxis (as static snap), Penrose, L-system, SemanticEmbedding.
- **Dynamic / iterative layouts**: implement `Layout<N>` (per-tick
  return deltas). Examples: FR, BarnesHut, SemanticEdgeWeight, rapier
  scene physics.

The registry stores both under one catalog, keyed by `LayoutId`, with a
tag indicating category. The user picks from a flat list; the runtime
dispatches to the correct lifecycle based on category.

Some layouts straddle the line — e.g., Phyllotaxis can run as one-shot
(damping=1.0) or as animate-in (damping<1.0). Those are registered under
both categories with shared config.

### 3.2 `LayoutCapability` metadata

Every registered layout declares:

```rust
pub struct LayoutCapability {
    pub id: LayoutId,                    // "graph_layout:force_directed"
    pub display_name: String,            // "Force Directed"
    pub category: LayoutCategory,        // Analytic | Dynamic | Both
    pub is_deterministic: bool,
    pub is_topology_sensitive: bool,
    pub config_schema: ConfigSchema,     // for settings UI
    pub supports_3d: bool,
    pub recommended_max_node_count: Option<usize>,
    pub provenance: LayoutProvenance,    // Builtin | NativeMod | WasmMod
    pub capability_tags: HashSet<String>,// "spatial-memory", "semantic", etc.
}
```

This metadata drives:

- Layout picker UI (grouping by tags / category).
- Recommendation logic (match node-count scale to capability).
- Fallback selection (if requested layout is unavailable).
- Diagnostics (requested vs resolved layout IDs).

### 3.3 Registration API

```rust
pub trait LayoutProvider: Send + Sync {
    fn capability(&self) -> LayoutCapability;
    fn create_analytic(&self) -> Option<Box<dyn LayoutAlgorithm>>;
    fn create_dynamic(&self) -> Option<Box<dyn DynLayout>>;
}

// Where DynLayout is an object-safe shim over Layout<N> for the
// common node key type. Details in §3.5.

pub struct LayoutRegistry {
    providers: HashMap<LayoutId, Arc<dyn LayoutProvider>>,
}

impl LayoutRegistry {
    pub fn register(&mut self, provider: Arc<dyn LayoutProvider>) -> Result<(), RegisterError>;
    pub fn unregister(&mut self, id: &LayoutId) -> bool;
    pub fn resolve(&self, id: &LayoutId) -> Option<Arc<dyn LayoutProvider>>;
    pub fn capabilities(&self) -> impl Iterator<Item = &LayoutCapability>;
    pub fn filter_by(&self, tag: &str) -> impl Iterator<Item = &LayoutCapability>;
}
```

Provenance:

- Built-in providers register in `LayoutRegistry::default()` at process
  start.
- Native-mod providers register via `inventory::submit!` or an explicit
  mod-load call.
- WASM-guest providers register via the WASM mod runtime, which wraps
  the guest in a `LayoutProvider` adapter.

### 3.4 Admission rules

For a layout to be admitted:

1. Stable `LayoutId` (URN-like: `graph_layout:<family>:<variant>`).
2. Deterministic input ordering (same input → same output, for
   analytic layouts).
3. Documented fallback: what happens when the layout can't apply (too
   few nodes, missing metadata, capability mismatch).
4. Config schema declared (even if all-optional); enables settings UI.
5. `LayoutCapability::recommended_max_node_count` set honestly; hosts
   enforce or warn at this threshold.

Providers that don't meet admission rules are rejected at register
time with a structured `RegisterError`.

### 3.5 Trait-object storage

`Layout<N>` has an associated type (`State`), which blocks naive
`dyn Layout<N>`. Two options:

- **Object-safe shim**: introduce a `DynLayout` trait where `State` is
  erased to `Box<dyn Any>`. Providers box their concrete state type
  internally and downcast on access. Runtime cost: one `Box` + one
  downcast per step.
- **Sum type**: enumerate all registered layouts in a single
  `ActiveLayout<N>` variant. Fast, no allocation, but doesn't support
  third-party registration dynamically (the sum is fixed at compile time).

The registry's value is runtime extensibility, so `DynLayout` is the
right choice. `ActiveLayout<N>` can still exist as a convenience for
the known-built-in set (used by hosts that don't care about third-party
mods).

### 3.6 User-visible layout picker

The picker shows all registered layouts grouped by category and
provenance:

- **Force / Physics** (dynamic): FR, Barnes-Hut, Semantic Edge Weight,
  Rapier Scene...
- **Analytic** (static): Grid, Radial, Phyllotaxis, Timeline, Kanban,
  Tree...
- **Semantic** (either, tagged): Semantic Embedding, Semantic Edge
  Weight, Domain Clustering (as primary layout)...
- **Experimental** (provenance-filtered): Penrose, L-system variants,
  third-party mods...

Each entry shows the config surface inline (at least the top-level
knobs) so the user can see what they're picking, not just a name.

---

## 4. Interaction with existing registries

- `app::graph_layout::LayoutRegistry` currently holds four layouts
  hardcoded in `Default`. That registry evolves to delegate to the
  unified `LayoutRegistry` described here, not to replace it.
- `PhysicsProfileRegistry` handles FR-specific tuning presets. Layouts
  that consume physics profiles declare that in their
  `LayoutCapability`; the host wires profiles in at activation time.
- `LayoutMode` in `registries::atomic::lens` (the `Free / Grid / Tree`
  trichotomy used by lens configs) is a higher-level intent that maps
  to one or more registry entries. Lens still selects intent; registry
  resolves to concrete provider.

---

## 5. First-pass scope

- Introduce `LayoutRegistry` + `LayoutProvider` trait in a new module
  `graphshell::registries::atomic::layout_registry` (or equivalent —
  needs the existing registry layer's owner to pick the location).
- Adapt the eight current built-in `Layout<N>` impls
  (FR, BarnesHut, Radial, Phyllotaxis, Grid, extras, Rapier) as
  `LayoutProvider` instances.
- Surface the registry in the existing Lens config so users can pick
  layouts by ID.
- **Do not yet** expose a "register a third-party layout" public API —
  admission rules + stable `LayoutId` URN scheme need bedding in first.
  Third-party registration lands in the second pass after the built-in
  set has proven the shape.

---

## 6. Open questions

- **Where does the registry physically live?** `graphshell-core`
  (portable), `graph-canvas` (alongside layouts), or `graphshell` proper
  (alongside other registries)? Leaning `graph-canvas` so the registry
  ships with its layouts; the host layer wraps it for user-facing UI.
- **Runtime layout swapping**: should the registry support hot-swapping
  (unregister + re-register under the same ID while a view is using it)?
  Probably not for built-ins; useful for WASM guest reloads during mod
  development.
- **Versioning**: `LayoutId` plus a version tag (e.g.,
  `graph_layout:force_directed@2`)? Saves persisted configs from
  breaking when a layout's config schema evolves.
- **Config migration**: when a layout's `ConfigSchema` changes, how do
  persisted user configs migrate? Needs a layer similar to
  `PhysicsProfile`'s `#[serde(default)]` schema-rev pattern.

---

## 7. Non-goals

- **WASM guest ABI** — handled by the WASM layout runtime plan.
- **Specific layout algorithm implementations** — each layout has its
  own plan lane; this registry is the surface they plug into.
- **Cross-host portability of third-party layouts** — native mods are
  host-specific by default (compile-time); WASM layouts bring portability.
  Not forced here.

---

## 8. Progress

### 2026-04-19

- Plan created alongside the Step-5 design pass. Captures the lane Mark
  identified when reviewing individual layouts: "every/all of these
  layouts [should be] pluggable mod[s], with a set of included defaults."
  Not scheduled for execution — the built-in set is still stabilizing,
  and the registry pattern should bed in before third-party API lands.

- **First-pass landed in `graph-canvas`** later the same day
  ([crates/graph-canvas/src/layout/registry.rs](../../../../crates/graph-canvas/src/layout/registry.rs)).
  Scope delivered:
  - `LayoutId` URN type alias, `LayoutCategory` (Force / Projection /
    Positional / Extras), `LayoutProvenance` (Builtin / NativeMod /
    WasmMod), `LayoutCapability` metadata struct.
  - Object-safe `DynLayout<N>` shim with a blanket impl for every
    `L: Layout<N> + Send` whose `State: Any + Default + Send`. State is
    erased to `Box<dyn Any + Send>` and downcast in `step_dyn`. One
    `Arc<dyn LayoutProvider<N>>` per registered layout; one downcast per
    step.
  - `LayoutProvider<N>` trait + zero-sized `BuiltinProvider<L, N>`
    helper parameterized by a capability-builder function pointer, so
    each built-in registers in one line.
  - `LayoutRegistry<N>` with `empty()` / `register()` / `unregister()`
    / `resolve()` / `capabilities()` / `filter_by_tag()` /
    `filter_by_category()` / `filter_by_provenance()` / `len()` /
    `is_empty()`.
  - `RegisterError::{InvalidId, DuplicateId}`.
  - `Default` impl auto-registers sixteen built-ins: ForceDirected,
    BarnesHut, SemanticEdgeWeight, Grid, Radial, Phyllotaxis, Timeline,
    Kanban, Penrose, LSystem, SemanticEmbedding, DegreeRepulsion,
    DomainClustering, SemanticClustering, HubPull, FrameAffinity. A
    seventeenth (RapierLayout) is registered when the `simulate`
    feature is active.
  - Nine unit tests cover default-registry composition, category
    filtering, tag filtering, provenance filtering, empty-id and
    duplicate-id rejection, unregister removal, and an end-to-end
    resolve-create-step round trip on the `graph_layout:grid` provider.
  - Two small non-registry changes this pass required:
    - `Radial<N>` switched from `#[derive(Default)]` to a manual
      `Default` impl, so `N` is not forced to implement `Default` when
      constructed through the registry. (`RadialConfig<N>` and
      `DomainClustering<N>` already had manual `Default` impls.)

- **Deferred to follow-on passes** (not yet landed):
  - `ConfigSchema` on `LayoutCapability`. The landed capability struct
    omits it; config editing still happens against concrete types until
    a settings-UI surface wants introspection.
  - The `LayoutAlgorithm` / analytic-one-shot category described in
    §3.1. The landed registry covers `Layout<N>` (iterative + delta)
    providers only. Static layouts like Grid are expressed as `Layout<N>`
    impls that emit a full delta in a single step, which covers the
    user-facing use cases at registry granularity. The
    `app::graph_layout::LayoutAlgorithm` registry in the host still
    exists and is untouched.
  - Host-level integration. Lens config still references host-level
    layout IDs directly; wiring the `graph-canvas` registry through to
    the user-facing picker is a separate pass.
  - Third-party registration API + admission-rule enforcement. The
    `register()` path works for any caller today but the URN scheme,
    capability-tag vocabulary, and version tagging described in §6 are
    not yet frozen; third-party usage should wait for that.
  - Versioning (`@N` suffix on `LayoutId`) and config migration. Open
    questions §6 items 3 and 4 stand.

- **Receipts**: `cargo check -p graph-canvas --lib` clean; `cargo test
  -p graph-canvas --lib registry::` 9 passed / 0 failed;
  `cargo test -p graph-canvas --features simulate --lib` 221 passed / 0
  failed; `cargo check --workspace` clean (only pre-existing warnings
  in other crates).
