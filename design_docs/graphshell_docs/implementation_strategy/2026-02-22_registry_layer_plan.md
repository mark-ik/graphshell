# Registry Layer Architecture Plan (2026-02-22)

**Status**: In Progress
**Goal**: Decompose Graphshell's monolithic logic into a modular ecosystem of registries, enabling extensibility and the "Knowledge User Agent" vision.

## Plan

### Context
Graphshell currently hardcodes behavior for URL handling, rendering, and action dispatch. To support the "Verse" vision (P2P, alternative protocols, custom viewers) and advanced UX (Lenses, Agents), we need a mod-extensible architecture where capabilities are registered rather than hardcoded.

### Migration Policy Note

Since there are no active users, we prioritize **code cleanliness** over backward compatibility. We will replace subsystems directly rather than maintaining parallel legacy paths.

### Iterative Replacement Strategy (Operational)

We execute replacement in thin, test-gated slices:

1. Implement one registry-owned path end-to-end.
2. Add/expand diagnostics contract tests + harness scenario assertions.
3. Make the new path the default runtime path immediately.
4. Delete the replaced legacy branch in the same slice once tests are green.

No long-lived dual-path mode, no feature-flag forks for migrated behavior.

### Canonical Terminology

To keep architecture and implementation language consistent:

- **Action** is the canonical term for executable user/system behaviors (`ActionId`, `ActionRegistry`).
- **Mod** is the canonical term for extension units (plugin/extension are aliases in explanatory text only).
- **DiagnosticsRegistry** is the canonical component for diagnostic channel and schema contracts.
- **Atomic Registry** means primitive capability registry; **Domain Registry** means composite context registry.
- **Lens / Input / Verse / Workflow** are domain names; concrete implementations are `*Registry` types.


### The Registry Ecosystem

We organize the registries into **Atomic Primitives** (The Vocabulary) and **Domain Configurations** (The Sentences).

#### 1. Atomic Registries (Primitives)
These manage specific, isolated resources or algorithms. Mods can extend these directly.

**Persistence & I/O Domain**
*   **Protocol Registry**:
    *   **Role**: Maps URL schemes (`ipfs://`, `file://`) to handlers. **(The Persistence Layer)**.
    *   **Interface**: `resolve(uri) -> Result<ContentStream>`
*   **Index Registry**:
    *   **Role**: Search backends (Local, Federated) and **History/Timeline** retrieval.
    *   **Interface**: `query(text) -> Iterator<Result>`

**Presentation & Physics Domain**
*   **Viewer Registry**:
    *   **Role**: Maps MIME types/extensions to renderers (PDF, Markdown, CSV).
    *   **Interface**: `render(ui, content)`
*   **Layout Registry** (Atomic):
    *   **Role**: Positioning algorithms (`LayoutId` -> `Algorithm`).
    *   **Interface**: `compute_layout(graph) -> Positions`
*   **Theme Registry** (Atomic):
    *   **Role**: Manages UI themes, node color palettes, and syntax highlighting styles.
    *   **Interface**: `get_theme(id)`
*   **Physics Registry** (Atomic):
    *   **Role**: Manages force simulation parameters (`PhysicsId` -> `PhysicsProfile`).
    *   **Interface**: `get_profile(id)`

**Logic & Security Domain**
*   **Action Registry** (Atomic):
    *   **Role**: Definitions of executable actions (`ActionId` -> `Handler`).
    *   **Interface**: `execute(context) -> Vec<GraphIntent>`
*   **Identity Registry**:
    *   **Role**: Manages keys/DIDs for signing and auth. **(The Security Root)**.
    *   **Interface**: `sign(payload, persona)`
*   **Mod Registry**:
    *   **Role**: Manages lifecycle and **Capabilities/Sandboxing** of WASM mods.
    *   **Interface**: `load_mod(path)`, `unload_mod(id)`
*   **Agent Registry** (Atomic):
    *   **Role**: Definitions of autonomous background tasks (`AgentId` -> `Task`).
    *   **Interface**: `spawn(context)`, `schedule(cron)`
*   **Diagnostics Registry** (Atomic):
    *   **Role**: Central definition of diagnostic channels, schemas, and configuration (sampling/retention).
    *   **Interface**: `register_channel(def)`, `get_config(channel_id)`, `set_config(channel_id, config)`.

