# Egui WGPU and Custom Canvas Migration Strategy

**Date**: 2026-02-27  
**Status**: Implementation strategy  
**Relates to**:

- `../research/2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md`
- `../research/2026-02-27_egui_stack_assessment.md`
- `2026-02-22_multi_graph_pane_plan.md`
- `2026-02-26_composited_viewer_pass_contract.md`
- `workbench_frame_tile_interaction_spec.md`

---

## Goal

Split the previously combined `wgpu` migration into two distinct workstreams:

1. **Renderer backend migration**
   - `egui_glow` -> `egui_wgpu`
2. **Graph canvas migration**
   - `egui_graphs` -> a Graphshell-owned custom canvas

while intentionally retaining:

- `egui`
- `egui_tiles`

for UI chrome and workbench layout.

This strategy assumes:

- no external users,
- no compatibility promise,
- no obligation to preserve prototype behavior while rebuilding,
- willingness to make hard cuts and delete old systems as soon as replacement seams are proven.

The objective is not "safe incrementalism." The objective is a cleaner architecture fast enough to keep momentum, while still respecting the real technical dependencies that can make a renderer migration fail.

These two workstreams are related, but they are not the same issue, should not share the same blocker model, and should not be forced into one execution slice.

---

## Deferral Decision

The renderer backend migration (`egui_glow` -> `egui_wgpu`) is explicitly **deferred behind application maturity and embedder decomposition readiness**.

That means:

- Graphshell should first fix its current architecture, UX, and core feature-set problems on the existing stack.
- Graphshell should first become meaningfully usable as an application.
- Only after the app logic, ownership boundaries, baseline interactions, and embedder/runtime boundaries are stable enough should Graphshell attempt the renderer backend migration from `egui_glow` to `egui_wgpu`.

Reason:

- If Graphshell's app logic is still fighting `egui_graphs`, `egui_tiles`, and `egui_glow` today, moving to `wgpu` too early is likely to reproduce the same architectural mistakes under a newer backend.
- `wgpu` is not the fix for unclear authority, weak interaction contracts, or immature application semantics.
- The backend migration is primarily blocked by `lane:embedder-debt` (`#90`) because the current compositor and host/UI boundary are still too GL-specific.
- The custom canvas migration remains a conditional architectural option, not an automatic prerequisite for the backend swap.

The practical consequence is:

- **Near term:** stabilize the app, clarify ownership, and close the most important UX and correctness gaps.
- **Medium term:** only replace `egui_graphs` if it becomes a proven product or performance bottleneck.
- **Later:** migrate `egui_glow` -> `egui_wgpu` once the app is structurally stable and `lane:embedder-debt` has cleared the backend cut.

---

## Migration Doctrine

This plan follows five rules.

### 1. Break prototype behavior freely, but not blindly

Graphshell can tolerate:

- broken visuals during intermediate phases,
- temporary loss of non-core graph affordances,
- reset of non-essential persisted UI/canvas state,
- deletion of low-value compatibility shims,
- redefinition of internal-only APIs.

Graphshell should not tolerate:

- unclear ownership of state,
- indefinite dual runtime paths,
- hidden technical debt carried "for later,"
- migration slices that cannot be validated.

### 2. Replace architecture, not just dependencies

This is not a crate-swap exercise.

The target is:

- Graphshell-owned canvas semantics,
- Graphshell-owned render pipeline shape,
- Graphshell-owned interaction authority,

with `egui_wgpu` serving as a backend, not as the new architecture.

### 3. Keep `egui_tiles` until it proves itself to be the blocker

`egui_tiles` remains useful as:

- pane tree,
- split/tab layout host,
- pane rect source,
- tile chrome host.

Do not expand scope by rebuilding docking/layout at the same time unless the canvas migration proves that `egui_tiles` itself is the next hard constraint.

### 4. Delete old paths quickly

The mainline branch should not carry long-lived dual implementations of:

- old graph canvas vs new graph canvas,
- old renderer backend vs new renderer backend,
- old interaction logic vs new interaction logic.

Research spikes can be isolated, but once the chosen path is validated, old code should be removed aggressively.

### 5. Respect dependency-correct sequencing

Even in a prototype, order still matters:

- Graphshell can hard-break behavior.
- Graphshell should not start the `egui_wgpu` swap before the application is stable enough to justify it.
- Graphshell should not start the `egui_wgpu` swap before the runtime viewer surface bridge is understood.

Those are the major sequencing constraints this strategy refuses to ignore.

---

## Target End State

The default planned end state should be:

1. `egui` remains the chrome/UI toolkit.
2. `egui_tiles` remains the workbench layout host.
3. `egui_wgpu` renders egui on top of a Graphshell-owned `wgpu` runtime.
4. The graph surface is either:
   - still rendered through `egui_graphs`, if that remains sufficient, or
   - rendered by a Graphshell-owned custom canvas subsystem, if the replacement is justified.
5. `egui_glow` is removed from the dependency graph.
6. Graphshell owns interaction semantics, scene derivation, and frame-pass ordering.

If the custom-canvas migration is activated later, a stricter long-term end state also removes `egui_graphs`.

---

## Explicit Non-Goals

This migration does not initially try to:

- replace `egui_tiles`,
- start the `egui_wgpu` backend swap before Graphshell is usable as an application,
- deliver true 3D as part of the first working canvas,
- preserve all current graph visuals during the first hard cut,
- preserve prototype-only persisted canvas state if it complicates the migration,
- optimize everything before correctness is re-established.

The first target is architectural control and stable 2D correctness.

---

## Allowed Breaking Changes

Because this is a prototype rebuild, these are explicitly allowed:

1. Delete `egui_graphs`-specific adapters and state instead of translating them.
2. Reset or invalidate graph-pane visual state if it was tied to widget internals.
3. Simplify the graph feature set temporarily while the new canvas is established.
4. Remove old test snapshots tied to `egui_graphs` behavior and replace them with new contracts.
5. Change internal module structure freely.
6. Change persistence for graph-pane view state if maintaining the old format slows the migration.
7. Temporarily reduce rendering fidelity while re-establishing ownership boundaries.

These are not accidents. They are part of the strategy.

---

## Non-Negotiable Preconditions

Before Phase 1 implementation begins, Graphshell needs:

1. A decision that `egui_tiles` stays for this migration.
2. A decision that the combined migration umbrella is split into separate backend and canvas tracks.
3. A short proof-of-concept plan for the runtime viewer GL -> `wgpu` bridge.
4. A chosen owner for the future `wgpu` device/queue.
5. Agreement that feature parity may temporarily regress during the backend cutover.

Only if the custom-canvas issue is activated:

6. A chosen initial custom-canvas presentation model:
   - render-to-texture, or
   - direct callback

If any of these are unresolved, the migration remains in research mode.

---

## Application Readiness Gate

Before Graphshell begins the backend migration to `egui_wgpu`, the app should first satisfy a practical readiness bar on the current stack.

Minimum readiness expectations:

1. Core architecture is coherent enough that ownership boundaries are explicit and enforced.
2. Core UX flows are usable without constant framework-fighting regressions:
   - graph camera/navigation works reliably
   - pane focus and first-render activation are deterministic
   - node/tile creation semantics are Graphshell-owned and consistent
3. The basic feature set is complete enough to function as an application rather than as a fragile prototype shell.
4. Current-stack bugs are no longer dominating everyday development.
5. The team can evaluate the renderer/backend migration as an upgrade, not as an act of desperation.

This gate exists to prevent the backend migration from becoming a distraction from fixing app-level problems that would survive any renderer swap.

---

## Migration Runway (What Must Happen Before The Cut)

This is the practical runway before the migration starts in earnest.

The point of the runway is:

- remove ambiguity,
- reduce avoidable architectural blast radius,
- stop spending effort on old-path fixes that the migration is about to delete,
- identify which current issues are true prerequisites versus likely-to-be-superseded work.

Because `egui_wgpu` is now explicitly deferred, this runway should be read as:

- what to prepare before the eventual hard cut,
- not what must be executed immediately.

### Runway Category A: Hard gate before backend migration

This is the one real blocker for the `egui_glow` -> `egui_wgpu` cut:

- **#180** prove the runtime-viewer GL -> `wgpu` bridge

Why it matters:

- The current runtime-viewer composition path is explicitly GL-bound.
- Until Graphshell knows how current runtime viewer surfaces enter a `wgpu` frame, the backend swap is not technically ready.

Rule:

- Do not start the renderer backend cut before:
  - the application readiness gate is met, and
  - `#180` is answered with measured evidence.

### Runway Category B: Strongly recommended structural prep

These are not formal blockers, but they materially reduce migration risk:

- **#181** extract `GraphCanvasBackend` seams to decouple graph-surface ownership from `egui_graphs`
- **#118** split `gui.rs` responsibilities and reduce `RunningAppState` coupling
- **#119** split `gui_frame.rs` responsibilities

Why they matter:

- They reduce the size of the cut.
- They isolate renderer-specific logic from orchestration logic.
- They make it possible to delete old graph and renderer paths without also rewriting unrelated frame coordination at the same time.

Rule:

- If time is limited, do `#181` first.
- `#118` and `#119` are the highest-value cleanup work before the major cut, but may be scoped to only the slices needed to support the migration.

### Runway Category C: Issues that still matter after the migration

These are not made irrelevant by the move; they survive because they are about ownership, lifecycle, or platform/runtime correctness:

- **#174** new tile/pane focus activation race leaves viewport blank until follow-up focus changes
- **#175** web content new-tile/context-menu path bypasses Graphshell node/tile creation semantics
- **#168** per-tile GPU budget and degradation diagnostics
- **#169** viewer backend hot-swap intent and state contract
- **#162** overlay affordance policy per `TileRenderMode`

Why they matter:

- `#174` is about focus/render activation ordering, not only `egui_graphs`.
- `#175` is about Graphshell semantic authority over web content actions, not the graph widget.
- `#168` becomes more important, not less, under a `wgpu`-driven render path.
- `#169` remains relevant for multi-backend/runtime behavior.
- `#162` is a policy issue that survives implementation changes.

Rule:

- Treat these as migration-adjacent work, not old-stack-only work.

### Runway Category D: Issues likely superseded or deprioritized once the relevant cut begins

These mostly describe bugs or contracts tied to the current `egui_graphs` or GL-specific graph path and should not absorb major effort if the migration is genuinely underway:

- **#173** graph canvas pan/wheel zoom/zoom commands no-op across contexts
- **#104** zoom-to-fit regression repro + fix + regression test
- **#102** lasso selection metadata ID hardcoded after per-view metadata split
- **#101** camera commands target global pending state in multi-view

These are likely partially superseded as old-GL-path work:

- **#160** Surface Composition Pass Model and CompositorAdapter for GL state isolation
- **#166** compositor replay traces for callback-state forensics
- **#171** compositor chaos mode for GL isolation invariants

Why they are lower priority:

- `#173`, `#104`, `#102`, and `#101` are heavily tied to the current `egui_graphs` graph path.
- `#160`, `#166`, and `#171` protect and instrument the current GL callback model.
- The underlying needs (camera correctness, observability, pass-order proof) still matter, but the specific old-path implementation work may be thrown away by the migration.

Execution note (2026-03-01):

- `#171` has been implemented as a diagnostics-gated compositor chaos probe for GL isolation invariants (viewport/scissor/blend/active-texture/framebuffer), with pass/fail diagnostics channels and focused regression coverage in `shell/desktop/workbench/compositor_adapter.rs`.
- Receipt: `design_docs/archive_docs/checkpoint_2026-03-01/2026-03-01_issue_171_compositor_chaos_mode_receipt.md`

Rule:

- Do not invest heavily in these unless they are blocking day-to-day development before the cut.
- Prefer to solve their underlying concerns in the new canvas and new renderer path instead of polishing the old implementation.

### Runway Category E: What not to do before the move

Do not spend significant time:

- perfecting `egui_graphs` camera semantics,
- polishing old `egui_graphs` metadata edge cases,
- deepening GL-specific callback diagnostics,
- optimizing old-path graph behavior that the custom canvas will replace soon.

These are attractive because they are concrete, but they are exactly the kind of prototype work that becomes sunk cost during a planned hard cut.

### Runway Exit Criteria

The runway is complete enough to begin the backend migration when:

1. The application readiness gate is met.
2. `#180` has answered the backend gate with measured evidence.
3. `#181` has established the graph/canvas seam (or the equivalent structural prep is complete).
4. The major orchestration hotspots are reduced enough that the cut is contained.
5. The team has explicitly stopped prioritizing old-path-only graph fixes unless they block development.

At that point, Graphshell should stop treating the backend migration as a future plan and start executing the renderer cut.

---

## Phase 0: Pre-Migration Hard Decisions

### Objective

Reduce ambiguity before code movement begins.

### Actions

1. Lock the accepted target stack in docs:
   - `egui`
   - `egui_tiles`
   - `egui_wgpu`
   - Graphshell custom canvas
2. Decide the initial canvas presentation model:
   - `wgpu` texture presented inside egui, or
   - direct `egui_wgpu::Callback`
3. Decide `wgpu` ownership:
   - Graphshell-owned `Instance` / `Adapter` / `Device` / `Queue`
   - or egui-owned with Graphshell attached
4. Define the minimum feature set for the first custom canvas:
   - 2D orthographic
   - node + edge rendering
   - camera ownership
   - hit testing
   - drag
   - selection
   - lasso
5. Explicitly mark which current features may temporarily disappear during transition.

### Done Gate

- The migration target and sequencing are documented.
- The first implementation slice has a bounded scope.
- There is no open ambiguity over ownership or basic presentation strategy.
- The team agrees that backend migration is deferred until the app is stable enough to benefit from it.

---

## Phase 1: Prove the Renderer Gate

### Objective

Answer the only hard dependency before backend swap: runtime viewer surface interoperability.

This phase may be executed as research or spike work before the backend cut, but it does not, by itself, authorize starting the backend migration.

### Actions

1. Build a narrow spike that takes exactly one current composited runtime viewer surface and presents it in a `wgpu`-backed frame.
2. Measure:
   - copies required
   - latency
   - frame-time cost
   - resize behavior
   - behavior under repeated tile-rect changes
3. Document whether the bridge is:
   - acceptable now,
   - acceptable with constraints,
   - or unacceptable without deeper runtime changes.
4. Pick the actual bridging approach to support the future renderer migration.

### Prototype-Breaking Policy

- This spike can live outside the mainline code path.
- It does not need full application integration.
- It exists only to kill uncertainty.

### Done Gate

- Graphshell knows how runtime viewer surfaces will enter the `wgpu` frame.
- The cost of that path is measured, not guessed.
- Backend migration can be sequenced rationally.

---

## Phase 2: Carve the Replacement Seams

### Objective

Make the current app structurally ready to lose `egui_graphs` and `egui_glow`.

### Actions

1. Split current graph integration code out of the large orchestration modules.
2. Extract Graphshell-owned interfaces and data contracts:
   - `GraphCanvasBackend`
   - `GraphCanvasScene`
   - `GraphCanvasInput`
   - `GraphCanvasFrameConfig`
   - `GraphCanvasOutput`
3. Make the graph pane consume those contracts instead of directly depending on `egui_graphs`.
4. Define a renderer-facing abstraction boundary for the UI backend:
   - minimal surface/context lifecycle API
   - texture registration/update boundary
   - custom pass registration boundary
5. Move all graph interaction semantics behind Graphshell-owned intent emitters.
6. Reduce `render/mod.rs` to coordination only; move graph-specific bridges into dedicated modules.

### Prototype-Breaking Policy

- Internal module paths can change freely.
- Old helper APIs can be deleted rather than preserved.
- Tests should be rewritten to target new contracts, not old implementation shapes.

### Done Gate

- Graphshell has a stable canvas contract that can be backed by the old implementation temporarily.
- `egui_graphs` is no longer referenced from broad orchestration code.
- `egui_glow` is isolated to the renderer boundary instead of bleeding through the UI stack.

---

## Phase 3: Conditional Canvas Migration (`egui_graphs` -> Graphshell Canvas)

### Objective

Remove the graph widget dependency and make the graph pane Graphshell-owned, but only if `egui_graphs` has become a proven bottleneck.

### Strategy

Do **not** force this cut as a prerequisite for `egui_wgpu`.

This phase should start only if one of the following becomes true:

- `egui_graphs` is a measured performance bottleneck,
- `egui_graphs` blocks required interaction ownership,
- `egui_graphs` prevents a needed render path or product behavior from landing cleanly.

Until then, the correct timing is:

- fix the app enough that the interaction model is understood,
- keep `egui_graphs` if it remains sufficient,
- prioritize the backend migration separately once its own blocker lane is cleared.

### Actions

1. Build the first custom canvas implementation under the existing mainline renderer.
2. Start simple:
   - 2D orthographic
   - explicit camera ownership
   - node/edge rendering
   - pointer hit testing
   - drag
   - selection
   - lasso
3. Reimplement only the core interaction model first.
4. Freeze or temporarily simplify low-priority features if needed:
   - advanced overlays
   - rich visual styling
   - non-essential tooltip behavior
   - complex physics integration polish
5. Delete the `egui_graphs` adapter path once the custom canvas renders and accepts basic interaction.
6. Remove the `egui_graphs` dependency from `Cargo.toml`.

### Prototype-Breaking Policy

- Visual parity is not required before deletion.
- Temporary regression in graph aesthetics is acceptable.
- Temporary removal of lower-priority graph affordances is acceptable.
- The old graph widget path should be deleted once the new canvas owns the pane.

### Done Gate

- The app builds and runs without `egui_graphs`.
- The graph pane is rendered through Graphshell-owned canvas code.
- Core graph interactions operate through Graphshell-owned authority.

---

## Phase 4: Rebuild Canvas Correctness and Feature Parity

### Objective

Stabilize the new canvas as the real graph surface before changing renderer backends.

### Actions

1. Restore or intentionally redesign graph interactions:
   - zoom
   - pan
   - hover
   - tooltips
   - focus ring
   - selection visuals
   - search highlighting
2. Rebuild per-view camera state and persistence on Graphshell terms, not widget terms.
3. Rebuild physics integration through explicit Graphshell-owned contracts.
4. Add golden or contract tests for:
   - camera transforms
   - hit testing
   - lasso behavior
   - selection truth
   - pane focus ownership
5. Add diagnostics for:
   - frame timing
   - draw counts
   - fallback/degradation

### Prototype-Breaking Policy

- Any current behavior may be redefined if the new version is cleaner and more explicit.
- "Parity" means preserving important user semantics, not reproducing old widget quirks.

### Done Gate

- The graph surface is functionally stable on the new canvas.
- The most important graph interactions are validated by tests.
- Graphshell no longer depends on widget-provided graph semantics.

---

## Phase 5: Prepare the `egui_wgpu` Landing Zone

### Objective

Get the rest of the app ready for the backend swap without performing it yet.

### Actions

1. Isolate all `egui_glow` references to a narrow renderer module.
2. Refactor GL-specific compositor logic behind a backend-neutral compositor contract.
3. Define the new renderer integration boundary for:
   - surface lifecycle
   - texture registration
   - custom pass registration
   - frame submission
4. If the custom canvas is active, decide whether it is presented by:
   - texture into egui, or
   - direct backend callback
5. Make sure the graph pane and runtime viewer composition paths both target this new renderer contract, whether the graph pane is still `egui_graphs` or a custom canvas.

### Prototype-Breaking Policy

- Existing compositor internals can be deleted and rebuilt around the new contract.
- Backend-neutrality matters more than preserving the current call graph.

### Done Gate

- `egui_glow` is now an isolated implementation detail.
- The rest of the app is structurally ready to change renderer backends.
- The old GL callback path is no longer a cross-cutting architectural dependency.

---

## Phase 6: Hard Cut from `egui_glow` to `egui_wgpu`

### Objective

Replace the egui renderer backend and remove OpenGL-specific UI rendering code from the mainline path.

This phase is explicitly deferred until:

- the application readiness gate is met,
- `lane:embedder-debt` (`#90`) has cleared the host/render boundary blocker,
- and the runtime-viewer bridge has been proven.

### Actions

1. Introduce the chosen `egui_wgpu` integration path.
2. Create the Graphshell-owned `wgpu` runtime if that is the selected ownership model.
3. Reconnect:
   - egui chrome
   - the active graph pane implementation (`egui_graphs` or custom canvas)
   - runtime viewer surface presentation
   to the new backend.
