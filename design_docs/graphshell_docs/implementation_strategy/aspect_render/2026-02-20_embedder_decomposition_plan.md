# Embedder Decomposition Plan (Revised)

**Date:** 2026-02-21 (revision of 2026-02-20 plan)
**Status:** Prospective — Stage 1 in progress, Stages 2+ planned
**Relates to:** Architectural Concerns doc §8 (Monolithic UI)

**Coordination note (2026-02-26):** Stage 4 decomposition should follow the foundation-first sequencing in `2026-02-26_composited_viewer_pass_contract.md` Appendix A and `PLANNING_REGISTER.md` §0.10. Specifically, compositor pass-order correctness + GL-state diagnostics hardening should land before UX-expansion slices that increase render-path complexity.

---

## Context

Graphshell's embedder layer was forked from servoshell and has grown substantially. The four core inherited files plus the graphshell-specific UI additions now show significant size and coupling problems:

| File | Lines | Role |
| --- | --- | --- |
| `window.rs` | ~646 | `EmbedderWindow`, `WebViewCollection`, `GraphSemanticEvent` |
| `running_app_state.rs` | ~768 | `WebViewDelegate` + `ServoDelegate` impls; central Servo hub |
| `desktop/app.rs` | ~213 | Servo init, `ApplicationHandler<AppEvent>` for Winit |
| `desktop/event_loop.rs` | ~150 | Headed/headless event loop abstraction |
| `desktop/gui.rs` | ~1216 | egui UI layer |
| `desktop/gui_frame.rs` | ~1198 | Per-frame rendering |
| `desktop/toolbar_ui.rs` | ~357 | Toolbar coordinator (decomposed into 7 submodules) |
| `desktop/tile_*.rs` (×9) | ~1300 | Tile layout and rendering |

**Note (2026-02-23):** The `toolbar_ui.rs` decomposition is complete—the main module now coordinates 7 focused submodules (toolbar_controls, toolbar_settings_menu, toolbar_right_controls, toolbar_location_panel, toolbar_location_submit, toolbar_location_dropdown, toolbar_omnibar). The `gui.rs` god-object problem remains the primary target for Stage 4b decomposition work.

---

## Reality Check Against Current Code (2026-02-20 audit)

This plan is directionally correct, but a few assumptions needed tightening:

- **Semantic bridge extraction is already real and tested**: `desktop/semantic_event_pipeline.rs`, `desktop/gui_frame.rs`, and helper tests exist and work.
- **Lifecycle intent boundary is not yet complete**: direct `graph_app.promote_node_to_active(...)` calls remain in `lifecycle_reconcile.rs` (lines 122, 139), bypassing the reducer.
- **`RunningAppState` is both embedder + policy surface**: it owns Servo, windows, webdriver channels, gamepad provider, and mutable runtime preference usage (webdriver port toggling), not just read-only embedder data.
- **`EmbedderWindow` creation API depends on `RunningAppState`**: constructor references `Rc<RunningAppState>` to access delegate properties, user content manager, gamepad provider. This is the primary coupling point for extraction.
- **UI split is underway but unfinished**: `toolbar_ui::render_toolbar_ui(...)` exists; remaining work is state ownership and responsibility isolation.

---

## Natural Decomposition Seams

### Seam 1: Servo Embedder Core (cleanest boundary, highest priority)

**Files:** `window.rs`, `running_app_state.rs`, `desktop/app.rs`, `desktop/event_loop.rs`

**Current problem:** `RunningAppState` is doing two jobs:
1. Implementing Servo's `WebViewDelegate` and `ServoDelegate` traits (pure embedder responsibility)
2. Holding graphshell-specific state: `app_preferences`, GUI handle, intent queues, gamepad provider

**Proposed split:**

```
EmbedderCore                        RunningAppState (graphshell layer)
────────────────────────────────    ────────────────────────────────────
Servo instance                      EmbedderCore (owned)
windows: HashMap<EmbedderWindowId,  app_preferences: AppPreferences
         EmbedderWindow>            gui: Option<Rc<Gui>>
waker: Box<dyn EventLoopWaker>      intent queues
WebViewDelegate impl                gamepad_provider
ServoDelegate impl                  screenshot coordination
────────────────────────────────    ────────────────────────────────────
emits: Vec<GraphSemanticEvent>      consumes: Vec<GraphSemanticEvent>
```

