<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# NAVIGATOR — Domain Spec

**Date**: 2026-03-25
**Status**: Canonical / Active
**Scope**: Navigator as a first-class domain with its own authority boundary,
projection rules, and interaction contract.

**Related**:

- [navigator_backlog_pack.md](navigator_backlog_pack.md) — dependency-ordered implementation backlog
- [navigator_interaction_contract.md](navigator_interaction_contract.md) — click grammar, selection, reveal, dismiss
- [navigator_projection_spec.md](navigator_projection_spec.md) — canonical five-stage projection pipeline, composition, annotation, portal, and refresh contract
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — workbench domain (arrangement and activation authority)
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph domain spec; the canvas is a graph-rendering surface for truth and context authority
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical term definitions

---

## 1. What the Navigator Is

The Navigator is a **projection and navigation domain**.

It reads from graph truth and workbench arrangement state. It does not own
either. It turns graph truth into navigable local worlds: graphlets, paths,
components, loop views, scoped search results, breadcrumbs, sectioned context,
and other purpose-driven projections. It routes user interactions back to the
correct authority.

The Navigator is not a second data model. It is not a sidebar version of the
Workbench. It is the domain that answers: what local world is the user
traversing right now?

---

## 2. Why the Navigator Is Its Own Domain

The Navigator was previously documented under the Workbench domain. That was
accurate when the sidebar was primarily a tree of workbench arrangement state
(frames, tiles, panes). It is no longer accurate.

The Navigator's job now spans:

- **graph truth** — node identity, relations, provenance, tags, recency
- **workbench arrangement state** — which nodes are open, in which frames,
  in which tiles
- **projection logic** — what gets shown, in what sections, in what order
- **interaction routing** — how clicks, selections, and commands route back
  to the correct authority

No single existing domain owns all four of those. Placing the Navigator spec
under Workbench misleads: it implies the Navigator's source of truth is
arrangement state, when in fact it reads from both graph truth and arrangement
state and must never conflate them.

The Navigator therefore gets its own domain directory and canonical spec.

---

## 3. What the Navigator Owns

The Navigator domain owns:

- **Graphlet derivation** — deriving local graph worlds from anchors, filters,
  graph algorithms, and traversal context
- **Projection rules** — which objects appear in which sections, under what
  conditions, in what order
- **Section model** — the named sections (Recent, Frames, Graph, Relations,
  etc.) and their projection sources
- **Purpose-driven specialty layouts** — radial, path/corridor, component,
  atlas, timeline, hierarchical, and other navigation-oriented graph-bearing
  presentations when they serve orientation better than a list or tree
- **Cross-surface verb adaptation** — how shared verbs such as `select`,
  `activate`, `reveal`, and `scope` are expressed in Navigator grammar without
  changing the underlying authority boundary
- **Interaction contract** — click grammar (single-click = select, double-click
  = activate), reveal rules, dismiss routing, command applicability
- **Selection propagation** — how Navigator interactions set and read global
  graph selection truth
- **Expansion/collapse state** — session-scoped per-row expansion state
  (not persisted as graph truth)
- **Filter/search model** — local filter semantics that do not mutate
  underlying truth
- **Context-aware search** — graphlet-scoped and relation-aware search behavior
  that uses graph context rather than raw global text matching alone
- **Shared chrome security projection** — focused-node trust and origin
  permission summaries rendered in Navigator chrome from security/runtime truth
- **Refresh triggers** — which graph and workbench state changes cause the
  Navigator projection to rebuild or update

---

## 4. What the Navigator Does Not Own

The Navigator explicitly does not own:

- **Node identity or graph structure** — owned by graph truth (graph domain)
- **Node lifecycle** (active / warm / cold) — owned by the runtime lifecycle
  subsystem
- **Tile tree structure or frame layout** — owned by workbench arrangement
- **Surface arrangement** — host placement, edge anchoring, and split geometry
  are workbench layout state, not Navigator projection state
- **Which node is open in which pane** — owned by workbench session state
- **Routing decisions** (which pane to open a node in) — owned by workbench
  routing
- **Durable graph mutation** — creating, deleting, or rewiring graph truth is
  graph authority, not Navigator authority
