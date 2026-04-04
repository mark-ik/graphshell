<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# SHELL — Domain Spec

**Date**: 2026-03-25
**Status**: Canonical / Active
**Scope**: Shell as the system-oriented command interpretation and control
domain, the application's only host, and the orchestration boundary for user
intent and app-level control.

**Related**:

- [shell_backlog_pack.md](shell_backlog_pack.md) — dependency-ordered execution pack and practical worklist for Shell host adoption
- [2026-04-03_shell_command_bar_execution_plan.md](2026-04-03_shell_command_bar_execution_plan.md) — concrete Workstream A execution lane for command-bar authority, omnibar session state, focused-target routing, and legacy bypass cleanup
- [shell_composition_model_spec.md](shell_composition_model_spec.md) — concrete composition model: ShellLayout named slots, egui_tiles scoping, graph canvas hosting contexts, NavigatorContextProjection / omnibar seam
- [../aspect_control/ASPECT_CONTROL.md](../aspect_control/ASPECT_CONTROL.md) — Control aspect (settings, history, diagnostics surfaces); Shell is the domain that hosts and exposes these
- [../aspect_control/settings_and_control_surfaces_spec.md](../aspect_control/settings_and_control_surfaces_spec.md) — canonical settings/control surface contract
- [../aspect_command/command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md) — command palette and command dispatch
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — Navigator domain (relationship projection and quick navigation)
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — Workbench domain (arrangement and activation authority)
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph domain spec; the canvas is its primary graph-rendering surface (graph truth, analysis, and management workspace)
- [../../technical_architecture/unified_view_model.md](../../technical_architecture/unified_view_model.md) — unified view model (Shell absorbs the "Chrome" layer defined there)
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical term definitions

---

## 1. What the Shell Is

The Shell is the **system face** of Graphshell and the application's only host.

It is a command interpreter: it translates user intent into operations dispatched
to the correct authority. It is the surface where aspects, subsystems, runtime
coordination concerns, and app-level overview concerns meet the user. Settings,
preferences, commands, filters, configuration, ambient status, and top-level
surface composition are Shell responsibilities.

The name is not incidental. A shell, in the computing tradition, is the layer
between the user and the system's internal machinery. Graphshell is named for
this layer. The Shell domain makes that relationship explicit.

The precise rule is:

- the Shell is the orchestration boundary for user intent and app-level control,
- the Shell is not the semantic owner of graph truth, pane truth, or content truth.

---

## 2. Why the Shell Is Its Own Domain

The unified view model (`unified_view_model.md §1`) previously grouped the
Shell's responsibilities under an undifferentiated "Chrome" layer:

```
CHROME
  Omnibar, workbar, commands, node creation, navigation, scope changes
```

That framing was adequate when chrome was a thin addressing layer, but it
conflated two fundamentally different orientations:

- **Inward / system-oriented**: settings, preferences, aspect configuration,
  subsystem control, command interpretation, the control panel, the signal bus,
  the register — the connective tissue of the app
- **Outward / UX-oriented**: tile tab bars, graphlet switching, pane promotion,
  graph bars, relationship projection — how the user navigates content

The Navigator domain already owns the outward orientation. The Shell domain owns
the inward orientation. Collapsing both into "Chrome" obscures this boundary and
produces the kind of scope conflation documented in
`chrome_scope_split_plan.md §1`: controls from three distinct semantic scopes
rendered as one flat bar with no durable semantic identity.

The Shell makes the split explicit: it is the domain where the application's
internal systems present themselves to the user for observation and control.

---

## 3. What the Shell Owns

The Shell domain owns:

- **Command interpretation** — the omnibar's input/dispatch side, the command
  palette, keyboard command routing, and the translation of user intent into
  graph intents, workbench intents, or runtime effects
- **Top-level mounting/composition** — the app-level placement and exposure of
  graph surfaces, Navigator hosts, Workbench surfaces, and Shell-owned control
  or status surfaces
- **Aspect exposure** — the UI surfaces through which aspects (render, control,
  command, input) present their configurable state to the user
- **Subsystem control surfaces** — settings pages, history manager, diagnostics
  tool surfaces, import/persistence pages, and other app-owned control pages
  (inherits the scope of `ASPECT_CONTROL.md §2`)
- **Preferences and configuration** — global, per-graph, and per-view settings;
  physics presets as cross-domain policy; profile management
- **The register and control panel face** — the user-facing projection of
  runtime coordination (worker status, intent ingress, background task
  visibility)
