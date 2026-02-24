# Registry System Migration Plan (2026-02-22)

**Status**: In Progress
**Goal**: Execute the migration of hardcoded subsystems to the Registry Layer defined in `2026-02-22_registry_layer_plan.md`.

## Consolidated Checkpoint (2026-02-23)

Completed in thin, test-gated slices:

1. **Phase 1 callsite migration (runtime path promotion)**
  - Toolbar input resolution now routes through registry input bindings by default.
  - Address-bar submit/omnibox/detail flows now route through registry protocol/action helpers by default (no diagnostics feature-gated fallback path).
  - Files: `desktop/toolbar_routing.rs`, `desktop/webview_controller.rs`.

2. **Phase 1.4 capability topology slice (path-only)** [Complete]
  - Diagnostics registry implementation moved to `registries/atomic/diagnostics.rs`.
  - Temporary compatibility re-export at `desktop/registries/diagnostics.rs` has been removed (cleanup complete as of 2026-02-23).
  - Module wiring added: `registries/mod.rs`, `registries/atomic/mod.rs`, crate root registration in `lib.rs`.

3. **Diagnostics contract continuity**
  - Config-change channel emission path validated via deterministic scenario flow (`registry.diagnostics.config_changed`).

Validation evidence for this checkpoint:
- `cargo test webview_controller:: -- --nocapture` (pass)
- `cargo test desktop::registries:: -- --nocapture` (pass)
- `cargo test desktop::tests::scenarios::registries -- --nocapture` (pass)
- `cargo check` (pass)

## Context
The Registry Layer Architecture defines the *destination*. This plan defines the *journey*: specific refactoring steps to move existing logic from `input/`, `app.rs`, and `render/` into the new registries without destabilizing the application.

The current repository layout has outgrown a flat, root-heavy structure. Registry migration should run in parallel with a **topology refactor** so architecture boundaries are visible in the filesystem (Data, Capabilities, Services, Shell) rather than implicit in module names.

---

## Structural Concern (Added 2026-02-23)

The registry migration plan now formally includes a repository topology track.

**Why this is in-scope for registry migration**:
- Registries are becoming the capability API surface; their location should reflect that role.
- `GraphBrowserApp` currently mixes model/state with shell concerns, making borrow boundaries and test boundaries harder to reason about.
- A future multi-shell target (`desktop`, `mobile`, `cli`) requires model/services layers that are shell-agnostic.

**Decision**:
- Keep registry feature migration and directory migration coupled, but execute in **thin slices** (no big-bang move).

---

## Target Repository Topology

### 1) Core (Data & Logic)

`src/model/`
- `graph/` (existing graph module)
- `intent.rs` (GraphIntent + reducer ownership split from app shell)
- `session.rs` (GraphBrowserApp core state after shell fields are carved out)
- `selection.rs`

### 2) Capabilities (Registries)

`src/registries/`
- `mod.rs` (RegistryRuntime)
- `infrastructure/` (`diagnostics`, `mod_loader`)
- `atomic/` (`protocol`, `index`, `action`, `identity`, `agent`)
- `domain/layout/` (`layout`, `workbench_surface`, `graph_surface`, `viewer_surface`)
- `domain/presentation/` (`presentation`, `theme`, `physics_profile`)
- `domain/` (`lens`, `input`)

`src/mods/`
- `mod.rs` (mod manifest, dependency resolution, loading)
- `native/` (compile-time registered mods: `verso`, `verse`, `default_themes`, etc.)
- `wasm/` (dynamically loaded sandboxed mods)

### 3) Services (Infrastructure)

`src/services/`
- `persistence/`
- `search/`
- `physics/` (engine integration and long-running simulation utilities)

**Mod-first principle**: Registries define capability contracts (empty surfaces). Core seeds provide minimal defaults so the app is usable without any mods (graph organizer with local files). Mods populate registries with richer capabilities (web rendering, P2P networking, alternative viewers).

### 4) Shell (Presentation)

`src/shell/desktop/`
- `host/` (`window`, `headed_window`, `headless_window`, `event_loop`, embedder glue)
- `workbench/tiles/` (all `tile_*` files)
- `workbench/frame.rs` (from `gui_frame.rs`)
- `lifecycle/` (`webview_controller`, `lifecycle_reconcile`, `webview_backpressure`)
- `ui/toolbar/`, `ui/panels/`, `ui/gui.rs`

---

## Topology Migration Rules

