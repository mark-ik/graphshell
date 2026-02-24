# Registry Architecture Critique & Restructuring Plan (2026-02-23)

**Status**: Historical / Superseded
**Target**: Reconcile with `registry_migration_plan.md`
**Goal**: Pivot registry architecture from Function-First (Layout/Ontology) to Domain-First (Graph/Workbench) to reflect the system's two sovereign territories, and elevate **Modularity** to a core principle where even the Browser Engine (Verso) and Network (Verse) are optional extensions.

> **Editorial note (2026-02-24)**: This document is a point-in-time analysis. Its recommendations have been incorporated into `registry_migration_plan.md` and `TERMINOLOGY.md` with the following nomenclature changes: `GraphLayoutRegistry` → `GraphSurfaceRegistry`, `WorkbenchLayoutRegistry` → `WorkbenchSurfaceRegistry`, `ViewerLayoutRegistry` → `ViewerSurfaceRegistry`, `OntologyDomainRegistry` → `PresentationDomainRegistry`, `OntologyRegistry` (UDC) → `KnowledgeRegistry`, `LensRegistry` → `LensCompositor`, `VerseRegistry` removed (Verse is a native mod). See TERMINOLOGY.md for canonical terms.

## 1. The Structural Critique

**The Flaw**: The current plan creates a generic `LayoutDomain` that conflates **Window Management** (Workbench) with **Data Visualization** (Graph).

*   **Why it fails**: "Layout" implies two distinct responsibilities:
    *   **Workbench Layout**: Container-based (Splits, Tabs, Grids). Rigid, hierarchical, user-driven.
    *   **Graph Layout**: Topology-based (Force-directed, Tree, Radial). Organic, mathematical, data-driven.
    *   Grouping them under one `LayoutRegistry` implies interchangeable strategies, which is semantically incorrect.

**The Fix**: Elevate **Graph** and **Workbench** to top-level Domain Registries.

## 2. The Two-Pillar Architecture

Reorganize registries to reflect the two primary data structures: the **Node Graph** (File System) and the **Tile Tree** (Window Manager).

### Pillar A: The Graph Domain (The "File System")
*Responsible for the semantic web of nodes and edges.*

1.  **`GraphPolicyRegistry` (New)**: Defines the "Physics of Logic".
    *   *Role*: Enforces topological invariants.
    *   *Examples*: "Is this graph directed?", "Are cycles allowed?", "Can I create an edge between these types?".
    *   *Use Case*: A "File Tree" mode enforces a strict DAG. A "Mind Map" mode allows cycles.
2.  **`GraphLayoutRegistry`**: Defines spatial arrangement.
    *   *Examples*: Force-Directed, Radial, Tree, Grid.
3.  **`PhysicsRegistry`**: Defines dynamics of the layout.
    *   *Correction*: Moved from Ontology domain. Physics is about movement/dynamics, not semantic meaning.
4.  **`OntologyRegistry`**: Defines semantics of nodes/edges.
    *   *Examples*: UDC tags, Schema.org types, custom node types.

### Pillar B: The Workbench Domain (The "Window Manager")
*Responsible for the tile tree, panes, and user interface.*

1.  **`WorkbenchPolicyRegistry` (New)**: Defines interaction rules.
    *   *Examples*: "Can I split this pane?", "Do tabs auto-group?", "Is this a fixed dashboard or a tiling WM?".
2.  **`WorkbenchLayoutRegistry`**: Defines structural arrangement.
    *   *Examples*: Binary Split, Golden Ratio, Stacked, Tabbed-Only.

### Pillar C: Shared Services (The "Kernel")
*Resources used by both pillars.*

1.  **`ActionRegistry`**: Verbs/Commands.
2.  **`InputRegistry`**: Bindings.
3.  **`ViewerRegistry`**: Renderers (how a Node is drawn inside a Pane).
4.  **`ProtocolRegistry`**: I/O and Persistence.
5.  **`ModRegistry`**: The dependency resolver and capability loader.

## 3. The Modularity Thesis: Verso & Verse as Mods

A "Module Registry" should be the place where cross-domain dependent manifestations of multiple primitives exist.

*   **Graphshell Core**: The minimal runtime (Graph + Workbench + Registry Kernel). Capable of functioning as an offline document organizer ("The Notebook Mode") without a web engine or network.
*   **Verso (The User Agent)**: A **Default Mod**.
    *   Registers: `ViewerRegistry` (HTML/Servo), `ProtocolRegistry` (HTTP/HTTPS).
    *   Dependency: None (Core).
