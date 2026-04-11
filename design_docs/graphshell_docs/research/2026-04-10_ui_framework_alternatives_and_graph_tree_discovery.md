# UI Framework Alternatives and GraphTree Discovery

**Date**: 2026-04-10
**Status**: Research synthesis
**Scope**: Framework comparison (Makepad, iced, xilem, gpui, slint, custom winit+wgpu),
wgpu-gui-bridge demo findings, crate recommendations, and the discovery that a
graphlet-native tile tree (GraphTree) is the right next architectural step.

**Related**:

- `2026-02-27_egui_stack_assessment.md` — prior egui strategy ("encircle, then replace selectively")
- `2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md` — custom canvas migration
- `../../technical_architecture/graph_tree_spec.md` — GraphTree crate API design (produced from this research)
- `../../technical_architecture/graphlet_model.md` — graphlet semantics
- `../../technical_architecture/unified_view_model.md` — five-domain model
- `../implementation_strategy/navigator/NAVIGATOR.md` — Navigator domain spec
- `../implementation_strategy/workbench/graphlet_projection_binding_spec.md` — graphlet binding
- `../implementation_strategy/graph/2026-03-14_graph_relation_families.md` — relation families
- `../../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md` — host envelope model

**Context**: Graphshell uses egui + egui_tiles + egui_graphs + egui_wgpu. The
existing strategy is "encircle, then replace selectively" — custom graph canvas
first, keep egui for chrome, keep egui_tiles for layout. The wgpu-gui-bridge
project tested Servo embedding across 4 frameworks (winit, xilem, iced, gpui).
This research asks whether those experiments, plus Makepad and other options,
reveal synergies worth pursuing.

---

## 1. What the wgpu-gui-bridge Demos Actually Proved

### Framework-agnostic Servo embedding is solved at the texture layer

The core interop crate (`wgpu-native-texture-interop`) works independently of any
UI framework. Servo content reaches the host via native GPU texture interchange
(Vulkan FD on Linux, IOSurface on Apple, CPU readback on Windows/ANGLE). This means
**the choice of UI framework does not constrain Servo integration** — it's orthogonal.

### Comparative results across frameworks

| Framework | LOC | Strengths | Weaknesses |
|-----------|-----|-----------|------------|
| **winit+wgpu** | 764 | Full GPU control, zero-copy path, minimal overhead | No UI toolkit; everything manual |
| **xilem** | 538 | Reactive view tree, clean async, masonry layout | Young (0.4), `task_raw` Fn impedance |
| **iced** | 449 | Shortest code, Elm-arch clarity, stable (0.14) | Async texture flicker (solved via allocate()) |
| **gpui** | 483 | Good for desktop IDEs, natural animation loop | Pre-1.0, API churn, patch-heavy, custom key types |

### Key lesson

> "Don't try to build a generic Servo webview crate. Build native texture interchange
> as the first layer; let each app solve its own event loop + UI glue."

**Implication for Graphshell**: The Servo embedding layer is already decoupled from
framework choice. A framework switch would NOT require redoing the interop work.

---

## 2. Makepad Assessment

### What Makepad offers

- **GPU-first rendering**: Custom shader-based rendering pipeline, excellent for
  animation-heavy, highly visual UIs