1. Move one semantic area at a time; preserve behavior before cleanup.
2. Prefer `pub(crate)` re-exports during transition to avoid broad callsite churn.
3. Do not combine large path moves with logic rewrites in the same change-set.
4. Every move slice must pass:
  - `cargo test desktop::registries:: -- --nocapture`
  - `cargo test desktop::tests::scenarios::registries -- --nocapture`
  - `cargo check`
5. Once a slice is stable, delete compatibility re-exports quickly (no long-lived dual paths).

## Migration Strategy: Iterative Replacement

Since there are no active users, we prioritize **code cleanliness** over backward compatibility. We will replace subsystems directly rather than maintaining parallel legacy paths.

1.  **Stand Up**: Initialize **The Register** (central container) to hold registries.
2.  **Seed**: Register core-seed defaults (the minimum set that makes the app usable without any mods).
3.  **Replace**: Switch call sites to use the registry. Delete the old hardcoded logic immediately.
4.  **Verify**: Use Diagnostics to confirm registry hits.
5.  **Semantic Gap Check**: Before adding/changing registry boundaries, ask whether the boundary maps cleanly to technical, architectural, or design concerns.

### Mod-First Architecture Principle

Registries define **contracts** (empty capability surfaces with fallback defaults). Mods **populate** those surfaces with implementations. Two mod tiers exist:

- **Native Mods**: Compiled into the binary, registered at startup via `inventory::submit!`. Not sandboxed. Used for first-party capabilities too large or too tightly integrated for WASM (Verso, Verse, default themes/physics).
- **WASM Mods**: Dynamically loaded at runtime via `extism`. Sandboxed, capability-restricted, quota-limited. Used for third-party extensions and optional capabilities.

Both tiers use the same `ModManifest` format declaring `provides` (registry entries the mod registers) and `requires` (registry contracts that must exist before the mod loads). The mod loader performs topological sort at startup to resolve dependency order.

### Core vs. Mod-Provided Defaults

The application must be fully functional as an offline graph-based document organizer **without any mods loaded**. This defines the "core seed" — the minimal registry population:

| Registry | Core Seed (no mods) | Verso Mod Adds | Verse Mod Adds |
|---|---|---|---|
| ProtocolRegistry | `file://`, `about:` | `http://`, `https://`, `data:` | `ipfs://`, `activitypub://` |
| ViewerRegistry | `viewer:plaintext`, `viewer:metadata` | `viewer:webview` (Servo) | — |
| ActionRegistry | `graph.*`, `view.*`, `workspace.*` | `navigation.*`, `webview.*` | `verse.share`, `verse.sync` |
| InputRegistry | Graph/workspace keybindings | Browser-style keybindings | — |
| ThemeRegistry | `theme:default`, `theme:dark` | — | — |
| PhysicsProfileRegistry | `physics:liquid`, `physics:gas` | — | — |
| LayoutRegistry | `layout:default`, `layout:grid` | — | — |
| IdentityRegistry | `identity:local` (generated keypair) | — | P2P personas, DID providers |
| IndexRegistry | Local tantivy search | — | Federated search providers |
| KnowledgeRegistry | UDC defaults | — | Schema.org providers |

Without Verso: no webviews, no HTTP. Nodes display as metadata cards with title/URL/tags. The graph is a visual outliner / Zettelkasten.
Without Verse: no P2P, no federated search. Fully offline. Local identity only.

---

## Phase 1: Input & Actions (High Impact, Low Risk)

**Target**: Replace `input/mod.rs` (hardcoded `KeyboardActions`) and `render/mod.rs` (enum-based `GraphAction`) with `InputRegistry` and `ActionRegistry`.

### Step 1.1: Action Definitions
- **Task**: Define `ActionId` constants for all current capabilities.
  - `graph.node.create`, `graph.selection.delete`, `view.zoom.in`, etc.
- **Task**: Implement `ActionHandler` trait wrappers for existing `GraphIntent` emission logic.
- **Task**: Seed `ActionRegistry` with these defaults on startup.

### Step 1.2: Input Mapping
- **Task**: Extract keybindings from `input/mod.rs` into a data structure.
- **Task**: Seed `InputRegistry` with these default bindings mapping to `ActionId`s.

### Step 1.3: Call Site Migration
- **Target**: `desktop/gui.rs` (main loop) and `desktop/tile_behavior.rs`.
- **Refactor**:
  - Replace `input::collect_actions(ctx)` with `app.services.input.resolve(ctx)`.
  - Replace `match action { ... }` dispatch blocks with `app.services.actions.execute(action_id)`.

**Validation**:
- Verify all keyboard shortcuts still work.
- Verify Command Palette (which will now query `ActionRegistry`) lists all commands.

