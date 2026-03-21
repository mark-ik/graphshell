# Unified UX Semantics Architecture Plan

**Date**: 2026-03-08
**Last updated**: 2026-03-21
**Status**: Active / architecture cleanup plan
**Scope**: Reconcile the canonical UX Semantics subsystem contract with the actual runtime state of UxTree projection, dispatch diagnostics, and still-partial bridge/harness closure. Also catalogs high-leverage extensions that the live UxTree projection unlocks.

**Related**:
- `SUBSYSTEM_UX_SEMANTICS.md`
- `ux_tree_and_probe_spec.md`
- `ux_event_dispatch_spec.md`
- `ux_scenario_and_harness_spec.md`
- `2026-03-04_model_boundary_control_matrix.md`
- `workbench_layout_policy_spec.md` — semantic layout policy built on UxTree projection

---

## 1. Why This Plan Exists

The UX Semantics subsystem is no longer "not implemented," but it is also not yet the fully closed platform the docs describe.

Today, the repo has:

- a real UxTree snapshot model
- real snapshot build/publish behavior in the workbench render pipeline
- some UX diagnostics and dispatch contracts implemented
- some Rust scenario coverage around snapshots/diff gates
- a small set of YAML UX scenarios
- Wave 1–4 diagnostic channels covering bounds coverage, gutter/overlap detection, paint confirmation, and native overlay rect drift

But it does not yet have:

- the full generic `UxProbeSet` / invariant-engine architecture
- the full `UxBridge` command surface described in the specs
- the YAML-driven UxScenario runner described in the specs
- complete AccessKit consumption from one canonical UxTree path
- feature flags (`ux-semantics`, `ux-probes`, `ux-bridge`) — everything compiles unconditionally
- per-frame S/N/M invariant probe evaluation

The biggest gap: the subsystem has a well-built projection layer (the tree exists and is published) but no per-frame contract verification engine consuming it. The Wave 1–4 diagnostic channels are adjacent to this — they detect layout/paint problems — but the S/N/M invariant probes (e.g. "every button has a label", "exactly one focused node") are unimplemented at runtime.

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
- `UxPresentationNode.bounds` population from tile rects (landed Wave 1)

Current reality:

- this track is real and active in runtime
- `workbench/ux_tree.rs` and tile post-render integration already implement a meaningful subset
- bounds are now populated for all NodePane-role nodes, enabling bounds-dependent probes

### 3.2 `UxContracts`

Owns:

- structural, navigation, and state-machine invariant definitions (S1–S9, N1–N4, M1–M4)
- probe registration and evaluation model
- violation event routing and severity classification

Current reality:

- S/N/M invariant families are specified in docs but no generic `UxProbeSet` engine exists
- checks are ad-hoc in tests rather than per-frame runtime probes
- the subsystem currently has narrower checks than the full S/N/M architecture implies
- S9 (bounds ≥ 32×32px for interactive nodes) is now checkable without additional instrumentation — bounds are on the snapshot

### 3.3 `UxDispatch`

Owns:

- semantic event dispatch phase ordering
- modal isolation behavior
- focus/dispatch diagnostics
- authority routing from UX events into graph/workbench intent boundaries

Current reality:

- dispatch-phase logic and diagnostics exist in GUI orchestration
- some coverage: toolpane intents, focus orchestration
- not yet general-purpose — still tightly coupled to current workbench/orchestration seams

### 3.4 `UxBridge`

Owns:

- app-side command handling for semantic snapshots and actions
- stable machine-facing control/query surface (`GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`, etc.)
- bridge transport semantics

Current reality:

- this track is only partially present
- the full command catalog in the specs is not implemented as a live runtime bridge
- ux-snapshot YAML export to disk is not implemented

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
- the snapshot diff-gate exists but is not wired to CI

---

## 4. Current Implementation Snapshot

### 4.1 Landed