**Knowledge Domain**
*   **Ontology Registry**:
    *   **Role**: Semantic definitions, validation, and runtime indexing (`NodeKey` -> `CompactCode`).
    *   **Interface**: `validate(tag)`, `distance(a, b)`, `get_label(code)`.
    *   **Pattern**: Router for schema providers (UDC, Schema.org).

#### 2. Domain Registries (Composites)
These combine primitives to define a user experience context.

*   **Lens Domain**:
    *   **Role**: Defines how the graph *looks* and *moves*.
    *   **Composition**: `LayoutId` + `ThemeId` + `PhysicsId` + `FilterSet`.
    *   **Registry**: `LensRegistry`.
*   **Input Domain**:
    *   **Role**: Defines how the user *controls* the app.
    *   **Composition**: `InputEvent` -> `ActionId` mapping (Keybinds/Mousebinds).
    *   **Registry**: `InputRegistry`.
*   **Verse Domain**:
    *   **Role**: Defines the **Access Policy**, network context, and identity.
    *   **Composition**: `IdentityId` + `Protocol` policies + `Index` sources.
    *   **Registry**: `VerseRegistry`.
*   **Workflow Domain** (Future):
    *   **Role**: Defines the high-level *session mode*.
    *   **Composition**: Active `Lens` + Active `InputProfile` + Window Layout.
    *   **Registry**: `WorkflowRegistry`.

### Registry Testing Rule (Required)

Each registry ships with two test layers:

1.  **Registry Contract Tests** (unit-level):
    - Validate registration, lookup, conflict behavior, and fallback semantics.
    - Validate diagnostics schema stability for each exposed channel.
2.  **Harness Scenario Tests** (integration-level):
    - Validate end-to-end behavior through `desktop/tests/scenarios/*` using diagnostics snapshots.
    - Required for any new registry capability before it is considered complete.

### Diagnostics Contract Checklist (Required for Every Registry)

Before a registry is marked complete, all items below must be defined and validated:

- [ ] **Channel Naming**: Channels follow `registry.<name>.<event>` naming convention.
- [ ] **Schema Version**: Every emitted diagnostic payload includes `schema_version`.
- [ ] **Field Stability**: Required fields are documented; optional fields are explicitly marked nullable/optional.
- [ ] **Compatibility Policy**: Additive changes are allowed; removals/renames require deprecation for at least one release cycle.
- [ ] **Snapshot Coverage**: At least one harness scenario asserts channel counts or payload shape.
- [ ] **Failure Path Signals**: Error and fallback paths emit distinct diagnostics.
- [ ] **Test Fixtures**: Registry contract tests include at least one schema-golden fixture.

### Registry Contract Matrix (Execution Checklist)

