# Egui Stack Assessment and Strategy for Graphshell

**Date:** 2026-02-27  
**Status:** Background rationale / comparative research (superseded as the primary strategy source)  
**Scope:** Background analysis of how to get the best results from `egui` + `egui_tiles` + `egui_graphs`, what technical boundaries must exist between Graphshell and the egui stack, and what tradeoffs informed the later migration strategy.

**Use this doc for:**

- rationale
- tradeoff analysis
- historical framing

**Do not use this doc as the canonical execution plan.**  
Use `implementation_strategy/aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md` for current migration sequencing and issue posture.

---

## 1. Executive Summary

The current egui stack is still viable for Graphshell, but only if Graphshell treats it as a set of backends rather than as product architecture.

**Historical decision snapshot:** this document was written when the working assumption was that Graphshell would retain `egui` and `egui_tiles`, and eventually migrate away from both `egui_graphs` and `egui_glow`.

That framing is now partially narrowed:

- `egui_glow` -> `egui_wgpu` remains the active backend migration target
- `egui_graphs` replacement is now conditional rather than automatic

The present problems are mostly not "egui is wrong"; they are boundary problems:

- too much Graphshell orchestration still lives inside framework integration files,
- multiple layers can still mutate closely-related state in the same frame,
- `egui_graphs` and `egui_tiles` are both being asked to support workbench semantics they do not natively model.

The right strategy is:

1. **Encircle the current stack first**: keep `egui`, `egui_tiles`, `egui_graphs`, and `egui_glow` only long enough to narrow them to paint/layout/event roles and create stable replacement seams.
2. **Make Graphshell the sole authority for interaction semantics**: focus, camera policy, viewer dispatch, selection semantics, lifecycle, and persistence invariants all belong to app code.
3. **Replace `egui_graphs` with a Graphshell-owned custom canvas**: this is the primary product-surface migration and the core long-term target.
4. **Replace `egui_glow` with `egui_wgpu` once the runtime viewer surface bridge is proven**: the renderer backend migration follows the canvas seam and should not lead it.
5. **Keep `egui_tiles` unless it becomes a proven blocker**: it remains useful as the layout/workbench host far longer than `egui_graphs`.
6. **Avoid long-lived forks unless forced**: prefer thin adapters or upstreamable patches; fork only for short-lived, tightly-scoped gaps.

Rule of thumb:

> Extend, wrap, and constrain while stabilization and core feature delivery dominate.  
> Replace or go custom only when framework constraints become the main source of churn.

---

## 2. Current Stack and What It Really Gives Us

### Current crates in use

From `Cargo.toml`:

| Crate | Version | What Graphshell uses it for |
| --- | --- | --- |
| `egui` | `0.33.3` | Immediate-mode desktop UI, widget chrome, panels, popups, overlays |
| `egui-winit` | `0.33.3` | `winit` input bridge |
| `egui_glow` | `0.33.3` | OpenGL-backed egui renderer |
| `egui_graphs` | `0.29.0` | Graph widget, force-directed layout state, node/edge events |
| `egui_tiles` | `0.14.1` | Docking/tiling tree layout |
| `egui-file-dialog` | `0.12.0` | File picker |
| `egui-notify` | `0.21` | Toast notifications |

### What the code already does correctly

The codebase already contains the right architectural instinct in several places:

- [render/mod.rs](../../../render/mod.rs) disables `egui_graphs` zoom/pan (`with_zoom_and_pan_enabled(false)`) and routes camera movement through Graphshell's custom navigation path.
- [render/mod.rs](../../../render/mod.rs) uses an event sink (`Rc<RefCell<Vec<Event>>>`) and converts framework events into Graphshell actions instead of mutating most app state inline.
- [shell/desktop/workbench/tile_compositor.rs](../../../shell/desktop/workbench/tile_compositor.rs) already treats rendering/compositing as Graphshell-owned logic layered on top of `egui_tiles`, not something `egui_tiles` itself can provide.
- [model/graph/egui_adapter.rs](../../../model/graph/egui_adapter.rs) already acts as a useful boundary between Graphshell graph state and `egui_graphs`.

