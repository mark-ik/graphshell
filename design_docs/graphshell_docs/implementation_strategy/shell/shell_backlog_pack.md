<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Shell Backlog Pack

**Date**: 2026-03-25
**Status**: Planning / handoff pack
**Scope**: Dependency-ordered backlog for Shell as Graphshell's only host, app-level orchestration boundary, overview surface authority, and control-surface router.

**Related docs**:

- [SHELL.md](SHELL.md) — Shell domain spec and authority boundaries
- [2026-04-03_shell_command_bar_execution_plan.md](2026-04-03_shell_command_bar_execution_plan.md) — active Workstream A closure lane for command-bar ownership, omnibar session/mailbox state, focused-target routing, and legacy command-route cleanup
- [../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — companion cross-subsystem closure lane for command-surface provenance diagnostics, UxTree/probe modeling, and Shell command-surface AT validation
- [shell_composition_model_spec.md](shell_composition_model_spec.md) — top-level slot composition, command-bar seam, and host-thread panel rules
- [shell_overview_surface_spec.md](shell_overview_surface_spec.md) — concrete Shell overview UI and routing model
- [../aspect_command/command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md) — canonical command-entry and dispatch contract consumed by Shell
- [../aspect_control/settings_and_control_surfaces_spec.md](../aspect_control/settings_and_control_surfaces_spec.md) — control-surface routing and page-hosting contract consumed by Shell
- [../domain_interaction_acceptance_matrix.md](../domain_interaction_acceptance_matrix.md) — cross-domain review matrix
- [../../technical_architecture/domain_interaction_scenarios.md](../../technical_architecture/domain_interaction_scenarios.md) — canonical cross-domain scenario flows
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — Navigator projection/navigation peer domain
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — Workbench arrangement/activation peer domain
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph truth/analysis peer domain; canvas remains the rendered graph surface

## Tracker mapping

- Hub issue: #306 (`Hub: five-domain architecture adoption — Shell host, graphlet model, cross-domain scenarios`)
- Primary implementation issue: #303 (`Implement Shell host and overview surface adoption`)
- Review/evidence issue: #305 (`Operationalize cross-domain scenario IDs and acceptance evidence`)

---

## Wave 1

1. `SH01` Shell Host Boundary. Depends: none. Done gate: one canonical doc defines Shell as the application's only host and names what it does not own.
2. `SH02` Shell Command Routing Contract. Depends: `SH01`. Done gate: Shell command entry points are explicitly mapped to Graph, Navigator, Workbench, Viewer, or runtime/control destinations.
3. `SH03` Shell Overview Module Contract. Depends: `SH01`. Done gate: overview modules, summary sources, and routing rules are defined without flattening ownership.
4. `SH04` Shell Ambient Status / Attention Contract. Depends: `SH01`, `SH03`. Done gate: runtime warnings, trust state, and background task surfacing are distinct from domain truth and have explicit return-context rules.
5. `SH05` Shell Diagnostics / Routing Evidence Pack. Depends: `SH02`, `SH04`. Done gate: failed handoff, blocked route, and interruption-return paths emit diagnosable evidence.
6. `SH06` Shell Milestone Closure Receipt. Depends: `SH01`-`SH05`. Done gate: one closure doc states what Shell host behavior is canonical and what downstream lanes can safely assume.

---

## Practical Execution Worklist

This section translates the Wave 1 backlog into the concrete Shell workstreams most likely to
produce visible architectural progress without blurring domain ownership.

### Workstream A — Command / Omnibar Seam

Primary backlog IDs: `SH02`, `SH05`

Execution plan: `2026-04-03_shell_command_bar_execution_plan.md`

Purpose:

- make Shell command entry points honest about what they own
- unify omnibar input, command palette dispatch, and focused-surface target resolution
- stop legacy or embedder paths from bypassing Graphshell command authority

Concrete slices:

1. Define the `CommandBar` surface as Shell-owned input/dispatch UI that consumes Navigator
     breadcrumb/context projection without owning that projection.
2. Land explicit focused-surface targeting for command-bar controls so per-pane viewer actions are
     resolved from a first-class Shell input rather than whichever subsystem rendered last.
3. Audit legacy context-menu / new-tab / open-in-new-view flows and reroute them through the
     Graphshell command authority or document them as explicit bridged exceptions.
4. Ensure background suggestion/search providers feed the omnibar through a Shell-owned mailbox
     rather than detached toolbar threads.
5. Close the companion observability and AT receipts required to prove command-surface routing and focus return rather than leaving those checks implicit in Shell-only wording.

Done shape:

- omnibar input, palette dispatch, and focused-target resolution have one canonical Shell seam
- blocked or legacy-bypassed command routes emit diagnosable evidence
- Workstream A can point at shared provenance, semantic, and AT receipts instead of treating them as undocumented follow-on work

### Workstream B — App Chrome And Ambient Status

Primary backlog IDs: `SH01`, `SH04`

Purpose:

- make the Shell visibly present as the host layer
- separate system-facing status/chrome from Navigator content navigation

Concrete slices:

1. Harden the `CommandBar` / `StatusBar` slot model from `shell_composition_model_spec.md` as the
     canonical top-level shell chrome.
2. Define what ambient status belongs in persistent Shell chrome: sync state, background jobs,
     trust/security warnings, worker/process indicators, and current interruption-return anchor.
3. Keep content-navigation affordances out of Shell chrome unless they are truly system-oriented.
4. Define attention severity/order so warnings, trust issues, and background activity do not render
     as one flat undifferentiated strip.

Done shape:

- persistent Shell chrome has explicit authority and content rules
- ambient status is legible as system/runtime truth, not graph or workbench truth

Current checkpoint as of `2026-04-04`:

- `shell_status_bar` is now a real Shell slot rather than latent layout metadata.
- sync, interruption-return context, and diagnostics attention are surfaced as ambient Shell chrome
  instead of being mixed into command-entry controls.
- ambient attention ordering now admits both analyzer alerts and direct runtime/channel risk
  signals such as navigation violations and compositor fallback pressure.
- settings-route ingress in `shell/desktop/ui/gui.rs` now routes through a dedicated Shell helper
  that preserves `prefer_overlay` while reusing canonical open-decision diagnostics.
- workbench-host surface navigation and overview-plane surface navigation now converge on
  `WorkbenchIntent` for focus/open/transfer/toggle actions instead of each surface pushing its own
  direct reducer path.
- remaining workbench-host frame-layout and navigator-specialty reducer hops now enqueue
     `WorkbenchIntent` as well, leaving direct setter mutation as the primary unresolved host-action
     routing seam.
- workbench-host layout/policy setters for pin state, draft constraint edits, host scope,
     first-use policy, and session-only suppression now also enqueue `WorkbenchIntent`, narrowing the
     remaining host-action exceptions to persistence/request helpers.
- workbench-host rename/delete/save/restore/prune actions now also route through shared
     `WorkbenchIntent`, so `apply_workbench_host_action(...)` no longer performs direct persistence or
     request dispatch itself.
- workbench-host and toolbar pin/unpin controls now emit shared frame intents instead of calling
     persistence helpers directly from the view layer.
- toolbar Navigator view tabs now route graph-view focus through `WorkbenchIntent` instead of
     writing directly to the raw graph-intent buffer.
- toolbar Overview toggle now routes through `WorkbenchIntent::ToggleOverviewPlane`, matching the
     workbench host and overview plane surfaces.
- omnibar provider suggestions already run through `ControlPanel::spawn_blocking_host_request(...)`
     and a frame-bound mailbox with diagnostics, so the first Workstream D audit did not expose a
     new shell-visible detached-worker bug to fix.
- `gui.rs` now centralizes its long-lived lifecycle/registry signal bridges behind a typed
     `GuiFrameInbox`, making frame-bound drain semantics explicit without collapsing those relays
     into a generic request/result mailbox.

Next bypass seam after the current checkpoint:

1. Shell chrome routing surfaces are now substantially converged: workbench-host chrome, toolbar
     frame controls, toolbar Navigator view tabs, and overview-plane surface actions all use the
     shared `WorkbenchIntent` path for frame requests and surface routing. The next routing audit
     should only reopen if a newly added Shell surface bypasses that contract.
2. Overview-plane graph-view-slot edits in `shell/desktop/ui/overview_plane.rs` remain explicitly
     Graph-owned layout mutations. Any future overview interaction that changes surface routing,
     focus, or pane activation should stay on the `WorkbenchIntent` path instead.
3. Workstream D should now focus on proving the host-thread/mailbox boundary rather than chasing
     already-routed chrome actions: the remaining Shell-facing async bridges are the frame-bound
     signal relays in `shell/desktop/ui/gui.rs`, which should either stay documented as intentional
     channel bridges or be folded into a shared mailbox abstraction if a stronger contract is needed.
4. Future Shell-facing async subscriptions should prefer the typed frame-inbox/signal-relay-set
     pattern when they are long-lived and frame-drained; request/result mailboxes should remain the
     default only for one-shot background jobs such as omnibar provider suggestion fetches.

### Workstream C — Overview Surface

Primary backlog IDs: `SH03`, `SHS02`

Purpose:

- give Shell a concrete cross-domain summary surface
- make reorientation routes explicit without flattening Graph, Navigator, Workbench, Viewer, and
  Shell/runtime into one blob

Concrete slices:

1. Land the six-module overview structure from `shell_overview_surface_spec.md` in priority order:
     Active Context strip, Graph Context, Workbench Context, Viewer/Content, Runtime/Attention,
     Suggested Next Actions.
2. Define compact and standard modes first; keep diagnostic mode as an extension if the module data
     sources are not yet stable.
3. Make every overview action route to the owning domain rather than mutating state directly in
     Shell.
4. Add explicit `DI05` acceptance evidence for overview-to-domain handoff behavior.

Current status:

- first standard-mode slice landed: the overview plane now builds Active Context, Graph Context,
     Workbench Context, Viewer/Content, Runtime/Attention, and Suggested Next Actions from live
     `GraphBrowserApp` state plus `WorkbenchChromeProjection::from_tree(...)`, while keeping
     graph-view slot create/rename/move/archive/restore controls in the graph-owned manager below
     the summary cards
- per-card action affordances now route through explicit owning-domain Graph or Workbench paths,
     with focused `DI05` evidence tests covering graph-card, viewer-card, and runtime-card routing
- compact mode now reuses the same live overview summary model inside the Navigator host via a
     compact context/runtime chip bar above the existing mini-grid and region list
- overview domain summaries now distinguish warm vs cold active-graphlet members, expose
     frontier-ready cold peers, and report semantic-tab linked vs detached workbench binding in
     both standard cards and compact chips
- viewer/content summaries now expose effective backend, override-vs-auto selection, placeholder
     fallback reasons, and runtime blocked/crashed state, with a viewer-owned diagnostics route
     when the active pane is degraded
- focused `DI05` evidence now also covers an integrated overview reorientation scenario spanning
     graph, workbench, viewer fallback, runtime attention, and compact-chip surfacing
- Workstream C can close here unless broader end-to-end overview UI rendering evidence is needed

Done shape:

- Shell overview exists as a real host-owned summary surface
- overview cards/chips reorient into the correct owning domain predictably and diagnosably

### Workstream D — Host-Thread And Mailbox Cleanup

Primary backlog IDs: `SH04`, `SH05`, `SHS03`

Purpose:

- make the Shell host-thread boundary real in code instead of just architectural prose
- route user-visible background results through supervised runtime channels

Concrete slices:

1. Audit Shell-facing UI code for ad hoc detached background work launched from toolbar, omnibar,
     or top-level chrome paths.
2. Move those tasks under `ControlPanel`/Register supervision where they represent real background
     work.
3. Define mailbox/result carriers for one-shot Shell-owned background requests, and use a typed
     frame inbox / signal relay set for long-lived subscription bridges that Shell drains only at
     frame boundaries.
4. Emit diagnostics for failed handoff, stale mailbox delivery, and interruption-return routing.

Current status:

- first host-thread cleanup slice landed: `Gui::new` no longer spawns raw signal subscription
     tasks for Shell-facing frame relays; `GuiFrameInbox::spawn(...)` now installs those relays
     under `ControlPanel` supervision with an explicit Shell signal-relay worker tier while
     preserving frame-bound drain semantics
- one-shot Shell-owned provider fetches now return through a typed `HostRequestMailbox<T>`
     carrier from `ControlPanel`, so the omnibar no longer stores or polls raw host-request
     receivers directly on the frame thread
- no additional user-visible one-shot Shell-owned background requests currently bypass typed
     mailbox/result carriers; remaining Workstream D follow-up should target interruption-return
     evidence (`SHS03` / `DI06`) rather than more mailbox conversion
- diagnostics-pane routing now participates in the same tool-surface return capture/restore path
     as settings/history, giving Shell-owned evidence that an interruption surface can open and
     return to the prior graph/workbench anchor without losing domain context

Done shape:

- Shell-owned visible state is frame-thread authoritative
- background work no longer mutates shell-visible UI state directly
- interruption and return-context paths have evidence instead of hand-waving

---

## Suggested Order

Recommended practical execution order:

1. Workstream A — Command / Omnibar Seam
2. Workstream B — App Chrome And Ambient Status
3. Workstream C — Overview Surface
4. Workstream D — Host-Thread And Mailbox Cleanup

Reasoning:

- A establishes the Shell's most important user-facing authority boundary.
- B makes the host layer visible and removes chrome-scope confusion.
- C becomes much easier once command routing and chrome slots are stable.
- D should be applied against the concrete host seams from A/B rather than as an abstract cleanup.

---

## Scenario Track

- `SHS01` `DI03` Graphlet-to-Workbench handoff. Depends: `SH02`, `SH03`. Done gate: Shell can route `open in workbench` from graphlet context to Navigator + Workbench without creating arrangement truth itself.
- `SHS02` `DI05` Shell overview reorientation. Depends: `SH03`. Done gate: overview summary chips/cards route to the correct owning domain and preserve domain-specific ownership semantics.
- `SHS03` `DI06` Runtime/trust interruption return path. Depends: `SH04`, `SH05`. Done gate: interruption handling preserves graphlet/workbench return context and exposes diagnostic evidence.
