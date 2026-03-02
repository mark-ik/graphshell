# Cross-Cutting Subsystem: UX Semantics

**Status**: Proposed / Design Phase
**Subsystem label**: `ux_semantics`
**Long form**: UX Semantics Subsystem
**Scope**: Runtime-queryable semantic tree of Graphshell's own native UI; UX contract verification; snapshot regression testing; UxBridge test harness integration
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `accessibility` (Accessibility), `diagnostics` (Diagnostics), `focus` (Focus)
**Doc role**: Canonical subsystem implementation guide
**Research basis**: `../../2026-03-01_automated_ux_testing_research.md`
**Related**:
- `../subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` (UxTree maps to AccessKit nodes — one tree, two consumers)
- `../subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` (UxViolation events routed through diagnostics channels)
- `../subsystem_diagnostics/2026-02-26_test_infrastructure_improvement_plan.md` (T1/T2 infrastructure — Phase 0 prerequisite)
- `PLANNING_REGISTER.md`

---

## 1. Why This Exists

Graphshell already has a pure reducer (`apply_intents`), a reconcile boundary, a diagnostics
channel system, a WebDriver command loop, and a headless execution path. The missing piece is a
**machine-readable model of Graphshell's own native UI** — distinct from both the web content
served inside Node Panes and the OS platform accessibility tree.

Without this model:

- There is no automated way to assert "every interactive control has a label."
- There is no automated way to verify "opening a node reaches a working viewer."
- There is no regression signal when a refactor silently changes the UX shape.
- The Accessibility subsystem's AccessKit bridge has no internal source of truth to validate
  against — it can inject nodes but cannot verify the structural invariants of what it injects.

The UX Semantics subsystem addresses all four gaps by building one canonical tree that serves
three consumers: the test harness, the UxBridge, and (via mapping) the AccessKit OS bridge.

---

## 2. Subsystem Model (Four Layers)

| Layer | UX Semantics Instantiation |
|---|---|
| **Contracts** | Structural invariants (S1–S9), navigation invariants (N1–N4), state machine invariants (M1–M4) — §3 |
| **Runtime State** | `UxTree` (rebuilt per frame, cached), `UxProbeSet` (registered invariant checkers), open `UxViolation` events |
| **Diagnostics** | `ux:structural_violation`, `ux:navigation_violation`, `ux:contract_warning`, `ux:tree_build` channels — §5 |
| **Validation** | UxScenario runner, snapshot diffing, CI gates — §6 |

---

## 3. Required Invariants / Contracts

### 3.1 Structural Invariants (S-series)

These must hold at every observable app state. Violations emit on `ux:structural_violation`
(Error) or `ux:contract_warning` (Warn) depending on severity.

| ID | Invariant | Severity |
|----|-----------|----------|
| `S1` | Every `UxNode` with role in `{Button, TextInput, MenuItem, RadialSector, Tab, ToggleButton}` has a non-empty `label`. | Error |
| `S2` | No `UxNode` has `focused = true` and `hidden = true` simultaneously. | Error |
| `S3` | Exactly one `UxNode` in the tree has `focused = true`, or zero if the app holds no keyboard focus. | Error |
| `S4` | Every `Dialog` `UxNode` has at least one child `Button` with `UxAction::Dismiss` available. | Error |
| `S5` | Every `NodePane` with `blocked = true` has a visible child `Button` with a recovery action (label contains "Retry" or "Reload" or equivalent declared action). | Warn |
| `S6` | Every `GraphView` has at least one keyboard-accessible action for node selection (a `UxNode` with `UxAction::Invoke` and a non-empty keyboard shortcut). | Warn |
| `S7` | No two `UxNode`s in the same snapshot share the same `UxNodeId`. | Error |
| `S8` | The `RadialMenu` subtree, when present, contains between 1 and 8 `RadialSector` children. | Warn |
| `S9` | Every `UxNode` with any action listed has `bounds.width ≥ 32` and `bounds.height ≥ 32` (logical pixels). Only checked when bounds are present. | Warn |

### 3.2 Navigation Invariants (N-series)

| ID | Invariant | Severity |
|----|-----------|----------|
| `N1` | The focus traversal graph (implied by `tab_index` ordering within each region) contains no cycles within a single modal context. | Error |
| `N2` | From every open `Dialog`, the `UxAction::Dismiss` action is reachable within ≤ 10 Tab/arrow steps. | Error |
| `N3` | Top-level landmark regions (`GraphView`, `NodePane`, `ToolPane`, Toolbar) are each reachable via F6 cycling in ≤ `region_count` steps. | Warn |
| `N4` | Tab traversal from the focused node visits all `enabled = true` interactive widgets in the current modal context before returning to the start. | Warn |

