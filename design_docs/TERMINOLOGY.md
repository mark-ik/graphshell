# Graphshell Terminology

**Status**: Living Document
**Goal**: Define canonical terms for the project to ensure consistency across code, documentation, and UI. Terms must reflect actual architectural structures, not just semantic convenience.

## Core Identity

*   **Graphshell**: The product name. A local-first, spatial browser combining a tile-tree and a file-tree.
*   **Spatial Graph Browser**: The user-facing description of the interface. It emphasizes the force-directed graph and tiling window manager.
*   **Knowledge User Agent**: The architectural philosophy. Unlike a passive "User Agent" that just renders what servers send, Graphshell actively crawls, indexes, cleans, and stores data on the user's behalf.
*   **Verse**: The optional decentralized, peer-to-peer network component for sharing graph data.
*   **Verso**: A native mod and user agent component packaging Servo/Wry web rendering. An homage.

## Tile Tree Architecture

The layout system is built on `egui_tiles`. Every visible surface is a node in a recursive **Tile Tree**.

### Primitives

*   **Tile**: The fundamental node in the layout tree. Either a **Pane** (leaf) or a **Container** (branch). Identified by a `TileId` (opaque `u64`, unique within one tree). Code: `egui_tiles::Tile<TileKind>`.
*   **Pane**: A leaf Tile that renders content. The payload is a `TileKind` enum:
    *   `TileKind::Graph(GraphViewId)` — a force-directed graph canvas.
    *   `TileKind::Node(NodePaneState)` — a node viewer pane bound to a graph node and resolved viewer backend (legacy serde alias preserves old `WebView(NodeKey)` layouts).
    *   `TileKind::Tool(ToolPaneState)` — a tool/subsystem pane host (diagnostics today; history/settings/subsystem panes over time).
*   **Tab**: A tab-bar affordance inside a **Tab Group** container used to select the active child Tile. A Tab is not a Pane; it is one UI control for addressing a Pane/Tile within a container.
*   **Container**: A branch Tile that holds and arranges child Tiles. Three structural types exist:

    | Container type | egui_tiles type | Children visible | Resizable | Layout direction |
    |---|---|---|---|---|
    | **Tab Group** | `Container::Tabs` | One at a time (active tab) | No | Tab bar selects active child |
    | **Split** | `Container::Linear` | All simultaneously | Yes, via drag handles | `Horizontal` (left↔right) or `Vertical` (top↔bottom) |
    | **Grid** | `Container::Grid` | All simultaneously | Yes, rows & columns | 2D, auto or fixed column count |

*   **Tab Group**: A container that renders a tab bar; only the **active** child Tile is visible. Every Pane is always wrapped in a Tab Group (enforced by `all_panes_must_have_tabs: true`), so each split region always has its own tab strip that can accept additional tabs.
*   **Split**: A container that arranges children side-by-side (`Horizontal`) or stacked (`Vertical`) with resizable dividers. Children are ordered in `Vec<TileId>`. **Shares** control the proportional width/height each child receives. User-facing label for `Container::Linear`; rendered as `Split ↔` (horizontal) or `Split ↕` (vertical) in tab strips.
*   **Grid**: A container that arranges children in a 2D matrix. Layout is either `Auto` (dynamic column count) or `Columns(n)`.
*   **Shares**: Per-child `f32` weights within a Split that determine proportional space allocation. Default share is `1.0`.

### Composition Rules

*   **Arbitrary nesting**: Containers can hold other Containers to any depth. A Tab Group can contain Splits, which contain Tab Groups, which contain more Splits, etc.
*   **Cross-direction nesting preserved**: A `Horizontal` Split inside a `Vertical` Split (or vice versa) is never collapsed — this is how complex layouts form.
*   **Same-direction merging**: A `Horizontal` Split nested directly inside another `Horizontal` Split is automatically absorbed (children promoted, shares recalculated). Controlled by `join_nested_linear_containers: true`.
*   **Simplification**: The tree is simplified every frame. Empty containers are pruned, single-child containers are collapsed (except Tab Groups wrapping a lone Pane, which are kept for the tab strip). Controlled by `SimplificationOptions`.

### Composite Structures