That means Graphshell does not need a panic rewrite. It needs stricter ownership boundaries.

### What the current code makes difficult

The largest migration blocker is not `egui_tiles`; it is the current renderer/compositor coupling:

- [shell/desktop/ui/gui.rs](../../../shell/desktop/ui/gui.rs) is currently built around `egui_glow::EguiGlow`.
- [shell/desktop/workbench/compositor_adapter.rs](../../../shell/desktop/workbench/compositor_adapter.rs) is explicitly OpenGL-bound and uses `egui_glow::CallbackFn` plus GL state guardrails.
- Current composited runtime viewer content is therefore not just "egui-rendered"; it is wired into a GL callback path with concrete OpenGL invariants.

That means the first serious technical question in any `egui_glow` -> `egui_wgpu` migration is:

> How do current runtime viewer surfaces cross the GL -> `wgpu` boundary without unacceptable latency, copies, or complexity?

Until that is answered, the renderer migration is not ready.

### What the upstream crates are actually designed for

Upstream source review reinforces the same conclusion:

- `egui` is optimized for immediate-mode UI composition and custom widget authoring, not for being a domain-specific scene engine.
- `egui_tiles` is a docking/layout crate centered on a `Tree` plus a `Behavior` implementation; it is not a workbench runtime or persistence authority.
- `egui_graphs` is a graph widget; it bundles rendering, interaction, and layout helpers. It is not a spatial browser/workbench engine.

Those crates are useful, but they are not substitutes for Graphshell's domain model.

---

## 3. Real Risks of Staying on `egui` + `egui_tiles` + `egui_graphs`

### 3.1 Dual-authority bugs

This is the main practical risk today.

When Graphshell and the framework both believe they may mutate related state in the same frame, "fighting" bugs appear:

- camera jitter or camera drift,
- selection mismatches,
- drag state desynchronization,
- focus moving to a visually stale pane,
- physics state appearing to lag or snap.

Current high-risk zones:

| State | Framework can hold/mutate | Graphshell can hold/mutate | Risk |
| --- | --- | --- | --- |
| Camera | `egui_graphs::MetadataFrame` | `GraphViewFrame` / camera commands | Camera "fighting" |
| Physics | `egui_graphs` layout state | `GraphWorkspace.physics` and per-view simulation | Drift / hidden writes |
| Selection | Graph widget internal selection/hover state | `selected_nodes` | Divergence after complex sequences |
| Focus | egui response focus and hover state | workbench focus + active pane state | Input routed to wrong owner |
| Tile layout | `egui_tiles::Tree` runtime tree | app policies and persistence reconstruction | Structural mismatch |

If Graphshell does not enforce one authority per state category, every new feature increases the chance of subtle regressions.

### 3.2 Abstraction mismatch over time

`egui_graphs` and `egui_tiles` are generic UI crates. Graphshell is not building a generic graph viewer or generic docking UI.

Graphshell-specific semantics that do not naturally fit widget assumptions:

- workbench lifecycle (`Active` / `Warm` / `Cold`),
- viewer routing and render-mode dispatch,
- tile compositor pass scheduling,
- graph-as-browser semantics,
- multi-view graph policies,
- persistence invariants over tile trees and frame state,
- explicit authority boundaries between semantic graph, spatial layout, and runtime instances.

The longer these semantics are expressed as ad hoc code inside widget adapters, the more the product model gets squeezed into the framework's default shape.

### 3.3 Upgrade drag

This risk is manageable today because Graphshell is still on upstream crates with no long-lived local forks.

It becomes expensive if:

- Graphshell starts patching internals of `egui_graphs` or `egui_tiles`,
- app logic depends on undocumented crate behavior,
- large integration modules absorb crate-specific assumptions everywhere.