### Phase 1.4: Capability Topology Slice (path-only)
- **Task**: Move `desktop/registries/*` into `src/registries/{atomic,domain}` with minimal path adaptation.
- **Task**: Keep temporary re-exports in `desktop::registries` for one slice only.
- **Done Gate**: All registry tests/scenarios green with old shim removed.

---

## Phase 2: Mod Infrastructure & Protocol/Viewer Contracts

**Target**: Stand up the mod system as the primary extensibility mechanism. Define protocol and viewer contracts as registry surfaces. Package current Servo integration as the Verso native mod.

### Step 2.1: Mod Manifest & Loader
- **Task**: Define `ModManifest` struct: `mod_id`, `display_name`, `mod_type` (Native | WASM), `provides` (list of registry entry IDs), `requires` (list of registry contract IDs), `capabilities` (network, filesystem, etc.).
- **Task**: Implement mod dependency resolver (topological sort on `requires` → `provides` edges).
- **Task**: Implement native mod loader using `inventory::submit!` for compile-time registration.
- **Task**: Register mod lifecycle diagnostics (`registry.mod.load_started`, `registry.mod.load_succeeded`, `registry.mod.load_failed`, `registry.mod.dependency_missing`).

### Step 2.2: Protocol & Viewer Contracts (Registry Surfaces)
- **Task**: Define `ProtocolHandler` trait as the contract surface (scheme → handler).
- **Task**: Define `ViewerHandler` trait as the contract surface (MIME/content → renderer).
- **Task**: Seed core defaults: `protocol:file`, `protocol:about`, `viewer:plaintext`, `viewer:metadata`.
- **Task**: Ensure the app is fully functional with only core seeds (graph + metadata display, no web rendering).

### Step 2.3: Verso Native Mod
- **Task**: Package current Servo/Wry integration as a native mod with manifest:
  - `provides`: `protocol:http`, `protocol:https`, `protocol:data`, `viewer:webview`
  - `requires`: `ProtocolRegistry`, `ViewerRegistry`
  - `capabilities`: `network`
- **Task**: Refactor `webview_controller.rs` so webview creation is gated on `viewer:webview` being registered; if absent, nodes display as metadata-only.
- **Task**: Ensure startup without Verso mod succeeds (offline graph organizer mode).

**Validation**:
- App starts and functions as graph organizer with Verso mod disabled.
- App starts and functions as browser with Verso mod enabled.
- Mod dependency resolution rejects a mod with unmet `requires`.

---

## Phase 3: Layout Domain (Structure + Interaction First)

**Target**: Resolve all three surface forms through a unified layout domain: workbench (tile tree), graph (file tree), and viewer document surfaces. Each surface registry controls structure, interaction policy, and rendering policy — everything about how a surface is arranged and interacted with before styling.

### Step 3.1: Layout Domain Coordinator
- **Task**: Introduce `LayoutDomainRegistry` as the coordinator for surface subregistries.
- **Task**: Keep `LayoutRegistry` as the top-level domain entrypoint/fallback resolver.
- **Refactor**: Lens resolution obtains a composed layout profile from layout domain, not a single mode only.

### Step 3.2: Graph Surface Subregistry
- **Task**: Define `GraphSurfaceRegistry` covering the full scope of graph canvas behavior:
  - **Layout algorithms**: Wrap `egui_graphs::LayoutForceDirected` in a `LayoutAlgorithm` trait impl. Register IDs (`graph_layout:force_directed`, `graph_layout:grid`, `graph_layout:tree`).
  - **Interaction policy**: Selection modes, zoom/pan ranges, node creation positions, edge creation rules. Extract from hardcoded `SettingsNavigation` / `SettingsInteraction` in `render/mod.rs`.
  - **Rendering policy**: Node shapes/sizes, edge routing/style, label format, badge display rules. Extract from hardcoded `SettingsStyle`.
  - **Physics engine integration**: Which force profiles are available, energy thresholds, auto-pause triggers. (Parameters come from Presentation Domain; engine execution is Layout Domain.)
- **Refactor**: Update `render/mod.rs` to instantiate graph layout/interaction/style via surface-registry dispatch.

### Step 3.3: Workbench Surface Subregistry
- **Task**: Define `WorkbenchSurfaceRegistry` covering tile tree behavior:
  - **Layout policy**: `SimplificationOptions`, split direction defaults, tab wrapping rules.
  - **Interaction policy**: Drag-to-rearrange rules, resize constraints, drop zone behavior.
  - **Rendering policy**: Tab bar style, container labels (semantic: `Split ↔`, `Tab Group`, etc.), title truncation.