### 3.3 State Machine Invariants (M-series)

| ID | Invariant | Severity |
|----|-----------|----------|
| `M1` | No `NodePane` node in the `UxTree` maps to a graph `Node` in `NodeLifecycle::Tombstone` state. | Error |
| `M2` | No `NodePane` with `TileRenderMode::Placeholder` remains in that state for more than 120 frames after the last viewer attachment attempt. | Warn |
| `M3` | The WebView creation backpressure counter does not exceed the configured burst limit during any UxScenario run. | Warn |
| `M4` | No graph node enters `RuntimeBlocked` state during a clean UxScenario run (no fault injection active). | Warn |

### 3.4 Contract Extension Points

Mods and surfaces may register additional `UxInvariant`s via the `UxContractSet` mechanism.
Every registered invariant must carry:
- A unique ID (namespaced: `mod.<mod_id>.<name>` for mod-contributed invariants).
- A `ChannelSeverity` classification.
- A probe function: `fn(&UxTree) -> Option<UxContractViolation>`.
- A human-readable description.

---

## 4. UxTree Architecture

### 4.1 Core Types

```
UxTree
  A per-frame projection of the Graphshell GUI state into a stable semantic tree.
  Rebuilt every frame in test/probe mode; skipped in release unless ux-probes is active.
  Not a persistent data structure — it is recomputed, not diffed, each frame.

UxNode
  One node in the UxTree. May be a leaf (interactive/informational) or a branch (region/pane).
  Fields: id, role, label, hint, state, value, actions, shortcuts, tab_index, bounds,
          children, metadata.

UxNodeId
  Stable, deterministic, path-based string identifier.
  Format: uxnode://{surface}/{region}/{component}
  Examples:
    uxnode://workbench/omnibar/location-field
    uxnode://workbench/tile[node:key-42]/nav-bar/back-button
    uxnode://dialog[confirm-delete]/confirm-button
    uxnode://radial-menu/sector[open-in-new-tab]
  Stability contract: a UxNodeId is identical across frames for the same semantic entity.
  Construction: derived from stable app identities (NodeKey, GraphViewId, TileId) —
                never from raw pointers, frame-local indices, or egui widget hashes.

UxRole
  Semantic role enum. Maps to AccessKit Role where a direct equivalent exists.
  Layout/structure: Landmark, Region, Dialog, Toolbar, StatusBar, MenuBar
  Pane types: GraphView, NodePane, ToolPane, WorkbenchChrome
  Interactive: Button, ToggleButton, MenuItem, RadialSector, TextInput, SearchField,
               OmnibarField, List, ListItem, Tab, TabPanel
  Informational: Heading, Text, Badge, ProgressBar, StatusIndicator
  Graph-domain: GraphNode, GraphEdge, GraphNodeGroup

UxState
  Bitfield of dynamic state.
  Fields: enabled, focused, selected, expanded (Option<bool>), hidden, blocked,
          degraded, loading.

UxAction
  Discrete action available on a UxNode in its current state.
  Values: Invoke, Focus, Dismiss, SetValue, Open, Close, ScrollTo, Expand, Collapse.

UxSnapshot
  Serializable, complete export of a UxTree. Format: YAML.
  Used for snapshot baseline storage and regression diffing.
  Human-readable; stored in tests/scenarios/snapshots/.

UxDiff
  Structured diff between two UxSnapshots.
  Separates: structural changes (node added/removed, role/label/actions changed)
             from state changes (focus, selected, loading, etc.).
  Structural changes block merge. State-only changes produce a warning artifact.
```

### 4.2 UxTree Build Algorithm

The builder runs in the egui frame loop, after tile layout, before render. It is a
read-only projection — it does not modify app state.

Build order:
1. Walk `Gui::tiles_tree` (`egui_tiles::Tree<TileKind>`).
2. Emit workbench-level landmarks: omnibar (`OmnibarField`), workbar (Tab region),
   status bar (`StatusBar`).
3. For each `TileKind::Graph(GraphViewId)`: emit a `GraphView` region. For each
   graph node at LOD ≥ Compact, emit a `GraphNode` child.
