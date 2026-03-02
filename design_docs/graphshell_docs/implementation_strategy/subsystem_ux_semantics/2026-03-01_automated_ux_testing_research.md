# Automated UX Testing and Development Research Report

**Date**: 2026-03-01
**Status**: Research / Proposal
**Author**: Research synthesis from prior discussion + codebase audit
**Linked subsystems**: Diagnostics, Accessibility, Focus
**Linked docs**:
- `PLANNING_REGISTER.md`
- `subsystem_diagnostics/2026-02-26_test_infrastructure_improvement_plan.md`
- `subsystem_accessibility/accessibility_interaction_and_capability_spec.md`
- `subsystem_diagnostics/diagnostics_observability_and_harness_spec.md`
- `2026-02-26_composited_viewer_pass_contract.md`

---

## 1. Executive Summary

Graphshell already has the structural prerequisites for high-confidence automated UX
testing: a pure-function reducer (`apply_intents`), a reconcile boundary, a
diagnostics channel system, a WebDriver command loop, and a headless execution path.
What is currently missing is a **UX Semantics Tree** (`UxTree`) — a machine-readable,
runtime-queryable model of Graphshell's own native UI — and a **UX Contract
verification layer** that runs on top of it.

This document defines:

1. The new canonical vocabulary for automated UX testing within Graphshell.
2. The `UxTree` subsystem — architecture, terminology, hierarchy, and configuration.
3. The `UxContract` verification layer — what contracts exist, how they compose, how
   they fail.
4. The test harness extensions needed to drive and observe the app from the outside.
5. A sequenced execution plan aligned to existing lanes.
6. Research directions and Rust crate candidates.

---

## 2. What We Can and Cannot Guarantee

### 2.1 High-confidence guarantees (machine-checkable)

| Guarantee Class | Examples |
|-----------------|----------|
| **Structural completeness** | Every interactive widget has a non-empty label; every modal has a dismiss action; no pane is unreachable via focus traversal |
| **Flow reachability** | From state S with input sequence I, the app reaches state T without dead-ending or panicking |
| **Regression detection** | A refactor did not silently break the UX contract visible at the last passing snapshot |
| **Invariant preservation** | No `RuntimeBlocked` node has an active Node Pane without a visible recovery affordance |
| **Latency budgets** | Intent → reconcile → render round-trip stays under N ms at P95 |
| **Backpressure correctness** | No frame exceeds the WebView creation budget; blocked/deferred counters match expected bounds |

### 2.2 Out of scope for this system

- "Will users find this intuitive?" — requires human study.
- "Is this design optimal?" — requires comparative user research.
- "Is the visual design attractive?" — pixel aesthetics are not machine-checkable.

The correct framing: **formalize UX contracts and make the app prove it still satisfies
them after every change**.

---

## 3. New Canonical Terminology

These terms extend `TERMINOLOGY.md`. All additions are consistent with the existing
Subsystem / Aspect / Domain / Registry / Contract / Capability vocabulary.

### 3.1 Core Concepts

| Term | Definition |
|------|------------|
| **UxTree** | A runtime-queryable tree representation of Graphshell's own native UI, analogous to a platform accessibility tree. Each node carries a stable ID, role, label, state, value, actions, and navigation order. Distinct from the web content accessibility tree exposed by Servo/AccessKit. |
| **UxNode** | One node in the UxTree. Leaf nodes are interactive or informational; branch nodes are regions or panes. |
| **UxNodeId** | A stable, deterministic identifier for a UxNode. Stable across non-semantic re-renders. Constructed from a hierarchy path, not a raw pointer or frame-local index. |
| **UxRole** | The semantic role of a UxNode: `Button`, `TextInput`, `List`, `ListItem`, `Tab`, `Dialog`, `Landmark`, `GraphCanvas`, `NodePane`, `ToolPane`, `StatusRegion`, `MenuItem`, `RadialSector`, `OmnibarField`, etc. |
| **UxState** | The observable state bits of a UxNode: `enabled`, `disabled`, `focused`, `selected`, `expanded`, `collapsed`, `hidden`, `blocked`, `degraded`. |
| **UxAction** | A discrete action available on a UxNode: `Invoke`, `Focus`, `Dismiss`, `SetValue`, `Open`, `Close`, `ScrollTo`, `Expand`, `Collapse`. |
| **UxSnapshot** | A serializable point-in-time export of the full UxTree, including all node IDs, roles, labels, states, and action availability. Format: YAML or JSON. Used for snapshot diffing and regression detection. |
| **UxDiff** | A structured diff between two UxSnapshots. Highlights: added nodes, removed nodes, changed states, changed labels. Used in CI to detect unintended UX regressions. |