- **Live design DSL**: Rapid iteration on visual design without recompilation
- **WASM support**: Compiles to web targets
- **Mobile support**: iOS and Android targets (stronger than egui's mobile story)
- **Built for custom UIs**: Rik Arends' background in code editors; framework
  designed for apps that don't look like standard widget toolkits

### Where Makepad conflicts with Graphshell's architecture

1. **No tile/docking system**: egui_tiles equivalent doesn't exist. Graphshell's
   entire workbench — tile tree, split/tab/frame arrangement, compositor pass
   scheduling — would need to be rebuilt from scratch.

2. **No graph visualization**: egui_graphs equivalent doesn't exist. But this
   matters less because Graphshell is already planning to replace egui_graphs
   with a custom canvas.

3. **Custom rendering abstractions, not wgpu**: Makepad uses its own GPU
   abstraction layer. Graphshell's architecture assumes it owns the
   `wgpu::Device` and shares it with Servo/WebRender. Makepad's rendering
   pipeline would need careful investigation to determine if wgpu texture
   import/sharing is even feasible.

4. **Opinionated DSL**: Makepad's live-design DSL is powerful but non-standard.
   It's a different programming model from Rust's normal patterns. This adds
   cognitive load and couples deeply to Makepad's ecosystem.

5. **Accessibility**: Makepad's accessibility story is significantly weaker than
   egui + AccessKit. Graphshell targets WCAG 2.2 Level AA with AccessKit
   integration, screen reader support, and deterministic focus order. This is a
   hard requirement.

6. **Smaller ecosystem**: Fewer third-party widgets, less community knowledge,
   harder to hire/onboard contributors.

### The core tension

Makepad's strength (GPU-native custom rendering) overlaps exactly with what
Graphshell is already building custom (the graph canvas). But Makepad would also
force rebuilding everything that egui currently handles well (chrome widgets,
panels, dialogs, file pickers, notifications, accessibility). The cost-benefit
ratio is unfavorable.

### Verdict on Makepad

**Not advisable as a replacement.** The switching cost is enormous (rebuild all
UI chrome, tile tree, accessibility) and the primary benefit (GPU rendering) is
already being captured by the custom canvas strategy. Makepad would be an
interesting choice for a greenfield project with different requirements, but
Graphshell's existing investment and architectural direction don't align.

---

## 3. Other Frameworks Assessed

### iced

- **Fit**: Best alternative IF egui ever becomes untenable for chrome
- **Strengths**: Elm architecture maps well to Graphshell's reducer pattern;
  stable 0.14; wgpu-native; proven in wgpu-gui-bridge demos
- **Weaknesses**: No tile/docking crate; would still need custom graph canvas;
  retained-mode requires rethinking immediate-mode patterns
- **Synergy from demos**: The allocate() texture pre-allocation pattern is
  directly relevant to Servo content display. Cleanest Servo embedding code
  of all frameworks tested (449 LOC)

### xilem

- **Fit**: Most architecturally interesting long-term option
- **Strengths**: Reactive, Linebender ecosystem (parley for text, masonry for
  layout, vello for rendering — all wgpu-native); clean async patterns
- **Weaknesses**: Still 0.4, explicitly early; masonry layout doesn't include
  docking/tiling; ecosystem not mature enough for production
- **Synergy from demos**: Reactive view tree + async frame delivery is
  conceptually clean. Watch this space.
- **Notable**: Linebender's vello renderer and parley text engine are the same
  components that could power a future WebRender alternative

### gpui (Zed's framework)

- **Fit**: Poor for Graphshell's needs
- **Strengths**: Proven in a shipping product (Zed editor); good for IDE-like apps
- **Weaknesses**: Pre-1.0 API churn; !Send model; custom key types incompatible
  with winit; required workspace patches in demos; not designed for external use
- **Verdict**: Not a viable option. Too coupled to Zed's specific needs.

### Fully custom (winit + wgpu + AccessKit)

- **Fit**: Maximum control, maximum cost
- **When**: Only if egui chrome becomes the dominant source of churn
- **The wgpu-gui-bridge winit demo proves this works** — but at 764 LOC for a
  simple URL bar + web view, the extrapolation to Graphshell's full UI (panels,
  command palette, settings, file dialogs, toasts, diagnostics overlays) is
  daunting
- **Verdict**: Keep as the nuclear option. The encirclement strategy already
  moves Graphshell toward owning more of its rendering.

### slint

- **Not tested in wgpu-gui-bridge** (was the inspiration but not directly evaluated)
- **Licensing concerns**: GPL or commercial license for the runtime
- **Strengths**: Structured app UI, declarative markup, good tooling
- **Weaknesses**: Weaker fit for spatial/graph workbench; licensing friction for
  an open-source project

---

## 4. Synchronicities and Non-Obvious Insights

### The real insight: the layers are already decoupling

The wgpu-gui-bridge work and Graphshell's encirclement strategy are converging on
the same architectural truth: **the critical rendering is framework-agnostic**.

- Servo content → native texture interchange → framework-agnostic (proven)
- Graph canvas → custom Graphshell subsystem → framework-agnostic (planned)
- Compositor passes → Graphshell-owned scheduling → framework-agnostic (implemented)
- Authority boundaries → app-owned semantic state → framework-agnostic (in progress)

What remains framework-dependent is:

- **Chrome widgets** (panels, dialogs, settings, command palette)
- **Tile layout rectangles** (egui_tiles computes rects)
- **Input event sourcing** (egui/winit)
- **Accessibility bridge** (egui + AccessKit)