| Registry | Owner | Primary Interface | Required Diagnostics Channels | Minimum Tests |
|---|---|---|---|---|
| ProtocolRegistry | Platform/Networking | `resolve(uri)` | `registry.protocol.resolve_started`, `registry.protocol.resolve_succeeded`, `registry.protocol.resolve_failed`, `registry.protocol.fallback_used` | 2 contract tests (lookup/fallback, error path), 1 harness scenario |
| ViewerRegistry | Desktop/Rendering | `render(ui, content)` | `registry.viewer.render_started`, `registry.viewer.render_succeeded`, `registry.viewer.render_failed`, `registry.viewer.fallback_used` | 2 contract tests (mime resolution, fallback), 1 harness scenario |
| ActionRegistry | App/Core | `execute(action, context)` | `registry.action.execute_started`, `registry.action.execute_succeeded`, `registry.action.execute_failed` | 2 contract tests (registration/conflict, intent emission), 1 harness scenario |
| InputRegistry | App/Input | `map_input(event)` | `registry.input.binding_resolved`, `registry.input.binding_missing`, `registry.input.binding_conflict` | 2 contract tests (mapping/override, missing binding), 1 harness scenario |
| LayoutRegistry | App/Layout | `compute_layout(graph)` | `registry.layout.compute_started`, `registry.layout.compute_succeeded`, `registry.layout.compute_failed`, `registry.layout.fallback_used` | 2 contract tests (algorithm lookup, fallback), 1 harness scenario |
| ThemeRegistry | Desktop/UI | `get_theme(id)` | `registry.theme.lookup_succeeded`, `registry.theme.lookup_failed`, `registry.theme.fallback_used` | 2 contract tests (lookup/fallback, missing theme), 1 harness scenario |
| PhysicsRegistry | App/Simulation | `get_profile(id)` | `registry.physics.lookup_succeeded`, `registry.physics.lookup_failed`, `registry.physics.fallback_used` | 2 contract tests (lookup/fallback, profile validation), 1 harness scenario |
| IdentityRegistry | Verse/Security | `sign(payload, persona)` | `registry.identity.sign_started`, `registry.identity.sign_succeeded`, `registry.identity.sign_failed`, `registry.identity.key_unavailable` | 2 contract tests (persona resolution, signing failure), 1 harness scenario |
| IndexRegistry | Search/Recall | `query(text)` | `registry.index.query_started`, `registry.index.query_succeeded`, `registry.index.query_failed`, `registry.index.fallback_used` | 2 contract tests (provider selection, failure fallback), 1 harness scenario |
| OntologyRegistry | Knowledge/Semantics | `get_schema(type_id)` | `registry.ontology.lookup_succeeded`, `registry.ontology.lookup_failed`, `registry.ontology.fallback_used` | 2 contract tests (schema lookup/versioning, fallback), 1 harness scenario |
| DiagnosticsRegistry | System/Observability | `register_channel(def)` | `registry.diagnostics.channel_registered`, `registry.diagnostics.config_changed` | 2 contract tests (registration, config override), 1 harness scenario |
| ModRegistry | Platform/Extensibility | `load_mod(path)`, `unload_mod(id)` | `registry.mod.load_started`, `registry.mod.load_succeeded`, `registry.mod.load_failed`, `registry.mod.security_violation`, `registry.mod.quarantine` | 3 contract tests (load/unload, denied capability, quarantine path), 1 harness scenario |
| LensRegistry | App/Presentation | `resolve_lens(id)` | `registry.lens.resolve_succeeded`, `registry.lens.resolve_failed`, `registry.lens.fallback_used` | 2 contract tests (composition resolution, fallback), 1 harness scenario |
| VerseRegistry | Verse/Runtime | `resolve_context(id)` | `registry.verse.resolve_started`, `registry.verse.resolve_succeeded`, `registry.verse.resolve_failed` | 2 contract tests (composition determinism, fallback), 1 harness scenario |
| WorkflowRegistry (future) | App/Session | `activate_workflow(id)` | `registry.workflow.activate_started`, `registry.workflow.activate_succeeded`, `registry.workflow.activate_failed` | 2 contract tests (activation/switching, fallback), 1 harness scenario |

**Matrix policy**:
- Each row must map to at least one concrete test file path before phase closure.
- Channels listed here are the canonical minimum set; rows may add channels but should not remove these without deprecation.

### Registry Interface Standard

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

### Refactoring Strategy: Data vs. Systems

To avoid borrow-checker conflicts and monolithic state, `GraphBrowserApp` must be split:

1.  **`GraphWorkspace` (Data)**: Pure data (Graph, Selection, Camera, Physics State). Serializable.
2.  **`AppServices` (Systems)**: Runtime systems (Registries, Windowing, Network). Ephemeral.
3.  **Registry Context**:
    - Registry methods take `&GraphWorkspace` or `&mut GraphWorkspace` as arguments, not `&mut GraphBrowserApp`.
    - Example: `ActionRegistry::execute(action, &mut workspace, &services)`.

This keeps the runtime modular while ensuring tests live in the existing harness topology (rather than hidden in ad-hoc module tests).

### Phase 0: Walking Skeleton (Concrete MVP Slice)
**Goal**: Deliver one thin vertical slice end-to-end before broad registry rollout.

1.  Create `desktop/registries/` root with `protocol`, `viewer`, and `diagnostics_contract` modules.
2.  Route a single path (`https://` + `text/html`) through `ProtocolRegistry` -> `ViewerRegistry`.
3.  Register and emit diagnostics channels for success + fallback + failure (`registry.protocol.*`, `registry.viewer.*`).
4.  Add one contract test module (`desktop/registries/tests/protocol_viewer_contract.rs`) covering register/resolve/fallback behavior.
5.  Add one harness scenario validating emitted diagnostics for the same flow.
6.  Remove the replaced legacy branch for that flow in the same change-set once matrix is green.

**Phase 0 done gate:**
- Single-path registry flow is active and deterministic.
- Contract tests and harness scenario are green in CI/local matrix.
- Fallback path is verified and emits diagnostics.
- No regression in current `desktop/tests/scenarios/*` matrix.