Upgrade drag is therefore mostly a **boundary discipline** problem, not yet a dependency problem.

### 3.4 Performance and rendering ceiling

This is not the current top blocker, but it is the medium-term ceiling:

- `egui` is immediate mode, so the app rebuilds UI descriptions every frame.
- `egui_glow` is OpenGL-oriented and not ideal for a future deeply custom graph render pipeline.
- `egui_graphs` is not a full custom scene renderer; it is a widget with layout helpers.
- Graphshell's most distinctive future needs (graph-native compositing, richer physics, custom hit testing, more explicit render passes) will eventually outgrow a widget-oriented graph layer.
- The current compositor path is tied to OpenGL callback semantics, which raises the cost of moving the UI renderer to `wgpu` until a practical surface-interop plan exists.

That is why the graph canvas is the first planned replacement, while `egui_tiles` remains in the target stack.

---

## 4. What Should Be Different in the Egui Stack (From Graphshell's Perspective)

If the upstream stack were ideal for Graphshell, it would offer three things more explicitly:

### 4.1 First-class single-authority input and camera ownership

Graphshell needs stronger primitives for:

- "this region owns drag semantics right now,"
- "this pointer sequence is captured by this subsystem,"
- "camera updates come from one path only."

Without that, Graphshell must manually prevent duplicate behavior through policy gates and careful event ordering.

### 4.2 Stronger separation between rendering primitives and interaction behavior

Graphshell benefits when a framework can be used as:

- a pure renderer,
- a pure hit-test/event source,
- or a pure layout helper,

without also pulling in built-in interaction policy.

`egui_graphs` is useful, but it couples those concerns more tightly than Graphshell ideally wants.

### 4.3 Explicit extension points for docking/workbench semantics

Graphshell needs workbench-specific concepts that generic docking crates do not define:

- pane activation/deactivation contracts,
- compositor scheduling hooks,
- tile lifecycle cleanup,
- persistence invariants and structural validation,
- focus transfer and cross-pane routing rules.

Those extension points are not really "missing features" in `egui_tiles`; they are product-specific runtime semantics. Graphshell must own them explicitly.

---

## 5. What We Can Improve Without Going Custom

This is the highest-value near-term work.

### 5.1 Define strict authority contracts per subsystem

Graphshell should formalize one owner per state category.

Recommended boundary:

| Concern | Graphshell owns | egui stack owns |
| --- | --- | --- |
| Semantic graph | Nodes, edges, metadata, lifecycle, selection truth | Nothing |
| Camera policy | Bounds, commands, focus-target behavior, persistence | Ephemeral per-frame widget metadata only |
| Graph interaction semantics | Click meaning, lasso rules, multi-select rules, drag semantics | Raw pointer/key observations and widget-local hit results |
| Physics policy | Config, presets, per-view simulation ownership, persisted state | Temporary layout engine state bridge only |
| Workbench semantics | Pane identity, focus, active tile, routing, lifecycle, persistence invariants | Tile rect/layout computation only |
| Viewer routing | Viewer ID selection, render mode policy, degradation rules | Nothing |
| Compositor | Pass ordering, overlay rules, surface scheduling | Nothing |
| UI chrome content | App meaning, commands, settings state | Widget drawing, layout helpers, ephemeral widget state |

The principle is simple:

- **Graphshell owns all durable meaning.**
- **The egui stack owns only ephemeral rendering and widget-local mechanics.**

### 5.2 Keep framework as backend

This is the most important implementation rule.

The app should:

- compute policies,
- derive the current scene,
- route intent,
- decide semantic outcomes,
- persist durable state.

The framework should:

- draw UI,
- provide layout rectangles,
- emit raw widget/graph events,
- store strictly local transient frame data where unavoidable.

In practice:

- `egui_graphs` should be treated as a graph canvas backend, not as a graph authority.
- `egui_tiles` should be treated as a tile layout backend, not as a workbench authority.
- `egui` should be treated as the chrome UI toolkit, not as the owner of interaction semantics.

