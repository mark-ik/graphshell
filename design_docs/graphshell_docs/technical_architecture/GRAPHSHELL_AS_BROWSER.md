# GRAPHSHELL AS A WEB BROWSER

**Purpose**: Detailed specification for how Graphshell operates as a functional web browser and universal content viewer.

**Document Type**: Behavior specification (not implementation status)
**Status**: Core browsing graph functional; delegate-driven desktop navigation, three-tier lifecycle (Active/Warm/Cold), LRU eviction, frame-context routing, registry layer Phases 0–4 complete, badge/UDC tagging in progress
**See**: [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md) for actual code status

---

## Design Principle: Unified Spatial Tile Manager

Graphshell is a spatial tile manager with three authority domains:

- **Graph**: semantic state (node identity, lifecycle, edges, tags, viewer preference).
- **Tile tree**: layout/focus/visibility state.
- **Webviews / viewers**: runtime rendering instances reconciled from graph lifecycle.

- **Graph view pane**: Overview and organizational control surface. Drag nodes between clusters, create edges, delete nodes — all affect the tile tree and viewers.
- **Node viewer panes**: Focused working contexts. Each pane's tile-selector row shows the nodes in that pane's cluster. Closing a tile closes the viewer and demotes the node to `Cold` (node remains in graph unless explicitly deleted).
- **Tool panes**: Diagnostics/history/settings (including sync + physics controls) and other utility surfaces using the same tile-tree focus/layout semantics (not graph lifecycle owners).
- **Tile selector rows**: Per-pane projections of graph clusters. Active tiles (with viewer) are highlighted; inactive tiles (no viewer) are dimmed and reactivatable.

**Key invariant**: semantic truth lives in graph/intents; tile and viewer runtime state are coordinated through explicit intent/reconciliation boundaries.

---

## 1. Graph-Tile-Viewer Relationship

### Node Identity

Each node is canonical graph identity and is represented through one or more tiles in frame contexts. Node identity is not its URL or content type.

- **URLs are mutable**: Within-tile navigation changes the node's current URL. The node persists.
- **Duplicate URLs allowed**: The same URL can be open in multiple tiles (multiple nodes). Each is independent.
- **Stable ID**: Nodes are identified by a stable UUID (not URL, not petgraph NodeIndex). Persistence uses this UUID.
- **Per-node history**: Each node has its own back/forward stack. Servo provides this via `notify_history_changed(webview, entries, index)` for web nodes.
- **Content type**: Nodes carry `mime_hint: Option<String>` and `address_kind: AddressKind` that drive viewer selection for non-web content. See [2026-02-24_universal_content_model_plan.md](../implementation_strategy/viewer/2026-02-24_universal_content_model_plan.md).

### Servo Signals (Web Nodes)

Servo provides two distinct signals that drive the graph (no Servo modifications required):

| User action | Servo delegate method | Graph effect |
|-------------|----------------------|--------------|
| Click link (same tab) | `notify_url_changed(webview, url)` | Update node's current URL and title. Push to history. No new node. |
| Back/forward | `notify_url_changed(webview, url)` | Update node's URL. History index changes. No new node. |
| Ctrl+click / middle-click / window.open | `request_create_new(parent_webview, request)` | Create new node. Create edge from parent node. Add to current frame context as a new tile. |
| Title change | `notify_title_changed(webview, title)` | Update node's title. |
| History update | `notify_history_changed(webview, entries, index)` | Store back/forward list on node (from Servo, not custom). |

---

## Research Conclusions (2026-02-15 / updated 2026-02-24)

The architecture plan identified a previous mismatch (URL-polling assumptions and fragmented routing). For desktop tile flow, this has been addressed: navigation semantics are delegate-driven, structural node creation is not polling-driven, and mutations route through intent/reconciliation boundaries. Remaining deferred scope is EGL/WebDriver explicit-target parity. See [2026-02-21_lifecycle_intent_model.md](../implementation_strategy/2026-02-21_lifecycle_intent_model.md) and [2026-02-20_embedder_decomposition_plan.md](../implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md).

### Edge Types

**Current implementation** uses `EdgeType` enum:

| Edge type | Created by | Meaning |
|-----------|-----------|---------|
| `Hyperlink` | `request_create_new` (new tab from parent) | User opened a new tab from this page |
| `History` | Back/forward detection (existing reverse edge) | Navigation reversal |
| `UserGrouped` | Explicit split-open grouping gesture (`Shift + Double-click` in graph) | User deliberately associated two nodes |

