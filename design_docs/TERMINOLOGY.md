# Graphshell Terminology

<!-- markdownlint-disable MD030 MD007 -->

**Status**: Living Document
**Goal**: Define canonical terms for the project to ensure consistency across code, documentation, and UI. Terms must reflect actual architectural structures, not just semantic convenience.

## Core Identity

* **Graphshell**: The product name. A local-first, spatial browser. The graph is the persistent substrate; the workbench is the structural presentation layer; the Workbench Sidebar (navigator) projects graph relations into a readable tree. See `design_docs/graphshell_docs/implementation_strategy/canvas/2026-03-14_graph_relation_families.md` for the relation family model that supersedes the legacy "file-tree" metaphor.
* **Spatial Graph Browser**: The user-facing description of the interface. It emphasizes the force-directed graph and tiling window manager.
* **Knowledge User Agent**: The architectural philosophy. Unlike a passive "User Agent" that just renders what servers send, Graphshell actively crawls, indexes, cleans, and stores data on the user's behalf.
* **Verso**: A native mod and user agent component packaging (1) Servo/Wry web rendering and (2) local peer-to-peer collaboration via iroh. An homage to Servo. The private, fast, device-local layer.
* **Verse**: The optional public community network for federated knowledge sharing. Long-horizon research. The public, community, federated layer. Distinct from Verso's local collaboration.

## Tile Tree Architecture

The layout system is built on `egui_tiles`. Every visible surface is a node in a recursive **Tile Tree**.

### Projection Rule

Graphshell intentionally keeps **graph identity terms** separate from **workbench presentation terms**.

- A **Node** is graph-semantic identity/state.
- A **Tile** is the workbench presentation/container that hosts a node-bearing or graph-view-bearing leaf.
- A **Graphlet** is a graph-semantic grouped arrangement object.
- A **Tile Group** is the workbench presentation of that grouped arrangement.

Canonical projection law:

- nodes **project as tiles** in workbench chrome
- graphlets **project as tile groups** in workbench chrome
- frames **project as frames** across graph, navigator, and workbench presentations

This is a presentation correspondence, not a term collapse. A node can exist without a tile; a tile is not the canonical owner of node identity.

### Primitives

* **Tile**: The fundamental node in the layout tree. Either a **Pane** (leaf) or a **Container** (branch). Identified by a `TileId` (opaque `u64`, unique within one tree). Code: `egui_tiles::Tile<TileKind>`. A Tile is a workbench presentation/container term, not the canonical semantic identity of a graph node.
* **Pane**: A leaf Tile that renders content. The payload is a `TileKind` enum:
    * `TileKind::Graph(GraphViewId)` — a force-directed graph canvas.
    * `TileKind::Pane(PaneState)` — an unenrolled content pane with no `NodeKey` yet. This is the canonical representation for ephemeral pane-opening modes before promotion.
    * `TileKind::Node(NodePaneState)` — a promoted/enrolled node viewer pane bound to an existing graph node and resolved viewer backend (legacy serde alias preserves old `WebView(NodeKey)` layouts).
    * `TileKind::Tool(ToolPaneState)` — a tool/subsystem pane host (diagnostics today; history/settings/subsystem panes over time).
* **Tab** (**UI affordance term**): A visual selector control for choosing an active Tile among sibling tiles. Canonical structural term is **Tile**; "tab" is presentation shorthand only.
* **Container**: A branch Tile that holds and arranges child Tiles. Three structural types exist:

    | Container type | egui_tiles type | Children visible | Resizable | Layout direction |
    | --- | --- | --- | --- | --- |
    | **Tab Group** | `Container::Tabs` | One at a time (active tab) | No | Tab bar selects active child |
    | **Split** | `Container::Linear` | All simultaneously | Yes, via drag handles | `Horizontal` (top↔bottom regions; horizontal divider) or `Vertical` (left↔right regions; vertical divider) |
    | **Grid** | `Container::Grid` | All simultaneously | Yes, rows & columns | 2D, auto or fixed column count |