`EmbedderWindow` already emits `GraphSemanticEvent` — this is the correct interface boundary. `EmbedderCore` collects them; `RunningAppState` drains them per frame and feeds `apply_intents`.

**Test benefit:** `EmbedderCore` becomes unit-testable without egui or petgraph. Delegate ordering tests (currently integration-only) can become unit tests.

**Critical blocker to resolve first:** Delegate paths check/toggle webdriver-related preferences at runtime, so this is **not** read-only. Phase 2 (Contract Freeze) must map which preference fields belong to `EmbedderCore` vs. graphshell policy layer.

### Seam 2: Semantic Bridge (already partially extracted)

**Files:** `desktop/semantic_event_pipeline.rs` (96 lines), `desktop/lifecycle_reconcile.rs` (223 lines), `desktop/webview_controller.rs` (395 lines)

**Current state:** These are already reasonably focused modules. The intent system is the correct API boundary.

**Action needed:** Close remaining direct lifecycle mutations in `lifecycle_reconcile.rs` (promote_node_to_active at lines 122, 139) so all lifecycle state changes are intent-driven. See Stage 1 below.

### Seam 3: UI Shell (hardest, deferred to Stage 4)

**Files:** `desktop/gui.rs`, `desktop/gui_frame.rs`, `desktop/toolbar_ui.rs`, `desktop/tile_*.rs`

**Problem:** Each conflates state (persistent fields), coordination (what to render), and rendering (egui draw calls).

**Proposed decomposition (not a rewrite):**

```
ToolbarState            — persistent fields only
toolbar_ui::render()    — stateless fn(ctx, &mut ToolbarState, &GraphBrowserApp) -> Output

GuiState                — persistent fields
gui_frame::render()     — stateless fn(ctx, &mut GuiState, ...) -> Output
```

Target: no single file > ~600 lines after decomposition; each file has one stated responsibility.

---

## Ordered Delivery Plan (Single Sequence)

### Stage 1: Lifecycle Stabilization (Immediate)

**Goal:** Establish deterministic lifecycle transitions as the foundation for all subsequent work.

**Surface area:** `app.rs`, `desktop/lifecycle_reconcile.rs`, `desktop/webview_backpressure.rs`, tests.

**Tasks:**

1. [x] **Finalize reconcile-only intent emission:** Reconcile now emits cause-bearing promotion intents (no direct `graph_app.promote_node_to_active(...)` calls in reconcile paths).
2. [x] **Implement explicit blocked/cooldown behavior:** `webview_backpressure.rs` now emits `MarkRuntimeBlocked` / `ClearRuntimeBlocked` with `backon`-based cooldown.
   - Crash handling was consolidated into the same runtime-block model (single authoritative block state), replacing separate crash-only runtime state storage.
3. [x] **Add `LifecycleCause` metadata to intents:** Lifecycle intents now carry `cause` and callsites are migrated.
4. [x] **Codify lifecycle invariants:** Implemented tests and debug assertions for reconcile mutation boundaries, blocked gating, memory-pressure cause stability, and crash-reactivation policy.

**Acceptance gates:**

- `cargo check` and `cargo test` pass cleanly.
- No direct lifecycle mutation calls remain in reconcile-like functions.
- All five lifecycle test scenarios from model §7 exist and pass.
- Retry loops and cooldown paths are deterministic and testable.

**Estimated scope:** ~150–250 lines changed in reconcile/backpressure; ~150–200 lines of tests added.

✓ **Status:** Functionally complete; remaining work is broader regression coverage expansion, not model-gap closure.

---

### Stage 2: Contract Freeze (Prerequisite to Embedder Isolation)

**Goal:** Lock ownership and API contracts before structural moves to prevent refactor drift and clarify risk.

**Surface area:** Documentation, module contracts; no code structural changes yet.

**Tasks:**

1. [x] **Create RunningAppState field ownership table:** Added initial ownership mapping with rationale.
2. [x] **Define minimal `EmbedderCore` API surface:** Added scaffold API in `desktop/embedder.rs` for window/event responsibilities.
3. [x] **Define `EmbedderWindow` constructor dependency contract:** Established `WebViewCreationContext` contract (required: `Servo`, `UserContentManager`, delegate; optional: gamepad provider).
4. [x] **Design `EmbedderApi` trait** (recommended adapter pattern over large moves): Implemented as `WebViewCreationContext` and integrated into `create_toplevel_webview*`.
5. [x] **Record explicit non-goals:** Documented no crate split and no delegate-order semantic changes as decomposition constraints.