- `UxTreeSnapshot` with semantic/presentation/trace layers
- snapshot build and publish in the workbench render path
- diff-gate severity model for semantic vs presentation changes
- a concrete snapshot-consistency check
- UX dispatch diagnostics around focus/navigation/orchestration flows
- Rust scenario-style tests for snapshot health and diff-gate policy
- `UxPresentationNode.bounds` populated from `active_node_pane_rects` (Wave 1)
- `run_coverage_analysis` — gutter/overlap detection for tile rects (Wave 2)
- `TileAffordanceAnnotation.paint_callback_registered` — per-tile paint confirmation (Wave 3)
- Native overlay rect drift tracking via `LAST_SENT_NATIVE_OVERLAY_RECTS` (Wave 4)
- Five Wave 1–4 diagnostic channels registered in `PHASE3_CHANNELS`

### 4.2 Partial / Inconsistent

- subsystem docs still describe the runtime as if no implementation exists in some sections
- probe/invariant architecture is broader in docs than in code
- YAML scenario inventory exists, but not the fully realized runner/driver stack
- dispatch semantics are real but still tightly coupled to current orchestration code
- AccessKit closure remains partial — wired for tile nodes, not yet the full canonical path
- `UxDispatch` has some coverage but is not general-purpose

### 4.3 Missing

- full generic `UxProbeSet` runtime (S1–S9, N1–N4, M1–M4 not evaluated per-frame)
- full `UxBridge` command/response implementation
- generic YAML UxScenario runner with typed driver API
- full core scenario suite closure as described in subsystem docs
- complete AccessKit consumption from the same UxTree authority surface
- feature flags `ux-semantics`, `ux-probes`, `ux-bridge` — everything compiles unconditionally
- ux-snapshot YAML export to disk

---

## 5. Architectural Corrections

### 5.1 Stop Treating The Subsystem As One Maturity Level

The subsystem guide should distinguish:

- landed runtime projection (real, per-frame)
- partial dispatch and diagnostics closure (real but narrow)
- planned bridge and harness closure (specced, not implemented)

### 5.2 Reframe "Canonical UxTree" Carefully

`UxTree` is the canonical semantic projection surface, but not all intended consumers are fully closed yet.

### 5.3 Separate Projection From Harness

The docs should stop blurring "semantic snapshots exist" with "the full bridge-driven UX testing platform exists."

### 5.4 Separate Contract Definitions From Contract Engine Implementation

The S/N/M invariant model is useful, but the generic runtime engine for it is not fully implemented.

### 5.5 Align Runtime Docs With Actual Scenario Inventory

The subsystem guide should stop claiming the original core trio is already wired in CI if the repo currently shows a different and narrower scenario set.

---

## 6. Sequencing Plan

### Phase A. Reality Alignment

1. Update subsystem docs to reflect the landed projection layer including Wave 1–4 channels.
2. Mark bridge/harness closure as partial rather than absent or complete.
3. Normalize the "current status" and roadmap sections against the repo state.

### Phase B. Contract Engine Closure

1. Define the minimal real `UxProbeSet` architecture that matches the docs.
2. Separate implemented checks from planned invariant families.
3. Add explicit tracking for which contracts are live versus specified-only.
4. Implement S9 bounds probe as the first live per-frame probe — unblocked by Wave 1.

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

Highest-leverage next steps — all buildable from existing infrastructure, all producing visible developer-facing value:

1. **Live UxTree viewer in diagnostics pane** (§9.1) — uses the already-published snapshot, no new instrumentation needed.
2. **S9 bounds probe** (§9.2) — unblocked by Wave 1 bounds landing; trivial pure function over the snapshot.
3. **Radial collision probe** (§9.4) — data already in `UxDomainIdentity::RadialSummary`; just needs a probe wire.

Beyond those:

1. Update `SUBSYSTEM_UX_SEMANTICS.md` to reference this architecture plan.
2. Replace stale "no code exists yet" statements with a landed/partial/missing split.
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
- at least one per-frame S/N/M probe is live and firing in the diagnostics bus

---

## 9. High-Leverage Extensions

The following ideas are enabled by the live, per-frame UxTree projection — things that almost nothing else does. These are ordered by leverage, not by difficulty. The engineering closure work (§6) is necessary but not differentiated; these are.

### 9.1 Live UxTree Viewer in the Diagnostics Pane

