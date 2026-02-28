# Egui WGPU and Custom Canvas Migration Requirements

**Date:** 2026-02-27  
**Status:** Research baseline (full custom-canvas branch; not the default immediate execution plan)  
**Scope:** What Graphshell must know, decide, and prove before pursuing the full migration path from `egui_glow` + `egui_graphs` to `egui_wgpu` + a Graphshell-owned custom canvas while retaining `egui` + `egui_tiles`.

---

## 1. Decision Baseline

This document describes the **full custom-canvas target branch**, not the minimum default path.

The current canonical execution stance is:

- backend migration (`egui_glow` -> `egui_wgpu`) is tracked separately and remains blocked by embedder/runtime readiness
- custom canvas replacement is conditional and should only advance if `egui_graphs` becomes a proven bottleneck

For the purposes of this research document only, assume the target stack is:

- `egui` for chrome and widget UI
- `egui_tiles` for docking/workbench layout
- `egui_wgpu` as the egui renderer backend
- a Graphshell-owned custom canvas replacing `egui_graphs`

Assume the current stack being replaced is:

- `egui_glow` for egui rendering
- `egui_graphs` for graph canvas rendering and interaction

This document intentionally starts from that stricter target and asks: what must be true before the full migration is safe, coherent, and worth doing?

---

## 2. First Principles

This migration is not "swap a renderer crate and call it done."

It is at least two distinct migrations:

1. **Graph canvas migration**
   - `egui_graphs` -> Graphshell custom canvas
2. **Renderer backend migration**
   - `egui_glow` -> `egui_wgpu`

Those migrations are related, but they are not identical, and they do not have the same blockers.

The graph canvas migration is primarily an architectural and interaction-ownership project.

The renderer backend migration is primarily a graphics/runtime interoperability project.

They should be planned separately, even if they converge into one long-term stack.

---

## 3. Hard Questions That Must Be Answered Before Starting

These are not implementation details. They are gating questions.

### 3.1 Runtime viewer surface interoperability

Graphshell currently has a GL-bound compositor path:

- [gui.rs](../../../shell/desktop/ui/gui.rs) uses `egui_glow::EguiGlow`
- [compositor_adapter.rs](../../../shell/desktop/workbench/compositor_adapter.rs) uses `egui::PaintCallback` plus `egui_glow::CallbackFn`
- that adapter enforces OpenGL state guardrails around composited runtime viewer passes

Before any `egui_wgpu` migration begins, Graphshell must know:

- How do current runtime viewer surfaces become visible in a `wgpu`-backed frame?
- Is there a practical zero-copy path?
- If not, what is the exact copy path?
- What is the latency and frame-time cost of that path?
- What happens under multiple composited panes?
- What happens when tile sizes change every frame?

If this is unknown, the backend migration is not ready.

### 3.2 GPU ownership

Graphshell must decide:

- Does Graphshell own the `wgpu::Instance`, `Adapter`, `Device`, and `Queue`?
- Or does the egui integration layer own them?
- Can all Graphshell rendering subsystems share one device?
- Will future compute workloads (for example AI-adjacent GPU work) share that device?

This decision affects:

- resource lifetime,
- synchronization,
- custom rendering integration,
- debugging,
- future performance strategy.

If device ownership is ambiguous, the future graphics stack will become fragmented.

### 3.3 Presentation strategy for the custom canvas

The custom canvas needs a defined presentation model. There are only two sane initial choices:

1. **Render to texture**
   - Graphshell renders the canvas into a `wgpu::Texture`
   - egui displays that texture in a pane
2. **Render via callback**
   - Graphshell renders directly into the egui frame through backend-specific callback integration

Before migration, Graphshell must decide:

- which is the initial strategy,
- which one is easier to test,
- which one better supports clipping and pane-local composition,
- which one gives the right tradeoff between coupling and latency.

Do not let this stay implicit.

### 3.4 Success criteria

The migration needs measurable success targets before work begins.

Graphshell should define:

- expected frame budget
- target node counts
- target pane counts
- acceptable camera latency
- acceptable drag/selection latency
- acceptable resize behavior
- acceptable GPU memory growth
- acceptable degradation modes

Without this, it is impossible to tell whether the migration solved anything.

### 3.5 Rollback and fallback

Before the first implementation slice, Graphshell should know:

- how to feature-flag the new path,
- how to run the old path in parallel if needed,
- what conditions trigger rollback,
- whether mixed-mode testing is possible,
- how to preserve development momentum if the renderer migration stalls.

If there is no rollback plan, the migration risk is too high.

---

## 4. Comprehensive Requirement Categories

This section lists what must be specified before the migration is considered implementation-ready.

