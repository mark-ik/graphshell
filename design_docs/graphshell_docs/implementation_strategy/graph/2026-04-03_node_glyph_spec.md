# Node Glyph Spec

**Date**: 2026-04-03
**Status**: Draft — canonical visual-form authority for graph nodes
**Priority**: Post-renderer prerequisite; informed by current `GraphNodeShape` implementation

**Related**:

- `GRAPH.md` — Graph domain authority; §9 defers the broader `canvas_render_pipeline_spec.md`
- `graph_node_edge_interaction_spec.md` — §4.8 defines canonical LOD tiers (Point / Compact / Expanded)
- `node_badge_and_tagging_spec.md` — badge visual system; badges compose atop the resolved glyph
- `faceted_filter_surface_spec.md` — PMEST facets are inputs to glyph resolution
- `2026-02-24_physics_engine_extensibility_plan.md` — "Node glyph renderer plugins" stub extracted here
- `2026-03-14_edge_visual_encoding_spec.md` — edge visual encoding; analogous authority for edges
- `../aspect_render/render_backend_contract_spec.md` — render backend abstraction
- `../viewer/universal_content_model_spec.md` — MIME detection and viewer binding
- `../../TERMINOLOGY.md`

---

## 1. Scope

This spec defines the **node glyph**: the visual form of a node on the canvas. A glyph
determines what a node looks like — its shape, fill, imagery, and how those compose across
zoom levels.

A glyph is **not** the node. The `Node` is the graph entity: it has identity, address,
content, history, and classification. The glyph is what you see when that node is drawn on
the canvas. PMEST facets describe what a node IS; the glyph describes how it APPEARS.

### 1A. What this spec owns

- Glyph anatomy: the visual elements that compose a node's canvas appearance.
- Glyph resolution: how the system selects and composes a glyph for a given node.
- LOD presentation: how the glyph simplifies or enriches across LOD tiers.
- Content imagery: what visual content (favicon, thumbnail, emblem, data representation) a
  glyph incorporates and how it is composed.
- User-authored glyphs: the contract for user-defined glyph rules.

### 1B. What this spec does not own

- **LOD tier thresholds** — canvas-wide policy, defined by
  `graph_node_edge_interaction_spec.md §4.8`. This spec defines glyph behavior AT each
  tier, not WHEN tiers transition.
- **Physics hull** — collision shapes and Rapier/Parry bodies are owned by the physics
  system. The glyph may inform hull shape (e.g. circular glyph implies circle collider),
  but the hull is registered and managed by the physics authority.
- **Interaction state machine** — selection, hover, focus, drag states are owned by graph
  interaction semantics (`graph_node_edge_interaction_spec.md`). The glyph defines how it
  renders in response to each state, not how states transition.
- **Badge system** — badges are a layer that composes atop the glyph
  (`node_badge_and_tagging_spec.md`). Glyph resolution does not produce badges.
- **Batching, culling, and GPU resource lifecycle** — deferred to
  `canvas_render_pipeline_spec.md`.
- **Edge glyphs** — edge visual encoding has its own authority
  (`2026-03-14_edge_visual_encoding_spec.md`).

### 1C. Three-Tree Authority Contract

#### Graph Tree authority

- Node data that feeds glyph resolution (address, MIME hint, tags, classifications,
  lifecycle, thumbnail/favicon content) is graph-owned state.
- Glyph resolution reads this data; it does not mutate it.

#### Workbench Tree authority

- Workbench owns canvas tile arrangement and which views host graph surfaces.
- Workbench does not own glyph selection or glyph rendering.

#### UxTree contract

- At `Compact` and `Expanded` LOD, the resolved glyph shape informs the UxTree node's
  bounding region for accessibility hit-testing.
- At `Point` LOD, individual node UxTree children are omitted per
  `graph_node_edge_interaction_spec.md §4.8`.

---

## 2. Glyph Anatomy

A resolved glyph is composed of the following visual elements, drawn in this order
(back-to-front):

### 2.1. Body

The base shape and fill of the node mark on the canvas.

| Property | Description |
|---|---|
| **Silhouette** | The closed path defining the node's outer boundary. Default: circle. Future: rounded rectangle, hexagon, custom SVG path, procedural shape. |
| **Fill** | Interior color or gradient. Resolved from ThemeRegistry tokens, possibly overridden by content-derived or user-authored rules. |
| **Border** | Stroke around the silhouette. Width, color, and dash pattern. |
| **Material** | Optional surface treatment beyond flat fill: texture, pattern, subtle noise. Speculative — included for future glyph expressiveness. |