### Phase 1: Core Decoupling (Protocols & Viewers)
**Goal**: Move `webview_controller` logic into registries.
1.  Create `desktop/registries/` module.
2.  Move/Integrate `ProtocolRegistry` from `desktop/protocols/` to `desktop/registries/protocol.rs`.
3.  Refine `ProtocolHandler` trait to support async/streaming without blocking the frame loop.
4.  Implement `ViewerRegistry`.
5.  Refactor `webview_controller.rs` to use `app.registries.protocol.resolve(url)`.
6.  Introduce `DiagnosticsRegistry` contract for protocol/viewer channel schemas.

### Phase 2: UX Flexibility (Actions & Lenses)
**Goal**: Data-driven UI and view configuration.
1.  Implement `ActionRegistry` (superseding hardcoded match dispatch).
2.  Implement `InputRegistry` (superseding `input/mod.rs`).
3.  Implement `PhysicsRegistry`, `ThemeRegistry`, `LayoutRegistry`.
4.  Implement `LensRegistry` (composing the above).
5.  Update `GraphViewState` to use `LensConfig` (referencing IDs instead of raw profiles).

### Phase 3: Verse Foundation (Identity & Index)
**Goal**: Prepare for P2P (see `2026-02-22_verse_implementation_strategy.md`).
1.  Implement `IdentityRegistry` (keyring integration).
2.  Implement `IndexRegistry` (tantivy abstraction).
3.  Implement `VerseRegistry` to compose identity, protocol policies, and index sources.

### Phase 4: Advanced Knowledge (Ontology & Agents)
**Goal**: Semantic understanding and automation.
1.  Implement `OntologyRegistry` for Schema.org types.
2.  Implement `AgentRegistry` (as a specialized view of `ActionRegistry`) for background tasks.
3.  Implement `ModRegistry` to manage WASM host integration.
4.  **WASM Host Integration**: Allow agents and custom viewers to be loaded as WASM modules (using `wasmer` or `wasmtime`).

### Per-Phase Done Gates

Each phase is complete only when all gates pass.

#### Phase 1 Done Gate
- Protocol + viewer routing for target paths is registry-owned (no hidden bypass).
- Protocol resolution is non-blocking and cancellation-aware.
- Diagnostics contracts for protocol/viewer channels pass checklist and tests.
- Replaced legacy protocol/viewer dispatch branches are deleted.

**Phase 1 status (2026-02-22): Complete**
- Target submit paths in `desktop/webview_controller.rs` are registry-owned and now call control-aware phase-0 decision APIs.
- Protocol resolution exposes cancellation-aware control (`resolve_with_control`) and submit flow short-circuits cleanly on cancellation before viewer selection.
- Protocol/viewer diagnostics channels remain contract-backed and validated by unit + scenario suites.
- Legacy duplicated URL fallback policy in submit paths has been removed; registry policy is canonical.

#### Phase 2 Done Gate
- Action dispatch is registry-owned for migrated actions.
- Input mappings are data-driven through `InputRegistry`.
- Lens configuration uses IDs (layout/theme/physics) with fallback behavior.
- Config save/load roundtrip for user registries passes.

#### Phase 3 Done Gate
- Identity + index providers are pluggable and observable.
- Verse composition resolves identity/policy/index deterministically.
- Failure and offline modes have explicit fallback behavior and diagnostics.

#### Phase 4 Done Gate
- Ontology and agent paths emit stable diagnostics and respect runtime quotas.
- Mod loading/unloading is sandboxed, observable, and recoverable on failure.
- At least one mod-provided action and one mod-provided viewer pass contract + harness tests.

### Technical Stack & Patterns (Refinements)

To avoid reinventing wheels, we will adopt these established ecosystem patterns:

1.  **Protocol Registry as `tower::Service`**:
    -   **Crate**: `tower`
    -   **Pattern**: Middleware.
    -   **Refinement**: Define `ProtocolHandler` as `tower::Service<Uri, Response = ContentStream>`. This enables free use of standard middleware for timeouts, retries, concurrency limits, and tracing on any protocol (IPFS, Gemini, etc.).

2.  **Mod Registry via `extism`**:
    -   **Crate**: `extism` (wraps Wasmtime)
    -   **Pattern**: Universal Plug-in System.
    -   **Refinement**: Use Extism to handle the complex memory/host-function binding for WASM agents. It simplifies the "Host" implementation significantly compared to raw Wasmtime.

