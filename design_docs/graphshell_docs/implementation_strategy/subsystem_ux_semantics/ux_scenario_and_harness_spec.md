# UxScenario and UxHarness Spec

**Date**: 2026-03-01
**Status**: Canonical subsystem contract
**Priority**: Pre-renderer/WGPU required

**Related**:
- `SUBSYSTEM_UX_SEMANTICS.md`
- `ux_tree_and_probe_spec.md`
- `../subsystem_diagnostics/2026-02-26_test_infrastructure_improvement_plan.md`
- `../subsystem_diagnostics/diagnostics_observability_and_harness_spec.md`
- `../../2026-03-01_automated_ux_testing_research.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **UxScenario** format, the **UxDriver**
client library, the **UxBridge** server-side command handlers, and the **snapshot
regression** CI gate.

It governs:

- UxScenario file format and execution semantics
- UxDriver API surface and error handling contracts
- UxBridge command contracts (input, output, error cases)
- Snapshot baseline storage, diffing, and update policy
- CI gate requirements
- Determinism requirements for repeatable scenario runs

It does **not** govern:

- UxTree construction (see `ux_tree_and_probe_spec.md`)
- UxProbe invariant definitions (see `SUBSYSTEM_UX_SEMANTICS.md §3`)
- AccessKit bridge mapping (see `SUBSYSTEM_ACCESSIBILITY.md`)

---

## 2. Canonical Model

The UxScenario/UxHarness system has three distinct components:

1. **UxScenario definitions** — declarative YAML files describing a flow, assertions,
   and expected end state. They are the source of truth for what the harness tests.
2. **UxDriver** — a Rust library (test-side) that parses scenarios, drives the app via
   UxBridge commands, and evaluates assertions.
3. **UxBridge** — the app-side handler (server) that accepts `UxBridgeCommand` messages
   and responds with `UxBridgeResponse` payloads.

These components must remain separate. Scenario YAML is not executed directly by the
app. The UxDriver is never linked into the app. The UxBridge does not implement
scenario logic.

---

## 3. Normative Core

### 3.1 Scenario Execution Contract

**E1 — Atomic precondition evaluation**
All `preconditions` in a scenario are evaluated before any `steps` execute. If any
precondition fails, the scenario is skipped with a `Precondition not met` result —
not a test failure. Skipped scenarios are reported separately.

**E2 — Sequential step execution**
Steps execute in declaration order. A step failure (assertion fails, action returns
error) stops the scenario and reports a `Failed` result with the step index and error.
Subsequent steps do not run.

**E3 — Frame budget per step**
Every assertion step that waits for a state transition has a `within_frames` field
(default: 120 frames ≈ 2 seconds at 60 fps). If the expected state is not reached
within the budget, the step fails with `Timeout`.

**E4 — Invariant assertions are cumulative**
`assert: SnapshotInvariant` checks run against the current live snapshot at the
moment of assertion. They do not check historical frames. Invariants specified in
the step are checked; invariants not listed are not checked at that step.

**E5 — Diagnostics state is checked per-step**
`assert: DiagnosticsChannel` checks query the app's current diagnostics event ring
for events since the scenario started (or since a `ClearDiagnostics` step). They
do not retroactively observe events before scenario start.

**E6 — Clean scenario state**
Before a scenario starts, the UxDriver sends:
- `SeedRng` with the scenario's configured seed (default: 0).
- `SetClock` to a fixed base timestamp.
- `SetInputMode` to the scenario's configured mode (default: `Keyboard`).

These commands ensure a deterministic starting condition. They do not reset the
app's graph state — that is controlled by `preconditions`.

### 3.2 UxDriver API Contract

The `UxDriver` is the primary test-facing API. It provides:

**D1 — Command/response symmetry**
Every `UxBridgeCommand` sent by the driver has a corresponding `UxBridgeResponse`.
Commands are synchronous from the driver's perspective: `send_command` blocks until
the response arrives or a timeout elapses.

**D2 — Typed assertion methods**
The driver exposes typed assertion methods rather than raw command invocation:
```
driver.assert_node_exists(selector)      → Ok(UxNode) | Err(AssertionError)
driver.assert_snapshot_invariant(ids)    → Ok(()) | Err(Vec<UxContractViolation>)
driver.assert_no_diagnostics_errors(ch)  → Ok(()) | Err(Vec<DiagnosticEvent>)
driver.snapshot()                        → Ok(UxSnapshot) | Err(BridgeError)
driver.invoke_action(id, action)         → Ok(()) | Err(UxContractViolation | BridgeError)
```

**D3 — Selector syntax**
Selectors for `FindUxNode` and `assert_node_exists` support:
```
by_id("uxnode://workbench/omnibar/location-field")
by_role(UxRole::Button)
by_label("Back")
by_role_and_label(UxRole::Button, "Back")
by_state(|s| s.focused)
```
If a selector matches multiple nodes, the first match (tree order, depth-first) is
returned. If zero nodes match, the command returns `NotFound`.

**D4 — Error types**
`BridgeError` — transport-level failure (connection lost, timeout, malformed response).
`AssertionError` — the assertion condition was not satisfied.
`UxContractViolation` — `InvokeUxAction` returned a violation (the action was
  attempted but the app reported a contract breach during execution).
`NotFound` — a node selector matched zero nodes.
`Precondition` — a scenario precondition was not met.

**D5 — Timeout policy**
Default command timeout: 5 seconds. Default `within_frames` wait: 120 frames.
Both are overridable per-step in YAML or per-call in the driver API. Timeout is
not a `BridgeError` — it produces `AssertionError::Timeout`.

### 3.3 UxBridge Server Contract

**B1 — Stateless responses**
Each `UxBridgeCommand` is handled independently. The UxBridge holds no per-connection
state. If the connection drops between commands, the next command starts fresh.

**B2 — Snapshot freshness**
`GetUxSnapshot` always returns the snapshot from the **most recently completed frame**.
It does not trigger a new frame. The UxDriver is responsible for waiting an appropriate
number of frames before requesting a snapshot (via `within_frames` semantics).

**B3 — InvokeUxAction routing**
`InvokeUxAction(id, action)` must:
1. Find the `UxNode` by `id` in the current snapshot.
2. Verify `action` is in the node's `actions` list. If not: return
   `UxContractViolation { contract_id: "action_not_available", node_path: id, ... }`.
3. Route the action to the correct app subsystem:
   - `Invoke` / `Focus` / `Dismiss` on navigation elements → emit `GraphIntent` variants.
   - `SetValue` on `OmnibarField` → update the omnibar text field state.
   - `Open` on `GraphNode` → emit `GraphIntent::PromoteNodeToActive` and open a pane.
   - `Close` on `NodePane` → emit the tile-close intent.
4. Return `Ok(())` on successful dispatch. The driver waits for the effect to be
   observable in a subsequent snapshot.

**B4 — StepPhysics semantics**
`StepPhysics(n)` advances the force-directed simulation by exactly `n` ticks without
advancing wall-clock time. After `n` ticks, the app pauses physics until the next
`StepPhysics` or until normal frame-driven simulation resumes. This command is only
valid when `SetClock` has been used to take manual control of time.

**B5 — SetClock semantics**
`SetClock(ms)` overrides the app's monotonic clock to the given timestamp. All
time-dependent behavior (backpressure cooldowns, animation easing, lifecycle timeouts)
uses this clock while the override is active. The clock does not advance automatically
after `SetClock` — subsequent calls to `SetClock` with a larger value simulate time
passing. `SetClock(None)` releases the override and returns to wall-clock time.

**B6 — Error response format**
All error responses carry: `error_kind` (string enum), `message` (human-readable),
and optionally `node_path` and `context` (arbitrary key-value pairs for debugging).

---

## 4. UxScenario File Format

Scenario files are YAML, stored in `tests/scenarios/ux/`. The runner discovers all
`*.yaml` files in this directory.

### 4.1 Top-Level Fields

```yaml
id: "flow:open-node"           # Unique scenario ID. Convention: "{category}:{name}"
description: "..."             # Human-readable purpose
tags: [core, navigation]       # Optional tags for filtering (e.g., cargo test -- --tag core)
seed_rng: 0                    # Physics RNG seed (default: 0)
input_mode: Keyboard           # Mouse | Keyboard | Gamepad (default: Keyboard)
preconditions:                 # List of precondition checks (all must pass)
  - graph_has_nodes: 1