These are exactly the things egui does well and that would be painful to rebuild.

### Linebender ecosystem as future gravity well

The most interesting long-term signal is the Linebender project (xilem + masonry +
vello + parley + AccessKit). These components individually overlap with things
Graphshell needs:

- **vello**: GPU-accelerated 2D rendering (could power custom graph canvas)
- **parley**: Text layout (same as what Servo/WebRender needs)
- **masonry**: Layout engine (could eventually support tiling)
- **AccessKit**: Already used by egui; would carry over

If Linebender matures, it could become a natural migration target — not as a
wholesale switch, but as a gradual adoption of individual components. This is
worth monitoring but not acting on yet.

### The "framework as backend" strategy is the right abstraction

The Feb 2026 assessment's core principle — "Graphshell owns all durable meaning;
the framework owns only ephemeral rendering" — is validated by the wgpu-gui-bridge
experiments. Every demo framework was essentially reduced to: receive a texture,
display it, forward events. That's exactly what "framework as backend" means.

---

## 5. Recommendation

**Stay the course with the current strategy.** The existing plan is well-reasoned
and the wgpu-gui-bridge work validates rather than challenges it:

1. **Don't switch to Makepad** — cost/benefit is unfavorable; GPU rendering
   strength is already captured by the custom canvas plan
2. **Don't switch to any framework right now** — the switching cost dwarfs the
   benefit for all candidates
3. **Continue the encirclement** — custom graph canvas, authority hardening,
   egui_wgpu migration
4. **Monitor xilem/Linebender** — most interesting long-term alternative, but
   not ready yet
5. **Keep iced as the "plan B"** — if egui chrome ever becomes untenable, iced
   is the most viable replacement, with proven Servo embedding from the demos
6. **The wgpu-gui-bridge interop layer is the strategic asset** — it makes any
   future framework migration less costly by keeping Servo integration decoupled

The most productive near-term action, if any, would be to ensure the custom graph
canvas design doesn't accidentally couple to egui internals — making it portable
enough to work with any framework's texture/callback presentation model.

---

## 6. Deep Dive: iced, xilem, and the Graph/Tile Question

### 6.1 Could we improve on the graph and tile tree?

**Graph**: Graphshell already owns its physics (`graphshell_force_directed.rs`,
`barnes_hut_force_directed.rs`, `ActiveLayout`, `PhysicsProfile`). The planned
custom canvas replaces the rendering/hit-testing surface. This is
framework-agnostic by design.

- **fdg** crate: framework-agnostic force-directed layout on petgraph. Viable
  fallback for basic physics, but Graphshell's custom impls are tuned for
  semantic clustering.
- **vello**: the real upgrade path for graph rendering. GPU compute shaders
  enable GPU-native hit testing (which curve/region is under cursor via GPU
  readback) vs. CPU geometric queries. This is a qualitative capability
  difference for a graph canvas with hundreds of nodes.

**Tile tree**: No framework-agnostic tiling/docking crate exists in Rust.
egui_tiles is the best available. egui_dock is binary-split-only. taffy is a
layout engine, not a docking system. In any non-egui framework, tiling must be
built from scratch.

### 6.2 Extension/PWA architectural implications

The portable core (`graphshell-core`) is already UI-framework-agnostic — compiles
to wasm32-unknown-unknown with zero egui/wgpu deps. The framework choice is
per-host-envelope, not global.

**Where iced/xilem have a structural advantage**: Both enforce explicit
message-passing state flow. In an extension context where `GraphIntent` crosses
the JS-WASM boundary as JSON, the Elm/reactive model is the natural fit. egui's
dual-authority risk only exists in the desktop shell (shared process), so this
advantage is real but already mitigated by the portable core's `apply_intents()`
discipline.

**Where the portable core already solves this**: The `GraphIntent` + WAL + JSON
serialization boundary enforces message-passing by construction in non-desktop
hosts. The framework choice for the desktop shell doesn't affect extension/PWA
architecture.

### 6.3 What egui genuinely offers (beyond inertia)

1. **PaintCallback — custom GPU commands in the frame.** egui's three-pass
   compositor works because PaintCallback lets Servo content render directly
   into the egui frame. No separate texture copy for chrome overlays. iced and
   xilem would require the graph canvas to render to a separate texture first.

