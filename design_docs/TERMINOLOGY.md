# Graphshell Terminology

<!-- markdownlint-disable MD030 MD007 -->

**Status**: Living Document
**Goal**: Define canonical terms for the project to ensure consistency across code, documentation, and UI. Terms must reflect actual architectural structures, not just semantic convenience.

## Core Identity

* **Graphshell**: The product name. A local-first, spatial browser. Shell is the unconditional application host; Graph is the canonical truth domain; Navigator projects graph truth into navigable local worlds; Workbench is the invoked arrangement system; Viewer realizes requested content. See `design_docs/graphshell_docs/implementation_strategy/graph/2026-03-14_graph_relation_families.md` for the relation family model that supersedes the legacy "file-tree" metaphor.
* **Spatial Graph Browser**: The user-facing description of the interface. It emphasizes the force-directed graph and tiling window manager.
* **Knowledge User Agent**: The architectural philosophy. Unlike a passive "User Agent" that just renders what servers send, Graphshell actively crawls, indexes, cleans, and stores data on the user's behalf.
* **Verso**: The shell's routing authority for viewer/engine choice, pane ownership, and backend escalation — plus the `verso://` internal namespace. As of 2026-04-21, `crates/verso` owns the routing decisions (`select_viewer_for_content`, `resolve_route_for_content`, `VersoResolvedRoute`, `VersoPaneOwner`, `VersoRouteReason`) and consults `middlenet-engine` for Middlenet lane selection. The legacy "Verso mod" — Servo/Wry integration and local Gemini/Gopher/Finger helpers — was renamed to the `web_runtime` provider bundle (`mods/native/web_runtime/`) and is now an input *into* Verso, not the authority itself.
* **Verse**: The optional public community network for federated knowledge sharing. Long-horizon research. The public, community, federated layer. Distinct from Verso's local collaboration.

## Projection Concepts

Projection is the cross-cutting pattern for deriving a representation in one domain from truth/substrate in another. Most architectural layers in Graphshell are connected by named projections; this section defines the umbrella concept, a word-form convention that resolves ambient ambiguity, and the canonical mechanisms that implement projections.

