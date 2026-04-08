# Cross-Cutting Subsystem: UX Semantics

**Status**: Active / Partial Migration
**Subsystem label**: `ux_semantics`
**Long form**: UX Semantics Subsystem
**Scope**: Runtime-queryable semantic tree of Graphshell's own native UI; UX contract verification; snapshot regression testing; UxBridge test harness integration
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `accessibility` (Accessibility), `diagnostics` (Diagnostics), `focus` (Focus)
**Doc role**: Canonical subsystem implementation guide
**Research basis**: `../../2026-03-01_automated_ux_testing_research.md`
**Related**:
- `../../technical_architecture/unified_view_model.md` — Shell host + five-domain architecture model
- `../../technical_architecture/domain_interaction_scenarios.md` — end-to-end examples of cross-domain routing and surface collaboration
- `../subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` (UxTree maps to AccessKit nodes — one tree, two consumers)
- `../subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` (UxViolation events routed through diagnostics channels)
- `../subsystem_diagnostics/2026-02-26_test_infrastructure_improvement_plan.md` (T1/T2 infrastructure — Phase 0 prerequisite)
- `PLANNING_REGISTER.md`
- `2026-04-05_command_surface_observability_and_at_plan.md`
- `2026-03-08_unified_ux_semantics_architecture_plan.md`
- `2026-03-13_chrome_scope_split_plan.md` (Navigator host chrome architecture — affects §4.2 UxTree build order and the top-level graph-scoped / workbench-scoped host landmark split)

**Policy authority**: This file is the single canonical policy authority for the UX Semantics subsystem.
Supporting UX-semantics docs may refine contracts, interfaces, and execution details, but must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

Hierarchy note:

- Shell is the application's only host and remains above all projected surfaces.
- the default graph-scoped Navigator host names graph-owned targets (`GraphId`, `GraphViewId`) one UI level above workbench hosting
- the workbench tile tree is a contextual hosting structure for the active branch's leaves
- `UxTree` must preserve that distinction in its semantic projection instead of making tile hosting look like semantic ownership

For UxTree purposes, this means semantic projection should remain legible across the five domains rather than flattening them into a single tree of apparent ownership.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6):
- **WCAG 2.2 Level AA** — S9 (32 dp minimum hit targets → SC 2.5.8), N2 (dismiss reachability → SC 2.4.11), AccessKit mapping invariants; UxTree structural invariants are behavioral implementations of WCAG requirements
- **OpenTelemetry Semantic Conventions** — `ux:*` diagnostic channel naming and severity

---

## 0A. Subsystem Policies

1. **Canonical-ux-tree policy**: UxTree is the authoritative machine-readable model of native Graphshell UI semantics.
2. **Contract-verification policy**: UX contract violations must be expressible as deterministic probe/assertion results.
3. **Snapshot-regression policy**: Structural UX changes require explicit snapshot/contract updates rather than silent drift.
4. **Bridge-separation policy**: UxBridge transports test/control commands but does not become semantic owner of UX policy.
5. **Accessibility-alignment policy**: UX semantics and accessibility mapping must stay aligned without duplicating ownership.

## 0B. Current Closure State (2026-04-07)

- UxTree runtime snapshot build/publish, core probe contracts, and diagnostics emission are active in the workbench render pipeline.
- A minimal real `UxProbeSet` is active for the currently landed core invariant wrappers, and the runtime now emits `ux:probe_registered` lifecycle receipts for active descriptors plus `ux:probe_disabled` receipts when probes are disabled after panic isolation trips.
- Point-tier graph semantic parity is now active: GraphView surfaces suppress `GraphNode` semantic children below the Point LOD threshold and project the `StatusIndicator` child labeled `Zoom in to interact with nodes.` instead.
- The UxTree -> AccessKit path is not fully closed yet as a single source-of-truth path across all surfaces.
- The bounded `3A`-`3E` contract-coverage lane is now explicitly classified and closed as a finite endpoint.
- The pre-WGPU runtime-capable contract lane is now fully closed: `M2` and `M5` joined the live probe set, and the remaining contract register is either live, scenario-only, or explicitly deferred.
- Remaining closure gaps are therefore bridge/accessibility/harness alignment plus the end-to-end mapping path for WebView bridge injection and Graph Reader virtual-tree output under the same canonical UxTree ownership model.

