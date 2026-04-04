<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Shell Command Bar Execution Plan (2026-04-03)

**Status**: Active follow-on execution plan
**Scope**: Turns Workstream A from `shell_backlog_pack.md` into a concrete execution lane for the Shell-owned `CommandBar`, omnibar/session state, focused-target resolution, and legacy command-route cleanup.

**Related**:

- `SHELL.md`
- `shell_backlog_pack.md`
- `shell_composition_model_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../navigator/NAVIGATOR.md`

**Implementation anchors**:

- `shell/desktop/ui/toolbar/toolbar_ui.rs` — current `shell_command_bar` panel and mixed-control rendering
- `shell/desktop/ui/toolbar/toolbar_location_panel.rs` — omnibar session lifecycle and provider fetch debounce/ingest path
- `shell/desktop/ui/toolbar/toolbar_omnibar.rs` — provider suggestion fetch helpers
- `shell/desktop/ui/gui_orchestration.rs` — command palette open/toggle routing
- `shell/desktop/ui/gui/focus_state.rs` — command-palette focus authority and return-target realization

---

## Context

The Shell specs already define the right authority boundary:

- Shell owns command entry, omnibar input, top-level command chrome, and host-thread visible state.
- Navigator contributes read-only context projection into the omnibar display seam.
- Command meaning and availability belong to the unified command authority, not to individual UI widgets.

The current code is partway there but still structurally mixed:

- `toolbar_ui.rs` renders `shell_command_bar`, but the left column still mixes Shell controls,
  Navigator-hosted graph-view tabs, graph mutation shortcuts, lens/physics menus, and command
  triggers in one panel pass.
- `toolbar_location_panel.rs` already uses a receiver-based provider fetch path, but the Shell-level
  mailbox/session contract is still only implicit in code.
- command palette open/toggle and focus return behavior exist in `gui_orchestration.rs` and
  `gui/focus_state.rs`, but the overall command-bar target-resolution contract is not yet gathered
  into one execution lane.

This plan exists to make that seam honest and executable without waiting on every other Shell lane.

---

## Non-Goals

- redesigning the entire Shell layout in this lane
- redefining command meaning outside the canonical command-surface spec
- moving Navigator breadcrumb ownership into Shell
- solving Shell overview or ambient-status work here
- replacing all existing omnibar/provider code before the authority seams are defined

---

## Feature Target 1: Split CommandBar Authority From Mixed Toolbar Content

### Target 1 Context

The current `shell_command_bar` panel is named correctly but still semantically mixed. The first
job is to make the redistribution explicit.

### Target 1 Tasks

1. Inventory the controls currently rendered in `toolbar_ui.rs` and classify each as Shell,
   Navigator, Graph, Workbench, Viewer, or bridged legacy.
2. Keep Shell-owned controls in the `CommandBar`: omnibar input, command palette trigger,
   settings/control entry, and app-level status affordances.
3. Move Navigator-owned graph-view tabs and similar projection controls out of Shell-owned render
   code into Navigator projection/host surfaces.
4. Identify graph mutation shortcuts (`+Node`, `+Edge`, `+Tag`) and decide whether they remain as
   explicit Shell command triggers or should move to a more domain-local presentation.

### Target 1 Validation Tests

- The `CommandBar` has an explicit ownership inventory rather than a grab-bag of toolbar widgets.
- Navigator projection controls are no longer rendered by Shell-owned code paths.
- Every remaining `CommandBar` control has a clear authority story.

### Target 1 Concrete Code-Change Checklist

`toolbar_ui.rs`

- enumerate the current left-column render calls in `TopBottomPanel::top("shell_command_bar")` and
    record the ownership decision for each one before moving code. The current inventory is:
    `render_navigator_view_tabs(...)` as a Navigator projection host surface candidate,
    `render_wry_compat_button(...)` as a legacy bridged control to keep visible during the split,
    `render_graph_history_buttons(...)` as a classify-or-relocate control,
    `+Node` / `+Edge` / `+Tag` as explicit Graph command triggers,
    `render_graph_bar_lens_menu(...)` and `render_graph_bar_physics_menu(...)` as graph-scoped
    controls that should not survive in the Shell bar by inertia alone,
    `Overview` as a Shell-overview-vs-graph-layout classification point, and
    `Cmd` as a retained Shell-owned trigger.
- split the current three-column rendering into ownership-named helper passes so the file stops
   treating the bar as one undifferentiated toolbar block.
- keep `render_location_search_panel(...)` as the center Shell-owned input seam and document that it
   consumes Navigator projection without owning it.
- audit `toolbar_controls::render_navigation_buttons(...)` and `render_toolbar_right_controls(...)`
   the same way as the left column so right-side chrome does not remain a mixed-authority escape
   hatch.

`gui_orchestration.rs` and `gui/focus_state.rs`

- verify that the `Cmd` button and keyboard command-palette entry still converge on the same
   `WorkbenchIntent::ToggleCommandPalette` and focus-return path after the toolbar split.
- keep any temporary legacy bridge controls explicitly labeled in the plan and diagnostics until
   they can be rerouted or relocated.

---

## Feature Target 2: Make Focused-Target Resolution A First-Class Shell Input

### Target 2 Context

Per-pane viewer controls and command routing must not infer their targets ad hoc from whichever
surface rendered last.

### Target 2 Tasks

1. Define and land a per-frame `CommandBarFocusTarget`-style carrier as described in
   `shell_composition_model_spec.md`.
2. Route `toolbar_ui.rs` and adjacent command-bar helpers through that carrier rather than local
   inference from `focused_toolbar_node`, `active_toolbar_pane`, or similar partial state.
3. Keep command-palette open/toggle and return-target behavior aligned with the focus authority in
   `gui/focus_state.rs`.
