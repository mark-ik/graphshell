# Pre-WGPU Plot — Graphshell v0.0.2 Release Plan

**Date**: 2026-03-03  
**Status**: Active / Canonical release plan  
**Purpose**: Define the overarching v0.0.2 release — the first feature-complete pre-wgpu milestone where the application is usable.

**Versioning policy reference**: `system/VERSIONING_POLICY.md` — v0.0.2 is a **minor bump** representing "significant architecture milestone + notable user-facing capability expansion."

---

## 1) What v0.0.2 Means

v0.0.2 is the **application readiness gate**: the point at which Graphshell is a usable spatial browser on the current rendering stack (egui_glow / Servo GL compositor). It is the precondition for starting the egui_glow → egui_wgpu renderer migration.

**v0.0.2 ships when**:

1. All Application Gates (AG0–AG9) are closed with linked evidence.
2. All P1–P2 spec conflict resolutions are landed.
3. Per-view selection is implemented (per-`GraphViewId`, not per-`GraphBrowserApp`).
4. Undo stack exists for destructive graph mutations.
5. WCAG 2.2 Level A is passed across all 7 surface classes.
6. Active scaffolds impacting core UX closure are retired or explicitly de-scoped.
7. Test guide §4 minimum acceptance checks are green.

**v0.0.2 does NOT include**:

- egui_glow → egui_wgpu renderer migration (that is the post-v0.0.2 gate sequence).
- WebRender wgpu backend (Track B, phases P0–P12).
- Speculative features (DOI/fisheye, SketchLay, advanced gesture modes).
- Verse intelligence subsystem (incubation lane).

---

## 2) Gate Naming Convention

This plan introduces a **disambiguated gate namespace** to prevent confusion between application-readiness gates and renderer-switch gates:

| Prefix | Scope | Source document |
| --- | --- | --- |
| **AG0–AG9** | Application readiness gates (pre-wgpu feature closure) | This document; derived from `2026-03-03_pre_wgpu_feature_validation_gate_checklist.md` |
| **G1–G5** | Renderer switch authorization gates (Glow → wgpu) | `aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md` |

**Sequencing**: AG0–AG9 must all close before G1–G5 evaluation begins. G1–G5 are post-v0.0.2.

The existing `2026-03-03_pre_wgpu_feature_validation_gate_checklist.md` continues to use G0–G9 notation internally. When cross-referencing between documents, prepend `AG` to disambiguate. A future doc-maintenance pass should update the checklist itself.

---

## 3) Contradiction Resolution Decisions (2026-03-03)

These decisions were made during the doc audit that produced this plan. They are binding for v0.0.2 scope.

### CR1 — Undo capability is required for v0.0.2

- `command_semantics_matrix.md` marks most actions as "Undoable: Yes/Soft."
- `ux_integration_research.md` §7.2 states undo is NOT implemented (GraphIntent applied destructively, no undo stack).
- **Decision**: Undo stack for destructive `GraphIntent` mutations is in v0.0.2 scope. The command semantics matrix describes target semantics that must be implemented, not just documented.
- **Impact**: New work item under AG4 (Command Surface Unification). Requires undo/redo stack architecture, `GraphIntent` reversal model, and UI surface (Ctrl+Z / Ctrl+Shift+Z or equivalent).

### CR2 — Application gates are renamed AG0–AG9

- `pre_wgpu_feature_validation_gate_checklist.md` uses G0–G9 (application readiness).
- `webrender_readiness_gate_feature_guardrails.md` uses G1–G5 (renderer switch).
- **Decision**: Application gates become AG0–AG9. Renderer-switch gates keep G1–G5.
- **Impact**: Cross-references in this document use AG prefix. Source checklist retains G-prefix internally until a maintenance pass.

### CR3 — Per-view selection is required for v0.0.2

- `focus_and_region_navigation_spec.md` §4.7 defines per-`GraphViewId` selection truth (target architecture).
- `ux_integration_research.md` §5.3 gap G-PS-1 states selection is currently per-`GraphBrowserApp` (global).
- **Decision**: Per-view selection migration is a v0.0.2 requirement. This closes gap G-PS-1.
- **Impact**: New work item under AG1 (Graph Camera + Interaction Reliability). Selection state must be scoped to `GraphViewId`, not `GraphBrowserApp`.