*   **Verse (The Network)**: A **Default Mod**.
    *   Registers: `IdentityRegistry` (Keys), `ProtocolRegistry` (IPFS/Iroh), `IndexRegistry` (Federated).
    *   Dependency: Registry prerequisites.

This decoupling ensures the application is not monolithic. If a user wants a local-only Markdown graph, they disable the Verso and Verse mods.

## 4. Semantic Gaps & Corrections

1.  **Physics Placement**: `PhysicsRegistry` belongs in the **Graph Domain**, not Ontology. It governs dynamics, not meaning.
2.  **The "Policy" Gap**: Hardcoded rules (e.g., "Shift+Double Click = UserGrouped edge") should be data-driven policies in `GraphPolicyRegistry`.
3.  **Lens Definition**: A `Lens` configures the **Graph Domain**.
    *   `Lens` = `GraphPolicy` + `GraphLayout` + `Physics` + `OntologyFilter`.
4.  **Session Definition**: A `Session` configures the **Workbench Domain**.
    *   `Session` = `WorkbenchPolicy` + `WorkbenchLayout` + `InputProfile`.

## 5. The "File Tree" Realization

A **File Explorer** is not a separate mode; it is a specific configuration of the Graph Registry:
*   **Policy**: Strict Hierarchy (DAG), Unique Names per Parent.
*   **Layout**: Indented Tree List.
*   **Physics**: None (Static).
*   **Ontology**: Files and Folders.

## 6. Migration Impact

The `registry_migration_plan.md` should be refactored to adopt this structure:

**Old Structure:**
`src/registries/{atomic, domain}` where domain = `layout`, `ontology`.

**New Structure:**
`src/registries/{atomic, domain}` where domain = `graph`, `workbench`, `system`.

### Refactoring Steps
1.  **Split Layout Domain**: Separate `GraphLayoutRegistry` and `WorkbenchLayoutRegistry`.
2.  **Introduce Policy Registries**: Scaffold `GraphPolicy` and `WorkbenchPolicy`.
3.  **Move Physics**: Relocate `PhysicsRegistry` to Graph Domain.
4.  **Redefine Lens/Session**: Update config structs to match the new domain boundaries.

---

reconcile 2026-02-23_registry_architecture_critique.md,

2026-02-22_registry_layer_plan.md,

and registry_migration_plan.md

into one comprehensive, contradiction free, best of all worlds and semantically consistent registry layer plan. don't edit the files, just discuss what that should look like.

let me be clear: I want to consolidate the other two plans into the registry layer plan and archive them.

---

#### Gemini's analysis

Here is a synthesis of the three documents into a single, cohesive architectural vision. This proposal resolves the structural conflicts by adopting the Two-Pillar (Graph/Workbench) model from the critique while preserving the Mod-First execution strategy from the original plans.

Unified Registry Architecture: The Two-Pillar Plan
1. Core Philosophy
Graphshell is not a monolithic browser; it is a Universal Graph Runtime. It consists of two sovereign territories ("Pillars") supported by a shared kernel.

Pillar A: The Graph (Data). The semantic web of nodes and edges. It has its own physics, topology, and logic. It is the "File System."
Pillar B: The Workbench (View). The tile tree of panes and tabs. It has its own constraints, containers, and user interactions. It is the "Window Manager."
The Kernel (Shared). The vocabulary used by both pillars (I/O, Actions, Identity, Mods).
2. The Registry Map
We move from a functional grouping (Layout/Ontology) to a domain grouping (Graph/Workbench/Kernel).

Pillar A: The Graph Domain
Configured by a Lens.

GraphPolicyRegistry (New): Defines the "Physics of Logic".
Responsibility: Enforces invariants (e.g., "Is this a DAG?", "Are cycles allowed?", "Can I link A to B?").
Use Case: A "File Tree" mode enforces strict hierarchy; a "Mind Map" mode allows anything.
GraphLayoutRegistry: Defines spatial arrangement algorithms.
Entries: force_directed, radial, tree, grid.
PhysicsRegistry: Defines dynamics and forces.
Moved: From Ontology/Presentation to Graph. Physics is about movement, not appearance.
OntologyRegistry: Defines semantic meaning.
Responsibility: UDC tagging, schema validation, semantic distance.
Pillar B: The Workbench Domain
Configured by a Session.