### 4.1 Product and UX requirements

Graphshell should specify:

- Which user-visible problems the migration is intended to solve.
- Which problems are not part of the migration.
- Whether 2D parity is required before any 2.5D / isometric / 3D exploration.
- Whether visual behavior must remain identical at first or may intentionally change.
- Which interactions must remain exact:
  - click-to-select
  - lasso
  - node drag
  - focus-to-open
  - pane focus routing
  - zoom semantics
  - pan semantics
- Which visual affordances must remain:
  - node states
  - hover state
  - focus ring
  - selection markers
  - diagnostics overlays

If the migration objective is not framed as concrete UX goals, the implementation will drift toward technology work for its own sake.

### 4.2 Architecture and authority requirements

Graphshell must define, explicitly:

- what state is authoritative in app code,
- what state may exist only as per-frame render state,
- what state the canvas may cache,
- what state the canvas may never own.

At minimum:

- Graphshell owns semantic graph truth
- Graphshell owns selection truth
- Graphshell owns camera policy
- Graphshell owns focus and active pane ownership
- Graphshell owns render-mode and viewer policy
- Graphshell owns lifecycle and persistence
- The custom canvas may only own ephemeral render caches and per-frame interaction intermediates

If this contract is not explicit, the new canvas will recreate the same dual-authority bugs under a different API.

### 4.3 Rendering and frame-graph requirements

Graphshell must define the intended frame structure:

1. runtime viewer update/composition pass
2. custom graph canvas pass
3. egui chrome pass
4. overlay/diagnostics pass
5. capture/export pass (if needed)

Questions that must be answered:

- What owns pass ordering?
- How are pane-local clips expressed?
- Can the custom canvas render outside a pane? If not, how is clipping enforced?
- Are overlays painted in egui, in the custom canvas, or both?
- How are tooltips handled over GPU-drawn content?
- What is the z-order policy between:
  - graph content
  - composited runtime viewer content
  - tile chrome
  - diagnostics overlays

The migration must produce a more explicit frame graph than the current implicit GL callback path.

### 4.4 GPU and resource requirements

Graphshell must specify:

- texture lifetime rules,
- buffer lifetime rules,
- atlas or cache strategy,
- resource reuse policy on resize,
- multisampling policy,
- sRGB / linear color policy,
- texture format policy,
- depth buffer policy (if any),
- screenshot/export requirements,
- GPU memory budget and eviction strategy.

These are required even if the first implementation is visually simple, because they determine whether the canvas remains sustainable as it grows.

### 4.5 Input and interaction requirements

The new canvas must not inherit ambiguous input behavior.

Graphshell should define:

- exact input routing ownership,
- how a pane captures pointer sequences,
- when the canvas owns scroll versus parent containers owning scroll,
- how keyboard focus is attached to a graph pane,
- whether hover alone may affect app state,
- how drag initiation and drag termination are defined,
- whether lasso selection is canvas-native or app-routed,
- how pointer-to-node hit testing is resolved,
- how cross-pane focus transitions occur.

This is the heart of the architectural value of the custom canvas. If it remains underspecified, the migration is not worth doing.

### 4.6 Workbench integration requirements

Because `egui_tiles` remains in the target stack, Graphshell must define the boundary with it.

`egui_tiles` should continue to own:

- split/tab tree
- pane rectangle computation
- active-tab layout
- docking gestures
- tile chrome hosting

Graphshell must continue to own:

- pane meaning
- pane focus policy
- lifecycle semantics
- viewer dispatch
- compositor scheduling
- persistence invariants

Before migration, Graphshell should define:

- how the custom canvas receives pane rects,
- how pane-local clipping is enforced,
- whether the graph pane is one canvas per pane or one shared canvas with pane projections,
- how tab visibility affects the canvas update loop,
- how background panes throttle rendering.

### 4.7 Persistence and migration requirements

Because `egui_tiles` remains, the layout format can likely stay stable, but Graphshell still must define:

- whether canvas-specific state is persisted,
- what camera state is saved,
- what per-view canvas state is saved,
- what caches are never persisted,
- whether any old `egui_graphs`-specific state must be migrated or discarded,
- whether the new canvas changes undo/redo boundaries.

Persistence must be treated as a first-class contract, not a follow-up cleanup.

### 4.8 Platform and lifecycle requirements

Graphshell must define:

- which platforms are required for the first supported migration release,
- minimum GPU feature assumptions,
- fallback behavior for weak adapters,
- swapchain/surface recreation rules,
- resize handling rules,
- DPI/scaling handling rules,
- device lost / surface lost recovery,
- suspend/resume expectations.

This is especially important for `wgpu`, where platform behavior is part of the engineering cost.