steps:                         # Ordered list of actions and assertions
  - ...
expected_end_state:            # Optional: snapshot comparison at scenario end
  snapshot_match: "snapshots/open_node_flow_end.yaml"
  allow_diff_keys: [bounds]    # Fields exempt from structural diff
```

### 4.2 Precondition Types

```yaml
preconditions:
  - graph_has_nodes: 1                    # Graph contains at least N nodes
  - graph_has_no_blocked_nodes: true      # No RuntimeBlocked nodes
  - feature_active: ux-probes            # Cargo feature is enabled
  - input_mode_is: Keyboard              # Current input mode matches
```

### 4.3 Step Types

**Action steps** — send a command to the app:

```yaml
- action: InvokeUxAction
  target: "uxnode://workbench/tile[graph:a1b2...]/graph-node[1]"
  action_kind: Open

- action: SetValue
  target: "uxnode://workbench/omnibar/location-field"
  value: "https://example.com"

- action: StepPhysics
  ticks: 300

- action: SetClock
  ms: 5000
```

**Assertion steps** — check app state:

```yaml
- assert: UxNodeExists
  selector: {role: NodePane, state: {loading: false}}
  within_frames: 60

- assert: UxNodeAbsent
  selector: {role: Dialog}

- assert: SnapshotInvariant
  invariants: [S1, S2, S3, S4, S5, N1]