**Acceptance gates:**

- Ownership table is present, unambiguous, and agreed.
- `EmbedderApi` trait design is reviewed (signatures, lifetimes, trait bounds clear).
- `cargo check` passes with new contract/ownership comments added.
- Zero functional changes; static analysis only.

**Estimated scope:** ~100–150 lines of documentation and code comments.

---

### Stage 3: Embedder Core Isolation (Medium Risk, High Payoff)

**Goal:** Extract pure-embedder responsibilities from graphshell policy/UI layer.

**Prerequisites:** Stage 1 (promotion intents working), Stage 2 (contracts locked).

**Surface area:** `running_app_state.rs`, `window.rs`, new `embedder.rs`, callers in `lifecycle_reconcile.rs`, `webview_controller.rs`, tile runtime.

**Tasks:**

1. [x] **Introduce `embedder.rs` module** with `EmbedderCore` struct:
   - Owns `Servo` instance, `windows: HashMap<EmbedderWindowId, EmbedderWindow>`, `waker`
   - Collects `pending_events: Vec<GraphSemanticEvent>` for per-frame drain
2. [x] **Move `impl WebViewDelegate` and `impl ServoDelegate`** from `RunningAppState` to `EmbedderCore` (strangler phase; no behavior change, preserve delegate call order).
   - `WebViewDelegate` now lives on a dedicated delegate wrapper (`RunningAppStateWebViewDelegate`) instead of `RunningAppState`; `ServoDelegate` was already separate.
3. [x] **Refactor `EmbedderWindow::create_toplevel_webview*` signatures** to accept narrow `EmbedderApi` trait reference instead of `Rc<RunningAppState>`.
4. [x] **Update all constructor call sites** to pass `EmbedderApi` reference (reconcile, webview controller, tile runtime).
5. [x] **Thin `RunningAppState`:** now shells around `EmbedderCore` for embedder runtime ownership (`Servo`, windows, focused window, pending semantic-event drain), plus graphshell policy/runtime state.
6. [x] **Add delegate ordering parity tests:** Added unit coverage for emission-time sequence stamping and deterministic sequence-order sorting in embedder/window event drains.
   - Semantic-event ordering/property snapshots exist; explicit delegate-callback parity harness remains pending.
7. [x] **Update semantic event drain path:** pre-frame ingest now drains semantic events via `RunningAppState -> EmbedderCore` instead of directly from `EmbedderWindow`.

**Acceptance gates:**

- `impl WebViewDelegate` and `impl ServoDelegate` no longer live on `RunningAppState`.
- `window.rs` webview creation has no direct `RunningAppState` dependency; uses `EmbedderApi` trait.
- Delegate event ordering identical to baseline (parity tests validate).
- `cargo check`, `cargo test`, and integration tests pass without regressions.
- Desktop startup, navigation, and webview lifecycle behavior matches baseline.

**Estimated scope:** ~400–600 lines changed; ~200–300 lines of tests.

**Risk:** Medium. Delegate implementations are tightly coupled to shared state. Requires careful field attribution and incremental testing.

---

### Stage 4: GUI Decomposition (High Risk, Deferred After Stage 3)

**Goal:** Decompose monolithic `Gui` and `toolbar_ui` from god-objects into state + renderer pairs.

**Prerequisites:** Stages 1–3 complete; embedder boundary clear.

**Surface area:** `desktop/gui.rs`, `desktop/gui_frame.rs`, `desktop/toolbar_ui.rs`, `desktop/tile_*.rs`.

**Tasks (per file, in sequence):**

**4a. toolbar_ui.rs:**
1. [x] Extract `ToolbarState` struct (persistent toolbar location/status/navigation state moved out of top-level `Gui` fields).
2. [x] Move rendering into free functions: decomposed into submodules (toolbar_controls, toolbar_settings_menu, toolbar_right_controls, toolbar_location_panel/submit/dropdown, toolbar_omnibar) as of 2026-02-23. Main module is now ~357 lines.
3. [ ] Define `toolbar_ui::Input` (state mutations) and `toolbar_ui::Output` (render results) boundary.
4. [~] Add unit tests for at least one stateful flow: scenario tests exist for pin/tag sync, settings routing, omnibar workflows; focused unit tests still pending.