4. Ensure keyboard focus owner precedence and last-pointer fallback precedence are explicit and
   testable.

### Target 2 Validation Tests

- Viewer-targeted controls in the command bar resolve against one Shell-owned target carrier.
- Command palette focus entry/exit preserves deterministic return targets.
- Focus precedence is testable and does not depend on widget render order.

---

## Feature Target 3: Turn Omnibar Session State Into An Explicit Host-Thread Mailbox Contract

### Target 3 Context

The current omnibar provider flow already uses debounced receivers, but it still reads like local
toolbar logic rather than a host-owned Shell session seam.

### Target 3 Tasks

1. Define the omnibar session as Shell-owned frame-loop state, including query text, mode, matches,
   active index, and provider status.
2. Make provider/background suggestion results an explicit mailbox owned by the current omnibar
   session, not just an implementation detail of `toolbar_location_panel.rs`.
3. Ensure all background provider/index fetches are launched under `ControlPanel` or equivalent
   Shell/Register supervision.
4. Keep the Navigator contribution read-only: breadcrumb/context projection in display mode and
   scope badge in input mode.

### Target 3 Validation Tests

- Omnibar provider results only enter visible Shell state at frame boundaries.
- Background suggestion fetches are supervised and diagnosable.
- Switching between display and input mode preserves the Shell/Navigator seam.

### Target 3 Concrete Code-Change Checklist

`toolbar_ui.rs`

- treat `OmnibarSearchSession` as the current carrier to stabilize rather than replacing it first.
- separate session lifecycle ownership from widget rendering so the top bar only consumes session
   state instead of also acting as the mailbox controller.
- keep the existing `OmnibarSessionKind`, `query`, `matches`, `active_index`, and provider status
   fields as the baseline contract while the mailbox cleanup lands.

`toolbar_location_panel.rs`

- isolate the session-state transitions that currently happen inline when:
   entering provider mode, reusing cached suggestions, arming
   `provider_debounce_deadline`, attaching `provider_rx`, draining `try_recv()` outcomes, and
   merging provider results back into visible matches.
- make the mailbox edge explicit around the existing fields on `OmnibarSearchSession`:
   `provider_rx`, `provider_debounce_deadline`, and `provider_status`.
- preserve the current display-mode/input-mode split, including Navigator breadcrumb rendering and
   scope badge behavior, while moving background fetch ownership out of ad hoc panel logic.
- ensure session invalidation and query changes cancel or supersede stale provider deliveries
   instead of silently letting late results win.

`toolbar_omnibar.rs`

- keep `spawn_provider_suggestion_request(...)` as the single background launch point unless a
   stronger Shell runtime seam replaces it.
- document that `ControlPanel::spawn_blocking_host_request(...)` is the required supervision path
   for provider suggestions and future omnibar background fetches.
- keep provider cache writes and parsed-metadata reuse compatible with the new mailbox ownership so
   cache hits and fetched outcomes flow through the same session contract.

`gui_frame.rs` or equivalent frame-bound ingest site

- identify the frame-bound point where completed provider outcomes become visible Shell state and
   use that as the canonical mailbox-drain boundary.
- add diagnostics for stale delivery, disconnected receivers, and provider failure states at that
   boundary rather than burying them inside widget-local behavior.

---

## Feature Target 4: Audit And Reroute Legacy Command Bypass Paths

### Target 4 Context

The docs already flag legacy embedder/context-menu flows as a correctness problem. This lane should
own the audit and reroute plan for Shell-facing command entry points.

### Target 4 Tasks

1. Audit browser-native or embedder-originated open/new-tab/context-menu paths that still bypass
   Graphshell command and pane semantics.
2. Reroute those paths through the canonical command/action or workbench-intent routes where
   possible.
3. Where rerouting is not yet practical, document the exception explicitly and emit diagnostics so
   the bypass remains visible instead of accidental.
4. Keep the audit scoped to Shell-facing command entry and routing seams, not every viewer/embedder
   integration issue.

### Target 4 Validation Tests

- Known legacy command-bypass paths are either rerouted or explicitly documented as temporary.
- Bypass cases emit diagnosable evidence instead of silently mutating pane/view state.
- Shell command routing remains the user-visible authority even when an embedder surface exists.

---

## Feature Target 5: Land Diagnostics And Acceptance Evidence For Command Handoff

### Target 5 Context

This lane is only useful if Shell can prove when command routing is correct, blocked, or bypassed.

### Target 5 Tasks

1. Reuse and extend existing UX/dispatch diagnostics channels for command-bar and omnibar routing.
2. Add scenario-backed evidence for `SH02`, `SH05`, and the relevant command-bar/focus return paths.
3. Ensure blocked commands, missing targets, and focus-return fallbacks are visible in diagnostics.
4. Tie acceptance to both command-surface parity and Shell host-thread ownership rules.

### Target 5 Validation Tests

- Failed or blocked command-bar dispatch emits explicit diagnostic evidence.
- Command palette return-path failures are diagnosable.
- The lane can produce evidence that Shell routes commands without becoming the owner of Graph,
  Navigator, Workbench, or Viewer truth.

---

## Suggested Slice Order

1. Authority inventory and control redistribution in `toolbar_ui.rs`
2. Focused-target carrier and focus-return cleanup
3. Omnibar session/mailbox contract cleanup
4. Legacy bypass audit and reroute pass
5. Diagnostics and scenario evidence

This order keeps the architectural seam clear before doing cleanup work that depends on it.

---

## Exit Condition

This plan is complete when Graphshell has a Shell-owned `CommandBar` with explicit control
ownership, a first-class focused-target carrier, an omnibar session/mailbox contract supervised by
Shell/Register runtime boundaries, and diagnosable routing for both canonical command entry and
legacy bypass cases.
