# UxTree and UxProbe Spec

**Date**: 2026-03-01
**Status**: Canonical subsystem contract
**Priority**: Pre-renderer/WGPU required

**Related**:
- `SUBSYSTEM_UX_SEMANTICS.md`
- `../subsystem_accessibility/accessibility_interaction_and_capability_spec.md`
- `../subsystem_diagnostics/diagnostics_observability_and_harness_spec.md`
- `../../2026-03-01_automated_ux_testing_research.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **UxTree** and **UxProbe** components
of the UX Semantics subsystem.

It governs:

- UxTree construction correctness and stability
- UxNodeId uniqueness and stability rules
- UxState derivation from app state
- UxProbe registration, execution, and isolation
- UxViolation event shape and routing
- Feature flag boundaries (what ships in which build profile)

It does **not** govern:

- Test scenario execution (see `ux_scenario_and_harness_spec.md`)
- UxBridge transport (see `SUBSYSTEM_UX_SEMANTICS.md §7`)
- AccessKit bridge mapping (see `SUBSYSTEM_ACCESSIBILITY.md`)

Hierarchy note:

- Graph Bar chrome names graph-owned targets (`GraphId`, `GraphViewId`)
- the tile tree hosts contextual leaves for those targets
- UxTree must therefore project `TileKind::Graph(GraphViewId)` as a hosted graph-view surface, not as the owner of graph-view identity

---

## 2. Canonical Model

The UxTree subsystem has three distinct runtime components:

1. **UxTreeBuilder** — per-frame read-only projection of GUI state into the semantic tree.
2. **UxProbeSet** — per-frame structural invariant checkers; emit `UxViolationEvent` on failure.
3. **UxViolation routing** — `UxViolationEvent`s routed into the Diagnostics event ring.

These components must remain separate. The builder does not check invariants. The probes
do not modify app state. The routing does not depend on either.

### 2.1 Layered Payload Model (normative)

Each `UxNode` identity is represented across three payload layers that must remain
logically separate:

- **Semantic content layer** (contract-authoritative): `ux_node_id` (canonical, stable),
  `role`, `label`, `state` (`focused`, `selected`, `blocked`, `degraded`),
  `allowed_actions`, and domain identity (`GraphViewId`, `NodeKey`, tool kind).
- **Presentation layer** (non-authoritative hints): bounds/rect hints, render mode
  (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`), and z-pass/
  visual/transient flags.
- **Interaction/runtime trace layer** (non-authoritative telemetry): per-node event route,
  backend path, timing, and diagnostic counters.

The semantic layer is the only blocking contract source.
Presentation and trace layers are informative by default.

### 2.2 Canonical ID and Build Order (normative)

- A single canonical `ux_node_id` namespace spans semantic, presentation, and trace layers.
- Build order is fixed:
  1) semantic layer,
  2) presentation derivation,
  3) trace derivation.
- Hard consistency probe: every presentation `ux_node_id` must exist in the semantic layer.
- Snapshot versioning is tracked per layer (`semantic_version`, `presentation_version`, `trace_version`).
- Diff policy:
  - semantic diffs are blocking,
  - presentation diffs are informational unless explicitly promoted by the owning contract,
  - trace diffs are informational.

---

## 3. Normative Core

### 3.1 UxTree Construction Contracts

**C1 — Read-only projection**
The UxTree builder must not modify any app state, emit any intents, or produce any
side effects. It is a pure read of `Gui` and `GraphBrowserApp` state.

**C2 — Per-frame rebuild**
The UxTree is rebuilt from scratch every frame it is needed. It is not an incrementally
updated data structure. Caching is permitted within one frame only.