### 5.3 Add conformance tests before adding features

Before adding new interaction complexity, Graphshell should lock down existing invariants.

Required test coverage:

- camera never escapes bounds across frame sequences,
- custom navigation is the only path that changes the effective camera policy,
- pinned nodes do not move during drag/physics sequences,
- selection truth remains synchronized after click/drag/lasso sequences,
- tile tree invariants hold after split/merge/close/open operations,
- pane focus transfer is deterministic after close or retarget,
- physics state round-trips through the layout bridge without silent drift.

These tests make framework integration safer by detecting the exact category of bug that "dual authority" causes.

### 5.4 Minimize ad hoc patches

There should be only two acceptable integration styles:

1. **Upstreamable changes**
2. **Thin local adapters**

There should not be a third category where Graphshell quietly depends on deep crate internals spread across large modules.

That means:

- avoid scattering crate-specific workarounds through broad orchestration files,
- isolate framework-specific logic into bridge modules,
- keep any local patch small, documented, and easy to delete.

### 5.5 Break up the existing integration chokepoints

Two files currently hold too much mixed responsibility:

- [render/mod.rs](../../../render/mod.rs)
- [shell/desktop/workbench/tile_behavior.rs](../../../shell/desktop/workbench/tile_behavior.rs)

Recommended split for the graph path:

- `render/graph_canvas_backend.rs`
- `render/graph_camera_bridge.rs`
- `render/graph_event_bridge.rs`
- `render/graph_selection_bridge.rs`
- `render/graph_physics_bridge.rs`

Recommended split for the workbench path:

- `shell/desktop/workbench/tile_layout_backend.rs`
- `shell/desktop/workbench/focus_router.rs`
- `shell/desktop/workbench/pane_dispatch.rs`
- `shell/desktop/workbench/tile_persistence_contract.rs`

This is how Graphshell stops "framework adapters" from becoming de facto controllers.

---

## 6. What a Custom Stack Uniquely Unlocks

Going custom is not automatically better. It is only justified when it solves the problems the framework keeps reintroducing.

A Graphshell-owned custom layer uniquely unlocks:

### 6.1 Full control of interaction semantics and timing

No hidden defaults, no widget-provided drag semantics, no ambiguity over who consumed a pointer sequence.

This matters when Graphshell wants:

- richer graph gestures,
- custom camera animation contracts,
- multi-modal input routing,
- exact focus/selection transitions,
- domain-specific interaction timing.

### 6.2 Domain-native layout and physics contracts

A custom graph canvas can be built around Graphshell's actual needs:

- graph view as a first-class scene,
- custom hit testing and culling,
- richer layout models than a generic graph widget provides,
- physics that reflect Graphshell's workbench semantics instead of widget assumptions.

### 6.3 Predictable long-term evolution

When UX/spec is novel and central to product identity, owning the critical rendering and interaction layer removes a class of future blockers:

- upstream API changes stop breaking core product behavior,
- framework constraints stop shaping feature design,
- performance and rendering decisions become product-driven rather than crate-driven.

This is the main long-term argument for a custom graph canvas.

---

## 7. Strategic Options

Before choosing an option, it is important to separate three different migrations:

1. **Renderer backend migration**: `egui_glow` -> `egui_wgpu`
2. **Graph canvas migration**: `egui_graphs` -> Graphshell custom canvas
3. **Workbench layout migration**: `egui_tiles` -> custom docking/layout engine (deferred unless `egui_tiles` becomes a proven blocker)

These should not be treated as one rewrite. They have different risks, different dependencies, and different rollback paths.

### Option A: Transitional hardening of the current stack

**Keep temporarily:** `egui` + `egui_tiles` + `egui_graphs` + `egui_glow`  
**Change:** Integration discipline and ownership boundaries

This is the best short-term move because it:

- preserves current delivery velocity,
- reduces bug classes immediately,
- creates replacement seams before any rewrite,
- avoids premature framework churn.

This is the recommended immediate path, but only as a staging state.

### Option B: Keep `egui` and `egui_tiles`, replace `egui_graphs`

**Keep:** `egui`, `egui_tiles`  
**Replace:** `egui_graphs` with a Graphshell-owned graph canvas backend

This is the most likely medium-term path because:

- the graph canvas is where Graphshell diverges most from generic widget assumptions,
- the workbench tree is still reasonably served by `egui_tiles`,
- the graph path is where custom rendering/interaction control yields the most product value.

This is the recommended first major replacement and the chosen canvas direction.

### Option B.1: Keep `egui` and `egui_tiles`, move to `egui_wgpu`, then replace `egui_graphs`

**Keep:** `egui`, `egui_tiles`  
**Migrate:** `egui_glow` -> `egui_wgpu`  
**Replace:** `egui_graphs` with a Graphshell-owned graph canvas backend

This is the likely long-term target if Graphshell wants a unified `wgpu` render path, but the sequencing should be reversed in implementation planning: establish the custom canvas seam first, then do the backend swap.

This path only makes sense after Graphshell has answered the GL -> `wgpu` compositor question for runtime viewer surfaces. If that answer is poor (for example, excessive copies or unstable interop), the backend swap should wait while the graph canvas and backend seams are prepared first.

### Option C: Transitional encirclement

This is the "encirclement" path:

- keep `egui_tiles` while treating `egui_graphs` and `egui_glow` as legacy integration layers to be replaced,
- disable more default behavior,
- move more meaning into Graphshell-owned controllers,
- allow the framework layer to shrink to backends over time.

This is effectively Option A executed in stages, and it is the correct transition path even if Graphshell later chooses Option B.

### Option D: Fork one or both subcrates

Possible, but costly.

#### Fork `egui_graphs` only

This is acceptable as a short-lived bridge if:

- Graphshell needs one missing API or bug fix,
- the diff stays small,
- the patch can plausibly be upstreamed or removed quickly.

#### Fork `egui_tiles` only

Lower-value first move. `egui_tiles` is not the main current architectural mismatch.

#### Fork both and "combine" them

Technically possible. Strategically weak.

The overlap between `egui_graphs` and `egui_tiles` is not where Graphshell's real architecture belongs. Their "redundancy" is mostly widget-level assumptions. The correct place to unify behavior is not inside a combined fork; it is inside Graphshell-owned controller and scene layers.

This is the highest-maintenance option and should be avoided unless Graphshell is already committed to long-term internal ownership of both forks.

### Option E: Switch frameworks entirely

This should only happen if the egui stack becomes the main source of churn.

Plausible alternatives:

- **`iced`**: stronger message/update architecture, better fit for explicit state flow, but still immature for Graphshell's custom workbench needs and offers no drop-in equivalent to the current graph + docking combination.
- **`xilem`**: interesting to watch, but still explicitly early and not a practical replacement target today.
- **`makepad`**: stronger custom rendering posture, but much more opinionated and less aligned with Graphshell's current architecture.
- **`slint`**: strong for structured application UI, weaker fit for a graph-heavy spatial workbench and carries licensing/product tradeoffs.
- **Fully custom (`winit` + renderer + AccessKit + Graphshell UI systems)**: maximum control, maximum cost.

A full framework switch is not the recommended near-term move; the preferred target remains `egui` + `egui_tiles` + `egui_wgpu` + a Graphshell-owned custom canvas.

---

## 8. Pre-Migration Research and Decision Gates

Before attempting a renderer or canvas migration, Graphshell should answer these questions explicitly.

### 8.1 Renderer and surface interoperability

- Can current runtime viewer content be presented in a `wgpu`-backed frame without unacceptable copies?
- Is there a practical zero-copy or low-copy path from the current GL-oriented runtime viewer surfaces into the future render path?
- If not, what is the measured cost of CPU readback + re-upload or GPU copy bridges?
- What is the acceptable per-frame budget for that bridge under normal multi-pane use?