- **Promotion of derived graphlets into durable graph truth** — Navigator may
  request or suggest this; Graph owns whether it becomes truth
- **Persist/delete operations** — the Navigator may route these intents;
  it does not execute them

When the Navigator initiates an action (select, activate, dismiss), it emits
a graph intent or workbench intent and trusts the relevant authority to execute
it. The Navigator does not directly mutate graph state or workbench arrangement
state.

**Uphill rule**: any change initiated from a Navigator projection — host row,
graphlet view, swatch canvas instance, specialty layout — goes uphill to the
relevant authority and is presumed ephemeral until that authority promotes it.
Authority routing by intent kind:

- node identity, edges, relations, durable graph mutation → **Graph**
- frame composition, frame switching, frame snapshot persistence → **Shell**
- tile tree mutation, pane lifecycle, split geometry → **Workbench**
- node lifecycle state, scheduling, warm/cold transitions → **runtime lifecycle**
- traversal events, recency aggregation → **SUBSYSTEM_HISTORY**

Projection-local hover, scaffold selection, viewport, expansion, and filter
state stay projection-local. Identity, structure, arrangement, and durable
state never do.

---

## 5. The Five-Domain Model

These five domains form the coherent application model:

| Domain | Is | Owns | Does Not Own |
|--------|----|------|--------------|
| **Shell** | Host + app-level control | command dispatch, top-level composition, settings surfaces, subsystem control, app-scope chrome | graph truth, arrangement, projection rules, content rendering |
| **Graph** | Truth + analysis + management | node identity, relations, provenance, durable state, algorithmic analysis, graph enrichment | where or how nodes are arranged in the workbench |
| **Navigator** | Projection + navigation | graphlet derivation, projection rules, interaction contract, selection propagation, scoped search, specialty navigation layouts | node identity, arrangement structure, system settings |
| **Workbench** | Arrangement + activation | tile tree, frame layout, pane lifecycle, routing | what a node is or what its graph relations mean |
| **Viewer** | Realization | backend selection, fallback policy, render strategy, content-specific interaction | graph truth, arrangement structure, system settings |

A node is one durable object. All five domains agree on what that object is.
Graph stores it and lets you manage its relationships. Navigator derives the
current navigable local world. Workbench hosts detailed work. Viewer realizes
requested facets. Shell exposes and orchestrates the system that makes the
others possible.

See `../shell/SHELL.md` for the Shell domain spec and
`../graph/GRAPH.md §2.2` for the Graph interactive management workspace and its canvas surface.

---

## 6. Cross-Surface Verb Mapping

The unified view model is easiest to keep consistent when the major user verbs
are named once and then mapped into each surface.

| Verb | Navigator expression | Authority |
|--------|----|------|
| `select` | row selection, keyboard move, single-click | shared graph selection truth |
| `activate` | double-click row, Enter on selected node, open command | workbench routing / activation |
| `reveal` | scroll row into view, ask graph view to reveal selected node | projection-local effect; may require workbench host activation |
| `scope` | local filter, section switch, explicit graph/workbench scope host setting | projection state |
| `arrange` | host placement, sidebar edge, frame/tile geometry | workbench layout authority |
| `mutate` | create/delete/retag/rewire node truth | graph mutation authority |

The practical rule is simple: Navigator may *express* all of these verbs in UI,
but it only *owns* projection-local aspects of `reveal` and `scope`. The rest
are routed to the authority that owns them.

---

## 7. Interaction Invariants

These invariants hold across graph canvas, workbench, and navigator. If any
surface violates one, it is a bug, not a design choice.

### I1 — Identity invariant

A node is one object with one identity. Selecting, activating, or dismissing
it on any surface targets the same underlying node. There is no "graph copy"
and "navigator copy."

### I2 — State separation invariant

Existence, visibility, selection, and activation are four distinct states.
None implies another except by explicit intent:

- A node can exist without being visible in the current graph view.
- It can be visible without being selected.
- It can be selected without being activated (open in a pane).
- It can be activated without being the primary selection.

### I3 — Selection propagation invariant

