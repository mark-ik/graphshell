# Graph Relation Families

**Date**: 2026-03-14
**Status**: Design — Pre-Implementation
**Purpose**: Define the canonical vocabulary of relation families that extend the
current `EdgeKind` model, enabling typed projection, persistence tiers, and
physics semantics — so the workbench navigator, the graph canvas, and the physics
engine share one coherent model rather than three ad hoc ones.

**Related**:

- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — §6 WorkbenchChromeProjection, §5 sidebar sections
- `graph_node_edge_interaction_spec.md` — §5.2 richer relationship tooling
- `../subsystem_history/edge_traversal_spec.md` — `TraversalDerived` lifecycle and decay rules
- `agent_derived_edges_spec.md` — `AgentDerived` provenance and assertion protocol
- `2026-03-21_edge_family_and_provenance_expansion_plan.md` — follow-on plan for widening edge vocabulary and introducing a dedicated Provenance family
- `../canvas/2026-02-25_progressive_lens_and_physics_binding_plan.md` — physics profiles
- `../../TERMINOLOGY.md` — `EdgeKind`, `EdgePayload`, `Graph`, `Frame`, `TileGroup`
- `../workbench/WORKBENCH.md` — workbench owns arrangement interaction/session mutation truth, not graph meaning
- `../system/register/SYSTEM_REGISTER.md` — register-owned routing / diagnostics carriers
- `../aspect_control/settings_and_control_surfaces_spec.md` — control-surface ownership and settings routing

---

## 1. Problem

The current `EdgeKind` set — `Hyperlink`, `TraversalDerived`, `UserGrouped`,
`AgentDerived` — captures semantic and traversal relations well but is silent on
three increasingly important relation categories:

1. **Arrangement / layout relations** — frames and tiles have historically
   lived outside the graph entirely (in tile-tree snapshots and in-memory
   workbench state). Without graph-backed carriers they cannot be projected by
   the navigator or reasoned about by the physics engine using the same
   semantics as other relation families.

2. **Containment / hierarchy relations** — the file:// URL hierarchy, domain
   membership, URL path prefixes, and user-defined notebooks or folders are all
   "a node belongs under another node" assertions. They currently have no edge
   representation; the FileTree tool pane approximated them with an ad hoc
   containment source enum, not a real graph structure.

3. **Imported / derived external relations** — bookmarks folders, filesystem
   directory structure, and future RSS feed membership need a distinct provenance
   marker so they can be treated as read-mostly derived imports rather than
   first-class user-authored edges.

The insight driving this design: these relation categories are not fundamentally
different from the existing `EdgeKind` kinds — they are all relations between
nodes with different semantics, lifecycle, and projection rules. Unifying them
under one typed family vocabulary lets Navigator hosts render all relation
types legibly, lets the physics engine apply appropriate forces per family, and
lets the persistence layer apply appropriate durability per family.

### 1.1 Shared-carrier consequence

This family model is intentionally not canvas-only. The same relation-family
vocabulary is meant to be reused by:

- the **Navigator** for section ownership and row hierarchy,
- workbench-scoped **Navigator hosts** for arrangement projection,
- the **History** subsystem for recent/traversal projection,
- **filesystem/import** flows for derived hierarchy and imported grouping,
- **lens + physics** policy (`FamilyPhysicsPolicy`) for family-aware layout and
  visibility,
- **settings / diagnostics** surfaces for inspection, toggles, and health
  reporting.

If a subsystem needs a new hierarchy, adjacency list, or grouping surface, it
should first ask whether that behavior can be expressed as a relation family,
its projection rule, or its diagnostics exposure before introducing a second
parallel structure.

### 1.2 View-local edge policy consequence

Relation-family truth and edge presentation are intentionally distinct.

- Underlying edge/relation truth is graph-owned.
- Rendering, suppression, and emphasis are `GraphViewId`-local `EdgePolicy`
  concerns.