- assert: NoUxViolations
  # asserts zero events on ux:structural_violation and ux:navigation_violation
  # since scenario start

- assert: DiagnosticsChannel
  channel: "viewer:capability_validation"
  expect_no_severity: Error

- assert: FocusPath
  ends_with: "uxnode://workbench/tile[node:1]/nav-bar/location-field"

- assert: NodePaneState
  node_key: 1
  lifecycle: Active
  render_mode: CompositedTexture

- assert: SnapshotMatch
  file: "snapshots/after_open.yaml"
  allow_diff_keys: [bounds, state.loading]
```

**Control steps**:

```yaml
- control: Wait
  frames: 10          # Advance N frames before continuing

- control: ClearDiagnostics
  # resets the "since scenario start" diagnostics window to now
```

### 4.4 Expected End State

```yaml
expected_end_state:
  snapshot_match: "snapshots/open_node_flow_end.yaml"
  allow_diff_keys: [bounds]
  require_invariants: [S1, S2, S3]
  require_no_violations: true
```

All fields are optional. If `snapshot_match` is present, the live snapshot at scenario
end is diffed against the stored baseline using the diff policy (§5.2).

---

## 5. Snapshot Baseline Management

### 5.1 Storage

Baseline files are stored in `tests/scenarios/snapshots/`. File naming convention:
`{scenario_id}_{checkpoint_name}.yaml` (slashes in ID replaced with underscores).

Example:
```
tests/scenarios/snapshots/
  flow_open-node_end.yaml
  flow_focus-cycle_toolbar.yaml
  flow_modal-dismiss_dialog-open.yaml
  flow_modal-dismiss_end.yaml
