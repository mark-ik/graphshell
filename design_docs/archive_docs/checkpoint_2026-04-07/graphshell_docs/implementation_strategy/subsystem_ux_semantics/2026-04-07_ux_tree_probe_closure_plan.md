# UxTree and UxProbe Closure Plan

**Date**: 2026-04-07
**Status**: Archived 2026-04-07
**Scope**: Close the gap between the canonical `ux_tree_and_probe_spec.md`
contract and the runtime that already exists in `graphshell`.

**Archive note**:

- This closure plan is complete for the pre-WGPU UX semantics target and is retained as the implementation receipt for slices `2`-`7` and `3A`-`3E`.
- Active authority now lives in `ux_tree_and_probe_spec.md`, `SUBSYSTEM_UX_SEMANTICS.md`, and `ux_scenario_and_harness_spec.md`.
- Any additional work in this area should be treated as a new extension plan rather than a continuation of this closure document.

**Related**:

- `ux_tree_and_probe_spec.md`
- `SUBSYSTEM_UX_SEMANTICS.md`
- `2026-03-08_unified_ux_semantics_architecture_plan.md`
- `2026-04-05_command_surface_observability_and_at_plan.md`
- `../graph/graph_node_edge_interaction_spec.md`
- `../subsystem_accessibility/accessibility_interaction_and_capability_spec.md`
- `../subsystem_diagnostics/diagnostics_observability_and_harness_spec.md`
- `../PLANNING_REGISTER.md` item 12

---

## 1. Why This Plan Exists

`ux_tree_and_probe_spec.md` remains the canonical contract, but parts of it now
describe the intended end state rather than the current runtime.

Today the repo already has:

- a real layered `UxTreeSnapshot` with semantic, presentation, and trace layers
- per-frame snapshot build + publish in the workbench render path
- real diagnostics for `ux:tree_build`, `ux:tree_snapshot_built`, and several
  ad hoc invariant failures
- concrete scenario evidence for the pre-WGPU critical-path gates around graph
  navigation, pane lifecycle, command surfaces, modal isolation, and degraded
  viewers

But it still does not have:

- the feature split described by the spec (`ux-semantics`, `ux-probes`,
  `ux-bridge`)
- clear runtime evidence for the Point-LOD `StatusIndicator` parity rule

This plan turns those gaps into a small number of implementation slices instead
of treating the whole UX semantics subsystem as equally unfinished.

---

## 2. Current Repo Reality

### 2.1 Landed Runtime Surfaces

- `shell/desktop/workbench/ux_tree.rs` already owns the layered snapshot model,
  schema versions, semantic role/domain projection, snapshot publication, and a
  small library of pure invariant helpers.
- `shell/desktop/workbench/ux_probes.rs` now owns the minimal real probe runtime:
  registered core descriptors, lifecycle receipts, panic isolation,
  suppression/budget state, and probe-to-channel routing for the currently
  landed invariant wrappers.
- `shell/desktop/workbench/tile_post_render.rs` already builds the snapshot each
  frame behind a recoverable boundary, publishes it, emits `ux:tree_build` /
  `ux:tree_snapshot_built`, optionally writes snapshots when
  `GRAPHSHELL_UX_SNAPSHOT_PATH` is set, drains probe lifecycle receipts,
  evaluates the registered core probes, and feeds layout-policy evaluation.
- `registries/atomic/diagnostics.rs` and
  `shell/desktop/runtime/registries/mod.rs` already declare the probe lifecycle
  channels, and the runtime now emits `ux:probe_registered` for active core
  descriptors.
- `shell/desktop/tests/scenarios/pre_wgpu_critical_path.rs` and
  `shell/desktop/tests/scenarios/ux_tree.rs` already provide real scenario-level
  evidence for the landed portions of the contract.

### 2.2 Still-Ad-Hoc Instead Of Closed

- The core invariant checks are now registered probes, but only for the narrow
  set already present in runtime code.
- The contract-coverage lane is now explicitly bounded as slices `3A`-`3E`
  instead of continuing as open-ended follow-on labels.
- `ux:probe_disabled` now covers runtime panic disablement as well as static
  inactive descriptors, but there are still no statically disabled core
  descriptors in the current runtime configuration.
- Builder isolation currently falls back to a degraded root-only snapshot rather
  than attempting partial-tree salvage.

### 2.3 Stale Spec Areas