- Dismissing an edge removes only that edge instance's presentation/effect in
  the current graph view unless a broader family policy is explicitly changed.
- Copying a graph view clones its `EdgePolicy`, including per-family toggles and
  per-edge dismissal state, so layout and visibility choices survive the copy.

---

## 2. Relation Family Vocabulary

A **relation family** is a named class of edge that shares persistence tier,
visibility rules, deletion behavior, layout influence, and projection priority.

Families extend (do not replace) the existing `EdgeKind` enum. The mapping from
existing kinds to families is explicit in §3.

### 2.1 Semantic Family

**What it captures**: Explicit, user-authored or inferred conceptual relationships
between nodes. The relation is meaningful in and of itself — it represents "these
two things are related."

**Members**:
- `EdgeKind::UserGrouped` — explicit user grouping association
- `EdgeKind::AgentDerived` — inferred by an agent from content similarity,
  co-access, or semantic proximity

**Persistence tier**: Durable. Survives session close and graph reload.
`UserGrouped` is always durable. `AgentDerived` is durable until its decay window
expires (default 72 h, no navigation; see `edge_traversal_spec.md §2.5`).

**Visibility rule**: Always shown in graph canvas by default. Primary target for
"link" and "related" affordances in the navigator.

**Deletion behavior**: Explicit user action required. `AgentDerived` can be
dismissed, creating a suppression record.

**Layout influence**: Medium attractive force. Default physics profile applies
elastic-association semantics (nodes are drawn together but not rigidly).

**Projection precedence in navigator**: Primary. Semantic edges appear as
explicit connection rows and drive the "route to adjacent" action.

### 2.2 Traversal Family

**What it captures**: Navigation history — the temporal trace of which nodes were
visited from which other nodes. Implicitly created during browsing.

**Members**:
- `EdgeKind::TraversalDerived` — browser history traversal

**Persistence tier**: Rolling-window durable. Oldest traversal events are evicted
when a configurable max record count or age is reached. Aggregate `EdgeMetrics`
are retained indefinitely even after event eviction.

**Visibility rule**: Hidden from canvas by default; surfaced in history/timeline
view and in the sidebar history section. Not shown as visible edges in the graph
canvas unless the user explicitly enables a traversal-overlay lens.

**Deletion behavior**: Archive or eviction only; not directly user-deletable per
node pair (only via full history clear).

**Layout influence**: Weak attractive force when traversal lens is active;
zero/negligible otherwise. Avoids polluting default physics with browsing noise.

**Projection precedence in navigator**: Supplementary. Traversal edges appear in
a collapsible "Recent" section of the sidebar and in the History subsystem views.
They do not define navigator tree structure.

### 2.3 Containment Family

**What it captures**: "A is contained within B" — a hierarchical membership
assertion. This is a directed, acyclic, or weakly-acyclic relation. Examples:
a page is "under" a domain; a document is "in" a folder; a clipped node is
"under" its source page; a notebook entry is "in" a notebook.

**Sub-kinds** (all share the Containment family, distinguished by a tag field):

| Sub-kind tag | Created by | Example |
| --- | --- | --- |
| `url-path` | Derived automatically from URL structure | `https://example.com/docs/api` ← `https://example.com/docs` |
| `domain` | Derived automatically from hostname | `https://example.com/a` ← `example.com` |
| `filesystem` | Imported from filesystem ingest | `file:///projects/foo/bar.md` ← `file:///projects/foo/` |
| `user-folder` | Explicit user action ("Add to folder / notebook") | user-defined grouping with a title |
| `clip-source` | Automatic on clip creation | clipped node ← its source page |

**Persistence tier**: Derived containment (`url-path`, `domain`, `filesystem`,
`clip-source`) is derived-readonly — it is recomputed on graph reload from node
URL data and filesystem ingest and is never persisted as durable edges.
User-folder containment is durable (user-authored).

