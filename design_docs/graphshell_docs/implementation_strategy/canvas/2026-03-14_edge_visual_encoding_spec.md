# Edge Visual Encoding Spec

**Date**: 2026-03-14
**Status**: Design — Pre-Implementation
**Purpose**: Define the canonical visual encoding for graph edges by relation
family. Establishes the mapping from `EdgeKind` (and future family variants) to
stroke style, color, width, opacity, directionality cue, and interaction
affordance. Replaces the ad hoc three-value `GraphEdgeVisualStyle` enum with a
principled, extensible encoding.

**Related**:

- `2026-03-14_graph_relation_families.md` — family vocabulary, persistence tiers
- `2026-03-14_canvas_behavior_contract.md` — physics scenarios (family force assertions)
- `layout_behaviors_and_physics_spec.md` — frame-affinity backdrop rendering (§4.6)
- `graph_node_edge_interaction_spec.md` — edge interaction model authority
- `../aspect_render/2026-03-12_compositor_expansion_plan.md` — render pass contract

---

## 1. Current State and What Changes

The current `GraphEdgeVisualStyle` enum in `model/graph/egui_adapter.rs` has
three variants with hardcoded colors:

| Current variant | Color | Style | Width |
| --- | --- | --- | --- |
| `Hyperlink` | Gray `(160, 160, 160)` | Solid | 1.4 |
| `History` | Blue `(120, 180, 210)` | Dashed | 1.8 + traversal bonus |
| `UserGrouped` | Amber `(236, 171, 64)` | Solid | 3.0 |

`AgentDerived` has no visual representation — it falls through to `Hyperlink`
style. The three future family variants (`ContainmentRelation`,
`ArrangementRelation`, `ImportedRelation`) have no encoding at all.

**What this spec changes:**

1. Extends `GraphEdgeVisualStyle` to cover all five relation families plus
   `AgentDerived` distinctly.
2. Establishes encoding rules that make family membership readable at a glance
   without requiring a legend for common cases.
3. Defines interaction affordances (hover, selection, inspect) per family.
4. Defines visibility rules — which families render by default and which require
   a lens to become visible.
5. Preserves and locks the existing color choices where they are already good
   (`UserGrouped` amber, `History` blue-dashed).

---

## 2. Encoding Principles

Five principles govern all family encoding decisions:

**P1 — Family readable at a glance.** A user looking at two connected nodes
must be able to determine *why* they are connected from the edge appearance
alone, without hovering. This means: stroke style (solid/dashed/dotted) carries
primary family signal; color carries secondary signal; width carries weight.

**P2 — Default canvas is not a visual dump.** Families that are not relevant to
everyday browsing (Traversal, Containment, Arrangement, Imported) are hidden by
default. The default canvas shows only Semantic family edges — the ones the user
explicitly created or navigated. A user does not need to see their URL hierarchy
drawn as graph edges while browsing.

**P3 — Lens activation reveals, not introduces.** When a containment or
traversal lens activates, edges that were hidden become visible. They do not
appear suddenly — they fade in. Deactivating the lens fades them out. The user
always knows the edge was always there; the lens just chose to show it.

**P4 — Multiplicity is readable.** A single pair of nodes can carry multiple
edge kinds (e.g., both `Hyperlink` and `TraversalDerived`). The encoding must
handle multi-kind edges without becoming illegible. The rule: render the
highest-priority family's stroke; add a secondary family indicator (a small
badge dot or a secondary thin stroke offset) rather than trying to merge the
strokes.

**P5 — Theme coherence.** Family colors are theme-aware tokens, not hardcoded
`Color32` values. The tokens map to theme palette roles so dark/light mode and
custom themes all produce coherent, distinguishable encodings.

---

## 3. Family Encoding Table

### 3.1 Default Visibility

| Family | Default visible | Visible when |
| --- | --- | --- |
| Semantic (`Hyperlink`, `UserGrouped`, `AgentDerived`) | Yes | Always |
| Traversal (`TraversalDerived`) | No | Traversal lens active |
| Containment (`ContainmentRelation`) | No | Containment lens active |
| Arrangement (`ArrangementRelation`) | No | Arrangement overlay lens active |
| Imported (`ImportedRelation`) | No | Import review mode active |

### 3.2 Stroke Encoding