**4b. gui.rs:**
1. [x] Extract `GuiRuntimeState` struct (texture caches, frame flags, backpressure state).
2. [x] Refactor `Gui::update()` as coordinator calling extracted stateless render functions.
3. [x] Define `gui::Input` and `gui::Output` boundary; no render function has side effects outside return value.
4. [x] Tighten GUI state/helper visibility boundaries: `gui_state::{ToolbarState, GuiRuntimeState}` visibility narrowed to UI-supermodule scope, mutating focus-state helpers moved to `gui.rs` owner module, and orchestration entry-point visibility aligned with state ownership.

**4c. tile_*.rs:**
1. [x] Extract `TileCoordinator` from `tile_runtime.rs`: owns tile→node mapping, pruning logic, mutations.
2. [x] Leave `tile_render_pass.rs` and `tile_compositor.rs` as stateless renderers.

**Acceptance gates:**

- No `desktop/*.rs` file exceeds ~1200 lines (first pass); < ~800 in follow-up.
- At least one stateful toolbar workflow covered by focused unit tests.
- Frame orchestrator (`gui_frame`) owns sequencing; render functions are side-effect-scoped.
- Mutation-capable GUI state helpers are owner-scoped; cross-layer writes from non-owner modules require explicit visibility escalation.
- `cargo test` passes; no regressions in UI rendering or interaction.

**Estimated scope:** ~800–1200 lines refactored per file; ~300–500 lines of tests.

**Risk:** High. Requires preserving behavior exactly while decomposing tightly coupled code.

---

### Stage 5: Control Plane Scaling (Future, Policy-Gated)

**Goal:** Introduce concurrent event ingestion and supervision primitives for multi-producer lifecycle updates.

**Prerequisites:** Stages 1–3 complete and proven stable.

**Trigger criteria** (at least two needed):
- Concurrent prefetch/restore/background-retry workers require cancellation supervision to prevent orphans.
- Multiple event producers (UI, network, memory monitor) cause observable ordering non-determinism.
- Production telemetry reports task leaks or orphaned workers.

**Outline (full spec in separate control-plane design doc):**

1. [ ] `tokio::sync::mpsc` lifecycle event queue (single consumer, multiple producers).
2. [ ] `watch` channel for policy snapshot fan-out (memory limits, retention policies).
3. [ ] `tokio-util::CancellationToken` for background worker supervision and cancellation.
4. [ ] Concurrency tests validating deterministic transition ordering under concurrent producers.

**Acceptance gates:**

- Deterministic lifecycle ordering under concurrent load (property tests).
- No orphan workers after cancellation signal.
- Golden traces stable under concurrent producers.

**Estimated scope:** New module `control_plane.rs` (~300–500 lines); new dep: `tokio-util`.

---

### Stage 6: Data Plane Caching (Future, Policy-Gated)

**Goal:** Add concurrent non-authoritative artifact caches without reintroducing direct lifecycle mutation.

**Prerequisites:** Stages 1–3 complete and proven stable.

**Trigger criteria** (at least two needed):
- Memory profiling shows repeated parsing/decompression of identical metadata.
- Latency profiles show artifact recreation in critical paths.
- Cache miss rate > 40% in telemetry for suggestions or metadata.

**Outline (full spec in separate caching design doc):**

1. [ ] `moka` caches for: thumbnails, parsed metadata, suggestion results, snapshot artifacts.
2. [ ] Cache policy (TTL, max size, cost weights, eviction listeners).
3. [ ] Eviction listener hooks for optional async rewarm (do not block lifecycle).
4. [ ] Verify cache eviction does not trigger lifecycle changes.

**Acceptance gates:**

- Cache behavior measurable and bounded (latency, hit rate, memory overhead).
- Lifecycle tests remain green under cache churn.
- No direct lifecycle mutation reintroduced via cache layer.

**Estimated scope:** New module `caches.rs` (~200–400 lines); new dep: `moka`.

---

### Stage 7: Optional Typed FSM Formalization (Future, Reactive)

**Goal:** Migrate lifecycle transition core to compile-time-safe FSM if transition complexity justifies it.

**Prerequisites:** Stages 1–3 complete; transition matrix has been active for at least one major refactor cycle.

