<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Shell Command Bar Execution Plan (2026-04-03)

**Status**: Prototype-conformance closure lane (refreshed 2026-04-06)
**Scope**: Turns Workstream A from `shell_backlog_pack.md` into a finishable prototype-conformance lane for the Shell-owned `CommandBar`, omnibar/session state, focused-target resolution, and removal or relocation of non-Shell and legacy command-route seams.

**Related**:

- `SHELL.md`
- `shell_backlog_pack.md`
- `shell_composition_model_spec.md`
- `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`
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

Refresh note (2026-04-05): the original 2026-04-03 draft is no longer a pure forward plan. Several carriers and routing seams described here are now landed in code, so this document now tracks only the remaining work needed to close Workstream A cleanly and make the file archivable.

Prototype posture (2026-04-06): Graphshell is still a prototype with no user-migration constraint, so this lane should prefer deletion or relocation over compatibility-preserving bridge work. Temporary host seams are useful only as short-lived inventory aids. They are not acceptable closure state for the Shell UI itself. The target architecture is a Shell-owned command bar that retains only Shell-owned controls plus the explicit Navigator read-only omnibar-context seam.

---

## Non-Goals

- redesigning the entire Shell layout in this lane
- redefining command meaning outside the canonical command-surface spec
- moving Navigator breadcrumb ownership into Shell
- solving Shell overview or ambient-status work here
- replacing all existing omnibar/provider code before the authority seams are defined

---

## Current Checkpoint (2026-04-05)

Already landed and no longer primary blockers for this plan:

- `CommandBarFocusTarget` is now a real Shell-owned carrier used by toolbar, workbench, render, and radial-menu call sites rather than remaining only a proposed shape.
- command palette toggle/open routing converges on `WorkbenchIntent::ToggleCommandPalette` / related `WorkbenchIntent`s through shared toolbar-routing helpers instead of one-off toolbar button behavior.
- omnibar provider requests now return through `HostRequestMailbox<T>` under `ControlPanel` supervision rather than leaving raw request receivers on the frame thread.
- long-lived Shell-facing signal relays now drain through `GuiFrameInbox`, making the frame-bound ingest seam explicit for subscription-style updates.
- toolbar Navigator view-tab focus and overview-plane toggle already route through `WorkbenchIntent`, so those actions are no longer the main command-routing bypass risk in this lane.

Still open and required before this plan can be archived:

- the `shell_command_bar` still renders non-Shell controls that should be removed or relocated if the prototype is to converge on the target architecture: Navigator view tabs, viewer navigation, the viewer compat toggle, and the graph history / mutation / lens / physics / fit cluster.
- the lane still speaks in migration language about temporary host seams, legacy bridge controls, and documented exceptions; for prototype closure, those should collapse to canonical reroutes, relocations, deletion, or narrowly bounded host/runtime adapters outside the Shell UI surface.
- `SH02` / `SH05` evidence is only partially represented by code-level diagnostics and focused tests; this plan still needs a clear closure receipt or equivalent evidence handoff before archival.
- the cross-subsystem observability, UxTree, and AT work needed to prove command-surface closure now lives in `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md` and remains an explicit dependency for archival.

Recent implementation progress recorded against this lane:

- command-surface routing now emits structured `resolved`, `blocked`, `fallback`, and `no-target` provenance receipts rather than relying only on later downstream effects.
- command-palette dismiss and tool-surface focus restore now preserve explicit fallback evidence when the stored return target is stale.
- command-surface semantic metadata now projects into the UxTree snapshot path, trace schema, and diagnostics-oriented inspector counts instead of remaining toolbar-local state.
- broader Shell cleanup has already refreshed stale diagnostics assertions and pre-wgpu UxTree baselines to match the new receipt and schema shape.
- scoped UxTree contract helpers now enforce command-surface capture-owner / return-target invariants plus trace-layer ID consistency and semantic parent-link integrity through the existing publish diagnostics path.
- the previously failing broad-shell expectations around anchored new-node placement and default lens physics have now been reconciled to the current contracts, and `cargo test shell::desktop -- --quiet` is green again.
- command-surface scenario coverage now includes a healthy command-surface UxTree snapshot case, a stale-return fallback receipt scenario, and refreshed diff-gate expectations, while the accessibility baseline checklist now carries explicit first-pass command-surface AT tasks and a concrete Windows manual-pass recipe instead of leaving the lane implicit.
- `shell_command_bar` now renders only Shell-owned steady-state controls in code: `Overview` /
   `Cmd` on the left and `Settings` / `More` on the right. Viewer navigation remains in the
   workbench host chrome, viewer backend toggles remain reachable through Settings, and graph-local
   lens/view controls remain pane-local rather than command-bar residents.