| Sub-type | Stroke style | Color token | Width | Opacity | Directionality |
| --- | --- | --- | --- | --- | --- |
| `Hyperlink` | Solid | `edge.semantic.hyperlink` — neutral gray | 1.4 | 0.85 | Arrowhead on hover only |
| `UserGrouped` | Solid, bold | `edge.semantic.grouped` — amber | 3.0 | 1.0 | None (undirected) |
| `AgentDerived` | Solid, thin | `edge.semantic.agent` — muted violet | 1.2 | 0.55 | None; fades with decay |
| `TraversalDerived` | Dashed | `edge.traversal` — steel blue | 1.8 + traversal bonus | 0.7 | Arrow on dominant direction |
| `ContainmentRelation` / `url-path` | Dotted | `edge.containment.url` — teal | 1.0 | 0.6 | Arrowhead toward parent |
| `ContainmentRelation` / `domain` | Dotted, faint | `edge.containment.domain` — teal, lighter | 0.8 | 0.4 | Arrowhead toward parent |
| `ContainmentRelation` / `user-folder` | Solid | `edge.containment.folder` — teal, strong | 1.6 | 0.9 | Arrowhead toward parent |
| `ContainmentRelation` / `clip-source` | Dashed, short | `edge.containment.clip` — light blue | 1.2 | 0.75 | Arrowhead toward source |
| `ArrangementRelation` / `frame-member` | Double stroke | `edge.arrangement.frame` — indigo | 1.0 outer + 0.5 inner gap | 0.5 | None |
| `ArrangementRelation` / `tile-group` | Dotted, tight | `edge.arrangement.group` — indigo, lighter | 0.8 | 0.4 | None |
| `ImportedRelation` | Dashed, long gap | `edge.imported` — warm gray | 0.8 | 0.35 | None |

**Color token defaults** (dark theme base; light theme tokens are lighter/darker
inversions of the same hue):

| Token | Dark theme default |
| --- | --- |
| `edge.semantic.hyperlink` | `rgb(150, 150, 155)` |
| `edge.semantic.grouped` | `rgb(236, 171, 64)` ← existing, locked |
| `edge.semantic.agent` | `rgb(180, 140, 220)` |
| `edge.traversal` | `rgb(120, 180, 210)` ← existing, locked |
| `edge.containment.url` | `rgb(80, 190, 170)` |
| `edge.containment.domain` | `rgb(80, 190, 170)` at 60% opacity |
| `edge.containment.folder` | `rgb(60, 200, 160)` |
| `edge.containment.clip` | `rgb(140, 200, 240)` |
| `edge.arrangement.frame` | `rgb(130, 110, 220)` |
| `edge.arrangement.group` | `rgb(130, 110, 220)` at 60% opacity |
| `edge.imported` | `rgb(160, 150, 140)` |

### 3.3 AgentDerived Decay Opacity

`AgentDerived` edges have a time-based opacity that maps decay progress to
visual fade. The opacity decreases linearly from 0.55 (freshly asserted) to
0.15 (near eviction threshold), then the edge disappears at eviction.

```
opacity = lerp(0.55, 0.15, decay_progress)
where decay_progress = elapsed_since_last_assertion / decay_window
```

This makes the provisional nature of agent suggestions visually apparent without
requiring a separate UI element.

---

## 4. Multi-Kind Edge Rendering

When a single node pair carries multiple edge kinds (e.g., `Hyperlink` +
`TraversalDerived`, or `Hyperlink` + `UserGrouped`), rendering priority
determines the primary stroke; secondary kinds add a small indicator:

**Priority order** (highest renders as primary stroke):

1. `UserGrouped` (Semantic/grouped)
2. `ContainmentRelation` / `user-folder`
3. `Hyperlink` (Semantic/hyperlink)
4. `AgentDerived` (Semantic/agent)
5. `TraversalDerived` (Traversal)
6. `ContainmentRelation` / other sub-kinds
7. `ArrangementRelation`
8. `ImportedRelation`

**Secondary indicator:** a small filled dot (radius 3px) rendered at the edge
midpoint in the secondary family's color. At most one secondary indicator per
edge — if three or more kinds are present, only the highest-priority secondary
is shown; a `+` superscript on the dot indicates more kinds exist. Hovering the
dot opens the edge inspect popover (§6).

This avoids stroke layering complexity while keeping multi-kind edges
distinguishable from single-kind edges.

---

## 5. Interaction Affordances

### 5.1 Hover

On edge hover:
- Primary stroke brightens by 20% and width increases by 0.5px
- Arrowhead appears for directed families (Hyperlink, ContainmentRelation) if
  not already shown
- A small tooltip appears after 400ms: family name + sub-kind + key metadata
  (e.g., "Semantic · User grouped · Label: research")
- For `TraversalDerived`: tooltip includes navigation count and last traversal
  date

### 5.2 Selection

Selected edges:
- Primary stroke uses `edge.selected` color token (bright accent, theme-defined)
- Width increases by 1.0px
- Multi-kind secondary dot becomes brighter

Edge selection is triggered by click on the edge stroke (hit target: 8px wide
regardless of actual stroke width — edges are thin and need a wider click area).

### 5.3 Edge Inspect Popover

Right-click on any edge (or click the secondary-kind dot) opens an edge inspect
popover anchored to the click position:

```
┌──────────────────────────────────────┐
│ [node A title]  →  [node B title]    │
│                                      │
│ Kinds:                               │
│  ● Semantic · Hyperlink              │
│  ● Traversal · 14 visits · last Mon  │
│                                      │
│ [Remove UserGrouped]  [Dismiss Agent]│
└──────────────────────────────────────┘
```