```

### 5.2 Diff Policy

| Field class | Match requirement | On mismatch |
|-------------|------------------|-------------|
| `id` | Exact | Block merge |
| `role` | Exact | Block merge |
| `label` | Exact | Block merge |
| `actions` (set) | Exact set equality | Block merge |
| `state.enabled` | Exact | Block merge |
| `state.hidden` | Exact | Block merge |
| Structural (node added / removed) | Exact | Block merge |
| `bounds` | Allowed to vary (default exempt) | Warn artifact |
| `state.focused` | Scenario-specific (use `allow_diff_keys` to exempt) | Warn artifact |
| `state.loading` | Allowed to vary (default exempt) | Warn artifact |
| `state.selected` | Scenario-specific | Warn artifact |
| `value` | Allowed to vary unless `require_value_match: true` | Warn artifact |

Structural changes (nodes added, removed, role/label/actions changed) require explicit
baseline update and human review. State-only diffs attach a diff artifact to the PR
comment but do not block merge.

### 5.3 Baseline Update Workflow

To update a baseline:
```
cargo test --features test-utils --test scenarios -- --update-snapshots
```

This overwrites existing baselines with the live snapshot output. Updated baseline
files must be reviewed and committed deliberately. CI does not auto-commit baseline
updates.

### 5.4 New Scenario Baselines

When a new scenario is added without a baseline, the first run generates the baseline
file and the scenario is marked `Baseline created (not a test failure)`. CI does not
fail on a new baseline — it requires a human to commit it.

---

## 6. Core Scenario Suite

These scenarios are required for CI. They must pass before any PR touching UI, workbench,
graph, or UX semantics code can merge.

| Scenario file | ID | Coverage |
|---------------|----|----------|
| `open_node_flow.yaml` | `flow:open-node` | GraphNode → Open → NodePane, viewer active |
| `focus_cycle.yaml` | `flow:focus-cycle` | F6 visits all landmark regions in order |
| `modal_dismiss.yaml` | `flow:modal-dismiss` | Dialog open → keyboard dismiss (N2) |
| `blocked_node_recovery.yaml` | `flow:blocked-node-recovery` | Node enters blocked state → recovery action visible (S5) |
| `radial_menu_structural.yaml` | `flow:radial-menu-structural` | Radial menu open → 8 sectors present, all labeled (S1, S8) |
| `command_surface_action_parity.yaml` | `flow:command-surface-action-parity` | Same `ActionId` invoked via keyboard/palette/radial/omnibar yields identical semantic result, target-scope resolution, and disabled-state reason text |
| `omnibar_focus_ownership.yaml` | `flow:omnibar-focus-ownership` | Omnibar/search focus is explicit-only (no default capture), visible without caret dependency, and keyboard commands route to non-omnibar owners until explicit omnibar focus |
| `modal_focus_return_close_restore.yaml` | `flow:modal-focus-return-close-restore` | Modal open/close and explicit restore paths follow deterministic return targets from shared modal-isolation contract table |
| `focus_cycle_deterministic.yaml` | `flow:focus-cycle-deterministic` | Region-cycle (`F6`) order, wrap behavior, and capture exclusions are deterministic and match shared modal-isolation contract table |

Additional scenarios are recommended but not required for CI gate:

| Scenario file | ID | Coverage |
|---------------|----|----------|
| `omnibar_navigation.yaml` | `flow:omnibar-nav` | Omnibar → URL entry → navigation intent |
| `node_pane_degraded.yaml` | `flow:degraded-pane` | TileRenderMode::Placeholder → degraded state visible (M2) |
| `graph_node_labels.yaml` | `flow:graph-node-labels` | All visible GraphNode nodes have labels (S1) |
| `workbar_tab_switch.yaml` | `flow:workbar-switch` | Tab switch → correct pane becomes active |

### 6.1 Command-Surface Parity Assertions (required for `flow:command-surface-action-parity`)

- For a shared action fixture set (same graph/workbench preconditions), each invocation path (`Keyboard`, `SearchPalette`, `ContextPalette`, `RadialPalette`, `Omnibar`) must dispatch the same `ActionId`.
- Target scope must resolve to the same semantic target identity (`NodeKey`, pane identity, graph scope) for equivalent invocation context.
- Disabled actions must expose the same blocked/precondition reason text across all invocation paths.
- Any parity mismatch is a blocking CI failure and must report invocation path, `ActionId`, and divergence payload.

### 6.2 Omnibar Focus Assertions (required for `flow:omnibar-focus-ownership`)

- Initial frame state must prove omnibar/search field is not focused by default.
- Global keyboard command fixtures must execute without being captured by omnibar until explicit omnibar focus selection occurs.
- After explicit omnibar focus selection, text-entry keystrokes are routed to omnibar/search field owner.
- Focus indicator assertions must not depend on caret visibility; scenarios must validate a deterministic focus marker in UxSnapshot state.

### 6.3 Modal Isolation + Focus Return Assertions (required for `flow:modal-focus-return-close-restore` and `flow:focus-cycle-deterministic`)

- Scenario assertions must validate the shared modal-isolation/focus-return contract table mirrored in:
  - `aspect_input/input_interaction_spec.md`
  - `subsystem_focus/focus_and_region_navigation_spec.md`
  - `ux_tree_and_probe_spec.md`
- Close/restore flows must prove captured return target restoration when valid, and deterministic fallback target selection when the original target no longer exists.
- Focus-cycle flows must prove `F6` region traversal order is deterministic, wraps predictably, and does not break modal capture ownership.
- Any mismatch against the contract table is a blocking CI failure and must include transition name, observed capture owner, and observed return target.

---

## 7. CI Integration

### 7.1 Gate Conditions

The UxScenario CI gate runs on PRs that touch:
- `shell/desktop/ui/` — egui frame loop, omnibar, workbar, pane rendering
- `shell/desktop/workbench/` — tile tree, compositor, pane model
- `render/` — radial menu, command palette
- `graph_app.rs` — intent reducer, graph state
- `webdriver.rs` — UxBridge command handlers
- `tests/scenarios/ux/` — scenario definitions themselves
- `tests/scenarios/snapshots/` — baseline files

### 7.2 Gate Actions

1. `cargo test --features test-utils --test scenarios` — run the core suite.
2. On scenario failure: report step index, assertion error, and the diff (if snapshot).
3. On structural diff: fail the gate; attach the diff YAML to the PR check output.
4. On state-only diff: pass the gate; attach a warning artifact.
5. On new baseline creation: pass the gate; prompt for human review and commit.

### 7.2A Required Core UX Scenario Slices

The core suite is not limited to the original open/focus/modal trio. The
minimum named flow set now includes:

| Scenario file | Required purpose |
|---|---|
| `tests/scenarios/ux/open_node_flow.yaml` | Baseline node-open success path |
| `tests/scenarios/ux/focus_cycle_flow.yaml` | Deterministic region focus traversal |
| `tests/scenarios/ux/modal_dismiss_flow.yaml` | Modal isolation and dismissal |
| `tests/scenarios/ux/facet_filter_entry_omnibar.yaml` | Valid facet-filter entry through omnibar with diagnostics success path |
| `tests/scenarios/ux/facet_pane_route_success.yaml` | Single-node facet route success and pane-focus transfer |
| `tests/scenarios/ux/facet_pane_route_blocked_multiselect.yaml` | Blocked facet route emits warning and preserves rail focus |
| `tests/scenarios/ux/facet_pane_focus_return.yaml` | Pane dismiss/back restores captured focus anchor or deterministic fallback |

Any future change to facet filtering, facet-pane routing, blocked-route
semantics, or focus return must extend or update the corresponding scenario
fixture in this set.

### 7.3 Headless Environment Requirements

| Requirement | Implementation |
|-------------|---------------|
| No OS window | `GRAPHSHELL_HEADLESS=1` |
| Fixed window size | `GRAPHSHELL_WINDOW_SIZE=1280x800` |
| Fixed DPR | `GRAPHSHELL_DEVICE_PIXEL_RATIO=1.0` |
| Software renderer | `LIBGL_ALWAYS_SOFTWARE=1` (Linux), WARP (Windows) |
| Fixed physics seed | `SeedRng(0)` at scenario start |
| Fixed clock | `SetClock(0)` at scenario start |
| Normal memory pressure | `GRAPHSHELL_MEMORY_PRESSURE_OVERRIDE=Normal` |
| No network | Test pages served from `file://` or `data:` URLs |