- **App-scope chrome** — the persistent top-level bar housing settings access,
  command entry, and ambient system status (sync state, process indicators)
- **Shell overview surfaces** — app-level summaries that relate graph truth,
  workbench truth, and runtime/shell truth without collapsing them into one
  abstraction

---

## 4. What the Shell Does Not Own

The Shell explicitly does not own:

- **Graph truth** — node identity, edges, addresses, topology; owned by the
  Graph domain
- **Graph projection and navigation** — graphlets, relationship projection,
  section model, breadcrumb/context semantics, scoped search, and specialty
  graph navigation layouts; owned by Navigator
- **Graph-view slot layout truth** — creating, naming, positioning, archiving,
  and restoring graph-view slots in the graph layout manager; owned by Graph
  even when exposed through a Shell overview surface
- **Arrangement and activation** — tile tree, frame layout, pane lifecycle,
  routing; owned by Workbench
- **Content rendering** — viewer selection, render mode, content display; owned
  by Viewer
- **Aspect internals** — the Shell exposes aspect configuration to the user; it
  does not own the aspect's runtime logic. The render aspect still owns
  compositing; the command aspect still owns dispatch semantics; the Shell is
  the surface, not the engine

When the Shell accepts a user command (typed in the omnibar, invoked via
keyboard, selected from the command palette), it emits the appropriate intent
to the authority that owns execution. The Shell does not directly mutate graph
state, workbench arrangement, or runtime internals.

---

## 5. The Five-Domain Model

These five domains form the coherent application model:

| Domain | Is | Owns | Does Not Own |
|--------|----|------|--------------|
| **Shell** | Host + app-level control | command dispatch, top-level composition, settings surfaces, subsystem control, app-scope chrome | graph truth, arrangement, projection rules, content rendering |
| **Graph** | Truth + analysis + management | node identity, relations, topology, graph-space interaction, algorithmic analysis | where or how nodes are arranged in the workbench |
| **Navigator** | Projection + navigation | graphlet derivation, projection rules, section model, interaction contract, scoped search, relationship display | node identity, arrangement structure, system settings |
| **Workbench** | Arrangement + activation | tile tree, frame layout, pane lifecycle, routing, split geometry | what a node is or what its graph relations mean |
| **Viewer** | Realization | backend selection, fallback policy, render strategy, content-specific interaction | graph truth, arrangement, command/control routing |

A node is one durable object. All five domains agree on what that object is.
Graph stores it and lets you manage its relationships. Navigator turns shared
truth into navigable local worlds. Workbench hosts detailed work. Viewer
realizes requested facets. Shell lets you configure and command the system that
makes the others possible.

---

## 6. Relationship to the Navigator

The Shell and Navigator share chrome real estate but serve different
orientations.

| Concern | Shell | Navigator |
|---------|-------|-----------|
| Omnibar input / command entry | Owns | Does not own |
| Omnibar graph-position breadcrumb | Does not own | Owns (projection of containment ancestry) |
| Command palette | Owns | Does not own |
| Settings / preferences access | Owns | Does not own |
| Tile tab bars / tile group switching | Does not own | Owns |
| Pane promotion UI | Does not own | Owns |
| Graph bar / graph view hosting | Does not own | Owns (Navigator host with graph scope) |
| Find-in-page | Owns (command) | Does not own |
| Content zoom level | Owns (setting/preference) | Does not own |
| Downloads manager | Owns (subsystem surface) | Does not own |
| Media mute | Owns (control action) | Does not own |
| Sync status indicator | Owns (ambient system status) | Does not own |
| Security / trust summary | Does not own | Owns (per-node projection; see NAVIGATOR.md §11A) |

The omnibar is the natural seam: the Shell owns command interpretation and input
dispatch; the Navigator owns the contextual breadcrumb and graph-position
display within the same visual element.

---

## 7. Relationship to the Control Aspect

The existing Control aspect (`ASPECT_CONTROL.md`) defines settings surfaces,
history pages, diagnostics pages, and control pages as app-owned surfaces. The
Shell domain does not replace or absorb the Control aspect — it is the domain
that **hosts and contextualizes** it.

The distinction:

- **Control aspect** = the runtime system that manages settings state, history
  query, diagnostics collection, and apply/revert semantics inside control pages
- **Shell domain** = the architectural authority that determines where and how
  those control surfaces are exposed to the user, how they are reached (command
  palette, menu, keyboard shortcut, omnibar entry), and how they compose with
  other Shell-owned chrome

