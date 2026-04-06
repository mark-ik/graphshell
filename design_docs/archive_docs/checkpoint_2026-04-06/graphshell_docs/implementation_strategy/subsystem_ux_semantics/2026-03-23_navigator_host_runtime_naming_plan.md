# Navigator Host Runtime Naming Plan

**Date**: 2026-03-23
**Status**: Archived 2026-04-06
**Scope**: Runtime naming migration for desktop chrome state and default-host exposure terminology

**Archive note**:

- This migration is complete in runtime, persistence, and the immediate desktop UI consumers.
- The active authority for ongoing chrome/host semantics remains `2026-03-13_chrome_scope_split_plan.md` and the related Navigator / Shell / Workbench specs.
- This file is retained as the historical receipt for the host-oriented naming rollout.

**Related**:

- `2026-03-13_chrome_scope_split_plan.md`
- `../navigator/NAVIGATOR.md §12`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../workbench/workbench_layout_policy_spec.md`
- `../../../../shell/desktop/ui/workbench_host.rs`
- `../../../../shell/desktop/ui/toolbar/toolbar_ui.rs`
- `../../../../shell/desktop/ui/gui_frame/toolbar_dialog.rs`

---

## 1. Purpose

The documentation now treats the desktop chrome model as one Navigator semantic
surface rendered through default hosts rather than as a fixed split between
toolbar-form and workbench-host chrome surfaces.

The runtime now uses host-oriented naming for the desktop UI module,
`ChromeExposurePolicy` variants, and the persisted workbench pinned state.
Because this is a prototype with no migration requirement, the persistence path
now writes and reads only the host-oriented key. This note records the rename
sequence and remaining follow-up work needed to finish the runtime terminology
alignment without changing behavior.

This is a **naming migration plan**, not a multi-host implementation plan.

Non-goals for this slice:

- enabling multiple live Navigator hosts
- changing chrome layout behavior
- replacing the existing right-side workbench host implementation
- renaming user-facing labels unless they are directly derived from runtime enum names

---

## 2. Current Runtime Anchors

The current desktop runtime already has a stable derived-state seam for chrome
exposure.

### 2.1 Primary file

Current authority:

- `shell/desktop/ui/workbench_host.rs`

Current symbols:

```rust
pub(crate) enum WorkbenchLayerState {
    GraphOnly,
    GraphOverlayActive,
    WorkbenchActive,
    WorkbenchPinned,
}

pub(crate) enum ChromeExposurePolicy {
    GraphOnly,
    GraphWithOverlay,
    GraphPlusWorkbenchHost,
    GraphPlusWorkbenchHostPinned,
}
```

Current persistence seam:

```rust
pub const SETTINGS_WORKBENCH_HOST_PINNED_NAME: &'static str =
  "workspace:settings-workbench-host-pinned";
