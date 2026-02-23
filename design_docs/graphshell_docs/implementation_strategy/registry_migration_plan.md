# Registry System Migration Plan (2026-02-22)

**Status**: In Progress
**Goal**: Execute the migration of hardcoded subsystems to the Registry Layer defined in `2026-02-22_registry_layer_plan.md`.

## Consolidated Checkpoint (2026-02-23)

Completed in thin, test-gated slices:

1. **Phase 1 callsite migration (runtime path promotion)**
  - Toolbar input resolution now routes through registry input bindings by default.
  - Address-bar submit/omnibox/detail flows now route through registry protocol/action helpers by default (no diagnostics feature-gated fallback path).
  - Files: `desktop/toolbar_routing.rs`, `desktop/webview_controller.rs`.

2. **Phase 1.4 capability topology slice (path-only)**
  - Diagnostics registry implementation moved to `registries/atomic/diagnostics.rs`.
  - Temporary compatibility re-export retained at `desktop/registries/diagnostics.rs` for this transition slice.
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
- `atomic/` (`protocol`, `layout`, `theme`, `physics`, `action`, `identity`, `ontology`)
- `domain/` (`lens`, `input`, `verse`)

### 3) Services (Infrastructure)

`src/services/`
- `persistence/`
- `search/`
- `physics/` (engine integration and long-running simulation utilities)

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
2.  **Seed**: Register existing behaviors as default items in the registry.
3.  **Replace**: Switch call sites to use the registry. Delete the old hardcoded logic immediately.
4.  **Verify**: Use Diagnostics to confirm registry hits.

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

## Phase 2: Protocols & Viewers (Core Architecture)

**Target**: Decouple `webview_controller.rs` from HTTP-only assumptions.

### Step 2.1: Protocol Handlers
- **Task**: Move `about:` logic to `InternalProtocolHandler`.
- **Task**: Wrap `reqwest` logic in `HttpProtocolHandler`.
- **Task**: Register `http`, `https`, `about` in `ProtocolRegistry`.

### Step 2.2: Viewer Handlers
- **Task**: Create `WebViewer` (wraps current Servo logic).
- **Task**: Register `text/html` (and fallback) to `WebViewer` in `ViewerRegistry`.

### Step 2.3: Controller Refactor
- **Target**: `desktop/webview_controller.rs`.
- **Refactor**:
  - Change `load_url` to call `app.services.protocol.resolve(url)`.
  - Use the returned stream/content to select a viewer via `app.services.viewer.select(content_type)`.
  - *Note*: For Phase 2, we can just assert the viewer is `WebViewer` to keep Servo integration stable, then generalize in Phase 4.

---

## Phase 3: Layout & Physics (Visuals)

**Target**: Move `app.physics` and `render/mod.rs` layout logic to registries.

### Step 3.1: Physics Registry
- **Task**: Extract `PhysicsProfile` defaults (Liquid, Gas, Solid) from `app.rs`.
- **Task**: Register them in `PhysicsRegistry`.
- **Refactor**: Update `LensConfig` to store `physics_id: String` instead of raw struct.

### Step 3.2: Layout Registry
- **Task**: Wrap `egui_graphs::LayoutForceDirected` in a `LayoutAlgorithm` trait impl.
- **Task**: Register as `layout.force_directed`.
- **Refactor**: Update `render/mod.rs` to instantiate the layout engine via `app.register.layout.create(id)`.

---

## Phase 4: Themes (UI Polish)

**Target**: Remove hardcoded `SettingsStyle` in `render/mod.rs`.

### Step 4.1: Theme Definition
- **Task**: Define `ThemeData` struct (colors, strokes, font sizes).
- **Task**: Create `DefaultTheme` matching current hardcoded values.

### Step 4.2: Registry Integration
- **Task**: Register `theme.default` in `ThemeRegistry`.
- **Refactor**: Update `render/mod.rs` to fetch `ThemeData` from registry based on `LensConfig.theme_id`.

---

## Phase 5: Identity & Verse (New Capabilities)

**Target**: Prepare for P2P by abstracting identity.

### Step 5.1: Identity Registry
- **Task**: Implement `IdentityRegistry` with `LocalIdentity` (generated keypair).
- **Task**: Update persistence to sign snapshots (if enabled) using the active identity.

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
        - **Signal Bus**: Manages inter-registry events (`IdentityChanged`, `ThemeChanged`).
        - **Control Panel**: Exposes configuration logic.
2.  **Phase 1 (Input/Action)**: Unblocks "Command Palette" and "Keybind Config".
3.  **Phase 1.4 (Capability Topology Slice)**: Move registries to `src/registries/*`.
4.  **Phase 3 (Layout/Physics)**: Unblocks "Multi-Graph Pane" advanced features.
5.  **Phase 2 (Protocols)**: Unblocks "Verse" and "Settings" pages.
6.  **Phase 4 (Themes)**: Low priority polish.
7.  **Phase 5 (Identity/Verse)**.
8.  **Phase 6 (Topology Consolidation)**: model/services/shell finalization.

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
*   **Risk**: Large path moves hide logic regressions.
  *   *Mitigation*: Separate path-only commits from behavior commits; run scenario matrix after each slice.
*   **Risk**: Test harness path churn breaks migration velocity.
  *   *Mitigation*: Update harness imports in the same slice as each move and keep diagnostics contracts as continuity checks.
```