The body silhouette also defines the **label exclusion region** — the area the label
positioning algorithm avoids when placing text.

### 2.2. Content Imagery

Visual content composed inside or adjacent to the body, derived from the node's content
and metadata.

| Source | Resolution rule | Display |
|---|---|---|
| **Favicon** | `Node.favicon_rgba` when present | Centered within body at ~75% of radius; shown at Compact and Expanded LOD |
| **Thumbnail** | `Node.thumbnail_png` when present | Shown on hover/selection or always at Expanded LOD; sized to body bounding box |
| **Emblem** | Derived from `AddressKind` or `mime_hint` when no favicon exists | Scheme-specific icon (file icon, directory icon, clip marker, data URI icon) |
| **Data representation** | Future: sparkline, color swatch, or miniature visualization for structured data nodes | Gated by content type and glyph rule |

Content imagery is populated based on what the node carries. The glyph does not fetch
content — it renders what the node already has. Content acquisition (favicon download,
thumbnail capture) is owned by the viewer/lifecycle systems.

### 2.3. State Rendering

How the glyph's visual elements respond to interaction and lifecycle states. The glyph
does not own these states — it defines the visual contract for each.

| State | Visual treatment |
|---|---|
| **Default** | Body at full opacity with theme-resolved fill and border. |
| **Hovered** | Hover ring (`graph_node_hover_ring` token). Thumbnail revealed at Compact LOD. |
| **Selected** | Selection ring (`graph_node_selection` token). Secondary selection uses a fainter halo. |
| **Focused** | Focus ring (`graph_node_focus_ring` token). Must meet WCAG 2.2 SC 2.4.11 minimum area. |
| **Dragged** | Subtle drop-shadow or scale pulse (120 ms ease-out). |
| **Crashed** | `Crashed` badge overlaid (badge spec authority). Border flashes error token. |
| **Archived** | Reduced opacity (0.35–0.45) per badge spec `#archive` rendering. |
| **Ghost (Tombstone)** | Dashed border ring at reduced opacity; no content imagery. Per existing `push_ghost_dashed_ring` contract. |
| **Search match** | Highlight ring (`graph_node_search_match` / `_active` tokens). |

### 2.4. LOD Presentation

How the glyph adapts to the canonical LOD tiers. Tier thresholds and hysteresis are
defined by `graph_node_edge_interaction_spec.md §4.8`; this section defines glyph
behavior at each tier.

| LOD tier | Glyph rendering contract |
|---|---|
| **Point** (`camera.scale < 0.55`) | Body silhouette only, rendered as a minimal mark (filled circle/dot). No content imagery, no label. Color carries node identity (domain-hue or theme default). Size is a fixed minimum screen-space radius (not smaller than 3 dp). |
| **Compact** (`0.55 ≤ camera.scale < 1.10`) | Body with border, favicon or emblem if available, up to one key badge (per badge spec §3.2 compact slot budget). Label on hover/selection only. |
| **Expanded** (`camera.scale ≥ 1.10`) | Full glyph: body, content imagery (thumbnail when available), all visual affordances. Persistent label above threshold 1.5×. |

#### Graphlet collapse presentation

When a graphlet collapses to a single representative node (per `graphlet_model.md`
collapse semantics), the representative node's glyph gains a **cluster indicator** —
a subtle concentric ring or stacked-silhouette treatment communicating that this mark
represents a group. The cluster indicator count or density should reflect the collapsed
member count, capped at a visual maximum.

---

## 3. Glyph Resolution

A glyph is **resolved, not stored**. No `glyph` field exists on `Node`. Instead, the
rendering pipeline resolves a glyph for each visible node each frame (with caching for
stable inputs).

### 3.1. Resolution Pipeline

```
Node data
  ├─ AddressKind, mime_hint, tags, classifications  (PMEST inputs)
  ├─ lifecycle (Active/Warm/Cold/Tombstone)
  ├─ content (favicon_rgba, thumbnail_png)
  └─ is_clip, is_pinned, etc.
      │
      ▼
  Glyph Rule Matching
      │  Rules are evaluated in priority order (§3.2).
      │  First matching rule produces a GlyphTemplate.
      ▼
  Theme Application
      │  ThemeRegistry resolves fill, border, ring colors
      │  from the active ThemeTokenSet.
      ▼
  LOD Adaptation
      │  Current camera.scale selects the LOD tier.
      │  GlyphTemplate emits a tier-appropriate shape list.
      ▼
  Resolved Glyph  →  Vec<Shape> for the render pipeline
```

### 3.2. Glyph Rule Matching