### 4.9 Accessibility requirements

Keeping egui preserves a large part of chrome accessibility, but the canvas remains a policy question.

Graphshell must decide:

- Is the graph canvas primarily visual, with alternate non-canvas navigation?
- Or is the graph canvas itself an accessibility surface?
- If interactive, what semantic entities are exposed?
- How are focused nodes represented?
- How are selection changes announced?
- How is keyboard-only use supported?

If the graph pane is central to the product, this cannot be deferred indefinitely.

### 4.10 Diagnostics and observability requirements

The new path should be easier to reason about than the old one.

Graphshell should require:

- frame timing instrumentation,
- pass timing instrumentation,
- draw-call or batch-count visibility,
- texture upload/copy counters,
- resource residency counters,
- fallback/degradation events,
- debug overlays,
- capture hooks for render debugging.

Without this, the migration will make graphics failures harder to diagnose than they are today.

### 4.11 Testing requirements

Before implementation, Graphshell should know what "correct" means in automated form.

Needed test categories:

- unit tests for scene derivation
- unit tests for hit testing
- unit tests for camera transforms
- interaction conformance tests
- snapshot/golden tests for basic visuals
- integration tests for pane focus and routing
- performance regression checks
- backend smoke tests for device init / surface loss / resize recovery

The custom canvas needs a stronger test story than the current graph widget path, not a weaker one.

### 4.12 Team and process requirements

This migration will fail if it is treated as a purely local refactor.

Graphshell should decide:

- what milestone slices are allowed,
- which slices must be independently shippable,
- how long the dual-path period is allowed to last,
- what constitutes "migration complete,"
- who owns renderer decisions versus UX decisions versus persistence decisions.

If ownership is unclear, the migration becomes open-ended.

---

## 5. What Must Be Researched Before Writing the First Implementation Slice

These are the concrete research tasks that should precede implementation.

### 5.1 Surface-interop spike

Build a narrow proof-of-concept for exactly one composited runtime viewer surface:

- current source surface
- target presentation in a `wgpu`-backed frame
- measure:
  - copies
  - latency
  - resize behavior
  - steady-state frame cost

This spike answers the single most important renderer question.

### 5.2 `egui_wgpu` integration model

Research and choose whether Graphshell should integrate via:

- `egui_wgpu::winit::Painter`
- or `egui_wgpu::Renderer` directly inside a Graphshell-owned render runtime

Current upstream docs make two relevant points:

- `Painter::new` only creates the `wgpu::Instance`
- surface/device initialization happens later via `set_window`

That means window lifecycle integration is a real design concern, not just startup boilerplate.

### 5.3 `wgpu` version and dependency strategy

Research and document:

- the exact `wgpu` version expected by the selected `egui_wgpu` version,
- whether Graphshell will use direct `wgpu` APIs in app code,
- how to avoid version skew between direct `wgpu` use and the egui backend.

Do not allow two incompatible `wgpu` versions into the same migration by accident.

### 5.4 Presentation path trade study

Do a short study comparing:

- texture presentation
- callback presentation

Evaluate:

- coupling to egui internals
- latency
- testability
- resize handling
- clipping complexity
- multi-pass flexibility

Pick one as the initial implementation target and document why.

### 5.5 Custom canvas interaction model

Define, on paper first:

- the input snapshot format,
- the hit-test contract,
- the event-to-intent contract,
- the camera command model,
- the drag/lasso model,
- the hover model.

This is a design task before it is a coding task.

### 5.6 Current feature parity inventory

Inventory everything the current graph path actually does, including:

- camera behavior
- zoom bounds
- search highlighting
- selection visuals
- lasso selection
- group drag behavior
- node hover tooltips
- edge hover tooltips
- focus ring behavior
- graph diagnostics overlays

The migration cannot preserve or intentionally change behavior unless the current behavior is enumerated.

### 5.7 Performance budget research

Define and validate:

- expected draw counts,
- likely node/edge batching strategy,
- text rendering approach,
- icon/thumbnail strategy,
- culling needs,
- background-pane throttling policy.

This avoids building a beautiful but unscalable first version.

### 5.8 Accessibility approach

Research whether the first custom canvas version will:

- expose meaningful interactive regions,
- expose only pane-level semantics,
- rely on alternate non-canvas navigation for accessibility.

Choose deliberately. Do not leave this as accidental behavior.

### 5.9 Rollout plan

Research how to stage:

- compile-time flags,
- runtime toggles,
- diagnostic toggles,
- fallback to current graph path during development.

This determines whether the migration can be safely landed in slices.

---

## 6. What the Custom Canvas Should Be

The custom canvas should be a Graphshell subsystem with a stable contract, not a replacement widget with ad hoc state.