* **Tab Group**: A container that renders a tab bar; only the **active** child Tile is visible. Promoted tiles (`TileKind::Node`, `TileKind::Graph`, `TileKind::Tool`) are always wrapped in a Tab Group (enforced by `all_panes_must_have_tabs: true`), so each split region always has its own tab strip that can accept additional tabs. Ephemeral panes (`TileKind::Pane`) are exempt from this invariant — they are placed directly in a split region without a Tab Group wrapper, so no tab selector appears for them.
    Canonical projection note: when a graph-rooted grouped arrangement is rendered in workbench chrome, the Tab Group / Tile Group is its workbench presentation rather than a separate semantic owner.
* **Split**: A container that arranges children in either top/bottom regions (`Horizontal`, horizontal divider) or left/right regions (`Vertical`, vertical divider) with resizable dividers. Children are ordered in `Vec<TileId>`. **Shares** control the proportional width/height each child receives. User-facing label for `Container::Linear`; rendered as `Split ↔` (horizontal arrangement label) or `Split ↕` (vertical arrangement label) in tile selector strips.
* **Grid**: A container that arranges children in a 2D matrix. Layout is either `Auto` (dynamic column count) or `Columns(n)`.
* **Shares**: Per-child `f32` weights within a Split that determine proportional space allocation. Default share is `1.0`.

### Composition Rules

* **Arbitrary nesting**: Containers can hold other Containers to any depth. A Tab Group can contain Splits, which contain Tab Groups, which contain more Splits, etc.
* **Cross-direction nesting preserved**: A `Horizontal` Split inside a `Vertical` Split (or vice versa) is never collapsed — this is how complex layouts form.
* **Same-direction merging**: A `Horizontal` Split nested directly inside another `Horizontal` Split is automatically absorbed (children promoted, shares recalculated). Controlled by `join_nested_linear_containers: true`.
* **Simplification**: The tree is simplified every frame. Empty containers are pruned, single-child containers are collapsed (except Tab Groups wrapping a lone promoted tile, which are kept for the tab strip). Controlled by `SimplificationOptions`.
* **SimplificationSuppressed**: A flag set on a Split container for the lifetime of any ephemeral pane (`TileKind::Pane`) it directly hosts. While set, same-direction merging and single-child collapse are suppressed for that container, preventing the surrounding layout from reflowing under the ephemeral pane. The flag is cleared and normal simplification resumes on the frame after the ephemeral pane is dismissed.

### Composite Structures

* **Tile Tree**: The complete recursive structure of Tiles forming the layout. Backed by a flat `Tiles<TileKind>` hashmap keyed by `TileId`, plus a root `TileId`. Code: `egui_tiles::Tree<TileKind>`, stored as `Gui::tiles_tree`.
* **App Scope**: The top-most global scope for a running Graphshell process. App Scope owns workbench switching/navigation.
* **Workbench**: A global container within App Scope paired to one complete graph dataset (`GraphId`). It hosts the Tile Tree (`Tree<TileKind>`), tracks frame ordering, and drives frame switching/render. Graph Bar state remains graph-scope app chrome; workbench-hosted chrome state is projected through the Workbench Sidebar. The workbench is a contextual presentation layer, not the semantic owner of nodes, graphlets, or `GraphViewId`.
* **Workbench Scope**: The full, unscoped graph domain of one Workbench (`GraphId`-bound).
* **Frame**: A persisted branch/subtree of the Workbench Tile Tree that groups tiles and preserves their arrangement/focus as a unit. Frame is the canonical runtime/UI term for top-level working contexts within one Workbench.
* **Frame Snapshot** (**Persistence Snapshot**, canonical storage term): A persistable snapshot of a Workbench/Frame layout plus its content manifest. Serialized as `PersistedFrame`, which contains:
    * `FrameLayout` — the `Tree<PersistedPaneTile>` shape
    * `FrameManifest` — the pane-to-content mapping and member node UUIDs
    * `FrameMetadata` — timestamps for creation, update, last activation
    Frame Snapshot is the canonical save/restore/storage term; Frame remains the primary runtime/UI container term.

### Pane Types

* **Graph View**: A Pane (`TileKind::Graph`) containing a force-directed canvas visualization powered by `egui_graphs`. Renders the `Graph` data model with physics simulation, node selection, and camera controls.
* **GraphViewId**: A stable identifier for a specific Graph View pane instance. `GraphViewId` is the canonical identity for per-view camera state, `ViewDimension`, Lens assignment, and `LocalSimulation` (for Divergent layout views). Generated at pane creation; persists across reorder, split, and move operations.
    A `GraphViewId` may be hosted in a tile tree surface, but tile hosting does not become the owner of that graph-view identity.