## 0C. Runtime Reality Split

- `UxProjection`: active
- `UxDispatch`: partially active
- `UxContracts`: active / partial
- `UxBridge`: partial / not yet general-purpose
- `UxScenarioHarness`: partial

Command-surface note:

- Shell command-surface execution carriers are landed enough that remaining work is primarily semantic observability, probe/scenario closure, and AT-aligned projection. That closure is tracked in `2026-04-05_command_surface_observability_and_at_plan.md`.

`2026-03-08_unified_ux_semantics_architecture_plan.md` is the canonical cleanup plan for this split.

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

The UX Semantics subsystem addresses these gaps by building one canonical tree that serves
three consumers: the test harness, the UxBridge, and (via mapping) the AccessKit OS bridge.

Current implementation note: the first two consumers are active; the AccessKit mapping consumer is partially active and still under closure work tracked in `../subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`.

---

## 2. Subsystem Model (Four Layers)

| Layer | UX Semantics Instantiation |
|---|---|
| **Contracts** | Structural invariants (S1–S10), navigation invariants (N1–N5), state machine invariants (M1–M5) — §3 |
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
| `S10` | When a `CommandBar` subtree is present, exactly one visible command-surface capture owner may advertise semantic focus for Shell command entry. | Error |

### 3.2 Navigation Invariants (N-series)

| ID | Invariant | Severity |
|----|-----------|----------|
| `N1` | The focus traversal graph (implied by `tab_index` ordering within each region) contains no cycles within a single modal context. | Error |
| `N2` | From every open `Dialog`, the `UxAction::Dismiss` action is reachable within ≤ 10 Tab/arrow steps. | Error |
| `N3` | Top-level landmark regions (`GraphBar`, `WorkbenchChrome`, `GraphView`, `NodePane`, `ToolPane`) are each reachable via F6 cycling in ≤ `region_count` steps. | Warn |
| `N4` | Tab traversal from the focused node visits all `enabled = true` interactive widgets in the current modal context before returning to the start. | Warn |
| `N5` | Omnibar capture exit and command-palette dismiss restore the stored valid return target or emit an explicit fallback receipt. | Warn |

### 3.3 State Machine Invariants (M-series)

| ID | Invariant | Severity |
|----|-----------|----------|
| `M1` | No `NodePane` node in the `UxTree` maps to a graph `Node` in `NodeLifecycle::Tombstone` state. | Error |
| `M2` | No `NodePane` with `TileRenderMode::Placeholder` remains in that state for more than 120 frames after the last viewer attachment attempt. | Warn |
| `M3` | The WebView creation backpressure counter does not exceed the configured burst limit during any UxScenario run. | Warn |
| `M4` | No graph node enters `RuntimeBlocked` state during a clean UxScenario run (no fault injection active). | Warn |
| `M5` | Stale omnibar/provider deliveries and no-target command-surface resolutions remain observable and do not silently replace newer visible command-surface state. | Warn |

### 3.4 Contract Extension Points

Mods and surfaces may register additional `UxInvariant`s via the `UxContractSet` mechanism.
Every registered invariant must carry:
- A unique ID (namespaced: `mod.<mod_id>.<name>` for mod-contributed invariants).
- A `ChannelSeverity` classification.
- A probe function: `fn(&UxTree) -> Option<UxContractViolation>`.
- A human-readable description.

### 3.5 Pre-WGPU Contract Classification (2026-04-07)