**C3 — Completeness under feature gate**
When `ux-semantics` is active, every visible pane (`TileKind::Graph`, `TileKind::Pane`,
`TileKind::Node`, `TileKind::Tool`) must produce at least one `UxNode` in the tree.
`TileKind::Pane` payloads are pane-only semantic surfaces; they must not be treated as
graph-enrolled node panes. `TileKind::Graph(GraphViewId)` payloads are hosted
presentations of graph-owned scoped views already named by Graph Bar chrome; they must
not be treated as owning the `GraphViewId`. Invisible or hidden tiles (outside the viewport, collapsed
by egui_tiles simplification) may be omitted.

**C4 — No partial-construction panics**
If a pane's internal state is inconsistent (e.g., a `NodePaneState` references a
`NodeKey` not present in the graph), the builder must emit a degraded `UxNode` (with
`state.degraded = true` and an empty children list) rather than panicking.

**C5 — Graph node LOD threshold**
`GraphNode` children are emitted only for graph nodes at LOD ≥ `Compact`. Nodes at
LOD `Point` are below the legible threshold and are omitted from the UxTree to avoid
tree pollution. When all nodes are at `Point` LOD (fully zoomed out), the `GraphView`
node carries a `StatusIndicator` child with label "Zoom in to interact with nodes."

**C5A — LOD transition emission parity (canvas cross-link)**
Canvas LOD policy is defined in `../canvas/graph_node_edge_interaction_spec.md §4.8`.
UxTree build output must match the active LOD tier each frame:

- `Point` tier: omit `GraphNode` children and emit the `StatusIndicator` child.
- `Compact` / `Expanded` tiers: emit `GraphNode` semantic children for interactable nodes.

Mismatch between active canvas LOD tier and UxTree emission mode must emit
`ux:navigation_violation` (or `ux:contract_warning` when degraded fallback applies).

### 3.2 UxNodeId Stability Contracts

**ID1 — Path derivation**
`UxNodeId`s are derived from stable app identity sources:
- `GraphViewId` (opaque UUID, stable across pane reorder and split)
- pane instance id (opaque UUID or equivalent, stable for the lifetime of the pane)
- `NodeKey` (u32, stable for the lifetime of the node)
- `TileId` (u64, stable within one tile tree session)
- Named dialog identifiers (string constants, not frame-local handles)
- Semantic string constants for fixed UI elements (omnibar, graph-scoped Navigator host controls, workbench-scoped Navigator host controls, status bar)

`UxNodeId`s must never be derived from:
- Raw pointer values
- `egui::Id` hashes (frame-local)
- Frame-local indices or iteration order
- `HashMap` key ordering

**ID2 — Stability across non-semantic refreshes**
A `UxNode` whose semantic identity has not changed between frame N and frame N+1 must
have the same `UxNodeId` in both snapshots. "Semantic identity" means: same pane type,
same content binding (same pane instance id, `NodeKey`, `GraphViewId`, or dialog name),
same structural role in its parent region.

**ID3 — Uniqueness within snapshot**
No two `UxNode`s in the same `UxSnapshot` may share the same `UxNodeId`. This is
invariant S7 and is also enforced during build (not only during probe execution).
If a collision is detected during build, the builder emits a `ux:structural_violation`
event and marks both colliding nodes with `state.degraded = true`.

**ID4 — Format**
Format: `uxnode://{surface}/{...path segments}`

Path segment rules:
- Surface root: `workbench`, `dialog[{name}]`, `radial-menu`, `tooltip[{id}]`
- Tile segment: `tile[{kind}:{stable-id}]` where kind is `graph`, `pane`, `node`, or `tool`
  and stable-id is the `GraphViewId` UUID, pane instance id, `NodeKey` decimal, or tool name string
- Leaf segments: kebab-case semantic names (`back-button`, `location-field`,
  `confirm-button`, `sector[{action-id}]`, `node[{key}]`)

