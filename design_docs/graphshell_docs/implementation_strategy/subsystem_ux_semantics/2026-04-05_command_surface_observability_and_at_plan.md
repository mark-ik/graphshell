# Command Surface Observability and AT Plan (2026-04-05)

**Status**: Active companion closure lane
**Scope**: Cross-subsystem closure plan for Shell command-surface provenance diagnostics, UxTree/UxProbe/UxScenario modeling for command surfaces, and Shell command-bar / omnibar assistive-technology validation.

**Related**:

- `../shell/2026-04-03_shell_command_bar_execution_plan.md`
- `../shell/SHELL.md`
- `../shell/shell_composition_model_spec.md`
- `ux_tree_and_probe_spec.md`
- `ux_event_dispatch_spec.md`
- `SUBSYSTEM_UX_SEMANTICS.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`
- `../subsystem_accessibility/accessibility_interaction_and_capability_spec.md`
- `../../design/accessibility_baseline_checklist.md`
- `../domain_interaction_acceptance_matrix.md`

---

## Context

The Shell command-bar lane is no longer blocked on inventing new carriers.
Several important execution seams are already landed in code:

- `CommandBarFocusTarget` exists as a real Shell-owned focused-target carrier.
- omnibar provider/background suggestion work already flows through a Shell-owned mailbox path supervised by `ControlPanel`.
- long-lived frame-bound Shell signal relays already have an explicit `GuiFrameInbox` path.
- toolbar and command-palette routing already converge substantially on shared `WorkbenchIntent` helpers.

The remaining closure problem is now mostly **observability and evidence**, not baseline carrier design.
Today Graphshell can often do the right thing, but the canon still lacks one linked closure lane for proving all of the following at once:

- which target the `CommandBar` resolved and why,
- how that decision appears in `UxTree` / `UxSnapshot` / probes / scenarios,
- how omnibar and command-palette focus capture and return are validated for assistive technology,
- which receipts the Shell lane must consume before it can archive.

This plan exists so those gaps are owned jointly without fragmenting the work into three unrelated mini-plans.

Progress checkpoint (2026-04-05):

- structured command-surface provenance channels are now landed for `resolved`, `blocked`, `fallback`, and `no-target` routing outcomes, with retained payload fields visible in recent diagnostics receipts.
- Shell command chrome now publishes command-surface semantic metadata that projects into UxTree semantic/presentation/trace snapshots and the accessibility inspector's semantic-node summary.
- focused cleanup is already complete for the immediate fallout from those schema and receipt changes: stale diagnostics expectations, scenario schema assertions, and committed pre-wgpu UxTree baselines have been updated.
- scoped helper-style UxTree probes now cover command-surface capture-owner conflicts, missing command-surface return paths, orphan trace-layer IDs, and missing semantic parent links via the existing publish-time diagnostics seam.
- focused `command_surface_` and broader `shell::desktop::workbench::ux_tree::tests::` validation slices are green with those helpers in place; the remaining broad `shell::desktop` failures currently observed sit outside this closure lane.
- the main work still open in this companion lane is stronger scenario coverage and manual AT validation, not the initial provenance vocabulary or projection plumbing.

---

## Owner Split

- **Shell** owns command-surface execution, visible state, and host-thread ingestion boundaries.
- **UX Semantics** owns semantic projection, probe invariants, snapshot/trace vocabulary, and scenario hooks.
- **Accessibility** owns capability declarations, AT-facing behavior, and manual screen-reader validation.

This plan does not move execution ownership away from Shell. It only makes the remaining cross-subsystem closure work explicit.

---

## Non-Goals

- redesigning command semantics outside the canonical command-surface spec
- replacing `CommandBarFocusTarget` or the current omnibar mailbox baseline without a proven need
- duplicating the Shell execution lane in a second Shell-only plan
- creating a separate accessibility-only planning lane unless the follow-on evidence work proves this split is insufficient

---

## Workstream A: Diagnostics Provenance

**Goal**: Shell command-surface routing must produce explicit receipts for target resolution, blocked routing, fallback routing, and no-target outcomes.

### Diagnostics Tasks

1. Extend the existing Shell command-bar and omnibar diagnostics so target resolution is recorded as an explicit receipt rather than inferred indirectly from later effects.
2. Define the minimum resolution payload for command-surface diagnostics:
   - command surface kind (`command_bar_button`, `command_palette`, `omnibar_submit`)
   - requested action/intent id
   - resolution source (`keyboard_focus`, `last_pointer`, `stored_return_target`, `none`)
   - resolved target kind / ids
   - blocked reason, fallback reason, or no-target reason when applicable
   - session/request identity for omnibar provider and command-palette flows