| Contract | Classification | Current runtime surface |
|---|---|---|
| `S1` | Live runtime probe | `ux.probe.interactive_label_presence` |
| `S2` | Explicitly deferred | Current semantic state does not project `hidden` |
| `S3` | Live runtime probe | `ux.probe.focus_uniqueness` |
| `S4` | Explicitly deferred | No `Dialog` / dismiss-button semantic subtree is projected |
| `S5` | Explicitly deferred | Blocked recovery actions are not yet semantic child nodes |
| `S6` | Explicitly deferred | Keyboard shortcut / node-selection action metadata is not projected |
| `S7` | Live runtime probe | `ux.probe.semantic_id_uniqueness` |
| `S8` | Live runtime probe | `ux.probe.radial_sector_count` |
| `S9` | Live runtime probe | `ux.probe.interactive_bounds_minimum` |
| `S10` | Live runtime probe | `ux.probe.command_surface_capture_owner` |
| `N1` | Scenario-only | Needs focus-graph / tab-order traversal not present in snapshot |
| `N2` | Scenario-only | Needs modal traversal / dismiss reachability via synthetic input |
| `N3` | Scenario-only | Needs F6 region-cycle execution rather than snapshot-only inspection |
| `N4` | Scenario-only | Needs tab traversal replay within modal context |
| `N5` | Live runtime probe | `ux.probe.command_surface_return_target` |
| `M1` | Live runtime probe | `ux.probe.node_pane_tombstone_lifecycle` |
| `M2` | Live runtime probe | `ux.probe.node_pane_placeholder_timeout` |
| `M3` | Scenario-only | Defined against clean `UxScenario` execution windows |
| `M4` | Scenario-only | Defined against clean `UxScenario` runs without fault injection |
| `M5` | Live runtime probe | `ux.probe.command_surface_observability_projection` |

Non-canonical but still-live runtime checks outside the S/N/M register remain active:
`ux.probe.presentation_id_consistency`, `ux.probe.trace_id_consistency`, and
`ux.probe.semantic_parent_links`.

---

## 4. UxTree Architecture

### 4.1 Core Types

