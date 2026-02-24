# Registry Layer Architecture Plan (2026-02-22)

**Status**: In Progress
**Supersedes**: `registry_migration_plan.md`, `2026-02-23_registry_architecture_critique.md` (archived to `archive_docs/checkpoint_2026-02-23/`)
**Goal**: Decompose Graphshell's monolithic logic into a modular ecosystem of registries, enabling extensibility and the "Knowledge User Agent" vision.

## Context

Graphshell currently hardcodes behavior for URL handling, rendering, and action dispatch. To support the "Verse" vision (P2P, alternative protocols, custom viewers) and advanced UX (Lenses, Agents), we need a mod-extensible architecture where capabilities are registered rather than hardcoded.

The architecture follows a **mod-first principle**: registries define capability contracts (empty surfaces with fallback defaults); mods populate them with implementations. The application must be fully functional as an offline graph organizer with only core seeds (no mods loaded). Both Verso (web rendering) and Verse (P2P networking) are native mods, not hardcoded subsystems.

### Migration Policy Note

Since there are no active users, we prioritize **code cleanliness** over backward compatibility. We will replace subsystems directly rather than maintaining parallel legacy paths.

### Iterative Replacement Strategy (Operational)

We execute replacement in thin, test-gated slices:

1. Implement one registry-owned path end-to-end.
2. Add/expand diagnostics contract tests + harness scenario assertions.
3. Make the new path the default runtime path immediately.
4. Delete the replaced legacy branch in the same slice once tests are green.

No long-lived dual-path mode, no feature-flag forks for migrated behavior.

---

## Design Rationale: Two-Pillar Architecture

Graphshell has two sovereign data territories with fundamentally different structure and interaction models:

1. **Graph Domain** (Pillar A — "The File System"): The semantic web of nodes and edges.
   - Concerns: Topology policy (directed? cycles allowed? valid edge types?), spatial layout (force-directed, tree, radial), interaction policy (selection, creation, linking), rendering (node shapes, edge routing, badges)
   - Registry: `CanvasRegistry` — three distinct sections: **topology policy**, **layout algorithms**, **interaction/rendering policy**
   - Physics: Engine *execution* happens here (driven by layout algorithms); parameter *presets* live in the Presentation Domain
   - Extensibility: Mods can register new layout algorithms, topology rule sets, interaction policies

2. **Workbench Domain** (Pillar B — "The Window Manager"): The tile tree of panes and tabs.
   - Concerns: Tile-tree structure (splits, tabs, grids), drag/drop rules, resizing constraints, layout simplification
   - Registry: `WorkbenchSurfaceRegistry` — two sections: **layout policy**, **interaction policy**
   - Extensibility: Mods can register new split policies, tab bar styles, container layouts

**Why This Matters**: These aren't "two layout algorithms"—they're fundamentally different data structures with parallel contract surfaces. Both implement the same three-section contract (topology/structure, layout/arrangement, interaction/rendering), but they govern entirely different domains. Grouping them as "Layout" obscures this reality.

### CanvasRegistry: Three Distinct Sections

`CanvasRegistry` covers three concerns that must not be conflated:

- **Topology Policy** ("Physics of Logic"): Graph-theoretic invariants enforced at the data level. Is the graph directed? Are cycles allowed? What edge types can connect which node types? These are data constraints, distinct from UX affordances. Named policy sets: `topology:dag`, `topology:free`, `topology:tree`.
- **Layout Algorithms**: Spatial arrangement of nodes (Force-Directed, Tree, Radial, Grid). These are positioning strategies, not topology constraints.
- **Interaction & Rendering Policy**: UX affordances (selection modes, zoom/pan, node creation positions, edge routing, badge display, physics engine execution). These are presentation concerns, not data constraints.

A future `GraphPolicyRegistry` is the natural extraction slice if the surface registry grows too large. For now, three explicit sections within `CanvasRegistry` are sufficient.

### The "File Explorer" Lens: Proof of the Model

A File Explorer is not a separate mode — it is a specific Lens configuration of `CanvasRegistry`:

- **Topology Policy**: Strict DAG (hierarchy enforced), unique names per parent
- **Layout Algorithm**: Indented Tree List
- **Physics**: None (static, auto-pause immediately)
- **Knowledge Filter**: Files and Folders ontology

This generalizes: "Mind Map" = cycles allowed + force-directed + liquid physics. "Citation Graph" = directed + radial + UDC knowledge filters. The same registry surfaces, different configurations.

### Cross-Domain Orchestration

**`LensCompositor`** composes a reusable, named **Lens** = Canvas profile + Presentation profile + Knowledge filters. A Lens is a *graph view configuration* — it does not include Workbench layout. Lenses are reusable across different Workbench configurations.

**`WorkflowRegistry`** (Future) = active Lens + active InputProfile + active WorkbenchSurface profile. This is the full session mode. The semantic hierarchy is: **Workflow = Lens × WorkbenchProfile** (where WorkbenchProfile = Workbench + Input configuration).

---

## Canonical Terminology

- **Action**: canonical term for executable user/system behaviors (`ActionId`, `ActionRegistry`).
- **Mod**: canonical term for extension units. Two tiers: **Native Mod** (compiled in via `inventory::submit!`, not sandboxed) and **WASM Mod** (dynamically loaded via `extism`, sandboxed). Both use `ModManifest` with `provides`/`requires`.
- **Core Seed**: minimal registry population that makes the app functional without mods (offline graph organizer).
- **DiagnosticsRegistry**: canonical component for diagnostic channel and schema contracts.
- **Atomic Registry**: primitive capability registry. **Domain Registry**: composite context registry.
- **Lens / Input / Workflow**: domain names; concrete implementations are `*Registry` types.
- **Layout Domain**: controls structure + interaction + rendering policy via `CanvasRegistry`, `WorkbenchSurfaceRegistry`, and `ViewerSurfaceRegistry`.
- **Presentation Domain**: controls appearance + motion semantics via `ThemeRegistry` + `PhysicsProfileRegistry`.
- **KnowledgeRegistry**: the atomic UDC/taxonomy registry (not a domain coordinator).
- **LensCompositor** (not LensRegistry): composes Graph-domain Canvas + Presentation + Knowledge + Filters into a named, reusable Lens.
- **WorkflowRegistry** (Future): activates a Lens + InputProfile + WorkbenchSurface = a full session mode. `Workflow = Lens × WorkbenchProfile`.

---

## The Registry Ecosystem

We organize the registries into **Atomic Primitives** (The Vocabulary) and **Domain Configurations** (The Sentences).

### 1. Atomic Registries (Primitives)

These manage specific, isolated resources or algorithms. Mods can extend these directly.