4. For each `TileKind::Node(NodePaneState)`: emit a `NodePane` region with:
   - Navigation bar sub-region (back/forward buttons, location field).
   - Viewer area sub-region (role reflects `TileRenderMode`).
   - Overlay affordances sub-region (visible focus/selection indicators).
5. For each `TileKind::Tool(ToolPaneState)`: emit a `ToolPane` region with
   subsystem-specific children.
6. Collect all open dialogs as `Dialog` nodes.
7. If the radial menu is open, emit the `radial-menu` subtree (8 `RadialSector` children).
8. Assign `UxNodeId` paths using stable app identity sources.
9. Derive `UxState` from: `NodeLifecycle`, `TileRenderMode`, focus state, dialog open state.
10. Cache built tree for the frame duration.

### 4.3 Stability Guarantee

The builder must satisfy: for any `UxNode` whose semantic identity has not changed between
frame N and frame N+1, the `UxNodeId` is identical in both snapshots. This contract
enables reliable snapshot diffing and meaningful UxScenario assertions.

---

## 5. Diagnostics Integration

### 5.1 Required Channels

| Channel | Severity | Description |
|---------|----------|-------------|
| `ux:structural_violation` | Error | A structural invariant (S-series, N-series) was violated |
| `ux:navigation_violation` | Error | A navigation invariant (N-series) was violated |
| `ux:contract_warning` | Warn | A soft invariant (Warn-severity S/M/N) was violated |
| `ux:tree_build` | Info | UxTree build completed: node count, build duration, any build errors |
| `ux:snapshot_written` | Info | A UxSnapshot was written to disk (debug mode) |
| `ux:probe_registered` | Info | A UxProbe was registered at startup |
| `ux:probe_disabled` | Warn | A UxProbe was registered but is disabled (feature gate inactive) |

### 5.2 UxViolation Event Schema

```
UxViolationEvent:
  contract_id: &'static str     // e.g., "S1", "N2", "mod.verso.viewer_label"
  message: String               // Human-readable explanation
  node_path: Option<String>     // UxNodeId of violating node (or None for tree-level)
  actual: Option<String>        // What was observed
  expected: Option<String>      // What the contract requires
  frame_index: u64              // Frame counter at violation time
```

### 5.3 Health Summary (Diagnostic Inspector)

The UX Semantics section in the Diagnostic Inspector exposes:
- UxProbe count: active / disabled / total registered.
- Violation counts per severity (Error / Warn) for the current session.
- Last violation: contract ID, node path, timestamp.
- UxTree node count (per-frame, rolling average).
- Build latency (rolling average; warn if > 1 ms).
- UxBridge: connected / disconnected / last command received.

---

## 6. Validation Strategy

### 6.1 Test Categories

**UxProbe tests (deterministic)**
- Synthesize minimal `UxTree` inputs that violate each invariant.
- Assert the probe fires and emits the correct `UxViolationEvent`.
- Assert the probe does not fire on valid inputs.
- Must be pure: no app state, no GUI, no egui context.

**UxScenario tests (integration)**
- Drive the app via `UxDriver` + `UxBridge`.
- Navigate to a checkpoint, call `GetUxSnapshot`.
- Assert invariants S1–S9 hold at the checkpoint.
- Compare snapshot to stored baseline; fail on structural diff.
- Assert no `ux:structural_violation` or `ux:navigation_violation` events in the
  diagnostics channel for the scenario duration.

**Snapshot regression tests (CI)**
- Stored in `tests/scenarios/snapshots/`.
- On every PR: run scenario suite, diff snapshots, block on structural changes.
- State-only diffs (focus, loading): attach diff artifact, warn, do not block.

**Manual smoke tests (milestone gates)**
- Run 3–5 core scenarios manually in a headed build.
- Verify the UxTree displayed in the Diagnostic Inspector reflects the visible UI.
- Verify no spurious `ux:structural_violation` events appear during normal usage.

### 6.2 CI Gates

Required checks for PRs touching:
- `shell/desktop/ui/` — UI surfaces and egui frame loop
- `shell/desktop/workbench/` — Tile tree, compositor, pane model
- `render/` — Radial menu, command palette, omnibar
- `graph_app.rs` — Intent reducer, graph state
- Any file registering new `TileKind` variants or `UxRole` extensions

Gate actions:
- Run UxProbe unit tests.
- Run core UxScenario suite (open_node, focus_cycle, modal_dismiss).
- Diff snapshots; fail on structural change without explicit baseline update.
- Assert zero `ux:structural_violation` events in scenario diagnostics logs.

### 6.3 Snapshot Baseline Policy