4. Validate:
   - startup
   - resize
   - tile changes
   - multiple panes
   - surface recreation
   - device loss handling
5. Delete:
   - `egui_glow` integration code
   - old GL-specific callback plumbing
6. Remove the `egui_glow` dependency from `Cargo.toml`.

### Prototype-Breaking Policy

- This can temporarily break visual correctness while the new backend is wired up.
- If the old path and new path must coexist briefly, keep that coexistence short and explicitly temporary.
- Mainline should not carry both renderer backends longer than necessary to make the cut safely.

### Done Gate

- The app builds and runs without `egui_glow`.
- egui chrome is rendered through `egui_wgpu`.
- The active graph pane path and runtime viewer content are visible through the new frame path.

---

## Phase 7: Post-Cut Stabilization and Optimization

### Objective

Turn the new stack into a durable foundation instead of just a working replacement.

### Actions

1. Optimize batching, culling, and resource reuse.
2. Add explicit degradation policies for GPU pressure.
3. Improve diagnostics and profiling hooks.
4. Normalize visuals and polish after the functional architecture is stable.
5. Reintroduce deferred enhancements only after correctness:
   - richer canvas visuals
   - improved animation
   - 2.5D / isometric projection experiments
   - optional AI-adjacent GPU overlays

### Prototype-Breaking Policy

- Performance-oriented structural changes are acceptable if they improve the new architecture.
- Do not reintroduce old architectural shortcuts in the name of fast polish.

### Done Gate

- The new stack is measurably stable.
- The architecture is simpler than the old one.
- Performance and diagnostics are acceptable for ongoing feature work.

---

## Phase 8: Only Then Re-Evaluate `egui_tiles`

### Objective

Avoid expanding the migration until the real blocker is proven.

### Actions

1. Reassess whether `egui_tiles` is still constraining:
   - pane clipping
   - cross-pane composition
   - focus routing
   - performance
   - docking semantics
2. Replace it only if the new canvas and `wgpu` backend prove that layout hosting is now the main remaining mismatch.

### Done Gate

- Either `egui_tiles` is explicitly retained,
- or there is a separate, justified project to replace it.

This migration should not smuggle in a custom docking engine unless the evidence says it must.

---

## Execution Rules

These rules keep the migration from decaying into a half-rewrite.

### Rule 1: One authority per state category

The new stack must not recreate:

- camera dual-authority
- selection dual-authority
- focus dual-authority
- render-mode dual-authority

If a phase introduces ambiguity, that phase is incomplete.

### Rule 2: No sentimental compatibility

If preserving an old internal behavior complicates the new architecture and has no user value, delete it.

### Rule 3: No indefinite dual paths

Temporary coexistence is allowed only to make a cut safely. It is not allowed as a resting state.

### Rule 4: Prefer smaller hard cuts over endless "almost there"

A sharp, validated break is better than six weeks of compatibility scaffolding for a prototype with no users.

### Rule 5: Tests must follow the new architecture

Do not keep tests that merely protect old widget quirks. Replace them with tests that protect the new contracts.

---

## Recommended First Three Implementation Slices

If implementation starts immediately, the first three slices should be:

1. **Slice 1: Renderer-gate spike**
   - prove runtime viewer GL -> `wgpu` bridge
   - choose device ownership and presentation strategy

2. **Slice 2: Canvas seam extraction**
   - introduce `GraphCanvasBackend` contracts
   - split `render/mod.rs`
   - isolate `egui_glow`

3. **Slice 3: Backend landing prep**
   - make the graph path consume the backend-neutral renderer contract
   - keep `egui_graphs` unless it has become a demonstrated blocker

This sequence gives the fastest route to a viable backend migration while keeping the optional canvas replacement honest and evidence-driven.

---

## Final Position

The correct strategy is:

- keep `egui`
- keep `egui_tiles`
- split the old combined migration into separate backend and canvas issues
- treat `egui_glow` -> `egui_wgpu` as blocked by `lane:embedder-debt` until the host/render boundary is ready
- keep `egui_graphs` unless it becomes a proven bottleneck
- only then consider the custom-canvas cut
- refuse long-lived compatibility scaffolding

The prototype is allowed to break.

The architecture is not allowed to stay ambiguous.