- graph-view switching now projects through Navigator host chrome in code: rendered graph-scope
   hosts show the view switcher directly, while a narrow host-owned fallback strip appears only
   when no graph-scope host is currently rendered.

---

## Prototype Conformance Target (2026-04-06)

The Shell command bar should converge to the following steady state:

- retain only Shell-owned controls in the bar itself: omnibar input, command entry, app-level settings/control access, ambient status, and any genuine Shell overview trigger;
- allow one explicit cross-domain seam inside the omnibar: Navigator may contribute read-only context projection, but not own command interpretation or general chrome residency;
- remove Navigator, Viewer, and Graph controls from the bar as steady-state residents, even when they currently route correctly;
- permit host/runtime protocol adapters only outside the Shell UI ownership boundary, and only if they terminate in canonical Shell/Workbench command surfaces rather than mutating app truth directly.

The prototype goal is not "better labeled mixed chrome." It is architectural convergence.

### Current CommandBar Inventory And Prototype Disposition

| Current control | Authority | Current state | Prototype disposition |
| --- | --- | --- | --- |
| Omnibar input field | Shell | correct resident | Keep in `CommandBar` |
| Omnibar breadcrumb / context display | Navigator (read-only seam) | acceptable cross-domain seam | Keep only as explicit read-only omnibar contribution |
| Graph view tabs | Navigator | relocated to Navigator host chrome, with a host-owned fallback strip only when no graph-scope host is rendered | Continue converging toward fully configured graph-scope host defaults |
| Wry / Servo compat toggle | Viewer legacy bridge | temporary viewer bridge in `shell_command_bar` | Delete from Shell bar; relocate only if a viewer-local debug/control surface still needs it |
| Back / Forward / Reload / Zoom | Viewer | temporary host seam in `shell_command_bar` | Relocate to pane-local viewer chrome |
| Undo / Redo | Graph | temporary host seam in `shell_command_bar` | Relocate to graph-local chrome or command surfaces |
| `+Node` / `+Edge` / `+Tag` | Graph | temporary host seam in `shell_command_bar` | Relocate to graph-local chrome or command surfaces |
| Lens menu | Graph / Navigator-adjacent | temporary host seam in `shell_command_bar` | Relocate to graph-view or Navigator-owned chrome |
| Physics menu | Shell policy or graph-facing config, depending on final contract | currently grouped with graph controls | Remove from the bar; if retained, expose through a Shell control surface rather than primary command chrome |
| Fit | Graph | temporary host seam in right-column chrome | Relocate to graph-local chrome |
| Overview | Shell | plausible steady-state Shell control | Keep only if it remains an app-level overview surface rather than graph-local affordance |
| Cmd / command palette trigger | Shell | correct resident | Keep in `CommandBar` |
| Settings / More / ambient status | Shell | correct resident | Keep in `CommandBar` or `StatusBar` |

---

## Feature Target 1: Split CommandBar Authority From Mixed Toolbar Content

**Status**: In progress — the Shell bar is now Shell-only in code; remaining work is relocation evidence, destination-surface cleanup, and archival-strength validation

### Target 1 Context

The current `shell_command_bar` panel is no longer mixed in code, but this target is not fully
closed until the removed controls have stable destination-surface evidence and the surrounding
docs/semantics stop modeling them as command-bar residents.

### Target 1 Tasks

1. Keep the control inventory explicit in the document and code, but treat inventory as an input to removal or relocation decisions rather than closure by itself.
2. Finish the ownership split in `toolbar_ui.rs` so the file's helper passes are not only named by authority, but also stop leaving non-Shell controls in the Shell bar by inertia.
3. Relocate Navigator-owned graph-view tabs out of the Shell-owned bar, leaving only the explicit read-only omnibar-context seam in shared chrome.
4. Relocate Viewer controls out of the Shell-owned bar: the compat toggle plus pane-targeted navigation should move to viewer-local chrome or be deleted if no longer justified.
5. Relocate Graph controls out of the Shell-owned bar: history, mutation shortcuts, fit, lens, and graph-local config should live in graph-local or Navigator-owned chrome rather than app-level command chrome.
6. Decide whether `Overview` remains a true Shell overview trigger; if not, relocate it as well.

### Target 1 Validation Tests