Examples:
```
uxnode://workbench/omnibar/location-field
uxnode://workbench/graph-bar/view-target-chip
uxnode://workbench/workbench-sidebar/frame-chip[frame-0]
uxnode://workbench/tile[pane:a1b2c3d4-e5f6-...]/viewer-content
uxnode://workbench/tile[graph:a1b2c3d4-e5f6-...]/graph-canvas
uxnode://workbench/tile[graph:a1b2c3d4-e5f6-...]/graph-node[42]
uxnode://workbench/tile[node:42]/nav-bar/back-button
uxnode://workbench/tile[node:42]/nav-bar/location-field
uxnode://workbench/tile[node:42]/viewer-area
uxnode://workbench/tile[tool:diagnostics]/inspector/channel-list
uxnode://dialog[confirm-close]/confirm-button
uxnode://dialog[confirm-close]/cancel-button
uxnode://radial-menu/sector[open-in-new-tab]
```

### 3.3 UxState Derivation Contracts

`UxState` is derived from authoritative app state sources. Derivation rules:

| `UxState` field | Authoritative source |
|-----------------|---------------------|
| `enabled` | Widget-level: true unless the underlying action is gated (e.g., back button disabled when history is empty). Not inferred from visual styling. |
| `focused` | Focus subsystem: the node that holds keyboard/accessibility focus per the Focus subsystem's authority. |
| `selected` | Graph selection state (`SelectionState`): true for nodes in the current selection set. |
| `expanded` | Applicable only to expandable containers (e.g., subsystem pane sections, command palette groups). |
| `hidden` | Derived from `egui_tiles` visibility: a pane not in the current active tab of its Tab Group is hidden. |
| `blocked` | `NodeLifecycle::RuntimeBlocked`: true when the graph node's lifecycle is blocked. |
| `degraded` | `TileRenderMode::Placeholder`: true when the tile is in the fallback render mode. Also true for builder-detected inconsistent pane state (C4). |
| `loading` | WebView creation in progress: true between `MapWebviewToNode` intent and first `UrlChanged` event for the node. |

### 3.4 UxProbe Contracts

**P1 — Pure functions**
Every `UxProbe` function has signature `fn(&UxTree) -> Option<UxContractViolation>`.
It receives the tree, returns a violation or nothing. No mutation, no side effects,
no I/O, no async, no locking.

**P2 — Panic isolation**
Each probe invocation is wrapped in `std::panic::catch_unwind`. A panicking probe
emits a `ux:contract_warning` event with message "UxProbe {id} panicked: {message}"
and is disabled for the remainder of the session. Other probes continue.

**P3 — Registration**
Probes are registered at startup (not lazily). The `UxProbeSet` is immutable after
startup. Registration collects: probe ID, human-readable description, severity,
and the probe function pointer.

**P4 — Execution frequency**
Under `ux-probes`, all registered probes run every frame the UxTree is built. There
is no per-probe throttling at the framework level. Probes that are inherently expensive
should be moved to UxScenario assertions instead.

**P5 — Violation event routing**
Every `Some(UxContractViolation)` returned by a probe is converted to a
`UxViolationEvent` and routed to the Diagnostics event ring on the channel matching
the violation's `ChannelSeverity`:
- `Error` → `ux:structural_violation` or `ux:navigation_violation` (based on series)
- `Warn` → `ux:contract_warning`

**P6 — No duplicate violation flood**
Each probe emits at most one violation event per unique `(probe_id, node_path)` pair
per second. Subsequent violations within the same second increment a suppression
counter; the counter value is included in the event when suppression lifts.

---

## 4. UxRole Normative Catalogue

The following roles are normative. Extensions (for mod-contributed surfaces) must be
declared in the mod's `ModManifest` and must not shadow core role names.

### 4.1 Layout and Structure Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `Landmark` | Top-level navigable region (no interactive content itself). | `GenericContainer` + landmark flag |
| `Region` | Named sub-region within a pane. | `Section` |
| `Dialog` | Modal or non-modal dialog surface requiring explicit dismissal. | `Dialog` |
| `Toolbar` | Row of controls (omnibar, workbar, action bar). | `ToolBar` |
| `StatusBar` | Informational status strip (read-only). | `StatusBar` |
| `MenuBar` | Top-level menu container. | `MenuBar` |