```
UxTree
  A per-frame projection of the Graphshell GUI state into a stable semantic tree.
  In the current runtime it is rebuilt in the workbench post-render path and cached
  for same-frame/test consumers. The current Cargo shape now exposes
  `ux-semantics`, `ux-probes`, and `ux-bridge`; `ux-probes` and `ux-bridge`
  depend on `ux-semantics`, and the default desktop feature set enables both.
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
  Layout/structure: CommandBar, Landmark, Region, Dialog, Toolbar, StatusBar, MenuBar
  Pane types: GraphView, NodePane, ToolPane, WorkbenchChrome
  Interactive: Button, ToggleButton, MenuItem, RadialSector, TextInput, SearchField,
               OmnibarField, List, ListItem, Tab, TabPanel
  Informational: Heading, Text, Badge, ProgressBar, StatusIndicator
  Graph-domain: GraphNode, GraphEdge
  Deferred extension role: GraphNodeGroup (not part of the current pre-WGPU runtime closure)

UxState
  Bitfield of dynamic state.
  Fields: enabled, focused, selected, expanded (Option<bool>), hidden, blocked,
          degraded, loading.

UxAction
  Discrete action available on a UxNode in its current state.
  Values: Invoke, Focus, Dismiss, SetValue, Open, Close, ScrollTo, Expand, Collapse.

UxSnapshot
  Serializable export shape for a UxTree.
  Used for snapshot baseline storage and regression diffing.
  Current runtime note: in-memory snapshot publication is landed; file export is still
  a closure item.

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
1. Read the active chrome projection plus `Gui::tiles_tree`
   (`egui_tiles::Tree<TileKind>`).
2. Emit top-level chrome landmarks in focus order: graph-scoped Navigator host
  (`Toolbar`), workbench-scoped Navigator host (`WorkbenchChrome`) when
  present, status bar
   (`StatusBar`) when present.
3. Walk `Gui::tiles_tree`.
4. For each `TileKind::Graph(GraphViewId)`: emit a `GraphView` region representing
  a hosted presentation of a graph-owned scoped view already named by the
  graph-scoped Navigator host. For each
   graph node at LOD ≥ Compact, emit a `GraphNode` child.
5. For each `TileKind::Node(NodePaneState)`: emit a `NodePane` region with:
   - Navigation bar sub-region (back/forward buttons, location field).
   - Viewer area sub-region (role reflects `TileRenderMode`).
   - Overlay affordances sub-region (visible focus/selection indicators).
6. For each `TileKind::Tool(ToolPaneState)`: emit a `ToolPane` region with
   subsystem-specific children.
7. Collect all open dialogs as `Dialog` nodes.
8. If the radial menu is open, emit the `radial-menu` subtree (8 `RadialSector`
   children).
9. Assign `UxNodeId` paths using stable app identity sources.
10. Derive `UxState` from: `NodeLifecycle`, `TileRenderMode`, focus state,
    dialog open state.
11. Cache built tree for the frame duration.

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
| `ux:snapshot_written` | Info | A UxSnapshot was written to `GRAPHSHELL_UX_SNAPSHOT_PATH` when runtime export is enabled |
| `ux:probe_registered` | Info | A UxProbe was registered at startup |
| `ux:probe_disabled` | Warn | A UxProbe is disabled either because its descriptor is inactive or because runtime panic isolation disabled it for the session |

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

**Rust critical-path scenario tests (current integration)**
- Drive the app through `TestRegistry`, reducer/orchestration intents, and direct
  `UxTree` snapshot capture.
- Assert the pre-WGPU graph navigation, pane lifecycle, command-surface, modal,
  and degraded-viewer flows in `shell/desktop/tests/scenarios/pre_wgpu_critical_path.rs`.
- Compare normalized JSON snapshots against committed baselines in
  `tests/scenarios/snapshots/` via `shell/desktop/tests/scenarios/ux_tree_diff_gate.rs`.
- Assert diagnostics behavior directly from runtime state in the same Rust tests.

**YAML UxScenario tests (planned closure)**
- The future generic runner still targets `UxDriver` + `UxBridge` over committed
  YAML fixtures in `tests/scenarios/ux/`.
- Those fixtures are not executed by a generic runner today.

**Snapshot regression tests (CI)**
- Stored in `tests/scenarios/snapshots/` as normalized JSON baselines for the
  current Rust-first gate.
- On every PR touching the relevant surfaces: run the Rust critical-path snapshot
  gate, diff baselines, and block on semantic structural changes.
- Presentation-only or trace-only diffs can remain non-blocking depending on the
  diff-gate policy encoded in `ux_tree_diff_gate.rs`.

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
- Run the Rust critical-path scenario modules under `shell/desktop/tests/scenarios/`.
- Run the JSON snapshot diff gate against `tests/scenarios/snapshots/`.
- Run `cargo test --features test-utils --test scenarios` for the separate
  smoke/capability binary.
- YAML fixtures under `tests/scenarios/ux/` are not a merge-blocking runner gate yet.

### 6.3 Snapshot Baseline Policy

Stored in `tests/scenarios/snapshots/` as normalized JSON baselines for the
current Rust-first gate. Diff policy:

| Field class | On mismatch |
|-------------|-------------|
| `id`, `role`, `label` | Block merge |
| `actions` (set changed) | Block merge |
| `state.enabled`, `state.hidden` | Block merge |
| `bounds` | Warn only (may vary by window size) |
| `state.focused`, `state.selected` | Warn only (transient, scenario-specific) |
| `state.loading` | Warn only (timing-sensitive) |
| `value` (text input contents) | Warn only (unless the scenario tests value) |

To update the current pre-WGPU baselines:

```powershell
$env:GRAPHSHELL_UPDATE_UX_SNAPSHOTS='1'; cargo test pre_wgpu_critical_path_snapshots_match_baselines --quiet
```

Baseline updates require human review in PR.

---

## 7. UxBridge

The runtime now exposes a narrow real `UxBridge`: `shell/desktop/workbench/ux_bridge.rs`
provides in-process handlers for `GetUxSnapshot`, `FindUxNode`, and
`GetFocusPath`, plus a first real `InvokeUxAction` slice for command-surface
open/dismiss flows, pane-backed node focus/dismiss, tool-pane focus/close, and
graph-surface focus/close. The same module now also carries a small Rust-side
`UxDriver` helper that emits the reserved WebDriver execute-script payloads for
those commands, and the shared Rust-first desktop harness now consumes that
helper directly.

### 7.1 Transport

**Current (2026-04-07 follow-up)**: In-process Rust handlers, used by same-process
tests and other runtime callers. The existing WebDriver execute-script path now
also accepts a reserved `graphshell:ux-bridge:` payload prefix. Query commands
are answered immediately from the latest published snapshot; the narrow
action slice is queued onto the host graph-event path as a `WorkbenchIntent`
rather than mutating UI state directly from the host layer.

**Current transport caveat**: the host/runtime boundary still prevents direct UI
mutation inside `WebDriverRuntime`. Transport-backed actions are therefore
limited to actions that can be queued honestly and observed in a subsequent
snapshot. Today that includes command-surface open/dismiss and pane-backed node
focus/dismiss, tool-pane focus/close, and graph-surface focus/close.

**Future (Phase 6+)**: Optional dedicated Unix socket / named pipe for sub-millisecond
probing latency during animation contract checks.

### 7.2 Current and Target Command Catalogue

| Command | Input | Output | Status |
|---------|-------|--------|--------|
| `GetUxSnapshot` | — | Full `UxTreeSnapshot` | Current: in-process handler plus reserved WebDriver execute-script envelope |
| `FindUxNode` | selector (by ID, role, label, focused state) | Matching `UxSemanticNode` or `None` | Current: in-process handler plus reserved WebDriver execute-script envelope |
| `GetFocusPath` | — | Ordered `Vec<UxNodeId>` from root to focused node | Current: in-process handler plus reserved WebDriver execute-script envelope |
| `InvokeUxAction` | selector, `UxAction` | Action receipt (`Applied` in-process, `Queued` via WebDriver transport) | Current: command-surface open/dismiss, pane-backed node focus/dismiss, tool-pane focus/close, and graph-surface focus/close |
| `GetDiagnosticsState` | channel filter (optional) | Current channel event state | Target-state |
| `StepPhysics` | tick count | — (advances the physics simulation N steps) | Target-state |
| `SetClock` | timestamp (ms) | — (overrides monotonic clock; for animation determinism) | Target-state |
| `SetInputMode` | `Mouse` / `Keyboard` / `Gamepad` | — | Target-state |
| `SeedRng` | u64 | Seeds the physics RNG for deterministic layout | Target-state |
| `GetActiveContracts` | — | List of registered `UxInvariant` IDs and their current status | Target-state |

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

The current harness is mixed rather than fully generic:
- active today: Rust `TestRegistry` scenario modules under `shell/desktop/tests/scenarios/`,
  normalized JSON snapshot baselines under `tests/scenarios/snapshots/`, and a
  narrow `test-utils` integration binary at `tests/scenarios/main.rs`
- planned closure: typed `UxDriver`, generic `UxBridge`, and a YAML scenario runner

### 8.1 Directory Layout

```
shell/
  desktop/
    tests/
      harness/                    # Current Rust-side test harness utilities
      scenarios/                  # Current critical-path and diff-gate Rust tests
        pre_wgpu_critical_path.rs
        ux_tree_diff_gate.rs

