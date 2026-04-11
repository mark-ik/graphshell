# Scene Mode UX Plan (2026-04-02)

**Status**: In progress
**Goal**: Introduce a user-facing `Browse / Arrange / Simulate` scene-mode model for graph views, with explicit interaction contracts and persistence boundaries that fit Graphshell's graph/view/scene/style architecture.

**Relates to**:

- `../../research/2026-04-02_scene_mode_ux_sketch.md`
- `../../research/scene_customization.md`
- `2026-04-02_parry2d_scene_enrichment_plan.md`
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`
- `graph_node_edge_interaction_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `multi_view_pane_spec.md`
- `../system/register/canvas_registry_spec.md`

---

## 1. Purpose

Graphshell needs a user-facing scene model, not just deeper internal layout machinery.

This plan defines the first explicit mode structure for graph-scene interaction:

- `Browse`
- `Arrange`
- `Simulate`

These are not separate graph types. They are different interaction and projection modes over the same graph truth.

The purpose of this plan is to make that distinction explicit in runtime state, interaction contracts, and persistence boundaries before scene behavior grows further.

**Architecture alignment (2026-04-10)**:

- the canonical multi-layer scene substrate is now defined in
  `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`,
- `Browse`, `Arrange`, and `Simulate` are the user-facing modes over that
  shared scene substrate,
- `Browse` and `Arrange` target the Vello world-render path plus Parry-backed
  query/editor geometry without requiring live rigid-body simulation,
- `Simulate` is the only scene mode that activates Rapier,
- node-avatar presets, scene props, triggers, routes, scene packages, and
  Wasmtime-backed scene objects are follow-on capabilities inside this same
  architecture rather than separate parallel tracks.

**Execution update (2026-04-02)**:

- `SceneMode` now exists as persisted per-view state on `GraphViewState`.
- Graph-facing UI now exposes a first scene-mode switch (`Browse`, `Arrange`, `Simulate`) in both the graph overlay and the summoned `Scene` surface.
- `Arrange` is now the first mode with concrete behavioral effect:
  - scene authoring affordances are foregrounded,
  - on-canvas authored-region interactions are enabled,
  - selected regions now carry a small canvas-local action strip for quick gather commands,
  - the floating `Scene` surface behaves as the richer editing companion for the active graph view.
- `Arrange` also now carries the first `Gather Here` interaction:
  - selected regions can gather the current selection,
  - selected regions can gather the current projection-aware graphlet,
  - selected regions can gather by selection-derived classification, tag, domain, or frame candidates,
  - selected regions can gather the active graph search result set,
  - selected regions can gather the current filtered-view node set.
- `Browse` remains intentionally quiet, while `Simulate` now has a first concrete legibility slice:
  - `Browse` keeps authoring affordances subdued,
  - `Simulate` now exposes `Reveal Nodes` and `Relation X-Ray` as per-view scene controls,
  - `Simulate` now also exposes `Float`, `Packed`, and `Magnetic` as per-view behavior presets that tune the existing scene-runtime pass,
  - `Float` now glides longer, settles softly, and responds more loosely to bounds; `Packed` settles quickly with firmer personal space and stronger boundary response; `Magnetic` sits in the middle while making regions feel more assertive,
  - dragged node-objects in `Simulate` now retain a short decaying release impulse after pointer release so they coast and settle instead of stopping dead,
  - richer object-world behavior beyond those overlays, preset biases, and release coasting is still future work.

---

## 2. Canonical Mode Model

### 2.1 Scene mode enum

Add a per-`GraphViewId` scene mode concept:

```rust
enum SceneMode {
    Browse,
    Arrange,
    Simulate,
}
```

This mode is view-owned state, not graph-canonical state.

### 2.2 Mode semantics

#### Browse

- calm graph browsing is primary
- edges are subdued or hidden unless useful
- selection and peek interactions are emphasized
- authored scene controls are present but not foregrounded
- no Rapier world allocation is required

#### Arrange

- semantic spatial composition is primary
- region creation and gather/sort actions are foregrounded
- soft scene behaviors are available
- users can deliberately shape space without full simulation complexity
- Parry/Vello-backed scene composition is available without requiring live
  rigid-body simulation

#### Simulate

- object-world behavior is primary
- richer scene rules are available
- relationship overlays are demand-driven
- the canvas behaves as a scene without losing graph explainability
- per-view behavior presets can bias the scene feel without changing graph truth
- Rapier is the live physics world for this mode
- node-avatar presets, scene props, triggers, and routes are the intended
  object-world extension surface for this mode

---

## 3. State and Persistence Boundaries

### 3.1 State ownership

`SceneMode` belongs to per-view state.

It should live adjacent to other `GraphViewId`-scoped carriers such as:

- camera state,
- active layout selection,
- lens selection,
- runtime scene overlay state.

### 3.2 Persistence rule

Persist `SceneMode` with graph-view state in snapshots once the mode is user-visible and stable.

Rationale:

- it is part of how a view is experienced,
- it is not graph truth,
- it should round-trip with the rest of view interpretation.

### 3.3 Separation rule

Persisting `SceneMode` does **not** imply persisting all scene runtime details.

First boundary:

- `SceneMode` may persist
- ephemeral scene runtime may remain non-persisted
- richer scene-overlay persistence is a separate later decision

---

## 4. Interaction Contracts

### 4.1 Shared across all modes

All modes must preserve:

- node selection,
- node activation,
- canvas pan/zoom,
- graph search/highlight,
- graph-view routing semantics,
- frame/workbench authority boundaries.

### 4.2 Browse interactions

Required first-class actions:

- select node
- activate node
- peek relations for selected/hovered node
- reveal graphlet / neighborhood
- temporarily surface relation families
- switch scene/layout preset

Expected visual result:

- low clutter,
- calm motion,
- demand-driven relationship visibility.

### 4.3 Arrange interactions

Required first-class actions:

- create region
- label region
- move region
- gather nodes into region by semantic selector
- create anchor / attractor
- pin or unpin structural nodes
- separate nodes/groups spatially

Expected visual result:

- regions feel understandable and soft,
- arrangement actions produce obvious, explainable spatial change,
- the graph feels increasingly user-shaped rather than purely algorithm-shaped.

### 4.4 Simulate interactions

Required first-class actions:

- drag object and release into scene
- reveal nodes when scene richness obscures them
- semantic x-ray / relation reveal overlay
- toggle object-vs-graph emphasis
- choose a simulate behavior preset such as `Float`, `Packed`, or `Magnetic`
- let released objects coast briefly before settling

Expected visual result:

- scene remains intelligible,
- graph structure is still recoverable on demand,
- simulation reinforces semantic organization rather than obscuring it.

---

## 5. Overlay and Affordance Model

### 5.1 Peek Relations

Add a scoped overlay behavior:

- reveal relationships for the current node/object,
- fade back out when focus changes,
- do not permanently switch the whole canvas into "all edges visible" mode.

### 5.2 Reveal Nodes

Add a scene-legibility affordance:

- temporarily outline or halo all graph node-objects,
- visually separate them from regions, labels, and decorative scene affordances.

This is especially important in `Simulate`.

### 5.3 Gather Here

Treat semantic gather/sort actions as a first-class mode action rather than a later convenience:

- gather selected graphlet here,
- gather nodes by tag here,
- gather nodes by frame/domain/semantic class here.

This is the key interaction that makes `Arrange` valuable as a thinking tool rather than mere visual styling.

### 5.4 Semantic X-Ray

Add a mode-safe relation reveal overlay:

- highlights graph structure without leaving the current scene,
- supports busy scenes where edges should not remain permanently visible.

---

## 6. UI Surface Plan

### 6.1 Scene mode switch

Expose a small scene-mode control in graph-view chrome:

- `Browse`
- `Arrange`
- `Simulate`

This should be a view-level control, not a global app mode.

### 6.2 Scene actions cluster

When the mode is `Arrange` or `Simulate`, foreground relevant actions in graph chrome or a scene palette:

- add region
- gather here
- add attractor
- reveal nodes
- x-ray relations
- choose scene preset
- choose simulate behavior preset

### 6.3 Progressive disclosure

Do not surface the full scene toolbox in `Browse`.

The UI should feel:

- lightweight in `Browse`,
- authorable in `Arrange`,
- richer in `Simulate`.

---

## 7. First Implementation Slice

The first execution slice should be intentionally small:

1. add `SceneMode` to per-view state,
2. add a simple scene-mode switch in graph-view chrome,
3. add `Peek Relations` as a view-safe overlay behavior,
4. add `Reveal Nodes` as a scene-legibility overlay,
5. add the `Gather Here` interaction contract in plan form, even if only partially wired at first,
6. wire `Arrange` mode to the runtime scene-region system from the `parry2d` plan once available.

This slice does **not** require:

- full scene persistence,
- Rapier,
- a scene editor,
- or major render-path replacement.

**Execution update (2026-04-02, later)**:

The initial mode slice is now materially beyond scaffolding:

1. `SceneMode` persists per view,
2. `Arrange` owns authored-region interaction and semantic gather actions,
3. `Simulate` owns `Reveal Nodes` and `Relation X-Ray`,
4. `Simulate` also owns first behavior presets (`Float`, `Packed`, `Magnetic`) that bias scene-runtime separation, containment feel, and region response per view,
5. `Simulate` now gives dragged node-objects a short decaying release coast so object motion feels less abruptly halted.

---

## 8. Acceptance Criteria

The mode model is established when:

- every `GraphViewId` can carry an explicit `SceneMode`,
- `Browse`, `Arrange`, and `Simulate` are user-visible view states,
- relationship peeking is demand-driven rather than all-or-nothing,
- node visibility can be clarified in busy scenes via `Reveal Nodes`,
- `Simulate` exposes at least one concrete behavior-control surface beyond overlays,
- arrange-oriented commands exist conceptually as scene actions rather than hidden future behavior,
- the graph remains canonical and none of the new mode logic becomes graph truth.

**Shared acceptance shape with the scene/projection architecture plan**:

- `Browse` and `Arrange` do not require Rapier world allocation,
- `Simulate` enables Rapier behaviors without mutating graph topology,
- scene composition roundtrips through view snapshots while derived runtime
  state does not,
- scene scripts operate only through explicit Wasmtime-backed
  capabilities/events.

---

## 9. Follow-On Boundary

This plan intentionally stops before heavier scene behavior.

After the first mode slice lands, the next follow-on decisions are:

- how much of `Arrange` should become persistable scene overlay state,
- when `parry2d` runtime scene regions become user-authored UI,
- how much farther `Simulate` should be pushed with `parry2d`-backed behavior before introducing `rapier2d`,
- whether `Simulate` eventually becomes the entry point to a future `rapier2d` scene mode.