### 4.2 Pane Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `GraphView` | Force-directed graph canvas. Contains `GraphNode` and `GraphEdge` children. | `ScrollView` |
| `NodePane` | Node viewer pane. Contains nav bar, viewer area, overlay sub-regions. | `Pane` |
| `ToolPane` | Tool / subsystem pane (diagnostics, history, settings, etc.). | `Pane` |
| `WorkbenchChrome` | Workbench-level chrome (non-pane UI surface). | `GenericContainer` |

### 4.3 Interactive Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `Button` | Single-action trigger. | `Button` |
| `ToggleButton` | Two-state toggle. | `CheckBox` |
| `MenuItem` | Item in a menu or command palette. | `MenuItem` |
| `RadialSector` | One sector of the radial menu. | `MenuItem` |
| `TextInput` | Editable single-line text field. | `TextInput` |
| `SearchField` | Text input semantically scoped to search/filter. | `SearchInput` |
| `OmnibarField` | The primary address/search field (combines navigation and search). | `TextInput` |
| `List` | Ordered or unordered list container. | `List` |
| `ListItem` | One item in a list. | `ListItem` |
| `Tab` | Tab selector affordance (visual; canonical term is Tile). | `Tab` |
| `TabPanel` | The content region associated with a `Tab`. | `TabPanel` |

### 4.4 Informational Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `Heading` | Section heading (read-only). | `Heading` |
| `Text` | Static text content (read-only). | `Label` |
| `Badge` | Compact node label or status indicator overlaid on a graph node. | `Label` |
| `ProgressBar` | Loading / progress indicator. | `ProgressIndicator` |
| `StatusIndicator` | Current state readout (connected, degraded, blocked, etc.). | `Label` |

### 4.5 Graph-Domain Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `GraphNode` | One node in the graph canvas. Interactive (selectable, openable). | `TreeItem` |
| `GraphEdge` | One edge between two graph nodes (if individually addressable). | `TreeItem` |
| `GraphNodeGroup` | A cluster of graph nodes (Semantic Gravity group). | `Group` |

### 4.6 Projection and Boundary Roles

| Role | Semantics | AccessKit mapping |
|------|-----------|-------------------|
| `GraphViewLensScope` | Graph-view lens/scope projection node carrying active lens/profile/filter semantics for a `GraphViewId`. | `Group` |
| `FileTreeProjection` | **Legacy alias — use `NavigatorProjection` in new code.** Workbench Navigator projection node carrying active relation-family section, sort/filter state, and row/selection expansion metrics. Maps to the workbench-scoped Navigator host section projection. | `Tree` |
| `RouteOpenBoundary` | Workbench route/open projection node carrying pending contextual-open boundary state (context target, open-node mode, connected-open scope). | `Group` |

---

## 5. UxAction Normative Catalogue

| Action | Semantics | Applicable roles |
|--------|-----------|------------------|
| `Invoke` | Primary action (click/activate). | `Button`, `ToggleButton`, `MenuItem`, `RadialSector`, `GraphNode`, `Tab` |
| `Focus` | Move keyboard/accessibility focus to this node. | All interactive roles |
| `Dismiss` | Close or cancel this surface. | `Dialog`, `RadialSector` (cancel), `MenuItem` (escape) |
| `SetValue` | Set the current value (text input, search field). | `TextInput`, `SearchField`, `OmnibarField` |
| `Open` | Open the node's content in a pane. | `GraphNode` |
| `Close` | Close the associated pane or tab. | `NodePane`, `ToolPane`, `Tab` |
| `ScrollTo` | Scroll the parent container to make this node visible. | `GraphNode`, `ListItem` |
| `Navigate` | Move semantic navigation context within a projection or route boundary without mutating graph truth ownership. | `GraphView`, `GraphViewLensScope`, `FileTreeProjection` (legacy) / `NavigatorProjection`, `RouteOpenBoundary`, `Region` |
| `Expand` | Expand a collapsed region or group. | `GraphNodeGroup`, collapsible `Region` |
| `Collapse` | Collapse an expanded region or group. | `GraphNodeGroup`, collapsible `Region` |