**Visibility rule**: Hidden from canvas by default; surfaced only when a
containment lens is active. The canvas is not a file tree and should not look
like one by default.

**Deletion behavior**: Derived containment cannot be deleted (recomputed each
load). User-folder containment requires explicit user action.

**Layout influence**: Strong rigid-containment force when the containment lens
is active for the relevant sub-kind. Zero influence otherwise. Avoids polluting
force-directed layout with URL hierarchy at rest.

**Projection precedence in navigator**: Navigator tree structure owner. When the
navigator is in containment-projection mode, containment edges define the tree
rows. A node's position in the tree is determined by its highest-priority
containment edge (user-folder > clip-source > url-path > domain > none).

### 2.4 Arrangement Family

**What it captures**: "A is displayed alongside or grouped with B for work" —
presentation and layout membership. Frames and tiles are the primary
carriers. This is the graph-rooted equivalent of what the workbench tile tree
currently stores in memory.

**Carrier note**: Frames and tiles are the preferred first-class carriers
for arrangement relations. They already provide collapsible, metadata-bearing,
multi-node structure and should absorb most "relation pseudonode" use cases for
workbench semantics before a generic hyperedge object is introduced.

**Sub-kinds**:

| Sub-kind tag | Meaning | Created by |
| --- | --- | --- |
| `frame-member` | Nodes belong to a named frame (persistent layout) | User creates a named frame; saving it persists these edges |
| `tile-member` | Nodes share a tile in the current session (solo or multi-node) | Automatic when user opens one or more nodes in a tile |
| `split-pair` | Two nodes appear in adjacent splits | Automatic when user splits a pane |

**Persistence tier**: Split into two tiers:
- **Durable**: `frame-member` edges with a named frame persist across sessions
  when the user explicitly saves the frame. They are the mechanism by which
  "pinning a frame" works — a frame is a named set of arrangement edges.
- **Session-only**: `tile-member` and `split-pair` edges are created automatically
  during the session and evaporate on close unless promoted to durable
  (i.e., unless the user saves the frame they belong to).

This split is crucial: arrangement relations are graph-rooted, but not every
runtime layout wiggle becomes durable graph truth. Promotion into a saved frame
is the explicit bridge from session-only arrangement to durable arrangement.

**Visibility rule**: Hidden from canvas by default. Arrangement is visible in a
workbench-scoped Navigator host, not as canvas edges. Optionally visible as faint
spatial grouping indicators in the canvas when a "workbench overlay" lens is
active.

**Deletion behavior**: Session-only edges evaporate automatically. Durable
`frame-member` edges are deleted when the user removes a node from a frame or
deletes the frame.

**Layout influence**: Local-arrangement semantics — moderate attractive force
within frame members when the arrangement lens is active. The frame acts as a
soft cluster without overriding the global force-directed layout.

**Projection precedence in navigator**: Default navigator tree structure owner
when the workbench is active. In workbench mode, the sidebar tree is organized
by arrangement edges (frames, tiles) first, then falls back to semantic
grouping. Nodes without any arrangement edge appear in an "Uncategorized" section.

### 2.5 Imported Family

**What it captures**: Relations imported from external systems — bookmarks
folders, RSS feed membership, browser history import from another browser,
shared collection links. These are derived at import time and carry their
external provenance explicitly.

**Persistence tier**: Derived-readonly at import time. The import record itself
is stored, but the edges are recomputed from the import record, not stored as
first-class durable edges. Promoted to durable only by explicit user action
("Keep this grouping").

**Visibility rule**: Hidden from canvas. Shown in navigator under a collapsible
"Imported" section, labeled with the import source.

**Deletion behavior**: Delete the import record to remove the edges. Individual
imported edge deletion archives that edge from re-import.

**Layout influence**: None by default. Imported relations do not affect physics.

**Projection precedence in navigator**: Supplementary. Below arrangement,
containment, and semantic in priority.

---