WorkbenchPolicyRegistry (New): Defines interaction rules.
Responsibility: "Can I split this pane?", "Do tabs auto-group?", "Is this a tiling WM or a floating dashboard?".
WorkbenchLayoutRegistry: Defines container structures.
Entries: binary_split, golden_ratio, stacked_tabs.
Pillar C: The Kernel (Atomic Primitives)
Resources used by both pillars.

ProtocolRegistry: I/O (HTTP, IPFS, File).
ViewerRegistry: Content rendering (HTML, PDF, Markdown).
ActionRegistry: Executable commands.
InputRegistry: Bindings (Keyboard/Mouse -> Actions).
IdentityRegistry: Keys and personas.
ThemeRegistry: Visual tokens (colors, fonts) shared by Graph and Workbench.
ModRegistry: The dependency resolver and loader.
3. The Modularity Thesis
The application is built as a small Core extended by Native Mods.

Graphshell Core: The offline "Notebook Mode".
Capabilities: Local file protocol, plaintext/metadata viewers, graph organization, workbench tiling.
No: Web engine, P2P networking.
Verso (Native Mod): The "Browser" capability.
Registers: viewer:webview (Servo), protocol:http, protocol:https.
Verse (Native Mod): The "Network" capability.
Registers: protocol:ipfs, identity:did, action:verse.share.
4. Reconciled Migration Plan
The execution order in registry_migration_plan.md needs to be adjusted to reflect the split between Graph and Workbench domains.

Phase 1: Kernel Foundation (In Progress)
Goal: Stand up the Atomic registries that power everything else.
Steps:
Input & Action Registries (Complete).
Protocol & Viewer Registries (Active).
Mod Infrastructure (Loader & Manifest).
Phase 2: The Workbench Domain (Split from "Layout")
Goal: Formalize the Tile Tree behavior.
Steps:
Create WorkbenchPolicyRegistry and WorkbenchLayoutRegistry.
Migrate tile_behavior.rs logic into these registries.
Define Session configuration format.
Phase 3: The Graph Domain (Split from "Layout")
Goal: Formalize the Node Graph behavior.
Steps:
Create GraphPolicyRegistry and GraphLayoutRegistry.
Move PhysicsRegistry into this domain.
Migrate render/mod.rs logic (interaction/style) into these registries.
Define Lens configuration format.
Phase 4: Presentation & Ontology
Goal: Visuals and Semantics.
Steps:
Implement ThemeRegistry (Atomic).
Implement OntologyRegistry (Graph Domain).
Wire UDC tagging and semantic physics.
Phase 5: The Mods (Verso & Verse)
Goal: Package the heavy subsystems.
Steps:
Package Servo integration as the Verso Mod.
Package P2P logic as the Verse Mod.
Summary of Changes to Existing Docs
registry_migration_plan.md: Needs a rewrite of Phase 3 ("Layout Domain") to split it into "Workbench Domain" and "Graph Domain".
2026-02-22_registry_layer_plan.md: Needs to update the hierarchy diagram to reflect the Two-Pillar structure and move Physics out of Presentation.
2026-02-23_registry_architecture_critique.md: This document is the source of the new direction and can be archived once the other two are updated.

---

#### Copilot's analysis (claude haiku)

I've reviewed all three documents in detail. Here's what a reconciled, best-of-all-worlds plan would look like:

High-Level Structure
The Three Documents Today:

registry_layer_plan.md (2026-02-22): The destination architecture — atomic registries, domain composites, testing strategy, technical stack
registry_architecture_critique.md (2026-02-23): Point-in-time analysis identifying semantic gaps — proposes splitting Layout into GraphLayout/WorkbenchLayout, moving Physics to Graph Domain, elevating Verso/Verse as mods
registry_migration_plan.md (2026-02-22): The execution roadmap — 6 phases of iterative work, topology restructuring, mod-first principle
Current State: The critique's recommendations have partially landed in the migration plan (via the mod-first principle and renamed registries), but the layer plan hasn't fully incorporated the critique's two-pillar structure concept.

What the Reconciled Plan Should Look Like
📐 Architecture Layer (Destination)
A single, unified "Registry Architecture" document combining:

1. Three Data Territories (Two-Pillar + Kernel)

Graph Domain (Pillar A): Node graph, edges, semantic meaning

GraphSurfaceRegistry (layout algorithms, interaction policy, rendering policy, physics execution)
LayoutRegistry (atomic: positioning algorithms)
KnowledgeRegistry (UDC/taxonomy, semantic distance)
Physics parameter presets → Presentation Domain (not here)
Workbench Domain (Pillar B): Tile tree, window management