### CR4 — All surfaces must pass WCAG 2.2 Level A

- `accessibility_baseline_checklist.md` shows most criteria as "Untested."
- `accessibility_closure_bundle_audit_301.md` records a concrete fail (radial disabled contrast 3.21:1 vs 4.5:1 required for AA text).
- **Decision**: All 7 surface classes must pass WCAG 2.2 Level A before v0.0.2. AA criteria are tracked but not gating. Known Level A failures (radial contrast per audit #301) must be fixed.
- **Impact**: Expands AG0 scope. Accessibility baseline checklist must be updated to reflect audit results (currently shows "Untested" for rows with known test evidence). Level A criteria across non-Graph surfaces must be tested and passed.

---

## 4) Application Gates (AG0–AG9)

Each gate corresponds to a gate in the `pre_wgpu_feature_validation_gate_checklist.md` with the AG prefix applied and v0.0.2-specific amendments from §3 incorporated.

A gate is `closed` only when: (1) feature semantics implemented end-to-end, (2) diagnostics evidence for the critical contract, (3) scenario coverage for the critical path, (4) spec/code parity artifacts updated.

### AG0 — UX Contract Baseline Integrity

**Status**: `open`

Feature objective: Core UX baseline from UX control-plane is coherent and executable.

Primary issues: `#292`–`#301`, `#302`.

**v0.0.2 amendments**:

- WCAG 2.2 Level A must pass across all 7 surface classes (per CR4).
- Accessibility baseline checklist must be synchronized with audit results from `#298` and `#301`.

Validation gate:

- Control-plane and matrix artifacts agree on status.
- No contradictory claim between docs and runtime behavior for active milestone slices.
- Accessibility baseline checklist reflects actual test results (no stale "Untested" rows).

### AG1 — Graph Camera + Interaction Reliability

**Status**: `partial`

Feature objective: Graph camera/navigation/selection interactions are deterministic.

Primary issues: `#173`, `#104`, `#101`, `#103`, `#271`, `#185`, `#102`.

**v0.0.2 amendments**:

- Per-view selection migration (per CR3): selection state scoped to `GraphViewId`, not `GraphBrowserApp`.
- Lasso metadata keying must be per-view (`egui_graphs_metadata_` path indexed by view ID).

Validation gate:

- Camera pan/zoom/reset/fit pass in harness.
- Lasso + selection semantics work under modifier combinations and in multi-pane split layout.
- Per-view selection is implemented with test coverage for second pane / non-default `GraphViewId`.
- User-visible regressions in stabilization bug register are closed with scenario evidence.

### AG2 — Frame / Pane Lifecycle Determinism

**Status**: `partial`

Feature objective: Open/close/split/focus handoff and first-render activation are deterministic.

Primary issues: `#174`, `#186`, `#187`, `#118`, `#119`.

Validation gate:

- Spawn/activate, close-successor, focus handoff scenarios pass.
- No blank viewport / focus-race repros in active stabilization register.

### AG3 — Content Opening Semantics

**Status**: `open`

Feature objective: Content-opening actions always route through Graphshell semantic ownership.

Primary issues: `#175`.

Validation gate:

- No legacy context-menu bypass path creates pane state outside declared intent flow.
- Routing diagnostics include reason/path for open decisions.

### AG4 — Command Surface Unification

**Status**: `open`

Feature objective: F2/global, contextual, and radial/menu command surfaces share one command model.

Primary issues: `#176`, `#106`, `#107`, `#108`, `#178`, `#270`.

**v0.0.2 amendments**:

- Undo/redo stack for destructive `GraphIntent` mutations (per CR1).
  - Architecture: undo stack model for `GraphIntent` reversal.
  - UI surface: Ctrl+Z / Ctrl+Shift+Z (or platform equivalent).
  - Scope: graph mutations (node create/delete, edge create/delete, metadata changes). Non-graph actions (view state, UI toggles) may be excluded from v0.0.2 undo scope.

Validation gate:

- Command naming/scope reflects canonical semantics.
- Command invocation parity exists across keyboard/pointer entry points.
- Disabled-state policy is explicit and test-covered.
- Undo/redo works for destructive graph mutations with test coverage.

### AG5 — Settings + Control Surfaces Are First-Class

**Status**: `partial`

Feature objective: Settings/history/control surfaces behave as first-class panes.

Primary issues: `#109`, `#110`, `#177`, `#189`.

Validation gate:

- Settings changes with persistence semantics have scenario tests.
- Tool pane entry/exit return-target is deterministic.

### AG6 — Viewer Presentation + Fallback Clarity

**Status**: `partial`

Feature objective: Viewer render mode behavior and fallback states are explicit, deterministic, and diagnostics-visible.

Primary issues: `#162`, `#188`, `#111`, `#112`.

Validation gate:

- Surface Composition Contract provable in runtime evidence (pass ordering, fallback visibility).
- `TileRenderMode`-driven behavior test-covered.

### AG7 — Diagnostics + UX Semantics Automation Authority

**Status**: `open`

Feature objective: Reliability enforced through scenario + diagnostics + semantic probes.

Primary issues: `#94`, `#251`, `#257`, `#261`, `#269`, `#272`, `#273`.

Validation gate:

- Critical-path UxHarness scenarios exist and run as a required gate.
- UxTree/Probe invariants catch ownership/focus/routing regressions before merge.

### AG8 — Active Scaffold Retirement

**Status**: `open`

Feature objective: No pre-wgpu-critical feature remains in scaffold state for core UX closure paths.

Active scaffolds:

- `[SCAFFOLD:view-dimension-ui-wiring]`
- `[SCAFFOLD:divergent-layout-commit]`
- `[SCAFFOLD:viewer-wry-runtime-registration]`
- `[SCAFFOLD:verse-protocol-handler]`
- `[SCAFFOLD:wasm-mod-loader-runtime]`

Validation gate:

- Core-UX-impacting scaffolds: closure criteria met and marker removed.
- Non-core scaffolds: explicitly de-scoped from v0.0.2 with rationale documented.

### AG9 — WGPU Start Authorization Gate

**Status**: `blocked` (by AG0–AG8 + `#180`)

Feature objective: Begin renderer migration only when application readiness is true.

This gate closes v0.0.2 and opens the post-v0.0.2 renderer migration track.

Validation gate:

- All AG0–AG8 closed with evidence.
- Runtime viewer bridge precondition (`#180`) evidenced as solved.
- Application readiness conditions from `egui_wgpu_custom_canvas_migration_strategy.md` §Application Readiness Gate all true.

### AG1/AG2 Stabilization Notes (2026-03-03)

- Detail-view omnibar submit routing now resolves target deterministically by preferring focused node mapping, then preferred input webview mapping, before create-node fallback.
- Toolbar submit dispatch now ignores `lost_focus + Enter` as an implicit submit trigger; submission requires focused-enter or explicit queued submit state.
- Added regression tests for detail-submit target resolution and toolbar submit-dispatch gating to guard against focus/routing regressions.
- Added pane-transition focus handoff regression coverage to verify that frame activation retargets to the remaining mapped node after close transitions.
- Remaining AG2 follow-up: validate that tile focus handoff after pane/tab transitions keeps `preferred_input_webview_id` aligned with expected toolbar target in multi-pane scenarios.

---

## 5) Spec Conflict Resolution Status

From `2026-03-03_spec_conflict_resolution_register.md`. P1–P2 must be resolved before v0.0.2; P3–P4 should be resolved but may be deferred with explicit documentation.

| Priority | Item | Status | v0.0.2 required? |
| --- | --- | --- | --- |
| **P1** | Rewrite `pane_chrome_and_promotion_spec.md` (separate Pane Opening Mode from chrome/promotion) | `landed` | **Yes** |
| **P2.2** | Add internal frame address semantics (`graphshell://frame/<FrameId>` spec basis; `verso://frame/<FrameId>` runtime canonical alias) to `graph_first_frame_semantics_spec.md` | `landed` | **Yes** |
| **P2.3** | Add `NavigationTrigger::PanePromotion` to `edge_traversal_spec.md` | `landed` | **Yes** |
| **P3.4** | Terminology cleanup in `workbench_tab_semantics_overlay_and_promotion_plan.md` | `landed` | Recommended |
| **P3.5** | Add demotion-driven Tombstone path to `node_lifecycle_and_runtime_reconcile_spec.md` | `landed` | Recommended |
| **P3.6** | Clarify address-as-identity impact on `storage_and_persistence_integrity_spec.md` | `landed` | Recommended |
| **P4.7** | New plan: Pane Opening Mode + SimplificationSuppressed | `landed` | **Yes** (D1) |
| **P4.8** | New plan: internal address scheme implementation (`graphshell://` original plan basis; `verso://` runtime canonical namespace) | `landed` | **Yes** (D3) |

Key decisions backing this resolution order:

- **D1**: Pane Opening Mode is a separate plan (not folded into tab-semantics plan).
- **D2**: "Promotion" reserved exclusively for graph-enrollment semantics.
- **D3**: Internal address scheme implementation is a closure prerequisite (`graphshell://` as original spec baseline; `verso://` as current runtime canonical namespace with compatibility parsing).

---

## 6) Lane-to-Gate Mapping

How the 10 active execution lanes from `PLANNING_REGISTER.md` §1C map to AG gates. This determines lane priority for v0.0.2 closure.

| Lane | Rank | Primary AG gates served | v0.0.2 critical? |
| --- | --- | --- | --- |
| `lane:stabilization` (`#88`) | 1 | AG0, AG1, AG2, AG6 | **Yes** — blocks all interaction reliability gates |
| `lane:control-ui-settings` (`#89`) | 2 | AG4, AG5 | **Yes** — command surface + undo + settings |
| `lane:embedder-debt` (`#90`) | 3 | AG3, AG6 | **Yes** — content opening semantics + compositor pass contract |
| `lane:runtime-followon` (`#91`) | 4 | AG7 (partial) | Partial — SR2/SR3 contributes to diagnostics authority |
| `lane:viewer-platform` (`#92`) | 5 | AG6, AG8 | **Yes** — `TileRenderMode`, viewer fallback, scaffold retirement |
| `lane:accessibility` (`#95`) | 6 | AG0 | **Yes** — all-surfaces WCAG A (per CR4) |
| `lane:diagnostics` (`#94`) | 7 | AG7 | **Yes** — automation authority gate |
| `lane:subsystem-hardening` (`#96`) | 8 | AG5, AG8 | Partial — storage/history/security integrity |
| `lane:test-infra` (`#97`) | 9 | AG7, AG9 | **Yes** — CI gate infrastructure for release validation |
| `lane:knowledge-capture` (`#98`) | 10 | AG8 | Partial — UDC/badges/tagging if in scaffold scope |

**Lanes 1–3 are the critical path.** They serve the gates with the most open items and the broadest failure-mode coverage.

---

## 7) New v0.0.2 Work Items (From Contradiction Resolutions)

These items are net-new requirements surfaced by the doc audit and contradiction resolution. They must be scoped, ticketed, and assigned to lanes.

### W1 — Undo/Redo Stack (CR1)

**Lane**: `lane:control-ui-settings` (`#89`)  
**AG gate**: AG4  
**Scope**:

1. Design undo stack architecture for `GraphIntent` reversal (intent log with inverse operations, or snapshot-based rollback).
2. Implement undo/redo for destructive graph mutations (node/edge create, delete, metadata changes).
3. Wire UI surface (Ctrl+Z / Ctrl+Shift+Z or platform equivalent).
4. Add scenario tests for undo/redo across critical graph operations.
5. Update `command_semantics_matrix.md` to annotate which "Undoable: Yes" rows are implemented vs. planned.

**Done gate**: Ctrl+Z reverts the last destructive graph mutation; Ctrl+Shift+Z re-applies it. Covered by tests.

### W2 — Per-View Selection Migration (CR3)

**Lane**: `lane:stabilization` (`#88`)  
**AG gate**: AG1  
**Scope**:

1. Refactor selection state from `GraphBrowserApp`-global to per-`GraphViewId` scope.
2. Update lasso metadata keying to per-view (`egui_graphs_metadata_` indexed by `GraphViewId`).
3. Validate multi-pane selection independence (selecting in pane A does not affect pane B).
4. Update `focus_and_region_navigation_spec.md` to mark per-view selection as implemented (not just target).

**Done gate**: Selection state is per-`GraphViewId`; multi-pane selection test passes; lasso metadata works in second pane.

**Progress note (2026-03-03)**:

- W2 implementation work is landed on `main` for runtime paths (selection reads/writes now route through focused/per-view selection helpers instead of global runtime consumers).
- Final W2 sweep found no notable remaining runtime callsites requiring migration; residual `workspace.selected_nodes` uses are test-only or compatibility-mirror plumbing inside `graph_app.rs`.
- W2 is treated as implementation-complete; AG1 remains `partial` pending the other AG1 stabilization items.

### W3 — All-Surfaces WCAG 2.2 Level A (CR4)

**Lane**: `lane:accessibility` (`#95`)  
**AG gate**: AG0  
**Scope**:

1. Run WCAG 2.2 Level A audit across all 7 surface classes (Graph Pane, Node Pane, Tool Pane, Radial Menu, Command Palette, Omnibar, Settings).
2. Fix known Level A failures (radial disabled contrast from audit #301).
3. Update `accessibility_baseline_checklist.md` to reflect actual test results (replace "Untested" rows).
4. Document any Level A criteria that are not applicable or explicitly deferred with rationale.

**Done gate**: Accessibility baseline checklist shows Pass or N/A for all Level A criteria across all 7 surfaces.

---

## 8) Doc Health Audit Results

### Stale Documents (Require Update Pass)

| Document | Date | Staleness | Recommended action |
| --- | --- | --- | --- |
| `technical_architecture/ARCHITECTURAL_CONCERNS.md` | 2026-02-17 | Partial | Synchronize with post-Feb-17 canonical specs. Several concerns now have more thorough resolutions. |
| `technical_architecture/ARCHITECTURAL_OVERVIEW.md` | 2026-02-17 | Partial | Update "Not Yet Implemented" list (diagnostics inspector, selection consolidation are now advanced). Reconcile crate versions and LOC counts. Remove archived checkpoint references as primary sources. |
| `aspect_render/2026-02-20_embedder_decomposition_plan.md` | 2026-02-21 | Partial | Stage 4 task status needs reconciliation. Line-number references in Reality Check section are likely outdated after refactoring. |
| `viewer/2026-02-26_composited_viewer_pass_contract.md` | 2026-02-26 | Partial | §A.0.3 overlay affordance "accidentally correct" debt claim needs validation against current code. If structurally enforced since then, update the debt assessment. |

### Accessibility Baseline Checklist Desync

The `accessibility_baseline_checklist.md` does not reflect concrete test results from `#298` (keyboard focus audit) and `#301` (accessibility closure bundle). WCAG 1.4.3 row shows "Untested" for surfaces where audit #301 recorded a specific fail. **Must be updated before v0.0.2.**

### Additional Doc Contradictions (Non-Blocking)

- `command_semantics_matrix.md` "Undoable" column will be accurate once W1 (undo stack) is implemented — no immediate rewrite needed, but an annotation distinguishing "implemented" from "target" is recommended.
- Multiple subsystem UX semantics specs (`ux_tree_and_probe_spec.md`, `ux_event_dispatch_spec.md`, `ux_scenario_and_harness_spec.md`) have unchecked acceptance criteria despite being tagged "Pre-renderer/WGPU required." This is expected — they define the target contract, not current implementation.

---

## 9) Execution Sequencing

### Phase 1 — Stabilization + Interaction Reliability (AG1, AG2, AG3)

**Lanes**: `lane:stabilization` (#88), `lane:embedder-debt` (#90)

1. Close stabilization bug register regressions (camera, focus activation, lasso boundary, deselect).
2. Implement per-view selection (W2).
3. Close AG3 content-opening semantics (legacy context-menu bypass retirement).
4. Close AG2 lifecycle determinism (spawn/activate, close-successor, focus handoff).

**Exit criteria**: AG1/AG2/AG3 moved from `partial`/`open` to `closed`.

### Phase 2 — Command Surface + Undo (AG4, AG5)

**Lanes**: `lane:control-ui-settings` (#89)

1. Unify F2/contextual/radial command surfaces.
2. Implement undo/redo stack for graph mutations (W1).
3. Close settings scaffold (AG5).

**Exit criteria**: AG4/AG5 moved to `closed`.

### Phase 3 — Viewer + Compositor Closure (AG6)

**Lanes**: `lane:viewer-platform` (#92), `lane:embedder-debt` (#90)

1. Land `TileRenderMode` on `NodePaneState` with ViewerRegistry-driven resolution.
2. Close compositor pass contract (Content Pass → Overlay Affordance Pass structurally enforced, not "accidentally correct").
3. Validate viewer fallback/degraded-state clarity with diagnostics.

**Exit criteria**: AG6 moved to `closed`.

### Phase 4 — Spec Conflict Resolution (P1–P4)

**Lane**: `lane:roadmap` (docs)

**Status**: `in progress` (all named P1–P4 doc slices landed; parity pass and any follow-on tracker synchronization remain)

1. P1 rewrite landed (`pane_chrome_and_promotion_spec.md`).
2. P2.2 and P2.3 landed (frame address semantics, navigation trigger).
3. P3 terminology/lifecycle clarifications landed.
4. P4 new plan docs landed (Pane Opening Mode, internal address scheme), and the implementation has since advanced to `verso://` as the canonical system namespace with legacy `graphshell://` compatibility plus initial `notes://`, `graph://`, and `node://` domain-address scaffolds.

**Post-P4 implementation delta (2026-03-03)**:

- Typed internal address parsing/formatting is live, with canonical `verso://` emission and legacy `graphshell://` parse compatibility.
- Workbench authority routing is live for `verso://settings/...`, `verso://frame/...`, `verso://tool/...`, and `verso://view/...`.
- `verso://view/...` is no longer just a raw single-ID route:
  - legacy `verso://view/<id>` remains as a compatibility graph-pane alias,
  - canonical routing shape is now `verso://view/<kind>/<id>`,
  - `verso://view/note/<NoteId>` routes to the note-open path,
  - `verso://view/node/<NodeId>` routes to node-pane opening,
  - `verso://view/graph/<GraphId>` now queues named graph restore when a matching snapshot exists.
- Durable note scaffolding is landed in code (`notes://<NoteId>`, in-memory `NoteRecord`, note creation/open routing), but a real note pane/editor surface is still pending.
- `graph://<GraphId>` and `node://<NodeId>` now participate in explicit route-intent handling (`graph` snapshot restore; `node` pane open/focus when resolvable).
- `notes://<NoteId>` and `node://<NodeId>` are now emitted directly from address-bar domain routing; `notes` resolves into note-open queueing and `node` is intercepted by workbench authority.
- `OpenNoteUrl` is now intercepted in workbench authority (with reducer-side leak warning parity to `OpenGraphUrl`/`OpenNodeUrl`/`OpenClipUrl`).
- Pending note-open requests are now consumed in the semantic lifecycle (open linked node pane when available and focus History manager as an interim note surface path).
- Graph-view address submission now has explicit non-mutation route parity for `verso://view/node/<NodeId>`, `verso://view/note/<NoteId>`, and `verso://view/graph/<GraphId>` (all emitted as workbench route intents).
- Graph-view address submission now also has explicit non-mutation parity for internal `settings`, `tool`, and `clip` routes (legacy `graphshell://...` canonicalized to `verso://...` intent emission).
- Legacy `graphshell://view/node/<NodeId>` submissions now have explicit canonicalization parity (`verso://view/node/<NodeId>`) with non-mutation route-intent coverage.
- Legacy `graphshell://view/note/<NoteId>` and `graphshell://view/graph/<GraphId>` now also have explicit canonicalization parity tests (`verso://view/note/...`, `verso://view/graph/...`).
- Legacy `graphshell://view/<GraphViewId>` submissions now also have explicit canonicalization parity (`verso://view/<GraphViewId>`) with non-mutation route-intent coverage.
- `OpenViewUrl` now has explicit reducer-boundary scenario coverage across `view/node`, `view/note`, and `view/graph` route variants (`OpenViewUrl` remains workbench-authority and does not mutate graph state when reducer-applied).
- `resolve_view_route(...)` parser coverage now explicitly includes node/note/graph target variants for canonical `verso://view/<kind>/<id>` routes.
- Legacy `graphshell://frame/...` and `graphshell://tool/...` routes now have explicit canonicalization parity tests (`verso://frame/...`, `verso://tool/...`).
- `OpenFrameUrl` and `OpenToolUrl` now have explicit reducer-boundary scenario coverage (workbench-authority only; no reducer graph mutation).
- Legacy `graphshell://settings/...` routes now have explicit canonicalization parity tests (`verso://settings/...`).
- Invalid `OpenFrameUrl` inputs now have explicit pass-through fallback coverage (unconsumed by orchestration authority when route parse fails).
- `verso://clip/<id>` now routes through workbench authority and its queued clip-open requests are consumed during the semantic lifecycle (History manager focus as interim clip surface path).

**Current practical blocker after Phase 4 docs**:

- The next meaningful closure is a durable note pane/editor authority so `notes://<NoteId>` routes resolve into a first-class note surface rather than queue-only scaffolding.

**Exit criteria**: P1–P2 closed; P3–P4 landed or explicitly deferred.

### Phase 5 — Automation + Accessibility + Scaffolds (AG0, AG7, AG8)

**Lanes**: `lane:accessibility` (#95), `lane:diagnostics` (#94), `lane:test-infra` (#97)

1. Run all-surfaces WCAG A audit and fix failures (W3).
2. Land UxHarness critical-path scenarios.
3. Retire or de-scope active scaffolds.
4. Update stale docs from §8.

**Exit criteria**: AG0/AG7/AG8 moved to `closed`.

### Phase 6 — Release Validation (AG9)

**Lane**: All lanes converge.

1. Verify all AG0–AG8 are closed with linked evidence.
2. Run test guide §4 minimum acceptance checks.
3. Verify runtime viewer bridge precondition (`#180`) is evidenced.
4. Bump `Cargo.toml` version to `0.0.2`.
5. Tag `v0.0.2`, build release artifacts, publish release notes.

**Exit criteria**: v0.0.2 tagged and released.

---

## 10) Phase Parallelism

Phases are not strictly sequential. The following can run in parallel:

- **Phase 1 + Phase 4**: Stabilization code work and spec conflict resolution docs work touch different hotspots.
- **Phase 2 + Phase 3**: Command surface unification and viewer/compositor work are in different lanes and different hotspot files (except where `render/mod.rs` is shared — serialize those PRs).
- **Phase 5 accessibility + Phase 5 diagnostics**: Different subsystems, different code paths.

**Serialization constraints**:

- Phase 6 blocks on all other phases.
- AG7 (automation authority) should have progress before AG1/AG2/AG6 close, since UxHarness scenarios validate those gates.
- Undo stack (W1) may have architectural implications for graph intent dispatch; coordinate with stabilization work touching `GraphIntent` paths.

---

## 11) Post-v0.0.2: Renderer Migration (Not In Scope)

After v0.0.2 ships, the following sequence begins:

1. **Renderer switch readiness (G1–G5)** from `webrender_readiness_gate_feature_guardrails.md`:
   - G1: Dependency control and reproducibility
   - G2: Backend contract parity
   - G3: Pass-contract safety
   - G4: Platform confidence
   - G5: Regression envelope

2. **WebRender wgpu implementation (P0–P12)** from `webrender_wgpu_renderer_implementation_plan.md`.

3. **Five unanswered gating questions** from `egui_wgpu_custom_canvas_migration_requirements.md`:
   - §3.1: Runtime viewer surface interop (the #1 blocker)
   - §3.2: GPU ownership model
   - §3.3: Canvas presentation strategy
   - §3.4: Measurable success criteria (frame budget, node counts, latency targets)
   - §3.5: Rollback/fallback plan

These are explicitly out of v0.0.2 scope. Track A (v0.0.2) ships on Glow. Track B (renderer migration) is post-v0.0.2.

---

## 12) Document Cross-References

This plan subsumes, references, and deduplicates the following planning artifacts:

| Document | Relationship to this plan |
| --- | --- |
| `2026-03-03_pre_wgpu_feature_validation_gate_checklist.md` | Gates AG0–AG9 derived from this checklist's G0–G9 with CR amendments |
| `2026-03-03_spec_conflict_resolution_register.md` | §5 incorporates its P1–P4 backlog and D1–D3 decisions |
| `2026-03-01_complete_feature_inventory.md` | Feature counts and status codes provide the baseline; pre-wgpu closure checklist aligns with AG gates |
| `2026-03-02_scaffold_registry.md` | AG8 references its 5 active scaffolds |
| `2026-03-01_ux_migration_lifecycle_audit_register.md` | UX closure items feed AG0 and AG7 |
| `2026-03-01_ux_migration_feature_spec_coverage_matrix.md` | Spec coverage status informs AG0 and spec conflict resolution |
| `2026-03-01_ux_migration_design_spec.md` | Authoritative UX design target; v0.0.2 implements its Phase 1–3 |
| `subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md` | Baseline done-gate and milestone checklist inform AG0 and Phase 5 |
| `aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md` | Deferral doctrine + application readiness gate define AG9 |
| `aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md` | G1–G5 renderer-switch gates are post-v0.0.2 (§11) |
| `aspect_render/2026-03-01_webrender_wgpu_renderer_implementation_plan.md` | P0–P12 phases are post-v0.0.2 (§11) |
| `PLANNING_REGISTER.md` §1C | Top 10 lanes mapped to AG gates in §6 |
| `system/VERSIONING_POLICY.md` | v0.0.2 bump semantics |
| `2026-02-28_stabilization_progress_receipt.md` | Evidence for AG1/AG2 partial closure |
| `design/accessibility_baseline_checklist.md` | AG0 accessibility gate; desync noted in §8 |
| `design/command_semantics_matrix.md` | AG4 command surface; undo annotation needed per CR1 |
| `testing/test_guide.md` | §4 minimum acceptance checks are the AG9 release gate |

---

## 13) Execution Policy Reminders

From the pre-wgpu gate checklist, unchanged:

1. **No net-new UX feature expansion** while AG1, AG2, AG3, or AG6 is open.
2. Any PR touching a gate-owned area must include: contract-level scenario evidence, diagnostics evidence, parity doc delta.
3. If a regression reopens a closed gate, that gate reverts to `open` immediately.
4. AG9 cannot be manually overridden by schedule pressure.
5. Progress is measured only by **feature closure + validation gate evidence**, not time.

---

## 14) Summary Status Dashboard

| Gate | Status | Blocking lanes | Key open items |
| --- | --- | --- | --- |
| AG0 | `open` | stabilization, accessibility | All-surfaces WCAG A, a11y checklist desync |
| AG1 | `partial` | stabilization | Camera/lasso/selection reliability, per-view selection (W2) |
| AG2 | `partial` | stabilization, embedder-debt | Focus activation race, close-successor handoff |
| AG3 | `open` | embedder-debt | Legacy context-menu bypass retirement |
| AG4 | `open` | control-ui-settings | Command unification, undo/redo (W1) |
| AG5 | `partial` | control-ui-settings, subsystem-hardening | Settings scaffold, tool pane return-target |
| AG6 | `partial` | viewer-platform, embedder-debt | `TileRenderMode`, compositor pass contract |
| AG7 | `open` | diagnostics, test-infra | UxHarness scenarios, UxTree/Probe invariants |
| AG8 | `open` | viewer-platform, knowledge-capture | 5 active scaffolds |
| AG9 | `blocked` | All | Blocked by AG0–AG8 + `#180` |

**Gates closed: 0/10**  
**Gates partial: 4/10** (AG1, AG2, AG5, AG6)  
**Gates open: 5/10** (AG0, AG3, AG4, AG7, AG8)  
**Gates blocked: 1/10** (AG9)