## 3. Mapping to Current EdgeKind

The existing four `EdgeKind` values map to families as follows:

| Existing `EdgeKind` | Relation Family | Notes |
| --- | --- | --- |
| `Hyperlink` | Semantic | Hyperlinks are explicit semantic connections recorded when a user navigates a link |
| `TraversalDerived` | Traversal | Rolling-window history trace; existing decay and eviction rules unchanged |
| `UserGrouped` | Semantic | User-authored grouping; no change to creation or deletion behavior |
| `AgentDerived` | Semantic | Inferred; existing decay and suppression rules unchanged |

No existing `EdgeKind` variants are removed or renamed. New family-carrying
`EdgeKind` variants are additive.

**New EdgeKind variants** (to be added in a future implementation slice):

| New `EdgeKind` | Family | Sub-kind field |
| --- | --- | --- |
| `ContainmentRelation` | Containment | sub-kind tag (string or enum) |
| `ArrangementRelation` | Arrangement | sub-kind tag + frame name |
| `ImportedRelation` | Imported | import source identifier |

Each new variant adds a corresponding data blob to `EdgePayload`, following the
same pattern as `UserGroupedData` and `TraversalData`.

---

## 4. Persistence Tiers

Four tiers govern durability:

| Tier | Meaning | Examples |
| --- | --- | --- |
| **Durable** | Persisted to graph storage; survives session close and reload | `UserGrouped`, `Hyperlink`, named `ArrangementRelation` (saved frame), user-folder `ContainmentRelation` |
| **Session-only** | Lives in memory for the current session; evaporates on close | `tile-member` and `split-pair` `ArrangementRelation` edges |
| **Rolling-window** | Persisted but evicted when a max-age or max-count threshold is reached | `TraversalDerived` event records; `AgentDerived` within decay window |
| **Derived-readonly** | Recomputed from node data or external import on load; never persisted as edges | `url-path`, `domain`, `filesystem` `ContainmentRelation` edges |

---

## 5. Navigator Projection Policy

The Navigator, especially when rendered in a workbench-scoped host (see
`2026-03-13_chrome_scope_split_plan.md §5`), is the primary tree/list
projection surface over relation families.

### 5.0 Shared projection contract

Navigator is the canonical reusable hierarchical projection for relation
families. Workbench structure, containment, recent traversal, and imported
groupings should appear as sections or family-owned rows within Navigator rather
than as unrelated side panels.

Corollaries:

- workbench arrangement and graph hierarchy are distinct relation families, but
  share one projection surface;
- subsystem-specific trees should be avoided unless they need behaviors that
  Navigator cannot express;
- diagnostics and settings surfaces may configure Navigator sections/filters,
  but do not become alternate truth owners of hierarchy.
renders from a `WorkbenchChromeProjection`. That projection is now defined by
family-scoped sections with explicit priority ordering.

### 5.1 Section Priority (default navigator mode)

```
[Workbench]             ← Arrangement family: frames, tiles, active panes
  Frame A
    └─ node 1
    └─ node 2
  Tile B
    └─ node 3
[Folders]               ← Containment / user-folder sub-kind
  My Notebook
    └─ node 4
[Domain: example.com]   ← Containment / url-path + domain sub-kind
  /docs
    └─ /docs/api → node 5
[Unrelated]             ← No projection family — disconnected nodes
  node 6
  node 7
[Recent]                ← Traversal family: recently visited (collapsed by default)
  node 8 (3 visits)
[Imported]              ← Imported family (collapsed by default)
  Firefox bookmarks
    └─ node 9
```

### 5.2 Projection Ownership Rule

A node's **primary section** in the navigator is determined by the
highest-priority family that carries a relation to that node:

1. **Arrangement** (highest) — if the node has any arrangement-family edge in
   the current session or a saved frame
2. **Containment / user-folder** — if the node belongs to a user-defined folder
   or notebook
3. **Containment / derived** — if the node's URL implies a path or domain
   containment relation