Actions in the popover are family-appropriate:
- Semantic/UserGrouped: "Remove grouping"
- Semantic/AgentDerived: "Dismiss suggestion" / "Accept (keep permanently)"
- Traversal: "Clear traversal history for this pair"
- ContainmentRelation/user-folder: "Remove from folder"
- ContainmentRelation/derived: read-only, no action (label: "Derived from URL")
- ArrangementRelation: "Remove from frame" (durable) or read-only for
  session-only
- Any visible edge family: "Dismiss in this view" suppresses only that edge
  instance in the current `GraphViewId`'s `EdgePolicy`; it does not delete the
  underlying relation/event truth

### 5.4 Edge Creation Gesture

Edge creation gestures per family (interaction model authority is
`graph_node_edge_interaction_spec.md` — this spec only lists the family mapping):

| Family | Default creation gesture | Notes |
| --- | --- | --- |
| Semantic/UserGrouped | Shift+drag from node to node | Existing gesture |
| Semantic/AgentDerived | Agent-initiated; no user gesture | Accept via popover |
| Hyperlink | Automatic on link-follow navigation | Not user-created directly |
| TraversalDerived | Automatic on navigation | Not user-created directly |
| ContainmentRelation/user-folder | "Add to folder" command or drag-to-navigator | Via navigator, not canvas drag |
| ContainmentRelation/derived | Automatic from URL structure | No gesture |
| ArrangementRelation | Automatic from tile tree | No gesture; managed via workbench |
| ImportedRelation | Automatic from import | No gesture |

---

## 6. Canvas Visibility Filter Controls

When non-default families are visible (lens active), the Graph Bar chip for
that lens shows which families are currently rendered. The chip expands into
the current graph view's `EdgePolicy`, revealing per-family toggles:

```
[Lens: Containment ▾]
  ☑ url-path edges
  ☑ domain edges
  ☐ clip-source edges
  ☑ user-folder edges
```

These toggles mutate view-local `EdgePolicy` state, not graph state. They allow
the user to reduce visual noise within a lens without deactivating it entirely.

`EdgePolicy` rules (normative):

- Family/sub-kind toggles are `GraphViewId`-local.
- Per-edge dismissals are stored alongside the view's `EdgePolicy`.
- Dismissing one edge must not hide all edges of that family.
- Copying a graph view clones its `EdgePolicy`, including per-family toggles and
  per-edge dismissal state, so the copied view preserves the same rendered edge
  arrangement and derived node layout.

The default canvas (no lens) has no visible filter controls for hidden families
— the absence of non-semantic edges is the default, not a user-configured state.

---

## 7. GraphEdgeVisualStyle Migration

The current `GraphEdgeVisualStyle` enum in `egui_adapter.rs` must be extended
to cover the new variants. The migration is additive:

```rust
// Current (3 variants):
enum GraphEdgeVisualStyle {
    Hyperlink,
    History,
    UserGrouped,
}

// Extended (covers all families):
enum GraphEdgeVisualStyle {
    // Semantic family
    Hyperlink,
    UserGrouped,
    AgentDerived { decay_progress: f32 },
    // Traversal family
    TraversalHistory,
    // Containment family (visible only when containment lens active)
    ContainmentUrlPath,
    ContainmentDomain,
    ContainmentUserFolder,
    ContainmentClipSource,
    // Arrangement family (visible only when arrangement overlay active)
    ArrangementFrameMember,
    ArrangementTileGroup,
    // Imported family (visible only in import review mode)
    ImportedRelation,
}
```

`style_from_payload()` priority order follows §4 priority. The `hidden` field
on `GraphEdgeShape` drives default-off families — it is set based on the active
lens state passed into the draw context, not hardcoded.

---

## 8. Acceptance Criteria

- [ ] All five families have distinct, named visual encodings per §3.2
- [ ] Default canvas renders only Semantic family edges; all other families
  hidden unless a lens is active
- [ ] `AgentDerived` edges fade from opacity 0.55 → 0.15 as decay progresses
- [ ] Multi-kind edges show primary stroke + secondary dot indicator per §4
- [ ] Edge hover tooltip shows family name, sub-kind, and key metadata within
  400ms
- [ ] Edge inspect popover shows all kinds on the edge with family-appropriate
  actions
- [ ] Edge hit target is 8px wide regardless of stroke width
- [ ] `GraphEdgeVisualStyle` extended with all new variants; existing `Hyperlink`
  and `History` colors unchanged (locked)
- [ ] Family visibility toggles in lens chip work without mutating graph state
- [ ] All encodings tested with dark theme and light theme tokens
- [ ] No `AgentDerived` edge visible at `decay_progress >= 1.0`

---

## 9. Non-Goals

- Animated edges (flowing particles, animated dashes) — deferred; out of scope
  for initial encoding
- 3D edge rendering — follows `ViewDimension` stabilization; not covered here
- Edge label rendering at zoom-out — governed by `layout_algorithm_portfolio_spec.md`
  label overlap metrics
- Per-edge custom color overrides — not supported; family tokens are the
  encoding, not per-edge properties
- Edge bundling for dense graphs — separate layout concern, not an encoding concern