A glyph rule is a `(predicate, template)` pair. The system evaluates rules in priority
order and selects the first match.

**Rule priority** (highest to lowest):

1. **User-authored rules** — user-defined predicates and templates (§4).
2. **Tag-driven rules** — reserved system tags that imply a distinct visual form:
   - `#clip` → clip-marked glyph (distinct border treatment).
   - Future: additional tag-driven shapes.
3. **Content-derived rules** — MIME type or address kind implies a visual form:
   - `mime:image/*` → image-preview glyph body.
   - `AddressKind::Directory` → directory glyph.
   - Future: richer content-type specialization.
4. **Physics-informed rules** — the active physics profile may inform glyph aesthetics
   (e.g. "water droplet" shapes when `physics:liquid` is active). These are cosmetic
   hints, not structural. Registered via the render dispatch table, and informed by but
   not owned by `PhysicsProfileRegistry`.
5. **Theme default** — standard circular glyph with theme-resolved fill and border.

When no rule matches, the theme default always applies (cannot fail).

### 3.3. GlyphTemplate

A `GlyphTemplate` is the intermediary between rule matching and rendering. It encodes
enough information to produce shapes at any LOD tier.

**Conceptual shape** (not a final Rust struct — implementation will refine):

```
GlyphTemplate {
    silhouette: SilhouetteKind,      // Circle, RoundedRect, Hex, SvgPath, Procedural
    fill_policy: FillPolicy,          // ThemeDefault, ContentDerived, Fixed(Color)
    border_policy: BorderPolicy,      // ThemeDefault, Dashed, Double, Custom
    content_slot: ContentSlotPolicy,  // Favicon, Thumbnail, Emblem, DataViz, None
    point_lod_hint: PointLodHint,     // MinimalDot, ColoredDot, TinyIcon
    cluster_indicator: bool,          // Whether to render graphlet-collapse treatment
}
```

### 3.4. Caching

Glyph resolution is a pure function of its inputs. The resolved glyph for a node can be
cached and invalidated when any input changes:

- Node content change (favicon, thumbnail, MIME hint, tags, classifications, lifecycle)
- Theme change (active theme switch)
- Glyph rule change (user edits a custom glyph rule)
- Physics profile change (active profile switch, if physics-informed rules are active)

LOD adaptation is NOT cached — it depends on the current camera scale, which changes
continuously during zoom.

---

## 4. User-Authored Glyphs

Users can define custom glyph rules. A user-authored glyph is a `(predicate, template)`
pair where:

- **Predicate**: a combination of node attributes (address kind, MIME hint, tag set,
  classification scheme, domain pattern) that selects which nodes the glyph applies to.
- **Template**: a user-defined `GlyphTemplate` specifying silhouette, fill, border,
  content slot, and LOD behavior.

User-authored rules evaluate at highest priority (§3.2), so they override all
system-derived glyphs.

### 4.1. Persistence

User-authored glyph rules are persisted as part of the user's `GraphshellProfile`
(per `2026-03-02_graphshell_profile_registry_spec.md`). They are portable across
workspaces and syncable via the profile sync mechanism.

### 4.2. Assignment Surface

The glyph assignment surface is **deferred** — this spec defines the data contract,
not the UI for creating/editing glyphs. The assignment surface will likely integrate
with the tag assignment panel and the settings/control surfaces.

### 4.3. Constraints

- A user-authored glyph must specify at least one predicate criterion (an unconditional
  "apply everywhere" rule could override the theme default, but must be an explicit
  choice, not accidental).
- Custom SVG paths are bounded to a maximum complexity (vertex count, path length) to
  prevent degenerate rendering performance.
- User glyphs must not override the crash/ghost/search-match state treatments, which are
  system-owned safety indicators.

---

## 5. Relationship to Existing Systems

### 5.1. ThemeRegistry

The glyph resolution pipeline consumes `ThemeTokenSet` for fill, border, ring, and chrome
colors. The glyph does not define colors — it requests them from the theme.

Specifically, these existing tokens feed glyph rendering:

- `graph_node_chrome: GraphNodeChromeTheme` — badge/pin/clip/stroke chrome
- `graph_node_hover`, `graph_node_selection`, `graph_node_focus_ring`,
  `graph_node_hover_ring` — state ring colors
- `graph_node_search_match`, `graph_node_search_match_active` — search highlight

Future theme extension: themes may define glyph-aware tokens (e.g. silhouette rounding
radius, material texture presets) once glyph templates stabilize beyond circles.

### 5.2. PMEST Facets