**Persistence & I/O**
*   **Protocol Registry**:
    *   **Role**: Maps URL schemes (`ipfs://`, `file://`) to handlers. **(The Persistence Layer)**. Core seeds: `file://`, `about:`. Verso mod adds `http://`, `https://`, `data:`. Verse Tier 2 (future) adds `ipfs://`, `activitypub://` for community swarms.
    *   **Interface**: `resolve(uri) -> Result<ContentStream>`
*   **Index Registry**:
    *   **Role**: Search backends (Local, Federated) and **History/Timeline** retrieval.
    *   **Interface**: `query(text) -> Iterator<Result>`
*   **Viewer Registry**:
    *   **Role**: Maps MIME types/extensions to renderers (PDF, Markdown, CSV). Core seeds: `viewer:plaintext`, `viewer:metadata`. Verso mod adds `viewer:webview`.
    *   **Interface**: `render(ui, content)`

**Layout & Presentation Primitives**
*   **Layout Registry** (Atomic):
    *   **Role**: Positioning algorithms (`LayoutId` → `Algorithm`). Used by `CanvasRegistry` to resolve the active algorithm.
    *   **Interface**: `compute_layout(graph) -> Positions`
*   **Theme Registry** (Atomic):
    *   **Role**: Manages UI themes, node color palettes, and syntax highlighting styles.
    *   **Interface**: `get_theme(id)`
*   **Physics Profile Registry** (Atomic):
    *   **Role**: Manages named force simulation parameter presets (`PhysicsId` → `PhysicsProfile`). Semantic labels (Liquid/Gas/Solid) over numeric force params. Engine *execution* is in `CanvasRegistry` (Layout Domain); these are *parameters only*.
    *   **Interface**: `get_profile(id)`

**Logic & Security**
*   **Action Registry** (Atomic):
    *   **Role**: Definitions of executable actions (`ActionId` → `Handler`).
    *   **Interface**: `execute(context) -> Vec<GraphIntent>`
*   **Identity Registry**:
    *   **Role**: Manages keys/DIDs for signing and auth. **(The Security Root)**.
    *   **Interface**: `sign(payload, persona)`
*   **Mod Registry**:
    *   **Role**: Manages lifecycle, dependency resolution, and capabilities of all mods. Two tiers: **Native Mods** (compiled in, registered at startup via `inventory::submit!`, not sandboxed) and **WASM Mods** (dynamically loaded via `extism`, sandboxed, capability-restricted).
    *   **Interface**: `load_mod(path)`, `unload_mod(id)`, `resolve_dependencies()`, `list_mods()`
*   **Agent Registry** (Atomic):
    *   **Role**: Definitions of autonomous background tasks (`AgentId` → `Task`).
    *   **Interface**: `spawn(context)`, `schedule(cron)`
*   **Diagnostics Registry** (Atomic):
    *   **Role**: Central definition of diagnostic channels, schemas, and configuration (sampling/retention).
    *   **Interface**: `register_channel(def)`, `get_config(channel_id)`, `set_config(channel_id, config)`

**Knowledge**
*   **Knowledge Registry** (formerly Ontology Registry):
    *   **Role**: UDC tagging, semantic definitions, validation, and runtime indexing (`NodeKey` → `CompactCode`).
    *   **Interface**: `validate(tag)`, `distance(a, b)`, `get_label(code)`, `get_color_hint(code)`
    *   **Pattern**: Router for schema providers (UDC core seed; mods can add Schema.org, Wikidata, etc.).

### 2. Domain Registries (Composites)

These combine primitives to define a user experience context. **Domain sequencing principle**: resolve Layout (structure + interaction) first, then Presentation (style + motion) second.

*   **Layout Domain**:
    *   **Role**: Controls how information is *arranged* and *interacted with* before styling. Coordinates both sovereign data territories.
    *   **Coordinator**: `LayoutDomainRegistry`
    *   **Surface Registries**:
        *   `CanvasRegistry` — three sections: (1) topology policy (DAG/free/tree rule sets, edge type constraints), (2) layout algorithms (force-directed, tree, radial — delegates to atomic `LayoutRegistry`), (3) interaction/rendering policy (selection, zoom/pan, node creation, edge routing, badge display, physics engine execution).
        *   `WorkbenchSurfaceRegistry` — two sections: (1) layout policy (split types, tab rules, `SimplificationOptions`), (2) interaction policy (drag/drop rules, resize constraints, drop zone behavior, container labels).
        *   `ViewerSurfaceRegistry` — document viewport behavior: zoom/scaling, reader mode, scroll policy. (Viewer *selection* — MIME routing — stays in atomic `ViewerRegistry`; this governs how the selected viewer presents its viewport.)

*   **Presentation Domain**:
    *   **Role**: Controls *appearance* and *motion semantics* after layout.
    *   **Coordinator**: `PresentationDomainRegistry`
    *   **Subregistries**:
        *   `ThemeRegistry` — visual token/style resolution (colors, strokes, fonts).
        *   `PhysicsProfileRegistry` — named parameter presets (Liquid/Gas/Solid). Engine execution is in `CanvasRegistry`; this domain provides *which parameter set to use*.

*   **Input Domain**:
    *   **Role**: Defines how the user *controls* the app.
    *   **Composition**: `InputEvent` → `ActionId` mapping (Keybinds/Mousebinds).
    *   **Registry**: `InputRegistry`

*   **Cross-Domain Compositor**:
    *   **Role**: Composes a reusable, named **Lens** = GraphSurface profile + Presentation profile + Knowledge filters. Enforces domain sequencing (Layout first, then Presentation) during resolution. A Lens is a *graph view configuration*; it does not include Workbench layout.
    *   **Registry**: `LensCompositor`

*   **Workflow Domain** (Future):
    *   **Role**: Defines the high-level *session mode*: active Lens + active InputProfile + active WorkbenchSurface profile.
    *   **Semantic**: `Workflow = Lens × WorkbenchProfile` where WorkbenchProfile = Workbench + Input configuration.
    *   **Registry**: `WorkflowRegistry`

**Note**: Verse is **not** a domain registry. P2P networking, federated identity, and distributed indexing are packaged as the **Verse native mod** which registers into atomic registries (Protocol, Index, Identity, Action) on load. Phase 5 implements Tier 1 (bilateral sync via iroh); Tier 2 (community swarms, libp2p, public protocols) is future research — see Phase 5 header for details.

---

## Core Seed Floor

The application must be fully functional as an offline graph organizer **without any mods loaded**:

| Registry | Core Seed (no mods) | Verso Mod Adds | Verse Mod Adds |
|---|---|---|---|
| ProtocolRegistry | `file://`, `about:` | `http://`, `https://`, `data:` | (Tier 2 future: `ipfs://`, `activitypub://`) |
| ViewerRegistry | `viewer:plaintext`, `viewer:metadata` | `viewer:webview` (Servo) | — |
| ActionRegistry | `graph.*`, `view.*`, `workspace.*` | `navigation.*`, `webview.*` | `verse.share`, `verse.sync` |
| InputRegistry | Graph/workspace keybindings | Browser-style keybindings | — |
| ThemeRegistry | `theme:default`, `theme:dark` | — | — |
| PhysicsProfileRegistry | `physics:liquid`, `physics:gas` | — | — |
| LayoutRegistry | `layout:default`, `layout:grid` | — | — |
| IdentityRegistry | `identity:local` (generated keypair) | — | P2P personas, DID providers |
| IndexRegistry | Local tantivy search | — | Federated search providers |
| KnowledgeRegistry | UDC defaults | — | Schema.org providers |

**Without Verso**: No webviews, no HTTP. Nodes display as metadata cards. Graph is a visual outliner/Zettelkasten.
**Without Verse**: No P2P, no federated search. Fully offline. Local identity only.

---

## Registry Testing Rule (Required)

Each registry ships with two test layers:

1.  **Registry Contract Tests** (unit-level):
    - Validate registration, lookup, conflict behavior, and fallback semantics.
    - Validate diagnostics schema stability for each exposed channel.
2.  **Harness Scenario Tests** (integration-level):
    - Validate end-to-end behavior through `desktop/tests/scenarios/*` using diagnostics snapshots.
    - Required for any new registry capability before it is considered complete.

## Diagnostics Contract Checklist (Required for Every Registry)

Before a registry is marked complete, all items below must be defined and validated:

- [ ] **Channel Naming**: Channels follow `registry.<name>.<event>` naming convention.
- [ ] **Schema Version**: Every emitted diagnostic payload includes `schema_version`.
- [ ] **Field Stability**: Required fields are documented; optional fields are explicitly marked nullable/optional.
- [ ] **Compatibility Policy**: Additive changes are allowed; removals/renames require deprecation for at least one release cycle.
- [ ] **Snapshot Coverage**: At least one harness scenario asserts channel counts or payload shape.
- [ ] **Failure Path Signals**: Error and fallback paths emit distinct diagnostics.
- [ ] **Test Fixtures**: Registry contract tests include at least one schema-golden fixture.

## Registry Contract Matrix (Execution Checklist)

| Registry | Owner | Primary Interface | Required Diagnostics Channels | Minimum Tests |
|---|---|---|---|---|
| ProtocolRegistry | Platform/Networking | `resolve(uri)` | `registry.protocol.resolve_started`, `.resolve_succeeded`, `.resolve_failed`, `.fallback_used` | 2 contract tests (lookup/fallback, error path), 1 harness scenario |
| ViewerRegistry | Desktop/Rendering | `render(ui, content)` | `registry.viewer.render_started`, `.render_succeeded`, `.render_failed`, `.fallback_used` | 2 contract tests (mime resolution, fallback), 1 harness scenario |
| ActionRegistry | App/Core | `execute(action, context)` | `registry.action.execute_started`, `.execute_succeeded`, `.execute_failed` | 2 contract tests (registration/conflict, intent emission), 1 harness scenario |
| InputRegistry | App/Input | `map_input(event)` | `registry.input.binding_resolved`, `.binding_missing`, `.binding_conflict` | 2 contract tests (mapping/override, missing binding), 1 harness scenario |
| LayoutRegistry | App/Layout | `compute_layout(graph)` | `registry.layout.compute_started`, `.compute_succeeded`, `.compute_failed`, `.fallback_used` | 2 contract tests (algorithm lookup, fallback), 1 harness scenario |
| ThemeRegistry | Desktop/UI | `get_theme(id)` | `registry.theme.lookup_succeeded`, `.lookup_failed`, `.fallback_used` | 2 contract tests (lookup/fallback, missing theme), 1 harness scenario |
| PhysicsProfileRegistry | App/Presentation | `get_profile(id)` | `registry.physics.lookup_succeeded`, `.lookup_failed`, `.fallback_used` | 2 contract tests (lookup/fallback, profile validation), 1 harness scenario |
| IdentityRegistry | Core/Security | `sign(payload, persona)` | `registry.identity.sign_started`, `.sign_succeeded`, `.sign_failed`, `.key_unavailable` | 2 contract tests (persona resolution, signing failure), 1 harness scenario |
| IndexRegistry | Search/Recall | `query(text)` | `registry.index.query_started`, `.query_succeeded`, `.query_failed`, `.fallback_used` | 2 contract tests (provider selection, failure fallback), 1 harness scenario |
| KnowledgeRegistry | Knowledge/Semantics | `validate(tag)`, `get_label(code)` | `registry.knowledge.lookup_succeeded`, `.lookup_failed`, `.fallback_used` | 2 contract tests (schema lookup/versioning, fallback), 1 harness scenario |
| DiagnosticsRegistry | System/Observability | `register_channel(def)` | `registry.diagnostics.channel_registered`, `.config_changed` | 2 contract tests (registration, config override), 1 harness scenario |
| ModRegistry | Platform/Extensibility | `load_mod(path)`, `unload_mod(id)`, `resolve_dependencies()` | `registry.mod.load_started`, `.load_succeeded`, `.load_failed`, `.dependency_missing`, `.security_violation`, `.quarantine` | 3 contract tests (load/unload, denied capability, dependency resolution), 1 harness scenario |
| LensCompositor | App/Cross-Domain | `resolve_lens(id)` | `registry.lens.resolve_succeeded`, `.resolve_failed`, `.fallback_used` | 2 contract tests (composition resolution, fallback), 1 harness scenario |
| WorkflowRegistry (future) | App/WorkbenchProfile | `activate_workflow(id)` | `registry.workflow.activate_started`, `.activate_succeeded`, `.activate_failed` | 2 contract tests (activation/switching, fallback), 1 harness scenario |

**Note**: VerseRegistry has been removed from this matrix. Verse is a native mod that registers entries into atomic registries. Its diagnostics channels are scoped under the atomic registries it populates.

**Matrix policy**:
- Each row must map to at least one concrete test file path before phase closure.
- Channels listed here are the canonical minimum set; rows may add channels but should not remove these without deprecation.

---

## Registry Interface Standard

All registries must adhere to a common lifecycle and integration pattern:

1.  **Context Injection**: Operations (execute, resolve, render) receive a `RegistryContext` providing controlled access to `GraphBrowserApp` state, `DiagnosticsState`, and other registries.
2.  **Async/Sync Policy**:
    - `ProtocolRegistry`: `resolve` must be non-blocking (return `Future` or `Reader` that doesn't block main thread).
    - `ActionRegistry`: `execute` returns `Vec<GraphIntent>` (synchronous intent generation). Input handling moves to `InputRegistry`.
    - `ViewerRegistry`: `render` is immediate-mode (egui).
3.  **Persistence**: User configurations (custom actions, lenses) are serialized to `user_registries.json`.
4.  **Diagnostics**: Every registry must register its diagnostic channels on startup.
5.  **Fallback Policy**: Every Atomic Registry must implement `get_or_default(id)` to handle missing/unloaded items gracefully.
6.  **Execution Ownership**: Long-running or async work is owned by `AppServices` task runners; registries return intents/events and cancellation handles, not unmanaged background tasks.

## Refactoring Strategy: Data vs. Systems

To avoid borrow-checker conflicts and monolithic state, `GraphBrowserApp` must be split:

1.  **`GraphWorkspace` (Data)**: Pure data (Graph, Selection, Camera, Physics State). Serializable.
2.  **`AppServices` (Systems)**: Runtime systems (Registries, Windowing, Network). Ephemeral.
3.  **Registry Context**:
    - Registry methods take `&GraphWorkspace` or `&mut GraphWorkspace` as arguments, not `&mut GraphBrowserApp`.
    - Example: `ActionRegistry::execute(action, &mut workspace, &services)`.

This keeps the runtime modular while ensuring tests live in the existing harness topology (rather than hidden in ad-hoc module tests).

---

## Target Repository Topology

The current structure is flat and root-heavy. Registry migration is coupled with a filesystem restructure so architecture boundaries are visible in the filesystem rather than implicit in module names:

### 1) Core (Data & Logic): `src/model/`
- `graph/` — the persistent graph data structure
- `intent.rs` — GraphIntent + reducer ownership
- `session.rs` — application state after shell concerns are separated
- `selection.rs` — selection model

### 2) Capabilities (Registries): `src/registries/`
- `mod.rs` — RegistryRuntime (central container + Signal Bus)
- `infrastructure/` — `diagnostics`, `mod_loader`
- `atomic/` — `protocol`, `index`, `action`, `identity`, `agent`, `knowledge`
- `domain/layout/` — `layout`, `canvas`, `workbench_surface`, `viewer_surface`
- `domain/presentation/` — `presentation`, `theme`, `physics_profile`
- `domain/` — `lens`, `input`

### 3) Mods: `src/mods/`
- `mod.rs` — ModManifest, dependency resolution, loading
- `native/` — compile-time registered mods (`verso`, `verse`, `default_themes`, etc.)
- `wasm/` — dynamically loaded sandboxed mods

### 4) Services (Infrastructure): `src/services/`
- `persistence/` — workspace save/load
- `search/` — indexing and query
- `physics/` — engine integration and simulation utilities

### 5) Shell (Presentation): `src/shell/desktop/`
- `host/` — window, event loop, embedder glue
- `workbench/tiles/` — tile rendering and behavior
- `workbench/frame.rs` — frame composition
- `lifecycle/` — webview_controller, reconciliation, backpressure
- `ui/toolbar/`, `ui/panels/`, `ui/gui.rs` — UI components

---

## Migration Rules

1. Move one semantic area at a time; preserve behavior before cleanup.
2. Prefer `pub(crate)` re-exports during transition to avoid broad callsite churn.
3. Do not combine large path moves with logic rewrites in the same change-set.
4. Every move slice must pass: `cargo test`, `cargo check`, and diagnostics harness scenarios.
5. Delete compatibility re-exports quickly (no long-lived dual paths).

---

## Phase Plan

### Phase 0: Walking Skeleton [Complete]

**Goal**: Deliver one thin vertical slice end-to-end before broad registry rollout.

1. Created `desktop/registries/` root with `protocol`, `viewer`, and `diagnostics_contract` modules.
2. Routed a single path (`https://` + `text/html`) through `ProtocolRegistry` → `ViewerRegistry`.
3. Registered and emitted diagnostics channels for success + fallback + failure (`registry.protocol.*`, `registry.viewer.*`).
4. Added contract test module covering register/resolve/fallback behavior.
5. Added harness scenario validating emitted diagnostics for the same flow.
6. Removed replaced legacy branch for that flow.

**Status**: Complete.

---

### Phase 1: Core Decoupling [Complete]

**Goal**: Registry-owned paths for protocols, viewers, actions, input, and lenses; capability topology migration to canonical paths.

This phase encompasses what was originally planned as separate phases but was executed together. Completed work:

- Protocol/viewer routing fully registry-owned with cancellation-aware resolution (`ProtocolResolveControl`, `resolve_with_control`). MIME hint inference from URI/data-URI metadata. Legacy URL fallback policy removed.
- Action dispatch registry-owned for all migrated actions (`action.omnibox_node_search`, `action.graph_view_submit`, `action.detail_view_submit`).
- Input mappings data-driven through `InputRegistry` (toolbar submit, nav back/forward/reload bindings).
- Lens configuration uses IDs (layout/theme/physics) composed via `LensCompositor` with fallback behavior and per-component diagnostics.
- Persisted user defaults for lens/physics/layout/theme IDs via `workspace:settings-registry-*-id` keys; settings UI controls wired.
- Identity scaffolding via `IdentityRegistry`: persona resolution, sign/fallback, diagnostics contracts.
- `DiagnosticsRegistry` folded as sole contract source; `diagnostics_contract.rs` removed.

**Phase 1.4 — Capability Topology Slice**: Moved `desktop/registries/*` → `src/registries/{atomic,domain}`. Compatibility re-exports removed. All registry tests/scenarios pass with canonical paths.

**Phase 1 Done Gate** (met):
- Protocol + viewer routing is registry-owned; no hidden bypass.
- Protocol resolution is non-blocking and cancellation-aware.
- Action dispatch is registry-owned for migrated actions.
- Input mappings are data-driven through `InputRegistry`.
- Lens configuration uses IDs with fallback behavior via `LensCompositor`.
- Config save/load roundtrip for user registries passes.
- Diagnostics contracts for all migrated channels pass checklist and tests.
- Legacy dispatch branches deleted; no parallel paths.

**Status**: Complete (2026-02-23).

---

### Phase 2: Mod Infrastructure & Protocol/Viewer Contracts

**Goal**: Stand up the mod system. Define protocol and viewer contracts as registry surfaces. Package Servo integration as the Verso native mod. Establish the core seed floor (app works without mods).

#### Step 2.1: Mod Manifest & Loader
- Define `ModManifest` struct: `mod_id`, `display_name`, `mod_type` (Native | WASM), `provides`, `requires`, `capabilities`.
- Implement mod dependency resolver (topological sort on `requires` → `provides` edges).
- Implement native mod loader via `inventory::submit!` for compile-time registration.
- Register mod lifecycle diagnostics (`registry.mod.load_started`, `.load_succeeded`, `.load_failed`, `.dependency_missing`).

#### Step 2.2: Protocol & Viewer Contracts (Registry Surfaces)
- Define `ProtocolHandler` trait as the contract surface. Implement as `tower::Service<Uri, Response = ContentStream>` for free middleware composition (timeouts, retries, tracing).
- Define `ViewerHandler` trait as the contract surface (MIME/content → renderer).
- Seed core defaults: `protocol:file`, `protocol:about`, `viewer:plaintext`, `viewer:metadata`.
- Ensure the app is fully functional with only core seeds (graph + metadata display, no web rendering).

#### Step 2.3: Verso Native Mod
- Package current Servo/Wry integration as a native mod with manifest:
  - `provides`: `protocol:http`, `protocol:https`, `protocol:data`, `viewer:webview`
  - `requires`: `ProtocolRegistry`, `ViewerRegistry`
  - `capabilities`: `network`
- Gate webview creation on `viewer:webview` being registered; absent → nodes display as metadata-only.
- Ensure startup without Verso mod succeeds (offline graph organizer mode).

**Phase 2 Done Gate**:
- `ModManifest` defined. Native mod loader works via `inventory::submit!`.
- Mod dependency resolver (topological sort) rejects mods with unmet `requires`.
- Core seed floor verified: app starts and functions without Verso mod.
- Verso native mod loads and registers `protocol:http`, `protocol:https`, `viewer:webview`.
- Mod lifecycle diagnostics pass checklist and tests.

**Status**: Complete (2026-02-23).

---

### Phase 3: Layout Domain (Surface Registries)

**Goal**: Registry-owned structure, interaction, and rendering policy for all three surfaces. Implement the Two-Pillar architecture in code.

#### Step 3.1: Layout Domain Coordinator
- Introduce `LayoutDomainRegistry` as the coordinator for surface subregistries.
- Lens resolution obtains a composed layout profile from the domain, not a single mode.

#### Step 3.2: Graph Surface Subregistry
- Define `CanvasRegistry` with three explicit sections:
  - **Topology Policy**: Named rule sets (`topology:dag`, `topology:free`, `topology:tree`). Directed/undirected flag, cycle allowance, edge type constraint rules. Extract from hardcoded interaction logic in `SettingsNavigation`.
  - **Layout Algorithms**: Wrap `egui_graphs::LayoutForceDirected` in a `LayoutAlgorithm` trait impl. Register IDs (`graph_layout:force_directed`, `graph_layout:grid`, `graph_layout:tree`).
  - **Interaction & Rendering Policy**: Selection modes, zoom/pan ranges, node creation positions, node shapes/sizes, edge routing/style, label format, badge display rules. Extract from `SettingsInteraction` and `SettingsStyle`. Physics engine integration: available force profiles, energy thresholds, auto-pause triggers.
- Refactor `render/mod.rs` to instantiate graph layout/interaction/style via surface-registry dispatch.

#### Step 3.3: Workbench Surface Subregistry
- Define `WorkbenchSurfaceRegistry` with two sections:
  - **Layout Policy**: `SimplificationOptions`, split direction defaults, tab wrapping rules.
  - **Interaction Policy**: Drag-to-rearrange rules, resize constraints, drop zone behavior, tab bar style, container labels (semantic: `Split ↔`, `Tab Group`, etc.), title truncation.
- Refactor `tile_behavior.rs` to resolve policy profiles via workbench surface registry.

#### Step 3.4: Viewer Surface Subregistry
- Define `ViewerSurfaceRegistry` covering viewer viewport behavior: zoom/scaling defaults, reader mode, scroll policy.
- Update viewer entrypoints to resolve surface policies via layout domain.

**Phase 3 Done Gate**:
- Layout domain coordinator resolves surface profiles for graph, workbench, and viewer.
- `CanvasRegistry` has explicit topology policy, layout algorithm, and interaction/rendering sections.
- `SettingsNavigation`/`SettingsInteraction`/`SettingsStyle` are registry-owned (not hardcoded in `render/mod.rs`).
- Surface registries emit stable diagnostics.

**Status**: Complete (2026-02-23).

---

### Phase 4: Presentation Domain & Knowledge Registry

**Goal**: Resolve appearance and motion semantics after layout. Formalize knowledge classification as a distinct atomic registry.

#### Step 4.1: Theme Subregistry
- Define `ThemeData` struct (colors, strokes, font sizes).
- Create `DefaultTheme` matching current hardcoded values.
- Register `theme:default` in `ThemeRegistry` as core seed.

#### Step 4.2: Physics Profile Subregistry
- Extract `PhysicsProfile` presets (Liquid, Gas, Solid) from `app.rs` as named parameter sets.
- Register them in `PhysicsProfileRegistry` as presentation-domain semantic labels.
- Remove `layout_mode` from `PhysicsProfile`. Layout mode is independently resolved by the Layout Domain. A Lens composes both, but physics must not override layout.

#### Step 4.3: Presentation Domain Coordinator
- Make `PresentationDomainRegistry` the coordinator for `ThemeRegistry` + `PhysicsProfileRegistry`.
- Update `render/mod.rs` so theme/physics resolution occurs after layout profile selection.

#### Step 4.4: Knowledge Registry (Atomic)
- Formalize the existing `OntologyRegistry` (UDC tagging, `CompactCode`, fuzzy search, `get_color_hint`) as `KnowledgeRegistry` — an atomic capability, not a domain coordinator. Use `sophia` for RDF/JSON-LD parsing.
- Register as core seed with UDC defaults. Mods can add Schema.org, Wikidata, or custom taxonomy providers.
- Lens filters reference knowledge tags; knowledge resolution is independent of both Layout and Presentation domains.

**Phase 4 Done Gate**:
- Presentation domain coordinator resolves theme + physics profile after layout.
- `layout_mode` removed from `PhysicsProfile`.
- `KnowledgeRegistry` formalized with UDC core seed.
- Agent paths emit stable diagnostics and respect runtime quotas.

**Status**: Complete through Step 4.4 (2026-02-23).

---

### Phase 5: Verse Native Mod (Tier 1: Direct P2P Sync)

**Goal**: Package direct P2P networking as the Verse native mod. Implements bilateral, zero-cost sync between trusted devices via iroh (QUIC + Noise). No tokens, no servers, no Tier 2 complexity.

**Technical Reference**: See [`verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md`](../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) for complete specifications covering:
- Identity model (Ed25519, OS keychain, trust store)
- Transport (iroh, NAT traversal, connection model)
- Sync protocol (SyncUnit wire format, version vectors, conflict resolution)
- SyncWorker control plane integration
- UX designs (Sync Panel, pairing flows, conflict UI)
- Security (Noise auth, AES-256-GCM at-rest encryption)

**Tier 1 vs Tier 2**: This phase implements **Tier 1 only** (implementation-ready, bilateral sync). Tier 2 (libp2p community swarms, VerseBlob content addressing, Proof of Access economics, federated search) is documented separately in [`verse_docs/2026-02-23_verse_tier2_architecture.md`](../../verse_docs/2026-02-23_verse_tier2_architecture.md) as long-horizon research — not a Phase 5 dependency. Tier 2 validation begins Q3 2026 after Tier 1 is proven in production.

#### Step 5.1: iroh Scaffold & Identity Bootstrap

**Spec Reference**: Tier 1 plan §9.1 (iroh Scaffold & Identity Bootstrap), §2 (Identity & Pairing), §3 (Transport: iroh)

- Add `iroh`, `keyring`, `qrcode` dependencies.
- Define `VerseMod` with `ModManifest` via `inventory::submit!`:
  - `provides`: `identity:p2p`, `protocol:verse`, `action:verse.pair_device`, `action:verse.sync_now`, `action:verse.share_workspace`, `action:verse.forget_device`
  - `requires`: `IdentityRegistry`, `ActionRegistry`, `ProtocolRegistry`, `ControlPanel`, `DiagnosticsRegistry`
  - `capabilities`: `network`, `identity`
- Generate Ed25519 keypair on first launch, persist in OS keychain via `keyring`.
- Create iroh `Endpoint` with `SYNC_ALPN = b"graphshell-sync/1"`.
- Register `identity:p2p` persona in `IdentityRegistry` (NodeId accessible to rest of app).
- **Done gate**: `cargo run` starts iroh endpoint. `DiagnosticsRegistry` shows `registry.mod.load_succeeded` for "verse". `IdentityRegistry::p2p_node_id()` returns the device NodeId. App starts normally without Verse mod.

#### Step 5.2: TrustedPeer Store & IdentityRegistry Extension

**Spec Reference**: Tier 1 plan §9.2 (TrustedPeer Store & IdentityRegistry Extension), §2.2–2.3 (IdentityRegistry Extension, Trust Store), §7.2 (At-Rest Sync Cache)

- Extend `IdentityRegistry` with `P2PIdentityExt` trait: `p2p_node_id`, `sign_sync_payload`, `verify_peer_signature`, `get_trusted_peers`, `trust_peer`, `revoke_peer`.
- Implement `TrustedPeer` model (`PeerRole::Self_` | `PeerRole::Friend`, `WorkspaceGrant`).
- Persist trust store in `user_registries.json` under `verse.trusted_peers`.
- Implement `SyncLog` (per-workspace intent log + `VersionVector`) with rkyv + AES-256-GCM at rest.
- Diagnostics: `registry.identity.p2p_key_loaded`, `verse.sync.pairing_succeeded`, `verse.sync.pairing_failed`.
- **Done gate**: Contract tests cover P2P persona create/load, sign/verify round-trip, trust store persist/load round-trip, grant model serialization.

#### Step 5.3: Pairing Ceremony & Settings UI

**Spec Reference**: Tier 1 plan §9.3 (Pairing Ceremony & Settings UI), §2.4 (Pairing Flows), §6.2–6.3 (Sync Panel, Pairing Flow UX)

- Implement `verse.pair_device` action (initiator path): encode `NodeAddr` as 6-word phrase + QR data; show in dialog with 5-minute expiry.
- Implement `verse.pair_device` receiver path: decode code → connect via iroh → show fingerprint confirm → name device → workspace grant dialog.
- Implement mDNS advertisement (`_graphshell-sync._udp.local`) and discovery for local network pairing.
- Add Sync settings page (`graphshell://settings/sync`): device list, "Add Device" button, sync status per device.
- After confirmation: add peer to trust store, persist.
- **Done gate**: Two desktop instances on the same machine pair via printed 6-word code. Both show each other in Sync Panel device list. mDNS discovery shows peer on same LAN.

#### Step 5.4: Delta Sync (Core)

**Spec Reference**: Tier 1 plan §9.4 (Delta Sync), §4 (Sync Protocol), §5 (SyncWorker), §6.5 (Conflict Resolution UI)

- Implement `SyncWorker` as ControlPanel-supervised tokio task (accept loop + `mpsc::Receiver<SyncCommand>`).
- Implement `VersionVector` (per-peer sequence clocks, merge, dominates, increment).
- Implement `SyncUnit` wire format (rkyv → zstd → iroh QUIC stream).
- Implement bidirectional delta exchange: `SyncHello` VV exchange → compute diff → send `SyncUnit`s → `SyncAck`.
- Implement full snapshot trigger (VV is zero or delta > 10,000 intents).
- Remote intents apply via `GraphIntent::ApplyRemoteDelta` (bypass undo stack, update local VV).
- Implement conflict resolution per-intent-type (see Verse strategy §4.4): LWW for title/name, CRDT for tags, ghost-node for delete conflicts.
- Add sync status indicator to toolbar (`●`/`○`/`!`).
- Add non-blocking conflict notification bar.
- Diagnostics: `verse.sync.unit_sent`, `verse.sync.unit_received`, `verse.sync.intent_applied`, `verse.sync.conflict_detected`, `verse.sync.conflict_resolved`.
- **Done gate**: Create a node on instance A → appears on instance B within 5 seconds. Concurrent title rename → LWW resolves without crash. Harness scenario `verse_delta_sync_basic` passes.

#### Step 5.5: Workspace Access Control

**Spec Reference**: Tier 1 plan §9.5 (Workspace Access Control), §2.3 (Trust Store with WorkspaceGrant), §6.4 (Workspace Sharing Context Menu), §7.3 (Trust Boundary)

- Enforce `WorkspaceGrant` on inbound sync: reject `SyncUnit` for non-granted workspaces → `verse.sync.access_denied` diagnostic.
- Enforce read-only grants: incoming mutating intents from `ReadOnly` peers are rejected.
- Add "Manage Access" UI in Sync Panel (grant/revoke per device per workspace).
- Add workspace sharing context menu (right-click workspace → "Share with...").
- Implement `verse.forget_device` action: revoke all grants + remove from trust store.
- **Done gate**: Peer A grants Peer B `ReadOnly` on workspace W. Peer B receives mutations from W but its own mutations on W do not propagate to A. Harness scenario `verse_access_control` passes.

**Phase 5 Done Gate** (all steps):
- Verse native mod loads and registers all declared entries into atomic registries.
- App starts and functions fully without Verse mod loaded.
- Two instances pair, sync bidirectionally, and enforce per-workspace access control.
- Offline degradation: mod loaded but no peers → diagnostics emitted, app works normally, intents journal for later sync.
- All diagnostics channels registered and passing contract checklist.

---

### Phase 6: Topology Consolidation (Model / Services / Shell)

**Goal**: Align filesystem structure with architecture boundaries after capability paths are stable.

#### Step 6.1: Model Extraction
- Move data-centric types from root-heavy modules into `src/model/*`.
- Split shell/runtime fields from `GraphBrowserApp` into shell-facing adapters where practical.

#### Step 6.2: Service Extraction
- Normalize `persistence`, `search`, and physics-support logic under `src/services/*`.

#### Step 6.3: Desktop Shell Decomposition
- Re-home `desktop/*` into `src/shell/desktop/{host,workbench,lifecycle,ui}`.
- Group tile files under `workbench/tiles/`; keep diagnostics harness paths updated.

#### Step 6.4: Remove Transition Shims
- Delete temporary re-exports and old module aliases.
- Ensure docs and plan references use canonical paths only.

**Phase 6 Done Gate**:
- Filesystem structure matches `src/{model,registries,services,mods,shell}` layout.
- All transition shims and re-exports removed.
- Tests and diagnostics harness updated to canonical paths.

---

## Technical Stack & Patterns

To avoid reinventing wheels, we adopt these established ecosystem patterns:

1.  **Protocol Registry as `tower::Service`**:
    -   **Crate**: `tower`
    -   **Pattern**: Middleware.
    -   **Refinement**: Define `ProtocolHandler` as `tower::Service<Uri, Response = ContentStream>`. Enables free use of standard middleware for timeouts, retries, concurrency limits, and tracing on any protocol (IPFS, Gemini, etc.).

2.  **Mod Registry (Dual-Tier)**:
    -   **Native Mods**: `inventory` crate for compile-time registration via `submit!`. First-party capabilities (Verso, Verse, default themes) register at startup. Not sandboxed.
    -   **WASM Mods**: `extism` crate (wraps Wasmtime) for dynamic plugin loading. Sandboxed, capability-restricted. Used for third-party extensions.
    -   **Both tiers** use `ModManifest` with `provides`/`requires` for dependency resolution.

3.  **Knowledge Registry via `sophia`**:
    -   **Crate**: `sophia`
    -   **Pattern**: Linked Data / RDF.
    -   **Refinement**: Use `sophia`'s traits for efficient, zero-copy parsing of JSON-LD (Schema.org) data. Core seed provides UDC taxonomy; mods can add Schema.org or Wikidata providers.

4.  **Action Extraction (The Handler Pattern)**:
    -   **Inspiration**: `axum` / `bevy`.
    -   **Pattern**: Type-safe dependency injection.
    -   **Refinement**: Implement a `FromContext` trait. Actions declare typed arguments (`fn cmd(selection: Selection)`) and the registry extracts them from context. Decouples actions from full app state.

5.  **`schemars` (Auto-Configuration UI)**:
    -   **Crate**: `schemars`
    -   **Pattern**: Reflection / Schema Generation.
    -   **Refinement**: Registry items derive `JsonSchema`. The Settings UI uses this schema to auto-generate sliders, dropdowns, and inputs. Solves "how do I configure a mod?" without writing UI code.

6.  **`inventory` (Native Mod Registration)**:
    -   **Crate**: `inventory`
    -   **Pattern**: Distributed Slices.
    -   **Refinement**: Native mods register `ModManifest` + registry entries at compile time via `submit!`. Eliminates a central "register all" function. Same manifest contract as WASM mods, but discovered at startup without dynamic loading.

---

## Robustness & Integration

#### 1. The "Missing Mod" Strategy (Graceful Degradation)
*   **Problem**: A workspace references a Layout/Theme provided by a mod that is no longer installed.
*   **Solution**: Registries store a hardcoded `fallback_id` (e.g., `layout:default`, `theme:dark`). Lookups use `get_or_default(id)`. The UI shows a warning: "Layout 'SuperGrid' missing, using Default."

#### 2. Registry Signal Bus (Decoupling)
*   **Problem**: `IdentityRegistry` changes persona; `ProtocolRegistry` needs to update keys.
*   **Solution**: A synchronous broadcast channel in `AppServices`. Events: `IdentityChanged`, `ThemeChanged`, `ModLoaded`, `ModUnloaded`. Registries implement `on_signal(&mut self, signal: RegistrySignal)`.

#### 3. Configuration UI (Auto-Generation)
*   **Problem**: Users need to tweak settings for specific registry items (e.g., Physics parameters).
*   **Solution**: Use `schemars`. Registry items implement `Configurable` returning `schemars::schema_for!(Self)`. A generic `SchemaWidget` in `desktop/ui` renders controls based on JSON schema.

#### 4. Macros (Intents as Scripts)
*   **Idea**: Since `GraphIntent` is serializable, a "Macro" is just a persisted `Vec<GraphIntent>`.
*   **Implementation**: `ActionRegistry` supports a `MacroHandler` variant. Users can "Record" a sequence, save it as a new Action, and bind it to a key.

#### 5. Mod Security & Capability Policy
*   **WASM Mods**: Capability manifest per mod (`network`, `filesystem`, `identity`, `clipboard`, `exec`) with deny-by-default policy. Runtime quotas for CPU time, memory, message rate, outbound requests. Kill switch and quarantine mode for crashing or policy-violating mods.
*   **Native Mods**: Not sandboxed (compiled into binary). Security comes from code review at compile time. First-party (Verso, Verse) or explicitly opt-in.
*   Security diagnostics channels (`registry.mod.security_violation`, `registry.mod.quarantine`) apply to WASM mods only.

#### 6. Configuration Precedence (No Ambiguity)
*   **Precedence order**: `workspace override` > `user override` > `built-in default`.
*   Every resolved value can report provenance (`resolved_from = workspace|user|default`).
*   Conflicts emit diagnostics (`registry.config.conflict_detected`) and show deterministic UI resolution.

---

## Risks & Mitigations

*   **Risk**: Performance regression in Input/Action lookup (per-frame).
    *   *Mitigation*: `InputRegistry` uses a fast lookup (hash map of `KeyChord`). Only queried on input events, not every frame.

*   **Risk**: Circular dependencies between `RegistryRuntime` and `GraphBrowserApp`.
    *   *Mitigation*: Strict `RegistryContext` pattern. Registries never hold `&mut App`; they receive it during method calls only.

*   **Risk**: Semantic overlap between layout and presentation domains creates duplicate knobs.
    *   *Mitigation*: Enforce sequencing (layout → presentation) and keep cross-domain coupling out of registry APIs. Physics engine execution is Layout; physics parameter presets are Presentation. Lens orchestrates both without conflating them.

*   **Risk**: Large path moves hide logic regressions.
    *   *Mitigation*: Separate path-only commits from behavior commits; run scenario matrix after each slice.

*   **Risk**: Test harness path churn breaks migration velocity.
    *   *Mitigation*: Update harness imports in the same slice as each move; diagnostics contracts serve as continuity checks.

*   **Risk**: Mod loading order creates startup failures or silent capability gaps.
    *   *Mitigation*: Topological sort on mod `requires`/`provides` at startup. Missing dependency = mod load failure with diagnostics, not silent skip. Core seeds guarantee a functional floor.

*   **Risk**: Verso-as-mod creates a hard coupling to Servo that the mod contract can't cleanly express.
    *   *Mitigation*: Verso is a **native mod** — compiled in, not sandboxed. The mod contract (manifest + registry population) is the architectural boundary, not an execution sandbox. If the mod API can't express Verso's needs, fix the API, don't special-case Verso.

*   **Risk**: "Core seed floor" is too minimal to be useful, pushing all value into mods.
    *   *Mitigation*: Core seeds include graph manipulation, local file protocol, plaintext/metadata viewers, full keyboard/action pipeline, persistence, and search. This is a complete offline document organizer. Mods add web rendering and networking, not basic functionality.

*   **Risk**: `CanvasRegistry` becomes a god object (topology + layout + interaction + rendering + physics).
    *   *Mitigation*: Three explicit sections with clear boundaries prevent conflation in code and documentation. If growth exceeds manageable scope, `GraphPolicyRegistry` (topology invariants only) is the natural extraction slice.

---

## Findings

### Lenses as Named Graph View Configurations

A Lens is the composition of: **Topology Policy** + **Layout Algorithm** + **Physics Parameters** + **Theme** + **Knowledge Filter**. Two concrete examples:

**"Research Mode" Lens**:
- Topology Policy: `topology:free` (cycles allowed, undirected)
- Layout: `graph_layout:force_directed`
- Physics: `physics:liquid` (organic clustering)
- Theme: `theme:dark`
- Knowledge Filter: Show only `#research` nodes

**"File Explorer" Lens**:
- Topology Policy: `topology:dag` (strict hierarchy, unique names per parent)
- Layout: Indented Tree List
- Physics: None (static)
- Knowledge Filter: Files and Folders ontology

Both use the same `CanvasRegistry` contract — different configurations produce entirely different modes.

### Agents as Cognitive Processes

Agents are autonomous cognitive processes distinct from Actions. While Actions are discrete deterministic command handlers that return `Vec<GraphIntent>`, Agents are persistent observers that may connect to external AI intelligence providers (LLMs, classifiers, embedding models) and emit intent streams over time based on app state changes or timers. Managed through `AgentRegistry`: definitions include an observe trigger, an optional intelligence provider binding, and an intent emitter. Scheduling (timer-based agents like a prefetch scheduler) is a subset of this — the `AgentRegistry` is the registration surface for all such autonomous cognitive processes, from simple background tasks to full AI-driven graph analysis (Personal Crawler, automated UDC classification, semantic clustering suggestions).

### Storage Abstraction
The `ProtocolRegistry` effectively abstracts storage. In the future, Verse Tier 2 could enable saving a workspace to `ipfs://...` via an IPFS protocol handler, just as reading `https://...` is handled by the HTTP handler (Verso mod). Core seeds provide `file://` and `about:` for offline operation. Phase 5 (Verse Tier 1) focuses on bilateral sync between trusted devices, not content-addressed storage protocols.

---

## Progress

### 2026-02-22
- Plan created. `ProtocolRegistry` scaffolded in codebase.
- Phase 0 complete: `desktop/registries/` modules created, single-path routing active, diagnostics channels and contract tests added, harness scenarios added, legacy URL fallback removed.
- Phase 1 core decoupling (originally labeled Phase 1+3):
    - Protocol/viewer routing: cancellation-aware resolution, MIME hint inference, `phase0_decide_navigation_with_control` as canonical entrypoint.
    - Action decoupling: `ActionRegistry` with `action.omnibox_node_search`, `action.graph_view_submit`, `action.detail_view_submit`. Diagnostics contracts.
    - Input decoupling: `InputRegistry` with toolbar submit + nav bindings. `phase2_resolve_toolbar_submit_binding`, `phase2_resolve_input_binding`.
    - Lens scaffolding: `LensCompositor` with compositional lens definitions (layout_id, theme_id, physics_id). Atomic `LayoutRegistry`, `ThemeRegistry`, `PhysicsRegistry`. Composed diagnostics (per-component lookup channels).
    - Persisted user override source (`workspace:settings-registry-*-id`). Settings UI controls.
    - Identity scaffolding: `IdentityRegistry`, sign/fallback, diagnostics contracts.
    - `DiagnosticsRegistry` consolidated as sole contract source; `diagnostics_contract.rs` removed.
- Validation: `cargo check` (pass), all `desktop::registries::`, `webview_controller::tests::`, `desktop::tests::scenarios::registries::` pass.

### 2026-02-23 — Consolidated Checkpoint
- Phase 1 callsite migration complete (runtime path promotion):
    - Toolbar input resolution routes through registry input bindings by default (no diagnostics-gated fallback path).
    - Address-bar submit / omnibox / detail flows route through registry protocol/action helpers by default.
    - Files: `desktop/toolbar_routing.rs`, `desktop/webview_controller.rs`.
- Phase 1.4 capability topology slice complete:
    - Diagnostics registry implementation moved to `registries/atomic/diagnostics.rs`.
    - Temporary compatibility re-export at `desktop/registries/diagnostics.rs` removed.
    - Module wiring: `registries/mod.rs`, `registries/atomic/mod.rs`, crate root registration.
    - Diagnostics contract continuity: `registry.diagnostics.config_changed` emission path validated.
- Validation: `cargo test webview_controller::`, `desktop::registries::`, `desktop::tests::scenarios::registries::` all pass. `cargo check` pass.
- Documentation: `registry_migration_plan.md` and `2026-02-23_registry_architecture_critique.md` consolidated into this document and archived to `archive_docs/checkpoint_2026-02-23/`.
- Phase 2 complete:
    - `ModRegistry` lifecycle and status model implemented with dependency resolution and diagnostics channels.
    - Protocol and viewer contract surfaces formalized as atomic registries with core seeds.
    - Verso capability gating integrated (`viewer:webview`) including mod-disable paths.
- Phase 3 complete:
    - `LayoutDomainRegistry` introduced and used for composed profile resolution.
    - `CanvasRegistry` expanded with explicit topology/layout-algorithm/interaction policy sections.
    - `WorkbenchSurfaceRegistry` and `ViewerSurfaceRegistry` resolution integrated into runtime/layout callsites.
- Phase 4 complete through Step 4.4:
    - `ThemeData` formalized and default/dark theme seeds retained.
    - Physics profile atomic registry introduced (`physics:liquid`, `physics:gas`, `physics:solid`), with layout coupling removed.
    - `PresentationDomainRegistry` integrated into lens/profile normalization and resolution paths.
    - `KnowledgeRegistry` promoted to atomic layer with desktop compatibility shim for semantic reconciliation.
- Validation: focused registry/domain tests + full `cargo test --lib` regression pass (`540 passed; 0 failed; 3 ignored`).