The following spec sections are still useful as target contracts, but they are
stale as descriptions of the current runtime:

- **Section 7 feature flag behavior**: the Cargo split is now live, so any prose
  that still describes `test-utils` as the only UX-adjacent flag is stale.
- **Section 9 diagnostics contracts**: probe lifecycle and `ux:snapshot_written`
  channels are now wired inside the live `ux-semantics` / `ux-probes` boundary,
  not as an unconditional desktop path.
- **Section 10 degradation contracts**: builder failure isolation is now wired
  to a degraded root-only fallback, while best-partial salvage remains a future
  refinement rather than current behavior.
- **`GraphNodeGroup`**: now explicitly classified as a deferred post-WGPU
  extension role rather than part of the current runtime closure gate.

---

## 3. Refactor Direction

This plan keeps the spec canonical, but refactors execution around four rules:

1. Do not rewrite the spec to match every temporary runtime shortcut.
   Instead, add narrow implementation-alignment notes where needed.
2. Treat the current ad hoc invariant helpers as the seed of the real
   `UxProbeSet`, not as throwaway code.
3. Do not lead with dormant Cargo features.
   Land the probe runtime and snapshot/export behavior first, then decide
   whether the feature split still buys enough to justify the churn.
4. Keep the layered snapshot model authoritative.
   The open work is probe execution, failure isolation, export/bridge closure,
   and LOD parity, not a redesign of `UxTreeSnapshot`.

---

## 4. Execution Slices

### Slice 1 - Reality Alignment and Spec Hygiene

1. Add implementation-alignment notes to:
   - `ux_tree_and_probe_spec.md`
   - `SUBSYSTEM_UX_SEMANTICS.md`
2. Replace any unconditional wording that implies the generic `UxProbeSet`,
   snapshot export path, or feature split already exists.
3. Keep the normative contract intact, but distinguish:
   - landed runtime behavior
   - ad hoc-but-real checks
   - planned closure items

**Acceptance criteria:**

- No UX semantics doc claims the generic probe engine already exists.
- The feature-flag matrix is described as target-state unless and until those
  features are actually added.
- The canonical spec clearly points at this closure plan for execution order.

### Slice 2 - Minimal Real `UxProbeSet`

**Status**: Implemented 2026-04-07

1. Introduce a dedicated probe runtime module, for example:
   - `UxProbeDescriptor`
   - `UxProbeRegistry`
   - `UxProbeRuntimeState`
   - `UxContractViolation`
2. Promote the existing pure invariant helpers into registered probes rather
   than invoking them ad hoc from `tile_post_render.rs`.
3. Implement startup registration and emit `ux:probe_registered` for every live
   descriptor.
4. Implement disabled-path emission for probes that are compiled but inactive,
   and emit `ux:probe_disabled` with a concrete reason.
5. Route each probe result to the correct channel:
   - structural failures -> `ux:structural_violation`
   - navigation failures -> `ux:navigation_violation`
   - warn-class failures -> `ux:contract_warning`

**Initial probe set should be the probes already implied by landed code:**

- semantic/presentation ID consistency
- semantic/trace ID consistency
- semantic parent-link validity
- interactive label presence
- command-surface capture owner uniqueness
- command-surface return-target presence

**Acceptance criteria:**

- `tile_post_render.rs` calls one probe-runner entry point rather than a list of
  ad hoc invariant helper invocations.
- Probe registration produces lifecycle diagnostics at startup.
- The existing invariant behavior is preserved, but now flows through a probe
  registry.

**Implementation receipt (2026-04-07):**

- Landed in `shell/desktop/workbench/ux_probes.rs`.
- `tile_post_render.rs` now drains lifecycle receipts and evaluates registered
  probes instead of manually invoking each invariant helper.
- Core descriptors currently cover semantic/presentation consistency,
  semantic/trace consistency, semantic parent links, interactive label
  presence, command-surface capture ownership, and command-surface return
  targets.

### Slice 3 - Probe Isolation, Suppression, and Budgeting

**Status**: Implemented 2026-04-07

1. Wrap each probe with `catch_unwind`.
2. On panic:
   - emit `ux:probe_disabled`
   - disable only the failing probe for the rest of the session
   - continue running the remaining probes
3. Add suppression state keyed by `(probe_id, node_path)` to enforce the
   one-event-per-second flood rule.