This is the same relationship pattern as Canvas/Viewer: the Canvas identifies
what should be shown; the Viewer determines how it is rendered. The Shell
identifies what system surfaces should be accessible; the Control aspect
determines how settings are stored and applied.

---

## 8. Bridges to Other Domains

### 8.1 Shell -> Graph bridge

Used when user commands target graph truth.

Examples:

- create node (omnibar entry, command palette)
- delete node / clear data
- tag, retag, rewire operations issued from command surfaces
- create, rename, move, archive, or restore graph-view slots from the Shell
  overview plane

The Shell dispatches graph intents. The Graph domain executes them.

### 8.2 Shell -> Workbench bridge

Used when user commands target arrangement.

Examples:

- open settings page in a pane
- switch frame
- toggle pinned workbench host
- focus/open/transfer graph views from the Shell overview plane

The Shell dispatches workbench intents. The Workbench executes them.

### 8.3 Shell -> Navigator bridge

Used when Shell actions need Navigator projection updates.

Examples:

- scope change commands that affect Navigator projection
- omnibar navigation that the Navigator should reflect
- graphlet transition commands such as corridor, component, or frontier views

The Shell emits scope or navigation intents. The Navigator updates its
projection accordingly.

### 8.4 Aspect -> Shell bridge

Used when aspects need to surface configuration or status to the user.

Examples:

- render aspect exposes compositor diagnostics
- command aspect registers available commands for palette display
- control aspect provides settings page routes

Aspects register their surfaces and configuration points with the Shell. The
Shell determines presentation context and routing.

---

### 8.5 Shell host execution boundary

Shell is not only the composition host; it is also the **host-thread
authority** for frame-bound user-visible state.

The required split is:

- **Shell host thread / frame loop**:
  top-level panel composition, focus resolution, command-bar rendering, input
  dispatch, accessibility tree publication, UX-tree projection publication, and
  ingestion of background results into user-visible state
- **Register / ControlPanel supervised tasks**:
  network requests, protocol probes, indexing, sync workers, provider
  suggestion fetches, and other background/runtime work
- **Boundary rule**:
  background work returns through explicit mailboxes or intent/signal channels
  that Shell drains at frame boundaries; background tasks do not directly
  mutate Shell-owned UI state

This means accessibility, UX-tree, and top-level diagnostics presentation
remain frame-projected host concerns even when their data sources depend on
background runtime activity. They are not independent UI threads.

Short-lived UI-triggered background work still counts as background work. A
provider-suggestion fetch initiated by the omnibar should run under
`ControlPanel` supervision and return a mailbox result to Shell, not launch an
ad hoc detached thread from toolbar code.

The only acceptable exceptions are **pre-host bootstrap** tasks that occur
before the Shell host/runtime boundary exists at all (for example, startup
initialization done before `Gui`/Shell is live). Those exceptions should remain
rare and explicitly documented.

One explicit post-host exception is automation/embedding bridges such as
WebDriver. Those host-side bridges may address concrete `WebView` instances and
load/traversal state directly instead of routing through Shell command
interpretation, but they remain host-runtime concerns rather than domain truth.
They must stay explicitly documented, diagnosable, and semantically outside the
Shell's user-facing command authority.

---

## 9. Architectural Rules

- The Shell must never derive system truth from UI state. It reads from
  authoritative sources (graph domain, runtime state, aspect registries) and
  projects them into user-facing control surfaces.
- Command dispatch is always intent-based. The Shell emits intents; it does not
  directly mutate state owned by other domains.
- The Shell must not become a semantic god-object. It may orchestrate cross-domain
  behavior without becoming the owner of the underlying domain meaning.
- Settings surfaces are nodes, not dialogs (`settings_and_control_surfaces_spec.md §2.2`).
  Internal routes (`verso://settings/...`) are page-backed, pane-composable app
  surfaces hosted through the Workbench.
- The Shell must not conflate system-oriented controls with content-oriented
  navigation. If a control helps the user navigate content relationships, it
  belongs to the Navigator. If it helps the user configure or command the
  system, it belongs to the Shell.

---

## 10. Practical Reading

If a behavior answers:

- how does the user issue commands to the system,
- how does the user configure the app's behavior,
- how are aspects and subsystems exposed for user observation and control,
- where is the connective tissue between the system's internal machinery and
  the user's intent,

it belongs primarily to the **Shell**.