3.  **Ontology Registry via `sophia`**:
    -   **Crate**: `sophia`
    -   **Pattern**: Linked Data / RDF.
    -   **Refinement**: Use `sophia`'s traits for efficient, zero-copy parsing of JSON-LD (Schema.org) data, rather than generic JSON parsing.

4.  **Action Extraction (The Handler Pattern)**:
    -   **Inspiration**: `axum` / `bevy`.
    -   **Pattern**: Type-safe dependency injection.
    -   **Refinement**: Instead of passing a monolithic context to actions, implement an `FromContext` trait. Actions declare arguments (`fn cmd(selection: Selection)`) and the registry extracts them. This decouples actions from the full app state structure.

5.  **`schemars` (Auto-Configuration UI)**:
    -   **Crate**: `schemars`
    -   **Pattern**: Reflection / Schema Generation.
    -   **Refinement**: Registry items derive `JsonSchema`. The Settings UI uses this schema to auto-generate sliders, dropdowns, and inputs. This solves the "how do I configure a mod?" problem without writing UI code.

6.  **`inventory` (Static Registration)**:
    -   **Crate**: `inventory`
    -   **Pattern**: Distributed Slices.
    -   **Refinement**: Built-in actions/themes register themselves at compile time via `submit!`. Eliminates the need for a central "register all" function that touches every module.

### Robustness & Integration

To ensure the system scales safely:

#### 1. The "Missing Mod" Strategy (Graceful Degradation)
*   **Problem**: A workspace references a Layout/Theme provided by a mod that is no longer installed.
*   **Solution**:
    *   Registries store a hardcoded `fallback_id` (e.g., `layout:default`, `theme:dark`).
    *   Lookups use `get_or_default(id)`.
    *   The UI shows a warning: "Layout 'SuperGrid' missing, using Default."

#### 2. Registry Signal Bus (Decoupling)
*   **Problem**: `IdentityRegistry` changes persona; `ProtocolRegistry` needs to update keys.
*   **Solution**: A synchronous broadcast channel in `AppServices`.
    *   Events: `IdentityChanged`, `ThemeChanged`, `ModLoaded`, `ModUnloaded`.
    *   Registries implement `on_signal(&mut self, signal: RegistrySignal)`.

#### 3. Configuration UI (Auto-Generation)
*   **Problem**: Users need to tweak settings for specific registry items (e.g., Physics parameters).
*   **Solution**: Use `schemars` to derive schema.
    *   Registry items implement `Configurable` which defaults to returning `schemars::schema_for!(Self)`.
    *   A generic `SchemaWidget` in `desktop/ui` renders the controls based on the JSON schema.

#### 4. Macros (Intents as Scripts)
*   **Idea**: Since `GraphIntent` is serializable, a "Macro" is just a persisted `Vec<GraphIntent>`.
*   **Implementation**:
    *   `ActionRegistry` supports a `MacroHandler` variant.
    *   Users can "Record" a sequence of actions, save it as a new Action, and bind it to a key.

#### 5. Mod Security & Capability Policy
*   **Problem**: Mods can become an unbounded execution and data-exfiltration surface.
*   **Solution**:
    *   Capability manifest per mod (`network`, `filesystem`, `identity`, `clipboard`, `exec`) with deny-by-default policy.
    *   Runtime quotas for CPU time, memory, message rate, and outbound requests.
    *   Kill switch and quarantine mode for crashing or policy-violating mods.
    *   Security diagnostics channels (`registry.mod.security_violation`, `registry.mod.quarantine`).

#### 6. Configuration Precedence (No Ambiguity)
*   **Problem**: Built-in, user, and workspace settings can conflict.
*   **Solution**:
    *   Precedence order: `workspace override` > `user override` > `built-in default`.
    *   Every resolved value can report provenance (`resolved_from = workspace|user|default`).
    *   Conflicts emit diagnostics (`registry.config.conflict_detected`) and show deterministic UI resolution.

---

## Findings

### Lenses vs. Presets
A "Physics Preset" (Gas/Liquid/Solid) is just one component of a "Lens". A Lens might be "Research Mode" which composes:
- **Physics**: Liquid (Organic clustering)
- **Theme**: Dark Mode
- **Filter**: Show only `#research` nodes
- **Layout**: Free
- **Hub Lens**: Tree layout, high density, filename labels. Acts as the "File Explorer".

### Agents as Actions
Background agents (like the "Personal Crawler") are effectively actions that trigger automatically based on events or timers. They should likely be managed within the `ActionRegistry` infrastructure but exposed via an `AgentRegistry` interface for scheduling.