The diagnostics pane currently shows compositor frames and channel counts. The UxTree is published every frame via `publish_snapshot()`. An **Analysis tab** could render the UxTree live — a collapsible tree showing the semantic structure of the current workbench state.

- Click a node in the UxTree viewer → highlight the corresponding tile in the workbench.
- This is a spatial browser debugging its own spatial structure in real time.
- No analogues in other tooling.

### 9.2 S9 Bounds-Checking Probe — Now Feasible

Wave 1 landed `bounds` on `UxPresentationNode`. S9 ("every interactive node must have bounds ≥ 32×32px") is now checkable without any additional instrumentation. The probe is a trivial pure function over the snapshot.

With the `UxProbeSet` engine closed, S9 would fire in the diagnostics bus immediately on tiny tiles — catching a real class of touch/click-target bugs automatically.

### 9.3 UxTree Diff as Persistence-Aware Regression Signal

The snapshot diff-gate exists but isn't wired to CI. More interestingly: record a UxTree baseline for each saved Frame/layout configuration. When the user reopens a saved Frame, diff the rebuilt UxTree against the baseline.

- Structural divergence (role changed, node disappeared) = warn the user that a pane is in an unexpected state.
- This is persistence-aware UX regression detection at the user level — nobody does this.

### 9.4 Radial Menu Geometry Probe

`UxDomainIdentity::RadialSummary` already tracks `label_pre_collisions`, `label_post_collisions`, `overflow_hidden_entries`, `fallback_to_palette`, `fallback_reason`. A probe could emit `ux:radial_label_collision` when `label_post_collisions > 0` — detecting that the menu is silently hiding label text on the user.

This data is already in the tree, just not wired to a probe.

### 9.5 Focus Path Audit as a First-Class UxTree Query

S3 ("exactly one node has focused = true") and N1 (no focus cycles) could be evaluated as a pure walk over the semantic tree every frame. The interesting extension is surfacing the focus path — "you are here in the focus graph" — as a breadcrumb visible to keyboard-only users.

The UxTree already has the domain identity to express `NodePane[key-42] > NavigationBar > BackButton`. Expose that as a live HUD element driven entirely by UxTree projection.

### 9.6 UxTree → Natural Language Narration

The semantic tree has roles, labels, states, and domain identities. A small pass over the current-frame snapshot could generate: "Workbench has 3 panes: two node tiles (example.com, wikipedia.org) and one settings panel. Graph view shows 12 nodes, 3 selected."

This isn't screen-reader output — it's a natural-language status bar or tooltip that describes the current layout for users who find spatial interfaces disorienting. Nothing ships this for browser-like tools.

### 9.7 Invariant Violation History as a Debugging Timeline

When an S/N/M probe fires, the violation event includes `node_path` and `contract_id`. Store the last N violations in a ring, render them in the diagnostics pane with a timeline:

> "at frame 2847, S9 fired on `uxnode://workbench/tile[node:42]/nav-bar/back-button` (bounds 24×18px)"

This turns `UxContracts` from a pass/fail gate into a debugging timeline. Lets you see exactly which layout transition caused a contract breach.

### 9.8 Graphlet-Aware UxTree Projection

`UxDomainIdentity` has `GraphViewLensScope` with `lens_name` and `filter_count`. But graphlets (node groups) aren't yet projected into the UxTree as structural nodes — they're just edges in the graph model.

Projecting `GraphNodeGroup` nodes into the UxTree would:

- enable grouping-aware accessibility (a graphlet becomes a labelled landmark region)
- let invariant probes verify things like "every graphlet has a visible collapse affordance"

### 9.9 Semantic Layout Policy Driven by UxTree

The `WorkbenchLayoutPolicyEvaluator` (see `workbench_layout_policy_spec.md`) is a pure function `(UxTreeSnapshot, WorkbenchProfile) → Vec<WorkbenchIntent>`. It reads `UxDomainIdentity` to match surface roles to live nodes and reads `UxPresentationNode.bounds` for drift detection.

This is a concrete example of the UxTree being consumed as a first-class layout authority — not just a diagnostic output.