3. Reuse existing channel families where possible and add companion receipts only where current channels cannot represent provenance clearly.
4. Define explicit evidence expectations for `SH02` / `SH05`: command-surface routing receipts, blocked-route receipts, fallback-focus receipts, and stale-delivery receipts.

### Diagnostics Done Shape

- Shell diagnostics can distinguish `resolved`, `blocked`, `fallback`, and `no-target` outcomes for command-surface actions.
- focus-return fallback stops being an inferred warning and becomes a queryable receipt.
- stale omnibar/provider deliveries are visible as Shell host-boundary evidence instead of widget-local behavior.

---

## Workstream B: UxTree, Probe, and Scenario Modeling

**Goal**: Command surfaces become first-class semantic/testable surfaces rather than an implementation detail outside the UxTree contract.

### Semantic Modeling Tasks

1. Add command-surface semantic coverage to the canonical UxTree model:
   - `CommandBar` as a distinct Shell-owned semantic role
   - explicit omnibar, command-palette trigger, and command-surface status nodes
   - target-resolution trace fields attached to the command-surface subtree or snapshot trace layer
2. Extend build-order and projection language so Shell command chrome is emitted before downstream hosted regions in a deterministic way.
3. Add command-surface contract extensions to the S/N/M invariants set:
   - command-surface focused node must match visible capture state
   - command-palette and omnibar dismiss must restore the stored valid return path or emit an explicit fallback receipt
   - stale provider deliveries must not silently replace newer visible omnibar state
4. Add explicit scenario hooks for command-surface coverage:
   - `UXCS01` command-surface target-resolution parity
   - `UXCS02` omnibar capture, submit, and return-path restoration
   - `UXCS03` command-palette dismiss, blocked-route, and fallback diagnostics
5. Keep the UxTree / UxSnapshot vocabulary aligned with Shell terminology so `CommandBarFocusTarget`, omnibar session state, and focus-region names are referred to consistently across docs.

### Semantic Modeling Done Shape

- command surfaces appear in UxTree and snapshots as stable semantic surfaces rather than only as implicit chrome.
- probes and scenarios can fail specifically on command-surface routing or return-path regressions.
- the Shell lane can cite semantic and scenario receipts instead of relying only on implementation anchors.

---

## Workstream C: Accessibility and AT Validation

**Goal**: Shell command-bar and omnibar closure includes explicit accessibility capability declarations and manual AT baseline evidence instead of remaining mostly "Untested".

### Accessibility Tasks

1. Add Shell command-surface capability declarations to the accessibility canon:
   - command-bar role/state exposure
   - omnibar input labeling and purpose
   - command-surface status/error announcement policy
   - return-path expectations for omnibar and command palette
2. Define required announcement/status cases for Shell command surfaces:
   - omnibar provider loading / stale / failed states
   - invalid input or no-target command-surface errors
   - blocked route and fallback-focus outcomes when user-visible
3. Add manual AT baseline validation tasks for Windows first:
   - NVDA sanity pass
   - Narrator sanity pass
   - focus identification, status announcement, and return-path checks for omnibar and command palette
4. Keep reduced-motion and focus-visible expectations in scope for command-surface show/hide behavior.
5. Treat this work as a sub-gate for the Shell lane's closure evidence rather than as a separate replacement plan.

### Accessibility Done Shape

- Shell command surfaces have declared accessibility capabilities and degradation expectations.
- omnibar and command-palette behavior has explicit AT validation recipes.
- the accessibility baseline stops describing command surfaces only as generic untested rows.

---

## Slice Order

1. Link this plan from the active Shell lane and Shell canon so the ownership split is explicit.
2. Refresh UxTree, dispatch, and focus canon to add command-surface vocabulary, invariants, and scenario hooks.
3. Refresh accessibility canon and the baseline checklist so command-surface AT work is explicit.
4. Refresh acceptance/index docs so the doc graph points at this plan as the cross-subsystem closure dependency.
5. Consume the resulting receipts from the Shell execution lane before archiving the Shell command-bar plan.

---

## Closure Relationship To The Shell Lane

This file is not a replacement for `../shell/2026-04-03_shell_command_bar_execution_plan.md`.
The Shell plan remains the live execution lane for command-bar authority, omnibar mailbox state,
and legacy bypass cleanup.

Archive the Shell plan only after both of the following are true:

1. the remaining Shell execution tasks in that lane are complete, and
2. the observability / semantic / AT receipts defined here exist strongly enough that Workstream A no longer needs an active closure plan.

Until then, this companion plan should be treated as the required cross-subsystem dependency for Shell command-surface closure rather than as optional follow-on reading.