4. Track probe execution time separately from snapshot build time.
5. Enforce the budget behavior already promised by the spec:
   - log info-level timings under budget
   - warn on breach
   - skip probe execution for that frame if the build path exceeds the hard cap

**Acceptance criteria:**

- A deliberately panicking probe cannot take down the frame loop.
- Duplicate violations are rate-limited as specified.
- Probe runtime timing is visible in diagnostics.

**Implementation receipt (2026-04-07):**

- Each registered probe now runs behind `catch_unwind` in
  `shell/desktop/workbench/ux_probes.rs`.
- Panicking probes emit `ux:contract_warning`, enqueue `ux:probe_disabled`, and
  are disabled for the rest of the session without stopping remaining probes.
- Violations are suppression-keyed by `(probe_id, node_path)` with one emitted
  event per second plus deferred suppressed-count annotation when the window
  lifts.
- `tile_post_render.rs` now emits structured `ux:tree_build` timing summaries
  with build latency, probe latency, total latency, probe counts, skip state,
  and budget status.
- Probe execution is skipped for frames whose build path exceeds the 2 ms hard
  cap.

### Slice 3A - First Coverage Expansion (`S9` Bounds Probe)

**Status**: Implemented 2026-04-07

1. Add the first post-runtime-closure expansion probe rather than jumping
   directly to builder isolation.
2. Implement `S9` as a live runtime probe:
   - every interactive semantic node with present bounds must have
     `width >= 32` and `height >= 32`
   - route failures to `ux:contract_warning`
3. Preserve the pure-helper -> registered-probe pattern established in Slice 2.
4. Add focused coverage for both the helper and the registered-probe path.

**Acceptance criteria:**

- `S9` is no longer only "checkable" in docs; it is active in runtime code.
- Violations identify the offending `ux_node_id` so suppression applies per
  node path.

**Implementation receipt (2026-04-07):**

- Added a pure `interactive_bounds_violation(...)` helper in
  `shell/desktop/workbench/ux_tree.rs`.
- Registered `ux.probe.interactive_bounds_minimum` in
  `shell/desktop/workbench/ux_probes.rs`.
- The probe emits warn-class violations on `ux:contract_warning` when an
  interactive node with present bounds is smaller than `32x32` logical pixels.
- Focused tests cover both the helper and the registered runtime probe path.

### Slice 3B - Snapshot-Only Structural Coverage Expansion

**Status**: Implemented 2026-04-07

This is the first remaining bounded extension of the contract-engine lane.
It should close the purely snapshot-derived contracts that do not require
additional runtime orchestration beyond `UxTreeSnapshot` itself.

Target contract family:

- `S2` focus-hidden exclusion
- `S3` exactly-one-focused-node-or-zero
- `S8` radial-sector child-count bounds
- any remaining semantic-layer consistency checks that are decidable from one
  snapshot without modal traversal or frame-history state

**Acceptance criteria:**

- Snapshot-only structural invariants are either live runtime probes or
  explicitly classified as deferred/non-runtime.
- The contract lane no longer mixes one-off probe additions with unidentified
  future work.

**Implementation receipt (2026-04-07):**

- Added pure helpers in `shell/desktop/workbench/ux_tree.rs` for:
  - `S3` single-focus uniqueness (`semantic_focus_uniqueness_violation(...)`)
  - `S7` semantic ID uniqueness (`semantic_id_uniqueness_violation(...)`)
  - `S8` radial-sector count bounds (`radial_sector_count_violation(...)`)
- Registered the matching live probes in
  `shell/desktop/workbench/ux_probes.rs`:
  - `ux.probe.focus_uniqueness`
  - `ux.probe.semantic_id_uniqueness`
  - `ux.probe.radial_sector_count`
- Added focused helper and probe-runtime tests for those contracts.
- Explicitly classified the remaining snapshot-only structural contracts that
  are not honestly supportable from the current projection shape:
  - `S2` deferred until the semantic layer projects `hidden` state
  - `S4` deferred until `Dialog` / dismiss-button semantic subtrees exist
  - `S5` deferred until blocked recovery actions are projected as semantic
    children
  - `S6` deferred until keyboard shortcut / command-surface action metadata is
    projected into the snapshot

### Slice 3C - Traversal and Modal Coverage Expansion

**Status**: Implemented 2026-04-07