* **Projection** (umbrella): A pure function across a domain boundary — taking truth or substrate in one domain and producing a representation in another. Every named "projection" in Graphshell (Navigator projection, Cartography projection, contribution projection, branch projection, `AggregatedEntryEdgeView`, the Projection Rule's "nodes project as tiles," Viewer resolution) is an instance. A projection has a **source domain**, a **target representation shape**, a **specification** (config), and **refresh rules**. Projections are derivations, never owned truth — the source domain remains authoritative.
* **Three-form convention** (linguistic discipline):
    * **projection** (noun) — the *pattern* / rule / named function. "Navigator projection," "Cartography projection" are pattern names.
    * **projected X** (adjective+noun) — the *outcome* produced by applying the pattern. "Projected tree," "projected aggregate," "projected view."
    * **projecting** / **to project** (gerund/verb) — the *process* of applying the pattern. "Nodes project as tiles," "projecting runs on refresh."
    Never use **projection** bare in specs when the domain is not obvious — always prefer `X projection` or `projection of Y into Z`. Mechanism names (projection pipeline, projection spec, `ProjectionLens`) are compound nouns and sit alongside the three-form split.
* **Projection family** — the set of projections sharing a source domain or target representation. Navigator projections, Graph projections, Cartography projections, memory-substrate projections are each a family.
* **Domain projection matrix** — the catalog of named projections across domain pairs. Canonical doc: `graphshell_docs/technical_architecture/domain_projection_matrix.md`.
* **Projection Rule** (specific instance) — the Graph→Workbench correspondence projection (nodes→tiles, graphlets→tile groups, frames→frames). See Tile Tree Architecture §Projection Rule. One named instance of the umbrella concept.
* **Projection pipeline** (mechanism) — the Navigator's five-stage pipeline (Scope → Shape → Annotation → Presentation → Portal) that produces any projected Navigator view. Canonical spec: `graphshell_docs/implementation_strategy/navigator/navigator_projection_spec.md`. Producing-plan history: `archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md`.
* **Projection spec** (mechanism) — a config struct parameterizing one pipeline instance. Declares scope strategy, shape mode, annotation stack, cost class, layout inheritance.
* **ProjectionLens** (mechanism) — a Rust enum in the `graph-tree` crate that parameterizes the Shape stage for tree-family projections. Variants select which edge family drives parent-child (Traversal, Arrangement, Containment, Recency, etc.). Not a separate projection pattern — a concrete Shape-stage implementation for tree-shaped outputs. Non-tree shapes (graphlet-as-graph, time-axis, summary/minimap) require their own Shape-stage mechanisms.

### Projection Vocabulary (Recipe / Instance / Bucket)

This vocabulary tightens loose usage of "swatch," "minimap," "tree," and "log"
across navigator and shell docs. It is host-agnostic and does not depend on
which UI framework realizes the surfaces.

* **Graph authority** — the canonical truth domain for nodes, edges, identity,
  and durable structure. Owned by the Graph domain. Nothing else owns graph
  truth; projections derive from it.
* **Graph capacity** — any analysis or transformation that the graph layer can
  perform: graphlet derivation, relation filtering, layout selection, scene
  representation choice, hit testing, highlighting, focus, simulation, semantic
  projection, memory/activity overlay. Capacities apply equally to the main
  graph canvas and to scoped instances elsewhere.
* **Projection recipe** — a configured composition of: a graph analysis
  transform (scope/filter/derivation), a scene representation, a layout, and a
  render profile. A recipe is the durable description; a run of it is ephemeral.
  See `2026-04-03_layout_variant_follow_on_plan.md` Conceptual Model for the
  algorithm/scene/backend/render-profile split.
* **Canvas instance** — a renderable graph surface produced by running a
  projection recipe. The main graph canvas is one canvas instance among many;
  swatches are scoped canvas instances. All canvas instances use the same graph
  capacities, scaled to their render profile.
* **Navigator swatch** — a compact, navigator-scoped canvas instance. Not a
  thumbnail and not a separate mini-graph; a live or cached projection of graph
  truth through a particular recipe. Swatch state (hover, scaffold selection,
  viewport) is projection-local; mutation goes uphill via intents.
* **Tree spine** — a scannable orientation projection. Provides traversal
  hierarchy, frametree, graphlet sections, lens-driven containment views, and
  affordances for active-node location, expansion, badges, and cross-edge
  indicators. The tree spine is a presentation bucket, not the Navigator's
  whole identity.
* **Activity log** — a temporal projection covering active nodes, recently
  active (now-inactive) nodes, warmed/cold-but-relevant nodes, opened/closed/
  revealed/promoted events, graph mutations, navigation transitions, memory
  branch changes, and import events. Subsumes what older docs called the
  "Recent" and "Import Records" sections.
* **Presentation bucket** — one of the three canonical Navigator presentation
  shapes: **Tree Spine**, **Swatch**, **Activity Log**. The Navigator's job
  composes these three buckets; specific named projections (containment,
  recency, ego graphlet, frontier, frametree, etc.) are recipes that land in
  one of the three buckets. See `navigator/NAVIGATOR.md §8`.
* **Promotion** (projection sense) — turning an ephemeral projection result
  into graph or shell or workbench truth via an explicit intent. A swatch may
  produce a promotion intent ("save this graphlet," "apply this layout to the
  main canvas," "persist this projection recipe"); the receiving authority
  decides whether to commit. The pane-presentation sense of Promotion (Pane →
  Tile) is a specific instance of this pattern at the workbench layer.
* **Uphill rule** — any change initiated from a projection (host row, swatch,
  graphlet view, specialty layout) routes uphill to the relevant authority
  (graph, shell, workbench, runtime, history) and is presumed ephemeral until
  that authority promotes it. Projection-local hover, scaffold selection,
  viewport, expansion, and filter state remain projection-local; identity,
  structure, arrangement, and durable state never do. See
  `navigator/NAVIGATOR.md §4`.

## Settings and Permissions Spine

Added 2026-04-30. Five-scope hierarchy for layered settings and permission
grants: **default → persona → graph → view/tile → pane**. Reads walk
narrowest-to-broadest; writes target an explicit scope; permissions narrow
but never widen across scopes. Canonical spec:
`graphshell_docs/implementation_strategy/aspect_control/settings_and_permissions_spine_spec.md`.

* **Persona** (added 2026-04-30): The top-level user-identity scope above
  graph. A persona owns 1..N graphs, plus identity material (Verse / Nostr
  / Matrix keys), persona-default theme, keybindings, and permissions.
  Supersedes the egui-era "Profile" concept (which was one-graph-per-profile);
  the new model decouples user-identity from graph and admits multi-graph
  personas. Per-persona settings live under
  `{config_dir}/graphshell/personas/{persona_id}/settings/` in a
  `cosmic-config`-shape layered key-value store.
* **Settings Scope** (added 2026-04-30): One of `default | persona | graph |
  view/tile | pane`. The active scope path for a surface determines how
  setting reads resolve. Each scope has a canonical persistence backing
  per the spine spec.
* **Intent Idempotence + Replay Contract**: see Data Model §Intent for the
  canonical statement; settings writes satisfy this contract for crash
  recovery and sync.

## Tile Tree Architecture

The layout system is the per-Workbench arrangement of **Panes** within a Frame slot. A Pane is a spatial leaf in a Frame's split tree (Shell-owned per [SHELL.md §3](graphshell_docs/implementation_strategy/shell/SHELL.md)); each Pane shows graph nodes (active tiles of a graphlet, or a canvas instance of the graph). Not every visible surface is a tile — toolbars, omnibar, and tool panes are not tiles.

**Status note (2026-04-29 refactor):** the current code lives on `egui_tiles::Tree<TileKind>`, with an ephemeral-vs-promoted distinction (`TileKind::Pane(PaneState)` ↔ `TileKind::Node(NodePaneState)`) gated by **Promotion**. The canonical model below describes the post-iced-jump-ship target per [`shell/2026-04-28_iced_jump_ship_plan.md`](graphshell_docs/implementation_strategy/shell/2026-04-28_iced_jump_ship_plan.md) §4.4–§4.5: every Pane shows graph nodes; there is no ephemeral non-citizen Pane state; **Promotion** and **Demotion** retire as workbench-citizenship operations. Egui-era usage of these terms persists in legacy code only and is tracked in §Legacy / Deprecated Terms.

### Projection Rule

Graphshell intentionally keeps **graph identity terms** separate from **workbench presentation terms**.

* A **Node** is graph-semantic identity/state.
* A **Tile** is the workbench presentation/container that hosts a node-bearing or graph-view-bearing leaf.
* A **Graphlet** is a meaningful bounded graph subset produced by an active
    edge projection, filter, algorithm, or traversal rule. Graphlets are usually
    projection-derived rather than permanently fixed durable objects, though a
    graphlet may later be promoted into a named saved structure.
* A **Graphlet Anchor** is a node designated as one of the defining anchors of a
    graphlet. A graphlet may have multiple anchors and may optionally designate a
    **Primary Graphlet Anchor** when one node acts as the graphlet's current core
    for local ranking and derivation emphasis. Graphlet anchor state is distinct
    from spatial pin state. The system may suggest a primary anchor from strong
    signals such as pin state and repeated graphlet-local use, but suggestion does
    not by itself assign the role.
* A **Graphlet Backbone** is the graphlet-local set of semantic and traversal
    relations treated as the most explanatory or structurally central connections
    for the active graphlet, usually around its primary anchor. Backbone is a
    graphlet-local salience policy, not a separate global edge family.
* A **Graphlet Migration Proposal** is the explicit proposal emitted when a user
    drags a node from one anchored graphlet context toward another. It is a
    high-signal gesture that may resolve to `Move`, `Associate`, `Copy`, or
    `Cancel`; it is not itself an automatic graph truth rewrite.
* A **Tile Group** is the workbench presentation of a graphlet-aligned or
  graphlet-adjacent arrangement. A tile group may be explicitly **linked** to a
    graphlet definition or left **unlinked** as a session arrangement pending an
    explicit save or graphlet-fork decision.

Canonical projection law:

* nodes **project as tiles** in workbench chrome
* graphlets **project as tile groups** in workbench chrome
* frames **project as frames** across graph, navigator, and workbench presentations

This is a presentation correspondence, not a term collapse. A node can exist without a tile; a tile is not the canonical owner of node identity.

### Primitives

* **Tile**: An active rendering of a graph node inside a Pane. A graphlet contains many graph nodes; a tile pane shows the **active** subset (per the Active/Inactive presentation states defined below). Tiles are not separate identity from nodes — a tile *is* a node in its active rendered form. (Egui-era code uses `TileKind` enum variants to distinguish content kinds; that is an implementation detail of the egui host and does not change tile semantics.)
* **Pane**: A leaf in a Frame's split tree (Shell-owned). A Pane carries a `GraphletId` and a **pane type**, and shows graph nodes scoped to that graphlet:
    * **Tile pane** — renders the active tiles of the Pane's graphlet, with a tab bar over them (one tab per active tile).
    * **Canvas pane** — renders a canvas instance scoped to the graphlet (or to the full graph or a query result). See Projection Vocabulary §canvas instance.
    Every Pane shows graph nodes; there is no non-citizen ephemeral Pane state. Switching a Pane's `GraphletId` retargets it; toggling its pane type switches between tile and canvas rendering. (Egui-era `TileKind::Pane(PaneState)` ephemeral leaves are a legacy shape; see §Legacy.)
* **Split**: An H/V split container — internal node in a Frame's split tree. Carries a split axis (`Horizontal` for top↔bottom regions, `Vertical` for left↔right regions), child ordering, and **Shares** for proportional space allocation. Resizable via drag handles; nestable to arbitrary depth. Iced realization: `pane_grid::State<Pane>`. Egui-era realization: `egui_tiles::Container::Linear`. The abstract concept (H/V container) is host-neutral.
* **Tab** (**UI affordance term**): A visual selector control for choosing the active tile among the active tiles of a tile pane. Canonical structural term is **Tile**; "tab" is presentation shorthand only. Iced realization: `iced_aw::Tabs` inside a tile Pane. Egui-era realization: `Container::Tabs` Tab Group wrappers around promoted tiles.
* **Shares**: Per-child `f32` weights within a Split that determine proportional space allocation. Default share is `1.0`.

The following are **egui-era host primitives** retained for the egui codebase; they are not part of the post-iced canonical model. The iced host expresses these through `pane_grid` directly.

* **Container** (egui-era): A branch tile in `egui_tiles::Tree`. Three structural types: Tab Group (`Container::Tabs`), Split (`Container::Linear`), Grid (`Container::Grid`). The post-iced model uses Frame split tree + tile-pane tab bars instead.
* **Tab Group** (egui-era): An `egui_tiles::Container::Tabs` wrapper. In the post-iced model, tab grouping lives inside a tile Pane, not as a separate container kind.
* **Grid** (egui-era): An `egui_tiles::Container::Grid` 2D matrix container. Not part of the post-iced canonical model; use nested Splits or a custom canvas pane instead.

### Composition Rules

Canonical (host-neutral):

* **Arbitrary nesting**: Splits can hold other Splits to any depth. Cross-direction nesting forms complex layouts.
* **Cross-direction nesting preserved**: A `Horizontal` Split inside a `Vertical` Split (or vice versa) is never collapsed.
* **Closing a Pane**: Removing a Pane collapses its parent Split if one child remains; the surviving sibling expands.

Egui-era specifics (egui_tiles host only):

* **Same-direction merging** (egui-era): A `Horizontal` Split nested directly inside another `Horizontal` Split is automatically absorbed (children promoted, shares recalculated). Controlled by `join_nested_linear_containers: true`. Iced `pane_grid` does not merge.
* **Simplification** (egui-era): The tree is simplified every frame. Empty containers are pruned, single-child containers are collapsed (except Tab Groups wrapping a lone promoted tile, which are kept for the tab strip). Controlled by `SimplificationOptions`. Iced `pane_grid` collapses on Pane removal explicitly, not via per-frame simplification.
* **SimplificationSuppressed** (egui-era): A flag set on a Split container for the lifetime of any ephemeral pane (`TileKind::Pane`) it directly hosts. Bridges egui_tiles' simplification behavior over the egui-era ephemeral-pane lifecycle. Not part of the post-iced canonical model (no ephemeral panes).

### Composite Structures

* **Tile Tree**: The complete recursive structure of Tiles forming the layout. Backed by a flat `Tiles<TileKind>` hashmap keyed by `TileId`, plus a root `TileId`. Code: `egui_tiles::Tree<TileKind>`, stored as `Gui::tiles_tree`.
* **App Scope**: The top-most global scope for a running Graphshell process. App Scope owns workbench switching/navigation.
* **Workbench**: A graph-bound arrangement container paired to one complete graph dataset (`GraphId`). It hosts a Tile Tree (`Tree<TileKind>`) and owns tile/pane lifecycle within that tree. The workbench is a contextual presentation layer, not the semantic owner of nodes, graphlets, or `GraphViewId`. A workbench is composed *into* a Frame by the Shell; the workbench does not own frame composition or frame switching. Graph-scoped Navigator hosts may name active graph/view targets one UI level above workbench hosting; workbench-scoped Navigator hosts may project arrangement chrome and focused-pane status. Actionable viewer controls remain tile-local when a pane is presented as a tiled workbench citizen.
* **Workbench Scope**: The full, unscoped graph domain of one Workbench (`GraphId`-bound).
* **Frame**: A Shell-owned working context that composes one or more Workbenches into a single arrangement and preserves their state as a unit. The trivial case is one Frame containing one Workbench; richer frames may contain multiple Workbenches (different `GraphId`s, side-by-side or split). Frame is the canonical runtime/UI term for top-level working contexts. Ownership: Shell composes/switches Frames; Workbench provides the per-graph tile tree inside a Frame; Graph backs frame membership through `ArrangementRelation` edges; Navigator projects the frametree as part of its Tree Spine bucket. Frames **project as frames** across graph, navigator, and workbench presentations.
* **Frame Snapshot** (**Persistence Snapshot**, canonical storage term): A persistable snapshot of a Frame's composed Workbench layouts plus their content manifests. Serialized as `PersistedFrame`, which contains:
    * `FrameLayout` — the `Tree<PersistedPaneTile>` shape for each composed workbench
    * `FrameManifest` — the pane-to-content mapping and member node UUIDs
    * `FrameMetadata` — timestamps for creation, update, last activation
    Frame Snapshot is the canonical save/restore/storage term; Frame remains the primary runtime/UI container term. Save/restore is a Shell responsibility because Frame composition is Shell-owned.

### Pane Types

* **Graph View**: A Pane (`TileKind::Graph`) containing a force-directed canvas visualization powered by `egui_graphs`. Renders the `Graph` data model with physics simulation, node selection, and camera controls.
* **GraphViewId**: A stable identifier for a specific Graph View pane instance. `GraphViewId` is the canonical identity for per-view camera state, `ViewDimension`, Lens assignment, and `LocalSimulation` (for Divergent layout views). Generated at pane creation; persists across reorder, split, and move operations.
    A `GraphViewId` may be hosted in a tile tree surface, but tile hosting does not become the owner of that graph-view identity.
* **GraphLayoutMode**: The layout participation mode for a Graph View pane. `Canonical` — participates in the shared workspace graph layout (shared node positions, one physics simulation). `Divergent` — has its own `LocalSimulation` with independent node positions; activated explicitly by the user.
* **LocalSimulation**: An independent physics simulation instance owned by a `Divergent` Graph View. Does not affect Canonical pane node positions.

### Tile Presentation States (per-graphlet)

Each node in a graphlet has one of two **presentation states** that determine
whether its tile renders in any tile pane bound to that graphlet. State is
**per-graphlet** — the same node has the same Active/Inactive state across
every Pane that shows that graphlet. The Navigator is the surface for
discovering and toggling activation.

* **Active** (presentation): The node's tile renders in any tile pane
  bound to its graphlet. The tile is interactive; its viewer pass is
  alive. Activating an Inactive node opens its tile without touching
  graph truth.
* **Inactive** (presentation): The node remains in the graphlet but its
  tile is not shown. Inactive nodes are accessible via the Navigator's
  Tree Spine (per [NAVIGATOR.md §8](graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md))
  and can be activated at will. Inactive is the default for nodes that
  have not been opened in the current session and for nodes whose tiles
  the user has explicitly closed.

**Naming-collision note.** "Active" appears in two unrelated state
machines with different meanings:

* **Active (presentation)** — the per-graphlet state defined here:
  *the node's tile is shown in this graphlet's tile panes*. Toggled by
  Close tile / activate-from-Navigator. Owned by runtime lifecycle as
  presentation state, projected by Navigator.
* **Active (Runtime Lifecycle)** — the four-state node lifecycle
  (Active → Warm → Cold → Tombstone) defined in §Runtime Lifecycle:
  *the node has a live webview and is rendering*. Toggled by lifecycle
  reconciliation, not by user toggling.

The two axes are orthogonal: a node Active (presentation) typically also
needs to be Active or Warm (lifecycle) for its viewer to render
content. A node Inactive (presentation) can be in any lifecycle state.
Where a doc or code site needs disambiguation, prefer
`PresentationState::Active` vs `Lifecycle::Active`.

### Tile and Graphlet Operations

The three distinct operations on a node:

| Operation | Domain | Effect | Weight |
|---|---|---|---|
| **Close tile** | runtime lifecycle | Node → Inactive in this graphlet. Graph unchanged. | Safe; trivially reversible (re-activate from Navigator) |
| **Remove from graphlet** | Graph (organizational) | Node leaves this graphlet's membership; node persists in the full graph and in any other graphlets it belongs to. | Deliberate edit; reversible by re-adding |
| **Tombstone** | Graph (node lifecycle) | Node marked deleted in graph truth (see Runtime Lifecycle). Cascades to edges. | Destructive; confirmation required; reversible only via Ghost Node restore |

**Close tile** is the deactivation path: it changes presentation state, not
graph state. **Remove from graphlet** is the canonical organizational
graph edit: it changes which graphlet a node belongs to without touching
the node itself. **Tombstone** is the destructive operation; see Runtime
Lifecycle for Ghost Node semantics.

### Address-as-Identity (routing principle)

A node's graph identity is its canonical address. The address-resolution
contract: *given an address, look up the node in the graph by that
address.* No separate mapping structure exists. Every Pane shows nodes
whose addresses resolve in the graph; routing decisions (which viewer to
attach, which tool to invoke) consult the address. The graph's node set
is the authoritative membership list.

(The egui-era version of this principle framed it as a "graph
citizenship test" gating Pane vs Tile lifecycle. That gating retires
with Promotion/Demotion; the address-as-identity principle survives as
the routing/lookup contract.)

### Verso internal address scheme

Internal pane content uses a `verso://` address namespace so that internal
nodes follow the same address-resolution rule as web content nodes. Canonical
forms:

* `verso://view/<GraphViewId>` — a Graph View canvas-pane node.
* `verso://tool/<name>` — a tool/subsystem pane node. If multiple instances
  of the same tool are open simultaneously, a numeric discriminator is
  appended: `verso://tool/<name>/<n>` (starting at 2; the discriminator is
  recycled upon pane closure).
* `verso://frame/<FrameId>` — a Frame node. Frame nodes carry
  `ArrangementRelation` / `frame-member` edges to all member tile nodes;
  on the canvas, a named Frame is rendered as a titled, colored backdrop
  (legacy term: MagneticZone) attracting its member nodes via
  `FamilyPhysicsPolicy.arrangement_weight`.
* `verso://settings/<section>` — a Settings surface route.
* `verso://clip/<uuid>` — an internal clip node address.

Legacy `graphshell://...` forms remain accepted as compatibility aliases
during migration. Tool and settings address schemes follow the same
discriminator pattern.

### Pane Chrome and Locking

* **Pane Presentation Mode** (aka **Pane Chrome Mode**): How a Pane is presented in the workbench (chrome density, mobility, locking behavior), distinct from the Pane's content. Reduced chrome (docked) vs. full chrome (free-floating) are presentation choices; both apply to any Pane regardless of its `GraphletId` or pane type.
* **Docked Pane**: A Pane presented with reduced chrome and position-locked behavior inside the current arrangement. Intended to reduce accidental reflow and focus attention on content.
* **PaneLock**: The reflow lock state of a Pane, independent of `PanePresentationMode`. `Unlocked` (default) — all user-initiated reflow operations permitted. `PositionLocked` — cannot be moved or reordered; can be closed. Docked panes are implicitly position-locked from the user's perspective. `FullyLocked` — cannot be moved, reordered, or closed by the user; reserved for system-owned panes.
* **FrameTabSemantics** (egui-era): An optional semantic overlay on top of the `egui_tiles` structural tree. Its role is to persist semantic tab group membership so that meaning is not lost when `egui_tiles` simplification restructures the tree. It serializes into the frame bundle as frame state, not WAL data, and consumers must tolerate the field being absent during rollout. Not part of the post-iced canonical model — the iced host uses `iced_aw::Tabs` inside tile Panes and does not require a separate semantic overlay.
* **TabGroupMetadata** (egui-era): A record within `FrameTabSemantics` for one semantic tab group. Contains `group_id` (`TabGroupId`), ordered `pane_ids`, and `active_pane_id` (repaired to `None` if the previously active pane is removed from the group). Same egui-era status as FrameTabSemantics.
* **Subsystem Pane**: A pane-addressable surface for a subsystem's runtime state, health, configuration, and primary operations. Subsystems are expected to have dedicated panes, but implementations may be staged. Subsystem panes are hosted as tool panes. (Egui-era code path: `TileKind::Tool(ToolPaneState)`; iced realization: a tile Pane with a tool-pane type variant.)
* **Tool Pane**: A non-document pane host (e.g., Diagnostics today; History Manager, subsystem panes, settings surfaces over time). Tool panes may be subsystem panes or general utility surfaces. (Egui-era code path: `TileKind::Tool(ToolPaneState)`.)
* **Diagnostic Inspector**: A subsystem pane (currently the primary `ToolPaneState` implementation) for visualizing system internals (Engine, Compositor, Intents, and future subsystem health views).

### Surface Composition

* **Surface Composition Contract**: The formal specification of how a node viewer pane tile's render frame is decomposed into ordered composition passes (UI Chrome, Content, Overlay Affordance), with backend-specific adaptations per `TileRenderMode`.
* **Composition Pass**: One of three ordered rendering phases within a single node viewer pane tile frame: (1) UI Chrome Pass, (2) Content Pass, (3) Overlay Affordance Pass. Pass ordering is Graphshell-owned sequencing and must not rely on incidental egui layer behavior.
* **CompositorAdapter**: A wrapper around backend-specific content callbacks (for example Servo `render_to_parent`) that owns callback ordering, GL state isolation, clipping/viewport contracts, and the post-content overlay hook.
* **TileRenderMode**: The runtime-authoritative render pipeline classification for a node viewer pane tile: `CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, or `Placeholder`. Resolved from `ViewerRegistry` at viewer attachment time and used for compositor pass dispatch.

## Interface Components

*   **Shell**: The system-oriented command interpretation and control domain. The Shell translates user intent into operations dispatched to the correct authority (graph intents to Graph, workbench intents to Workbench, scope signals to Navigator). It owns command dispatch, top-level composition, aspect exposure, settings surfaces, subsystem control, and app-scope chrome. The Shell is also the application's only host and the orchestration boundary for user intent and app-level control; it is not the semantic owner of graph truth, pane truth, or content truth. Canonical doc: `graphshell_docs/implementation_strategy/shell/SHELL.md`.
*   **Omnibar**: The primary global navigation/input bar for location, search, and command entry. The omnibar straddles the Shell/Navigator boundary: the Shell owns its input/command interpretation and dispatch side; the Navigator owns the contextual graph-position breadcrumb display within it. It typically lives in a graph-scoped toolbar Navigator host, but host edge and form factor are layout policy rather than semantic ownership. See `graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md §11`.
*   **Navigator Host**: An edge-mounted chrome host that renders Navigator semantics. Each host has an anchor edge, form factor, scope, and cross-axis margins. Hosts may project graph scope, workbench scope, both, or auto-switch behavior. Canonical doc: `graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md §11`.
*   **Graph-scoped Navigator Host**: A Navigator host currently projecting graph scope, such as active graph/view identity, graph commands, and graph-level status. This replaces the older assumption that one fixed top bar owns all graph chrome.
*   **Workbench-scoped Navigator Host**: A Navigator host currently projecting workbench scope, such as frame/tree structure, arrangement controls, and focused-pane status badges. It is a structural/workbench-management surface, not the primary command owner for viewer-local Back/Forward/Reload/Zoom chrome. This replaces the older assumption that one fixed sidebar owns all workbench chrome.
*   **Graph Bar**: Legacy preset name for a toolbar-form Navigator host projecting graph scope. Retired as a fixed surface type; may remain as a user-facing preset label or historical reference.
*   **Workbench Sidebar**: Legacy preset name for a sidebar-form Navigator host projecting workbench scope. Retired as a fixed surface type; may remain as a user-facing preset label or historical reference.
*   **Workbar**: Legacy term — superseded by Navigator hosts and their graph/workbench scope presets. Do not use in new code or docs.
*   **History Manager**: The canonical non-modal history surface with Timeline and Dissolved tabs, backed by traversal archive keyspaces.
*   **Settings Pane**: A tool pane that aggregates configuration and controls across registries, subsystems, and app-level preferences. A settings pane may host subsystem-specific sections or summon dedicated subsystem panes.
*   **Control Panel**: The async coordination/process host for background workers and intent producers within The Register. In architectural terms it is an **Aspect** (runtime coordination concern), not a UI Surface. It supervises worker lifecycles and intent ingress, but does not own or render panes/surfaces directly; subsystem UI appears through dedicated tool/subsystem panes. Code-level: `ControlPanel` (supervised by `RegistryRuntime`).
*   **Lens** (revised 2026-04-30): A named **filter** configuration over graph truth (visibility, edge-family masks, query predicates). Lens is **not** a composite that aggregates Theme, Physics Profile, or Layout — those stand as their own first-class configurations and compose with Lens at the Workflow level, not under it. Earlier drafts framed Lens as `Layout × Theme × Physics × Filter`; that framing is retired. Treat `Lens` as a synonym for `Filter` in code and UI copy. Do not reuse **Lens** as a substitute term for Viewer, graph projection, pane identity, or domain naming.
*   **Habit** (added 2026-04-30): A **per-node** attribute that defines how the node spreads/grows when participating in a derived graphlet. A node's Habit is a node setting (like a tag); when graphlets derive from a node or include it as an anchor, the node's Habit informs the local spread/layout pattern around it. Examples: a feed-node's Habit is "stream-spread" (its derivations expand into a temporal feed), a corridor-node's Habit is "path-spread", a cluster-anchor's Habit is "radial-spread". Habit replaces the egui-era framing where `Layout` was a single per-view choice; the per-view layout still exists at the global scene level (the four-tier model in [`graph/2026-04-03_layout_variant_follow_on_plan.md`](graphshell_docs/implementation_strategy/graph/2026-04-03_layout_variant_follow_on_plan.md)) but is composed from contributing nodes' Habits rather than dictated globally. Working term — may be renamed (alternatives considered: Bloom, Form, Disposition); the role is canonical even if the spelling shifts.
*   **Command Palette**: A modifiable context menu that serves as an accessible interface for executing Actions.
*   **The Register**: See *Registry Architecture* section below for the canonical definition (this interface-components mention is intentionally a cross-reference only to avoid duplicate-definition drift).
*   **Camera**: The graph viewport state (pan offset, zoom level) for a Graph View/Frame pane. Camera semantics and interaction policy are Graph-owned; concrete camera state is stored per view in workbench/view state, can be persisted with that view state, and is hydrated into runtime render state when the view is live. Camera is never a single global layout authority.

## View Dimension Terms

*   **ViewDimension**: The per-Graph-View dimension mode state. Canonical values are `TwoD` and `ThreeD { mode, z_source }`. `ViewDimension` is persisted as part of Graph View state.
*   **ThreeDMode**: The 3D interaction/render sub-mode for a `ViewDimension::ThreeD` Graph View. Canonical values are `TwoPointFive`, `Isometric`, and `Standard`.
*   **ZSource**: The policy for deriving per-node `z` placement when a Graph View is in `ThreeD` mode. `ZSource` configuration is persisted through `ViewDimension`; derived per-node `z` positions are runtime data.
*   **Derived Z Positions**: Ephemeral per-node `z` values computed from `ZSource` and node metadata during 2D→3D transitions. Derived `z` values are never persisted independently.
*   **Dimension Degradation Rule**: If persisted `ThreeD` state is restored where 3D rendering is unavailable, Graphshell deterministically degrades that view to `TwoD` while preserving `(x, y)` positions.

## Camera Commands

*   **Camera Fit**: Fits the viewport to the bounding box of all nodes with a relaxed zoom factor. Triggered by `C` key or on startup with an existing graph.
*   **Focus Selection**: Fits the viewport to the bounding box of the selected nodes with tighter padding. Triggered by `Z` key when 2+ nodes are selected.
*   **Wheel Zoom**: Zoom in/out via mouse wheel, trackpad two-finger scroll, or smooth-scroll delta. Pointer-relative (zooms toward cursor position). Configurable via `scroll_zoom_requires_ctrl` setting.

## Data Model

*   **Graph**: The persistent data structure containing Nodes and Edges. Acts as the "File System".
*   **GraphId**: A stable identifier for a graph dataset bound to a Workbench context.
*   **Graph Scope**: A bounded render/query scope used by a pane/frame within one Workbench Scope (for example region/filter subsets of the Workbench graph).
*   **Scope Isolation**: Distinct graph scopes rendered in separate panes/frames within the same Workbench (`GraphId`) are interaction-isolated by default; selection, camera, gestures, and scope-local interactions do not implicitly affect sibling scopes unless an explicit bridge/sync rule is enabled.
*   **Inter-Workbench Scope**: App-level scope used to switch between workbenches (and therefore between complete graphs/`GraphId`s).
*   **Node**: A unit of content (webpage, note, file, or internal surface) identified by a stable UUID and a canonical address. Graph citizenship is determined by address: a tile is in the graph if and only if its address resolves to a live (non-tombstone) node. Nodes are created by **Promotion** — when a Pane in an ephemeral opening mode is promoted to `Tile`, its address is written into the graph as a new Node. Internal surfaces (Graph Views, tool panes, frames) get `verso://` addresses at creation time and are always graphed. Nodes are never created by merely opening a Pane in an ephemeral mode. See *Promotion* and *Address-as-Identity principle* in the Tile Tree Architecture section.
*   **Edge**: A durable relationship record between two nodes, represented as `EdgePayload`. Edges are asserted either (a) by Tile-to-Tile navigation (immediate, `NavigationTrigger::PanePromotion`), or (b) by Promotion of a pane that was sourced from an existing Tile — the pending source `NodeKey` recorded at Pane Open time resolves into an edge at promotion (`NavigationTrigger::PanePromotion`). Opening a Pane from a Tile in an ephemeral mode does not assert an edge but records a pending relationship that enables (b). Frame nodes hold edges to all member tile nodes; those edges drive canvas clustering. On **Demotion**, edges from the demoted node are tombstoned alongside the node and can be restored if the node is re-promoted.
*   **EdgePayload**: The canonical edge projection data type reduced from structural assertions and traversal events. Replaces the deprecated `EdgeType`.
*   **EdgeKind**: The structural classification of an edge projection. Existing kinds: `UserGrouped` — explicit connection created by the user; `TraversalDerived` — relationship state asserted when at least one traversal event exists for the node pair; `AgentDerived` — inferred by an `AgentRegistry` agent, subject to time-decay. Forthcoming additive kinds (not yet in code): `ContainmentRelation`, `ArrangementRelation`, `ImportedRelation` — see `canvas/2026-03-14_graph_relation_families.md` for the full relation family vocabulary. Note: `Hyperlink` is a fourth existing kind (`EdgeKind::Hyperlink`) recorded on link-follow navigation. All kinds may coexist on a single edge pair via `EdgePayload.kinds: BTreeSet<EdgeKind>`.
*   **Traversal**: A directed temporal navigation event (`timestamp`, `NavigationTrigger`, direction) in the traversal event stream. Traversals are appended and then projected into edge state (`EdgePayload`) for rendering and inspection.
*   **NavigationTrigger**: The cause of a `Traversal`. Canonical values: `LinkClick`, `BackButton`, `ForwardButton`, `AddressBarEntry`, `Programmatic`, `PanePromotion`, `Unknown`. `PanePromotion` is emitted when an edge is asserted at promotion time — either by direct Tile-opening-mode navigation, or by promotion of a pane that was sourced from an existing tile.
*   **Edge Traversal History**: The aggregate traversal event stream over node pairs, forming the complete navigation history of the graph.
*   **Edge Direction Summary**: A render-time derived dominant direction computed from traversal records/metrics; not an identity field on the edge.
*   **Workbench History Stream**: The ordered stream of workbench-structure operations (tile/frame/split/reorder/open/close) within one workbench context.
*   **Frame History**: A merged timeline over Edge Traversal History and Workbench History Stream for frame-contextual replay/inspection.
*   **Intent**: A data payload describing a desired state change routed through an explicit mutation authority. Graphshell uses two canonical intent authorities: `GraphReducerIntent` for reducer-owned semantic graph mutations and `WorkbenchIntent` for workbench-authority tile-tree/layout mutations.
*   **GraphReducerIntent**: A reducer-authority mutation request consumed by `apply_reducer_intents()` and reduced into durable graph semantics (nodes/edges/selection/lifecycle/history). It is the canonical reducer boundary type.
*   **WorkbenchIntent**: A workbench-authority mutation request (tile-tree/pane layout operations) handled in the frame loop before reducer application. `WorkbenchIntent` is concrete runtime authority, not a conceptual/migration-only routing class.
*   **Intent Idempotence + Replay Contract** (added 2026-04-30): All canonical Intents (`GraphReducerIntent`, `WorkbenchIntent`, and any future `HostIntent` family) **must** satisfy two contracts:
    1. **Idempotent application**: applying the same Intent twice produces the same final state as applying it once. Practical examples: `OpenNode { node_key }` for an already-Active node leaves it Active without duplicating viewer or edge state; `TagGraphlet` with an already-present tag is a no-op; `CloseTile` on an already-Inactive tile is a no-op. Where idempotence requires correlation (e.g., "create node N if not present"), the Intent carries the necessary identity/keying for the receiving authority to detect the duplicate.
    2. **Deterministic replay**: replaying a sequence of applied Intents from the WAL against an empty initial state produces the same end state, modulo timestamps and identity-allocations that are themselves WAL-recorded. The receiving authority is free to assert order; replay never produces a different graph than the original session.
    These contracts are the basis for crash recovery, sync (via `SyncedIntent`), undo/redo, and shell-state restore. New Intent variants must satisfy both contracts at design time; surfacing a non-idempotent or non-replayable mutation requires explicit deviation justification in the spec adding it. The sanctioned-writes test (per [`shell/2026-04-28_iced_jump_ship_plan.md` §5](graphshell_docs/implementation_strategy/shell/2026-04-28_iced_jump_ship_plan.md)) covers the allowlist boundary; this contract is the per-Intent semantic obligation.
*   **Direct Call** (routing): A synchronous call used only within the same module/struct ownership boundary (co-owned state, no authority crossing). Direct calls are not the mechanism for cross-registry decoupling.
*   **Signal** (routing): A decoupled notification/event routed through The Register's signal-routing layer (`SignalBus` or equivalent). Signals are for publish/subscribe coordination where emitters must not know consumers. Signals are not direct state mutation; they may result in `Intent`s downstream.
*   **Session**: A period of application activity, persisted via a specific write-ahead log (WAL). A temporal/persistence concept only — not to be confused with WorkbenchProfile.
*   **Tag**: A user-applied string attribute on a Node (e.g., `#starred`, `#pin`, `udc:51`) used for organization and system behavior.
*   **AddressKind**: The structural classification of a node's address, used as the primary viewer-selection dispatch axis. Canonical values: `Http`, `File`, `Data`, `GraphshellClip`, `Directory`, `Unknown`. Set at node creation from the address string; does not change unless the node's address changes.
*   **Pane Kind**: The semantic class of a pane-hosted surface. Canonical pane kinds are `GraphPane`, `NodeViewerPane`, and `ToolPane`. Pane Kind is graph-visible semantics and should remain stable under viewer-backend swaps.
*   **Content Kind**: The semantic class of the content shown inside a pane, independent of backend and render path. Examples include `WebDocument`, `Directory`, `Clip`, and `GraphshellInternalSurface`. Content Kind is distinct from `AddressKind`: `AddressKind` is a routing primitive derived from the address string, while Content Kind is the higher-level semantic classification surfaced to users and UI policy.
*   **Viewer Backend**: The concrete viewer provider selected to render a node pane, such as `viewer:webview`, `viewer:wry`, `viewer:plaintext`, or `viewer:pdf`. Viewer Backend is a runtime/provider choice, not a pane kind.
*   **Clip Node**: A graph node with `address_kind = GraphshellClip` and `tag = #clip`. Stores user-clipped content (selected text, image, or full-page extraction) at a `verso://clip/<uuid>` address. Created only via the `ClipContent` intent. Rendered by `ClipViewer`.
*   **mime_hint**: An optional `MimeType` field on a Node providing a content-type hint for viewer selection. Set at node creation from HTTP `Content-Type` header, user input, or MIME detection. Overridable by the detection pipeline if a higher-confidence result is found. Stored in the graph data model, not on `NodePaneState`.

## Visual System

*   **Badge**: A visual indicator on a Node or Tab representing a Tag or system state (e.g., Crashed, Unread).
*   **Overlay Affordance Policy**: Per-`TileRenderMode` rules for rendering focus/hover/selection/diagnostic affordances relative to content. `CompositedTexture` renders affordances over content in the compositor pipeline; `NativeOverlay` renders affordances in tile chrome/gutter regions.
*   **MagneticZone**: Legacy alias — see **Frame** (graph-first organizational entity) and **frame-affinity behavior** (`layout_behaviors_and_physics_spec.md §4`). The visual backdrop rendered on the canvas for a Frame's member nodes is informally called a MagneticZone, but the term should not be used in new code or docs. Canonical authority: frame identity and membership live in graph scope; the affinity force is a soft canvas bias, not a hard constraint; a node may belong to multiple frames (no exclusive-membership constraint).
*   **LensPhysicsBindingPreference** (retired 2026-04-30): The policy controlling whether applying a Lens automatically switches the physics profile for a view was an artifact of the old `Lens = Layout × Theme × Physics × Filter` aggregation. With Lens decomposed into Filter only (per the 2026-04-30 simplification), Physics Profile is independently selected per view; there is no "binding policy" since the two are no longer coupled. Egui-era code referencing `LensPhysicsBindingPreference` retires alongside its consumers.
*   **SemanticGravity**: An `ExtraForce` implementation that applies attractive forces between nodes sharing UDC semantic proximity. Registered by the `KnowledgeRegistry` when semantic tagging is active. Uses centroid optimization for O(N) computation.
*   **LOD** (Level of Detail): The rendering detail level applied to nodes and edges based on canvas zoom. Graphshell defines three LOD levels by zoom scale threshold (canonical values from `graph_node_edge_interaction_spec.md §4.8`, authoritative over `CanvasStylePolicy` defaults): Point (scale < 0.55) — minimal marks, suppressed node interactions; Compact (0.55 ≤ scale < 1.10) — compact glyph + badge; Expanded (scale ≥ 1.10) — full affordances. Thresholds are defined in `CanvasStylePolicy`.
*   **WryRenderMode**: The render mode for a `WryViewer` instance. `NativeOverlay` — a native child window owns the content region (always available). `CompositedTexture` — renders to an offscreen texture composited into egui (platform-dependent; unavailable on Linux). Maps directly to `TileRenderMode`.
*   **Backend Badge**: A graph-view or tile-view visual marker exposing the effective Viewer Backend or backend-derived runtime traits (for example `viewer:wry` or `NativeOverlay`) without changing the node's semantic pane kind.
*   **FilePermissionGuard**: The access-control gate for non-web viewer filesystem access. All file-based viewer reads go through `FilePermissionGuard`, which validates the node's address against the workspace's permitted path set. Enforces the no-direct-filesystem-access invariant for non-Servo viewers.

## Runtime Lifecycle

Node lifecycle follows a four-state model: `Active → Warm → Cold → Tombstone`. `Active`, `Warm`, and `Cold` are operational states; `Tombstone` is the code-level name for the deletion-with-preservation state whose user-facing concept is the **Ghost Node**.

*   **Active**: Node has a live webview and is rendering.
*   **Warm**: Node has a live webview but is hidden/cached (optional optimization).
*   **Cold**: Node has no webview; represented by metadata/snapshot only.
*   **Tombstone** (code-level: `NodeLifecycle::Tombstone`): The lifecycle state for a node that has been deleted but is structurally preserved in the graph data model. The user-facing name for a node in this state is **Ghost Node**. A Ghost Node retains its `NodeKey`, spatial position, edges, and a deletion timestamp. Ghost Nodes are filtered out of default graph queries; they are visible only when "Show Deleted" is enabled. Permanent removal requires an explicit garbage-collection action. Restoration transitions a Ghost Node back to `Cold`.
*   **Ghost Node** (user-facing term): A deleted node rendered as a faint dashed placeholder that preserves graph topology — the structural memory of a deletion. Backed by `NodeLifecycle::Tombstone`. Ghost Nodes show reduced opacity, dashed borders, and ghost edges; they are excluded from physics simulation by default. A "Show Deleted" per-view toggle controls visibility (default: off). Spec: `implementation_strategy/viewer/visual_tombstones_spec.md`.

## Registry Architecture

*   **The Register**: The root runtime infrastructure host. Owns both Atomic and Domain registries, the mod loader, inter-registry signal/event routing, and the **Control Panel** (async worker supervision, intent queue, cancellation tokens). The signal-routing layer is currently transitional and may be implemented as `SignalBus` or an equivalent abstraction over time. Code-level: `RegistryRuntime` + `ControlPanel` (+ signal routing layer).
*   **Atomic Registry (Primitive)**: A registry that manages a specific capability contract. The "Vocabulary". Registries define contracts (empty surfaces with fallback defaults); mods populate them with implementations.
    *   *I/O & Routing*: `ProtocolRegistry`, `ViewerRegistry`, `IndexRegistry`.
    *   *Logic*: `ActionRegistry` (discrete deterministic commands), `AgentRegistry` (autonomous cognitive agents that observe app state, connect to AI intelligence providers, and emit intent streams).
    *   *Security*: `IdentityRegistry`.
    *   *Knowledge*: `KnowledgeRegistry` (UDC tagging, semantic distance, validation — the genuine ontology).
    *   *Infrastructure*: `DiagnosticsRegistry`, `ModRegistry`, `LayoutRegistry` (atomic algorithm store: maps `LayoutId → Algorithm`; used by `CanvasRegistry` to resolve the active layout algorithm).
*   **Domain Registry (Composite / Subregister)**: A subregister that groups related primitives by semantic concern and evaluation order.
    *   *Primary domains*: `LayoutDomainRegistry`, `PresentationDomainRegistry`, `InputRegistry`.
*   **Domain**: An architectural concern boundary and evaluation layer (for example `layout`, `presentation`, `input`) that answers a class of behavior questions and defines sequencing constraints between related registries.
*   **Aspect**: A synthesized runtime concern-oriented system (often non-visual, or not inherently visual) that ingests domain/registry capabilities to perform a task or family of related tasks. Aspects may expose one or more UI surfaces, but are not themselves defined by having UI.
*   **Surface** (architectural): A UI presentation/interaction manifestation of a domain, aspect, or subsystem. Examples include the graph canvas, workbench tile-tree presentation, viewer viewport, and subsystem/tool panes. A Surface is not a synonym for a Pane: a Pane is a tile-tree host unit that contains a surface.
*   **Layout Domain**: The domain responsible for how information is arranged and interacted with before styling. Each registry controls structure, interaction policy, and rendering policy for its territory.
    *   `LayoutDomainRegistry` (domain coordinator)
    *   `CanvasRegistry` (graph canvas: topology policy, layout algorithms, interaction/rendering policy, physics engine execution, badge display — the infinite, spatial, physics-driven graph surface)
    *   `WorkbenchSurfaceRegistry` (tile-tree structure, drag/drop, container labels, resize constraints)
    *   `ViewerSurfaceRegistry` (document viewport: zoom/scaling, reader mode, scroll policy)
*   **Presentation Domain**: The domain responsible for appearance and motion semantics after layout.
    *   `PresentationDomainRegistry` (domain coordinator)
    *   `ThemeRegistry` (visual token/style resolution: colors, strokes, fonts)
    *   `PhysicsProfileRegistry` (named parameter presets: Liquid/Gas/Solid as semantic labels over force params)
*   **Cross-Domain Compositor**:
    *   `LensCompositor` (composes Layout + Presentation + Knowledge + Filters; enforces domain sequencing during resolution)
*   **Domain sequencing principle**: Resolve layout first (structure + interaction), then presentation (style + motion parameters).
*   **Domain / Aspect / Surface / Subsystem distinction**:
    *   `Domain` answers what class of behavior is being resolved and in what order. The four application-level domains are Graph (truth + analysis + management), Workbench (arrangement + activation), Navigator (projection + navigation), and Shell (command interpretation + system control). Registry-level domains (layout, presentation, input) define evaluation order within the register.
    *   `Aspect` is the synthesized runtime system oriented to a task family using registry/domain capabilities (may be headless or UI-backed). The Shell domain is the surface through which aspects are exposed to users for observation and control.
    *   `Surface` is the UI presentation through which users interact with or observe a domain/aspect/subsystem.
    *   `Subsystem` is a cross-cutting guarantee framework (diagnostics, accessibility, focus, security, storage, history) applied across domains/aspects/surfaces.
*   **Doc folder conventions** — implementation_strategy sub-folders use the following category prefixes:
    *   `subsystem_*` — a cross-cutting guarantee subsystem (diagnostics, accessibility, focus, security, storage, history, mods, ux_semantics).
    *   `graph/`, `workbench/`, `viewer/`, `navigator/`, `shell/` — Domain feature areas (no prefix; canonical domain names are sufficient). `canvas` remains the surface term for where graph content is rendered, not the domain/folder name.
    *   `aspect_*` — an Aspect (command, control, input, render).
    Plans and specs for prospective or unimplemented features stay in their category folder; removal requires explicit deferral or abandonment note, not just absence of implementation.
*   **Semantic gap principle**: On each architecture change, ask: "Is there a semantic gap that maps cleanly to technical, architectural, or design concerns and should become an explicit registry/domain boundary?"
*   **Mod-first principle**: Registries define contracts. Mods populate them. The application must be fully functional as an offline graph organizer with only core seeds (no mods loaded).
*   **SignalBus**: The planned (or equivalent) inter-registry event bus abstraction owned by The Register. Carries typed signals between registries without direct coupling. Registries subscribe to signal types; emitters do not know their consumers. This term may refer to the architectural role even while implementation remains transitional (for example direct fanout or facade-based routing before a dedicated bus type exists).
*   **Action**: An executable command defined in the `ActionRegistry`.
*   **AgentRegistry**: An atomic registry for autonomous cognitive agents — background processes that observe app state, connect to external AI/inference providers, and emit `GraphIntent` streams. Distinct from `ActionRegistry` (discrete, deterministic, user-triggered commands): agents are continuous, probabilistic, and self-directed.
*   **Mod**: A capability unit that registers entries into one or more registries. Two tiers:
    *   **Native Mod**: Compiled into the binary, registered at startup via `inventory::submit!`. Not sandboxed. Used for first-party capabilities (Verso, Verse, default themes).
    *   **WASM Mod**: Dynamically loaded at runtime via `extism`. Sandboxed, capability-restricted. Used for third-party extensions.
    Both tiers use the same `ModManifest` format declaring `provides` and `requires`.
*   **Core Seed**: The minimal registry population that ships without any mods, making the app functional as an offline document organizer (graph manipulation, local files, plaintext/metadata viewers, search, persistence).
*   **Verso**: A native mod providing two capability families under one transport identity: (1) **Web rendering** — packages Servo/Wry, provides `viewer:webview`, `protocol:http`, `protocol:https`; without Verso the app displays nodes as metadata cards. (2) **Local collaboration** — packages iroh-based bilateral device sync, provides `protocol:verso-sync`; enables private, offline-first peer-to-peer graph sharing between the user's own devices or trusted friends via QR/invite pairing, mDNS discovery, and QUIC transport. Verso owns the Ed25519 transport key that yields the local `NodeId`; if the app also exposes a public `UserIdentity`, the two are linked through a short-lived signed presence assertion rather than a shared keypair. The URL scheme `verso://pair/{NodeId}/{token}` is canonical for pairing. Without Verso, the app is fully offline and web-only (metadata cards only).
*   **Verse**: A **public community network** (long-horizon research, Tier 2). Verse is the federated, multi-stakeholder P2P layer for community knowledge sharing, federated search, and optional economic incentives. It is **not** the local sync layer — that belongs to Verso. Verse uses the same Ed25519 transport key as Verso for `NodeId` / libp2p peer identity, but public/user identity is a separate layer that must be bound onto that transport identity through explicit signed assertions. Provides `protocol:verse-blobs` (VerseBlob content-addressed knowledge units), `protocol:nostr` (optional signaling/invite relay), and `index:community` (federated tantivy search). Without Verse, the app is a local-first knowledge tool with Verso's private sync only.
*   **WorkbenchProfile**: The Workbench + Input configuration component of a Workflow. Captures active tile-tree layout policy, interaction bindings, and container behavior. Composed with Lens (Filter) + Theme + PhysicsProfile at the Workflow level.
*   **Workflow** (revised 2026-04-30): The full active session mode. `Workflow = Lens (Filter) × Theme × PhysicsProfile × WorkbenchProfile`. The four components compose at this top level rather than aggregating under Lens. Habit is per-node and not part of the Workflow composition (each node carries its own). Managed by `WorkflowRegistry` (future).
*   **Scaffold**: An implementation slice that is intentionally partial — core structures/intents/contracts exist, but one or more integration paths (UI wiring, runtime registration, lifecycle hookup, or automation coverage) are not fully closed yet.
*   **Scaffold Marker**: The canonical machine-readable tag for scaffolds: `[SCAFFOLD:<id>]`.
*   **Scaffold Registry**: The canonical index of active scaffolds and closure criteria at `design_docs/graphshell_docs/implementation_strategy/2026-03-02_scaffold_registry.md`.

## Subsystems

A **Subsystem** is a concern that spans multiple registries and components, where silent contract erosion — not one-time implementation gaps — is the dominant failure mode. All subsystems have (or will have) their own pane type. Each subsystem is defined by four layers:

1. **Contracts (schema/invariants)** — Declarative requirements that must hold across the system.
2. **Runtime State** — The live state managed by the subsystem (queued updates, counters, health status).
3. **Diagnostics** — Runtime channels, health metrics, and invariant violations emitted through the diagnostics system.
4. **Validation** — Unit/integration/scenario tests + CI gates that enforce contract compliance over time.

Graphshell defines eight cross-cutting runtime subsystems. For space-limited UI labels, the canonical short labels are: `diagnostics`, `accessibility`, `focus`, `security`, `storage`, `history`, `mods`, `ux_semantics`.

*   **Diagnostics Subsystem**: Runtime observability infrastructure. The reference subsystem — channel schema, invariant watchdogs, analyzers, and the diagnostic inspector pane. The subsystem comprises three registries: **ChannelRegistry** (rename of `DiagnosticsRegistry`) — declarative schema layer: channel IDs, ownership, invariant contracts, severity, sampling config, no behavior; **AnalyzerRegistry** (planned) — continuous stream processors that consume the live event stream and produce derived signals (health scores, alert conditions, pane sections), ships ungated; **TestHarness** (planned, `diagnostics_tests` feature) — in-pane runner with named `TestSuite` structs, background execution, and panic isolation via `catch_unwind`.
*   **TestRegistry**: The `cargo test` fixture struct in `shell/desktop/tests/harness.rs`. An app factory and assertion surface: constructs a fresh `GraphBrowserApp` + `DiagnosticsState` + tile tree for each test, and provides snapshot/channel-count helpers for observability-driven scenario assertions. Compiled only under `#[cfg(test)]` or `feature = "test-utils"`.
*   **Accessibility Subsystem** (`accessibility`): Guarantees that all surfaces remain navigable, comprehensible, and operable across input and assistive modalities (keyboard, screen reader / AccessKit, mouse, gamepad, touch, future speech/audio interaction). This subsystem is broader than the AccessKit bridge implementation alone.
*   **Focus Subsystem** (`focus`): Guarantees that keyboard focus, pointer focus, and accessibility focus are routed correctly across all surfaces; that focus transfers between panes are deterministic and observable; that no surface becomes a focus black hole; and that focus state is consistently exposed to AccessKit. The focus subsystem is the runtime authority on which surface owns input at any moment.
*   **Security & Access Control Subsystem**: Ensures identity integrity, trust boundaries, grant enforcement, and cryptographic correctness across local operations and Verse sync.
*   **Storage Subsystem** (`storage`; long form: **Persistence & Data Integrity Subsystem**): Ensures committed state survives restart, serialization round-trips are lossless, data portability remains intact, and single-write-path boundaries remain inviolable.
*   **History Subsystem** (`history`; long form: **Traversal & Temporal Integrity Subsystem**): Ensures traversal capture correctness, timeline/history integrity, replay/preview isolation, and temporal restoration semantics (including "return to present") remain correct as history features evolve.
*   **Mods Subsystem** (`mods`; long form: **Mod Lifecycle Integrity Subsystem**): Guarantees that mod loading, activation, sandboxing, and unloading cannot silently corrupt registry state or violate capability grants. Owns manifest validation, activation sequencing, WASM sandbox enforcement, mod health diagnostics, and the core seed invariant (the app must remain functional with zero mods loaded). Native mods (`inventory::submit!`) and WASM mods (`extism`) share the same manifest format and activation pipeline.
*   **UX Semantics Subsystem** (`ux_semantics`; long form: **UX Semantics Subsystem**): Provides a runtime-queryable semantic tree (**UxTree**) of Graphshell's own native UI, a per-frame structural invariant checker (**UxProbeSet**), a test-harness bridge (**UxBridge**), and a scenario-driven regression suite (**UxHarness**). Distinct from the web content accessibility tree exposed by Servo/AccessKit — the UxTree describes the *host* UI (workbench, panes, dialogs, radial menu) and maps to AccessKit nodes to serve as the single source of truth for both automated testing and OS screen reader integration. Canonical doc: `design_docs/graphshell_docs/implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md`.

### UX Semantics Subsystem Terms

*   **UxTree**: A per-frame, read-only projection of Graphshell's native GUI state into a stable semantic node tree. Rebuilt each frame; not incrementally updated. Distinct from the web content AccessKit tree. Powers UxProbe checks, UxBridge queries, and the AccessKit host-UI bridge.
*   **UxNode**: One node in the UxTree. May be a leaf (interactive or informational) or a branch (region or pane). Carries: `UxNodeId`, `UxRole`, `label`, `hint`, `UxState`, `value`, `UxAction` list, keyboard shortcuts, `tab_index`, optional `bounds`, children, and `UxMetadata`.
*   **UxNodeId**: A stable, deterministic, path-based string identifier for a `UxNode`. Derived from stable app identities (`GraphViewId`, `NodeKey`, dialog name constants) — never from raw pointers, frame-local indices, or egui hashes. Format: `uxnode://{surface}/{...path segments}`. Stable across non-semantic re-renders.
*   **UxRole**: The semantic role of a `UxNode`. Defines its accessibility semantics and maps to an AccessKit `Role`. Core roles: `Button`, `ToggleButton`, `TextInput`, `OmnibarField`, `SearchField`, `MenuItem`, `RadialSector`, `Tab`, `TabPanel`, `List`, `ListItem`, `Dialog`, `Toolbar`, `StatusBar`, `Landmark`, `Region`, `GraphView`, `NodePane`, `ToolPane`, `WorkbenchChrome`, `GraphNode`, `GraphEdge`, `Heading`, `Text`, `Badge`, `ProgressBar`, `StatusIndicator`. `GraphNodeGroup` remains a deferred extension role until grouping-aware UxTree projection is implemented.
*   **UxState**: The dynamic observable state of a `UxNode`. Fields: `enabled`, `focused`, `selected`, `expanded` (Option), `hidden`, `blocked` (maps to `NodeLifecycle::RuntimeBlocked`), `degraded` (maps to `TileRenderMode::Placeholder`), `loading`.
*   **UxAction**: A discrete action available on a `UxNode` in its current state. Only valid-in-state actions are listed. Values: `Invoke`, `Focus`, `Dismiss`, `SetValue`, `Open`, `Close`, `ScrollTo`, `Expand`, `Collapse`.
*   **UxSnapshot**: A serializable, complete export of the full `UxTree` at a point in time. Format: YAML. Used for snapshot baseline storage and regression diffing in CI.
*   **UxDiff**: A structured diff between two `UxSnapshot`s. Separates structural changes (node added/removed, role/label/actions changed — these block merge) from state changes (focus, loading, selection — these produce warnings only).
*   **UxBaseline**: A stored `UxSnapshot` YAML file serving as the expected state for a given scenario checkpoint. Committed to `tests/scenarios/snapshots/`. Baseline updates require human review.
*   **UxContract**: A machine-verifiable invariant over the UxTree or a UX flow. Three families: `UxInvariant` (holds at all times), `UxFlowContract` (holds over an input sequence), `UxScenario` (named test case exercising a flow contract).
*   **UxContractSet**: A named, versioned collection of `UxContract`s applied to a specific surface or scenario. Registered in the UX contract register (`2026-02-28_ux_contract_register.md`).
*   **UxContractViolation**: A structured failure report from a `UxContract` check. Contains: contract ID, violated node path, actual vs. expected value, and human-readable explanation.
*   **UxInvariant**: A `UxContract` that must hold at every observable program state. Implemented as a `UxProbe` function. S-series: structural. N-series: navigation. M-series: state machine.
*   **UxFlowContract**: A `UxContract` describing a specific interaction flow — starting state, input sequence, expected end state. Passes if the app reaches the expected state without invariant violations.
*   **UxScenario**: A named, reusable test scenario exercising a `UxFlowContract`. Defined as a YAML file in `tests/scenarios/ux/`. Parsed and executed by the `UxHarness` scenario runner.
*   **UxProbe**: A pure function registered at startup that runs every frame (under `ux-probes` feature) and emits `UxViolationEvent` on invariant breach. Signature: `fn(&UxTree) -> Option<UxContractViolation>`. Panic-isolated per probe.
*   **UxProbeSet**: The registry of all registered `UxProbe` functions. Immutable after startup. Executes all probes each frame the UxTree is built.
*   **UxViolation**: A first-class diagnostic event emitted when a `UxInvariant` is breached. Routed through the Diagnostics subsystem on `ux:structural_violation` (Error), `ux:navigation_violation` (Error), or `ux:contract_warning` (Warn) channels.
*   **UxBridge**: The app-side handler that exposes the UxTree and accepts `UxBridgeCommand` messages from a `UxDriver` client. Implemented as custom WebDriver command extensions on the existing WebDriver HTTP server. No new IPC channel required.
*   **UxBridgeCommand**: A discrete command accepted by the `UxBridge`. Core commands: `GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`, `GetFocusPath`, `GetDiagnosticsState`, `StepPhysics`, `SetClock`, `SeedRng`, `SetInputMode`, `GetActiveContracts`.
*   **UxDriver**: The test-side library that sends `UxBridgeCommand`s and evaluates assertions. Provides typed methods: `snapshot()`, `find_node()`, `invoke_action()`, `assert_snapshot_invariants()`, `assert_no_ux_violations()`, `step_physics()`, `set_clock()`, etc.
*   **UxHarness**: The full test infrastructure stack: `UxDriver` + `UxBridge` + `UxProbeSet` + scenario runner + snapshot store. Compiled only under `feature = "test-utils"`.
*   **UxSemantics Aspects**: The six dimensions of UX contract coverage — **Structural Aspect** (tree shape at a moment), **State Aspect** (dynamic state bits), **Navigation Aspect** (focus traversal graph), **Action Aspect** (available actions per node/state), **Flow Aspect** (temporal sequence of transitions), **Latency Aspect** (time between input and observable UxTree state change).

### Surface Capability Declarations (Folded Approach)

**Capability** (in this section) means a **declaration/conformance mechanism**, not a peer subsystem. Each viewer/surface registered in `ViewerRegistry`, `CanvasRegistry`, or `WorkbenchSurfaceRegistry` carries **Surface Capability Declarations** — structured sub-fields declaring the surface's support level for each cross-cutting subsystem. This is not a standalone registry; capabilities are co-located with ownership.

Each subsystem defines its own descriptor type (e.g., `AccessibilityCapabilities`, `SecurityCapabilities`). Surfaces declare `full`, `partial`, or `none` for each capability, plus a reason field for unsupported capabilities.

**Why folded, not standalone**: Capabilities are properties of surfaces. A standalone registry adds indirection without adding clarity. The diagnostics system carries the observability; the owning registries carry the declarations.

### Subsystem Conformance

*   **Subsystem Conformance**: The degree to which a surface/viewer satisfies a subsystem's contract. Conformance is the evaluated outcome (e.g., health checks, tests, diagnostics), whereas a capability declaration is the claimed support level. This distinction prevents overloading the word "capability."

### Degradation Mode

*   **Degradation Mode**: The declared and observed state of a subsystem or surface when full contract compliance is unavailable. Canonical values are `full`, `partial`, and `unavailable` (or `none` in capability declarations). Degradation must be explicit, observable, and tested.

### Invariant Class

*   **Invariant Class**: A category of subsystem contract (e.g., integrity, routing, focus, replay, permission, serialization) used to organize diagnostics, validation, and ownership boundaries consistently across subsystems.

### Subsystem Health

*   **Subsystem Health**: The current runtime assessment of a subsystem derived from contract/invariant checks and diagnostics signals (not merely "is the pane open"). Used in subsystem panes and diagnostics summaries.

---

## Network & Sync (Verse)

### Co-op Terminology

Use **co-op** as the canonical prose and UI label for live collaborative browsing sessions.

Identifier rule:

* Internal Rust/type/action/file identifiers may retain the `Coop` stem (`CoopSessionId`, `StartCoopSession`, `coop_session_spec.md`) until or unless an explicit repo-wide rename is planned.
* Docs, headings, diagrams, and UI copy should prefer **co-op** / **co-op session** / **Start Co-op**.

### Data Sovereignty: Share vs. Publish

These terms have canonical meaning in graphshell and must be used precisely in code, docs, and UI copy.

*   **Share**: Transfer data to a named, known counterparty over a relationship-scoped channel (iroh P2P transport, co-op session, Device Sync). The data exists on counterparty infrastructure only while the relationship is active. Revocable: closing a co-op session, ending a sync relationship, or revoking a `WorkspaceGrant` terminates the channel. The counterparty retains a local copy (snapshot) after the relationship ends, but the live link is gone. Trust is explicit — the receiving peer is identified by `NodeId` or `CoopSessionId`. Examples: sharing a graph view in a co-op session; syncing a workspace to a trusted device.

*   **Publish**: Commit data to infrastructure the user does not fully control (Nostr relays, Verse community DHT, libp2p gossipsub). Infrastructure-committed, not relationship-scoped. Practically irrevocable — once a NIP-84 event propagates to relays, deletion cannot be guaranteed across all copies. Trust is open or pseudonymous; the receiving audience is not enumerated at publish time. Examples: publishing a clip as a NIP-84 highlight; submitting a `Report` to a Verse community index.

**The defining distinction is infrastructure commitment, not trust or audience size.** You can share with an untrusted stranger (co-op guest) and publish to a private relay only you control — in both cases, the above definitions hold. "Sharing" to a relay is publishing; "publishing" to a named peer over iroh is sharing.

**Degradation rule**: when a sharing relationship ends (host goes offline, co-op session closes), the counterparty's local snapshot is their fallback. They own their copy; they do not own the live link. Publishing is the only path to durable URL-stable identity for an annotation beyond the session.

**Usage notes**:

* Use "share" for co-op node visibility (`SetCoopShareVisibility`), Device Sync (`WorkspaceGrant`), and Verso bilateral sync.
* Use "publish" for Nostr event emission, Verse community blob submission, and wallet relay export.
* Avoid "shared" as a modifier for data that has been published — prefer "published" or "community-visible."
* In UI copy: "Share with session" / "Publish to Nostr" / "Publish to community" — never "share to relay."

### Identity & Trust

*   **NodeId**: A 32-byte Ed25519 public key. The canonical transport peer identity across both Verse tiers. Derives `iroh::NodeId` (raw bytes, Tier 1) and `libp2p::PeerId` (identity multihash, Tier 2) from the same secret key. Distinct from public/user identity (`npub`, `did:plc`, etc.); the binding between those layers is an explicit signed assertion, not key reuse.
*   **TrustedPeer**: A persisted record of a known, explicitly paired device. Stores `NodeId`, display name, `PeerRole`, and `WorkspaceGrant` entries. Held in the `IdentityRegistry`.
*   **PeerRole**: Classification of a trusted peer. `Self_` (the user's own other devices) or `Friend` (another user's device).
*   **WorkspaceGrant**: Per-peer, per-workspace access permission. `ReadOnly` or `ReadWrite`.

### Sync Protocol (Tier 1)

*   **SyncUnit**: The wire format for a delta sync exchange. Contains a `VersionVector`, a batch of `SyncedIntent`s, and an optional `WorkspaceSnapshot` for fast-forward. Serialized via rkyv, compressed via zstd, transported over iroh QUIC.
*   **VersionVector**: A map of `NodeId → sequence_number`. Records how far ahead each peer's intent log this node has observed. The causal ordering mechanism for conflict detection. Not a Lamport Clock (single scalar) — a vector clock (per-peer scalars).
*   **SyncWorker**: A ControlPanel-supervised tokio task owning the iroh `Endpoint`. Manages the accept loop, peer connections, and `SyncUnit` exchange. Emits `GraphIntent`s into the normal intent pipeline on receipt.
*   **SyncLog**: The append-only local journal of all intents applied by this node. The source of truth for constructing outbound `SyncUnit`s. AES-256-GCM encrypted at rest.

### Verse Content (Tier 1 + Tier 2)

*   **VerseBlob**: The universal content atom. Content-addressed (CID = BLAKE3 of header ++ payload), typed (`BlobType`), signed by author `NodeId`, optionally encrypted. Transport-agnostic — the same CID over iroh or libp2p Bitswap.
*   **Report**: A `VerseBlob` containing a signed user observation: a node's URL, readability-extracted text, UDC tags, graph traversal context, and a timestamp. The passive indexing unit — the user's browsing record as a publishable, verifiable knowledge asset.
*   **MediaClip**: A `VerseBlob` containing a WARC-format archive of an HTTP response (headers + body). Forensic fidelity — preserves the exact response, not just the rendered DOM. Enables distributed web archiving as a side effect of community membership.
*   **IndexArtifact**: A `VerseBlob` containing a serialized tantivy index segment. Compact, mergeable, and memory-mappable. The unit of federated search — communities share knowledge by sharing IndexArtifacts.

### Community Model (Tier 2)

*   **CommunityManifest**: The signed definition of a Verse community. Contains: `VerseVisibility` policy, `VerseGovernance` (organizers, admin threshold, stake openness), GossipSub `index_topic`, `index_root` CID (current `IndexArtifact`), and a Filecoin `StakeRecord`.
*   **VerseVisibility**: The disclosure and join policy for a community. `PublicOpen` (anyone, anonymous OK), `PublicWithFloor` (discoverable, min rebroadcast required), `SemiPrivate` (permissioned join), or `Dark` (existence itself not broadcastable — enforced at protocol layer).
*   **RebroadcastLevel**: The consent spectrum for joining a Verse. `SilentAck` (local index only, nothing propagated) → `ExistenceBroadcast` (tell peers the Verse exists) → `ContentRelay` (serve blobs via Bitswap) → `Endorsement` (public vouching, implies ContentRelay). The community's `VerseVisibility` sets the minimum floor; joining requires committing to at or above it.
*   **MembershipRecord**: A signed record of a node's commitment to a community. Contains `community_id`, `member_node_id`, `RebroadcastLevel`, `storage_allocation`, and the member's signature — their consent to the community's policy.
*   **VerseGovernance**: The governance model of a community. Organizers (founding stakers, absolute authority) set the `admin_stake_threshold`. Participants who stake above the threshold become admins. `open_to_late_stakers` controls whether new admin paths are available post-founding.

### Search (Tier 2)

*   **SearchProvider**: A Verse node that hosts a large tantivy index and serves `QueryRequest` RPCs over iroh QUIC. Earns Verse Tokens per query via a query-receipt in the Proof of Access economic model. Advertised via Nostr profile or DHT record.
*   **CrawlBounty**: A signed request posted by a community curator offering Verse Tokens for a completed `IndexArtifact` covering a defined `CrawlScope`. The mechanism that turns web indexing into a decentralized gig-economy job.
*   **Crawler**: A Verse node that claims and fulfills `CrawlBounty` requests. Uses the personal crawler pipeline (`reqwest-middleware` + `scraper` + `readability` + `json-ld`) to produce `IndexArtifact` blobs.
*   **Validator**: A Verse node that spot-checks submitted `IndexArtifact` blobs against the `CrawlScope`. Randomly selected from the staked pool; earns a per-check fee from the community validator pool.

## New Terms (2026-03-13 / 2026-03-14)

*   **Relation Family**: A named class of edge (`EdgeKind`) sharing persistence tier, visibility rules, deletion behavior, layout influence, and navigator projection priority. Five families: Semantic, Traversal, Containment, Arrangement, Imported. Canonical doc: `canvas/2026-03-14_graph_relation_families.md`.
*   **FamilyPhysicsPolicy**: An optional field on `LensConfig` carrying per-family force weights (`semantic_weight`, `traversal_weight`, `containment_weight`, `arrangement_weight`, `imported_weight`). Weight `0.0` means the family contributes no attractive force. Canonical doc: `canvas/2026-03-14_graph_relation_families.md §6.1`.
*   **WorkbenchLayerState**: The derived state machine governing default Navigator-host visibility: `GraphOnly`, `GraphOverlayActive`, `WorkbenchActive`, `WorkbenchPinned`. Replaces the ad hoc `is_graph_view`/`has_node_panes` booleans. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §7`.
*   **ChromeExposurePolicy**: The render-time policy derived from `WorkbenchLayerState` each frame: `GraphOnly`, `GraphWithOverlay`, `GraphPlusWorkbenchHost`, `GraphPlusWorkbenchHostPinned`. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §8`.
*   **WorkbenchChromeProjection**: The derived model fed into workbench-scoped Navigator host render each frame. Computed from graph state and tile tree; render-form agnostic. Contains: nav state, focused-pane status badges, frame projection, tile group projection, pane tree rows, topology path, pane badges, adjacent candidates. Actionable viewer controls are intentionally excluded; they belong to tile-local chrome for tiled panes. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §6`.
*   **Navigator**: The section-structured projection that renders graph relations into a readable tree and related chrome sections through one or more Navigator hosts. Sections may include Workbench (arrangement family), Folders (containment/user-folder), Domain (containment/derived), Unrelated, Recent (traversal), and Imported. Canonical doc: `canvas/2026-03-14_graph_relation_families.md §5`.
*   **Edge Inspector Popover**: The per-edge disclosure surface showing family, sub-kind, participants, provenance, durability, and available actions. Opened by right-click on any edge or click on the multi-kind secondary indicator dot. Canonical doc: `canvas/2026-03-14_edge_visual_encoding_spec.md §5.3`.
*   **ArrangementRelation**: An `EdgeKind` (forthcoming) representing frame membership, tile group membership, or split adjacency. Sub-kinds: `frame-member` (durable when named), `tile-group` (session-only), `split-pair` (session-only). Canonical doc: `canvas/2026-03-14_graph_relation_families.md §2.4`.
*   **ContainmentRelation**: An `EdgeKind` (forthcoming) representing hierarchical membership. Sub-kinds: `url-path`, `domain`, `user-folder`, `filesystem`, `clip-source`. Derived sub-kinds are never persisted; recomputed on load. Canonical doc: `canvas/2026-03-14_graph_relation_families.md §2.3`.

---

## Legacy / Deprecated Terms

*   *Context Menu*: Replaced by **Command Palette** (context-aware).
*   *EdgeType*: Replaced by **EdgePayload** (edge projection with `kinds` + traversal records/metrics). The old `EdgeType` variants map to `EdgeKind::UserGrouped` and `EdgeKind::TraversalDerived`.
*   *Navigation History Panel / Traversal History Panel*: Replaced by **History Manager** as the single history UI surface.
*   *View Enum*: Replaced by **Workbench** tile state.
*   *Servoshell*: The upstream project Graphshell forked from.
*   *OntologyDomainRegistry / OntologyRegistry*: Renamed to **KnowledgeRegistry** (atomic). The UDC system is `KnowledgeRegistry`; `PresentationDomainRegistry` is the separate domain coordinator for appearance/motion.
*   *VerseRegistry*: Removed as a domain registry. Verse is a native mod that registers into atomic registries.
*   *GraphLayoutRegistry / WorkbenchLayoutRegistry / ViewerLayoutRegistry*: Renamed to `CanvasRegistry` / `WorkbenchSurfaceRegistry` / `ViewerSurfaceRegistry` to signal that scope includes structure + interaction + rendering policy, not just positioning.
*   *GraphSurfaceRegistry*: Renamed to **CanvasRegistry**. The graph view is an infinite, spatial, physics-driven canvas — semantically distinct from the bounded Workbench and Viewer surfaces.
*   *Workspace* (runtime/window-like UI grouping): Replaced by **Frame**.
*   *Session* (in Workflow/registry context): Replaced by **WorkbenchProfile**. Session remains valid only as the WAL-backed temporal activity period.
*   *Tokenization* (Verse): Replaced by **VerseBlob** + **Proof of Access**. The original concept of "anonymizing a Report and minting it as a digital asset" is now the `Report` BlobType + the receipt economy.
*   *Lamport Clock* (Verse): Replaced by **VersionVector**. Verse uses per-peer monotonic sequence numbers (a vector clock), not a single Lamport scalar. A VersionVector records causal dependencies across all peers; a Lamport clock only orders events globally.
*   *DID / Decentralized Identifier* (Verse): Not used. Verse identity is an Ed25519 `NodeId` stored in the OS keychain. The `NodeId` is the DID equivalent — it is self-sovereign, portable, and derives both iroh and libp2p peer handles from a single keypair. Formal DID method integration is deferred.
*   *MagneticZone*: Legacy alias for the frame affinity backdrop on the canvas. Replaced by the `ArrangementRelation` / `frame-member` model. A frame node's attractive force is governed by `FamilyPhysicsPolicy.arrangement_weight`, not a separate zone primitive. Multiple frame memberships are allowed (a node can carry `frame-member` edges to multiple frames). The visual backdrop rendering is an implementation detail of the canvas, not a separate data concept. See `canvas/2026-03-14_graph_relation_families.md §2.4`.
*   *FileTree*: Legacy pane mode name. Redistributed: the All-nodes projection maps to **Navigator** (all-nodes section), saved view collections map to **ArrangementRelation** saved frames, and filesystem hierarchy maps to **ContainmentRelation** / `filesystem` sub-kind. The `FileTree` UI remains operational until Navigator sections are validated.
*   *Recent section / Frames section / Graph section / Relations section / Import Records section* (Navigator): Replaced by the three **Presentation Buckets** — **Tree Spine** (frametree, containment lenses, traversal hierarchy), **Swatches** (graph and relation projections as scoped canvas instances), **Activity Log** (recency, lifecycle events, import events). The five-section list was a flat catalog; the bucket model names the presentation shape. Specific named projections still exist as recipes that land in one of the three buckets. See `navigator/NAVIGATOR.md §8`.
*   *Workbench-owned Frame*: Frame ownership moved to **Shell**. Frames compose one or more Workbenches into a working context; the Workbench owns its tile tree but not Frame composition or Frame switching. The legacy spec text "a persisted branch/subtree of the Workbench Tile Tree" was workbench-internal phrasing; Frame is now top-level Shell-owned and may scope a single workbench (the trivial case) or multiple workbenches. See `shell/SHELL.md §3` and `workbench/WORKBENCH.md §2`.
*   *Promotion (pane-citizenship sense)* / *Pane Promotion* / *Demotion*: Retired as workbench-citizenship lifecycle operations. The egui-era model distinguished an ephemeral non-citizen Pane (`TileKind::Pane(PaneState)`) from a promoted graph-citizen Tile (`TileKind::Node(NodePaneState)`); Promotion was the event that wrote the address into the graph and Demotion was its inverse. The post-iced canonical model (per [`shell/2026-04-28_iced_jump_ship_plan.md`](graphshell_docs/implementation_strategy/shell/2026-04-28_iced_jump_ship_plan.md) §4.4) eliminates the ephemeral state — every Pane shows graph nodes from the start. The three operations replacing the Promotion/Demotion lifecycle are **Close tile** (deactivate; presentation only), **Remove from graphlet** (organizational graph edit), and **Tombstone** (destructive node deletion). The *projection-sense* Promotion (turning an ephemeral projection result into authority truth via intent) is a separate, retained term — see Projection Vocabulary §Promotion.
*   *Pane Opening Mode* (`QuarterPane` / `HalfPane` / `FullPane` / `Tile`): Retired with the ephemeral-Pane lifecycle. Every Pane is now a spatial leaf in a Frame's split tree; size is expressed through Split proportions, not through ephemeral mode. Opening a node opens it as an Active tile in a tile pane (creating or selecting one) — there is no separate "open ephemeral" intermediate step.
*   *Pane (graph-citizenship clarification)* / *Tile (graph-citizenship)*: Retired. The Pane/Tile distinction in the egui-era was citizenship-vs-non-citizenship; the post-iced model has only the spatial Pane definition (leaf in Frame split tree) and the active-graph-node Tile definition (rendered inside a tile Pane). See Tile Tree Architecture §Primitives.
*   *Pane Open Event* (egui-era): The egui-era undoable event that recorded a pending source-`NodeKey` relationship for later promotion. Retired alongside Promotion. The post-iced equivalent is a graph-side node-open event (a `GraphReducerIntent` that opens or activates a node) plus a presentation-side activation event that does not require a separate pending-edge step.
*   *Tile-to-Tile Navigation* (egui-era): The egui-era event for opening a new Tile (in `Tile` mode) directly from an existing Tile, asserting an edge at open time. Retired with Pane Opening Mode; the post-iced equivalent is "open node in a tile pane, asserting traversal edge" — a single graph-side event without the Pane Opening Mode dimension.
*   *GraphletView*: Never reached canonical TERMINOLOGY.md. Mentioned in pre-2026-04-29 iced-plan drafts and retired before adoption. Today's model uses **Pane** (carrying `GraphletId`) as the rendering surface for a graphlet; there is no separate GraphletView concept.

<!-- markdownlint-enable MD030 MD007 -->