Selecting a node on any surface sets graph selection truth. Surfaces project
that truth — they do not own their own selection copy. If the Navigator and
the graph canvas show different nodes as selected, one of them is wrong.

### I4 — Click grammar invariant

Across all surfaces:

- **Single-click** = select / give focus
- **Double-click** = activate / open
- This applies to node rows in the Navigator, node objects on the graph canvas,
  and node tabs in workbench tiles.
- Structural rows (Frame, Tile, Section) are not nodes; single-click
  expands/collapses them.

### I5 — Reveal invariant

Reveal is a local orientation effect, not a second form of selection truth.
It may be triggered by selection or by an explicit "reveal" command, but it
never changes graph identity by itself.

Examples:

- Navigator reveal = scroll the relevant row into view or expand enough local
  structure to show it
- Graph reveal = move the viewport enough to show the selected node
- Workbench reveal = foreground the pane or host that already contains the node

Reveal does not change selection, activation, or graph structure.

### I6 — Scope invariant

Navigator-local filters, collapsed sections, and temporary neighborhood
projections are scope state, not graph truth.

Only explicit scope handoff should cross surfaces. Ad hoc local filtering in one
Navigator host must not silently rewrite the scope of every other graph surface.

### I7 — Dismiss / delete invariant

- **Dismiss** = remove a node from its current surface context (tile, frame,
  view). The node still exists. Dismiss is recoverable.
- **Delete** = remove the node from graph truth. The node no longer exists.
  Delete is not recoverable without undo.

The Navigator may offer dismiss actions for nodes in arrangement contexts.
It must not offer delete as an equivalent or fallback to dismiss.

### I8 — Command applicability invariant

A command is available only if it validly applies to every object in the
current selection set. The Navigator must not silently narrow the target
to a subset or fall back to a single implicit primary target.

---

## 8. Presentation Bucket Model (Canonical)

The Navigator composes three canonical **Presentation Buckets**. Specific named
projections (recency, frametree, ego graphlet, frontier, relation family,
import event stream, etc.) are recipes that land in one of the three buckets.
Bucket membership is always derived; the Navigator never stores it as its own
truth.

| Bucket | What it provides | Projection sources | Example recipes |
|--------|------------------|--------------------|-----------------|
| **Tree Spine** | Orientation. Scannable hierarchy with active-node location, expand/collapse, badges, cross-edge indicators. | Graph (containment, traversal, lens-driven), Shell (frametree), Navigator (graphlet sections) | containment tree, traversal spine, frametree, graphlet sections, cycle/bridge/frontier badges |
| **Swatches** | Shape analysis. Compact navigator-scoped canvas instances applying graph capacities (filter, layout, scene representation, simulation) to scoped projections of graph truth. | Graph (truth), Graph Cartography (aggregates), Navigator (recipe selection) | ego graphlet, corridor, frontier, bridge, semantic cluster, workbench correspondence, domain cluster, active session map, graph overview |
| **Activity Log** | Temporal analysis. Distinguishes active / recently-active-now-inactive / warmed / cold-but-relevant nodes; surfaces events. | SUBSYSTEM_HISTORY, runtime lifecycle, Graph (mutation events), Shell (import events) | recency lane, lifecycle transitions, navigation transitions, graph mutation log, import event stream, memory branch changes |

The bucket names describe the *presentation shape*. The five legacy section
names (Recent, Frames, Graph, Relations, Import Records) were a flat catalog
that conflated shape with source — they map cleanly into buckets:

- *Recent* → Activity Log (recency lane)
- *Frames* → Tree Spine (frametree recipe; Shell-owned composition surfaced in
  the spine)
- *Graph* → Swatches (overview/all-nodes recipe) and/or Tree Spine (containment
  lens recipe), per host configuration
- *Relations* → Swatches (relation-family graphlet recipes)
- *Import Records* → Activity Log (import event stream)

A Navigator host may render one, two, or all three buckets depending on its
form factor, scope, and available space. Bucket presence is layout policy; the
bucket model itself is canonical.

---

## 8A. Graphlets And Specialty Presentations

Navigator graphlets are first-class navigation objects.

Useful graphlet forms include:

- ego graphlets,
- corridor/path graphlets,
- component graphlets,
- loop/SCC graphlets,
- frontier graphlets,
- facet-filtered graphlets,
- session graphlets,
- bridge graphlets,
- Workbench-correspondence graphlets.

Navigator may present those graphlets through different local forms:

- tree or list when hierarchy is clearer than space,
- radial or corridor graph layouts when spatial relation is the point,
- timeline or hierarchical layouts when traversal or dependency structure is the point,
- atlas/component views when shell- or session-level overview is needed.

This is why Navigator is not thin. It owns navigation-oriented projection semantics,
not merely breadcrumb UI.

---

## 9. Projection Sources and Authority

| Navigator reads from | Authority | How |
|---------------------|-----------|-----|
| Node identity, tags, relations | Graph domain | Read from `domain_graph()` |
| Node recency / lifecycle state | Runtime lifecycle | Read from `graph_runtime` state |
| Frame composition / frametree | Shell domain | Read from Shell-owned frame state; Workbench provides per-graph tile trees within each composed frame |
| Frame membership of nodes | Graph (`ArrangementRelation`) | Read graph-backed frame-member edges; rendered into the Tree Spine bucket as the frametree recipe |
| Active tile contents | Workbench session state | Read from tile tree at projection time |
| Import / activity events | SUBSYSTEM_HISTORY + Shell import event stream | Read at projection time; surfaced into the Activity Log bucket |

The Navigator reads these at projection time. It does not cache copies of
graph or workbench state independently — stale projection is diagnosed, not
silently tolerated.

The same rule applies to security and permission exposure. Navigator chrome may
summarize trust and permission state for the focused node or origin, but those
summaries are projections from security/runtime truth, not Navigator-owned
flags.

---

## 10. Refresh Triggers

The Navigator projection rebuilds or updates when:

- A node is added to or removed from the graph
- A node's title, tags, or relations change
- A node's lifecycle state changes (active / warm / cold)
- Frame composition changes (Shell adds/removes/reorders a workbench in the
  frame, switches the active frame, or persists a frame snapshot)
- Frame membership of any node changes (`ArrangementRelation` edge added or
  removed)
- The tile tree changes (pane open, close, move, split)
- An import or activity event is emitted into the Activity Log stream
- The user applies a local filter or search query

Refresh is routed through the shared signal path (`phase3_publish_workbench_projection_refresh_requested`), not through ad hoc observers.

Security and permission chrome must refresh on the same principle: when focused
node trust state changes, when mixed-content state changes, and when origin
permission state changes, the Navigator updates through shared signals rather
than polling or UI-local caches.

---

## 11. Relationship to the Workbench Domain

The Workbench domain previously claimed the "Workbench Sidebar" as its own.
That claim is superseded by this spec.

Updated boundary:

- **Workbench** owns the sidebar's *chrome container* (the layout slot, its
  resize handle, its show/hide toggle) and the routing decisions that happen
  when a Navigator action requires opening a pane.
- **Navigator** owns everything inside the sidebar content area: the sections,
  the rows, the click grammar, the selection propagation, the projection rules.

This is the same relationship as graph canvas / graph domain: the workbench
chrome hosts the graph view slot; the graph domain owns what renders inside it.

The `WORKBENCH.md` sidebar ownership claim is updated to reflect this split.

## 11A. Security and Permission Exposure Contract

The Navigator is the canonical shared chrome surface for node-scoped trust and
origin-scoped permission visibility.

For any focused or selected node backed by remote or embedded web content,
Navigator chrome must expose:

- current transport trust state (`secure`, `degraded`, or `insecure` at minimum)
- an entry point to certificate or identity details when such details exist
- mixed-content or other degraded-origin warnings when present
- per-origin permission state for camera, microphone, location, and
  notifications

These signals are part of the Navigator's projection responsibility because
they help the user decide whether to activate, trust, and interact with the
content represented by the current node. They are not optional embellishments
and must not be hidden behind settings pages or diagnostics-only views.

Scope implications:

- `GraphOnly` and `Both` must show the trust/permission summary whenever a
  node-backed graph context is active.