Actions that are not valid for the node's current state must not appear in the
node's `actions` list. Example: `Open` is absent on a `GraphNode` if its lifecycle
is `Tombstone`.

---

## 6. Invariant Reference (S/N/M Series)

Full invariant definitions are in `SUBSYSTEM_UX_SEMANTICS.md §3`. This section
provides implementation notes.

### 6.1 S1 — Label presence

**Check**: `node.label.trim().is_empty()` for roles in the interactive set.
**Builder note**: The `label` field must be populated at build time, not deferred.
If the label source is not yet available (e.g., a node title hasn't loaded), use
the URL string as a fallback. An empty label is never acceptable for interactive roles.

### 6.2 S3 — Single focus

**Check**: Count nodes with `state.focused = true`. Must be 0 or 1.
**Builder note**: Focus state is derived from the Focus subsystem's current focus
target. If the Focus subsystem reports no focus holder, all `focused` fields are
`false`. If it reports a focus holder not present in the tile tree (e.g., focus is
inside a web page), `focused` is `false` on all native UI nodes — the web content's
focus is handled by the Accessibility bridge, not the UxTree. Top-level native
chrome such as the Graph Bar may still hold focus while the active graph target is
named above the tile tree.

### 6.3 S7 — ID uniqueness

**Check**: Collect all `UxNodeId`s in a `HashSet` during build; any insert collision
is a violation.
**Builder note**: Detect and report at build time (not only at probe time).

### 6.4 N2 — Modal escape reachability

**Check**: For each open `Dialog` node, perform a BFS over the focus traversal graph
(implied by `tab_index` within the dialog subtree). Count steps to reach a node with
`UxAction::Dismiss`. Must be ≤ 10.
**Probe note**: This check involves a tree traversal; it may be skipped if no `Dialog`
node is present in the current snapshot (early return).

### 6.5 M2 — Placeholder timeout

**Check**: For each `NodePane` with `state.degraded = true` (i.e., `TileRenderMode::Placeholder`),
check if the pane has been in this state for more than 120 consecutive frames.
**Probe note**: This probe requires frame-level state across calls. The `UxProbeSet`
maintains a `HashMap<UxNodeId, u64>` of degraded-since frame counters. The counter
is reset when the node's `degraded` state clears. The counter is removed when the
node disappears from the tree.

### 6.6 Shared Modal Isolation + Focus Return Contract Table (normative)

This table is canonical and mirrored verbatim in:

- `aspect_input/input_interaction_spec.md`
- `subsystem_focus/focus_and_region_navigation_spec.md`

| Transition / surface state | Capture owner while active | Required escape path(s) | Deterministic focus return target |
|---|---|---|---|
| Modal opened from any host region | Modal surface (`Modal` context) | `Escape` or explicit dismiss action | Stored pre-modal semantic region if still valid; otherwise next valid visible region |
| Modal dismissed/confirmed | Focus router on pop from `Modal` context | Dismiss action completion | Same region/control anchor captured on modal open (or deterministic fallback as above) |
| Command palette or radial opened | Command surface (`CommandPalette` context) | `Escape`, click-away dismiss, or explicit close action | Prior semantic region/control captured at open |
| Command palette or radial dismissed | Focus router on pop from `CommandPalette` context | Dismiss action completion | Prior captured region/control; must not default to omnibar |
| Omnibar/search explicit focus acquisition | Text-entry control (`TextEntry` context) | `Escape`, explicit unfocus, or region-cycle command | Prior semantic region/control captured before text-entry capture |
| Embedded content focused | Embedded viewer (`EmbeddedContent` context) with host escape guarantee | Host-focus-reclaim binding (`Escape` or configured equivalent) | Last host semantic region before embedded capture |
| Region-cycle command (`F6`) while not modal-captured | Focus router | Repeated region-cycle / reverse cycle binding | Next/previous visible landmark in deterministic order; wraps predictably |

UxTree observability requirement: capture owner and focus-return target used by these
transitions must be inferable from semantic snapshot state (focused owner path + action
availability + modal presence), and violations must emit `ux:navigation_violation` or
`ux:contract_warning`.

---

## 7. Feature Flag Behaviour

| Feature flag | UxTree built | UxProbes run | UxBridge active | UxHarness available |
|---|---|---|---|---|
| *(none — release)* | No | No | No | No |
| `ux-semantics` | Yes | No | No | No |
| `ux-probes` | Yes | Yes | No | No |
| `ux-bridge` | Yes | No | Yes | No |
| `test-utils` | Yes | Yes | Yes | Yes |

`test-utils` implies all other UX flags. It is the flag used by the `[[test]]` binary.

In production builds with `ux-semantics` but without `ux-probes`, the UxTree is built
and available for consumption by the AccessKit bridge and the Diagnostic Inspector's
UxTree view, but no probes run. This allows the semantic tree to power OS accessibility
without paying the per-frame probe cost in shipping builds.

---

## 8. Build Performance Budget

| Metric | Target | Action on breach |
|--------|--------|-----------------|
| UxTree build latency (P95) | < 0.5 ms | Log to `ux:tree_build` Info |
| UxTree build latency (max) | < 2 ms | Emit `ux:tree_build` Warn; skip probe execution for that frame |
| UxProbe set execution latency (all probes, P95) | < 0.5 ms | Log to `ux:tree_build` Info |
| Total UX Semantics per-frame cost | < 1 ms | Emit `ux:tree_build` Warn if exceeded |

Probes exceeding their allocation are candidates for migration to UxScenario assertions
(run at explicit checkpoints, not every frame).

---

## 9. Diagnostics Channel Contracts

All channels must be declared at startup under the `ux-semantics` feature.

```
ux:structural_violation   Error    S-series hard violation or N-series hard violation
ux:navigation_violation   Error    N-series violations specifically (may overlap S)
ux:contract_warning       Warn     Any Warn-severity S/N/M invariant violation
ux:tree_build             Info     Per-build summary (node count, duration, errors)
ux:tree_snapshot_built    Info     Per-frame snapshot built counter (semantic node count payload)
ux:snapshot_written       Info     UxSnapshot written to GRAPHSHELL_UX_SNAPSHOT_PATH
ux:probe_registered       Info     UxProbe registered at startup
ux:probe_disabled         Warn     UxProbe disabled (feature gate inactive or probe panicked)
```

Channels must not be emitted when their feature gate is inactive. `ux:probe_registered`
and `ux:probe_disabled` emit at startup during probe registration, before the frame loop.

Implementation status note (2026-03-06): `ux:tree_build`, `ux:tree_snapshot_built`, and
`ux:contract_warning` are wired in the workbench UxTree build/publish path; probe lifecycle
channels (`ux:probe_registered`, `ux:probe_disabled`) are declared in diagnostics contracts and
reserved for explicit probe runtime wiring under `ux-probes` feature execution.

---

## 10. Degradation Contracts

**Builder failure**: If the UxTree builder encounters an unrecoverable error (e.g., an
unexpected `TileKind` variant with no handler), it must:
1. Return a partial tree containing only the nodes it successfully built.
2. Emit `ux:tree_build` with `error = true` and the error message.
3. Not panic. Not crash the app.

**Probe panic**: Per P2 — caught by `catch_unwind`, probe disabled, warning emitted.

**Probe violation flood**: Per P6 — rate-limited to one event per `(probe_id, node_path)`
pair per second.

**AccessKit consumer crash**: If the AccessKit bridge crashes while consuming UxTree
output, the crash is isolated to the Accessibility subsystem. The UxTree builder and
UxProbeSet continue operating.

---

## 11. Acceptance Criteria (concrete)

`AC1` — **Layer separation present**
- A frame-built snapshot contains semantic, presentation, and trace layers with independent schema versions.

`AC2` — **Canonical ID consistency**
- All presentation nodes reference `ux_node_id`s present in semantic nodes.
- Violation emits `ux:contract_warning`.

`AC3` — **Structural spine authority**
- UxTree build traverses `egui_tiles` as the structural spine.
- Semantic ownership does not depend on `egui_glow` state or APIs.

`AC4` — **Graph surface enrichment**
- Graph semantic nodes carry graph-domain identity (`GraphViewId`) and graph-surface metadata derived from app/graph surface state.
- Graph view semantic projection includes Lens/Scope state (`lens_id`, layout/physics/theme bindings, filters, dimension, fit-lock state, focused-view state).
- Workbench semantic projection includes Navigator projection state (active section, relation-family filter, sort mode, row/selection expansion counts). Legacy name: "file-tree navigation state".
- Workbench semantic projection includes route/open boundary state (pending context target, pending open-node mode, pending connected-open scope).

`AC5` — **Per-frame snapshot diagnostics**
- Each frame build emits `ux:tree_snapshot_built` with semantic node count payload.

`AC6` — **Contract gating semantics**
- Semantic-layer contract tests are blocking.
- Presentation-only diffs are non-blocking unless explicitly promoted by a domain contract.

`AC7` — **Boundary test coverage**
- At least one test proves semantic/presentation ID consistency on healthy path.
- At least one test injects a presentation-orphan node and verifies consistency probe failure.

`AC8` — **LOD semantic emission parity**
- At `Point` LOD, `GraphNode` semantic children are omitted and `StatusIndicator` is present.
- At `Compact` or `Expanded` LOD, `GraphNode` semantic children are present for interactable nodes.
- Any mismatch emits diagnostics (`ux:navigation_violation` or `ux:contract_warning`).

---

## 12. UxTree Authority Trajectory

**Added**: 2026-03-23
**Status**: Canonical — closes #272
**Gate**: Pre-renderer/WGPU required

### 12.1 Current authority model (two-authority runtime)

UxTree is a **read-only projection** built from two runtime authorities that it does not own or replace:

| Runtime authority | Owns | UxTree relationship |
|---|---|---|
| `egui_tiles::Tree<TileKind>` | Pane layout, tab grouping, tile visibility, active-tile set | Structural spine — UxTree traverses it as the walk order source |
| `GraphBrowserApp` / `GraphWorkspace` | Graph state, node selection, focus, camera, viewer registry | Semantic enrichment — UxTree reads but does not mutate |

UxTree's job is to make runtime state **contract-visible and testable**, not to become a third authority that the other two must consult.

### 12.2 Staged convergence gates

Authority expansion follows a gate sequence tied to pre-WGPU readiness milestones. Each gate has an explicit non-goal to prevent scope creep.

#### Gate G1 — Graph navigation + camera (closed)

**Done-gate**: UxTree reflects node selection and camera fit-lock state; `presentation_id_consistency_violation` passes on selection transitions.
**Non-goal**: UxTree does not drive camera state; it reflects it.
**Evidence**: `graph_navigation_*` scenarios in `pre_wgpu_critical_path.rs`.

#### Gate G2 — Pane lifecycle (closed)

**Done-gate**: Node pane open/close/focus-cycle are traceable through UxTree role changes without orphaned entries.
**Non-goal**: UxTree does not own pane ordering or tab group membership; `egui_tiles` retains layout authority.
**Evidence**: `pane_lifecycle_*` scenarios in `pre_wgpu_critical_path.rs`.

#### Gate G3 — Viewer fallback/degraded state (closed)

**Done-gate**: `TileRenderMode::Placeholder` maps to `degraded = true` in UxTree; `CompositedTexture` maps to `degraded = false`. The fallback signal is contract-visible before the WGPU switch.
**Non-goal**: UxTree does not decide which render mode a pane receives; the viewer registry retains that authority.
**Evidence**: `degraded_viewer_*` scenarios in `pre_wgpu_critical_path.rs`.

#### Gate G4 — Command surface + modal isolation (closed)

**Done-gate**: `WorkbenchIntent::ToggleCommandPalette` and `GraphIntent::ToggleCommandPalette` produce identical state; focus-cycle intents are consumed without leaking through modal boundaries.
**Non-goal**: UxTree does not mediate intent dispatch; `gui_orchestration` retains authority routing.
**Evidence**: `command_surface_*` and `modal_isolation_*` scenarios in `pre_wgpu_critical_path.rs`.

#### Gate G5 — UxProbe structural invariants (planned)

**Done-gate**: C1–C5 probe contracts emit diagnostics on violation; `ux:probe_registered` and `ux:contract_warning` channels carry actionable payloads.
**Non-goal**: Probes observe; they do not block or roll back runtime state.
**Depends on**: #255 (Phase 3: UxProbes + structural invariants).

#### Gate G6 — UxScenario CI baseline (planned)

**Done-gate**: Snapshot baseline/diff CI gate blocks merge on structural UxNode path regressions; scenario suite runs deterministically in headless mode.
**Non-goal**: Snapshot diffs are structural (semantic node count, role changes, ID consistency); they are not pixel-level screenshot comparisons.
**Depends on**: #257 (Phase 5: UxScenarios + snapshot baseline/diff CI gates).

#### Gate G7 — Critical-path coverage (closed)

**Done-gate**: All five coverage areas (graph navigation, pane lifecycle, command surface, modal isolation, degraded viewer) have committed UxHarness scenarios that pass in CI.
**Non-goal**: G7 does not authorize the WGPU switch; it is a readiness pre-condition, not the final gate.
**Evidence**: `shell/desktop/tests/scenarios/pre_wgpu_critical_path.rs` — 12 scenarios, all green.

### 12.3 Explicit non-goals for the current migration window

The following expansions are **out of scope** until post-WGPU stabilization:

1. **UxTree as layout authority** — tile ordering, tab grouping, and active-tile selection remain owned by `egui_tiles`. UxTree must not write back into tile tree state.
2. **UxTree as focus authority** — keyboard focus assignment remains owned by the Focus subsystem and `egui`. UxTree reflects focus state; it does not set it.
3. **UxTree as viewer-resolver** — viewer backend selection remains owned by the viewer registry. UxTree reflects `TileRenderMode`; it does not determine it.
4. **UxTree as intent dispatcher** — command routing remains owned by `gui_orchestration` and the `WorkbenchIntent`/`GraphIntent` reducer chain. UxTree is not an intermediate in the dispatch path.

### 12.4 Risk controls for dependency boundaries

| Dependency | Risk | Control |
|---|---|---|
| `egui_tiles` structural spine | API changes in `egui_tiles` break UxTree traversal | UxTree only uses the public `Tree::active_tiles()` + `Tiles::iter()` surface; no internal tile-tree access |
| `egui_graphs` rendering | Graph canvas state leaks into UxTree build path | UxTree reads from `GraphBrowserApp` state only; it does not call into `egui_graphs` render APIs |
| Two-authority consistency | `egui_tiles` and `GraphBrowserApp` diverge (e.g., pane exists in tile tree with no corresponding graph node) | `presentation_id_consistency_violation` detects orphaned IDs; `UxNodeState::degraded` signals broken viewer bindings |

### 12.5 Readiness criteria for post-WGPU authority expansion

UxTree authority may be expanded (e.g., to drive AccessKit, or to serve as a canonical focus-query surface) only after:

1. Gates G1–G7 are all closed.
2. Renderer switch authorization is granted (see `aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md`).
3. A dedicated spec issue scopes the authority expansion with explicit non-goals, a done-gate, and a risk register entry.

Authority expansion without these prerequisites is a migration risk and must not proceed.