*   **Tile Tree**: The complete recursive structure of Tiles forming the layout. Backed by a flat `Tiles<TileKind>` hashmap keyed by `TileId`, plus a root `TileId`. Code: `egui_tiles::Tree<TileKind>`, stored as `Gui::tiles_tree`.
*   **Workbench**: The top-level application surface: the Tile Tree (`Tree<TileKind>`) plus window chrome (toolbar, status bar, toasts). The Workbench owns the tree and drives the render loop. In code, this corresponds to `Gui` and its `tiles_tree` field.
*   **Workspace**: A persistable snapshot of a Workbench layout plus its content manifest. Serialized as `PersistedWorkspace`, which contains:
    *   `WorkspaceLayout` — the `Tree<PersistedPaneTile>` shape
    *   `WorkspaceManifest` — the pane-to-content mapping and member node UUIDs
    *   `WorkspaceMetadata` — timestamps for creation, update, last activation
    A Workspace is the unit of save/restore ("Project Context").

### Pane Types

*   **Graph View**: A Pane (`TileKind::Graph`) containing a force-directed canvas visualization powered by `egui_graphs`. Renders the `Graph` data model with physics simulation, node selection, and camera controls.
*   **Pane Presentation Mode** (aka **Pane Chrome Mode**): How a Pane is presented in the tile tree UI (chrome, mobility, and locking behavior), distinct from the Pane's content.
*   **Tiled Pane** (aka **Promoted Pane**): A Pane presented with tile/tab chrome and normal tile-tree mobility operations (split/tab/arrange/reflow).
*   **Docked Pane**: A Pane presented with reduced chrome and position-locked behavior inside the current tile arrangement. Intended to reduce accidental reflow and focus attention on content.
*   **Subsystem Pane**: A pane-addressable surface for a subsystem's runtime state, health, configuration, and primary operations. Subsystems are expected to have dedicated panes, but implementations may be staged. Subsystem panes are hosted as tool panes (`TileKind::Tool(ToolPaneState)`).
*   **Tool Pane**: A non-document pane hosted under `TileKind::Tool(ToolPaneState)` (e.g., Diagnostics today; History Manager, subsystem panes, settings surfaces over time). Tool panes may be subsystem panes or general utility surfaces.
*   **Diagnostic Inspector**: A subsystem pane (currently the primary `ToolPaneState` implementation) for visualizing system internals (Engine, Compositor, Intents, and future subsystem health views).

## Interface Components

*   **History Manager**: The canonical non-modal history surface with Timeline and Dissolved tabs, backed by traversal archive keyspaces.
*   **Settings Pane**: A tool pane that aggregates configuration and controls across registries, subsystems, and app-level preferences. A settings pane may host subsystem-specific sections or summon dedicated subsystem panes.
*   **Control Panel**: The async coordination/process host for background workers and intent producers within The Register. In architectural terms it is a peer coordinator (not owner) for registries, subsystems, mods, and UI surfaces — an **Aspect** of The Register's runtime composition. It does not own or render UI surfaces directly; subsystems expose UI through their dedicated tool/subsystem panes. Code-level: `ControlPanel` (supervised by `RegistryRuntime`).
*   **Lens**: A named configuration composing a Layout, Theme, Physics Profile, and Filter(s). Defines how the graph *looks* and *moves*.
*   **Command Palette**: A modifiable context menu that serves as an accessible interface for executing Actions.
*   **The Register**: See *Registry Architecture* section below for the full definition.
*   **Camera**: The graph viewport state (pan offset, zoom level) for a Graph View. Stored separately from the Tile Tree as it is per-view runtime state, not a layout concern.

## Camera Commands

*   **Camera Fit**: Fits the viewport to the bounding box of all nodes with a relaxed zoom factor. Triggered by `C` key or on startup with an existing graph.
*   **Focus Selection**: Fits the viewport to the bounding box of the selected nodes with tighter padding. Triggered by `Z` key when 2+ nodes are selected.
*   **Wheel Zoom**: Zoom in/out via mouse wheel, trackpad two-finger scroll, or smooth-scroll delta. Pointer-relative (zooms toward cursor position). Configurable via `scroll_zoom_requires_ctrl` setting.

## Data Model

*   **Graph**: The persistent data structure containing Nodes and Edges. Acts as the "File System".
*   **Node**: A unit of content (webpage, note, file) identified by a stable UUID.
*   **Edge**: A relationship between two nodes.
    *   **UserGrouped**: Explicit connection made by the user (flag on Edge).
    *   **Traversal-Derived**: Implicit connection formed by navigation events.
