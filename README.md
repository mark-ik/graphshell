# graphshell

    An open source, prototype, spatial browser that represents nodes in a force-directed graph as tabs in tilable workspaces.

- Force-directed graph canvas with Servo-powered web rendering
- Tiled multi-pane workspace: graph overview and webview panes, side by side
- Local-first persistent browsing graph with crash-safe recovery
- Event-driven navigation semantics from Servo delegate callbacks

## Build and Run (Standalone)

```bash
# Build
cargo build

# Run
cargo run -- https://example.com

# Test
cargo test

# Check/format/lint
cargo check
cargo fmt
cargo clippy
```

See `design_docs/graphshell_docs/BUILD.md` for platform prerequisites and extended options.

## Currently Implemented

### Graph UI

- Force-directed graph canvas: webpages are nodes, navigation and associations are edges
- Zoom, pan, smart-fit, keyboard zoom, and graph/detail view controls
- Thumbnail and favicon rendering on nodes with tiered fallback (thumbnail > favicon > lifecycle color)
- Fuzzy search and filtering (nucleo), with highlight and filter display modes
- Node selection, creation, deletion, and explicit edge operations via command/radial/context flows
- View-specific keyboard controls (guarded when text fields are focused)

### Tiled Workspace

- egui_tiles multi-pane layout: graph pane and webview panes coexist in a tile tree
- Per-pane tab bars with close/focus management and workspace-aware open routing
- Three-tier lifecycle (`Active`/`Warm`/`Cold`) with desired-vs-observed runtime reconciliation
- Explicit runtime backpressure and blocked/cooldown state for webview creation failures
- Omnibar with scoped graph search (`@` modes) and URL navigation routed to explicit tile targets

### Servo Integration

- Full webview lifecycle: create, navigate, destroy, track URL/title/history changes
- Delegate-driven semantic event pipeline (`request_create_new`, `notify_url_changed`, `notify_history_changed`)
- Intent/reducer control-plane with reconcile as the side-effect boundary
- Favicon ingestion from Servo's page metadata
- Thumbnail capture from webview rendering output

### Persistence

- Crash-safe local storage: fjall append-only mutation log + redb periodic snapshots + rkyv serialization
- Startup recovery: load latest snapshot, replay log entries since snapshot
- Runtime/session diagnostics and retention controls via Persistence Hub / settings flow
- Encryption-at-rest pipeline (compress + encrypt) and legacy migration path

### Identity and State Model

- Node identity is UUID-based and stable across sessions
- URL is mutable node metadata (duplicate URLs are valid)
- Graph intent reducer is the semantic source of truth; runtime webview state is reconciled

## Current State

M1 foundation is complete. Current active work is M2 architecture and UX stabilization:

- **Registry Migration**: Phase 1 (Input/Action) complete; Phase 2 (Protocols/Viewers) active.
- **Edge Traversal**: History Manager UI landed; archival storage active.
- **Embedder Decomposition**: Stage 4 (GUI/Toolbar split) largely complete (toolbar decomposed into 7 submodules as of 2026-02-23); remaining: Input/Output boundary formalization.
- **UDC Semantic Tagging**: Phase 1 (Registry & Parsing) active.
- **Settings Architecture**: Blocked by Registry Phase 2.
- **Workspace Routing**: Manifest-based persistence and membership complete.

## Planned

### Near-term

- Bookmarks/history import and node tagging UX expansion
- Edge traversal payload migration and History Manager consolidation (Timeline + Dissolved)
- Performance hardening for larger graphs (measurement-driven)
- Continued embedder/runtime decomposition and UI module split
- Accessibility improvements subject to Servo embedder/API surface constraints

### Graph UI (future)

- Rule-based node motility: physics system organizes nodes according to rules and graph structure
- Lasso zoning: prescribe exclusionary or inclusionary sections for specific access or domains
- Lifecycle policy and retention tuning (Active/Warm/Cold with memory pressure demotion)
- Level-of-detail rendering: zooming out groups nodes by time, domain, origin, or relatedness
- Minimap for large graphs
- 2D/3D canvas modes

### Detail View (future)

- Clipping: DOM inspection and element extraction from webpages into graph as independent nodes
- Collapsible groups from hub-connected node clusters
- Drag-and-reorganize reflected in graph structure

### Sessions (future)

- Graph export/import and portable backup workflows
- Individual nodes shareable as standard URLs with metadata
- Ghost nodes to represent deleted nodes while preserving graph shape

### Ergonomics (future)

- Arrow key focus traversal across all interactable elements
- Edge and node types differentiated by line style, shape, color, and icon
- Graph-to-list conversion for screen reader accessibility
- Mods: shareable physics parameters, custom node/edge/filter types, canvas region definitions

## Verse

    Optional, decentralized network component (design phase)

The second half of the project: pooling browsing data into a decentralized, permissions-based peer network.

### P2P Co-op Browsing

- Collaborative browsing where changes to a shared graph synchronize across participants
- Async mode: check in/check out with diffs
- Live mode: version-controlled realtime edits with time-synchronized web processes

## AI Disclaimer

First, a disclaimer: I use and have used AI to support this project.

The idea itself is not the product of AI. I have years of notes in which I drafted the graph browser idea and the decentralized network component. I iterated my way into the insight that users should own their data, not be tracked, and we ourselves can capture much richer browsing insights than trackers. That's the second, prospective half of this project, the Verse bit.

I'm not an experienced developer in the least but I've got opinions, a smidgen of coding experience, and honestly, I want to learn how to use these discursive tools and see how far I can get with them. I've also followed the Servo community for years, despite not being a real developer: please contribute if you are able!

This is an open source, non-commercial effort. These ideas work much better open source forever as far as I'm concerned.

## History

My first inkling of this idea actually came from a mod for the game Rimworld, which added a relationship manager that arranged your colonists or factions spatially with links defining their relationships. It occurred to me that this UI, reminiscent of a mind map, would be a good fit for representing tabs spatially, and that there were a lot of rule-based options for how to arrange not just the browsing data, but tons of data patterns in computing.

I learned there was a name for this sort of UI: a force-directed node graph. A repeating, branching pattern of nodes connected to nodes by lines (edges). The nodes are browser tabs (or any file, document, applet, application, etc.), edges represent the relationship between the two nodes (clicked hyperlink, historical previous-next association, user-associated), and all nodes have both attractive and repellant forces which orient the graph's elements.

Depending on the behavior you want from the graph or the data you're trying to represent, you alter the canvas's physics and node/edge rules/types. You could filter, search, create new rules and implement graph topologies conducive to representing particular datasets: trees, buses, self-closing rings, etc.

This leads to rich, opinionated web browsing datasets, and the opportunity to pool our resources to visualize the accessible web with collective browsing history that is anonymous, permissions- and reputation-based, peer-to-peer, and open source. The best implementation of both halves would be somewhere between federated googles combined with subreddits with an Obsidian-esque personal data management layer.

Other inspirations:

- The Internet Map <https://internet-map.net/>
- YaCy (decentralized search index)
- Syncthing (open source device sync)
- Obsidian (canvas, plugins)
- Anytype (IPFS, shared vaults)