```

Current downstream usage in that file includes:

- `WorkbenchLayerState::chrome_policy()`
- `WorkbenchChromeProjection::visible()`
- test assertions for graph-only, overlay, active, and pinned states
- string labels in `layer_state_label(...)`

### 2.2 Immediate consumers

- `shell/desktop/ui/toolbar/toolbar_ui.rs`
  - imports `WorkbenchLayerState`
  - uses it to suppress pane-local controls in graph-only / overlay states
- `shell/desktop/ui/gui_frame/toolbar_dialog.rs`
  - pattern-matches on `WorkbenchLayerState`
- workbench-host tests in `shell/desktop/ui/workbench_host.rs`
  - assert exact `ChromeExposurePolicy` variants

### 2.3 Likely follow-on consumers

- UX semantics / UxTree build code once explicit chrome landmarks are added or renamed
- diagnostics and snapshot labels that serialize enum-derived names
- future focus-region enums if desktop host landmarks become runtime data

---

## 3. Rename Targets

### 3.1 Keep stable

Keep these names for now:

- `WorkbenchLayerState`
  - it still describes whether workbench-hosted surfaces are active or pinned
  - it is already used broadly and does not encode the retired surface names
- `WorkbenchActive`
- `WorkbenchPinned`

### 3.2 Rename now

Rename `ChromeExposurePolicy` variants to match the new default-host model:

| Current | Proposed |
| --- | --- |
| `GraphOnly` | `GraphOnly` |
| `GraphWithOverlay` | `GraphWithOverlay` |
| `GraphPlusWorkbenchSidebar` | `GraphPlusWorkbenchHost` |
| `GraphPlusWorkbenchSidebarPinned` | `GraphPlusWorkbenchHostPinned` |

Reason:

- these names directly encode the retired fixed-surface assumption
- the new names preserve current behavior while removing the claim that the
  workbench surface is necessarily a sidebar

### 3.3 Defer until actual runtime support exists

Do not add these runtime names yet unless the implementation actually needs
them:

- `GraphScopedNavigatorHost`
- `WorkbenchScopedNavigatorHost`
- `ChromeRegion`

Reason:

- docs now use these names conceptually, but runtime does not yet model chrome
  regions as first-class host instances
- introducing them prematurely would create type churn without functional gain

---

## 4. File-by-File Migration Plan

### Phase A — Pure enum rename

Status: Complete

Edit:

- `shell/desktop/ui/workbench_host.rs`

Changes:

- rename `ChromeExposurePolicy::GraphPlusWorkbenchSidebar`
  -> `ChromeExposurePolicy::GraphPlusWorkbenchHost`
- rename `ChromeExposurePolicy::GraphPlusWorkbenchSidebarPinned`
  -> `ChromeExposurePolicy::GraphPlusWorkbenchHostPinned`
- update `WorkbenchLayerState::chrome_policy()`
- update `WorkbenchChromeProjection::visible()`
- update related unit tests

Expected behavioral effect:

- none

### Phase B — Immediate consumer cleanup

Status: Complete

Edit:

- `shell/desktop/ui/workbench_host.rs`
- `shell/desktop/ui/toolbar/toolbar_ui.rs`
- `shell/desktop/ui/gui_frame/toolbar_dialog.rs`

Changes:

- update match arms and any debug / trace / helper labels that mention sidebar
- ensure user-facing labels still read naturally; do not surface raw enum names

### Phase C — Persistence rename

Status: Complete

Completed changes:

- renamed `workbench_sidebar_pinned` runtime state to `workbench_host_pinned`
- renamed app accessors to `workbench_host_pinned()` and
  `set_workbench_host_pinned(...)`
- persisted the new `workspace:settings-workbench-host-pinned` key
- removed the legacy sidebar-key fallback to keep the prototype path simple

### Phase D — Snapshot and diagnostics reconciliation

Check for:

- test snapshots
- stringified debug output
- diagnostics payloads
- any UxTree or a11y snapshot fixture names depending on old variant strings

If snapshot names are user-visible or stable-artifact-visible, update them in
the same change rather than leaving mixed terminology.

### Phase E — Optional follow-up structural cleanup

After the enum rename lands cleanly, consider separating file naming from host
semantics:

- current file: `workbench_host.rs`
- possible later split:
  - keep file as implementation home for the default workbench-scoped host, or
  - extract a more neutral `workbench_chrome.rs` / `navigator_host_projection.rs`

This should be treated as optional refactor work, not part of the initial
naming migration.

---

## 5. Acceptance Criteria

The runtime naming migration is complete when:

1. `ChromeExposurePolicy` no longer contains `...WorkbenchSidebar` variants.
2. `WorkbenchLayerState` behavior is unchanged.
3. `toolbar_ui.rs` and `toolbar_dialog.rs` compile without old variant names.
4. persisted workbench pinned state uses host-oriented runtime names.
5. workbench-host unit tests pass with renamed variants.
6. no new runtime type claims multi-host support unless real host-selection
   behavior has been implemented.

---

## 6. Guardrails

- Do not rename `WorkbenchLayerState` in the same patch unless there is a
  concrete behavioral reason.
- Do not introduce multi-host persistence/runtime types in the naming-only pass.
- Do not mix enum renames with broad chrome-behavior changes.
- If debug labels change, update affected tests immediately in the same patch.

---

## 7. Suggested Execution Order

1. Run focused tests for `workbench_host.rs`, persistence, and toolbar-dialog routing.
2. Update any snapshot strings or diagnostics labels.
3. Only after that, consider a larger host-instance runtime design.