This is the hard technical gate for `egui_glow` -> `egui_wgpu`.

### 8.2 Device ownership and versioning

- Will Graphshell own the `wgpu::Instance` / `Adapter` / `Device` / `Queue`, or will the egui integration layer own them?
- Can all desired render and compute subsystems share one coherent `wgpu` device?
- Will Graphshell pin direct `wgpu` usage to the version expected by `egui_wgpu` to avoid split dependency trees?

These answers determine whether the future graphics stack is coherent or fragmented.

### 8.3 Custom canvas presentation model

- Will the custom canvas render to an offscreen texture, then present that texture in egui?
- Or will it render directly into the egui frame via backend-specific callback integration?
- Which model gives the best tradeoff for latency, testability, and backend coupling?

This choice should be made intentionally early.

### 8.4 UX, performance, and platform targets

- What frame-time budget must the custom canvas meet?
- What node-count and pane-count targets must it support?
- Which platforms must be first-class on day one?
- What are the acceptable degradation modes if GPU budgets are exceeded?

Without these targets, the migration cannot be evaluated honestly.

### 8.5 Accessibility, diagnostics, and rollback

- What accessibility obligations apply to the graph surface?
- How will the custom canvas expose diagnostics for frame timing, copy counts, and degraded modes?
- What feature flags or fallback paths allow rollback if the new path regresses?

These are product requirements, not optional polish.

### 8.6 No-go condition

Graphshell should not start the backend swap until it can answer this with confidence:

> How does a current runtime viewer surface, which today is composed through a GL callback path, become visible in the new `wgpu` frame without an unacceptable latency or bandwidth penalty?

If that answer is unknown, the migration should remain in research/spike mode.

---

## 9. What the Custom Canvas Should Be

The custom canvas should be a Graphshell subsystem, not just "another widget."

### 9.1 It should contain these layers

1. **Scene layer**
   - Graphshell-derived renderable scene data only
   - nodes, edges, labels, overlays, thumbnails, diagnostics primitives
2. **Camera/projection layer**
   - explicit camera state and projection modes
   - begin with orthographic 2D, then support projected 2.5D / isometric, then true 3D only if justified
3. **Interaction layer**
   - hit testing, lasso, drag, hover, camera navigation
   - consumes input snapshots and emits intents
4. **Render backend layer**
   - resource caches, GPU buffers, textures, pipelines
   - texture or callback presentation path
5. **Presentation bridge**
   - consumes pane rect, scale factor, input snapshot, and scene
   - returns render output plus emitted intents/hover state
6. **Diagnostics layer**
   - frame timings, draw counts, copy counts, fallback/degradation events

### 9.2 It should not own these things

The custom canvas should not become a second app model. It must not own:

- semantic graph truth,
- node identity,
- selection truth,
- pane lifecycle policy,
- viewer routing,
- persistence,
- command semantics outside the canvas interaction contract.

### 9.3 It should be introduced behind a stable Graphshell boundary

The cleanest future seam is a Graphshell-owned interface such as:

- `GraphCanvasBackend`

fed by Graphshell-owned inputs such as:

- `GraphCanvasScene`
- `GraphCanvasInput`
- `GraphCanvasFrameConfig`

This lets Graphshell prepare the seam now while still using the existing egui stack.

---

## 10. Technical Boundaries Graphshell Should Enforce

This is the core contract.

### Graphshell must control

These are product authorities and should not be delegated:

- semantic graph state,
- node and edge identity,
- viewer ID selection,
- render mode policy,
- focus and active pane ownership,
- pane open/close/split semantics,
- tile lifecycle semantics,
- camera policy and persistence,
- selection truth,
- command interpretation,
- physics policy and persisted configuration,
- compositor scheduling and pass ordering,
- degradation/fallback policy,
- persistence and invariants.

