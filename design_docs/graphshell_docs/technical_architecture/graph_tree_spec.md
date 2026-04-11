# GraphTree Specification

**Date**: 2026-04-10
**Status**: Architecture design — pre-implementation
**Scope**: API design for the `graph-tree` crate — a framework-agnostic,
graphlet-native tile tree that replaces egui_tiles and collapses the
Navigator/Workbench projection gap.

**Related**:

- `graphlet_model.md` — canonical graphlet semantics
- `unified_view_model.md` — five-domain model (Graph, Navigator, Workbench, Viewer, Shell)
- `../implementation_strategy/navigator/NAVIGATOR.md` — Navigator domain spec
- `../implementation_strategy/navigator/navigator_interaction_contract.md` — click grammar
- `../implementation_strategy/workbench/graphlet_projection_binding_spec.md` — graphlet binding
- `../implementation_strategy/graph/2026-03-14_graph_relation_families.md` — relation families
- `../implementation_strategy/subsystem_ux_semantics/ux_tree_and_probe_spec.md` — UxTree contract
- `../implementation_strategy/graph/view_dimension_spec.md` — GraphViewId
- `2026-03-29_portable_web_core_host_envelopes.md` — host envelope model
- `../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md` — research backing

---

## 1. Why egui_tiles Is the Wrong Abstraction for Graphlets

egui_tiles models **spatial geometry**: binary splits, tab groups, proportional
shares. Its `Container` types are `Linear | Tabs | Grid`. It has no concept of:

- why panes are grouped together (graph topology vs. manual arrangement)
- parent-child relationships between panes (opening B from A)
- graphlet membership, anchors, or derivation rules
- tree-style navigation (expandable hierarchy reflecting graph structure)
- linked vs. unlinked binding to a graphlet definition
- causality (why a node was added to a group)

Graphshell already works around this by layering workbench semantics on top
(`GraphshellTileBehavior`, `GraphletBinding`, `TileCoordinator`, frame grouping,
focus routing). But the core data structure doesn't speak graphlet — it speaks
rectangles.

---

## 2. What GraphTree Replaces

A portable `GraphTree<N>` where the tree structure comes from graph topology:

```
GraphTree<N>
├── Structure: derived from graphlet edges (parent → child = "opened from")
├── Anchors: primary + secondary anchors from graphlet definition
├── Members: node set with lifecycle state (Active/Warm/Cold)
├── Layout projection: how structure maps to spatial arrangement
│   ├── TreeStyleTabs → sidebar tree (Navigator chrome)
│   ├── FlatTabs → traditional tab bar
│   ├── SplitPanes → spatial layout (current egui_tiles behavior)
│   └── Hybrid → tree in sidebar, splits in main area
├── Binding: Linked | UnlinkedSessionGroup | ForkedPinned
└── Causality: per-member attachment reason (traversal, manual, spawn)
```

Key differences from egui_tiles:

- **Semantic tree**: parent-child comes from graph edges, not spatial splits
- **Multiple layout projections**: same tree, different spatial renderings
- **Navigator and workbench share the structure**: tree-style tab sidebar IS
  the graphlet tree; split panes are a spatial projection of it
- **Cold members are visible**: egui_tiles only knows about open panes;
  a graphlet tree knows about all members including cold ones

### What this replaces

- egui_tiles as the workbench layout data structure
- The ad hoc `GraphletBinding` → tile group correspondence
- Navigator's separate projection over workbench arrangement state
- The semantic gap between "tab management" and "graph navigation"

### What this does NOT replace

- egui_tiles' layout computation (simple proportional split is still needed
  for the spatial projection — ~200 lines of pure math, inlined or vendored)
- Graph truth (GraphTree is still a projection, not owned truth)
- The three-pass compositor (chrome → content → overlay still applies)

---

## 3. How GraphTree Maps to Navigator

Navigator's graphlet projection (§8A of NAVIGATOR.md) already describes:

- ego/corridor/component/frontier/session graphlets
- tree or list presentation when hierarchy is clearer than space
- radial or corridor layouts when spatial relation is the point

A GraphTree IS the Navigator's graphlet in tree form. The Navigator doesn't
need a separate projection — it reads the same tree and renders it as:

- tree-style tabs in a sidebar (expandable, with lifecycle badges)
- breadcrumb trail (path from anchor to focused node)
- section headers (frames = sub-trees with shared anchors)

The workbench reads the same tree and renders it as:

- spatial split panes (for active/warm members)
- tab groups (for members sharing a container)
- placeholder slots (for cold members that could be activated)

---

## 4. Framework-Agnostic by Construction

The core data structure has zero rendering dependencies. Framework adapters:

- `graph-tree-egui`: renders tree as egui widgets (sidebar + split panes)
- `graph-tree-iced`: renders tree as iced views
- `graph-tree-web`: renders tree as DOM elements (extension/PWA)

---

## 5. Extension/PWA Portability

In an extension/PWA context:

- The tree serializes naturally as JSON (same as `GraphIntent` WAL format)
- Tree-style tabs in a sidebar IS the primary navigation UI (no spatial split
  panes needed in a constrained viewport)
- The same data structure renders as a DOM tree (ul/li) in a browser extension
  sidebar, or as native split panes on desktop
- Cold members are visible in all hosts (the tree knows about them even
  without open panes)
- Navigation actions (`expand`, `activate`, `dismiss`, `scope`) are the same
  across hosts — only the rendering differs

This is the "one portable core, many host envelopes" principle applied to the
tile tree itself.

---

## 6. Crate API Design

### 6.1 Crate identity

```
graph-tree/
├── Cargo.toml          # deps: serde, taffy, petgraph (optional)
├── src/
│   ├── lib.rs          # public API re-exports
│   ├── tree.rs         # GraphTree<N> — the core
│   ├── topology.rs     # TreeTopology — graph-derived parent/child
│   ├── member.rs       # MemberEntry, Lifecycle, Provenance
│   ├── graphlet.rs     # GraphletRef — connected sub-structures
│   ├── lens.rs         # ProjectionLens — how to slice the tree
│   ├── layout.rs       # Layout projection via taffy
│   ├── nav.rs          # Navigation actions and intents
│   ├── query.rs        # Tree queries (ancestors, descendants, siblings)
│   ├── ux.rs           # UxNode emission for accessibility/testing
│   └── serde_compat.rs # Serialization + backward compat
```

No egui. No iced. No winit. No wgpu. Pure data + pure functions.

`petgraph` feature for deriving topology from a petgraph graph.
`taffy` for layout computation (flexbox/grid-capable, not just proportional).

### 6.2 Core types and naming rationale

**Why "GraphTree" not "GraphletTree":** The tree reflects the state of a graph
view. Graphlets are connected groupings *within* that view — like documents in
a folder, not the folder itself. A graph view may contain many graphlets
(disconnected clusters, session groups, derived sub-structures). Naming the
tree after its leaves instead of its trunk would be misleading.

**One GraphTree per GraphViewId.** Multiple graph views of the same graph get
independent trees (different expansion state, focus, scroll, active lens).

```rust
use std::collections::{HashMap, HashSet, BTreeMap};
use serde::{Serialize, Deserialize};

/// Portable rectangle — no framework dependency.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

/// Identity of a tree member. Generic so Graphshell uses NodeKey,
/// extensions use Uuid, tests use u64.
pub trait MemberId:
    Clone + Eq + std::hash::Hash + std::fmt::Debug
    + Serialize + for<'de> Deserialize<'de> {}
impl<T> MemberId for T where T:
    Clone + Eq + std::hash::Hash + std::fmt::Debug
    + Serialize + for<'de> Deserialize<'de> {}

/// Identity of a graph view. Lets the host manage multiple trees.
pub trait ViewId: MemberId {}
impl<T: MemberId> ViewId for T {}
```

### 6.3 GraphTree\<N\>

```rust
/// The core data structure. One per GraphViewId. Contains all members
/// of a graph view — active, warm, and cold — organized by graph
/// topology with multiple projection lenses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphTree<N: MemberId> {
    // --- Membership ---
    members: HashMap<N, MemberEntry<N>>,

    // --- Topology (graph-derived parent/child) ---
    topology: TreeTopology<N>,

    // --- Graphlet index (connected sub-structures) ---
    graphlets: Vec<GraphletRef<N>>,

    // --- Active projection lens ---
    active_lens: ProjectionLens,

    // --- Session state (not graph truth) ---
    active: Option<N>,
    expanded: HashSet<N>,
    scroll_anchor: Option<N>,

    // --- Layout ---
    layout_mode: LayoutMode,
}
```