WorkbenchSurfaceRegistry (layout policy, interaction rules, rendering)
Equivalent scope to Graph Domain but for "File Tree"
Kernel (Shared Services): Cross-cutting primitives

Atomic I/O: ProtocolRegistry, ViewerRegistry, IndexRegistry
Atomic Logic: ActionRegistry, InputRegistry, AgentRegistry
Atomic Security: IdentityRegistry, ModRegistry
Atomic Observability: DiagnosticsRegistry
2. Presentation Layer (Decoupled from Layout)

PresentationDomainRegistry (coordinator)
ThemeRegistry, PhysicsProfileRegistry (semantic labels over force params)
Principle: Resolved after layout; physics engine execution stays in Graph Domain
3. Cross-Domain Composites

LensCompositor (not LensRegistry): Graph layout + Graph physics params + Theme + Filters + Knowledge tags
SessionRegistry (future): Workbench layout + Input profile + Window state
4. Mod-First Foundation (from migration plan)

Two tiers: Native (compiled, inventory::submit!) → Verso, Verse, defaults
WASM (sandboxed, extism) → third-party extensions
Core seed floor: , about:, plaintext/metadata viewers, graph actions, local identity
Verso mod: http://, https://, viewer:webview
Verse mod: ipfs://, activitypub://, federated search, P2P identity
🚀 Execution Layer (Migration Roadmap)
Keep the 6-phase structure from registry_migration_plan.md but with corrected phase definitions:

Phase 1: Core Decoupling (Protocols & Viewers) — Registry ownership of URL dispatch ✅ (complete)
Phase 2: Mod Infrastructure — ModManifest, dependency resolver, native loader, core seeds, Verso as native mod
Phase 3: Atomic Registries — Input, Action, Layout, Theme (all the "easy" ones)
Phase 4: Layout Domain — The two-pillar realization: GraphSurfaceRegistry, WorkbenchSurfaceRegistry, ViewerSurfaceRegistry with explicit scope (structure + interaction + rendering, not just positioning)
Phase 5: Presentation Domain — Theme + PhysicsProfile coordinators; remove layout_mode from PhysicsProfile
Phase 6: Knowledge + Verse — KnowledgeRegistry (UDC), Verse native mod with P2P capabilities
Phase 7: Topology Consolidation — Filesystem alignment: src/{model, registries, services, mods, shell}
Key insight from critique: The ordering matters. The two-pillar idea isn't a restructuring — it's a conceptual clarification that should influence how Phase 4/5 are designed. GraphSurfaceRegistry and WorkbenchSurfaceRegistry aren't just sub-registries; they're co-equal domain coordinators.

✅ Testing & Validation Layer
From registry_layer_plan.md, keep:

Contract tests + harness scenario matrix
Diagnostics checklist per registry
Registry Contract Matrix (with VerseRegistry removed, KnowledgeRegistry/PhysicsProfileRegistry renamed)
No changes needed here; it's solid.

🎯 Semantic Alignment
Resolved contradictions:

Concept	Critique Says	Layer Plan Says	Migration Plan Says	Reconciled
Physics placement	Graph Domain	Presentation (implied)	Presentation params, Layout execution	Split: Engine execution in GraphSurfaceRegistry (Layout), parameter presets in PhysicsProfileRegistry (Presentation)
Layout scope	Both Graph/Workbench	Single LayoutRegistry	Surface registries	Both: Atomic LayoutRegistry (algorithms) + GraphSurfaceRegistry + WorkbenchSurfaceRegistry (structure/interaction/rendering)
Verse/Verso	Native mods, not domain registries	Implied via core seeds	Explicit native mods	Unified: Both are native mods; Core Seed Floor is the architectural enforcement
Lens definition	Configures Graph Domain	Composes layout/theme/physics	Composes via LensCompositor	Unified: LensCompositor resolves per domain sequencing: Layout first → Presentation second
Two territories	Graph vs Workbench (pillar concept)	Not explicit	Implicit in Phase 3/4	Explicit: Graph and Workbench are co-equal domain pillars with identical surface registry patterns
📋 The Best-of-All-Worlds Advantage
A reconciled plan would gain:

Conceptual clarity: The two-pillar framing makes it obvious why GraphSurfaceRegistry and WorkbenchSurfaceRegistry exist as distinct entities (they govern two different data structures with different interaction models, not just "two layout algorithms")

Execution confidence: Phase 4 becomes measurably clear — you're not generalizing "layout," you're filling out the parallel surface registries for two domains that happen to have the same contract shape