---

## 8. UxDriver Crate Layout

```
tests/
  harness/
    lib.rs
      pub mod driver;
      pub mod bridge_client;
      pub mod snapshot;
      pub mod contracts;
      pub mod scenario_runner;

  harness/driver.rs
    pub struct UxDriver          // Primary API: methods for driving and asserting
    pub struct DriverConfig      // timeout, base_url, log_level
    impl UxDriver {
      pub fn connect(config: DriverConfig) -> Result<Self, BridgeError>
      pub fn snapshot(&mut self) -> Result<UxSnapshot, BridgeError>
      pub fn find_node(&mut self, sel: Selector) -> Result<UxNode, FindError>
      pub fn assert_node_exists(&mut self, sel: Selector) -> Result<UxNode, AssertionError>
      pub fn assert_node_absent(&mut self, sel: Selector) -> Result<(), AssertionError>
      pub fn invoke_action(&mut self, id: &UxNodeId, action: UxAction) -> Result<(), ActionError>
      pub fn assert_snapshot_invariants(&mut self, ids: &[&str]) -> Result<(), Vec<UxContractViolation>>
      pub fn assert_no_ux_violations(&mut self) -> Result<(), Vec<UxViolationEvent>>
      pub fn assert_diagnostics_channel(&mut self, ch: &str, max_severity: ChannelSeverity) -> Result<(), AssertionError>
      pub fn step_physics(&mut self, ticks: u32) -> Result<(), BridgeError>
      pub fn set_clock(&mut self, ms: Option<u64>) -> Result<(), BridgeError>
      pub fn seed_rng(&mut self, seed: u64) -> Result<(), BridgeError>
      pub fn wait_frames(&mut self, n: u32) -> Result<(), BridgeError>
    }

  harness/snapshot.rs
    pub struct UxSnapshot         // Deserialized YAML tree
    pub struct UxDiff             // Structural and state diffs between two snapshots
    pub fn diff(a: &UxSnapshot, b: &UxSnapshot) -> UxDiff
    pub fn load_baseline(path: &Path) -> Result<UxSnapshot, io::Error>
    pub fn save_baseline(path: &Path, snap: &UxSnapshot) -> Result<(), io::Error>
    pub fn assert_matches_baseline(live: &UxSnapshot, baseline: &UxSnapshot, policy: DiffPolicy) -> Result<(), UxDiff>

  harness/contracts.rs
    // Pure functions; take &UxSnapshot, return Option<UxContractViolation>
    pub fn check_s1_labels(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_s2_focus_hidden(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_s3_focus_uniqueness(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_s4_dialog_dismiss(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_s7_id_uniqueness(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_n1_no_cycle(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_n2_modal_escape(tree: &UxSnapshot) -> Vec<UxContractViolation>
    pub fn check_all(tree: &UxSnapshot, ids: &[&str]) -> Vec<UxContractViolation>

  harness/scenario_runner.rs
    pub struct ScenarioRunner     // Parses YAML, drives UxDriver, collects results
    pub struct ScenarioResult     // Pass | Fail { step, error } | Skip { reason }
    impl ScenarioRunner {
      pub fn load(path: &Path) -> Result<Self, ScenarioError>
      pub fn run(&self, driver: &mut UxDriver) -> ScenarioResult
      pub fn run_all(dir: &Path, driver: &mut UxDriver) -> Vec<(String, ScenarioResult)>
    }
```