PMEST facets are the analytical inputs to glyph resolution, not the output:

| PMEST facet | Glyph input role |
|---|---|
| **Personality** (AddressKind) | Selects emblem fallback; informs content-derived rules |
| **Matter** (mime_hint, viewer binding) | Selects content-derived glyph rules (image preview, document icon, etc.) |
| **Energy** (edge kinds, traversal count) | Potential future input for visual weight/size modulation |
| **Space** (frame membership, cluster) | Informs graphlet-collapse cluster indicator |
| **Time** (lifecycle, last traversal) | Selects ghost glyph for tombstoned nodes; potential freshness indicator |

The glyph is orthogonal to PMEST — it consumes facet data, it is not a facet itself.

### 5.3. Badge System

Badges compose **atop** the resolved glyph. The badge spec
(`node_badge_and_tagging_spec.md`) defines badge positioning relative to the node's
bounding region. The glyph's body silhouette defines that bounding region.

The glyph-badge contract:

- Glyph provides: body center, body radius, silhouette bounding rect.
- Badge system uses those to position at-rest badges (top-right corner), orbit badges
  (expanded hover), and overflow chips.
- Glyph does not render badges. Badge rendering is badge-spec authority.

### 5.4. Physics System

The glyph may **inform** the physics hull shape. When the physics system needs a collider
for a node (Parry2d spatial queries, future Rapier bodies), it can query the glyph's
silhouette kind:

- Circle silhouette → circle collider
- Rounded rectangle → AABB or convex hull collider
- Custom path → convex hull approximation

This is a **read** relationship. The glyph does not register colliders or manage physics
state. The physics system owns its own collider lifecycle.

### 5.5. GraphNodeShape (Current Implementation)

The existing `GraphNodeShape` struct in `model/graph/egui_adapter.rs` is the current
monolithic implementation of what this spec decomposes. Today:

- `GraphNodeShape` holds position, visual state flags, content data (favicon/thumbnail),
  badge state, theme tokens, and produces `Vec<Shape>` via `DisplayNode::shapes()`.
- There is no dispatch — every node uses the same rendering path (circle + optional
  favicon + optional thumbnail + badges + label).

This spec establishes the architectural direction for replacing `GraphNodeShape` with
a dispatch-capable glyph resolution system. Migration is incremental:

1. **First**: extract the glyph resolution concept as a pure function from node data to
   visual description, layered atop the existing `GraphNodeShape` rendering.
2. **Then**: introduce glyph rules and template dispatch, initially with only the default
   circular template (behavior-preserving).
3. **Then**: add content-derived and tag-driven template variants.
4. **Then**: enable user-authored glyph rules.

---

## 6. Diagnostics

Glyph resolution participates in the graph diagnostics channel family:

| Channel | Content |
|---|---|
| `graph.glyph.resolution` | Per-frame: count of resolved glyphs, cache hit rate, rule-match distribution |
| `graph.glyph.content` | Favicon/thumbnail texture load events, emblem fallback triggers |

---

## 7. Non-Goals and Deferrals

- **Animated glyph transitions** — morph animations between glyph templates (e.g. when a
  node's MIME type is detected and the glyph switches from emblem to image-preview).
  Deferred to a future glyph-animation extension.
- **3D glyphs** — volumetric or perspective-projected node marks. Deferred to the
  projection-mode lane (`2026-04-03_twod_twopointfive_isometric_plan.md`).
- **Edge glyphs** — edges have their own visual authority
  (`2026-03-14_edge_visual_encoding_spec.md`).
- **Glyph marketplace / sharing** — user-authored glyphs are local/profile-scoped for now.
  Community sharing is a future social feature.
- **Audio-reactive glyph modulation** — glyph responding to audio input. Remains in the
  umbrella physics note as speculative.

---

## 8. Open Questions

1. Should the glyph template vocabulary be extensible at runtime (WASM-loaded custom
   silhouettes), or is the built-in set sufficient for the prototype era?
2. How tightly should physics-informed glyph hints couple to `PhysicsProfile`? A loose
   "aesthetic hint" string may be sufficient vs. a typed glyph selector on the profile.
3. Should graphlet-collapse cluster indicators be owned by the glyph spec or by a
   dedicated graphlet-presentation authority?

---

## Changelog

- 2026-04-03: Initial draft. Extracted from umbrella physics note "Node glyph renderer
  plugins" stub. Establishes glyph as the node's visual form, resolved via a rule-matching
  pipeline, orthogonal to PMEST facets, consuming ThemeRegistry tokens, and informing (but
  not owning) physics hull shape.