**Edge Traversal Model** (planned replacement): `EdgeType` will be replaced by `EdgePayload` containing `Vec<Traversal>` records. Each traversal captures the full navigation event (from_url, to_url, timestamp, trigger). This model preserves repeat navigations, timing data, and enables commutative P2P sync. See [2026-02-20_edge_traversal_impl_plan.md](../implementation_strategy/2026-02-20_edge_traversal_impl_plan.md) for implementation timeline.

### Pane Membership and Frames

- **Tile tree is the authority** on which node lives in which pane.
- **Navigation routing**: New nodes from `request_create_new` are added to the parent node's tile container.
- **New root node** (N key, no parent): Creates a new tile container in the tile tree.
- **Tile move** (drag between panes): Moves the tile. `UserGrouped` creation for drag-move is follow-up work; current explicit grouping trigger is split-open.

**Frame-context routing** (implemented; polish ongoing): Opening a node predictably restores the expected frame/workbench context. Nodes track frame-context membership metadata and recency. Routing resolver uses recency and membership index to decide whether to restore an existing frame context or open in current frame context. See [2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md](../implementation_strategy/workbench/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md) and [workbench_frame_tile_interaction_spec.md](../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md).

### Node Lifecycle

**Current code** implements a three-tier desired lifecycle model (see `graph/mod.rs` `NodeLifecycle` enum) with an observed runtime layer managed by reconciliation:

| Desired state | Has viewer? | Shown in tile selector row? | Shown in graph? | LRU eviction |
|-------|-------------|-------------------|-----------------|-------------|
| **Active** | Yes (Mapped) | Yes (highlighted) | Yes (full color) | LRU cache (default: 4 slots) |
| **Warm** | Maybe (Mapped or Unmapped; policy-controlled) | Yes (dimmed) | Yes (medium color) | LRU cache (default: 12 slots) |
| **Cold** | No (Unmapped) | Yes (dimmed) | Yes (dimmed) | Unlimited |

A Cold node is not an empty shell — it is a fully realized graph citizen with address, title, history, edges, tags, and thumbnail. It has no active viewer attached.

**Lifecycle transitions:**

- Focus a node → `PromoteNodeToActive` intent → reconcile creates/reactivates viewer
- Navigate away from an active tile → `DemoteNodeToWarm` intent → viewer may remain mapped (policy)
- Active LRU overflow → `DemoteNodeToWarm` intent (oldest non-pinned active node)
- Warm LRU overflow → `DemoteNodeToCold` intent → viewer destroyed, metadata retained
- Memory pressure (warning/critical) → stronger eviction cascade (Active → Warm → Cold)
- Close tab tile → `DemoteNodeToWarm` or `DemoteNodeToCold` (based on warm cache policy)
- Create failures/crashes → `MarkRuntimeBlocked` with backoff; `ClearRuntimeBlocked` on recovery
- Delete node → `RemoveNode` intent → node removed from graph entirely

**Lifecycle Intent Vocabulary** (authoritative): `PromoteNodeToActive`, `DemoteNodeToWarm`, `DemoteNodeToCold`, `MarkRuntimeBlocked`, `ClearRuntimeBlocked`, plus runtime mapping intents (`MarkRuntimeCreatePending`, `MarkRuntimeCreateConfirmed`).

See [2026-02-21_lifecycle_intent_model.md](../implementation_strategy/2026-02-21_lifecycle_intent_model.md) for the full desired/observed model, invariants, and cause metadata.

### Intent-Based Mutation

All user interactions produce intents processed at a single sync point per frame. No system directly mutates another mid-frame.

**Two-Phase Apply Model:**

1. **Phase 1 — Pure Reducer** (state mutation): Processes `GraphIntent`, updates graph structure and desired lifecycle state. No Servo calls or viewer effects.
2. **Phase 2 — Reconciliation** (viewer lifecycle): Converges observed runtime state toward desired state; creates/destroys viewers, applies blocked/backoff policy, and emits follow-up intents for the next frame if needed.

**Phase gap invariant**: Sub-frame gap between phases prevents contradictory viewer state (e.g., navigating a destroyed viewer, creating duplicate viewers for the same node).

Sources of intents:
- **Graph view**: drag-to-cluster, delete node, create edge, select
- **Tile selector row**: close tile, reorder tiles, drag tile to other pane/frame context
- **Keyboard**: N (new node), Del (remove), T (tag panel), etc.
- **Servo callbacks**: `request_create_new`, `notify_url_changed`, `notify_title_changed` → converted to GraphIntent

