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
graph-enrolled node panes. Invisible or hidden tiles (outside the viewport, collapsed
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

### 3.2 UxNodeId Stability Contracts

**ID1 — Path derivation**
`UxNodeId`s are derived from stable app identity sources:
- `GraphViewId` (opaque UUID, stable across pane reorder and split)
- pane instance id (opaque UUID or equivalent, stable for the lifetime of the pane)
- `NodeKey` (u32, stable for the lifetime of the node)
- `TileId` (u64, stable within one tile tree session)
- Named dialog identifiers (string constants, not frame-local handles)
- Semantic string constants for fixed UI elements (omnibar, workbar, status bar)

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
uxnode://workbench/workbar/tab[frame-0]
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
focus is handled by the Accessibility bridge, not the UxTree.

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

`AC5` — **Per-frame snapshot diagnostics**
- Each frame build emits `ux:tree_snapshot_built` with semantic node count payload.

`AC6` — **Contract gating semantics**
- Semantic-layer contract tests are blocking.
- Presentation-only diffs are non-blocking unless explicitly promoted by a domain contract.

`AC7` — **Boundary test coverage**
- At least one test proves semantic/presentation ID consistency on healthy path.
- At least one test injects a presentation-orphan node and verifies consistency probe failure.