Stored in `tests/scenarios/snapshots/`. Diff policy:

| Field class | On mismatch |
|-------------|-------------|
| `id`, `role`, `label` | Block merge |
| `actions` (set changed) | Block merge |
| `state.enabled`, `state.hidden` | Block merge |
| `bounds` | Warn only (may vary by window size) |
| `state.focused`, `state.selected` | Warn only (transient, scenario-specific) |
| `state.loading` | Warn only (timing-sensitive) |
| `value` (text input contents) | Warn only (unless the scenario tests value) |

To update a baseline: run `cargo test --features test-utils -- --update-snapshots`.
Baseline updates require human review in PR.

---

## 7. UxBridge

The UxBridge exposes the UxTree and accepts driver commands from test harness clients.

### 7.1 Transport

**Default (Phase 1–5)**: Custom WebDriver commands, handled in
`RunningAppState::handle_webdriver_messages()`. Reuses the existing WebDriver HTTP
server and Rust client. No new IPC channel needed.

**Future (Phase 6+)**: Optional dedicated Unix socket / named pipe for sub-millisecond
probing latency during animation contract checks.

### 7.2 Command Catalogue

| Command | Input | Output |
|---------|-------|--------|
| `GetUxSnapshot` | depth limit (optional) | Full or partial `UxSnapshot` (YAML) |
| `FindUxNode` | selector (by ID, role, label, state) | Matching `UxNode` or `NotFound` |
| `InvokeUxAction` | `UxNodeId`, `UxAction` | `Ok` or `UxContractViolation` |
| `GetFocusPath` | — | Ordered `Vec<UxNodeId>` from root to focused node |
| `GetDiagnosticsState` | channel filter (optional) | Current channel event state |
| `StepPhysics` | tick count | — (advances the physics simulation N steps) |
| `SetClock` | timestamp (ms) | — (overrides monotonic clock; for animation determinism) |
| `SetInputMode` | `Mouse` / `Keyboard` / `Gamepad` | — |
| `SeedRng` | u64 | Seeds the physics RNG for deterministic layout |
| `GetActiveContracts` | — | List of registered `UxInvariant` IDs and their current status |

### 7.3 Determinism Requirements

| Source of nondeterminism | Mitigation |
|--------------------------|------------|
| Physics layout positions | `SeedRng` + `StepPhysics(N)` |
| Animation frame timing | `SetClock` override |
| WebView creation timing | Fake webview lifecycle in unit scenarios |
| Window size / DPR | `GRAPHSHELL_WINDOW_SIZE` + `GRAPHSHELL_DEVICE_PIXEL_RATIO` env vars |
| Memory pressure levels | `GRAPHSHELL_MEMORY_PRESSURE_OVERRIDE` env var |
| Backpressure cooldown timers | `SetClock` advances past cooldown window |

---

## 8. UxHarness (Test Driver Infrastructure)

The `UxHarness` is the test-side infrastructure: `UxDriver` + scenario runner +
snapshot store. Compiled only under `feature = "test-utils"`.

### 8.1 Directory Layout

```
tests/
  scenarios/
    main.rs                       # [[test]] binary entry point (existing)
    ux/                           # UxScenario definitions (YAML)
      open_node_flow.yaml
      focus_cycle.yaml
      modal_dismiss.yaml
      radial_menu_structural.yaml
    snapshots/                    # UxBaseline files (YAML, reviewed on change)
      open_node_flow_end.yaml
      focus_cycle_toolbar.yaml
  harness/
    lib.rs                        # UxHarness root
    driver.rs                     # UxDriver: high-level API (open_node, assert_tree, etc.)
    bridge_client.rs              # WebDriver HTTP client for UxBridge commands
    snapshot.rs                   # UxSnapshot serialization + baseline diffing
    contracts.rs                  # UxInvariant checkers (S/N/M series, pure functions)
    scenario_runner.rs            # YAML scenario file parser + execution loop
```

### 8.2 UxScenario File Format

```yaml
id: "flow:open-node"
description: "Opening a node creates a NodePane with a working viewer"
preconditions:
  - graph_has_nodes: 1
steps:
  - action: InvokeUxAction
    target: "uxnode://workbench/tile[graph:view-0]/graph-node[key-1]"
    action_kind: Open
  - assert: UxNodeExists
    selector: {role: NodePane, state: {loading: false}}
    within_frames: 60
  - assert: SnapshotInvariant
    invariants: [S1, S2, S3, S5, N1]
  - assert: DiagnosticsChannel
    channel: "viewer:capability_validation"
    expect_no_severity: Error
expected_end_state:
  snapshot_match: "snapshots/open_node_flow_end.yaml"
  allow_diff_keys: [bounds]
```