**Delegate-driven routing**: Servo callbacks → GraphIntent emission → reducer application → reconciliation effects. No polling, no fragmented mutation paths.

See [2026-02-21_lifecycle_intent_model.md](../implementation_strategy/2026-02-21_lifecycle_intent_model.md) for the detailed phase model and [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md) for the architectural summary.

---

## 2. Navigation Model

### Within-Tile Navigation (Link Click)

**Scenario**: User is in a pane viewing node A (github.com), clicks a link to github.com/servo.

**Behavior**: The node's URL updates. No new node is created. Servo's `notify_url_changed` fires.

- Node A's `current_url` changes to github.com/servo
- Node A's title updates when `notify_title_changed` fires
- Node A's history stack gains an entry (provided by `notify_history_changed`)
- The tile selector entry for A updates to show the new title/URL
- No edge created, no new node

### Open New Tile (Ctrl+Click, Middle-Click, window.open)

**Scenario**: User Ctrl+clicks a link on node A, opening it in a new tile.

**Behavior**: A new node is created with an edge from A. Servo's `request_create_new` fires.

- New node B created with the target URL
- Edge A → B created (type: Hyperlink)
- B's tile is added to current frame context (same pane branch unless user routes elsewhere)
- B becomes the active tile in that pane/frame context
- A becomes inactive (no webview, still visible in tile selector row)

### Back/Forward Navigation

**Scenario**: User presses back button in the browser UI.

**Behavior**: Servo traverses its own history stack. `notify_url_changed` fires with the previous URL. The node's URL updates. No new node.

Servo provides the full back/forward list via `notify_history_changed(webview, entries, index)`. Graphshell stores this on the node or reads it from the WebView on demand — no need to maintain a custom history stack.

### New Root Tile (N Key)

**Scenario**: User presses N to create a blank tab.

**Behavior**: New node created with `about:blank`. New tile container created in tile tree. No parent, no edge.

---

## 3. Non-Web Content (Universal Content Model)

Graphshell nodes can hold any addressable content, not only web pages. The renderer (viewer) is
selected by `ViewerRegistry` based on the node's `mime_hint`, `address_kind`, and user preference.

### ViewerRegistry Selection Order

1. `Node.viewer_id_override` — explicit user choice (persisted to WAL).
2. Frame `viewer_id_default` — frame-level default.
3. `ViewerRegistry::select_for(mime, address_kind)` — highest-priority matching viewer.
4. `viewer:webview` — fallback for HTTP/S and HTML files.
5. `viewer:plaintext` — last resort; always succeeds.

### Viewer Types

| Viewer ID | Content | Rendering mode | Notes |
| --------- | ------- | -------------- | ----- |
| `viewer:webview` | HTTP/S, HTML | Texture (GPU surface) | Default; graph canvas + workbench tiles |
| `viewer:wry` | HTTP/S (native fallback) | Overlay (OS window) | Workbench tiles only; Windows primary |
| `viewer:plaintext` | text/\*, JSON, TOML, YAML, Markdown | Embedded egui | Syntax highlighting via syntect; Markdown via pulldown-cmark |
| `viewer:image` | image/\*, SVG | Embedded egui | Raster via image crate; SVG via resvg |
| `viewer:pdf` | application/pdf | Embedded egui | PDFium C FFI; feature-gated |
| `viewer:directory` | file:// directory | Embedded egui | Navigable file listing; stdlib only |
| `viewer:audio` | audio/\* | Embedded egui | symphonia + rodio; feature-gated |

**Texture vs Overlay**: Servo renders to a texture Graphshell owns and can draw anywhere (graph
canvas, workbench tiles, rotated/scaled). Wry creates a native OS window overlay that floats above
the app surface — workbench tiles only, cannot be placed on a moving graph node.

**Hybrid rule for Wry nodes in graph view**: if a node's viewer is `viewer:wry` and it is
currently displayed in the graph canvas, render the node's last thumbnail instead of a live
viewer. The user must open the node in a workbench pane to interact with it.

See [2026-02-24_universal_content_model_plan.md](../implementation_strategy/viewer/2026-02-24_universal_content_model_plan.md) and [2026-02-23_wry_integration_strategy.md](../implementation_strategy/2026-02-23_wry_integration_strategy.md).

### Address Kind

`AddressKind` on `Node` is a hint for viewer selection:

| AddressKind | Typical URL prefix | Default viewer |
| ----------- | ------------------ | -------------- |
| `Http` | `http://`, `https://` | `viewer:webview` |
| `File` | `file://`, local paths | depends on MIME |
| `Custom` | any other scheme | registry lookup |