- **Refactor**: Update `tile_behavior.rs` to resolve policy profiles via workbench surface registry.

### Step 3.4: Viewer Surface Subregistry
- **Task**: Define `ViewerSurfaceRegistry` covering viewer viewport behavior:
  - Zoom/scaling defaults, reader mode, scroll policy.
  - (Viewer *selection* — MIME routing — stays in atomic `ViewerRegistry`; this handles *how* the selected viewer presents its viewport.)
- **Refactor**: Update viewer entrypoints to resolve surface policies via layout domain.

**Note**: The graph surface registry is comparable in scope to the workbench surface registry. Both the tile tree and the file tree are extensible surfaces where mods can register new policies, types, and actions.

---

## Phase 4: Presentation Domain & Knowledge Registry

**Target**: Resolve appearance and motion semantics after layout. Formalize knowledge classification as a distinct atomic registry.

### Step 4.1: Theme Subregistry
- **Task**: Define `ThemeData` struct (colors, strokes, font sizes).
- **Task**: Create `DefaultTheme` matching current hardcoded values.
- **Task**: Register `theme:default` in `ThemeRegistry`. Core seed — available without mods.

### Step 4.2: Physics Profile Subregistry
- **Task**: Extract `PhysicsProfile` presets (Liquid, Gas, Solid) from `app.rs` as named parameter sets.
- **Task**: Register them in `PhysicsProfileRegistry` as presentation-domain semantic labels.
- **Task**: Remove `layout_mode` from `PhysicsProfile`. Layout mode is independently resolved by the Layout Domain. A Lens composes both, but physics must not override layout.
- **Refactor**: Physics *engine execution* stays in the Layout Domain (`GraphSurfaceRegistry`). The Presentation Domain provides *which parameter set to use*. The Lens resolves the profile from Presentation and passes it to the engine in Layout.

### Step 4.3: Presentation Domain Coordinator
- **Task**: Make `PresentationDomainRegistry` the domain coordinator for `ThemeRegistry` + `PhysicsProfileRegistry`.
- **Refactor**: Update `render/mod.rs` so theme/physics resolution occurs after layout profile selection.

### Step 4.4: Knowledge Registry (Atomic)
- **Task**: Formalize the existing `OntologyRegistry` (UDC tagging, `CompactCode`, fuzzy search, `get_color_hint`) as `KnowledgeRegistry` — an atomic capability, not a domain coordinator.
- **Task**: Register as core seed with UDC defaults. Mods can add Schema.org, Wikidata, or custom taxonomy providers.
- **Refactor**: Lens filters can reference knowledge tags; knowledge resolution is independent of both layout and presentation domains.

---

## Phase 5: Verse Native Mod (P2P Capabilities)

**Target**: Package P2P networking, federated identity, and distributed indexing as the Verse native mod.

### Step 5.1: Verse Mod Manifest
- **Task**: Define Verse mod manifest:
  - `provides`: `protocol:ipfs`, `protocol:activitypub`, `index:federated`, `identity:did`, `action:verse.share`, `action:verse.sync`
  - `requires`: `ProtocolRegistry`, `IndexRegistry`, `IdentityRegistry`, `ActionRegistry`
  - `capabilities`: `network`, `identity`
- **Task**: Implement Verse mod as a native mod that registers protocol handlers, index providers, identity providers, and actions into existing atomic registries on load.

### Step 5.2: Identity Providers
- **Task**: The Verse mod extends `IdentityRegistry` with P2P persona types (DID, Nostr keypairs).
- **Note**: `IdentityRegistry` itself is a core atomic registry (local keypair generation works without Verse). Verse adds networked identity types.

### Step 5.3: Offline Graceful Degradation
- **Task**: When Verse mod is not loaded, all `verse.*` actions are absent from ActionRegistry. Command Palette does not show share/sync commands.
- **Task**: When Verse mod is loaded but offline, protocol resolution for `ipfs://` / `activitypub://` fails gracefully with diagnostics and user-visible fallback messaging.

**Validation**:
- App starts and functions fully without Verse mod loaded.
- Loading Verse mod registers all declared entries into atomic registries.
- Unloading Verse mod removes provided entries; dependent workspaces fall back to defaults.

---

## Phase 6: Topology Consolidation (Model / Services / Shell)

**Target**: Align filesystem structure with architecture boundaries after capability paths are stable.

