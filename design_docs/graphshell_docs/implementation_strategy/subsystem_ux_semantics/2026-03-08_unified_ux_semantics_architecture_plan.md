# Unified UX Semantics Architecture Plan

**Date**: 2026-03-08  
**Status**: Active / architecture cleanup plan  
**Scope**: Reconcile the canonical UX Semantics subsystem contract with the actual runtime state of UxTree projection, dispatch diagnostics, and still-partial bridge/harness closure.

**Related**:
- `SUBSYSTEM_UX_SEMANTICS.md`
- `ux_tree_and_probe_spec.md`
- `ux_event_dispatch_spec.md`
- `ux_scenario_and_harness_spec.md`
- `2026-03-04_model_boundary_control_matrix.md`

---

## 1. Why This Plan Exists

The UX Semantics subsystem is no longer “not implemented,” but it is also not yet the fully closed platform the docs describe.

Today, the repo has:

- a real UxTree snapshot model
- real snapshot build/publish behavior in the workbench render pipeline
- some UX diagnostics and dispatch contracts implemented
- some Rust scenario coverage around snapshots/diff gates
- a small set of YAML UX scenarios

But it does not yet have:

- the full generic `UxProbeSet` / invariant-engine architecture
- the full `UxBridge` command surface described in the specs
- the YAML-driven UxScenario runner described in the specs
- complete AccessKit consumption from one canonical UxTree path

So the subsystem needs a more honest top-level architecture model: one that distinguishes what is already a runtime system from what is still a planned integration layer.

---

## 2. Canonical UX Semantics Taxonomy

The subsystem should explicitly distinguish five tracks:

1. `UxProjection`
2. `UxContracts`
3. `UxDispatch`
4. `UxBridge`
5. `UxScenarioHarness`

The subsystem as a whole spans all five, but they are not at the same maturity level.

---

## 3. Canonical Tracks

### 3.1 `UxProjection`

Owns:

- UxTree snapshot construction
- semantic/presentation/trace layer modeling
- stable `ux_node_id` projection rules
- snapshot publication for same-frame and test consumers

Current reality:

- this track is real and active in runtime
- `workbench/ux_tree.rs` and tile post-render integration already implement a meaningful subset

### 3.2 `UxContracts`

Owns:

- structural, navigation, and state-machine invariant definitions
- probe registration and evaluation model
- violation event routing and severity classification

Current reality:

- some contract enforcement exists, but not the full generic `UxProbeSet` model described in docs
- the subsystem currently has narrower checks than the full S/N/M architecture implies

### 3.3 `UxDispatch`

Owns:

- semantic event dispatch phase ordering
- modal isolation behavior
- focus/dispatch diagnostics
- authority routing from UX events into graph/workbench intent boundaries

Current reality:

- dispatch-phase logic and diagnostics exist in GUI orchestration
- the dispatch subsystem is partly implemented, but still closely coupled to current workbench/orchestration seams

### 3.4 `UxBridge`

Owns:

- app-side command handling for semantic snapshots and actions
- stable machine-facing control/query surface
- bridge transport semantics

Current reality:

- this track is only partially present
- the full command catalog in the specs is not implemented as a live runtime bridge

### 3.5 `UxScenarioHarness`

Owns:

- YAML scenario definitions
- driver execution semantics
- deterministic test orchestration
- snapshot baseline management and CI policy

Current reality:

- Rust snapshot/diff-gate tests exist
- YAML scenario files exist in limited form
- the generic YAML-driven runner and typed `UxDriver` described in the specs are not yet closed

---

## 4. Current Implementation Snapshot

### 4.1 Landed

- `UxTreeSnapshot` with semantic/presentation/trace layers
- snapshot build and publish in the workbench render path
- diff-gate severity model for semantic vs presentation changes
- a concrete snapshot-consistency check
- UX dispatch diagnostics around focus/navigation/orchestration flows
- Rust scenario-style tests for snapshot health and diff-gate policy

### 4.2 Partial / Inconsistent

- subsystem docs still describe the runtime as if no implementation exists in some sections
- probe/invariant architecture is broader in docs than in code
- YAML scenario inventory exists, but not the fully realized runner/driver stack
- dispatch semantics are real but still tightly coupled to current orchestration code
- AccessKit closure remains partial

### 4.3 Missing

- full generic `UxProbeSet` runtime
- full `UxBridge` command/response implementation
- generic YAML UxScenario runner with typed driver API
- full core scenario suite closure as described in subsystem docs
- complete AccessKit consumption from the same UxTree authority surface

---

## 5. Architectural Corrections

### 5.1 Stop Treating The Subsystem As One Maturity Level

The subsystem guide should distinguish:

- landed runtime projection
- partial dispatch and diagnostics closure
- planned bridge and harness closure

### 5.2 Reframe “Canonical UxTree” Carefully

`UxTree` is the canonical semantic projection surface, but not all intended consumers are fully closed yet.

### 5.3 Separate Projection From Harness

The docs should stop blurring “semantic snapshots exist” with “the full bridge-driven UX testing platform exists.”

### 5.4 Separate Contract Definitions From Contract Engine Implementation

The S/N/M invariant model is useful, but the generic runtime engine for it is not fully implemented.

### 5.5 Align Runtime Docs With Actual Scenario Inventory

The subsystem guide should stop claiming the original core trio is already wired in CI if the repo currently shows a different and narrower scenario set.

---

## 6. Sequencing Plan

### Phase A. Reality Alignment

1. Update subsystem docs to reflect the landed projection layer.
2. Mark bridge/harness closure as partial rather than absent or complete.
3. Normalize the “current status” and roadmap sections against the repo state.

### Phase B. Contract Engine Closure

1. Define the minimal real `UxProbeSet` architecture that matches the docs.
2. Separate implemented checks from planned invariant families.
3. Add explicit tracking for which contracts are live versus specified-only.

### Phase C. Bridge Surface Closure

1. Implement the real `UxBridge` command surface incrementally.
2. Keep transport separate from scenario logic.
3. Expose only the commands that can be supported deterministically.

### Phase D. Scenario/Harness Closure

1. Decide whether the canonical harness is YAML-first, Rust-first, or mixed.
2. If YAML-first remains the target, implement the runner and typed driver.
3. Align the actual scenario inventory with the subsystem guide and CI policy.

### Phase E. AccessKit Closure

1. Feed the accessibility bridge from canonical UxTree outputs where intended.
2. Resolve any remaining split ownership between UxTree projection and accessibility mapping.

---

## 7. Recommended Immediate Actions

1. Update `SUBSYSTEM_UX_SEMANTICS.md` to reference this architecture plan.
2. Replace stale “no code exists yet” statements with a landed/partial/missing split.
3. Add a `Current Runtime Closure` section listing projection, dispatch, bridge, and harness status separately.
4. Open follow-ons for:
   - real `UxBridge` command surface closure
   - explicit probe engine closure
   - scenario runner decision and implementation
   - AccessKit consumer closure

---

## 8. Done Definition

The UX Semantics subsystem architecture is coherent when:

- projection, contracts, dispatch, bridge, and harness are treated as distinct tracks
- the subsystem guide clearly marks which tracks are landed, partial, and missing
- the docs no longer overstate bridge/scenario closure
- the runtime command and test surfaces match the documented architecture
- the UxTree authority story is accurate for all active consumers
