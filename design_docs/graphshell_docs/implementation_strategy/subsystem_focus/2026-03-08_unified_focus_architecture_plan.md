# Unified Focus Architecture Plan

**Date**: 2026-03-08  
**Status**: Active / architecture cleanup plan  
**Scope**: Reconcile the canonical Focus subsystem contract with the actual runtime split across workbench activation, graph-view focus, GUI hints, local widget focus, and embedded-content input routing.

**Related**:
- `SUBSYSTEM_FOCUS.md`
- `focus_and_region_navigation_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `../system/2026-03-08_servoshell_debtclear_plan.md`

---

## 1. Why This Plan Exists

The Focus subsystem docs define the right behavioral target, but they overstate how unified the current runtime model is.

In practice, the app currently tracks several partially overlapping “active” or “focused” concepts:

- semantic region focus
- workbench tile/pane activation
- `GraphViewId` focus and selection scope
- local widget focus and text-entry capture
- embedded-content/webview input focus
- toolbar/chrome targeting heuristics

Those are related, but they are not the same thing. Treating them as one flat focus state has allowed old servoshell-era heuristics such as `preferred_input_webview` and `active_webview` to remain hidden authority surfaces.

This plan defines the missing top-level focus taxonomy so the subsystem can converge on an explicit state model instead of overlapping heuristics.

---

## 2. Canonical Focus Taxonomy

The Focus subsystem should explicitly distinguish six tracks:

1. `SemanticRegionFocus`
2. `PaneActivationFocus`
3. `GraphViewFocus`
4. `LocalWidgetFocus`
5. `EmbeddedContentFocus`
6. `FocusReturnAndCapture`

`ChromeProjectionTargeting` is closely related, but should remain a separate concern linked to focus rather than collapsed into it.

---

## 3. Canonical Tracks

### 3.1 `SemanticRegionFocus`

Owns:

- which top-level region currently owns keyboard/command semantic input
- region-cycle ordering
- explicit cross-region handoff
- focus diagnostics at the region level

Canonical authority:

- Focus router

Current reality:

- region-cycle and handoff behavior exist in workbench/gui orchestration
- no single explicit runtime focus-state object currently owns all semantic-region transitions

Architecture rule:

- semantic region focus is the cross-app authority for command targeting
- hover and local widget focus must not silently replace it

### 3.2 `PaneActivationFocus`

Owns:

- which tile/pane is currently active in the workbench
- deterministic successor on close
- pane-level handoff during structural workbench changes

Canonical authority:

- Workbench subsystem

Current reality:

- tile activation and close successor handoff exist and are tested
- pane activation is sometimes used as a proxy for broader focus state

Architecture rule:

- pane activation informs focus routing, but it does not replace semantic region focus

### 3.3 `GraphViewFocus`

Owns:

- active `GraphViewId`
- graph-view-scoped selection/camera/lens targeting
- graph-view-specific keyboard command targeting

Canonical authority:

- Graph / Focus bridge

Current reality:

- `workspace.focused_view` and per-view selection scope are real and important
- this track is the strongest implemented part of the subsystem

Architecture rule:

- `GraphViewId` is not pane identity and not region identity
- it is a graph-owned scoped view-state focus target

### 3.4 `LocalWidgetFocus`

Owns:

- widget-local focus within a focused region
- text-entry capture inside host UI
- local control restore tokens

Canonical authority:

- framework/UI layer, subordinate to Focus router

Current reality:

- omnibar/location field and widget-local focus checks exist through egui local focus state

Architecture rule:

- local widget focus is never the global semantic owner

### 3.5 `EmbeddedContentFocus`

Owns:

- input focus inside web content or embedded viewers
- host escape/reclaim bindings
- focus transfer between host UI and embedded content

Canonical authority:

- Focus router defines policy
- host/content boundary code enforces it

Current reality:

- several host paths still defer to `preferred_input_webview` / `active_webview`
- this is the main place where servoshell-era assumptions still distort focus semantics

Architecture rule:

- embedded content focus is subordinate to host-region routing and must always have a deterministic host escape path

### 3.6 `FocusReturnAndCapture`

Owns:

- modal capture
- command-surface capture
- text-entry capture
- stored return anchors
- deterministic fallback when return target is invalid

Canonical authority:

- Focus router

Current reality:

- capture/return behavior exists in pieces across orchestration, modal flags, and local control logic
- there is not yet one explicit capture stack or return-anchor model

Architecture rule:

- return-path semantics must be explicit state, not emergent behavior from current UI flags

---

## 4. Current Implementation Snapshot

### 4.1 Landed

- deterministic region-cycle behavior
- pane-close focus restoration paths
- graph-surface versus node-pane focus distinction in GUI runtime state
- `GraphViewId`-scoped selection targeting tied to focused view
- local text-entry focus-loss guards for omnibar/location flows

### 4.2 Partial / Inconsistent

- semantic region focus is spread across orchestration and UI state, not one focus-state authority
- pane activation and focus ownership are tightly coupled in practice
- toolbar target resolution uses fallback heuristics rather than canonical focus identities
- embedded-content focus still uses host heuristics rooted in preferred webview selection

### 4.3 Missing

- one explicit runtime focus model covering all six tracks
- explicit focus identities (`FocusRegion`, `PaneId`, `LocalFocusTarget`, `EmbeddedContentTarget`, `ReturnAnchor`)
- a canonical capture stack / return-anchor object
- explicit subsystem state for focus diagnostics and inspection
- full replacement of `preferred_input_webview` / `active_webview` as hidden focus authorities

---

## 5. Architectural Corrections

### 5.1 Define Explicit Focus Identities

The subsystem should introduce explicit focus-state vocabulary at runtime:

- `FocusRegion`
- `PaneId`
- `GraphViewId`
- `LocalFocusTarget`
- `EmbeddedContentTarget`
- `ReturnAnchor`

Without these, the app will keep using incidental state as proxy focus authority.

### 5.2 Separate Region Focus From Pane Activation

The workbench owns active pane/tile state.  
The Focus subsystem owns semantic region focus.

Those must stay linked but distinct.

### 5.3 Keep GraphView Focus As Its Own Track

`focused_view` should remain the authority for graph-view-scoped selection/camera targeting, but it must not be overloaded into pane identity or global region identity.

### 5.4 Extract Embedded Content Focus From Webview Preference Heuristics

The servoshell debt-clear plan and this plan intersect here:

- `preferred_input_webview`
- `active_webview`
- toolbar target webview fallback
- dialog owner by focused webview

These must become explicit embedded-content focus or chrome-projection decisions, not hidden focus authority.

### 5.5 Make Capture And Return Explicit

The modal / command-surface / text-entry return path should be backed by explicit capture state, not only by open-surface flags and local widget checks.

---

## 6. Sequencing Plan

### Phase A. Taxonomy Cleanup

1. Update subsystem docs to distinguish the six focus tracks.
2. Add explicit language that `GraphViewId` is view focus, not pane identity.
3. Add direct cross-link to the servoshell debt-clear plan for embedded-content focus cleanup.

Done-gate:

- focus docs no longer imply one already-centralized runtime focus state

### Phase B. State Model Definition

1. Define canonical runtime focus identities.
2. Define `FocusState` / capture-stack shape at the architecture level.
3. Map existing runtime fields (`focused_view`, `focused_node_hint`, `graph_surface_focused`, tile active state, local widget focus) onto that model.

Done-gate:

- the subsystem has an explicit state model, not only interaction prose

### Phase C. Authority Separation

1. Separate semantic region focus from pane activation in implementation seams.
2. Keep graph-view focus as graph-owned scoped state.
3. Define toolbar/chrome targeting as a separate linked concern.

Done-gate:

- pane, region, and graph-view focus are no longer conflated

### Phase D. Embedded Content Focus Cleanup

1. Remove `preferred_input_webview` / `active_webview` as hidden focus authorities.
2. Replace them with explicit embedded-content focus and host escape semantics.
3. Align host input routing with the servoshell debt-clear plan.

Done-gate:

- embedded-content focus is subordinate to host focus policy, not the other way around

### Phase E. Diagnostics And Inspection Closure

1. Expose a focus-state inspector/summary for diagnostics.
2. Emit explicit diagnostics for capture enter/exit, return-path fallback, and embedded-content reclaim failures.
3. Extend UX scenario coverage around the canonical capture/return table.

Done-gate:

- focus failures are diagnosable in subsystem terms rather than inferred from scattered UI state

---

## 7. Cross-Plan Dependencies

- servoshell debt clear plan: embedded-content focus, toolbar target resolution, dialog ownership, and webview-centric authority cleanup
- workbench specs: pane activation and close successor semantics
- UX semantics subsystem: focus diagnostics, no-trap guarantees, and scenario probes
- graph/workbench boundary docs: `GraphViewId` versus pane identity versus arrangement authority

This is not a separate optional side plan. It is the state-model cleanup required for focus, workbench, and host-content control flow to stop overlapping incorrectly.

---

## 8. Recommended Immediate Actions

1. Update `SUBSYSTEM_FOCUS.md` and `focus_and_region_navigation_spec.md` to reference this architecture plan.
2. Add an explicit “runtime reality gap” section to the focus subsystem guide.
3. Add a short state-model section naming the six tracks and the missing explicit focus identities.
4. Open follow-ons for:
   - focus-state type design
   - toolbar/chrome target decoupling from focus heuristics
   - embedded-content focus cleanup
   - capture-stack / return-anchor implementation

---

## 9. Done Definition

The Focus subsystem architecture is coherent when:

- region focus, pane activation, graph-view focus, local widget focus, embedded-content focus, and return/capture are modeled as distinct tracks
- explicit runtime focus identities exist
- no webview preference heuristic acts as hidden global focus authority
- graph-view focus remains graph-owned and does not collapse into pane identity
- capture and return semantics are backed by explicit state
- subsystem diagnostics can describe focus failures and fallbacks coherently

---

## 10. Implementation Delta (2026-03-12)

This section records what has been implemented since the original plan write-up.

### 10.1 Landed In Code

- `RuntimeFocusAuthorityState` now includes an explicit `capture_stack` (`Vec<FocusCaptureEntry>`), making capture/return state part of focus authority.
- `FocusCommand` now includes explicit capture commands:
   - `Capture { surface, return_anchor }`
   - `RestoreCapturedFocus { surface }`
- `apply_focus_command(...)` was extended so command-surface capture transitions mutate authority state directly (including stack push/pop behavior) instead of relying only on UI flags.
- Authority-handled workbench intents now use a realization reconciliation pass that compares desired semantic region against observed runtime state.
- A dedicated diagnostics channel was added for reconciliation mismatch reporting:
   - `ux:focus_realization_mismatch`

### 10.2 Why This Matters

This is the first concrete inversion toward authority-first focus flow:

- reducer writes authority state first
- realization attempts to project that onto tiles/app runtime state
- reconciliation emits diagnostics when realization diverges from authority

This addresses the previous blind spot where post-intent refresh could silently overwrite authority with observed state, masking realization failures.

### 10.3 Still Outstanding

- Full separation of `desired` vs `realized` fields inside `RuntimeFocusAuthorityState` is still pending.
- The frame-level sync path still contains mirror-style refresh behavior outside the authority-handled intent slice.
- Region cycle, command palette, and transient restore are only partially migrated to strict reducer+realizer ownership; broader path consolidation remains.
- Embedded-content focus and webview preference heuristic cleanup remains in follow-on work.

### 10.4 Updated Near-Term Next Slice

The next implementation slice should complete strict authority ownership for the existing high-impact path:

1. keep `CycleFocusRegion` planning in orchestration, but ensure semantic region authority is never overwritten by observed state on that path
2. fully route command palette open/close through reducer + realizer + reconciliation without mirror fallback writes
3. extend reconciliation checks to transient restore and tool-pane return semantics
4. add focused tests that assert mismatch diagnostics when realization cannot satisfy authority intent