tests/
  scenarios/
    main.rs                       # Separate [[test]] binary gated by `test-utils`
    ux/                           # Committed YAML fixtures for future runner closure
      facet_filter_entry_omnibar.yaml
      facet_pane_focus_return.yaml
      facet_pane_route_blocked_multiselect.yaml
      facet_pane_route_success.yaml
    snapshots/                    # Normalized JSON baselines reviewed on change
      pre_wgpu_*.json
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

Current Cargo/runtime shape:

```
desktop runtime     `ux-semantics` gates the per-frame UxTree build/publish path.
                    `ux-probes` and `ux-bridge` both depend on `ux-semantics`.
                    Default desktop features enable both today.

test-utils          Enables the narrow extra test surface that exists today:
                    `graphshell::test_utils`, the `[[test]] scenarios` binary,
                    and the Rust-first scenario modules that can call the
                    in-process query bridge. It does not imply a generic
                    transport-backed UxBridge/UxDriver stack yet.
```

---

## 9. Degradation Policy

### 9.1 Build Degradation

Current behavior with the landed feature split:

If `ux-semantics` is inactive, the post-render path clears the cached snapshot
and skips UxTree build/publish work entirely. If `ux-probes` is inactive, the
UxTree may still build for semantics consumers but probe lifecycle emission and
probe evaluation are no-op. If `ux-bridge` is inactive, the in-process query
handlers are unavailable.