This slice closes the bounded portion of the navigation family that can be
evaluated in runtime without a full YAML scenario harness.

Target contract family:

- modal reachability / dismiss-path checks where runtime metadata is already
  present
- focus-return / command-surface restoration invariants beyond the currently
  landed return-target probe
- any navigation invariants that require ordered tree walks but not synthetic
  input replay

**Acceptance criteria:**

- Runtime-capable navigation/modal checks are explicitly separated from
  scenario-only checks.
- The remaining `N`-series closure surface is bounded and named up front.

**Implementation receipt (2026-04-07):**

- Classified `N5` as live runtime coverage via the existing
  `ux.probe.command_surface_return_target` probe and its fallback-anchor logic.
- Classified `N1`, `N2`, `N3`, and `N4` as scenario-only for the current lane:
  the live semantic model does not yet project tab-order / focus-graph edges or
  modal-dialog subtrees richly enough to support those checks as honest runtime
  probes.
- Froze the runtime navigation endpoint for the pre-WGPU lane to:
  - graph LOD parity mismatch receipts
  - command-surface capture-owner uniqueness (`S10`)
  - command-surface return-target / fallback coverage (`N5`)

### Slice 3D - Stateful Runtime Coverage Expansion

**Status**: Implemented 2026-04-07

This slice closes the bounded portion of the contract engine that depends on
runtime/session state rather than one snapshot.

Target contract family:

- `M`-series checks backed by reducer/runtime state already available in the
  app model
- stateful placeholder / blocked / stale-delivery contracts that can run
  deterministically in-process without the scenario harness

**Acceptance criteria:**

- Runtime-state-dependent contracts are either promoted into live probes or
  explicitly left to the scenario lane.
- The probe engine stops growing by unnamed stateful follow-ons.

**Implementation receipt (2026-04-07):**

- Closed the stateful-runtime lane by explicit classification rather than by
  adding speculative projection fields just to satisfy stale contract text.
- Classified:
  - `M1` live runtime probe via `ux.probe.node_pane_tombstone_lifecycle`
    after projecting `NodeLifecycle` into `UxDomainIdentity::Node` for
    `NodePane` semantic output
  - `M2` live runtime probe via `ux.probe.node_pane_placeholder_timeout`
    after projecting per-node attachment-attempt metadata and tracking
    degraded-frame age inside the probe runtime
  - `M3` scenario-only: defined against clean `UxScenario` execution windows
  - `M4` scenario-only: defined against clean `UxScenario` runs with no fault
    injection
  - `M5` live runtime probe via
    `ux.probe.command_surface_observability_projection` after projecting
    command-surface mailbox/route sequence metadata
- This freezes the pre-WGPU runtime endpoint to a named finite set of future
  extensions only in the scenario-only/deferred lanes instead of unnamed
  follow-ons inside the runtime-capable set.

### Slice 3E - Runtime-vs-Scenario Final Classification

**Status**: Implemented 2026-04-07

This slice is the stop condition for the `3A`-`3E` lane.

1. Build an explicit matrix for all `S1-S10`, `N1-N5`, and `M1-M5` contracts:
   - live runtime probe
   - runtime-capable but not yet landed
   - scenario-only
   - explicitly deferred
2. Align the matrix across:
   - this closure plan
   - `SUBSYSTEM_UX_SEMANTICS.md`
   - `ux_tree_and_probe_spec.md`
3. Freeze the runtime contract-engine endpoint for the pre-WGPU lane.

**Acceptance criteria:**

- There is a finite, reviewable endpoint for the contract-engine lane.
- Future work can extend the subsystem intentionally, but the current closure
  target is no longer ambiguous.

**Implementation receipt (2026-04-07):**

- Added the explicit pre-WGPU contract matrix below and synced the same
  classification into `SUBSYSTEM_UX_SEMANTICS.md` and
  `ux_tree_and_probe_spec.md`.
- The runtime contract-engine endpoint is now frozen to:
  - live runtime probes already present in `ux_probes.rs`
  - graph LOD parity mismatch diagnostics in `tile_post_render.rs`
  - scenario-only traversal/state contracts (`N1-N4`, `M3-M4`)
  - explicitly deferred structural contracts whose required projection fields do
    not yet exist (`S2`, `S4`, `S5`, `S6`)

#### Pre-WGPU Contract Matrix (2026-04-07)

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

### Slice 4 - Builder Failure Isolation and Snapshot Export