4. **Semantic** — if the node has a `UserGrouped` or `AgentDerived` edge to
   another visible node (it appears alongside its group partner, not in a
   separate "semantic" section)
5. **Unrelated** (lowest) — if the node has no arrangement, containment, or
   active semantic edge that connects it to the current projection scope

A node appears in exactly one primary section. Cross-section annotation badges
(e.g., "also in Frame A" or "also in /docs/api") are shown on hover, not as
duplicate rows.

### 5.3 Navigator Mode Switching

The navigator can be switched to a single-family projection mode:

- **Workbench mode** (default when workbench is active): arrangement-first, as
  described in §5.1
- **Containment mode**: containment-first; tree structure driven by containment
  edges; frames/tiles become annotation badges
- **Semantic mode**: semantic-first; groups nodes by their UserGrouped clusters;
  useful for knowledge exploration without the workbench active
- **All nodes mode**: flat roster of all nodes, sorted by recency; no family
  filtering; equivalent to what FileTree's `GraphContainment` source currently
  approximates

Mode switching is a view-local preference, not a graph intent. It does not
mutate graph state.

---

## 6. Physics Semantics per Family

Each family's layout influence is a **view-scoped policy**, not a per-edge
property. The graph view chooses which families to weight and how. This prevents
per-edge physics chaos while still enabling family-differentiated layout.

| Family | Physics semantic | Default weight when active | Notes |
| --- | --- | --- | --- |
| Semantic | Elastic association | Medium | Nodes are drawn together but can separate; respects global layout |
| Traversal | Temporal trace | Weak | Only active in traversal-overlay lens; zero otherwise |
| Containment | Rigid containment | Strong | Cluster forms; children orbit parent |
| Arrangement | Local arrangement | Medium-weak | Soft cluster; frame members stay nearby but global forces still apply |
| Imported | None | Zero | Never affects physics |

The physics profile vocabulary (`rigid-containment`, `ordered-hierarchy`,
`elastic-association`, `temporal-trace`, `local-arrangement`, `background-derived`)
maps to these families. The physics system does not need to know about families
directly; the view-scope lens selects which families activate which profile
weights.

### 6.1 Mechanical Link to LensConfig

Family physics weights are carried as an optional `FamilyPhysicsPolicy` field on
`LensConfig`, alongside the existing `physics_profile_id`:

```rust
pub struct FamilyPhysicsPolicy {
    pub semantic_weight: f32,      // default 1.0
    pub traversal_weight: f32,     // default 0.0 (inactive unless lens enables it)
    pub containment_weight: f32,   // default 0.0 (inactive unless containment lens active)
    pub arrangement_weight: f32,   // default 0.5
    pub imported_weight: f32,      // always 0.0; field present for completeness
}

// Added to LensConfig:
// pub family_physics: Option<FamilyPhysicsPolicy>,
```

`None` means "use the global physics profile as-is; no family differentiation."
A non-`None` value causes the physics engine to scale each family's force
contributions by the corresponding weight before summing them for each frame.

The physics engine reads family membership from `EdgeKind` — it does not need
to know the higher-level family vocabulary. The mapping is:

| `EdgeKind` | Family weight field read |
| --- | --- |
| `Hyperlink`, `UserGrouped`, `AgentDerived` | `semantic_weight` |
| `TraversalDerived` | `traversal_weight` |
| `ContainmentRelation` | `containment_weight` |
| `ArrangementRelation` | `arrangement_weight` |
| `ImportedRelation` | `imported_weight` |

**Why weight = 0.0 is the correct "off" state:** setting a weight to `0.0`
means edges of that family contribute zero attractive force. Nodes connected
only by that family drift to their globally-determined positions. This is
exactly the desired behavior for derived containment at rest — the URL hierarchy
does not pull nodes together unless a containment lens is active. No special
"ignore this edge family" flag is needed; the weight field handles it cleanly.