### 3.2 Contract Vocabulary

| Term | Definition |
|------|------------|
| **UxContract** | A machine-verifiable invariant over the UxTree or a UX flow. May be a structural property (snapshot shape), a state invariant (no unlabeled controls), or a flow property (from state A, action B reaches state C). |
| **UxContractSet** | A named, versioned collection of UxContracts applied to a specific surface or scenario. Replaces ad-hoc assertion lists. |
| **UxContractViolation** | A structured failure report from a UxContract check. Contains: contract ID, violated node path, actual vs. expected value, and a human-readable explanation. |
| **UxInvariant** | A UxContract that must hold at every observable program state (not just at flow endpoints). Examples: "no widget has an empty label," "focus is never inside a hidden pane." |
| **UxFlowContract** | A UxContract that describes a specific interaction flow: starting state, input sequence, and expected end state. Passes if the app reaches the expected state without contract violations along the way. |
| **UxScenario** | A named, reusable test scenario that exercises the app through a UxFlowContract. Backed by a scenario definition file (YAML/TOML). |
| **UxBaseline** | A stored UxSnapshot that serves as the expected state for a given scenario checkpoint. Regression tests compare live snapshots against the baseline. |

### 3.3 Infrastructure Vocabulary

| Term | Definition |
|------|------------|
| **UxDriver** | The test-side harness that sends inputs, queries the UxTree, and asserts contracts. Communicates with the running app via the UxBridge. |
| **UxBridge** | The runtime-side component that exposes the UxTree and accepts UxDriver commands. Implemented as a set of custom WebDriver commands or a dedicated IPC channel. |
| **UxBridgeCommand** | A discrete command accepted by the UxBridge: `GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`, `GetFocusPath`, `GetDiagnosticsState`, `StepPhysics`, `SetClock`. |
| **UxViolation** | A first-class diagnostic event emitted when a UxInvariant is breached at runtime (even outside test runs). Stored in the Diagnostics subsystem. Severity: `Warn` (soft invariant) or `Error` (hard invariant). |
| **UxProbe** | A lightweight assertion registered at startup that runs every frame and emits `UxViolation` events on failure. Analogous to the existing compositor chaos probe concept. |
| **UxHarness** | The full set of test infrastructure: UxDriver + UxBridge + UxProbe + scenario runner + snapshot store. |

### 3.4 Domain Aspects

| Term | Definition |
|------|------------|
| **Structural Aspect** | The static shape of the UxTree at a given moment: which nodes exist, their roles and labels. |
| **State Aspect** | The dynamic state bits of each UxNode: focus, selection, block, degradation. |
| **Navigation Aspect** | The traversal graph implied by UxNode focus order and region relationships. |
| **Action Aspect** | What actions are available at each node and in which states. |
| **Flow Aspect** | The temporal sequence of UxTree transitions produced by an input sequence. |
| **Latency Aspect** | The time taken between driver input and UxTree state change, including intent dispatch and render confirmation. |

---

## 4. The UxTree Subsystem

### 4.1 Position in the Architecture

The UxTree is a **Subsystem** (parallel to Diagnostics, Accessibility, Focus, History).
It sits between the egui render layer and the test/accessibility consumer layer.

```
                   ┌──────────────────────────────────────────┐
                   │              Graphshell App                │
  Inputs ──────────►  apply_intents() ──► reconcile_runtime()  │
                   │         │                    │             │
                   │         ▼                    ▼             │
                   │   GraphBrowserApp        EmbedderWindow    │
                   │         │                                  │
                   │         ▼                                  │
                   │      Gui frame loop                        │
                   │         │                                  │
                   │    ┌────┴─────────────────────────┐        │
                   │    │  UxTree Builder (per-frame)   │        │
                   │    │  - walks egui_tiles tree      │        │
                   │    │  - queries widget states      │        │
                   │    │  - assigns stable UxNodeIds   │        │
                   │    └──────────────┬───────────────┘        │
                   │                  │                          │
                   └──────────────────┼──────────────────────────┘
                                      │
                          ┌───────────▼───────────┐
                          │       UxBridge         │
                          │  (WebDriver channel or  │
                          │   dedicated IPC)        │
                          └───────────┬────────────┘
                                      │
                   ┌──────────────────┼──────────────────┐
                   │                  │                    │
              UxDriver           UxProbes            AccessKit
           (test harness)     (runtime checks)    (OS a11y tree)
```