2. **Zero frame-lag for transient state.** Immediate mode reads current state
   every frame. For physics-driven graph rendering at 60fps where node positions
   change every frame, there's no diffing delay, no retained widget tree to
   update, no stale state. This genuinely matters for the graph surface.

3. **Ecosystem pieces exist.** egui_tiles, egui_graphs (as backend),
   egui-notify, egui-file-dialog, AccessKit — months of work you don't redo.

4. **Weakness is structural.** The dual-authority problem (camera fighting,
   selection mismatch, physics drift) is inherent to immediate mode when
   widgets can silently mutate shared state. Encirclement mitigates but
   doesn't eliminate. This is a permanent tax.

### 6.4 What xilem has over iced AND egui

Three genuinely different things:

1. **Compositional architecture.** xilem (view diffing) + masonry (layout) +
   vello (GPU rendering) + parley (text) + AccessKit (a11y) are independent
   crates. You can use vello without xilem, parley without masonry. This maps
   to Graphshell's "framework as backend" philosophy better than any monolithic
   framework.

2. **vello's GPU compute rendering.** egui: CPU tessellation to vertex buffers.
   vello: GPU compute shaders for parallel path rendering. For the custom graph
   canvas, vello enables GPU-native hit testing, not just rasterization. This
   is a capability neither egui nor iced offer.

3. **Async tasks are structural.** xilem's `task_raw` + `fork` pattern — where
   background Servo frame delivery feeds into the view tree via typed
   messages — is cleaner than egui (no native async) or iced (Task
   abstraction, more coupled). For multi-source frame delivery (Servo,
   physics, network), this matters.

### 6.5 Why not build toward xilem now?

- **xilem is 0.4.** API churn is real. Pre-1.0 rough edges (`task_raw` requires
  `Fn` not `FnOnce`, `OneOf2` type unions, manual masonry event forwarding).
- **No tiling/docking in masonry.** Same gap as iced. Months of work.
- **Vello IS ready (0.6). Xilem is NOT.** The renderer is solid; the framework
  is still finding its shape.

### 6.6 The synthesis: vello as bridge

**Adopt vello now for the custom graph canvas, within the current egui shell.**

vello doesn't need xilem — it's a standalone GPU renderer. Render the graph to
a wgpu texture via vello, present it in egui via PaintCallback or texture handle.
This captures the biggest win (GPU-native graph rendering with compute-shader
hit testing) without any framework switch cost.

When xilem matures to 1.0 and masonry gets richer layout:

- The custom canvas already speaks vello — no rendering rewrite
- Migrating chrome from egui to xilem becomes a UI-only change
- The tile tree could migrate to masonry when it supports split-pane semantics

**vello is the bridge between the current egui shell and a potential future
Linebender stack.** Neither adoption step requires throwing away the other.

### 6.7 Crate recommendations

| Need | Recommendation | Rationale |
|------|---------------|-----------|
| Graph topology | **petgraph** (keep) | Already used, excellent |
| Graph physics | **Keep custom impls** | Tuned for semantic clustering; fdg as fallback |
| Graph rendering | **vello** (adopt for custom canvas) | GPU compute rendering, hit testing |
| Graph labels | **parley** (adopt with vello) | Independent text engine, vello-native |
| Tile layout | **egui_tiles** (keep for now) | Best available; GraphTree replaces long-term |
| Chrome UI | **egui** (keep) | Ecosystem, stability, PaintCallback |
| Accessibility | **AccessKit** (keep) | Carries over to any Linebender migration |
| Future chrome | **xilem** (monitor) | Adopt when 1.0, if egui becomes constraint |
| Tile tree | **GraphTree** (build) | See `graph_tree_spec.md` — graphlet-native replacement |

---

## 7. Key Discovery: GraphTree

The most significant finding from this research is not about frameworks — it's
about the tile tree itself. egui_tiles models spatial geometry (splits, tabs,
proportional shares) but has no concept of graphlet membership, traversal
provenance, lifecycle state, or semantic grouping. Graphshell already layers
workbench semantics on top, but the core data structure doesn't speak graphlet.

A **GraphTree** — a framework-agnostic, graphlet-native tile tree — collapses
the Navigator/Workbench projection gap. The tree IS the navigator when rendered
as tree-style tabs, and IS the workbench when rendered as split panes. Navigator
sections become projection lenses over the same structure.

The full API design is in `../../technical_architecture/graph_tree_spec.md`.