### Storage Abstraction
The `ProtocolRegistry` effectively abstracts storage. Saving a workspace to `ipfs://...` should be handled by the IPFS protocol handler, just as reading `https://...` is handled by the HTTP handler.

---

## Progress

### 2026-02-22
- Plan created.
- `ProtocolRegistry` scaffolded in codebase.
- `Multi-Graph Pane Plan` updated to reference Lenses.
- `Workbench Workspace Manifest Persistence Plan` updated to reference Identity/Protocol registries.
- Added `DiagnosticsRegistry` as a first-class registry requirement with explicit test-contract policy.
- Phase 0 implementation started in code:
    - Added `desktop/registries/` modules: `protocol`, `viewer`, `diagnostics_contract`, runtime entrypoint.
    - Added Phase 0 diagnostics channels and contract tests under `desktop::registries::tests`.
    - Added diagnostics-driven harness scenarios in `desktop/tests/scenarios/registries.rs`.
    - Replaced observe-only integration with centralized URL policy entrypoint in `desktop/registries/mod.rs` (`phase0_normalize_navigation_url` / `phase0_normalize_navigation_url_for_tests`).
    - Introduced canonical phase-0 registry navigation decision entrypoint in `desktop/registries/mod.rs` (`phase0_decide_navigation` / `phase0_decide_navigation_for_tests`) returning normalized URL + protocol/viewer selections.
    - Updated `desktop/webview_controller.rs` graph-view + detail-view submit flows to call the centralized registry normalization entrypoint.
    - Removed duplicated URL fallback policy logic from `desktop/webview_controller.rs`.
    - Added MIME-hint decision coverage in unit + scenario tests (`viewer:csv` via `text/csv` hint), while keeping runtime path webview-backed in Phase 0.
    - Extended `ProtocolRegistry` phase-0 resolution to include best-effort inferred MIME hints (`inferred_mime_hint`) from URI/data-URI metadata.
    - Updated phase-0 runtime decision flow to use protocol-inferred MIME hint when explicit hint is absent, while preserving explicit hint precedence.
    - Added coverage for inferred MIME behavior in unit + scenario tests (`data:text/csv,...` selecting `viewer:csv` without explicit hint).
    - Added a cancellation-aware protocol resolution surface (`ProtocolResolveControl` + `resolve_with_control`) for Phase 1 non-blocking/cancellation contract progression.
    - Added control-aware phase-0 decision entrypoints (`phase0_decide_navigation_with_control`, `phase0_decide_navigation_for_tests_with_control`) that short-circuit before viewer selection when cancelled.
    - Added unit + scenario coverage for cancellation short-circuit behavior and diagnostics (`registry.protocol.resolve_failed` emitted; viewer selection channels not emitted on cancellation).
    - Switched graph-view + detail-view submit normalization in `desktop/webview_controller.rs` to `phase0_decide_navigation_with_control` with explicit control plumbing (default active control in runtime path).
    - Added explicit runtime no-op submit handling for cancelled protocol resolution decisions.
    - Started Phase 2 action decoupling by introducing `desktop/registries/action.rs` (`ActionRegistry`) with `action.omnibox_node_search` execution path.
    - Added Phase 2 diagnostics contracts for action execution channels (`registry.action.execute_started`, `registry.action.execute_succeeded`, `registry.action.execute_failed`).
    - Routed diagnostics-mode `@query` submit handling in `desktop/webview_controller.rs` through registry action execution (`phase2_execute_omnibox_node_search_action`) instead of local hardcoded dispatch.
    - Added `action.graph_view_submit` to `ActionRegistry` and routed diagnostics-mode graph-view address submit intent generation through registry execution (`phase2_execute_graph_view_submit_action`).
    - Added test-only execution helper for graph-submit actions (`phase2_execute_graph_view_submit_action_for_tests`) and scenario coverage asserting action channel emissions.
    - Added `action.detail_view_submit` to `ActionRegistry` and routed diagnostics-mode detail-view (non-live-webview) submit intent generation through registry execution (`phase2_execute_detail_view_submit_action`).
    - Added test-only detail-submit action execution helper and scenario coverage asserting action diagnostics channel emissions for focused-node detail updates.
    - Started Phase 2 input decoupling by introducing `desktop/registries/input.rs` (`InputRegistry`) with a concrete toolbar submit binding (`input.toolbar.submit` -> `action.toolbar.submit`).
    - Added Phase 2 input diagnostics contracts (`registry.input.binding_resolved`, `registry.input.binding_missing`, `registry.input.binding_conflict`) and contract tests.
    - Routed diagnostics-mode toolbar submit through registry input binding resolution in `desktop/toolbar_routing.rs` (`phase2_resolve_toolbar_submit_binding`) before submit dispatch.
    - Added toolbar nav input bindings (`input.toolbar.nav.back|forward|reload`) and routed diagnostics-mode `run_nav_action` through input binding resolution before executing webview nav operations.
    - Added generic input-binding resolution API (`phase2_resolve_input_binding`) and test helper coverage to reuse binding diagnostics across submit/nav paths.
    - Added diagnostics scenario coverage for input binding resolution channel emission (`phase2_input_registry_toolbar_submit_binding_emits_resolved_channel`).
    - Added diagnostics scenario coverage for toolbar nav binding resolution (`phase2_input_registry_toolbar_nav_binding_emits_resolved_channel`).
    - Added unit + scenario coverage for action registry behavior and action diagnostics channel emission.
    - Added `desktop/registries/lens.rs` (`LensRegistry`) with deterministic ID lookup + fallback (`lens:default`).
    - Added Phase 2 lens diagnostics contracts (`registry.lens.resolve_succeeded`, `registry.lens.resolve_failed`, `registry.lens.fallback_used`).
    - Added runtime/test lens resolution entrypoints in `desktop/registries/mod.rs` (`phase2_resolve_lens`, `phase2_resolve_lens_for_tests`).
    - Added harness scenario coverage for lens diagnostics channels (`phase2_lens_registry_default_id_emits_resolve_succeeded_channel`, `phase2_lens_registry_unknown_id_emits_failed_and_fallback_channels`).
    - Routed ID-based `GraphIntent::SetViewLens` handling through registry lens resolution with fallback (`lens:*` names resolve via registry; non-ID lens payloads remain unchanged in this slice).
    - Added atomic `LayoutRegistry`, `ThemeRegistry`, and `PhysicsRegistry` modules with deterministic `get_or_default`-style resolution semantics.
    - Refactored `LensRegistry` to hold compositional lens definitions (`layout_id`, `theme_id`, `physics_id`) instead of embedding raw profile structs.
    - Updated `phase2_resolve_lens` / `phase2_resolve_lens_for_tests` to compose final `LensConfig` via atomic registries and emit per-component lookup diagnostics.
    - Added Phase 2 diagnostics contracts for atomic lookup channels:
        - `registry.layout.lookup_succeeded|failed`, `registry.layout.fallback_used`
        - `registry.theme.lookup_succeeded|failed`, `registry.theme.fallback_used`
        - `registry.physics.lookup_succeeded|failed`, `registry.physics.fallback_used`
    - Extended harness scenarios to assert composed lens path emits layout/theme/physics lookup channels for both default and unknown lens IDs.
    - Extended `LensConfig` with optional registry ID fields (`lens_id`, `physics_id`, `layout_id`, `theme_id`) to support persisted/user-config registry references while keeping backward compatibility.
    - Updated `GraphIntent::SetViewLens` handling to prefer `lens_id` when present, then `lens:*` name routing, then explicit component-ID normalization for non-lens-ID payloads.
    - Added `phase2_resolve_lens_components` (+ test helper) to normalize explicit component IDs through atomic registries with diagnostics + fallback semantics.
    - Added unit + scenario coverage for explicit component-ID fallback normalization (`physics/layout/theme`) and resolved-ID propagation.
    - Added persisted user override source for registry IDs via existing workspace settings persistence keys (`workspace:settings-registry-{lens|physics|layout|theme}-id`).
    - Added app-level setters for persisted defaults (`set_default_registry_lens_id`, `set_default_registry_physics_id`, `set_default_registry_layout_id`, `set_default_registry_theme_id`).
    - Added app-level getters for persisted defaults (`default_registry_lens_id`, `default_registry_physics_id`, `default_registry_layout_id`, `default_registry_theme_id`) for settings UI binding.
    - Updated `SetViewLens` path to apply persisted default registry IDs when incoming payload omits IDs before routing through registry resolution.
    - Added app-level tests for persisted default roundtrip and `SetViewLens` default-ID application behavior.
    - Clarified and covered precedence behavior in app tests:
        - when persisted `lens_id` default is present, lens composition is resolved first (and can supersede persisted component defaults)
        - when persisted `lens_id` is absent, persisted component defaults (`physics/layout/theme`) are applied for missing incoming IDs
    - Added settings UI controls under `desktop/toolbar_ui.rs` (`Registry Defaults`) for Lens/Physics/Layout/Theme IDs and wired edits to persisted default setters (blank value clears persisted override).
    - Started Phase 3 verse foundation with `desktop/registries/identity.rs` (`IdentityRegistry`) providing deterministic persona resolution + sign/fallback behavior for initial keyring-contract scaffolding.
    - Added Phase 3 identity diagnostics channels and contracts:
        - `registry.identity.sign_started`
        - `registry.identity.sign_succeeded`
        - `registry.identity.sign_failed`
        - `registry.identity.key_unavailable`
    - Added runtime/test identity sign entrypoints in `desktop/registries/mod.rs` (`phase3_sign_identity_payload`, `phase3_sign_identity_payload_for_tests`) and scenario coverage for both success and key-unavailable failure paths.
    - Adapted `desktop/registries/protocol.rs` to use the existing `desktop/protocols/registry.rs` scaffold as backend.
