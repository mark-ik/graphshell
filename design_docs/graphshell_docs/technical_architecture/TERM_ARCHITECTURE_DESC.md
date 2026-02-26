# Terminology-Aligned Architecture Description

## Regeneration Command

Run this prompt every time `design_docs/TERMINOLOGY.md` changes:

"Completely describe the app's architecture with the terms in TERMINOLOGY.md and update this description every time the TERMINOLOGY.md file changes."

---

Graphshell's architecture is a layered system of `Tile Tree` UI hosting, `The Register` runtime composition, `Domains` and `Registries` for behavior resolution, `Aspects` for synthesized runtime systems, and `Subsystems` for cross-cutting guarantees.

## 1. Core Identity

- **Graphshell** is a local-first **Spatial Graph Browser** / **Knowledge User Agent**.
- It combines:
  - a persistent **Graph** (nodes/edges/traversal history)
  - a tiled **Workbench** UI (tile tree + panes)
  - runtime **Registries** and **Domains** (behavior contracts and policy resolution)
  - cross-cutting **Subsystems** (guarantees and validation)

## 2. UI Structure: Tile Tree -> Pane -> Surface

- The visible app is a **Workbench**.
- The Workbench is built from a recursive **Tile Tree** (`egui_tiles`):
  - **Tile** = fundamental layout node
  - **Container** = branch tile (`Tab Group`, `Split`, `Grid`)
  - **Pane** = leaf tile (content host)
- A **Tab** is only a selection affordance inside a `Tab Group`, not a pane.
- A **Pane** hosts a **Surface** (architectural UI manifestation):
  - graph canvas surface
  - viewer/document surface
  - tool/subsystem surface
- Pane payloads are represented by `TileKind`:
  - `TileKind::Graph(GraphViewId)`
  - `TileKind::Node(NodePaneState)`
  - `TileKind::Tool(ToolPaneState)`

## 3. Workbench / Workspace / Workflow

- **Workbench** = the active top-level UI runtime (tile tree, toolbar, status, toasts).
- **Workspace** = persisted snapshot of workbench layout + content manifest + metadata.
- **WorkbenchProfile** = workbench/input configuration component of a **Workflow**.
- **Workflow** = `Lens × WorkbenchProfile`
  - **Lens** controls look/motion/filter composition
  - **WorkbenchProfile** controls tile-tree/input behavior

## 4. Data Model (Persistent App State)

- **Graph** is the core persisted data structure.
- **Node** = content unit (webpage, note, file)
- **Edge** = relationship between nodes
- **Traversal** = temporal navigation record stored on edges
- **Edge Traversal History** = full temporal navigation history (replaces linear history)
- **Intent** (`GraphIntent`) = unit of desired state change
- **Session** = WAL-backed period of activity (persistence/temporal concept)
- **Tag** = user/system classification attribute on nodes

## 5. The Register (Runtime Composition Root)

- **The Register** is the root runtime infrastructure host.
- In code terms, it is currently:
  - `RegistryRuntime`
  - `ControlPanel`
  - a transitional signal/event routing layer (planned `SignalBus` or equivalent)
- It owns/supervises:
  - Atomic and Domain registries
  - mod loading/wiring
  - runtime coordination surfaces/processes
  - inter-registry signal/event routing

This is the main architectural composition boundary, not just a convenience label.

## 6. Control Panel (Runtime Coordinator)

- **Control Panel** is the async coordination/process host.
- It is a peer coordinator (not owner) for registries, subsystems, mods, and UI surfaces.
- It supervises background workers and feeds **QueuedIntent** into the deterministic sync reducer path.
- It should not absorb all runtime architecture responsibilities; it is one core part of **The Register**.

## 7. Registries (Contracts and Implementations)

### Atomic Registries (Primitives)

Atomic registries define capability contracts and hold implementations (often mod-provided), e.g.:
- `ProtocolRegistry`
- `ViewerRegistry`
- `IndexRegistry`
- `ActionRegistry`
- `AgentRegistry`
- `IdentityRegistry`
- `KnowledgeRegistry`
- `DiagnosticsRegistry`
- `ModRegistry`
- `LayoutRegistry` (algorithm store)

### Domain Registries (Composite/Subregisters)

**Domain Registries** group primitives by semantic concern and evaluation order:
- `LayoutDomainRegistry`
- `PresentationDomainRegistry`
- `InputRegistry` (as a primary domain coordinator in the terminology grouping)

## 8. Domains (Behavior Categories + Sequencing)