* **GraphLayoutMode**: The layout participation mode for a Graph View pane. `Canonical` — participates in the shared workspace graph layout (shared node positions, one physics simulation). `Divergent` — has its own `LocalSimulation` with independent node positions; activated explicitly by the user.
* **LocalSimulation**: An independent physics simulation instance owned by a `Divergent` Graph View. Does not affect Canonical pane node positions.
* **Pane Opening Mode**: The four canonical ways a Pane can be summoned into the Workbench, controlling both size and graph citizenship:
    * `QuarterPane` — occupies roughly one quarter of the available area; no tab bar, no graph node. Ephemeral.
    * `HalfPane` — occupies roughly half of the available area; no tab bar, no graph node. Ephemeral.
    * `FullPane` — occupies the full available area; no tab bar, no graph node. Ephemeral.
    * `Tile` — full graph citizen: the pane's address is written into the graph as a new `Node`, a tab bar appears, and the pane becomes freely rearrangeable. This is **Promotion**.
    Ephemeral modes (Quarter/Half/Full) are summoned into a split region with no tab selector of their own, so they do not demand context the way a tile does. They are dismissed once the user's inputs are confirmed, and their dimensions are remembered per pane type for next time. The default expectation is that panes stay ephemeral; promotion to Tile is a deliberate, relatively rare user decision.
* **Address-as-Identity principle**: A tile's graph citizenship is determined solely by whether its address resolves to a live (non-tombstone) node in the graph. No separate mapping structure exists. The canonical check is: *does a node with this address exist in the graph?* `TileKind::Pane(PaneState)` carries no address yet (or an address not yet written to the graph); `TileKind::Node(NodePaneState)`, `TileKind::Graph`, and `TileKind::Tool` all carry addresses in the canonical `verso://` internal-address scheme that resolve to graph nodes. Legacy `graphshell://` forms are compatibility aliases only. The graph's node set is itself the authoritative membership list — querying it is querying graph citizenship.
* **Pane** (graph-citizenship clarification): A Pane in any ephemeral opening mode has **no graph presence** — it carries no address yet written to the graph, does not participate in edge events, and is not tracked in the graph data model. It lives in the tile tree only as `TileKind::Pane(PaneState)`.
* **Tile** (graph-citizenship): A Pane whose address has been written to the graph as a `Node` — i.e., a promoted pane. The tab bar materializes as the visual symptom of that address being in the graph. `TileKind::Node(NodePaneState)` is the promoted content tile. `TileKind::Graph` and `TileKind::Tool` are also tiles in this sense: their canonical `verso://` addresses resolve to graph nodes and they carry tab bars. A collection of Tiles grouped into a Frame corresponds to a bounded region of nodes in the graph canvas, visually represented by a titled, colored frame backdrop on the canvas (bound by `ArrangementRelation` / `frame-member` edges).
    Projection reminder: a tile is how node-bearing or graph-view-bearing content appears in the workbench. It is not a synonym for `Node`.
* **Verso internal address scheme**: Internal tile types use a `verso://` address namespace so that their graph presence follows the same address-as-identity rule as web content tiles. Canonical forms:
    * `verso://view/<GraphViewId>` — a Graph View pane node.
    * `verso://tool/<name>` — a tool/subsystem pane node. If multiple instances of the same tool are open simultaneously, a numeric discriminator is appended: `verso://tool/<name>/<n>` (starting at 2; the discriminator is recycled upon pane closure).
    * `verso://frame/<FrameId>` — a Frame node. Frame nodes carry `ArrangementRelation` / `frame-member` edges to all member tile nodes; on the canvas, a named Frame is rendered as a titled, colored backdrop (legacy term: MagneticZone) attracting its member nodes via `FamilyPhysicsPolicy.arrangement_weight`.
    * `verso://settings/<section>` — a Settings surface route.
    * `verso://clip/<uuid>` — an internal clip node address.
    Legacy `graphshell://...` forms remain accepted as compatibility aliases during migration.
    Tool and settings address schemes follow the same discriminator pattern.