- Validation evidence:
    - `cargo test desktop::registries:: -- --nocapture` (pass)
    - `cargo test webview_controller::tests:: -- --nocapture` (pass)
    - `cargo test desktop::tests::scenarios::registries:: -- --nocapture` (pass)
    - `cargo check` (pass)
    - `cargo check` is green again after reconciling semantic-tagging drift in `app.rs` and `desktop/registries/ontology.rs`.
    - Ontology runtime alignment is currently app-owned (`GraphBrowserApp.semantic_tags`) with registry-driven reconcile/indexing; persistence-level tag transport is tracked as follow-up work.
    - `cargo test desktop::registries:: -- --nocapture` (pass, includes lens contract tests)
    - `cargo test desktop::tests::scenarios::registries:: -- --nocapture` (pass, includes lens diagnostics scenarios)
    - `cargo test desktop::registries:: -- --nocapture` (pass, includes layout/theme/physics contract tests)
    - `cargo test desktop::tests::scenarios::registries:: -- --nocapture` (pass, includes composed lens diagnostics assertions for layout/theme/physics channels)
    - `cargo check` (pass)
    - `cargo test desktop::registries:: -- --nocapture` (pass, includes explicit component-ID normalization test)
    - `cargo test desktop::tests::scenarios::registries:: -- --nocapture` (pass, includes explicit component-ID fallback channel scenario)
    - `cargo check` (pass)
    - `cargo test test_registry_component_defaults_persist_across_restart -- --nocapture` (pass)
    - `cargo test test_set_view_lens_applies_persisted_component_defaults_when_ids_missing -- --nocapture` (pass)
    - `cargo test desktop::registries:: -- --nocapture` (pass)
    - `cargo test desktop::tests::scenarios::registries:: -- --nocapture` (pass)
    - `cargo check` (pass)
    - `cargo check` (pass, after settings UI wiring for registry defaults)
    - `cargo test desktop::tests::scenarios::registries` (pass)
    - `cargo test test_registry_component_defaults_persist_across_restart -- --nocapture` (pass)
    - `cargo test test_set_view_lens_applies_persisted_component_defaults_when_ids_missing -- --nocapture` (pass)
    - `cargo test test_set_view_lens_applies_persisted_lens_default_when_lens_id_missing -- --nocapture` (pass)
    - `cargo test desktop::registries:: -- --nocapture` (pass, includes new identity registry + phase3 diagnostics contract tests)
    - `cargo test desktop::tests::scenarios::registries -- --nocapture` (pass, includes phase3 identity diagnostics scenarios)
    - `cargo check` (pass)
    - Folded diagnostics channel contract ownership under `desktop/registries/diagnostics.rs` (`DiagnosticsRegistry`) and switched registry runtime/test callsites to diagnostics-owned phase helper APIs.
    - Removed `desktop/registries/diagnostics_contract.rs` after callsites stabilized on diagnostics-owned descriptors; `DiagnosticsRegistry` is now the sole contract source.
    - Acknowledged ontology expansion in drift cleanup: `desktop/registries/ontology.rs` now includes `validate`, `distance`, and `get_color_hint` helpers; no behavioral rollback applied in this slice.
    - `cargo test desktop::registries:: -- --nocapture` (pass, includes diagnostics ownership fold)
    - `cargo test desktop::tests::scenarios::registries -- --nocapture` (pass)
    - `cargo check` (pass)