A **Domain** is an architectural concern boundary and evaluation layer.
It answers “what class of behavior is being resolved?” and “in what order?”

Current primary domains:
- **Layout Domain**
  - structure, arrangement, interaction policy before styling
  - includes `CanvasRegistry`, `WorkbenchSurfaceRegistry`, `ViewerSurfaceRegistry`
- **Presentation Domain**
  - appearance and motion semantics after layout
  - includes `ThemeRegistry`, `PhysicsProfileRegistry`
- **Input Domain**
  - input interpretation/routing/binding behavior

Key rule:
- **Domain sequencing principle**: resolve layout first, then presentation.

## 9. Aspects (Synthesized Runtime Systems)

An **Aspect** is the synthesized runtime concern-oriented system that uses registry/domain capabilities to perform a task family.
- It may be headless or UI-backed.
- It may expose one or more surfaces.

Examples (conceptually):
- runtime coordination (Control Panel behavior) as an aspect
- future multi-agent orchestration as aspects
- viewer/canvas orchestration systems that synthesize registry policies into runtime behavior

This is the missing middle layer between registries/domains and user-facing surfaces.

## 10. Surfaces (Architectural UI Manifestations)

A **Surface** is the UI presentation/interaction manifestation of a domain, aspect, or subsystem.
Examples:
- graph canvas surface
- workbench tile-tree surface
- viewer/document surface
- subsystem/tool pane surfaces

Important distinction:
- **Surface != Pane**
- A **Pane** is the tile-tree host unit
- A **Surface** is what the pane presents

## 11. Subsystems (Cross-Cutting Guarantees)

A **Subsystem** is a cross-cutting runtime guarantee domain where silent contract erosion is the main risk.

Graphshell's five subsystems:
- `diagnostics`
- `accessibility`
- `security`
- `storage`
- `history`

Long forms:
- Diagnostics Subsystem
- Accessibility Subsystem
- Security & Access Control Subsystem
- Persistence & Data Integrity Subsystem (`storage`)
- Traversal & Temporal Integrity Subsystem (`history`)

Each subsystem is defined by four layers:
1. Contracts / invariants
2. Runtime state
3. Diagnostics
4. Validation

Subsystems apply across domains/aspects/surfaces rather than replacing them.

## 12. Subsystem Panes / Tool Panes / Settings Pane

- **Tool Pane** = non-document pane under `TileKind::Tool(ToolPaneState)`
- **Subsystem Pane** = tool pane for subsystem state/health/config/operations
- **Settings Pane** = tool pane aggregating configuration across registries, subsystems, and app-level preferences

These are UI access points into subsystem/aspect behavior, not the architecture itself.

## 13. Capability Declarations and Conformance

- **Surface Capability Declarations** are folded into owning surface/viewer registries (not a standalone capability subsystem).
- They describe claimed support per subsystem (e.g. full/partial/none).
- **Subsystem Conformance** is the measured/evaluated outcome (tests/diagnostics/health), distinct from claims.

This prevents overloading “capability” and supports future-proof validation.

## 14. Degradation and Health

- **Degradation Mode** describes explicit reduced-operation states (`full`, `partial`, `unavailable`)
- **Subsystem Health** is the standardized health state derived from diagnostics + invariants + validation signals

This is how the architecture stays maintainable as features and mods expand.

## 15. Mod-First Runtime Extension Model

- Registries define contracts; mods populate them.
- **Mod-first principle**: app remains functional with core seeds, no mods loaded.
- **Native Mods** and **WASM Mods** register capabilities into registries.
- **Verso** and **Verse** are examples of native mods supplying major capabilities.

## 16. Signal Routing / SignalBus (Planned)

- `SignalBus` is currently a planned (or equivalent) abstraction, not fully implemented.
- Architectural role:
  - typed signals
  - pub/sub
  - decoupled observers
  - cross-registry and subsystem propagation
- It belongs under **The Register**, not as a synonym for `ControlPanel`.

## 17. Concise Relationship Model

- **Registry**: contract + implementations
- **Domain**: concern category + evaluation order
- **Aspect**: synthesized runtime system for a task family
- **Surface**: UI manifestation of a domain/aspect/subsystem
- **Pane**: tile-tree host unit containing a surface
- **Subsystem**: cross-cutting guarantees over domains/aspects/surfaces
- **The Register**: composition root that hosts registries, control panel, and signal routing
- **Control Panel**: async coordinator/process host within the Register

This description should be regenerated whenever `design_docs/TERMINOLOGY.md` changes.