- `WorkbenchOnly` must still show the focused pane's node trust/permission
  summary because workbench focus does not erase the node's security boundary.
- `Auto` must preserve these summaries across scope switches so the active
  safety state does not disappear merely because focus moved.

The Navigator does not own permission grants or trust evaluation. It projects
them and routes the user to the authority that does.

## 11B. Focused Content Status Contract

Navigator hosts may project focused-content **status**, but they are not the
canonical command surface for page-local viewer controls.

The pane/tile alignment is:

- floating panes stay ephemeral and chromeless except for Promote / Dismiss
- docked tiles keep reduced identity chrome only
- tiled tiles own full tile-local viewer chrome
- Navigator hosts remain structural/context surfaces around that chrome

Accordingly, Navigator hosts may expose focused-content status such as:

- load-state summary
- effective backend / compatibility / degraded-state badge
- media activity summary
- downloads activity summary

These are status projections. They must not become a surrogate viewer toolbar
for Back / Forward / Reload / Find in page / content zoom / compat toggle.

Scope implications:

- `WorkbenchOnly` may surface the focused pane's content status when a pane
  hosts live content, but commands remain tile-local.
- `GraphOnly` and `Both` may continue to surface relevant focused-node status
  summaries when Graphshell can identify the active node-backed content
  surface.
- `Auto` must preserve status visibility across scope switches without moving
  command ownership away from tile-local chrome.

If no focused node-backed content viewer is active, these status badges may
collapse or disappear entirely. Placeholder chrome is not required.

---

## 12. Navigator Scope and Form Factor

**Date**: 2026-03-22

### 11.1 One Navigator, Many Hosts

There is one Navigator projection grammar. It may be rendered through one or
more **Navigator hosts** around the workbench frame.

Each host has four orthogonal settings:

- **Form factor** — how it is presented: `Sidebar` (panel) or `Toolbar` (compact bar)
- **Scope** — what it projects: `Both`, `GraphOnly`, `WorkbenchOnly`, or `Auto`
- **Anchor edge** — where it is mounted: `Top`, `Bottom`, `Left`, or `Right`
- **Cross-axis margins** — adjustable insets from the host's non-anchor edges

These are independent. A sidebar can be graph-only. A toolbar can show both
scopes. Two different hosts may project different scopes at the same time.

Canonical rule:

- Navigator is one semantic surface family
- host count is a layout policy decision
- host settings are persisted per host, not globally

All active Navigator hosts must use the same row grammar, trust/permission
projection rules, focused-content status rules, and selection semantics. Hosts
may differ in scope, form factor, anchor edge, and margin settings only.

### 11.2 Scope Modes

| Scope | Behaviour |
|-------|-----------|
| `Both` | Projects graph truth and workbench arrangement state simultaneously, as named sections. Default. |
| `GraphOnly` | Projects graph truth only. Workbench sections hidden. |
| `WorkbenchOnly` | Projects workbench arrangement state only. Graph sections hidden. |
| `Auto` | Switches between graph scope and workbench scope when focus moves between the graph canvas and a workbench tile. Mirrors keybinding scope switching. |

### 11.3 Deprecating Graph Bar and Workbench Bar as Fixed Surface Types

The previous model had two separate horizontal bars:

- **Graph Bar** — toolbar Navigator, graph scope
- **Workbench Bar** — toolbar Navigator, workbench scope

These were implementation seams exposed as chrome. Under this model they are
replaced by Navigator hosts whose scope setting determines what they show.

Examples that are valid under this host model:

- one top toolbar host showing `Both`
- one left sidebar host showing `Both`
- a top toolbar host showing `GraphOnly` plus a bottom toolbar host showing
  `WorkbenchOnly`
- a left sidebar host showing `GraphOnly` plus a right sidebar host showing
  `WorkbenchOnly`

"Graph Bar" and "Workbench Bar" are therefore retired as surface names. They
may remain as host presets or section labels within the Navigator projection if
that aids orientation.

### 11.4 Host Model

Navigator hosts are edge-mounted. At most one Navigator host may occupy a given
edge. Multiple edges may host Navigator simultaneously.