Extensibility path: Future mods can register new GraphSurfaceRegistry entries (e.g., "Semantic Clustering" layout algorithm) and new WorkbenchSurfaceRegistry entries (e.g., "Golden Ratio" split policy) independently

Mod coherence: The critique's realization that "Verso and Verse are mods, not domains" becomes the foundation, not an afterthought. Core seed floor enforces this principle.

Physics clarity: No more ambiguity about where physics lives. Engine execution = Layout. Parameter presets = Presentation. Lens orchestrates both.

What Should Stay Separate
Layer plan (architecture + testing + technical stack) — destination state
Migration plan (execution phases + topology + risks) — journey to destination
Critique (now marked as "Historical / Superseded") — explains the reasoning behind the split
The critique becomes living documentation of why the architecture looks like it does, not prescriptive.

Bottom line: The three documents are nearly aligned now. What's missing is an explicit "two-pillar" theme in the layer plan that clarifies Graph/Workbench as co-equal domains, and a small reordering of the migration plan phases to surface Phase 2 (Mod Infrastructure) as the enabling prerequisite for all domain phases. Everything else is already harmonized by the mod-first principle and the renamed registries.

---

#### Claude's analysis

Good. All three are read. Let me lay out the analysis.

The Three Documents' Roles
Before reconciling, it's worth naming what each document actually is:

_layer_plan — the architecture specification: registry catalog, interface contracts, technical stack, done gates, testing rules
registry_migration_plan — the execution journal: concrete phase steps, what's been completed, target filesystem topology, risks
_architecture_critique — already marked "Historical / Superseded" via its own editorial note; its recommendations were meant to be absorbed into the other two
So this is really a two-document reconciliation, with the critique serving as a conceptual arbitration layer.