### 4.2 UxNode Schema

```rust
pub struct UxNode {
    /// Stable, deterministic identifier. Path-based, not pointer-based.
    pub id: UxNodeId,
    /// Semantic role of this node.
    pub role: UxRole,
    /// Human-readable label (what a screen reader would announce).
    pub label: String,
    /// Hint text (secondary description, tooltip equivalent).
    pub hint: Option<String>,
    /// Dynamic state bits.
    pub state: UxState,
    /// Current value for input-like nodes (text contents, slider value, etc.).
    pub value: Option<String>,
    /// Available actions. Only actions valid in current state are present.
    pub actions: Vec<UxAction>,
    /// Keyboard shortcuts that activate this node.
    pub shortcuts: Vec<KeyBinding>,
    /// Focus traversal index among siblings. `None` if not in traversal.
    pub tab_index: Option<u32>,
    /// Layout bounds (logical pixels, optional — for layout sanity checks).
    pub bounds: Option<UxRect>,
    /// Child nodes.
    pub children: Vec<UxNode>,
    /// Graphshell-specific metadata (e.g. pane kind, node lifecycle state).
    pub metadata: UxMetadata,
}

pub enum UxRole {
    // Layout / Structure
    Landmark, Region, Dialog, Toolbar, StatusBar, MenuBar,
    // Pane types
    GraphView, NodePane, ToolPane, WorkbenchChrome,
    // Interactive
    Button, ToggleButton, MenuItem, RadialSector,
    TextInput, SearchField, OmnibarField,
    List, ListItem, Tab, TabPanel,
    // Informational
    Heading, Text, Badge, ProgressBar, StatusIndicator,
    // Graph-domain
    GraphNode, GraphEdge, GraphNodeGroup,
}

pub struct UxState {
    pub enabled: bool,
    pub focused: bool,
    pub selected: bool,
    pub expanded: Option<bool>,   // None if not applicable
    pub hidden: bool,
    pub blocked: bool,            // RuntimeBlocked lifecycle state
    pub degraded: bool,           // Viewer degraded / fallback
    pub loading: bool,
}
```

### 4.3 UxNodeId Construction

Stable IDs are constructed from a hierarchy path, not from frame-local indices:

```
uxnode://workbench/omnibar/location-field
uxnode://workbench/tile[graph:view-0]/graph-canvas
uxnode://workbench/tile[node:key-42]/viewer-pane
uxnode://workbench/tile[node:key-42]/viewer-pane/nav-bar/back-button
uxnode://workbench/tile[tool:diagnostics]/inspector/channel-list
uxnode://dialog/confirm-delete/confirm-button
uxnode://radial-menu/sector[open-in-new-tab]
```

Path components:
- `workbench` — the top-level workbench chrome.
- `tile[kind:id]` — a tile keyed by its TileKind discriminant and stable GraphViewId or NodeKey.
- `dialog[name]` — a named dialog surface.
- `radial-menu` — the radial menu (when open).
- Leaf names are kebab-case semantic identifiers, not widget type names.

### 4.4 UxTree Generation

The UxTree builder runs during the egui frame loop, after tile layout but before render.
It does **not** modify app state — it is a read-only projection of GUI state into the
semantic tree.

**Build algorithm** (per frame, for testability):

1. Walk `Gui::tiles_tree` (the `egui_tiles::Tree<TileKind>`).
2. For each `TileKind::Graph`: emit a `GraphView` region node with `GraphCanvas` children
   (one per visible node above the LOD threshold).
3. For each `TileKind::Node`: emit a `NodePane` region with viewer-specific children
   (nav bar, viewer area, overlay affordances).
4. For each `TileKind::Tool`: emit a `ToolPane` region with subsystem-specific children.
5. Collect omnibar, workbar, and status bar surfaces as `Toolbar`/`StatusBar` landmarks.
6. Collect open dialogs (confirm, error, prompt) as `Dialog` nodes.
7. If the radial menu is open, emit a `radial-menu` subtree.
8. Assign `UxNodeId` paths, derive `UxState` from `NodeLifecycle`, viewer
   `TileRenderMode`, and focus state.