**Status**: Implemented 2026-04-07

1. Move from a raw `build_snapshot_with_rects(...)` call to a recoverable build
   boundary, for example `try_build_snapshot_with_rects(...)`.
2. On builder failure:
   - emit `ux:tree_build` with error context
   - return a degraded/root-only snapshot or the best partial snapshot the
     builder can salvage
   - do not panic the app
3. Add optional snapshot export when `GRAPHSHELL_UX_SNAPSHOT_PATH` is set.
4. Emit `ux:snapshot_written` only when a write actually succeeds.

**Acceptance criteria:**

- Builder failure becomes a diagnostics event, not an app crash.
- `ux:snapshot_written` exists as a real runtime event rather than a doc-only
  channel.
- Snapshot export is off by default and deterministic when enabled.

**Implementation receipt (2026-04-07):**

- Added `try_build_snapshot_with_rects(...)` in
  `shell/desktop/workbench/ux_tree.rs` and wrapped the live frame build in a
  recoverable `catch_unwind` boundary.
- Builder failures now degrade to a root-only snapshot via
  `degraded_root_only_snapshot(...)` instead of panicking the frame loop.
- Added opt-in snapshot export via `GRAPHSHELL_UX_SNAPSHOT_PATH` and real
  `ux:snapshot_written` runtime emission on successful writes only.
- `ux:tree_build` structured payloads now include build-degraded and
  snapshot-export state/error fields.
- Added focused tests for forced builder failure, degraded snapshot fallback,
  and deterministic snapshot writing.

### Slice 5 - Graph LOD Parity Closure

**Status**: Implemented 2026-04-07

1. Audit current `GraphView` / `GraphNode` semantic emission against
   `graph_node_edge_interaction_spec.md`.
2. Implement the Point-tier rule explicitly:
   - omit `GraphNode` semantic children
   - emit the `StatusIndicator` child labeled
     `Zoom in to interact with nodes.`
3. Add tests for Point vs Compact/Expanded semantic output parity.
4. Emit a diagnostic when active canvas LOD and semantic emission mode diverge.

**Acceptance criteria:**

- AC8 is backed by runtime code and tests, not only by spec text.
- The Point-tier status-indicator path appears in semantic snapshots.

**Implementation receipt (2026-04-07):**

- `shell/desktop/workbench/ux_tree.rs` now derives a graph semantic LOD tier
  from live graph-view zoom state, emits a `StatusIndicator` child labeled
  `Zoom in to interact with nodes.` at Point LOD, and suppresses `GraphNode`
  semantic children for that tier.
- Compact and Expanded LOD continue to emit `GraphNode` semantic children.
- `shell/desktop/workbench/tile_post_render.rs` now emits a structured
  `ux:navigation_violation` receipt if semantic emission diverges from the
  active graph-view LOD tier.
- Focused tests now cover Point-tier status-indicator projection,
  Compact-tier graph-node projection, and mismatch detection.

### Slice 6 - `GraphNodeGroup` and Extension-Role Triage

**Status**: Implemented 2026-04-07

1. Decide whether `GraphNodeGroup` is:
   - a near-term runtime deliverable, or
   - a post-WGPU extension role that should be explicitly deferred in the spec
2. If not implemented immediately, reduce ambiguity by marking it as deferred
   rather than leaving it implied as already-available runtime output.

**Acceptance criteria:**

- There is no ambiguity about whether `GraphNodeGroup` is required for the
  pre-WGPU closure gate.

**Implementation receipt (2026-04-07):**

- `GraphNodeGroup` is now explicitly classified as a post-WGPU extension role,
  not a pre-WGPU closure requirement.
- The canonical UX semantics docs now state that current runtime projection is
  limited to `GraphView` / `GraphNode` graph-domain output, while grouping /
  graphlet-aware projection remains a future extension once graph-backed group
  identity and collapse semantics are settled.
- The role catalog and action catalog no longer imply that live runtime output
  already contains `GraphNodeGroup` nodes or collapse affordances.

### Slice 7 - Feature-Gate and Bridge/Harness Alignment

**Status**: Completed

- Direction B was selected and landed in the follow-up that closed the
  pre-WGPU runtime-capable lane.