### 9.2 Runtime Degradation

| Condition | Behavior |
|-----------|----------|
| UxTree build exceeds 2 ms | Emit `ux:tree_build` Warn event with duration; skip probe evaluation for that frame |
| A UxProbe function panics | Isolate the probe; mark it failed; emit `ux:contract_warning` with probe ID; other probes continue |
| UxBridge client disconnects mid-scenario | Close the command channel; do not crash the app |
| `UxNodeId` collision detected | Emit `ux:structural_violation` for S7; flag the build as structurally invalid |

### 9.3 Probe Disabling

The runtime now supports disabled probe descriptors at registration time and emits
`ux:probe_disabled` for that path. Interactive per-probe disabling via the
Diagnostic Inspector remains planned rather than implemented.

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
| `UxProbeSet` | Registration and execution of core `UxInvariant` probe functions; `UxViolation` event emission; per-probe panic isolation, suppression, and timing/budget accounting. Lives in `shell/desktop/workbench/ux_probes.rs`. |
| `UxBridge` (new) | `shell/desktop/workbench/ux_bridge.rs` plus reserved WebDriver execute-script bridge handling in `shell/desktop/host/webdriver_runtime.rs`; serializes `UxSnapshot`/node/action receipts and routes the first command-surface action slice. |
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

**What exists**:
- `UxTree` snapshot build/publish exists in the workbench render pipeline.
- A minimal real `UxProbeSet` exists for the currently landed core invariant wrappers.
- Probe lifecycle registration receipts are emitted on `ux:probe_registered` for active core descriptors.
- Probe panic isolation, runtime disablement, and one-second suppression/rate limiting are active in the probe runtime.
- `S9` (minimum 32x32 logical-pixel bounds for interactive nodes when bounds are present) is active as a live warn-class runtime probe.
- Snapshot diff-gate logic exists for semantic versus presentation changes.
- Some UX dispatch and violation diagnostics are active in orchestration/runtime code.
- Rust scenario-style tests exist for UxTree snapshot health and diff-gate policy.
- A limited set of YAML UX scenarios exists under `tests/scenarios/ux/`.

**What's missing / open**:
- Full S/N/M invariant-family closure beyond the current core registered probes.
- Transport-backed and mutating `UxBridge` closure beyond the current in-process
  query handlers (`GetUxSnapshot`, `FindUxNode`, `GetFocusPath`).
- YAML-driven `UxScenario` runner and typed `UxDriver` closure.
- Full AccessKit consumption from the canonical UxTree path.

---

## 14. Implementation Roadmap

Use `2026-03-08_unified_ux_semantics_architecture_plan.md` as the canonical roadmap overlay.

### Phase 1: Projection Closure
- Continue extending the landed `UxTree` projection to additional native surfaces.
- Validate `UxNodeId` stability and snapshot semantics against the live tile tree.

### Phase 2: Contract Engine Closure
- Extend the minimal real `UxProbeSet` beyond the current core registered probes.
- Normalize implemented checks versus specified S/N/M invariant families.
- Add the real generic probe registration/evaluation model where needed.

### Phase 3: Bridge Surface Closure
- Implement the real `UxBridge` command surface incrementally.
- Keep command transport separate from scenario logic.

### Phase 4: Scenario/Harness Closure
- Decide and implement the canonical scenario runner shape (YAML-first, Rust-first, or mixed).
- Align the actual YAML scenario inventory and CI gating with the subsystem docs.

### Phase 5: AccessKit Closure
- Feed intended accessibility consumers from the canonical UxTree projection path.

---

## 15. Done Definition

The UX Semantics subsystem is fully operational when:

- `UxProjection`, `UxContracts`, `UxDispatch`, `UxBridge`, and `UxScenarioHarness` are all explicitly closed and aligned.
- `UxTree` is built and consumed through a coherent authority path for the intended runtime/test/accessibility consumers.
- The documented bridge command surface matches the runtime command surface.
- The documented scenario/harness model matches the actual CI-tested scenario platform.
- Structural UX contract verification is a maintained system property rather than a partially-landed architecture slice.