9. Cache the built tree for the frame.

The tree is rebuilt each frame. The **stable ID contract** means that between builds, a
node whose semantic identity has not changed will have the same `UxNodeId`.

---

## 5. The UxBridge

### 5.1 Transport

Two transport options are viable:

**Option A: Custom WebDriver commands** (recommended for now)
- Extend the existing `WebDriverCommandMsg` handler in `running_app_state.rs`.
- Add `GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`, `GetFocusPath`,
  `GetDiagnosticsState`, `StepPhysics`, `SetClock` to the custom command namespace.
- Reuses the existing WebDriver HTTP server and Rust client library.
- No new IPC channel needed.
- Limitation: serializes through HTTP JSON; fine for test use, not for high-frequency
  probing.

**Option B: Dedicated IPC channel**
- Unix socket (Linux/macOS) or named pipe (Windows).
- Binary protocol (e.g., bincode or messagepack).
- Needed only when sub-millisecond probing latency is required (e.g., latency contract
  verification during animation).

For the initial implementation, **Option A** is sufficient.

### 5.2 UxBridgeCommand Catalogue

| Command | Input | Output |
|---------|-------|--------|
| `GetUxSnapshot` | depth limit (optional) | Full or partial `UxSnapshot` (YAML/JSON) |
| `FindUxNode` | selector (by ID, role, label, state) | Matching `UxNode` or error |
| `InvokeUxAction` | `UxNodeId`, `UxAction` | Success / `UxContractViolation` |
| `GetFocusPath` | — | Ordered list of `UxNodeId` from root to focused node |
| `GetDiagnosticsState` | channel filter (optional) | Current `DiagnosticsState` for requested channels |
| `StepPhysics` | tick count | — (steps the force-directed simulation N times without real time) |
| `SetClock` | timestamp | Overrides the app's monotonic clock (for animation determinism) |
| `SetInputMode` | `Mouse`, `Keyboard`, `Gamepad` | — |
| `SeedRng` | u64 seed | Seeds the physics RNG for deterministic layout |

---

## 6. UxContracts: Invariants and Flow Contracts

### 6.1 Structural Invariants (always-on UxProbes)

These run every frame in test mode (and optionally in release builds behind a feature
flag). Violations emit `UxViolation` diagnostics events.

| ID | Invariant | Severity |
|----|-----------|----------|
| `S1` | Every `UxNode` with `role ∈ {Button, TextInput, MenuItem, RadialSector, Tab}` has a non-empty `label`. | Error |
| `S2` | No `UxNode` has `focused = true` and `hidden = true` simultaneously. | Error |
| `S3` | Exactly one `UxNode` has `focused = true` in the entire tree (or zero if no focusable widget exists). | Error |
| `S4` | Every `Dialog` node has at least one `Button` child with `UxAction::Dismiss` available. | Error |
| `S5` | Every `NodePane` with `blocked = true` has a visible `Button` child with label matching the recovery action (e.g., "Retry", "Reload"). | Warn |
| `S6` | Every `GraphView` has at least one accessible keyboard action for node selection. | Warn |
| `S7` | No `UxNodeId` is duplicated within a single snapshot. | Error |
| `S8` | The `RadialMenu` subtree, when present, has exactly 8 `RadialSector` children (or ≤ 8 if some sectors are inactive). | Warn |
| `S9` | Every interactive widget has bounds where `width ≥ 32` and `height ≥ 32` (logical pixels). | Warn |

### 6.2 Navigation Invariants

| ID | Invariant | Severity |
|----|-----------|----------|
| `N1` | The focus traversal graph (implied by `tab_index` and region relationships) is acyclic within a single modal context. | Error |
| `N2` | From every modal `Dialog`, F6 / Tab / Escape reaches a `Dismiss` action within ≤ 10 steps. | Error |
| `N3` | Top-level landmark regions (`GraphView`, `NodePane`, `ToolPane`, `Toolbar`) are reachable from each other via the F6 focus cycle in ≤ `region_count` steps. | Warn |
| `N4` | Tab traversal starting from the focused node visits all enabled interactive widgets in the current modal context before returning to the starting node. | Warn |

### 6.3 State Machine Invariants