---

## 9. Relationship to Existing Test Infrastructure

### 9.1 This system does not replace

- **Inline `#[cfg(test)]` unit tests** in `graph_app.rs`, `registries/`, etc.
  These test pure logic without the full app. They remain unchanged.
- **`shell/desktop/tests/` integration tests** (T2 migration target from the
  diagnostics plan). These test app state via `TestRegistry`. They remain valid and
  are migrated incrementally to the `[[test]]` binary.
- **`tests/scenarios/main.rs`** (the existing scenario binary). The UxScenario runner
  is added as a new module within the same binary — not a separate binary.

### 9.2 Integration with T2 (test binary split)

The UxScenario runner is registered as a test module in `tests/scenarios/main.rs`:

```rust
// tests/scenarios/main.rs
mod capability_scenarios;    // existing
mod ux_scenarios;            // new: imports harness::scenario_runner
```

UxScenarios require `feature = "test-utils"`, which implies `ux-bridge` and
`ux-semantics`. They run only when that feature is active:

```
cargo test --features test-utils --test scenarios -- ux_scenarios
```

### 9.3 Relation to DiagnosticsState Assertions

Existing `TestRegistry`-backed tests assert diagnostics state via
`DiagnosticsState::assert_channel_fired()`. UxScenarios assert the same via
`assert: DiagnosticsChannel` steps and `driver.assert_diagnostics_channel()`.
Both call through to the same underlying `GetDiagnosticsState` UxBridge command.
They are complementary — not duplicates.

---

## 10. Degradation Contracts

**Scenario runner failure**
If the UxDriver loses the bridge connection mid-scenario, the scenario fails with
`BridgeError::Disconnected`. The failure is reported with the step index. The runner
proceeds to the next scenario.

**Scenario timeout**
If a `within_frames` budget expires, the step fails with `AssertionError::Timeout`.
The scenario fails. The runner reports the last observed snapshot at timeout.

**Baseline missing**
If `snapshot_match` references a non-existent file, the scenario creates the file and
marks the result `Baseline created`. The runner continues to the next scenario.
This is not a failure — but the file must be committed for future runs to have a gate.

**Headless unavailable**
If the app cannot start in headless mode, the entire scenario suite is marked `Skipped:
headless unavailable`. This is not a test failure on platforms where headless rendering
is not supported. CI is responsible for ensuring the headless environment is available.
