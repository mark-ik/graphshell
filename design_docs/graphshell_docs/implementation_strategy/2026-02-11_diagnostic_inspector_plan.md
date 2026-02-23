# Diagnostic Inspector Plan (2026-02-11)

**Architecture update (2026-02-22):**
This plan predates the frame execution order architecture, `EdgePayload` traversal model,
and settings plan. The following table maps old concepts to current architecture:

| Old concept | Current equivalent |
| --- | --- |
| "Diagnostic graph" node/edge types | `DiagnosticGraph` — separate ephemeral struct; NOT `GraphBrowserApp` browsing graph |
| "Toggle to switch to diagnostic view" | **New Pane Type**: `TileKind::Diagnostic`. Allows side-by-side debugging in the workbench. |
| Diagnostic events in main loop | `crossbeam_channel` + ring buffer, tick-rate-limited (10 Hz render, 60 Hz egui unchanged) |
| Phase 4 export | Routes through settings architecture plan export section (no new export path) |
| Any persistence | **None** — diagnostic data is ephemeral, session-only; not written to fjall or redb |

**Availability update (2026-02-22):** Diagnostic inspector is now part of default desktop
builds for end users. No dedicated debug/diagnostic runtime mode flag is required.

---

## Diagnostic Inspector Plan

- Goal: Visualize system internals (Servo threads, Graphshell compositor, Intent pipeline) as a
  live diagnostic dashboard for debugging and performance profiling.
- Scope: `tracing` span instrumentation, `DiagnosticGraph` data structure, Compositor Inspector,
  `TileKind::Diagnostic` integration, and Mod/Plugin host boundary.
- Dependencies: `tracing` crate, `crossbeam_channel`, egui_graphs (reused with custom
  node/edge style), settings architecture plan (export).
- Phase 1: Instrumentation
  - Add `tracing` spans at thread and IPC boundaries in Servo compositor, layout, and
    script pipelines.
  - **New (2026-02-22):** Add `tracing` spans to Graphshell desktop layer:
    - `tile_render_pass`: active rects, mapped nodes, context availability.
    - `tile_compositor`: texture blits, GL context switches, surface rendering.
    - `app::apply_intents`: intent processing duration and causality.
    - **Future (Mods)**: `mod_host`: execution time, memory usage, and intent emission per mod.
  - Define `DiagnosticEvent` enum: `SpanEnter`, `SpanExit`, `MessageSent { channel_id,
    byte_len }`, `MessageReceived { channel_id, latency_us }`, `CompositorFrame { active_tiles, texture_uploads }`.
  - Produce events via `crossbeam_channel::Sender<DiagnosticEvent>`; consumer lives in
    the diagnostic aggregator (Phase 2). No impact on main render loop.
- Phase 2: Data pipeline
  - `DiagnosticGraph` struct holds thread nodes and channel edges. Completely separate
    from `GraphBrowserApp`'s `StableGraph<Node, EdgePayload>` — no `AddNode`/`AddEdge`
    intents, no fjall/redb persistence, no tags.
  - **New (2026-02-22):** `CompositorState` struct tracks last N frames of rendering data
    (rects, texture IDs, visibility flags) to debug "black tile" issues.
  - Ring buffer (bounded capacity, e.g., 512 events) drains at 10 Hz tick into
    `DiagnosticGraph` aggregated state: edge weights = recent message count, edge label =
    p95 latency. Old events dropped; no memory growth.
  - Thread topology (nodes) is mostly static (Servo thread pool is fixed at startup);
    channel edges are dynamic.
- Phase 3: UI Integration (`TileKind::Diagnostic`)
  - Add `TileKind::Diagnostic` to the workbench.
  - **Visual Design**:
    - **Engine Tab**: Node-link diagram of Servo threads. "Circuit board" theme (dark, neon edges).
    - **Compositor Tab**: Split view. Left: Tree of active tiles. Right: Minimap of window rects.
      - *Interaction*: Hovering a tile in the list draws a debug overlay on the real tile.
    - **Intents Tab**: Scrolling log of `GraphIntent`s with `LifecycleCause`.
  - Render engine graph with egui_graphs (custom style): thread nodes as boxes,
    channel edges with weight thickness and latency label; bottleneck edges highlighted
    in red (latency > threshold).
  - Diagnostic view is available in default desktop builds with direct shortcuts/toggle.