* **Promotion** (aka **Pane Promotion**): The event at which a Pane transitions from an ephemeral opening mode to `Tile`. Promotion writes the pane's address into the graph as a new `Node`, then upgrades the tile-tree payload from `TileKind::Pane(PaneState)` to `TileKind::Node(NodePaneState)`. If the pane was sourced from an existing Tile (opened via a tile-to-pane navigation), promotion also asserts an `Edge` between the source tile's node and the newly created node — resolving the pending relationship recorded at open time. If the pane was summoned independently (no source tile), promotion creates only the node. Long-term direction: layout mutations (split, reorder, resize) will emit `GraphIntent`s so that the graph becomes the write path for layout metadata, with `egui_tiles` acting as renderer rather than authority.
* **Demotion**: The inverse of Promotion. Demoting a tile removes its address from the graph (transitions the node to `Tombstone`), cascades tombstone to edges from that node, and downgrades the tile-tree payload back to `TileKind::Pane(PaneState)`. The tab bar disappears. Tombstoned edges and the tombstoned node are preserved in the graph for history and can be restored if the tile is re-promoted. Demotion is the undo target for a Promotion event.
* **Pane Open Event**: A tracked, undoable workbench event emitted whenever a Pane is summoned — regardless of opening mode and regardless of whether a source Tile exists. No graph mutation occurs at open time for ephemeral panes. When a source Tile is present, the Pane Open Event records the source tile's `NodeKey` as a pending relationship so that if the user later promotes the pane, the edge can be asserted retroactively. The pending `NodeKey` is preserved in the event log even if the source node is later tombstoned — the record is historical; live edge creation at promotion time requires the source node to be non-tombstone. Pane Open Events participate in global and scoped undo like all other workbench events. If the user's focus context moves away from the scope the pane was opened in before it is explicitly dismissed, the pane is auto-dismissed; system/subsystem/tool panes have their state autosaved on auto-dismiss.
* **Tile-to-Tile Navigation**: Opening a new Tile (Tile mode) directly from an existing Tile. An immediate edge event: the new tile's address is written to the graph as a node at open time, and an edge is asserted between the source node and the new node, without a pending-relationship step.
* **Pane Presentation Mode** (aka **Pane Chrome Mode**): How a Pane is presented in the tile tree UI (chrome, mobility, and locking behavior), distinct from the Pane's content. The `Tile` opening mode is the promoted presentation; ephemeral modes are the non-promoted presentations. Prefer **Pane Opening Mode** when discussing the user-facing choice; use Pane Presentation Mode only when discussing the underlying chrome/mobility implementation.
* **Docked Pane**: A Pane presented with reduced chrome and position-locked behavior inside the current tile arrangement. Intended to reduce accidental reflow and focus attention on content.
* **PaneLock**: The reflow lock state of a Pane, independent of `PanePresentationMode`. `Unlocked` (default) — all user-initiated reflow operations permitted. `PositionLocked` — cannot be moved or reordered; can be closed. Docked panes are implicitly position-locked from the user's perspective. `FullyLocked` — cannot be moved, reordered, or closed by the user; reserved for system-owned panes.
* **FrameTabSemantics**: An optional semantic overlay on top of the `egui_tiles` structural tree. Persists semantic tab group membership so that meaning is not lost when `egui_tiles` simplification restructures the tree. Serialized with rkyv into the frame bundle. This is frame state, not WAL data.
* **TabGroupMetadata**: A record within `FrameTabSemantics` for one semantic tab group. Contains `group_id` (`TabGroupId`), ordered `pane_ids`, and `active_pane_id` (repaired to `None` if the previously active pane is removed from the group).
* **Subsystem Pane**: A pane-addressable surface for a subsystem's runtime state, health, configuration, and primary operations. Subsystems are expected to have dedicated panes, but implementations may be staged. Subsystem panes are hosted as tool panes (`TileKind::Tool(ToolPaneState)`).
* **Tool Pane**: A non-document pane hosted under `TileKind::Tool(ToolPaneState)` (e.g., Diagnostics today; History Manager, subsystem panes, settings surfaces over time). Tool panes may be subsystem panes or general utility surfaces.
* **Diagnostic Inspector**: A subsystem pane (currently the primary `ToolPaneState` implementation) for visualizing system internals (Engine, Compositor, Intents, and future subsystem health views).