- Closure receipt (2026-04-07 follow-up):
  - `Cargo.toml` now defines `ux-semantics`, `ux-probes`, and `ux-bridge`.
  - `ux-probes` and `ux-bridge` both depend on `ux-semantics`.
  - `test-utils` now depends on `ux-probes` and `ux-bridge`.
  - The default desktop feature set enables `ux-probes` and `ux-bridge`,
    preserving the existing default runtime surface while making the split real.
  - Workbench post-render now clears/skips snapshot publication when
    `ux-semantics` is inactive.
  - Probe lifecycle/event evaluation now no-op when `ux-probes` is inactive.
  - `shell/desktop/workbench/ux_bridge.rs` now provides real in-process bridge
    handlers for `GetUxSnapshot`, `FindUxNode`, `GetFocusPath`, and a narrow
    `InvokeUxAction` slice covering command-surface open/dismiss flows plus
    pane-backed node focus/dismiss, tool-pane focus/close, and graph-surface
    focus/close.
  - `shell/desktop/host/webdriver_runtime.rs` now recognizes a reserved
    `graphshell:ux-bridge:` execute-script payload so WebDriver can query the
    latest snapshot directly and queue the same action slice via
    `WorkbenchIntent` graph events.
  - `shell/desktop/workbench/ux_bridge.rs` now also carries a small Rust-side
    `UxDriver` helper that emits the reserved WebDriver bridge payloads for the
    landed command set, and `shell/desktop/tests/harness.rs` now consumes that
    helper for Rust-first scenario utilities.
  - The Rust-first scenario coverage can exercise the bridge directly today,
    while YAML fixtures and broader transport-backed command routing remain
    future work.

- Pre-landing audit receipt (2026-04-07): Direction A was the only
  repo-supported state at the time of review because:
  - `Cargo.toml` only defined `test-utils`; no `ux-semantics`, `ux-probes`, or
    `ux-bridge` flags existed.
  - Desktop UxTree/UxProbe runtime wiring compiled unconditionally.
  - No real `UxBridge` command handlers or typed `UxDriver` implementation were
    present in Rust code.
  - `tests/scenarios/main.rs` is a small `test-utils` smoke/capability binary,
    while the actual pre-WGPU UX coverage lives in Rust test modules under
    `shell/desktop/tests/scenarios/` plus JSON baselines in
    `tests/scenarios/snapshots/`.
  - YAML fixtures under `tests/scenarios/ux/` are committed inputs for future
    closure, not an active runner-backed CI surface.

- `SUBSYSTEM_UX_SEMANTICS.md`, `ux_tree_and_probe_spec.md`, and
  `ux_scenario_and_harness_spec.md` now need to describe that landed runtime
  shape precisely: real Cargo wiring, live semantics/probe gates, the current
  command-surface plus pane-backed action slice, and the reserved WebDriver
  execute-script bridge envelope plus helper and Rust-first harness usage
  rather than a generic YAML/action harness.

**Acceptance criteria:**

- The docs, Cargo features, and scenario guidance all describe the same runtime.
- The bridge/harness sections no longer promise commands or gates that are not
  actually present.
- Direction B is implemented end-to-end and supersedes the temporary Direction A
  alignment state.

---

## 5. Recommended Commit Batches

To keep the history reviewable, land the work in these batches:

1. `docs: align ux tree spec with runtime`
2. `feat: add ux probe registry and lifecycle diagnostics`
3. `feat: isolate ux probe failures and rate-limit violations`
4. `feat: harden ux tree build and snapshot export`
5. `feat: enforce graph lod parity in ux snapshots`
6. `docs: align ux feature gating and bridge closure`

---

## 6. Done-Gate

This plan is complete when all of the following are true:

- `UxTreeSnapshot` remains the canonical layered snapshot surface.
- The current invariant helpers have been subsumed by a real `UxProbeSet`.
- `ux:probe_registered` and `ux:probe_disabled` are emitted by runtime code.
- Probe panic isolation and suppression semantics are implemented.
- The `3A`-`3E` contract-coverage lane is explicitly classified and bounded.
- Builder failure isolation is enforced.
- `ux:snapshot_written` is either implemented or explicitly removed from the
  spec as a non-goal.
- AC8 LOD parity is backed by runtime tests.
- The feature matrix in docs matches the actual Cargo/runtime shape.

At that point, the plan is complete for the pre-WGPU closure target, and any
remaining work is an intentional future extension rather than ambiguity about
what the UxTree and UxProbe runtime actually is.