The `LensPhysicsBindingPreference` and `ProgressiveLensAutoSwitch` preferences
defined in `2026-02-25_progressive_lens_and_physics_binding_plan.md §§1.2, 2.4`
apply normally — `FamilyPhysicsPolicy` is one more field that activates when a
lens is applied, subject to the same confirmation gate.

---

## 7. FileTree Tool Pane — Disposition

The current `ToolPaneState::FileTree` approximated hierarchy projection with a
flat list and three `FileTreeContainmentRelationSource` variants. With relation
families in place, its use cases are redistributed:

| Old source | Replacement |
| --- | --- |
| `GraphContainment` (flat node roster) | Navigator "All nodes mode" (§5.3) |
| `SavedViewCollections` | Arrangement-family saved frames in navigator Workbench section |
| `ImportedFilesystemProjection` | Containment-family `filesystem` sub-kind in navigator Domain/Folders section |

`ToolPaneState::FileTree` becomes a candidate for deprecation once the navigator
implements containment-mode projection. It is **not removed in this plan** — it
remains available as a legacy surface until the navigator sections covering its
use cases are shipped and validated.

---

## 8. Implementation Slices

This plan is intentionally additive. The existing `EdgeKind` model is not
disrupted. Implementation proceeds in three slices ordered by value delivered.

### Slice A — Navigator sections with existing edge kinds

Deliver the navigator section structure (§5.1) using only existing `EdgeKind`
values. No new variants needed yet:

- `Workbench` section: arrangement derived from egui_tiles Container shape and
  `FrameTabSemantics` (same source as current `WorkbenchChromeProjection`)
- `Unrelated` section: nodes with no active arrangement membership
- `Recent` section: nodes with `TraversalDerived` edges, sorted by recency

This is a pure render change — no graph model change. Delivers the multi-section
navigator immediately.

### Slice B — Containment family edges

Add `EdgeKind::ContainmentRelation` with the derived sub-kinds (`url-path`,
`domain`) computed at graph load from node URL data. Adds the Domain/Folders
section to the navigator. Enables containment-mode projection.

At this point, the FileTree tool pane's `ImportedFilesystemProjection` source
can be retired: filesystem containment edges replace it.

### Slice C — Arrangement family edges as durable graph relations

Add `EdgeKind::ArrangementRelation` with `frame-member`, `tile-member`, and
`split-pair` sub-kinds. Frames become named sets of durable `frame-member` edges
rather than workspace layout snapshots. This is a significant model change and
is the right final step once the navigator sections are stable and the containment
family is validated.

---

## 9. Acceptance Criteria

- [ ] All five relation families have documented persistence tier, visibility rule,
  deletion behavior, layout influence, and projection priority
- [ ] Navigator sections render correctly using Slice A (existing EdgeKind only)
- [ ] Nodes with no relation appear in "Unrelated" section, not missing entirely
- [ ] Single-family projection modes switch correctly without mutating graph state
- [ ] New `EdgeKind` variants (Slices B and C) added additively — no existing
  test regressions
- [ ] Derived containment edges (`url-path`, `domain`) are recomputed on load and
  not persisted to the snapshot
- [ ] Physics profile weights per family are view-scope policy, not per-edge
  properties
- [ ] FileTree tool pane remains functional until navigator sections covering its
  use cases are shipped

---

## 10. Non-Goals

- Generic pseudonodes for hyperedge membership — frames and tiles use
  `ArrangementRelation` edges, not synthetic "frame nodes"
- Custom per-edge physics properties — physics is always view-scope policy,
  never per-edge
- Merging the Containment family into a single tree across all sub-kinds —
  `url-path` and `user-folder` containment use different sections, not one merged
  hierarchy
- Any change to `AgentDerived` decay rules or suppression behavior — those are
  governed by `edge_traversal_spec.md`
- Mobile or touch-optimized navigator rendering — not a current target