What Each Contributes Uniquely
From the critique (that isn't fully absorbed yet):

The cleaner semantic framing: Graph Domain = "the file system", Workbench Domain = "the window manager"
The Lens vs. Session distinction: Lens configures the Graph domain; Session/Workflow configures the Workbench domain. The layer plan's LensCompositor currently spans both domains (it includes WorkbenchSurface via the Layout domain coordinator), which blurs this boundary
The "File Explorer as a Lens config" realization — a concrete proof of the model's correctness
The explicit articulation that topological invariants (directed? cycles allowed? valid edge types?) are a distinct concern from spatial layout algorithms
From the layer plan:

All the good *SurfaceRegistry naming (already absorbed the critique's naming)
The PhysicsProfileRegistry vs. engine execution split (cleaner than the critique's PhysicsRegistry)
LensCompositor (not LensRegistry)
KnowledgeRegistry (not OntologyRegistry)
No VerseRegistry — Verse is a native mod
Technical stack: tower, inventory, extism, sophia, schemars
Contract matrix, diagnostics checklist, done gates, testing rules
RegistryContext / GraphWorkspace / AppServices split (solves borrow-checker concerns)
From the migration plan:

Phase ordering: input/actions first — confirmed correct by execution (Phase 1 complete)
Target filesystem topology: src/{model,registries,mods,services,shell}
Core seed table (the matrix of what each registry provides without mods)
Topology migration rules (thin slices, no big-bang moves)
Risks & Mitigations — not present in other docs
The Contradictions
1. Phase Numbering (resolvable: migration plan wins)
The layer plan's phases don't match the migration plan's:

Layer Plan	Migration Plan
Phase 0: Walking Skeleton	(absorbed, already done)
Phase 1: Protocols & Viewers	Phase 2: Mod Infra + Protocol/Viewer
Phase 2: Mod Infrastructure	—
Phase 3: UX Flexibility (Actions, Input, Lenses)	Phase 1: Input & Actions (done)
Phase 4: Layout Domain	Phase 3: Layout Domain
Phase 5: Presentation + Knowledge	Phase 4: Presentation + Knowledge
Phase 6: Verse	Phase 5: Verse
Phase 7: Topology	Phase 6: Topology
Resolution: The migration plan's ordering is correct and its Phase 1 is already complete. The layer plan's early phase descriptions are superseded by execution. The reconciled plan uses migration plan numbering as canonical.

2. Lens scope (real tension, needs a decision)
The layer plan says LensCompositor composes: Layout + Presentation + Knowledge + Filters — where Layout includes WorkbenchSurfaceRegistry.

The critique says: Lens = GraphPolicy + GraphLayout + Physics + OntologyFilter (Graph domain only); Session = WorkbenchPolicy + WorkbenchLayout + InputProfile (Workbench domain only).

These are semantically different. The critique's version is cleaner: a Lens is a named graph view configuration, not a complete UI state. A Workflow/Session is what selects which Lens is active plus what Workbench layout wraps it.

Resolution: LensCompositor should compose Graph-domain configs only (GraphSurface + Presentation + Knowledge + Filters). WorkflowRegistry (already marked "Future" in the layer plan) is what the critique called Session — it activates a Lens plus InputProfile plus Workbench configuration. This clarifies that a Lens is reusable across different workbench configurations, which is the right model.

3. Topological policy placement (minor, a judgment call)
The critique argues GraphPolicyRegistry (topological invariants: directed? cycles? valid edge types?) should be separate from GraphLayoutRegistry (spatial arrangement). The layer plan folds this into GraphSurfaceRegistry's "interaction policy" scope.

These are semantically distinct: the surface registry's interaction policy governs UX affordances (selection modes, zoom ranges, node creation positions); topological invariants govern graph theory constraints (what the critique calls "Physics of Logic").

Resolution: Don't create a separate registry yet (that's premature abstraction at this stage), but GraphSurfaceRegistry should have an explicit topology policy section clearly distinct from its interaction and rendering policy sections. The "File Explorer mode" example from the critique (DAG enforcement, unique names per parent, static physics, tree layout) should be called out as the canonical example of a Lens config that sets topology policy. If the surface registry grows too large, extracting GraphPolicyRegistry is a clean future slice.

The Reconciled Structure
Atomic Registries (Kernel)
Protocol, Viewer, Index, Identity, Action, Input, Knowledge, Agent, Diagnostics, Mod — unchanged, the layer plan has this right

Domain: Graph (Pillar A — the "file system")
GraphSurfaceRegistry with three explicit sections:

Topology policy: directed/undirected, cycle rules, edge type constraints — what the critique called "Physics of Logic"
Layout algorithms: Force-directed, Tree, Radial, Grid — what the critique called GraphLayoutRegistry
Interaction & rendering policy: Selection modes, node creation, edge routing, badge display — the layer plan's existing scope
Coordinated by LayoutDomainRegistry

Domain: Workbench (Pillar B — the "window manager")
WorkbenchSurfaceRegistry with two explicit sections:

Layout policy: split types, tab rules, SimplificationOptions — what the critique called WorkbenchLayoutRegistry
Interaction policy: drag/drop rules, resize constraints, drop zone behavior — what the critique called WorkbenchPolicyRegistry
Domain: Viewer (Shared)
ViewerSurfaceRegistry: zoom, reader mode, scroll policy — unchanged

Domain Coordinator: Layout
LayoutDomainRegistry coordinates the three surface registries — unchanged

Domain: Presentation
PresentationDomainRegistry + ThemeRegistry + PhysicsProfileRegistry — unchanged, the layer plan has this right

Cross-Domain: Compositor
LensCompositor → composes GraphSurface profile + Presentation profile + Knowledge filters = a named Lens (Graph-domain view configuration only — Workbench is not in scope for a Lens)

Session Layer (Future)
WorkflowRegistry → active Lens + active InputProfile + active WorkbenchSurface profile = what the critique called a Session

This makes the semantic hierarchy unambiguous: Workflow = Lens × Session.

Phase Sequence (Canonical)
Inheriting from migration plan since Phase 1 is done:

Phase 1 ✅ Complete — Input & Actions + topology slice (1.4)
Phase 2 — Mod Infrastructure + Protocol/Viewer Contracts (Verso-as-mod, core seed floor)
Phase 3 — Layout Domain (GraphSurface with explicit topology-policy section, WorkbenchSurface, ViewerSurface)
Phase 4 — Presentation Domain + KnowledgeRegistry
Phase 5 — Verse Native Mod
Phase 6 — Topology Consolidation (src/{model,registries,mods,services,shell})
What This Means for the Docs
The consolidated plan should:

Use the migration plan as the primary execution document (it has phase status, topology, risks)
Use the layer plan as the primary reference document (it has the catalog, interfaces, contract matrix, done gates)
Absorb the critique's remaining unique insight — the Lens vs. Session semantic distinction and the topology-policy-as-distinct-concern — into the layer plan and migration plan directly
The critique document can stay as-is ("Historical/Superseded") since it's already marked that way
The only new language that needs to land somewhere is: explicitly calling WorkflowRegistry = Lens × Session and defining the topology-policy section of GraphSurfaceRegistry by name.