**Trigger criteria** (at least two needed):
- Illegal-transition bugs recur in regressions despite invariant tests.
- Reducer-based transition guards become too diffuse to maintain.
- New contributors frequently ask "which transitions are legal from this state?"

**Outline:**

1. [ ] Pilot `statig` crate for lifecycle transition core only (not full app reducer).
2. [ ] Define `State` and `Event` enums for `statig` FSM.
3. [ ] Port minimal transition subset and measure boilerplate overhead.
4. [ ] Evaluate if compile-time guarantees justify complexity.

**Acceptance gates:**

- Compile-time restrictions reduce bug classes vs. runtime guards.
- Boilerplate cost justified by defect reduction.
- Remaining app reducer stays in reducer pattern; `statig` limited to lifecycle.

**Estimated scope:** New module `lifecycle_fsm.rs` (~300–500 lines); optional dep: `statig`.

---

## Lifecycle Intent Model Reference

For the authoritative specification of lifecycle state machine rules, intent schemas, transition causes, and required invariants, see [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md).

---

## Additional Implementation Ideas

These are aligned with project goals and can be incorporated where useful:

1. **Event Trace Golden Tests:** Capture canonical `GraphSemanticEvent` sequences for common flows (new-tab, redirect chain, back/forward burst) and assert reducer intent outputs stay stable across refactors.

2. **Refactor Safety Budget:** Set temporary CI warnings for module size deltas on `desktop/gui.rs` and `desktop/toolbar_ui.rs` so decomposition progress is visible.

3. **Adapter-First Constructor Decoupling:** Add `EmbedderApi` trait **before** moving data ownership in Stage 3; this lowers risk vs. a large one-shot move.

---

## Key Design Principles

- **Single-crate now.** Clean module boundaries within the crate are the right intermediate step — a premature crate split creates overhead without benefit.
- **No legacy compatibility shims.** When refactoring, delete the old structure rather than keeping a compatibility layer, unless there is no other way for a crucial system to interact with the new system.
- **Two-phase apply model is the commit.** `apply_intents()` (pure state) + `reconcile_webview_lifecycle()` (effects) is the established foundation. All work builds on this, not against it.
- **Test coverage gates each stage.** Phase/stage completion requires new unit tests and proof of behavior preservation.
- **Prefer single authoritative runtime state over parallel stores.** Explicit maintainer preference: expand/migrate (`RuntimeBlockState`) rather than maintain redundant crash/block systems when one model can carry required metadata.

---

## Changelog

**2026-03-01 Revision:**
- Stage 4b boundary tightening slice landed: GUI runtime state/helper visibility narrowed and mutating focus-state helpers are now owner-scoped to `gui.rs` with compile-time guardrails.

**2026-02-21 Revision:**
- Collapsed Phases (E0–E4) + Workstreams (WS1–WS7) into single ordered Stages (1–7) sequence for clarity.
- Extracted Lifecycle Intent Model v2 into separate authoritative [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) document.
- Elevated `EmbedderApi` trait pattern from optional idea to **recommended** approach for Stage 3 (constructor decoupling).
- Moved Stages 5–7 (Control Plane, Data Plane, FSM) to policy-gated future work with explicit trigger criteria; removed from near-term roadmap.
- Added concrete task checkboxes for Stage 1 (promotion intent gap at reconcile lines 122, 139).
- Streamlined each stage description; removed speculative architectural tangents (Lifecycle Scalability Track original outline).
- Clarified Stage 2 as prerequisite to Stage 3 (contracts + ownership map before moves).
- Added acceptance gates and estimated scope per stage for planning.

**2026-02-20 Initial Revision:**
- Conducted reality check against current code; corrected delegate preference assumptions.
- Added Phase E0 (Contract Freeze) as explicit prerequisite.
- Documented workspace-aware demotion logic in lifecycle reconciliation.
- Introduced Lifecycle Intent Model v2 with two-layer state (Desired + Observed).

---

## References

- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Authoritative lifecycle state machine contract
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Two-phase apply model foundation
- [ARCHITECTURAL_CONCERNS.md](../technical_architecture/ARCHITECTURAL_CONCERNS.md) — §8 Monolithic UI Component
- [2026-02-12_servoshell_inheritance_analysis.md](../../archive_docs/checkpoint_2026-02-16/2026-02-12_servoshell_inheritance_analysis.md) — Original fork analysis