### File Manager Mode

When a `file://` URL points to a directory, the `DirectoryViewer` renders a navigable listing.
Each entry is clickable (navigate the node to that file) or draggable (create a new node with
that file's address). This turns Graphshell into a spatial file manager: bookmarks, web pages,
and local filesystem paths coexist in the same graph.

---

## 4. Tags and Semantic Organization

### Tag System

Nodes carry `tags: HashSet<String>`. Tags are user-applied attributes stored in the WAL.

**Reserved system namespace**: Tags beginning with `#` carry behavioral effects.

| Tag | Effect |
| --- | ------ |
| `#pin` | Physics anchor — not displaced by simulation |
| `#starred` | Soft bookmark; surfaces in omnibar `@b` scope |
| `#archive` | Hidden from default graph view; reduced opacity |
| `#resident` | Never cold-evicted regardless of frame/workbench context |
| `#private` | URL/title redacted in screen-sharing mode; excluded from export |
| `#nohistory` | Navigating through this node does not push a traversal entry |
| `#monitor` | Periodic reload + DOM hash comparison; badge pulse on change |
| `#unread` | Auto-applied on add/URL change; cleared on first activation |
| `#focus` | Boosts DOI score; floats toward layout center |
| `#clip` | DOM-extracted clip node; distinct shape/border in graph view |

User tags without `#` (e.g., `work`, `research`, `todo`) are purely organizational.

### Badge System

Node badges are visual overlays in the graph view communicating tag state at a glance.

- **At rest**: up to 3 icon-only badges (16×16 px) at top-right corner; `+N` overflow chip.
- **Hover/focus**: full orbit expansion with icon + label.
- **Priority order**: Crashed > WorkspaceCount > Pinned > Starred > Unread > system tags > ContentType > UDC tags > user tags.
- **ContentType badge**: when a node's viewer is not `viewer:webview`, a small content-type icon (📄 PDF, 🖼 image, 📝 text, 🎵 audio, 📁 directory) marks it as non-web content.

See [2026-02-20_node_badge_and_tagging_plan.md](../implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md).

### UDC Semantic Tags and Semantic Physics

Nodes can carry UDC (Universal Decimal Classification) tags (`udc:51` — Mathematics, `udc:004` — Computer science). These drive **semantic physics**: nodes attract each other in proportion to UDC prefix overlap, making the graph self-organize into subject clusters even without hyperlinks between them.

The tag assignment panel (`T` key) uses fuzzy search via `nucleo` against the `KnowledgeRegistry` to suggest UDC codes from natural-language input ("math" → "Mathematics (udc:51)").

See [2026-02-23_udc_semantic_tagging_plan.md](../implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md).

---

## 5. Bookmarks Integration

**Current status**: Manual edge creation; bookmarks as node metadata.

**Implemented foundation** (Persistence/Settings architecture phases):

- **Bookmarks are tags**: `Node.tags` field stores bookmark folder paths (e.g., `["bookmarks", "work", "research"]`).
- **Settings page**: `graphshell://settings/bookmarks` provides bookmark management UI (add/remove tags, bulk import, export).
- **Import/export**: Firefox `bookmarks.html` import creates nodes with tag metadata. Export generates standard bookmark HTML.
- **Search integration**: Omnibar searches node tags (fuzzy match via nucleo).
- **Visual indicator**: `#starred` badge overlay on tagged nodes in graph view.

See [2026-02-22_workbench_workspace_manifest_persistence_plan.md](../implementation_strategy/2026-02-22_workbench_workspace_manifest_persistence_plan.md) and [2026-02-20_settings_architecture_plan.md](../implementation_strategy/2026-02-20_settings_architecture_plan.md) for UI delivery.

---

## 6. Downloads & Files

**Scenario**: User downloads a file from a webpage.

- Download tracked with source node reference
- Downloads page (`graphshell://settings/downloads`) shows in-progress + completed downloads
- Download metadata stored per-node for provenance
- Downloaded files can be opened directly in a `File` node using the appropriate viewer

**Implementation note**: Download tracking integrates with settings architecture (`graphshell://` internal URL scheme). See [2026-02-20_settings_architecture_plan.md](../implementation_strategy/2026-02-20_settings_architecture_plan.md).

---

## 7. Search & Address Bar

**Omnibar** serves dual purpose: graph search + URL navigation.

- **URL input** (`http://...`): Navigates the current tile (within-tile navigation).
- **File input** (`file://...`): Opens local file or directory in current node using appropriate viewer.
- **Text search**: Fuzzy search via `nucleo` matcher (FT6 in roadmap, now implemented).
  - Searches node titles, URLs, tags, and MIME hints.
  - Score-ranked results with keyboard navigation.

**Graph search modes** (Ctrl+F panel):

- **Highlight mode** (default): Matching nodes highlighted in gold, dimmed non-matches remain visible.
- **Filter mode** (toggle in search panel): Hide non-matching nodes entirely, collapse edges to preserved nodes.

**Faceted search** (planned): Filter by lifecycle state, edge type, traversal recency, tag, visit count, viewer type, MIME type. See [2026-02-20_edge_traversal_impl_plan.md](../implementation_strategy/2026-02-20_edge_traversal_impl_plan.md) for traversal-backed search model integration.

---

## Summary: How Graphshell Differs from Traditional Browsers

| Feature | Firefox | Graphshell |
|---------|---------|-------|
| **Primary UI** | Tab bar | Force-directed graph + tiled panes + workbench frame bar |
| **Tab management** | Linear tab strip | Spatial tile arrangement (drag, cluster, edge) |
| **Navigation** | Click link → same tab or new tab | Same browser semantics mapped to tiles: within-tile nav or new tile |
| **History** | Global linear history | Per-node history (from Servo) + graph edges + traversal log |
| **Tab grouping** | Manual tab groups | Graph clusters = pane tile-selector rows; semantic UDC clusters |
| **Bookmarks** | Folder tree | Node tags (folder paths as metadata) |
| **Lifecycle** | Active/discarded binary | Active/Warm/Cold three-tier with LRU |
| **Frames / Workbench contexts** | Window-based sessions | Named frame/workbench snapshots with membership routing |
| **Settings** | Preferences dialog | Pane-hosted settings tool surface (`graphshell://settings/*` routes resolve to tool panes) |
| **Content types** | Web pages only | Web, PDF, images, SVG, text, audio, local directories |
| **Node annotation** | None | UDC semantic tags drive physics clustering and auto-grouping |
| **Viewer** | Browser engine | ServoRenderer (default), WryViewer (OS webview fallback), non-web renderers |

**Core difference**: The graph is the organizational layer. Tile selectors are projections of graph clusters. What you do in the graph is what the tile tree becomes. Nodes track frame/workbench membership context; opening a node restores expected context. Tags and semantic physics make the graph self-organize by subject.

---

## Related Documentation

**Core architecture:**
- [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md) — implementation status, data structures, key decisions
- [VERSO_AS_PEER.md](VERSO_AS_PEER.md) — how Verso (the web capability mod) connects Graphshell to the web and to Verse peers
- [IMPLEMENTATION_ROADMAP.md](../implementation_strategy/IMPLEMENTATION_ROADMAP.md) — feature targets and validation tests

**Implementation plans:**
- [2026-02-21_lifecycle_intent_model.md](../implementation_strategy/2026-02-21_lifecycle_intent_model.md) — two-phase apply model, lifecycle intents
- [2026-02-20_edge_traversal_impl_plan.md](../implementation_strategy/2026-02-20_edge_traversal_impl_plan.md) — EdgeType → EdgePayload migration
- [2026-02-22_workbench_workspace_manifest_persistence_plan.md](../implementation_strategy/2026-02-22_workbench_workspace_manifest_persistence_plan.md) — frame/workbench membership index and routing resolver foundation (legacy filename retained)
- [2026-02-20_settings_architecture_plan.md](../implementation_strategy/2026-02-20_settings_architecture_plan.md) — `graphshell://` internal URL scheme
- [2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md](../implementation_strategy/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md) — workbench tile-selector semantics overlay and routing polish addendum (legacy filename retained)
- [2026-02-23_graph_interaction_consistency_plan.md](../implementation_strategy/2026-02-23_graph_interaction_consistency_plan.md) — interaction/search-surface harmonization
- [2026-02-24_universal_content_model_plan.md](../implementation_strategy/viewer/2026-02-24_universal_content_model_plan.md) — non-web viewers, MIME detection, viewer selection policy
- [2026-02-23_wry_integration_strategy.md](../implementation_strategy/2026-02-23_wry_integration_strategy.md) — native OS webview overlay integration
- [2026-02-20_node_badge_and_tagging_plan.md](../implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md) — badge system and tag assignment UI
- [2026-02-23_udc_semantic_tagging_plan.md](../implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md) — UDC semantic tagging and semantic physics