### Surface Composition

* **Surface Composition Contract**: The formal specification of how a node viewer pane tile's render frame is decomposed into ordered composition passes (UI Chrome, Content, Overlay Affordance), with backend-specific adaptations per `TileRenderMode`.
* **Composition Pass**: One of three ordered rendering phases within a single node viewer pane tile frame: (1) UI Chrome Pass, (2) Content Pass, (3) Overlay Affordance Pass. Pass ordering is Graphshell-owned sequencing and must not rely on incidental egui layer behavior.
* **CompositorAdapter**: A wrapper around backend-specific content callbacks (for example Servo `render_to_parent`) that owns callback ordering, GL state isolation, clipping/viewport contracts, and the post-content overlay hook.
* **TileRenderMode**: The runtime-authoritative render pipeline classification for a node viewer pane tile: `CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, or `Placeholder`. Resolved from `ViewerRegistry` at viewer attachment time and used for compositor pass dispatch.

## Interface Components

*   **Omnibar**: The primary global navigation/input bar for location, search, and command entry. Anchored in the Graph Bar (center position). See `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §4`.
*   **Graph Bar**: The always-visible top chrome surface. Carries graph-scope controls (Undo/Redo, new node/edge/tag, omnibar, physics chip, lens chip, tag filter chips, sync badge, overflow). Stable regardless of what workbench tiles are open. Replaces the monolithic toolbar. See `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`.
*   **Workbench Sidebar**: The structural presentation chrome surface, visible when hosted workbench surfaces are active. Projects the current tile tree, frames, and tile groups as a navigator. Carries pane-local controls (Back/Forward/Reload/clip/viewer). Replaces the Workbar. See `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §5`.
*   **Workbar**: Legacy term — superseded by **Graph Bar** + **Workbench Sidebar**. Do not use in new code or docs.
*   **History Manager**: The canonical non-modal history surface with Timeline and Dissolved tabs, backed by traversal archive keyspaces.
*   **Settings Pane**: A tool pane that aggregates configuration and controls across registries, subsystems, and app-level preferences. A settings pane may host subsystem-specific sections or summon dedicated subsystem panes.
*   **Control Panel**: The async coordination/process host for background workers and intent producers within The Register. In architectural terms it is an **Aspect** (runtime coordination concern), not a UI Surface. It supervises worker lifecycles and intent ingress, but does not own or render panes/surfaces directly; subsystem UI appears through dedicated tool/subsystem panes. Code-level: `ControlPanel` (supervised by `RegistryRuntime`).
*   **Lens**: A named configuration composing a Layout, Theme, Physics Profile, and Filter(s). Defines how the graph *looks* and *moves*.
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
*   **LensPhysicsBindingPreference**: The policy controlling whether applying a Lens automatically switches the physics profile for a view. `Always` — auto-switch. `Ask` — prompt the user. `Never` — never auto-switch. Stored per-view in `GraphViewId` state.
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
    *   `Domain` answers what class of behavior is being resolved (layout/presentation/input) and in what order.
    *   `Aspect` is the synthesized runtime system oriented to a task family using registry/domain capabilities (may be headless or UI-backed).
    *   `Surface` is the UI presentation through which users interact with or observe a domain/aspect/subsystem.
    *   `Subsystem` is a cross-cutting guarantee framework (diagnostics, accessibility, focus, security, storage, history) applied across domains/aspects/surfaces.
*   **Doc folder conventions** — implementation_strategy sub-folders use the following category prefixes:
    *   `subsystem_*` — a cross-cutting guarantee subsystem (diagnostics, accessibility, focus, security, storage, history, mods, ux_semantics).
    *   `canvas/`, `workbench/`, `viewer/` — Layout Domain feature areas (no prefix; canonical registry names are sufficient).
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
*   **WorkbenchProfile**: The Workbench + Input configuration component of a Workflow. Captures active tile-tree layout policy, interaction bindings, and container behavior. Combined with a Lens to produce a full Workflow.
*   **Workflow**: The full active session mode. `Workflow = Lens × WorkbenchProfile`. A Lens defines how the graph looks and moves; a WorkbenchProfile defines how the Workbench and input are configured. Managed by `WorkflowRegistry` (future).
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
*   **UxRole**: The semantic role of a `UxNode`. Defines its accessibility semantics and maps to an AccessKit `Role`. Core roles: `Button`, `ToggleButton`, `TextInput`, `OmnibarField`, `SearchField`, `MenuItem`, `RadialSector`, `Tab`, `TabPanel`, `List`, `ListItem`, `Dialog`, `Toolbar`, `StatusBar`, `Landmark`, `Region`, `GraphView`, `NodePane`, `ToolPane`, `WorkbenchChrome`, `GraphNode`, `GraphEdge`, `GraphNodeGroup`, `Heading`, `Text`, `Badge`, `ProgressBar`, `StatusIndicator`.
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

### Data Sovereignty: Share vs. Publish

These terms have canonical meaning in graphshell and must be used precisely in code, docs, and UI copy.

*   **Share**: Transfer data to a named, known counterparty over a relationship-scoped channel (iroh P2P transport, Coop session, Device Sync). The data exists on counterparty infrastructure only while the relationship is active. Revocable: closing a Coop session, ending a sync relationship, or revoking a `WorkspaceGrant` terminates the channel. The counterparty retains a local copy (snapshot) after the relationship ends, but the live link is gone. Trust is explicit — the receiving peer is identified by `NodeId` or `CoopSessionId`. Examples: sharing a graph view in a Coop session; syncing a workspace to a trusted device.

*   **Publish**: Commit data to infrastructure the user does not fully control (Nostr relays, Verse community DHT, libp2p gossipsub). Infrastructure-committed, not relationship-scoped. Practically irrevocable — once a NIP-84 event propagates to relays, deletion cannot be guaranteed across all copies. Trust is open or pseudonymous; the receiving audience is not enumerated at publish time. Examples: publishing a clip as a NIP-84 highlight; submitting a `Report` to a Verse community index.

**The defining distinction is infrastructure commitment, not trust or audience size.** You can share with an untrusted stranger (Coop guest) and publish to a private relay only you control — in both cases, the above definitions hold. "Sharing" to a relay is publishing; "publishing" to a named peer over iroh is sharing.

**Degradation rule**: when a sharing relationship ends (host goes offline, Coop session closes), the counterparty's local snapshot is their fallback. They own their copy; they do not own the live link. Publishing is the only path to durable URL-stable identity for an annotation beyond the session.

**Usage notes**:
- Use "share" for Coop node visibility (`SetCoopShareVisibility`), Device Sync (`WorkspaceGrant`), and Verso bilateral sync.
- Use "publish" for Nostr event emission, Verse community blob submission, and wallet relay export.
- Avoid "shared" as a modifier for data that has been published — prefer "published" or "community-visible."
- In UI copy: "Share with session" / "Publish to Nostr" / "Publish to community" — never "share to relay."

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
*   **WorkbenchLayerState**: The derived state machine governing chrome surface visibility: `GraphOnly`, `GraphOverlayActive`, `WorkbenchActive`, `WorkbenchPinned`. Replaces the ad hoc `is_graph_view`/`has_node_panes` booleans. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §7`.
*   **ChromeExposurePolicy**: The render-time policy derived from `WorkbenchLayerState` each frame: `GraphOnly`, `GraphWithOverlay`, `GraphPlusWorkbenchSidebar`, `GraphPlusWorkbenchSidebarPinned`. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §8`.
*   **WorkbenchChromeProjection**: The derived model fed into Workbench Sidebar render each frame. Computed from graph state and tile tree; render-form agnostic. Contains: nav state, viewer controls, frame projection, tile group projection, pane tree rows, topology path, pane badges, adjacent candidates. Canonical doc: `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §6`.
*   **Navigator**: The Workbench Sidebar body section that projects graph relations into a readable section-structured tree. Sections: Workbench (arrangement family), Folders (containment/user-folder), Domain (containment/derived), Unrelated, Recent (traversal), Imported. Canonical doc: `canvas/2026-03-14_graph_relation_families.md §5`.
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

<!-- markdownlint-enable MD030 MD007 -->