### 6.1 Core subsystem layers

The canvas should be built from six layers.

1. **Scene derivation**
   - Convert Graphshell app state into renderable scene primitives
   - No rendering code here
2. **Camera and projection**
   - Explicit camera state, transforms, and projection rules
   - Start with orthographic 2D
   - Allow future 2.5D / isometric as projection variants
   - Defer true 3D unless product requirements justify the cost
3. **Interaction engine**
   - Hit testing
   - Hover resolution
   - Drag and lasso logic
   - Camera interaction
   - Emits Graphshell intents, does not mutate app truth directly
4. **Render backend**
   - GPU resources, pipelines, draw submission
   - No product semantics here
5. **Presentation bridge**
   - Pane rect + scale + input + scene in
   - rendered output + hover data + emitted intents out
6. **Diagnostics layer**
   - timings, counts, fallback reasons, debug overlays

### 6.2 The canvas must not own app truth

The canvas must not become a second state authority.

It should not own:

- node identity
- semantic graph structure
- selection truth
- lifecycle state
- viewer routing
- persistence policy
- workbench focus policy

Those all remain Graphshell responsibilities.

### 6.3 The canvas should be introduced behind Graphshell-owned interfaces

A practical boundary would look like:

- `GraphCanvasBackend`
- `GraphCanvasScene`
- `GraphCanvasInput`
- `GraphCanvasFrameConfig`
- `GraphCanvasOutput`

The point is not the exact names. The point is:

- Graphshell owns the contract
- backends implement the contract
- the renderer is replaceable without changing product semantics

### 6.4 Start with the simplest correct feature set

The first version of the custom canvas should aim for:

- 2D orthographic graph rendering
- explicit camera ownership
- node and edge rendering
- hit testing
- hover + selection
- drag
- lasso
- tooltips and overlays
- basic culling

It should not begin with:

- true 3D
- ambitious shader effects
- deep animation systems
- speculative GPU physics
- fancy multi-pass visuals that are not required for correctness

The first version should prove architecture and ownership, not visual ambition.

---

## 7. Non-Negotiable Requirements Before Migration Is "Go"

Graphshell should not begin the migration until all of the following are true:

1. The target stack is explicitly accepted: `egui` + `egui_tiles` + `egui_wgpu` + custom canvas.
2. The runtime viewer surface interoperability path is known and measured.
3. GPU device ownership is decided.
4. The custom canvas presentation model is chosen.
5. The custom canvas interaction contract is written down.
6. A current graph feature parity inventory exists.
7. Performance targets are documented.
8. A fallback/rollback strategy exists.
9. The migration is split into independently shippable slices.
10. Diagnostic and testing requirements are defined up front.

If any of these are missing, the migration is still in exploration mode.

---

## 8. Recommended Migration Framing

The best framing is:

1. Treat `egui_graphs` removal as the main architectural goal.
2. Treat `egui_glow` removal as a renderer/backend goal that follows explicit interop proof.
3. Keep `egui_tiles` unless it later becomes a demonstrated blocker.
4. Make the custom canvas the new product-owned surface.
5. Keep the early implementation small, measurable, and reversible.

This sequence produces the best odds of getting a stronger architecture instead of just a newer graphics backend.

---

## 9. Research Notes and External Sources

Primary sources consulted on 2026-02-27:

- `egui_wgpu` crate docs:
  - https://docs.rs/egui-wgpu/latest/egui_wgpu/
- `egui_wgpu::winit::Painter`:
  - https://docs.rs/egui-wgpu/latest/egui_wgpu/winit/struct.Painter.html
- `egui_wgpu::Renderer`:
  - https://docs.rs/egui-wgpu/latest/egui_wgpu/struct.Renderer.html
- `egui_wgpu::Callback`:
  - https://docs.rs/egui-wgpu/latest/egui_wgpu/struct.Callback.html
- `epaint::PaintCallback`:
  - https://docs.rs/epaint/latest/epaint/struct.PaintCallback.html
- `wgpu` crate docs:
  - https://docs.rs/wgpu/latest/wgpu/
- `egui_tiles` crate docs:
  - https://docs.rs/egui_tiles/latest/egui_tiles/
- `egui_tiles::Tree`:
  - https://docs.rs/egui_tiles/latest/egui_tiles/struct.Tree.html
- `egui_tiles::Behavior`:
  - https://docs.rs/egui_tiles/latest/egui_tiles/trait.Behavior.html
- `egui` repository/docs:
  - https://github.com/emilk/egui
  - https://docs.rs/egui/latest/egui/

The Graphshell-specific requirements in this document are an engineering inference from those sources combined with the current code structure.