*   **Traversal**: A temporal record of a navigation event (timestamp, trigger) stored on an Edge.
*   **Edge Traversal History**: The aggregate of all Traversal records, forming the complete navigation history of the graph. Replaces linear global history.
*   **Intent**: A data payload (`GraphIntent`) describing a desired state change. The fundamental unit of mutation in the system.
*   **Session**: A period of application activity, persisted via a specific write-ahead log (WAL). A temporal/persistence concept only — not to be confused with WorkbenchProfile.
*   **Tag**: A user-applied string attribute on a Node (e.g., `#starred`, `#pin`, `udc:51`) used for organization and system behavior.

## Visual System

*   **Badge**: A visual indicator on a Node or Tab representing a Tag or system state (e.g., Crashed, Unread).

## Runtime Lifecycle

*   **Active**: Node has a live webview and is rendering.
*   **Warm**: Node has a live webview but is hidden/cached (optional optimization).
*   **Cold**: Node has no webview; represented by metadata/snapshot only.

## Registry Architecture

*   **The Register**: The root runtime infrastructure host. Owns both Atomic and Domain registries, the mod loader, inter-registry signal/event routing, and the **Control Panel** (async worker supervision, intent queue, cancellation tokens). The signal routing layer may be implemented as `SignalBus` or an equivalent abstraction over time. Code-level: `RegistryRuntime` + `ControlPanel` (+ signal routing layer).
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
    *   `Subsystem` is a cross-cutting guarantee framework (diagnostics, accessibility, security, storage, history) applied across domains/aspects/surfaces.
*   **Semantic gap principle**: On each architecture change, ask: "Is there a semantic gap that maps cleanly to technical, architectural, or design concerns and should become an explicit registry/domain boundary?"
*   **Mod-first principle**: Registries define contracts. Mods populate them. The application must be fully functional as an offline graph organizer with only core seeds (no mods loaded).
*   **SignalBus**: The planned (or equivalent) inter-registry event bus abstraction owned by The Register. Carries typed signals between registries without direct coupling. Registries subscribe to signal types; emitters do not know their consumers. This term may refer to the architectural role even while implementation remains transitional.
*   **Action**: An executable command defined in the `ActionRegistry`.
*   **AgentRegistry**: An atomic registry for autonomous cognitive agents — background processes that observe app state, connect to external AI/inference providers, and emit `GraphIntent` streams. Distinct from `ActionRegistry` (discrete, deterministic, user-triggered commands): agents are continuous, probabilistic, and self-directed.
*   **Mod**: A capability unit that registers entries into one or more registries. Two tiers:
    *   **Native Mod**: Compiled into the binary, registered at startup via `inventory::submit!`. Not sandboxed. Used for first-party capabilities (Verso, Verse, default themes).
    *   **WASM Mod**: Dynamically loaded at runtime via `extism`. Sandboxed, capability-restricted. Used for third-party extensions.
    Both tiers use the same `ModManifest` format declaring `provides` and `requires`.
*   **Core Seed**: The minimal registry population that ships without any mods, making the app functional as an offline document organizer (graph manipulation, local files, plaintext/metadata viewers, search, persistence).
*   **Verso**: A native mod packaging Servo/Wry web rendering. Provides `viewer:webview`, `protocol:http`, `protocol:https`. Without Verso, nodes display as metadata cards.
*   **Verse**: A native mod packaging P2P networking. Provides `protocol:verse-blobs` (iroh QUIC sync), `protocol:nostr` (signaling/invite relay), and `index:community` (federated tantivy search). Tier 1 (private device sync via iroh) and Tier 2 (public community swarms via libp2p) are distinct phases. Without Verse, the app is fully offline.
*   **WorkbenchProfile**: The Workbench + Input configuration component of a Workflow. Captures active tile-tree layout policy, interaction bindings, and container behavior. Combined with a Lens to produce a full Workflow.
*   **Workflow**: The full active session mode. `Workflow = Lens × WorkbenchProfile`. A Lens defines how the graph looks and moves; a WorkbenchProfile defines how the Workbench and input are configured. Managed by `WorkflowRegistry` (future).

## Subsystems