Valid host sets therefore include:

- one host on any one edge
- two hosts on any two different edges
- three hosts on three different edges
- four hosts, one on each edge

Each host chooses its own form factor:

- `Top` / `Bottom` naturally default to `Toolbar`
- `Left` / `Right` naturally default to `Sidebar`

but this is a default, not a restriction.

If a host is dragged across axes:

- moving a `Toolbar` host from `Top`/`Bottom` to `Left`/`Right` converts it to
  `Sidebar` form by default
- moving a `Sidebar` host from `Left`/`Right` to `Top`/`Bottom` converts it to
  `Toolbar` form by default

This keeps drag behavior aligned with visual expectations while preserving the
host's scope.

### 11.5 Default Host Configuration

The default remains conservative:

- one primary Navigator host is enabled on first run
- that primary host defaults to scope `Both`

Additional hosts are optional and user-enabled.

When a second host is enabled, the natural default assignment is:

- existing host keeps its current scope
- new host receives the complementary scope if one exists

Examples:

- existing `Both` host + new host -> new host also starts as `Both`
- existing `GraphOnly` host + new host -> new host defaults to `WorkbenchOnly`
- existing `WorkbenchOnly` host + new host -> new host defaults to `GraphOnly`

Users may override these defaults freely. Mirrored or redundant hosts are valid
so long as their bounds do not overlap.

### 11.6 Host Persistence

Scope, form factor, anchor edge, enabled state, and cross-axis margins are
persisted in `WorkbenchProfile` per Navigator host. They are part of the layout
policy the user configures and are not reset between sessions.

### 11.7 Graph Overview Swatch Policy

Navigator may host a graph overview swatch when that serves orientation better
than a pure list.

This is the right home for the old "Atlas" idea only in its orientation role.
It is not a second graph-view layout authority.

Ownership split:

- Graph owns graph-view regions, slot layout, and direct structural editing
- Navigator owns compact overview projection, graphlet/context orientation, and routing affordances
- Workbench still owns arrangement/session projection shown alongside those overview affordances
- The compact overview projection may eventually be backed by a reusable `SwatchSpec` shared with other embedded graph-preview surfaces, but Navigator still contributes only host policy and routing semantics, not a second graph model

Host-form guidance:

- `Sidebar` hosts in `GraphOnly` or `Both` scope may render a minimap-like swatch or overview card when there is sufficient space
- spacious `Sidebar` hosts may additionally allow drag-transfer across that swatch when target cells remain reliably sized for gesture use
- the same host may place graph-view lists, graphlet summaries, selected-node context, and workbench-session lists beside that swatch rather than cramming all meaning into one tiny canvas
- `Toolbar` hosts should default to compact chips, strips, counters, or tab-like summaries of graph views and workbench sessions instead of a precision-target minimap

First release guidance:

- the first sidebar-host graph overview should be list-first, not minimap-first
- an optional swatch/overview card may appear only after host-width thresholds are met without crowding graph-view lists, selected-node context, or workbench-session summaries
- archived graph views should be hidden by default in compact overview projections and revealed through an explicit filter toggle
- dense inter-view relationships should appear as aggregated counts or adjacency hints rather than line-level micro-geometry

Critical rule:

- if the user needs to create, move, resize, rename, archive, or directly reorganize graph-view regions, Navigator must route into graph-owned controls rather than pretending the swatch is the full editor

This means an idiomatic graph-domain control such as "zoom out to Overview
Plane" remains important even when Navigator provides a helpful overview.

Current scope guardrail:

- floating/freeform Navigator hosts are not part of the current host model
- if a floating overview is desired later, that is a Shell host-model extension, not a silent expansion of Navigator semantics

### 11.8 Constraint With Layout Policy

Navigator hosting is governed by workbench layout policy. The layout policy
must be able to persist and restore:

- which Navigator hosts are enabled
- which edge each host occupies
- which form factor each host uses
- which scope each host projects
- the host's adjustable cross-axis margins

The scope setting is independent of the anchor edge. A bottom toolbar host
showing `WorkbenchOnly`, or left and right sidebar hosts projecting different
scopes, are both canonical-valid configurations.