### 8.3 Feature Gates

```
ux-semantics        Enables UxTree builder and UxBridge server-side handling.
                    No test harness. Ships in all builds when enabled.
                    Recommended for daily development builds.

ux-probes           Enables UxProbeSet: per-frame structural invariant checking
                    (S/N/M series). Requires ux-semantics.
                    Emits ux:structural_violation events.

ux-bridge           Enables UxBridge WebDriver command handlers.
                    Requires ux-semantics. For use in test and debug builds.

test-utils          Enables full UxHarness (UxDriver, scenario runner, snapshot store).
                    Requires ux-bridge. Never included in release builds.
                    Used by: [[test]] binary targets.
```

---

## 9. Degradation Policy

### 9.1 Build Degradation

If the `ux-semantics` feature is inactive (release builds without the feature), the
UxTree builder is compiled out entirely. The UxBridge command handlers return
`Feature not enabled` error responses. No runtime overhead.

### 9.2 Runtime Degradation

| Condition | Behavior |
|-----------|----------|
| UxTree build exceeds 2 ms | Emit `ux:tree_build` Warn event with duration; skip probe evaluation for that frame |
| A UxProbe function panics | Isolate the probe; mark it failed; emit `ux:contract_warning` with probe ID; other probes continue |
| UxBridge client disconnects mid-scenario | Close the command channel; do not crash the app |
| `UxNodeId` collision detected | Emit `ux:structural_violation` for S7; flag the build as structurally invalid |

### 9.3 Probe Disabling

Individual probes can be disabled at runtime via the Diagnostic Inspector (for
debugging scenarios where a known violation is expected). Disabling a probe emits
`ux:probe_disabled` Warn.

---

## 10. Surface Capability Declarations

Each surface that contributes nodes to the UxTree must declare:

```
surface_id: String
ux_semantics_capabilities:
  tree_contribution: full | partial | none
  stable_ids: guaranteed | best_effort | none
  action_routing: full | partial | none
  bounds_reporting: full | partial | none
  state_reporting: full | partial | none
degradation_mode: full | partial | none
notes: String   // reason for any 'partial' or 'none' entries
```

Declarations are co-located with surface ownership (ViewerRegistry, CanvasRegistry,
WorkbenchSurfaceRegistry entries). They mirror the pattern established by the
Accessibility subsystem's `AccessibilityCapabilities` struct.

---

## 11. Ownership Boundaries

| Owner | Responsibility |
|-------|----------------|
| `UxTreeBuilder` (new) | Per-frame UxTree construction; `UxNodeId` path assignment; `UxState` derivation from app state. Lives in `shell/desktop/ui/ux_tree.rs`. |
| `UxProbeSet` (new) | Registration and execution of `UxInvariant` probe functions; `UxViolation` event emission; panic isolation per probe. |
| `UxBridge` (new) | WebDriver command handler extensions; serialization/deserialization of `UxSnapshot`; `InvokeUxAction` routing. Lives in `webdriver.rs` or `shell/desktop/ui/ux_bridge.rs`. |
| `UxHarness` (new, gated) | `UxDriver` client; scenario file parser; snapshot store; baseline diffing. Lives in `tests/harness/`. |
| Diagnostics Subsystem | Receives `UxViolationEvent`s via the existing channel infrastructure; stores and exposes them in the event ring. |
| Accessibility Subsystem | Consumes `UxTree` output to populate the AccessKit node builder. One builder, two consumers. |

---

## 12. AccessKit Integration Path

The UxTree is designed so its output can be directly mapped to AccessKit nodes:

| UxRole | AccessKit `Role` |
|--------|------------------|
| `Button` | `Button` |
| `ToggleButton` | `CheckBox` |
| `TextInput`, `OmnibarField`, `SearchField` | `TextInput` |
| `Dialog` | `Dialog` |
| `Tab` | `Tab` |
| `TabPanel` | `TabPanel` |
| `List` | `List` |
| `ListItem` | `ListItem` |
| `GraphView` | `ScrollView` (with `Canvas` inner) |
| `GraphNode` | `TreeItem` |
| `ToolPane`, `NodePane` | `Pane` |
| `Toolbar` | `ToolBar` |
| `StatusBar` | `StatusBar` |
| `Landmark` | `GenericContainer` + landmark flag |
| `RadialSector` | `MenuItem` |

