# Graphshell Terminology

**Status**: Living Document
**Goal**: Define canonical terms for the project to ensure consistency across code, documentation, and UI.

## Core Identity

*   **Graphshell**: The product name. A local-first, spatial web browser.
*   **Spatial Graph Browser**: The user-facing description of the interface. It emphasizes the force-directed graph and tiling window manager.
*   **Knowledge User Agent**: The architectural philosophy. Unlike a passive "User Agent" that just renders what servers send, Graphshell actively crawls, indexes, cleans, and stores data on the user's behalf.
*   **Verse**: The optional decentralized, peer-to-peer network component for sharing graph data.
*   **Verso**: The internal user agent component (the engine wrapper around Servo/Wry). An homage.

## Interface Components

*   **Workbench**: The top-level container (IDE Window) that manages the tile-tree, layout of panes and workspaces.
*   **Workspace**: A persistable arrangement of panes where web content is rendered, within the tile-tree managed by the Workbench (a "Project Context").
*   **Pane**: A single tile in the Workbench (e.g., a Graph View, a Webview, a Diagnostic panel).
*   **Graph View**: A pane containing a force-directed canvas visualization.
*   **Diagnostic Inspector**: A specialized pane for visualizing system internals (Engine, Compositor, Intents).
*   **Lens**: A named configuration composing a Layout, Theme, Physics Profile, and Filter(s). Defines how the graph *looks* and *moves*.
*   **Command Palette**: A modifiable context menu that serves as an accessible interface for executing Actions.
*   **The Register**: The central system component that contains all Atomic and Domain registries, manages the inter-registry Signal Bus, and exposes configuration logic (Control Panel).


## Data Model

*   **Graph**: The persistent data structure containing Nodes and Edges. Acts as the "File System".
*   **Node**: A unit of content (webpage, note, file) identified by a stable UUID.
*   **Edge**: A relationship between two nodes.
    *   **UserGrouped**: Explicit connection made by the user (flag on Edge).
    *   **Traversal-Derived**: Implicit connection formed by navigation events.
*   **Traversal**: A temporal record of a navigation event (timestamp, trigger) stored on an Edge.
*   **Edge Traversal History**: The aggregate of all Traversal records, forming the complete navigation history of the graph. Replaces linear global history.
*   **Intent**: A data payload (`GraphIntent`) describing a desired state change. The fundamental unit of mutation in the system.
*   **Session**: A period of application activity, persisted via a specific write-ahead log (WAL).
*   **Tag**: A user-applied string attribute on a Node (e.g., `#starred`, `#pin`) used for organization and system behavior.

## Visual System

*   **Badge**: A visual indicator on a Node or Tab representing a Tag or system state (e.g., Crashed, Unread).

## Runtime Lifecycle

*   **Active**: Node has a live webview and is rendering.
*   **Warm**: Node has a live webview but is hidden/cached (optional optimization).
*   **Cold**: Node has no webview; represented by metadata/snapshot only.

## Registry Architecture

*   **Atomic Registry (Primitive)**: A registry that manages specific, isolated resources or algorithms. The "Vocabulary".
    *   *List*: `ProtocolRegistry`, `IndexRegistry`, `ViewerRegistry`, `LayoutRegistry`, `ThemeRegistry`, `PhysicsRegistry`, `ActionRegistry`, `IdentityRegistry`, `ModRegistry`, `OntologyRegistry`, `AgentRegistry`.
*   **Domain Registry (Composite)**: A registry that combines primitives to define a user experience context. The "Sentences".
    *   *Examples*: `LensRegistry`, `InputRegistry`, `VerseRegistry`.
*   **Action**: An executable command defined in the `ActionRegistry`.
*   **Mod**: A WASM-based extension unit that registers new capabilities.

## Network & Sync (Verse)

*   **Report**: A recorded jump from one webpage to another (Source -> Destination + Metadata). The fundamental unit of sharing.
*   **Tokenization**: The process of anonymizing a Report and minting it as a unique digital asset.
*   **Peer**: A device participating in the Verse network.
*   **Lamport Clock**: A logical clock used to order events in the distributed system.

## Legacy / Deprecated Terms

*   *Context Menu*: Replaced by **Command Palette** (context-aware).
*   *EdgeType*: Replaced by **EdgePayload** (containing Traversals).
*   *View Enum*: Replaced by **Workbench** tile state.
*   *Servoshell*: The upstream project Graphshell forked from.
```