### Step 6.1: Model extraction
- Move data-centric types from root-heavy modules into `src/model/*`.
- Split shell/runtime fields from `GraphBrowserApp` into shell-facing adapters where practical.

### Step 6.2: Service extraction
- Normalize `persistence`, `search`, and physics-support logic under `src/services/*`.

### Step 6.3: Desktop shell decomposition
- Re-home `desktop/*` into `src/shell/desktop/{host,workbench,lifecycle,ui}`.
- Group tile files under `workbench/tiles` and keep diagnostics harness paths updated.

### Step 6.4: Remove transition shims
- Delete temporary re-exports and old module aliases.
- Ensure docs and plan references use canonical paths only.

---

## Execution Order

1.  **Scaffold**: Create/normalize `RegistryRuntime` container and capability boundaries.
    - **The Register**:
        - **Container**: Holds all Atomic and Domain registries.
        - **Signal Bus**: Manages inter-registry events (`IdentityChanged`, `ThemeChanged`, `ModLoaded`, `ModUnloaded`).
        - **Control Panel**: Exposes configuration logic.
    - **Core Seeds**: Populate minimal defaults for each registry so the app is functional without mods.
2.  **Phase 1 (Input/Action)**: Unblocks "Command Palette" and "Keybind Config".
3.  **Phase 1.4 (Capability Topology Slice)**: Move registries to `src/registries/*`.
4.  **Phase 2 (Mod Infrastructure + Protocols/Viewers)**: Stands up the mod system. Packages Verso as a native mod. Establishes the "core seed floor" (app works without mods).
5.  **Phase 3 (Layout Domain)**: Unblocks multi-surface consistency across workbench, graph, and viewers. Surface registries control structure + interaction + rendering policy.
6.  **Phase 4 (Presentation Domain + Knowledge)**: Applies style and motion semantics to resolved layout. Formalizes UDC tagging as KnowledgeRegistry.
7.  **Phase 5 (Verse Mod)**: Packages P2P as a native mod with registry prerequisites.
8.  **Phase 6 (Topology Consolidation)**: model/services/shell finalization.

**Ordering rationale**: Mod infrastructure (Phase 2) must precede domain phases so that surface registries and presentation registries can be populated by mods from the start. Verso-as-mod is the forcing function — if the mod system can support Servo integration, it can support anything.

## Verification Plan

For each phase:
1.  **Unit Test**: Verify Registry returns expected default items.
2.  **Integration Test**: Use `TestHarness` to assert that triggering an input results in the correct `GraphIntent` via the registry path.
3.  **Diagnostics**: Check `registry.*.fallback_used` channels are 0 (unless testing fallback).

## Risks & Mitigations

*   **Risk**: Performance regression in Input/Action lookup (per-frame).
    *   *Mitigation*: `InputRegistry` should use a fast lookup (Hash map of KeyChord). It is only queried on input events, not every frame.
*   **Risk**: Circular dependencies between `Register` and `GraphBrowserApp`.
    *   *Mitigation*: Strict `Context` pattern. Registries never hold `&mut App`, they receive it during method calls.
*   **Risk**: Semantic overlap between layout and presentation domains creates duplicate knobs.
    *   *Mitigation*: Enforce sequencing (`layout -> presentation`) and keep cross-domain coupling out of registry APIs. Physics engine execution is Layout; physics parameter presets are Presentation.
*   **Risk**: Large path moves hide logic regressions.
    *   *Mitigation*: Separate path-only commits from behavior commits; run scenario matrix after each slice.
*   **Risk**: Test harness path churn breaks migration velocity.
    *   *Mitigation*: Update harness imports in the same slice as each move and keep diagnostics contracts as continuity checks.
*   **Risk**: Mod loading order creates startup failures or silent capability gaps.
    *   *Mitigation*: Topological sort on mod `requires`/`provides` at startup. Missing dependency = mod load failure with diagnostics, not silent skip. Core seeds guarantee a functional floor.
*   **Risk**: Verso-as-mod creates a hard coupling to Servo that the mod contract can't cleanly express.
    *   *Mitigation*: Verso is a **native mod** — compiled in, not sandboxed. The mod contract (manifest + registry population) is the architectural boundary, not an execution sandbox. If the mod API can't express Verso's needs, the mod API is wrong — fix the API, don't special-case Verso.
*   **Risk**: "Core seed floor" is too minimal to be useful, pushing all value into mods.
    *   *Mitigation*: Core seeds include graph manipulation, local file protocol, plaintext/metadata viewers, full keyboard/action pipeline, persistence, search. This is a complete offline document organizer. Mods add web rendering and networking, not basic functionality.
```