This mapping replaces the need for the Accessibility subsystem to maintain a
separate tree construction path for native UI. The `GraphAccessKitAdapter`
(Accessibility Phase 2) consumes `UxTree` output for graph canvas nodes rather
than walking the graph model directly.

---

## 13. Current Status

**What exists** (as of 2026-03-01):
- Research report defining the system: `2026-03-01_automated_ux_testing_research.md`.
- The two infrastructure prerequisites (T1 OnceLock fix, T2 test binary split) are
  planned but not yet merged.
- No `UxTree`, `UxProbe`, `UxBridge`, or `UxHarness` code exists yet.

**What's missing / open**:
- All implementation (Phases 1–7 from the research report).
- Feature flags `ux-semantics`, `ux-probes`, `ux-bridge` do not exist in `Cargo.toml`.
- No `UxNodeId` path schema has been validated against the live tile tree structure.
- `UxBridgeCommand` handlers are not wired to the WebDriver command loop.

---

## 14. Implementation Roadmap

### Phase 0: Prerequisites
- Land T1 (OnceLock test-safety) from `2026-02-26_test_infrastructure_improvement_plan.md`.
- Land T2 step 1 (`[[test]]` binary + `test-utils` feature flag).

### Phase 1: UxTree Scaffold
- Add `ux-semantics` feature flag.
- Implement `UxNode`, `UxNodeId`, `UxRole`, `UxState`, `UxAction`, `UxSnapshot` types.
- Implement minimal builder: workbench landmarks + omnibar + open dialogs.
- Add `GetUxSnapshot` WebDriver command returning YAML stub.
- Contract: `GetUxSnapshot` returns valid YAML with at least the omnibar node.

### Phase 2: UxBridge Commands
- Implement `FindUxNode`, `InvokeUxAction`, `GetFocusPath`.
- Implement `StepPhysics`, `SeedRng`, `SetClock` for deterministic setup.
- Add `tests/harness/` crate with `UxDriver` Rust client.

### Phase 3: Structural Invariants (UxProbes)
- Implement `ux-probes` feature: S1, S2, S3, S7 probes.
- Wire `ux:structural_violation` channel to diagnostics event ring.
- Unit tests: each probe fires on synthetic violation, does not fire on valid input.
- Add S4 (dialog dismiss) and S5 (blocked node recovery action) probes.

### Phase 4: Full UxTree Coverage
- Extend builder to emit `GraphNode` children for visible graph nodes.
- Extend builder to emit `NodePane` subtrees (nav bar, viewer area, overlay affordances).
- Extend builder to emit `RadialMenu` subtree when open.
- Add S8, S9, N1–N4, M1–M4 probes.

### Phase 5: UxScenarios and Snapshot CI
- Write `open_node_flow.yaml`, `focus_cycle.yaml`, `modal_dismiss.yaml` scenarios.
- Store baselines in `tests/scenarios/snapshots/`.
- Wire snapshot diff CI gate (block on structural change).

### Phase 6: AccessKit Bridge Consumption
- Map `UxTree` output to AccessKit node builder in `shell/desktop/host/window.rs`.
- Feed `GraphAccessKitAdapter` (Accessibility Phase 2) from `UxTree` rather than
  walking the graph model directly.
- Verify OS screen reader sees Graphshell's native UI.

### Phase 7: Advanced Contracts and Chaos
- Implement `UxChaosMode` (fault injection: RuntimeBlocked, viewer failure).
- Latency contracts in UxScenario runner.
- `SemanticGravity` layout contract scenario.

---

## 15. Done Definition

The UX Semantics subsystem is fully operational when:

- `UxTree` is built every frame (in test/probe mode) with correct stable `UxNodeId`s
  for all workbench surfaces, graph views, node panes, tool panes, and dialogs.
- S1–S9, N1–N4, M1–M4 invariants are implemented as UxProbes and are green under
  normal operation.
- `GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`, `GetFocusPath` are operational
  via the UxBridge.
- Core UxScenario suite (open_node, focus_cycle, modal_dismiss) runs in CI and
  blocks on structural UxDiff.
- `UxTree` output feeds the AccessKit bridge (replacing direct graph model walking).
- New `TileKind` variants and viewer backends are required to declare
  `ux_semantics_capabilities` in their registry entries.

Until then, UX Semantics is an implementation effort. After that, it is a maintained
system property: every UI change is automatically verified against the UX contract set.