| ID | Invariant | Severity |
|----|-----------|----------|
| `M1` | A `NodePane` with `NodeLifecycle::Tombstone` is never visible in the tile tree. | Error |
| `M2` | A `NodePane` with `TileRenderMode::Placeholder` does not remain in that mode for more than N frames after the viewer attachment window has closed. | Warn |
| `M3` | The `WebView creation backpressure` counter never exceeds the configured burst limit during a normal scenario run. | Warn |
| `M4` | No node enters `RuntimeBlocked` state during a clean scenario run (no injected failures). | Warn |

### 6.4 Flow Contracts (UxScenarios)

These are defined as YAML scenario files in `tests/scenarios/ux/`:

```yaml
# tests/scenarios/ux/open_node_flow.yaml
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
  allow_diff_keys: [bounds]  # bounds may vary by window size
```

### 6.5 Snapshot Diffing Policy

Snapshots are stored in `tests/scenarios/snapshots/`. The diff policy:

- **Exact match required**: `id`, `role`, `label`, `state.enabled`, `state.hidden`,
  `actions`.
- **Allowed to vary**: `bounds`, `value` (text input contents in non-value-contract tests),
  `state.loading` (transient).
- **Diff output**: structured YAML showing what changed, with `UxNodeId` paths.
- **Review gate**: any structural diff (new node, removed node, role change, label change)
  blocks merge. State-only diffs produce a warning with a diff artifact attached to the PR.

---

## 7. Test Harness Extensions

### 7.1 Determinism Requirements

For UxFlowContracts to be reliable, the following must be deterministic in test mode:

| Source of nondeterminism | Mitigation |
|--------------------------|------------|
| Force-directed layout positions | `SeedRng` UxBridgeCommand + fixed tick count via `StepPhysics` |
| Animation timing | `SetClock` UxBridgeCommand; disable easing curves in test mode |
| WebView creation timing | Fake WebView lifecycle in unit scenarios; real lifecycle in integration scenarios with deterministic pages |
| Window / device pixel ratio | Fixed via `GRAPHSHELL_WINDOW_SIZE` and `GRAPHSHELL_DEVICE_PIXEL_RATIO` env vars (already supported) |
| Memory pressure levels | Override via `GRAPHSHELL_MEMORY_PRESSURE_OVERRIDE=Normal` test env var |
| Backpressure cooldown timers | `SetClock` overrides monotonic time, allowing instant-forward of cooldown windows |

### 7.2 Headless Execution

UxScenarios run in headless mode:
- `GRAPHSHELL_HEADLESS=1` — no OS window, offscreen render target.
- Renderer: Mesa software rasterizer (`LIBGL_ALWAYS_SOFTWARE=1`) on Linux CI;
  DirectX WARP on Windows CI.
- Screenshot capture via existing `TakeScreenshot` WebDriver command for visual
  regression evidence (not for structural contract checking).

### 7.3 UxDriver Crate Structure

```
tests/
  scenarios/
    main.rs                   # [[test]] binary entry point (existing)
    ux/                       # New: UxScenario definitions (YAML)
      open_node_flow.yaml
      focus_cycle.yaml
      ...
  harness/
    lib.rs                    # UxDriver + UxBridge client
    snapshot.rs               # UxSnapshot serialization + diffing
    contracts.rs              # UxInvariant checkers
    driver.rs                 # High-level API: open_node(), assert_tree(), etc.
```

The `harness` crate is a `dev-dependency` of the main crate, compiled only when
`test-utils` feature is enabled (per the existing T2 plan in
`2026-02-26_test_infrastructure_improvement_plan.md`).

---

## 8. The UxViolation Diagnostic Channel

UxProbes emit `UxViolation` events through the Diagnostics subsystem.

```rust
pub struct UxViolationEvent {
    /// Contract ID (e.g., "S1", "N2").
    pub contract_id: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Path of the violating node (or empty if tree-level).
    pub node_path: Option<String>,
    /// The actual value that violated the contract.
    pub actual: Option<String>,
    /// The expected value according to the contract.
    pub expected: Option<String>,
}
```

Channel descriptors:

```rust
// Structural invariant violations
DiagnosticChannelDescriptor {
    channel_id: "ux:structural_violation",
    schema_version: 1,
    severity: ChannelSeverity::Error,
}

// Navigation invariant violations
DiagnosticChannelDescriptor {
    channel_id: "ux:navigation_violation",
    schema_version: 1,
    severity: ChannelSeverity::Error,
}

// Soft/warning-level invariant violations
DiagnosticChannelDescriptor {
    channel_id: "ux:contract_warning",
    schema_version: 1,
    severity: ChannelSeverity::Warn,
}

// UxTree build status (for debugging tree generation itself)
DiagnosticChannelDescriptor {
    channel_id: "ux:tree_build",
    schema_version: 1,
    severity: ChannelSeverity::Info,
}
```

---

## 9. Rust Crate Candidates

| Role | Crate | Notes |
|------|-------|-------|
| Accessibility tree / platform bridge | `accesskit` | Existing in the codebase; `UxTree` maps onto AccessKit nodes for OS accessibility. Use AccessKit node types as the canonical vocabulary where possible. |
| YAML serialization for snapshots | `serde_yaml` | Snapshot format. Human-readable diffs in PRs. |
| Snapshot diffing | `similar` | Structural text diffing for YAML snapshots in CI output. |
| Property-based testing (UxInvariants) | `proptest` | Generate random valid `UxSnapshot` inputs and verify invariants hold under mutation. |
| Scenario file parsing | `serde` + `toml` or `serde_yaml` | Parse `.yaml` or `.toml` scenario definitions. |
| Deterministic RNG | `rand` (already present) + seeded `SmallRng` | For physics simulation seeding. |
| Fake time / clock injection | `tokio::time::pause` (if async) or custom `MonotonicClock` trait | For cooldown/backpressure timer overrides. |
| WebDriver client (UxDriver) | `fantoccini` | Existing Rust WebDriver client library; handles the HTTP WebDriver protocol. |
| Latency measurement | `quanta` | High-resolution monotonic clock for latency contract checks. |
| Screenshot comparison | `image` + custom SSIM or pixel diff | For visual regression evidence. Not for structural contracts. |
| Fuzz testing (input sequences) | `cargo-fuzz` / `libfuzzer-sys` | Long-term: fuzz UxBridgeCommand sequences for crash discovery. |

### 9.1 AccessKit Integration Note

`accesskit` is the natural vocabulary for UxRole and UxAction, since we already plan
an AccessKit bridge for OS accessibility. The mapping is:

| UxRole | AccessKit `Role` |
|--------|-----------------|
| `Button` | `Button` |
| `TextInput` | `TextInput` |
| `Dialog` | `Dialog` |
| `GraphCanvas` | `Canvas` (or custom) |
| `GraphNode` | `TreeItem` or custom |
| `Landmark` | `GenericContainer` with `landmark` flag |

Building `UxTree` on top of AccessKit node types means the same data structure serves
both automated testing and OS screen reader integration — one implementation, two
consumers.

---

## 10. Subsystem Configuration

The UxTree subsystem is controlled by the following configuration surface:

### 10.1 Compile-time Gates

```toml
[features]
# Enables UxTree builder and UxBridge (no test harness)
ux-semantics = []

# Enables UxProbes (structural invariant checking, every frame)
ux-probes = ["ux-semantics"]

# Enables UxBridge WebDriver commands
ux-bridge = ["ux-semantics"]

# Enables full test harness (UxDriver, snapshot store, scenario runner)
test-utils = ["ux-semantics", "ux-bridge"]
```

### 10.2 Runtime Configuration

| Env var | Values | Effect |
|---------|--------|--------|
| `GRAPHSHELL_UX_PROBES=1` | 0/1 | Enable UxProbe structural invariant checks every frame (default: 0 in release, 1 in test) |
| `GRAPHSHELL_UX_SNAPSHOT_PATH` | path | Write a UxSnapshot YAML on every frame to this path (for debugging) |
| `GRAPHSHELL_UX_BRIDGE_PORT` | port | Override WebDriver port for UxBridge commands |
| `GRAPHSHELL_HEADLESS=1` | 0/1 | Headless mode; offscreen render target |
| `GRAPHSHELL_SEED_RNG` | u64 | Seed the physics RNG for deterministic layout |
| `GRAPHSHELL_MEMORY_PRESSURE_OVERRIDE` | Normal/Warning/Critical | Force memory pressure level |

### 10.3 UxContractSet Configuration

Each surface or subsystem declares its UxContractSet in its subsystem doc. Contracts
are tagged by surface scope:

```
Domain: workbench_surface
  UxContractSet: workbench_structural
    Invariants: S1, S2, S3, S7, N3
    Probes: enabled by default in ux-probes feature

Domain: node_viewer_surface
  UxContractSet: node_viewer_flow
    Invariants: S1, S5, M1, M2
    Probes: enabled by default

Domain: graph_canvas_surface
  UxContractSet: graph_canvas_navigation
    Invariants: S6, N1, N4
    Probes: enabled by default

Domain: radial_menu_surface
  UxContractSet: radial_menu_structural
    Invariants: S1, S8, S9
    Probes: enabled by default
```

---

## 11. Research Directions

### 11.1 Model-Based Testing

**Idea**: Represent the app's observable UX state as a finite state machine (or a typed
graph of `UxSnapshot → input → UxSnapshot` transitions). Use a model checker to
automatically find input sequences that produce invariant violations.

**Relevance to Graphshell**: The `GraphIntent` reducer is already close to a pure
state machine. A `UxStateMachine` wrapper could:
- Define the set of valid states (nodes in the machine).
- Define the set of valid transitions (edges = `UxBridgeCommand` sequences).
- Use exhaustive or bounded model checking (e.g., via `proptest` or `kani`) to
  find sequences that reach invalid states.

**Research reference**: Kani Rust Verifier (AWS open source) — bounded model
checking for Rust. Applicable to the `apply_intents` function body for
invariant verification without running the full app.

### 11.2 Chaos Engineering for UX

**Idea**: The `2026-02-26_composited_viewer_pass_contract.md` already introduces
"Compositor Pass Chaos Engineering" for rendering. Extend the same principle to UX:

- Randomly inject `RuntimeBlocked` states for nodes.
- Randomly inject viewer backend failures (`TileRenderMode::Placeholder` fallback).
- Randomly drop WebView creation requests.
- Verify that UxInvariants `S5`, `M3`, `M4` still hold under these injected failures.

**Implementation**: A `UxChaosMode` configuration (similar to the existing compositor
chaos mode) that activates random fault injection during test runs.

### 11.3 Accessibility Tree Snapshot Testing (Playwright-style)

**Idea**: For web content displayed in Node Panes, use AccessKit's serializable tree
(already planned for OS integration) to snapshot the content's accessibility tree.
Store YAML baselines. Diff in CI.

**Benefit**: Catches regressions where a Servo/Wry update changes the accessibility
tree of rendered pages — a class of bug invisible to functional tests.

**Existing art**: Playwright's `aria-snapshots` feature does exactly this for web
apps. The same YAML format and diff semantics can be adopted for Graphshell's content
tree without inventing a new format.

### 11.4 Latency Contract Verification

**Idea**: Define explicit latency contracts for key UX flows:

| Flow | Budget |
|------|--------|
| Intent dispatch → UxTree state updated | < 2 frames (33 ms at 60 fps) |
| Node open → NodePane `loading: false` | < 5 s (network-independent page) |
| Focus transfer between panes | < 1 frame |
| Radial menu open → all sectors visible | < 2 frames |

Verify these in UxScenarios by recording timestamps via `quanta` at:
- `UxBridgeCommand` dispatch time.
- Next `GetUxSnapshot` showing expected state.

Emit `UxViolation` events on the `ux:contract_warning` channel when budgets are
exceeded.

### 11.5 Semantic Gravity Testing

**Idea**: `SemanticGravity` (a canonical Graphshell term from `TERMINOLOGY.md`)
controls how related nodes cluster in the force-directed layout. A UxScenario
could verify that after `StepPhysics(N)`, nodes with the same tag are within a
specified spatial proximity — a layout contract.

This is a domain-specific UX contract unique to Graphshell's spatial interface.

---

## 12. Sequenced Execution Plan

### Phase 0: Foundations (prerequisite, no new code)

- [ ] Verify T1 and T2 from `2026-02-26_test_infrastructure_improvement_plan.md` are merged.
- [ ] Confirm `tests/scenarios/main.rs` binary is working in CI.
- [ ] Confirm `test-utils` feature flag is operational.

### Phase 1: UxTree Scaffold

- [ ] Add `ux-semantics` feature flag to `Cargo.toml`.
- [ ] Define `UxNode`, `UxNodeId`, `UxRole`, `UxState`, `UxAction`, `UxSnapshot` types
  in `shell/desktop/ui/ux_tree.rs`.