A **Subsystem** is a concern that spans multiple registries and components, where silent contract erosion — not one-time implementation gaps — is the dominant failure mode. All subsystems have (or will have) their own pane type. Each subsystem is defined by four layers:

1. **Contracts (schema/invariants)** — Declarative requirements that must hold across the system.
2. **Runtime State** — The live state managed by the subsystem (queued updates, counters, health status).
3. **Diagnostics** — Runtime channels, health metrics, and invariant violations emitted through the diagnostics system.
4. **Validation** — Unit/integration/scenario tests + CI gates that enforce contract compliance over time.

Graphshell defines five cross-cutting runtime subsystems. For space-limited UI labels, the canonical short labels are: `diagnostics`, `accessibility`, `security`, `storage`, `history`.

*   **Diagnostics Subsystem**: Runtime observability infrastructure. The reference subsystem — channel schema, invariant watchdogs, analyzers, and the diagnostic inspector pane.
*   **Accessibility Subsystem** (`accessibility`): Guarantees that all surfaces remain navigable, comprehensible, and operable across input and assistive modalities (keyboard, screen reader / AccessKit, mouse, gamepad, touch, future speech/audio interaction). This subsystem is broader than the AccessKit bridge implementation alone.
*   **Security & Access Control Subsystem**: Ensures identity integrity, trust boundaries, grant enforcement, and cryptographic correctness across local operations and Verse sync.
*   **Storage Subsystem** (`storage`; long form: **Persistence & Data Integrity Subsystem**): Ensures committed state survives restart, serialization round-trips are lossless, data portability remains intact, and single-write-path boundaries remain inviolable.
*   **History Subsystem** (`history`; long form: **Traversal & Temporal Integrity Subsystem**): Ensures traversal capture correctness, timeline/history integrity, replay/preview isolation, and temporal restoration semantics (including "return to present") remain correct as history features evolve.

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

### Identity & Trust

*   **NodeId**: A 32-byte Ed25519 public key. The canonical peer identity across both Verse tiers. Derives `iroh::NodeId` (raw bytes, Tier 1) and `libp2p::PeerId` (identity multihash, Tier 2) from the same secret key — one keypair, two peer handles.
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

## Legacy / Deprecated Terms

*   *Context Menu*: Replaced by **Command Palette** (context-aware).
*   *EdgeType*: Replaced by **EdgePayload** (containing Traversals).
*   *Navigation History Panel / Traversal History Panel*: Replaced by **History Manager** as the single history UI surface.
*   *View Enum*: Replaced by **Workbench** tile state.
*   *Servoshell*: The upstream project Graphshell forked from.
*   *OntologyDomainRegistry / OntologyRegistry*: Renamed to **KnowledgeRegistry** (atomic). The UDC system is `KnowledgeRegistry`; `PresentationDomainRegistry` is the separate domain coordinator for appearance/motion.
*   *VerseRegistry*: Removed as a domain registry. Verse is a native mod that registers into atomic registries.
*   *GraphLayoutRegistry / WorkbenchLayoutRegistry / ViewerLayoutRegistry*: Renamed to `CanvasRegistry` / `WorkbenchSurfaceRegistry` / `ViewerSurfaceRegistry` to signal that scope includes structure + interaction + rendering policy, not just positioning.
*   *GraphSurfaceRegistry*: Renamed to **CanvasRegistry**. The graph view is an infinite, spatial, physics-driven canvas — semantically distinct from the bounded Workbench and Viewer surfaces.
*   *Session* (in Workflow/registry context): Replaced by **WorkbenchProfile**. Session remains valid only as the WAL-backed temporal activity period.
*   *Tokenization* (Verse): Replaced by **VerseBlob** + **Proof of Access**. The original concept of "anonymizing a Report and minting it as a digital asset" is now the `Report` BlobType + the receipt economy.
*   *Lamport Clock* (Verse): Replaced by **VersionVector**. Verse uses per-peer monotonic sequence numbers (a vector clock), not a single Lamport scalar. A VersionVector records causal dependencies across all peers; a Lamport clock only orders events globally.
*   *DID / Decentralized Identifier* (Verse): Not used. Verse identity is an Ed25519 `NodeId` stored in the OS keychain. The `NodeId` is the DID equivalent — it is self-sovereign, portable, and derives both iroh and libp2p peer handles from a single keypair. Formal DID method integration is deferred.