### 6.4 MemberEntry — what each member carries

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberEntry<N: MemberId> {
    /// Display lifecycle. The tree does NOT own transitions —
    /// it receives them from the host via SetLifecycle.
    pub lifecycle: Lifecycle,

    /// Why this member is in the tree. Maps to Provenance family
    /// edge sub-kinds. Preserved for reconciliation and undo.
    pub provenance: Provenance<N>,

    /// Which graphlet(s) this member belongs to.
    pub graphlet_membership: Vec<GraphletId>,

    /// Optional taffy layout overrides (min size, flex grow, etc.).
    pub layout_override: Option<LayoutOverride>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lifecycle {
    Active,   // open in a pane, rendering, may have focus
    Warm,     // has runtime state, not focused
    Cold,     // in the graph view but not in a pane
}

/// Why this member is in the tree. Aligned with Provenance family
/// and arrangement edge sub-kinds from graph_relation_families.md.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Provenance<N: MemberId> {
    /// Opened by following a link/edge from another member.
    /// Maps to Traversal family edge.
    Traversal { source: N, edge_kind: Option<String> },
    /// Manually added by user action (drag, command palette, import).
    /// Maps to UserGrouped family edge.
    Manual { source: Option<N>, context: Option<String> },
    /// Present as a graphlet anchor or graph view root.
    Anchor,
    /// Derived by graphlet computation (component, ego, corridor, etc.).
    /// Placed as sibling of its connection point in the topology.
    Derived { connection: Option<N>, derivation: String },
    /// Agent-inferred (AI enrichment). Carries confidence + decay.
    /// Maps to AgentDerived family edge.
    AgentDerived { confidence: f32, agent: String, source: Option<N> },
    /// Restored from persistence.
    Restored,
}

/// Taffy-compatible layout overrides per member.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutOverride {
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub preferred_split: Option<SplitDirection>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SplitDirection { Horizontal, Vertical }
```

### 6.5 TreeTopology — graph-derived structure

```rust
/// The tree's parent-child structure, derived from graph edges.
/// This is NOT spatial layout — it's semantic grouping.
///
/// Placement rules:
/// - Traversal: child of source node ("opened B from A" → B is child of A)
/// - Manual add: sibling of connection point (same parent as the node
///   you were looking at when you added it)
/// - Derived (graphlet computation): sibling of connection point,
///   or child of graphlet anchor if no specific connection
/// - AgentDerived: sibling of source, pending user accept
/// - Anchor: root
/// - Restored: original position from persistence
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeTopology<N: MemberId> {
    parent: HashMap<N, N>,
    children: HashMap<N, Vec<N>>,
    roots: Vec<N>,
    insertion_order: Vec<N>,
}

impl<N: MemberId> TreeTopology<N> {
    pub fn attach_child(&mut self, child: N, parent: &N) { ... }
    pub fn attach_sibling(&mut self, member: N, sibling_of: &N) { ... }
    pub fn attach_root(&mut self, member: N) { ... }
    pub fn reparent(&mut self, member: &N, new_parent: &N) { ... }
    pub fn detach(&mut self, member: &N) -> Option<DetachedSubtree<N>> { ... }
    pub fn reorder_children(&mut self, parent: &N, new_order: Vec<N>) { ... }

    /// Depth-first walk respecting expansion state.
    pub fn visible_walk<'a>(
        &'a self,
        expanded: &'a HashSet<N>,
        lens: &ProjectionLens,
    ) -> impl Iterator<Item = TreeRow<'a, N>> { ... }

    pub fn descendants(&self, member: &N) -> Vec<&N> { ... }
    pub fn ancestors(&self, member: &N) -> Vec<&N> { ... }
    pub fn siblings(&self, member: &N) -> Vec<&N> { ... }
    pub fn depth_of(&self, member: &N) -> usize { ... }
}

pub struct TreeRow<'a, N: MemberId> {
    pub member: &'a N,
    pub depth: usize,
    pub is_expanded: bool,
    pub has_children: bool,
    pub is_last_sibling: bool,
    pub graphlet_id: Option<GraphletId>,
}