- [ ] Implement minimal UxTree builder: workbench landmarks + omnibar + open dialogs.
  No graph canvas or node pane internals yet.
- [ ] Add `GetUxSnapshot` WebDriver command returning a JSON stub.
- [ ] Test: `cargo test --features test-utils` — GetUxSnapshot returns valid JSON with
  at least the omnibar node.

### Phase 2: UxBridge Commands

- [ ] Implement `FindUxNode`, `InvokeUxAction`, `GetFocusPath`.
- [ ] Implement `StepPhysics` and `SeedRng` for deterministic scenario setup.
- [ ] Implement `GetDiagnosticsState` UxBridge command (wraps existing diagnostics
  channel access).
- [ ] Add `tests/harness/` crate with `UxDriver` Rust client.

### Phase 3: Structural Invariants (UxProbes)

- [ ] Implement `ux-probes` feature: S1, S2, S3, S7 as runtime probes.
- [ ] Add `DiagnosticChannelDescriptor` for `ux:structural_violation`.
- [ ] Verify probes fire on synthetic violations (unit test).
- [ ] Add S4 (dialog dismiss), S5 (blocked node recovery action).

### Phase 4: Graph Canvas and Node Pane UxTree Nodes

- [ ] Extend UxTree builder to emit `GraphNode` children for visible graph nodes
  (LOD ≥ Compact).
- [ ] Extend UxTree builder to emit `NodePane` subtrees (nav bar, viewer area).
- [ ] Extend UxTree builder to emit `RadialMenu` subtree when radial menu is open.
- [ ] Add `S8`, `S9` invariants.

### Phase 5: UxScenarios

- [ ] Write `open_node_flow.yaml` scenario.
- [ ] Write `focus_cycle.yaml` scenario (F6 traversal of all pane regions).
- [ ] Write `modal_dismiss.yaml` scenario (open confirm dialog, dismiss via keyboard).
- [ ] Store baseline snapshots in `tests/scenarios/snapshots/`.
- [ ] Integrate snapshot diffing into CI — fail on structural diffs.

### Phase 6: AccessKit Bridge

- [ ] Map `UxNode` → AccessKit `Node` (role, label, state, actions).
- [ ] Wire AccessKit bridge to the UxTree builder output.
- [ ] Verify OS screen reader sees Graphshell's native UI on Windows (Narrator) and
  Linux (Orca).

### Phase 7: Advanced Contracts and Chaos

- [ ] Implement `UxChaosMode` — random fault injection (RuntimeBlocked, viewer failure).
- [ ] Add latency contract checking to `UxScenario` runner.
- [ ] Add `SemanticGravity` layout contract scenario.
- [ ] Investigate Kani model checking for `apply_intents` invariants.

---

## 13. Alignment with Existing Subsystem Docs

| Existing doc | This report's dependency |
|-------------|--------------------------|
| `subsystem_diagnostics/2026-02-26_test_infrastructure_improvement_plan.md` | Phase 0 (T1/T2 must land first) |
| `subsystem_diagnostics/diagnostics_observability_and_harness_spec.md` | `GetDiagnosticsState` UxBridge command reuses diagnostics channel API |
| `subsystem_accessibility/accessibility_interaction_and_capability_spec.md` | UxTree structural invariants are a superset of the accessibility capability contract |
| `2026-02-26_composited_viewer_pass_contract.md` | `TileRenderMode` determines how NodePane subtrees are built in the UxTree |
| `2026-03-01_ux_execution_control_plane.md` | UxInvariants S1-S9 are the machine-checkable subset of the UX baseline gate |
| `2026-02-28_ux_contract_register.md` | UxContractSets should be registered in the UX contract register |

---

## 14. Summary: What This Gives You

| Capability | How |
|------------|-----|
| "Every control has a label" | S1 UxProbe, fires every frame in test mode |
| "No focus trap in modal" | N1/N2 UxProbe + focus_cycle.yaml scenario |
| "Blocked node has recovery action" | S5 UxProbe |
| "Opening a node reaches a working viewer" | open_node_flow.yaml UxScenario |
| "Refactor didn't break the UX shape" | Snapshot diffing in CI |
| "OS screen reader works" | AccessKit bridge consuming UxTree output |
| "Latency is within budget" | Latency contract in UxScenario runner |
| "Chaos faults produce visible recovery UI" | UxChaosMode + S5 invariant |