- The `CommandBar` retains only Shell-owned controls plus the explicit Navigator read-only omnibar-context seam.
- Viewer-targeted and graph-targeted controls are no longer steady-state residents of the Shell bar.
- Any control still present in `CommandBar` has a defensible Shell-ownership reason rather than only a migration note.

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
- reduce the current left-column mixed block so no helper remains a catch-all home for unresolved controls, then start removing or relocating the non-Shell helpers instead of merely documenting them.
- keep `render_location_search_panel(...)` as the center Shell-owned input seam and document that it
   consumes Navigator projection without owning it.
- remove or relocate `render_command_bar_navigator_projection(...)`, `render_command_bar_viewer_bridge(...)`, and `render_command_bar_graph_controls(...)` from `shell_command_bar` steady state; only Shell-owned helpers should remain there after closure.
- audit `toolbar_controls::render_navigation_buttons(...)` and `render_toolbar_right_controls(...)`
   the same way as the left column so right-side chrome does not remain a mixed-authority escape
   hatch.

`gui_orchestration.rs` and `gui/focus_state.rs`

- verify that the `Cmd` button and keyboard command-palette entry still converge on the same
   `WorkbenchIntent::ToggleCommandPalette` and focus-return path after the toolbar split.
- remove or relocate any temporary legacy bridge controls rather than treating their labeled presence as acceptable closure state.

---

## Feature Target 2: Make Focused-Target Resolution A First-Class Shell Input

**Status**: Closed in code; retained here as closure receipt

### Target 2 Context

Per-pane viewer controls and command routing must not infer their targets ad hoc from whichever
surface rendered last.

### Target 2 Tasks

1. Treat `CommandBarFocusTarget` as landed baseline rather than future design work.
2. Audit remaining command-bar-adjacent helpers for any direct partial-state inference that should instead consume the carrier or its canonical construction helper.
3. Keep command-palette open/toggle and return-target behavior aligned with the focus authority in `gui/focus_state.rs` and shared toolbar-routing helpers.
4. Expand evidence for keyboard-focus precedence and last-pointer fallback precedence where the behavior is now implemented but not yet closed by this plan.

Closure note (2026-04-06): the remaining ad hoc nav-action target construction now converges on `nav_targeting::command_bar_focus_target(...)`, while focused helper tests and toolbar-routing tests explicitly cover chrome-projection precedence over focused-node fallback.

### Target 2 Validation Tests

- Viewer-targeted controls in the command bar resolve against one Shell-owned target carrier.
- Command palette focus entry/exit preserves deterministic return targets.
- Focus precedence is testable and does not depend on widget render order.

---

## Feature Target 3: Turn Omnibar Session State Into An Explicit Host-Thread Mailbox Contract

**Status**: Partially landed; remaining work is closure proof and stale-delivery hardening

### Target 3 Context

The current omnibar provider flow already uses debounced receivers, but it still reads like local
toolbar logic rather than a host-owned Shell session seam.

### Target 3 Tasks

1. Treat the existing `OmnibarSearchSession` + provider mailbox carrier as the stabilized baseline for this lane.
2. Keep provider/background suggestion results flowing through the explicit Shell-owned mailbox boundary rather than regressing to widget-local receivers or direct background mutation.
3. Preserve `ControlPanel` supervision as the required launch path for provider/index fetches and future one-shot Shell-owned background jobs.
4. Keep the Navigator contribution read-only: breadcrumb/context projection in display mode and scope badge in input mode.

### Target 3 Validation Tests

- Omnibar provider results only enter visible Shell state at frame boundaries.
- Background suggestion fetches are supervised and diagnosable.
- Switching between display and input mode preserves the Shell/Navigator seam, including stale-delivery cancellation or supersession behavior.

### Target 3 Concrete Code-Change Checklist

`toolbar_ui.rs`

- treat `OmnibarSearchSession` as the current carrier to stabilize rather than replacing it first.
- separate session lifecycle ownership from widget rendering so the top bar only consumes session
   state instead of also acting as the mailbox controller.
- keep the existing `OmnibarSessionKind`, `query`, `matches`, `active_index`, and provider status
   fields as the baseline contract while the mailbox cleanup lands.
- do not replace the current carrier again unless a stronger Shell-owned contract is actually needed by remaining open tasks.

`toolbar_location_panel.rs`

- isolate the session-state transitions that currently happen inline when:
   entering provider mode, reusing cached suggestions, arming
   `provider_debounce_deadline`, attaching `provider_rx`, draining `try_recv()` outcomes, and
   merging provider results back into visible matches.
- make the mailbox edge explicit around the existing fields on `OmnibarSearchSession`:
   provider request identity, debounce deadline, mailbox/result carrier, and provider status.
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

**Status**: Open

### Target 4 Context