/// Derive topology from a petgraph graph using edge selectors.
#[cfg(feature = "petgraph")]
pub fn derive_topology<N, E>(
    graph: &petgraph::Graph<N, E>,
    roots: &[petgraph::NodeIndex],
    selector: impl Fn(&E) -> bool,
    placement: PlacementPolicy,
) -> TreeTopology<N>
where N: MemberId { ... }

/// How derived members are placed in the topology.
#[derive(Clone, Debug)]
pub enum PlacementPolicy {
    /// Child of the node they're connected to.
    ChildOfConnection,
    /// Sibling of the node they're connected to (same parent).
    SiblingOfConnection,
    /// Child of the graphlet anchor.
    ChildOfAnchor,
}
```

### 6.6 GraphletRef — connected sub-structures

```rust
pub type GraphletId = u32;

/// A graphlet is a connected sub-structure within the GraphTree.
/// Multiple graphlets exist in a graph view — like document groups
/// in a folder. Each tracks its own binding and anchor state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphletRef<N: MemberId> {
    pub id: GraphletId,
    pub anchors: Vec<N>,
    pub primary_anchor: Option<N>,
    pub binding: GraphletBinding,
    pub kind: Option<GraphletKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GraphletBinding {
    UnlinkedSession,
    Linked { spec: GraphletSpec },
    Forked { parent_spec: GraphletSpec, reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphletSpec {
    pub kind: GraphletKind,
    pub anchors: Vec<String>,
    pub primary_anchor: Option<String>,
    pub selectors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GraphletKind {
    Ego { radius: u8 }, Corridor, Component, Loop, Frontier,
    Facet, Session, Bridge, WorkbenchCorrespondence,
}
```

### 6.7 ProjectionLens — Navigator sections as lenses

```rust
/// A lens controls which topology drives the visible tree hierarchy.
/// This replaces Navigator's separate section model — sections become
/// lenses over the same GraphTree.
///
/// The underlying membership and topology don't change; the lens
/// controls which edges drive parent-child and how members are grouped.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProjectionLens {
    /// Traversal-first: parent-child from navigation history.
    /// "I opened B from A" → B is child of A.
    /// Natural tree-style-tabs view. **Default.**
    Traversal,

    /// Arrangement-first: group by graphlet → frame → tab group.
    /// The workbench-scope view.
    Arrangement,

    /// Containment-first: group by origin/domain → url-path → member.
    /// Derived from Containment family edges (domain, url-path).
    /// Good for origin-based lifecycle management and cleanup.
    Containment,

    /// Semantic-first: group by UserGrouped/AgentDerived relations.
    Semantic,

    /// Recency-first: ordered by last-touched timestamp.
    Recency,

    /// All members: flat with optional graphlet grouping.
    All,
}

impl ProjectionLens {
    pub fn primary_edge_families(&self) -> &[&str] {
        match self {
            Self::Traversal   => &["traversal", "navigation-history"],
            Self::Arrangement => &["frame-member", "tile-member", "tab-neighbor"],
            Self::Containment => &["domain", "url-path", "user-folder"],
            Self::Semantic    => &["user-grouped", "agent-derived"],
            Self::Recency     => &[],
            Self::All         => &[],
        }
    }
}
```

### 6.8 Layout — taffy-powered

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LayoutMode {
    /// Tree-style tabs + single focused content pane. **Default.**
    /// The tree IS the navigation; one member is active at a time.
    TreeStyleTabs,
    /// Flat tab bar: warm/active members as tabs, topology-ordered.
    FlatTabs,
    /// Split panes: active members get taffy-computed rects.
    /// Supports min/max, flex grow/shrink, nested splits.
    SplitPanes,
}

#[derive(Clone, Debug)]
pub struct LayoutResult<N: MemberId> {
    pub pane_rects: HashMap<N, Rect>,       // SplitPanes
    pub tab_order: Vec<TabEntry<N>>,        // FlatTabs
    pub tree_rows: Vec<TreeRow<N>>,         // Always (powers sidebar)
    pub active: Option<N>,
}

#[derive(Clone, Debug)]
pub struct TabEntry<N: MemberId> {
    pub member: N,
    pub lifecycle: Lifecycle,
    pub is_anchor: bool,
    pub depth: usize,
    pub graphlet_id: Option<GraphletId>,
}

/// Compute layout. Tree rows always computed (sidebar in every mode).
/// Pane rects only in SplitPanes mode, via taffy.
pub fn compute_layout<N: MemberId>(
    tree: &GraphTree<N>, available: Rect,
) -> LayoutResult<N> { ... }

fn build_taffy_tree<N: MemberId>(
    tree: &GraphTree<N>, available: Rect,
) -> (taffy::TaffyTree, HashMap<taffy::NodeId, N>) { ... }
```

### 6.9 Navigation actions

```rust
/// Verbs from NAVIGATOR.md §6 + lens switching + arrangement edges.
#[derive(Clone, Debug)]
pub enum NavAction<N: MemberId> {
    Select(N),
    Activate(N),
    Dismiss(N),
    ToggleExpand(N),
    Reveal(N),

    /// Attach with placement derived from provenance.
    /// Traversal → child of source. Manual → sibling of connection.
    /// Derived → sibling of connection or child of anchor.
    Attach { member: N, provenance: Provenance<N> },

    Detach { member: N, recursive: bool },
    Reparent { member: N, new_parent: N },
    Reorder { parent: N, new_order: Vec<N> },

    SetLifecycle(N, Lifecycle),
    SetLayoutMode(LayoutMode),
    SetLens(ProjectionLens),

    CycleFocus(FocusDirection),
    CycleFocusRegion(FocusCycleRegion),
}

#[derive(Clone, Copy, Debug)]
pub enum FocusDirection { Next, Previous }

#[derive(Clone, Copy, Debug)]
pub enum FocusCycleRegion { Roots, Branches, Leaves }

#[derive(Clone, Debug)]
pub struct NavResult<N: MemberId> {
    pub intents: Vec<TreeIntent<N>>,
    pub structure_changed: bool,
    pub session_changed: bool,
}

#[derive(Clone, Debug)]
pub enum TreeIntent<N: MemberId> {
    SelectionChanged(N),
    RequestActivation(N),
    RequestDismissal(N),
    RequestFocus(N),
    ReconciliationNeeded { graphlet: GraphletId, reason: String },
    /// Emitted when lens changes — host may update edge visibility.
    LensChanged(ProjectionLens),
}

pub fn apply_nav<N: MemberId>(
    tree: &mut GraphTree<N>, action: NavAction<N>,
) -> NavResult<N> { ... }
```

### 6.10 UxTree integration

```rust
/// Emit UxNode descriptors for the accessibility/automation tree.
/// Follows the ux_tree_and_probe_spec.md contract: every visible
/// pane produces at least one UxNode when ux-semantics is active.
///
/// This gives testing/automation a structural view of the GraphTree
/// without coupling to any framework's accessibility API.
#[derive(Clone, Debug)]
pub struct UxNodeDescriptor<N: MemberId> {
    pub ux_node_id: String,
    pub role: UxRole,
    pub label: String,
    pub state: UxState,
    pub member: Option<N>,
    pub depth: usize,
    pub children: Vec<UxNodeDescriptor<N>>,
}

#[derive(Clone, Debug)]
pub enum UxRole {
    TreeView,        // the sidebar tree itself
    TreeItem,        // a member row
    TabList,         // flat tab bar
    Tab,             // individual tab
    SplitContainer,  // split pane group
    Pane,            // content pane
}

#[derive(Clone, Debug)]
pub struct UxState {
    pub focused: bool,
    pub selected: bool,
    pub expanded: Option<bool>,  // None if not expandable
    pub lifecycle: Lifecycle,
}

/// Build the UxNode tree from a GraphTree. Pure function.
/// Host bridges this to AccessKit (egui), platform a11y (mobile),
/// or DOM aria (extension/PWA).
pub fn emit_ux_tree<N: MemberId>(
    tree: &GraphTree<N>,
) -> UxNodeDescriptor<N> { ... }
```

### 6.11 Public API summary

```rust
impl<N: MemberId> GraphTree<N> {
    // --- Construction ---
    pub fn new(layout: LayoutMode, lens: ProjectionLens) -> Self;
    pub fn from_members(
        members: Vec<(N, MemberEntry<N>)>,
        topology: TreeTopology<N>,
        graphlets: Vec<GraphletRef<N>>,
        layout: LayoutMode,
        lens: ProjectionLens,
    ) -> Self;

    // --- Membership ---
    pub fn contains(&self, member: &N) -> bool;
    pub fn get(&self, member: &N) -> Option<&MemberEntry<N>>;
    pub fn member_count(&self) -> usize;
    pub fn active_count(&self) -> usize;
    pub fn cold_count(&self) -> usize;
    pub fn members(&self) -> impl Iterator<Item = (&N, &MemberEntry<N>)>;

    // --- Topology ---
    pub fn topology(&self) -> &TreeTopology<N>;
    pub fn parent_of(&self, member: &N) -> Option<&N>;
    pub fn children_of(&self, member: &N) -> &[N];
    pub fn depth_of(&self, member: &N) -> usize;

    // --- Graphlets ---
    pub fn graphlets(&self) -> &[GraphletRef<N>];
    pub fn graphlet_of(&self, member: &N) -> Option<&GraphletRef<N>>;
    pub fn graphlet_members(&self, id: GraphletId) -> Vec<&N>;

    // --- Lens & layout ---
    pub fn active_lens(&self) -> &ProjectionLens;
    pub fn layout_mode(&self) -> LayoutMode;
    pub fn active(&self) -> Option<&N>;
    pub fn is_expanded(&self, member: &N) -> bool;

    // --- Layout computation ---
    pub fn compute_layout(&self, rect: Rect) -> LayoutResult<N>;
    pub fn visible_rows(&self) -> Vec<TreeRow<'_, N>>;

    // --- Navigation ---
    pub fn apply(&mut self, action: NavAction<N>) -> NavResult<N>;

    // --- Accessibility ---
    pub fn emit_ux_tree(&self) -> UxNodeDescriptor<N>;
}
```

### 6.12 Framework adapter trait

```rust
/// Each framework implements this once. Thin — the tree does the
/// heavy lifting; the adapter just paints.
pub trait GraphTreeRenderer<N: MemberId> {
    type Ctx;
    type Out;

    fn render_tree_tabs(
        &mut self, tree: &GraphTree<N>,
        rows: &[TreeRow<'_, N>], ctx: &mut Self::Ctx,
    ) -> Self::Out;

    fn render_flat_tabs(
        &mut self, tree: &GraphTree<N>,
        tabs: &[TabEntry<N>], ctx: &mut Self::Ctx,
    ) -> Self::Out;

    fn render_pane_chrome(
        &mut self, tree: &GraphTree<N>,
        rects: &HashMap<N, Rect>, ctx: &mut Self::Ctx,
    ) -> Self::Out;
}
```

---

## 7. Migration Mapping

| Current | New (GraphTree) |
|----|-----|
| `Tree<TileKind>` | `GraphTree<NodeKey>` |
| `TileKind` enum variants | `MemberEntry<NodeKey>` with lifecycle + provenance |
| `Container::Tabs / Linear` | TreeTopology parent-child + taffy layout |
| `GraphshellTileBehavior` | `GraphTreeRenderer` impl for egui |
| `GraphletBinding` in binding spec | Per-graphlet `GraphletBinding` inside the tree |
| `FrameLayout` persistence | `GraphTree` serialized directly |
| `tile_view_ops.rs` (40 functions) | `apply_nav()` with typed NavAction |
| `tile_compositor.rs` iteration | `compute_layout()` + per-pane render dispatch |
| Navigator sidebar (separate projection) | `visible_rows()` with active lens |
| Navigator sections (Recent/Frames/etc.) | `ProjectionLens` variants |
| `FocusCycleRegion` | `CycleFocusRegion` in NavAction |
| Navigator / Workbench scope split | Eliminated — one tree, lens chooses projection |

---

## 8. Authority Boundaries

The GraphTree does NOT:

- Own graph truth (node identity, edge topology, metadata)
- Execute lifecycle transitions (host tells tree via `SetLifecycle`)
- Render content (host dispatches per-pane rendering)
- Manage GPU resources (compositor is host-owned)
- Own selection truth (emits `SelectionChanged` intent)
- Resolve viewer routing (host decides backend per pane)
- Own containment/domain edges (reads them from graph truth)
- Own arrangement edges (reads frame-member, tile-member from graph)

**The GraphTree sits at the Navigator/Workbench boundary and serves both.**
Navigator becomes a set of projection lenses over the tree. Workbench
becomes the spatial layout projection. The five-domain model is preserved:
Graph owns truth, GraphTree owns projection + arrangement, Viewer owns
realization, Shell owns orchestration.

---

## 9. Origin/Domain Grouping

Origin-based grouping for lifecycle management (e.g., "close all tabs from
example.com") works through the Containment lens:

```
[Containment lens active]
├── example.com
│   ├── /docs/api  (Active)
│   ├── /blog/post (Warm)
│   └── /about     (Cold)
├── gemini://station.smolweb
│   └── /~user/log (Active)
└── [unresolved / local]
    └── note-2026-04-10 (Warm)
```

This is NOT a separate data structure — it's the same GraphTree with the
Containment lens applied. The domain/url-path hierarchy comes from
Containment family derived edges that graph truth already computes.

Lifecycle management actions ("dismiss all from origin X") apply to the
subtree under that origin node in the containment projection. The tree
emits `RequestDismissal` intents for each affected member.

---

## 10. Arrangement-Edge Extensibility

The arrangement-edge correspondence is extensible because:

1. Edge families are **registered sub-kinds**, not hard-coded enum variants.
   New sub-kinds (`pinned-in-frame`, `active-tab`, `constellation-member`)
   can be added without changing the tree's core types.

2. `ProjectionLens::primary_edge_families()` returns a list of family tags.
   A custom lens can reference any combination of registered families.

3. `Provenance` carries the edge kind as a `String`, not a closed enum.
   New provenance sources (e.g., "constellation-thread", "feed-subscription")
   slot in without API changes.

4. The topology is rebuilt when the lens changes — different lenses see
   different parent-child structures from the same underlying edge set.

---

## 11. How Navigator Collapses Into GraphTree

Navigator was trying to be three things:

- A projection engine (derive graphlets, sections, rankings)
- A navigation surface (tree-style sidebar, breadcrumbs)
- A scope arbiter (graph scope vs. workbench scope)

GraphTree absorbs all three:

- **Projection**: `ProjectionLens` + `GraphletRef` + `visible_walk()`
- **Navigation surface**: `TreeRow` + `GraphTreeRenderer::render_tree_tabs()`
- **Scope**: eliminated. There's one tree. You pick your lens.

What remains of Navigator as a domain:

- Graphlet derivation algorithms (ego, corridor, component, etc.)
- Scoped search over graph truth
- Specialty layouts (radial, timeline) for specific graphlet kinds
- Security/permission chrome projection

These are services that *feed* the GraphTree, not a separate data model.
Navigator becomes a set of algorithms and projections that operate on
GraphTree, not a parallel universe of sections and rows.

---

## 12. Feasibility and Effort

**Core crate** (framework-agnostic tree + layout + navigation):
- Tree model + topology: ~500 lines
- Layout computation (split/tab/tree modes): ~400 lines
- Navigation actions: ~300 lines
- Serialization: ~200 lines
- **Total: ~1400 lines**

**egui adapter** (replaces current egui_tiles usage):
- Sidebar tree renderer: ~400 lines
- Split pane renderer: ~300 lines (vendor egui_tiles' proportional split math)
- Tab bar renderer: ~200 lines
- Drag-drop interaction: ~500 lines
- **Total: ~1400 lines**

**Migration from current egui_tiles usage**:
- Replace `Tree<TileKind>` with `GraphTree<NodeKey>`
- Replace `GraphshellTileBehavior` with adapter that reads GraphTree
- Preserve persistence format (backward compat migration)

### Recommended phasing

1. Core tree + UnlinkedSessionGroup mode (works like current egui_tiles)
2. Tree-style tab rendering in Navigator sidebar
3. Linked graphlet binding (automatic roster updates)
4. Reconciliation UI (fork/rebase/unlink choices)

Phase 1-2 deliver the "tree-style tabs reflecting graph structure" value.
Phase 3-4 deliver the "graphlet-native arrangement" value.

### Risk

The main risk is scope creep: the graphlet binding/reconciliation spec is
complex (§4-7 of graphlet_projection_binding_spec.md). The tile tree
replacement must implement enough binding semantics to be useful without
trying to solve all of graphlet reconciliation at once.