If one of these is currently implicit in a widget callback or framework state object, that is a boundary leak.

### The egui stack may control

These are acceptable framework responsibilities:

- widget layout,
- widget painting,
- immediate-mode UI composition,
- ephemeral hover/focus state local to a widget,
- raw input observation,
- tile rectangle computation,
- widget-local drag/hover geometry,
- graph widget-local render caches,
- per-frame temporary layout metadata used strictly as a backend bridge.

These are implementation mechanics, not product semantics.

### What must never be ambiguous

At any point in time, it should be obvious:

- who owns the truth,
- who is allowed to mutate it,
- whether a framework value is authoritative or only a cache/bridge,
- whether a given callback is allowed to make semantic decisions or only emit events.

If that is unclear, the code will drift back toward dual-authority bugs.

---

## 11. Recommended Near-Term Strategy

### Phase 1: Harden boundaries on the current stack

1. Document the ownership matrix in architecture docs.
2. Add debug assertions for illegal state changes.
3. Extract bridge modules from large integration files.
4. Expand interaction conformance tests.
5. Treat all framework callbacks as event sources, not semantic authorities.

### Phase 2: Build explicit backend interfaces inside Graphshell

Introduce Graphshell-owned interfaces such as:

- `GraphCanvasBackend`
- `TileLayoutBackend`
- `InteractionRouter`
- `FocusAuthority`
- `WorkbenchScene`

Then implement them using the current egui stack.

This gives Graphshell clean seams without an immediate rewrite.

### Phase 3: Replace the graph canvas

The planned first replacement is:

- replace `egui_graphs`,
- keep `egui`,
- keep `egui_tiles` longer.

This preserves the useful parts of the stack while moving Graphshell's most product-specific surface under full local control.

### Phase 4: Only then decide on the renderer backend swap

After the custom canvas seam is stable:

- prove the runtime viewer surface bridge,
- choose device ownership,
- select texture-vs-callback presentation,
- then migrate `egui_glow` -> `egui_wgpu`.

This sequencing isolates the real risk and avoids turning the backend swap into a blind dependency churn project.

---

## 12. Decision Threshold for Going More Custom

Graphshell should stay on the current stack while:

- the main bugs are still boundary/ownership bugs in Graphshell code,
- the stack still accelerates delivery,
- framework constraints are annoying but not the dominant source of churn.

Graphshell should move toward a custom graph canvas when:

- core UX keeps being distorted to fit widget assumptions,
- adapter code becomes larger than the value the widget provides,
- framework workarounds dominate graph-canvas work,
- performance/rendering requirements clearly exceed what the widget path can do cleanly.

Graphshell should consider a fuller custom UI stack only if:

- even the chrome layer becomes blocked by framework constraints,
- or the maintenance cost of the entire stack outweighs its delivery advantage.

That threshold has not been reached yet.

---

## 13. Research Notes and Upstream References

Primary sources consulted on 2026-02-27:

- `egui` docs and repository:
  - https://docs.rs/egui/latest/egui/
  - https://github.com/emilk/egui
- `egui_tiles` docs and repository:
  - https://docs.rs/egui_tiles/latest/egui_tiles/
  - https://github.com/rerun-io/egui_tiles
- `egui_graphs` docs and repository:
  - https://docs.rs/egui_graphs/latest/egui_graphs/
  - https://github.com/blitzarx1/egui_graphs
- `iced` docs and repository:
  - https://docs.iced.rs/iced/
  - https://github.com/iced-rs/iced
- `xilem` repository and docs:
  - https://github.com/linebender/xilem
  - https://xilem.dev/
- `makepad` repository:
  - https://github.com/makepad/makepad
- `slint` docs:
  - https://slint.dev/

These sources are useful for understanding current crate scope, intended use, and maturity. The Graphshell-specific recommendation in this document is an inference from those sources combined with the current code structure.