- Phase 4: Export
  - "Export diagnostic snapshot" action in the settings panel (cross-ref settings
    architecture plan export section) — no new export path needed.
  - Formats: SVG (egui_graphs render to SVG) and JSON (serialize `DiagnosticGraph`
    state at point-in-time).
- Phase 5: End-of-Plan QoL Sweep (required before declaring complete)
  - Run a short "non-contradictory QoL ideas" pass after functional phases are done.
  - Include small improvements discovered during development but deferred for scope control
    (for example: hot-channel summary tables, pin/overlay affordances, small diagnostics UX polish).
  - Keep this sweep bounded (no architectural pivots), and verify each item does not
    conflict with plan constraints (ephemeral diagnostics, feature-gated code, no persistence pollution).

## Status Snapshot (2026-02-22)

**Servo wiring status (2026-02-22):** **Complete (Graphshell bridge layer)**.
Servo-originated delegate and event-loop boundaries now emit explicit diagnostics bridge
channels/spans (`servo.delegate.*`, `servo.graph_event.*`, `servo.event_loop.spin`) and are
wired into Engine topology aggregation (`Servo Runtime` -> `Semantic Ingress`).

- Implemented
  - `TileKind::Diagnostic` pane integration (side-by-side workbench model).
  - Compositor tab core: hierarchy, minimap, hover/pin overlay, click-to-focus.
  - Intents tab core with `LifecycleCause` badges.
  - Event-channel foundation: `DiagnosticEvent`, bounded ring buffer, 10 Hz drain pipeline.
  - Initial engine topology visualization driven by emitted channel activity.
  - Engine diagnostics latency labels (count + p95) + bottleneck threshold highlighting policy.
  - QoL sweep tranche: hot-channel summary table and in-pane metrics reset control.
  - Export integration routed via settings menu actions (JSON/SVG snapshot).
  - Active-tile invariant guard in render pass, including explicit warning emission for
    missing active tile mapping/context conditions.
  - Engine tab explicit active-tile violation status line
    (`tile_render_pass.active_tile_violation`) for fast black-tile triage.
  - Regression coverage for active-tile mapping invariant helper.
  - Engine graph controls now include selectable latency percentile policy (P90/P95/P99)
    and configurable bottleneck threshold used by graph highlighting and labels.
  - Semantic pipeline diagnostics breadth expanded with per-intent-kind channels
    (`semantic.intent.*`) and aggregate `semantic.intents_emitted` eventing.
  - Added targeted diagnostics robustness tests across matrix/property/snapshot/persistence
    paths, plus `tracing-test` log-capture coverage for semantic pipeline marker output.
- Remaining
  - Validation pass items listed below that still require headed/perf evidence
    (especially FPS impact checks).

### Servo Bridge Completion Checklist

- [x] Define Servo-originated diagnostic channel IDs and span naming contract
  (script/layout/compositor + IPC edges).
- [x] Add bridge emission points so Servo-originated span/message activity is
  translated into `DiagnosticEvent` inputs consumed by the existing ring buffer.
- [x] Add automated assertions proving Servo-originated events are represented in
  Engine topology output shape (Servo runtime node/edge + channel aggregation path).
- [x] Headed validation: confirm Engine tab displays Servo-originated activity
  while loading/navigating real pages.
  - Evidence (2026-02-22): user-confirmed during headed run that Engine tab
    displays live Servo-originated activity while navigating real pages.
- [x] Export parity check: confirm Servo-originated sections are present in JSON/SVG
  snapshots at capture time when activity exists.
  - Evidence (2026-02-22): `diagnostics-1771804408.json` contains non-zero
    `servo.delegate.*`, `servo.graph_event.*`, and `servo.event_loop.spin` channel entries.

## Validation Tests

- [x] Events appear in the ring buffer during diagnostics usage and are aggregated via bounded
  10 Hz drain. (Unit coverage exists for 10 Hz gating and p95 aggregation behavior.)
- [x] Diagnostic render tick runs at ≤10 Hz while main egui FPS does not degrade.
  - Evidence (2026-02-22): headed validation confirmed by manual run; diagnostics remained
    visibly rate-limited with no observed egui responsiveness regression.
- [x] Workspace/session persistence payloads exclude diagnostics runtime state
  (automated bundle-serialization invariant test coverage added).
- [x] Default end-user desktop builds include diagnostics pane/shortcuts (no explicit
  diagnostics mode flag required).