The docs already flag legacy embedder/context-menu flows as a correctness problem. This lane should
own the audit and reroute plan for Shell-facing command entry points, but for prototype closure it
should bias toward canonical reroute or deletion rather than leaving a broad class of "documented
exception" behavior behind.

### Target 4 Tasks

1. Audit browser-native or embedder-originated open/new-tab/context-menu paths that still bypass
   Graphshell command and pane semantics.
2. Reroute those paths through the canonical command/action or workbench-intent routes where
   possible.
3. Where rerouting is not yet practical, reduce the remaining case to a narrowly bounded host/runtime adapter outside Shell UI ownership and emit diagnostics so the boundary remains visible instead of accidental.
4. Keep the audit scoped to Shell-facing command entry and routing seams, not every viewer/embedder
   integration issue.

### Target 4 Validation Tests

- Known legacy command-bypass paths are either rerouted or reduced to explicit host/runtime adapters that do not mutate app truth directly.
- Bypass cases emit diagnosable evidence instead of silently mutating pane/view state.
- Shell command routing remains the user-visible authority even when an embedder surface exists.

---

## Feature Target 5: Land Diagnostics And Acceptance Evidence For Command Handoff

**Status**: Open

### Target 5 Context

This lane is only useful if Shell can prove when command routing is correct, blocked, or bypassed.
The Shell-owned receipts in this target now depend on the shared observability and AT closure work
tracked in `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`.
That companion lane owns the command-surface provenance vocabulary, UxTree/probe/scenario hooks,
and Shell command-surface AT validation recipes; this target consumes those receipts for `SH02` / `SH05` closure.

### Target 5 Tasks

1. Reuse and extend existing UX/dispatch diagnostics channels for command-bar and omnibar routing.
2. Add scenario-backed evidence for `SH02`, `SH05`, and the relevant command-bar/focus return paths.
3. Ensure blocked commands, missing targets, and focus-return fallbacks are visible in diagnostics.
4. Tie acceptance to both command-surface parity and Shell host-thread ownership rules.
5. Consume the shared command-surface provenance, UxTree, and AT receipts from the companion plan instead of leaving those obligations implicit inside Shell-only wording.

### Target 5 Validation Tests

- Failed or blocked command-bar dispatch emits explicit diagnostic evidence.
- Command palette return-path failures are diagnosable.
- Omnibar and command-palette routing evidence is strong enough to cite both semantic/probe coverage and AT validation tasks, not only local toolbar tests.
- The lane can produce evidence that Shell routes commands without becoming the owner of Graph,
  Navigator, Workbench, or Viewer truth.

---

## Suggested Slice Order

1. Finish the ownership inventory and control redistribution in `toolbar_ui.rs`.
2. Close the remaining legacy bypass audit and decide reroute vs. deletion vs. narrowly bounded host/runtime adapter.
3. Land the remaining `SH02` / `SH05` diagnostics and scenario evidence for command handoff, blocked routing, and focus return.
4. Close the companion observability / UxTree / AT tasks required for command-surface evidence handoff.
5. Write the closure receipt or backlog checkpoint delta that states what downstream Shell work may now assume.

This order keeps the finish work focused on real remaining blockers instead of reopening already-landed carrier and mailbox refactors.

---

## Archive Trigger

Archive this plan only when all of the following are true:

1. no unresolved mixed-authority control remains in the `shell_command_bar` without an explicit temporary-host justification and exit path,
2. no Navigator, Viewer, or Graph control remains a steady-state resident of the `shell_command_bar` other than the explicit Navigator read-only omnibar-context seam,
3. legacy command-bypass cases covered by this lane are either rerouted through canonical Shell/Workbench command authority or reduced to explicit host/runtime adapters outside Shell UI ownership,
4. `SH02` / `SH05` closure evidence exists in scenarios, diagnostics receipts, or a Shell closure receipt,
5. the companion observability / UxTree / AT tasks in `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md` are closed strongly enough that this file no longer carries them as an active dependency, and
6. `shell_backlog_pack.md`, `SHELL.md`, and `DOC_README.md` no longer need this file as the live execution surface for Workstream A.

---

## Exit Condition

This plan is complete when Graphshell has a Shell-owned `CommandBar` containing only Shell-owned
controls plus the explicit Navigator read-only omnibar seam, a first-class focused-target carrier,
an omnibar session/mailbox contract supervised by Shell/Register runtime boundaries, diagnosable
routing for both canonical command entry and any remaining explicit host/runtime adapters, and a
closure handoff strong enough that the remaining state belongs in a receipt or backlog checkpoint
rather than an active execution plan.
