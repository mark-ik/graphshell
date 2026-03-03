# Pre-WGPU Feature + Validation Gate Checklist

**Date**: 2026-03-03  
**Status**: Execution checklist (feature/gate based only)  
**Purpose**: Define the closure program that must pass before starting `egui_glow -> egui_wgpu`, consistent with the deferral and readiness policy.

**Canonical alignment**:

- `aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`
- `subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `subsystem_ux_semantics/2026-02-28_ux_contract_register.md`
- `viewer/2026-02-26_composited_viewer_pass_contract.md`
- `2026-03-02_scaffold_registry.md`
- `PLANNING_REGISTER.md` (`§1C` lanes and done gates)

---

## 1) Scope Rule (Non-negotiable)

This checklist is **not** time-based.  
Progress is measured only by **feature closure + validation gate evidence**.

A gate is `closed` only when:

1. feature semantics are implemented end-to-end,
2. diagnostics evidence exists for the critical contract,
3. scenario coverage exists for the critical path,
4. spec/code parity artifacts are updated.

If any one item is missing, gate remains `open`.

---

## 2) Pre-WGPU Critical Gates

## Gate G0 — UX Contract Baseline Integrity

Feature objective:

- Core UX baseline from the UX control-plane is coherent and executable (interaction correctness, viewer correctness, lifecycle/routing correctness, degradation clarity, spec/code parity).

Primary issue domains:

- `#292`-`#301` (UX integration deliverables and parity)
- `#302` parity baseline maintenance

Validation gate:

- Control-plane and matrix artifacts agree on status:

  - `subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
  - `2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
  - `2026-03-01_ux_migration_lifecycle_audit_register.md`

- No contradictory claim between docs and runtime behavior for active milestone slices.

**Status**: `open`

---

## Gate G1 — Graph Camera + Interaction Reliability

Feature objective:

- Graph camera/navigation/selection interactions are deterministic under normal and focus-churn paths.

Primary issue domains:

- `#173`, `#104`, `#101`, `#103`, `#271`, `#185`, `#102`

Validation gate:

- Critical interaction scenarios pass in harness:

  - camera pan/zoom/reset/fit
  - lasso + selection semantics under modifier combinations
  - keyboard and pointer interaction coexistence

- Diagnostics channels show no authority/routing ambiguity for camera dispatch in critical journeys.
- User-visible regressions in `lane:stabilization` bug register are closed with scenario evidence.

**Status**: `partial`

---

## Gate G2 — Frame / Pane Lifecycle Determinism

Feature objective:

- Open/close/split/focus handoff and first-render activation are deterministic for Workbench/Frame/Tile flows.

Primary issue domains:

- `#174`, `#186`, `#187` (primary)
- `#118`, `#119` (enabling decomposition)

Validation gate:

- Scenario coverage for:

  - spawn/activate first render
  - close-pane successor focus handoff
  - deterministic focus owner transitions across graph pane and node/tool panes

- No blank viewport/focus-race repros in active stabilization register.
- Coordinator-boundary invariants remain green for touched files.

**Status**: `partial`

---

## Gate G3 — Content Opening Semantics (Graphshell-owned)

Feature objective:

- Content-opening actions always route through Graphshell semantic ownership (no legacy bypass).

Primary issue domains:

- `#175`

Validation gate:

- All open-in-new-view paths create/route through canonical Graphshell node/tile semantics.
- No legacy context-menu bypass path can create pane state outside declared intent flow.
- Routing diagnostics include explicit reason/path for open decision.

**Status**: `open`

---

## Gate G4 — Command Surface Unification

Feature objective:

- F2/global command surface, contextual command surface, and radial/menu semantics share one command model.

Primary issue domains:

- `#176`, `#106`, `#107`, `#108`, `#178`, `#270`

Validation gate:

- Command naming/scope reflects canonical semantics (no misleading surface labels).
- Command invocation parity exists across keyboard/pointer entry points.
- Disabled-state policy and context category mapping are explicit and test-covered.

**Status**: `open`

---

## Gate G5 — Settings + Control Surfaces Are First-Class

Feature objective:

- Settings/history/control surfaces behave as first-class panes with stable apply and return-path behavior.

Primary issue domains:

- `#109`, `#110`, `#177`, `#189`

Validation gate:

- Settings changes with persistence semantics have scenario tests (including restart persistence where applicable).
- Tool pane entry/exit return-target behavior is deterministic.
- Control-surface state changes emit diagnostics or observable state transitions.
- Internal control-surface routing uses the canonical `verso://` namespace (with legacy `graphshell://` compatibility only as an alias), and workbench authority does not depend on hand-built address strings.

**Status**: `partial`

---

## Gate G6 — Viewer Presentation + Fallback Clarity

Feature objective:

- Viewer render mode behavior and fallback/degraded states are explicit, deterministic, and diagnostics-visible.

Primary issue domains:

- `#162`, `#188`, `#111`, `#112`
- related lanes: `lane:stabilization`, `lane:viewer-platform`, `lane:spec-code-parity`

Validation gate:

- Surface Composition Contract behavior is provable in runtime evidence:

  - pass ordering for composited mode
  - documented native-overlay affordance limitations
  - fallback reason visibility

- `TileRenderMode`-driven behavior is test-covered where applicable.
- Viewer/fallback docs match runtime reality.

**Status**: `partial`

---

## Gate G7 — Diagnostics + UX Semantics Automation Authority

Feature objective:

- Reliability is enforced through scenario + diagnostics + semantic probes, not ad hoc manual repro loops.

### Primary issue domains

- `#94`, `#251`, `#257`, `#261`, `#269`, `#272`, `#273`

### Validation gate

- Critical-path UxHarness scenarios exist and run as a required gate for affected areas.
- UxTree/Probe invariants catch ownership/focus/routing regressions before merge.
- Diagnostics evidence is present for all core authority boundaries.
- Address-routing coverage includes both system/workbench authority (`verso://...`) and domain-record handoff (`notes://...`, plus any active `graph://...` / `node://...` paths), so content identity and workbench placement failures are caught before merge.

**Status**: `open`

---

## Gate G8 — Active Scaffold Retirement (No silent partials)

### Feature objective

- No pre-wgpu-critical feature remains in scaffold state for core UX closure paths.

Primary scaffold markers:

- `[SCAFFOLD:view-dimension-ui-wiring]`
- `[SCAFFOLD:divergent-layout-commit]`
- `[SCAFFOLD:viewer-wry-runtime-registration]`
- `[SCAFFOLD:verse-protocol-handler]`
- `[SCAFFOLD:wasm-mod-loader-runtime]`

Validation gate:

- For each marker impacting core UX closure lanes: closure criteria met and marker removed or explicitly de-scoped from pre-wgpu closure.
- No “partial but merged” slice in core path without explicit scaffold marker and closure gate.

**Status**: `open`

---

## Gate G9 — WGPU Start Authorization Gate

Feature objective:

- Only begin renderer backend migration once application readiness is true by feature evidence.

Primary source:

- `aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`

Validation gate:

- Application readiness conditions are all true:

  1. authority boundaries explicit/enforced,
  2. core UX flows usable without constant regressions,
  3. app semantics complete enough for practical use,
  4. current-stack bugs are not dominating development,
  5. migration can be evaluated as upgrade, not desperation.

- Runtime viewer bridge precondition (`#180`) is evidenced as solved.

**Status**: `blocked` (by G0-G8 + `#180`)

---

## 3) Execution Policy (Critical)

1. No net-new UX feature expansion while any of G1, G2, G3, or G6 is open.
2. Any PR touching a gate-owned area must include:
   - contract-level scenario evidence,
   - diagnostics evidence,
   - parity doc delta where behavior changed.
3. If a regression reopens a closed gate, that gate reverts to `open` immediately.
4. `G9` cannot be manually overridden by schedule pressure.

---

## 4) What This Implies (Critical Read)

- The next move is not “more planning.” It is strict gate closure on partially implemented pre-wgpu features.
- The dominant risk is not missing ideas; it is accepting partially closed interaction contracts.
- Success criterion is not velocity of merged slices; it is number of gates moved from `open/partial` to `closed` with evidence.

---

## 5) Immediate Next Gate Sequence (Feature-order, not time-order)

1. Close `G3` (content opening semantics) and `G2` (lifecycle determinism) together where routing/lifecycle overlap.
2. Close `G1` residuals (camera/selection determinism under churn).
3. Close `G6` parity/evidence for render-mode fallback and affordance behavior.
4. Close `G7` automation authority so reopened regressions are caught automatically.
5. Re-evaluate `G9` only after G0-G8 status is evidence-complete.