- [x] Export produces valid JSON/SVG snapshot artifacts through settings action wiring.
- [x] Export parity check: JSON snapshot channel/frame/intent sections match
  in-memory diagnostics aggregates at capture time (automated snapshot parity tests).

### Validation Runbook (remaining open checks)

1. **Headed perf check: diagnostics tick vs FPS impact**
   - Build/run default desktop build:
     - `cargo run`
   - Open diagnostics pane and keep **Engine** + **Compositor** active during normal browsing
     for ~2-3 minutes.
   - Capture evidence:
     - Verify diagnostics updates are visibly rate-limited (no per-frame flood behavior).
     - Record before/after responsiveness notes (scroll, input latency, viewport interactions).
   - Pass criteria:
     - No obvious interaction regressions while diagnostics is active.
     - No runaway event growth or render-stall behavior.

2. **Ephemeral persistence check (restart + store inspection)**
   - Run diagnostics session and generate events.
   - Close app cleanly and restart normally.
   - Inspect persisted stores/workspace state used by app.
   - Pass criteria:
     - No diagnostic-only structures serialized as durable graph/workspace data.
     - Diagnostics state starts fresh on restart.

3. **Release behavior check (default path)**
   - Build and run release:
     - `cargo check --release --message-format short`
     - `cargo run --release`
   - Verify diagnostics entry points (`F12`, `Ctrl+Shift+D`, command-palette action)
     remain functional in release/default desktop build.

4. **Export parity spot-check (in-memory vs snapshot)**
  - Run default build and generate diagnostics activity.
   - Trigger JSON export from Settings.
   - Compare exported snapshot to current diagnostics pane state at capture moment
     (channel counts, span rows, active tile/compositor summary).
   - Pass criteria:
     - Exported JSON consistently reflects current in-memory diagnostics values.
     - No missing core sections (`diagnostic_graph`, `compositor_frames`, `intents`).

## Outputs

- `DiagnosticGraph` struct, `CompositorState` struct, and `DiagnosticEvent` enum.
- Diagnostic view mode integration in `gui.rs` frame loop.
- Export action in settings panel.

## Findings

- (See architecture update note at top of file.)
- `DiagnosticGraph` is intentionally separate from the browsing graph. Mixing them would
  pollute the user's graph with internal implementation details and break tag semantics,
  omnibar search, persistence, and physics.
- The 10 Hz diagnostic render tick is sufficient for latency/throughput visualization and
  avoids cache invalidation pressure on the main 60 Hz egui render.
- Servo's thread topology is semi-static: compositor, layout, and script threads are
  present from startup. Dynamic channels (IPC pipes per new pipeline/frame) add edges at
  runtime. The ring buffer handles the dynamic case gracefully.

## Progress

- 2026-02-11: Plan created.
- 2026-02-20: Aligned with frame execution order architecture (third view mode, step 8
  branch), `DiagnosticGraph` isolation from `GraphBrowserApp` browsing graph, ring-buffer
  with tick-rate-limited pipeline.
- 2026-02-22: Expanded scope to include Graphshell Desktop instrumentation (Compositor,
  Tile Runtime) to support debugging of rendering/layout issues.
  Refactored UI strategy to use `TileKind::Diagnostic` pane instead of modal view.
- 2026-02-22: Added Engine tab p95 latency labels and bottleneck highlighting, completed an
  initial QoL sweep pass, and wired settings export actions for JSON/SVG diagnostics snapshots.
- 2026-02-22: Added active-tile invariant guard instrumentation in render pass plus diagnostics
  surfacing (`tile_render_pass.active_tile_violation`) and test coverage for the active-tile
  invariant helper.
- 2026-02-22: Added diagnostics snapshot parity tests validating JSON snapshot core-section
  presence and channel aggregate parity with in-memory diagnostics state.
- 2026-02-22: Added workspace bundle serialization invariant test to verify diagnostics
  runtime payload (`diagnostic_graph`/channels/spans/event ring) is not persisted in
  workspace/session layout JSON artifacts.
- 2026-02-22: Completed Engine parity controls (percentile policy + bottleneck slider),
  expanded semantic diagnostics channel coverage, and added `tracing-test` assertion
  coverage for semantic pipeline trace marker logging.
- 2026-02-22: Completed Graphshell-side Servo diagnostics bridge wiring with explicit
  `servo.delegate.*` + `servo.graph_event.*` channels, `servo.spin_event_loop` span/latency
  instrumentation, and Engine topology Servo Runtime edge integration.
