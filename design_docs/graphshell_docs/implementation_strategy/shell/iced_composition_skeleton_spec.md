<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Composition Skeleton Spec

**Date**: 2026-04-29
**Status**: Canonical / Active — first concrete S2 deliverable for the iced jump-ship plan
**Scope**: How the iced host mounts and composes Graphshell's major surfaces;
the slot model and authority assignments; the Frame split-tree rendering path;
the canvas-instance code path shared by main canvas, canvas Panes, swatches,
and the base layer; the three Navigator Presentation Bucket surfaces; the
implicit (drag-only) split-creation interaction; and the omnibar / command
palette seams.

**Related**:

- [SHELL.md](SHELL.md) — Shell domain spec (authority boundaries, what Shell owns)
- [`shell_composition_model_spec.md`](shell_composition_model_spec.md) — egui-host parallel; this spec is the iced equivalent
- [`2026-04-28_iced_jump_ship_plan.md`](2026-04-28_iced_jump_ship_plan.md) — parent plan (§3.2.1 host-neutral necessities, §4.5 Frame/Split/Pane, §4.7 Presentation Buckets, §4.8 canvas instances, §4.9 uphill rule, §12 idiomatic iced)
- [`../navigator/NAVIGATOR.md`](../navigator/NAVIGATOR.md) — Navigator domain spec; §4 uphill rule, §8 Presentation Bucket Model, §11 host model
- [`../graph/GRAPH.md`](../graph/GRAPH.md) — Graph domain spec
- [`../workbench/WORKBENCH.md`](../workbench/WORKBENCH.md) — Workbench domain spec
- [`../graph/2026-04-03_layout_variant_follow_on_plan.md`](../graph/2026-04-03_layout_variant_follow_on_plan.md) — four-tier layout model (algorithm / scene representation / simulation backend / render profile)
- [`../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — command-surface provenance / AT validation
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — canonical terms (Frame, Split, Pane, Tile, Active/Inactive, Presentation Buckets, projection vocabulary)

---

## 1. Intent and Relation to the Egui Spec

This spec is the iced parallel to [`shell_composition_model_spec.md`](shell_composition_model_spec.md).
The egui spec composes named `TopBottomPanel` / `SidePanel` / `CentralPanel`
slots with `egui_tiles::Tree<TileKind>` scoped to a `WorkbenchArea` slot.
The iced equivalent uses iced's element tree (`row!` / `column!` /
`Container` / `pane_grid`) and `canvas::Program` for graph surfaces.

The slot identities and authority assignments are the same. The widget
implementations differ. Where this spec adds canonical behavior (splits via
drag, multi-canvas via shared Program, Active/Inactive toggle UI), those
additions reflect 2026-04-29 design work (the canonical TERMINOLOGY.md
refactor and the iced jump-ship plan §4.4–§4.9), not net-new invention.

The iced host implements this spec as it lands. The egui host is frozen per
[`2026-04-28_iced_jump_ship_plan.md`](2026-04-28_iced_jump_ship_plan.md) §S1
and is not migrated to this skeleton.

---

## 1.5 Application Skeleton (the Elm Triad)

iced is The Elm Architecture: one `Application`, one `update`, one pure `view`,
plus `Subscription`s that fold async / time / winit input into the same
Message stream. The composition skeleton sits inside that triad. Every other
section in this spec assumes this shape and adds widget detail.

```rust
pub struct GraphshellApp {
    /// View-model snapshot rebuilt each tick from runtime.tick().
    /// This is NOT authoritative state — it's a frame-stable read of
    /// graphshell-runtime / graphshell-core.
    view_model: FrameViewModel,

    /// Per-Frame split-tree authority. Mutated only via Shell intents.
    frame: Frame,                          // contains pane_grid::State<Pane>

    /// Per-canvas-instance state. Keyed by stable instance id (Pane id for
    /// canvas Panes, recipe id for swatches, sentinel for main canvas /
    /// base layer). Each value is one canvas::Program::State.
    canvas_states: HashMap<CanvasInstanceId, GraphCanvasState>,

    /// CommandBar omnibar session (input draft, mode, completion mailbox).
    omnibar: OmnibarSession,

    /// Command palette modal state (open, query, filter).
    command_palette: CommandPaletteState,

    /// Per-Navigator-host UI state (scroll position, expansion, focus).
    navigator_hosts: HashMap<NavigatorHostId, NavigatorHostUi>,

    /// Theme + style tokens.
    theme: GraphshellTheme,
}

pub enum Message {
    /// Subscription tick: 60Hz frame loop. Calls runtime.tick(),
    /// folds the result into view_model.
    Tick(Instant),

    /// Subscription: graphshell-runtime emitted an event we care about.
    /// (See §9 anti-patterns: subscribe, do not poll.)
    RuntimeEvent(RuntimeEvent),

    /// Subscription: async recipe result for a swatch / canvas instance.
    RecipeResult { recipe_id: RecipeId, generation: u64, payload: RecipePayload },

    /// pane_grid drag/resize/clicked events.
    PaneGrid(pane_grid::DragEvent),
    PaneGridResize(pane_grid::ResizeEvent),
    PaneFocused(pane_grid::Pane),

    /// Tile pane tab interactions.
    ActivateTab { pane_id: PaneId, tile_id: TileId },
    CloseTile { tile_id: TileId },              // Active → Inactive

    /// Navigator interactions.
    ToggleTilePresentationState { node_key: NodeKey, graphlet_id: GraphletId },
    NavigatorRowClicked { host_id: NavigatorHostId, row_id: RowId },
    SwatchHoverEnter { recipe_id: RecipeId },
    SwatchHoverExit { recipe_id: RecipeId },
    SwatchContextAction { recipe_id: RecipeId, action: SwatchAction },

    /// CommandBar / palette.
    OmnibarInput(String),
    OmnibarSubmit,
    PaletteOpen,
    PaletteQuery(String),
    PaletteSelect(ActionId),
    PaletteClose,

    /// Authority responses (graphshell-runtime confirmed an intent).
    IntentApplied { intent_id: IntentId, result: IntentResult },

    /// Window / lifecycle.
    WindowResized(Size),
    WindowClosed,
}

impl Application for GraphshellApp {
    type Message = Message;
    type Theme  = GraphshellTheme;
    type Flags  = GraphshellRuntime;        // injected at startup

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick(_) => {
                // Single mutation point: drain runtime, fold into view_model.
                self.view_model = self.runtime.tick();
                Task::none()
            }
            Message::RuntimeEvent(e) => self.apply_runtime_event(e),
            Message::PaneGrid(drag) => self.handle_pane_drag(drag),
            Message::ToggleTilePresentationState { node_key, graphlet_id } => {
                // Emit HostIntent uphill per the iced jump-ship plan §4.9.
                self.runtime.emit(HostIntent::Lifecycle(
                    LifecycleIntent::ToggleTilePresentationState { node_key, graphlet_id }
                ));
                Task::none()
            }
            // ... one arm per Message variant; each routes uphill or
            // mutates widget-local state. Never both.
        }
    }

    fn view(&self) -> Element<'_, Message, GraphshellTheme> {
        column![
            command_bar(self),
            optional(self.has_host_top(),    || navigator_host_top(self)),
            row![
                optional(self.has_host_left(),  || navigator_host_left(self)),
                frame_split_tree_or_base_layer(self),  // always present
                optional(self.has_host_right(), || navigator_host_right(self)),
            ].height(Length::Fill),
            optional(self.has_host_bottom(), || navigator_host_bottom(self)),
            status_bar(self),
        ].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Stage A done condition: 60Hz tick is a Subscription, not
            // a per-frame poll inside view.
            time::every(Duration::from_millis(16)).map(Message::Tick),

            // Subscribe to runtime events; do not poll. (§9 anti-pattern.)
            runtime_event_stream(&self.runtime).map(Message::RuntimeEvent),

            // Per-recipe async result streams.
            recipe_result_stream(&self.runtime).map(|(id, gen, payload)| {
                Message::RecipeResult { recipe_id: id, generation: gen, payload }
            }),

            // Window events folded into Message stream.
            iced::window::events().map(window_event_to_message),
        ])
    }

    fn theme(&self) -> GraphshellTheme {
        self.theme.clone()
    }
}
```

This is the Stage A done condition (per the iced jump-ship plan §12.3).
Every section below is detail for one slot or one widget within this
shape. If a section appears to require state outside this triad, that
is a bug in the section.

### 1.5.1 Stage mapping (per [iced jump-ship plan §12.3](2026-04-28_iced_jump_ship_plan.md))

| Section | Idiomatic stage |
|---|---|
| §1.5 Application skeleton (this section) | **Stage A** — Application + Subscription closure |
| §2 Top-level composition | **Stage A** — view-tree shape |
| §3 Frame split tree | **Stage B** — `pane_grid::State<Pane>` as authority |
| §4 Pane types (tile / canvas) | **Stage B / C** — pane_grid plus per-Pane canvas Program |
| §5 Canvas instances | **Stage C** — canvas::Program with local state, multiplied for swatches |
| §6 Navigator buckets | **Stage A + C** — view-tree shape + swatch canvas instances |
| §7 CommandBar / palettes | **Stage A + E** — view-tree shape + IME via `text_input` |
| §8 Authority routing | **Stage A** — Message → HostIntent dispatch |
| §10 Open items | spans Stages B/C/D/E/F |

Stage D (`WebViewSurface` widget) and Stage F (Theme + style consolidation)
do not have dedicated sections in this skeleton spec — they have their
own deeper specs. WebViewSurface is tracked in
[`2026-04-24_iced_content_surface_scoping.md`](2026-04-24_iced_content_surface_scoping.md)
and the iced jump-ship plan §11 G5/G6. Theme consolidation lands as a
post-S4 sweep, not a skeleton concern.

### 1.5.2 Theme and animation hooks

The skeleton declares two host-level hooks that downstream Stage E/F
work consumes:

- **`GraphshellTheme`** is the `Application::Theme` — an `iced::Theme`
  extension carrying Graphshell's palette tokens, surface tokens, and
  density tokens. Each widget reads tokens from `theme()` rather than
  hardcoding. Per-widget inline styles are valid for one-off
  affordances; anything reused across two or more widgets goes into
  the Theme. `libcosmic` extension compatibility is considered when
  COSMIC DE first-class support becomes a target.
- **Animations** use [`cosmic-time`](https://crates.io/crates/cosmic-time)
  (or its iced 0.14 successor) for keyframe-driven widget animation —
  drop-zone indicator pulses, swatch hover transitions, modal
  enter/exit, omnibar mode transitions. Per-frame interpolation lives
  in `Application::update` driven by the same Tick Subscription that
  drives the runtime tick. Animation state stays widget-local where
  possible (per §9 anti-pattern: don't hoist).

These two hooks are surface-level wiring, not full Stage F / E
specifications — those are downstream sub-deliverables (per §10).

---

## 2. Top-Level Composition

The iced root `Application::view` returns one element tree per frame. The
tree's outer shape is a column with named regions, each region rendered by
a slot-specific function:

```text
┌────────────────────────────────────────────────────────────┐
│  ShellSlot::CommandBar                                     │  Shell
│  omnibar input, command palette trigger, status            │
├────────────────────────────────────────────────────────────┤
│  ShellSlot::NavigatorTop  (optional toolbar host)          │  Navigator
├──────────┬───────────────────────────────────┬─────────────┤
│  Shell   │  ShellSlot::FrameSplitTree        │  Shell      │
│  Slot::  │  (or canvas base layer fallback)  │  Slot::     │
│  Naviga- │                                   │  Naviga-    │  Navigator /
│  torLeft │  pane_grid::State<Pane>           │  torRight   │  Shell /
│  (host)  │  Each Pane is a tile pane or a    │  (host)     │  Workbench
│          │  canvas pane.                     │             │
│          │                                   │             │
├──────────┴───────────────────────────────────┴─────────────┤
│  ShellSlot::NavigatorBottom  (optional toolbar host)       │  Navigator
├────────────────────────────────────────────────────────────┤
│  ShellSlot::StatusBar                                      │  Shell
│  ambient system status, background task indicators         │
└────────────────────────────────────────────────────────────┘
```

The composition is not a fixed grid; each Navigator host slot is conditional
on its host being enabled (per [NAVIGATOR.md §11](../navigator/NAVIGATOR.md)).
The center region — `FrameSplitTree` — is always present; it shows the
Frame's `pane_grid` if any Panes exist, or the canvas base layer if not.

### 2.1 Slot definitions

| Slot | iced primitive | Authority | Contents |
|---|---|---|---|
| `CommandBar` | `Container` wrapping `text_input` + `Modal` overlay trigger | Shell | Omnibar (input + Navigator context display), command palette trigger, settings access, sync status |
| `NavigatorTop` | `Container` wrapping a horizontal Navigator host | Navigator (content) / Shell (lifecycle) | Optional toolbar-form Navigator host when anchor = Top |
| `NavigatorLeft` | `Container` wrapping a vertical Navigator host | Navigator (content) / Shell (lifecycle, resize handle) | Sidebar Navigator host when anchor = Left |
| `NavigatorRight` | `Container` wrapping a vertical Navigator host | Navigator (content) / Shell (lifecycle, resize handle) | Sidebar Navigator host when anchor = Right |
| `FrameSplitTree` | `pane_grid::PaneGrid<Pane>` over `pane_grid::State<Pane>` — or canvas base layer when state has zero Panes | Shell (composition) / Workbench (per-Pane content for tile Panes) | The Frame's split tree (Splits + Panes); Pane content comes from the relevant authority. Canvas base layer fallback when empty. |
| `NavigatorBottom` | `Container` wrapping a horizontal Navigator host | Navigator (content) / Shell (lifecycle) | Optional toolbar-form Navigator host when anchor = Bottom |
| `StatusBar` | `Container` wrapping a status `Row` | Shell | Ambient system status, process indicators, background task count |

### 2.2 Composition order

iced has no panel-declaration-order requirement (unlike egui). The element
tree is built top-to-bottom, outer to inner; iced computes layout from the
tree shape directly. The canonical assembly is:

```rust
column![
    command_bar(state),
    optional(state.host_top, navigator_host_top(state)),
    row![
        optional(state.host_left, navigator_host_left(state)),
        frame_split_tree_or_base_layer(state),  // always present
        optional(state.host_right, navigator_host_right(state)),
    ].height(Length::Fill),
    optional(state.host_bottom, navigator_host_bottom(state)),
    status_bar(state),
]
```

`optional(condition, element)` is a small helper that returns the element
when the condition is true and an empty `Container` of zero height/width
otherwise. Iced does not care about declaration order; the tree shape is
authoritative.

### 2.3 Conditional Workbench-area / WorkbenchLayerState handling

The egui spec gates `WorkbenchArea` on a `WorkbenchLayerState`
(`GraphOnly` / `GraphOverlayActive` / `WorkbenchActive` / `WorkbenchPinned`).
The iced model does not need this state machine: the Frame's split tree is
always present in `FrameSplitTree`, and:

- **Empty Frame** (no Panes) — the slot renders the canvas base layer
  directly. This is the post-iced equivalent of `GraphOnly`.
- **Frame contains Panes** — the slot renders `pane_grid`. The base layer
  is hidden behind the Panes; closing the last Pane reveals it.
- **Overlay floating Pane** (future) — when a Pane is dragged out as a
  floating overlay, the underlying base layer remains live. Overlay
  geometry is Shell-owned.

There is no `WorkbenchActive` vs `WorkbenchPinned` distinction in the iced
model; per-Pane lock state (PaneLock) covers position-locking on a per-Pane
basis without requiring a global Workbench-layer mode.

Egui-era `WorkbenchLayerState` retires alongside the rest of the
ephemeral-Pane / Promotion model.

---

## 3. The Frame Split Tree

`pane_grid::State<Pane>` *is* the Frame's split-tree authority. The Frame
struct in `graphshell-core` carries the `pane_grid::State` directly (not a
sidecar abstraction); the iced `pane_grid` widget renders it.

```rust
pub struct Frame {
    pub frame_id: FrameId,
    pub composed_workbenches: Vec<WorkbenchId>,  // 1..N per canonical Frame def
    pub split_state: pane_grid::State<Pane>,
    pub focused_pane: Option<pane_grid::Pane>,
}

pub struct Pane {
    pub pane_id: PaneId,
    pub graphlet_id: GraphletId,
    pub pane_type: PaneType,
    pub workbench_id: WorkbenchId,  // which composed workbench this Pane belongs to
}

pub enum PaneType {
    Tile,    // renders the active tiles of the Pane's graphlet
    Canvas,  // renders a canvas instance scoped to the Pane's graphlet
}
```

### 3.1 Splits via drag, not buttons (canonical)

**Splits are created implicitly by dragging, not explicitly by buttons or
menu commands.** This is a hard constraint, distinct from the egui-era
"Split horizontal" / "Split vertical" toolbar actions.

Interaction model:

1. The user grabs a tile tab (from a tile Pane's tab bar) or grabs a Pane
   chrome handle (drag origin: `pane_grid::DragEvent::Picked`).
2. As the drag moves over the `pane_grid`, iced renders a **drop-zone
   indicator** showing where a drop would land:
   - drop on the **top edge** of a target Pane → new Pane appears above
     (vertical split, dragged Pane on top)
   - drop on the **bottom edge** → new Pane below (vertical split)
   - drop on the **left edge** → new Pane to the left (horizontal split)
   - drop on the **right edge** → new Pane to the right (horizontal split)
   - drop on the **center** → join the target Pane's tab bar (no new
     split, dragged tile becomes a tab in the target Pane)
3. On release (`pane_grid::DragEvent::Dropped`), the Frame's `split_state`
   mutates to insert the new Split + Pane in the indicated position.

The split axis (Horizontal vs Vertical) is **derived from the drop edge**;
the user never chooses an axis explicitly. This matches the Zed / VSCode
drag-to-split pattern.

Reasoning: explicit Split buttons treat split creation as a top-level UI
action that interrupts flow. Drag-to-split treats it as a continuation of
the natural "I want this thing over here" gesture. The Frame's split
structure becomes an artifact of how the user arranges Panes, not a
configuration mode.

Implementation seam: `pane_grid::State::split` exists in iced; the Pane
drag-and-drop handler (`Application::update` on `pane_grid::DragEvent`)
calls it with the derived axis. No additional iced primitive is required.

Out of scope for this spec: the visual styling of the drop-zone indicator
(Pane outline pulse, edge-highlight color, animation curve). Those are
design polish, not composition skeleton.

### 3.2 Resize and close

- **Resize**: `pane_grid` resize handles are built-in; drag a Split's
  handle to adjust child Shares. No further work required.
- **Close Pane**: removing a Pane from `split_state` collapses its parent
  Split if one child remains; the surviving sibling expands. iced handles
  this when `pane_grid::State::close` is called.
- **Close last Pane in Frame**: the `pane_grid` becomes empty; the
  composition switches to canvas base layer (per §2.3).

### 3.3 Multi-Workbench frames

A Frame may compose 1..N Workbenches per canonical TERMINOLOGY.md. The
trivial case (one Workbench per Frame) is the default. The non-trivial case:

- Two or more `WorkbenchId`s appear in `Frame::composed_workbenches`.
- Each Pane carries its `workbench_id` (and therefore implicitly a
  `GraphId`).
- Different Panes in the same `pane_grid` may belong to different
  Workbenches; iced renders them all the same — `pane_grid` doesn't care
  which Workbench a Pane belongs to.
- Cross-Workbench Pane drops (dragging a Pane from one Workbench into a
  Split adjacent to a different Workbench's Pane) require the Shell to
  emit a Workbench-composition intent; this is not a `pane_grid`-level
  operation.

Cross-Workbench composition is a future capability and not required for
the first iced bring-up. The data shape supports it; the UI for
"add a Workbench to this Frame" is deferred to a later S4/S5 sub-slice.

---

## 4. Pane Types: Tile Pane vs Canvas Pane

Each Pane's `pane_type` chooses how its content renders. The two types use
different iced widget shapes.

### 4.1 Tile Pane

A tile pane renders the **active tiles** of its `graphlet_id`. Each active
tile in the graphlet appears as a tab in the Pane's tab bar; the active
tab's content renders in the Pane body.

```rust
fn tile_pane_content(pane: &Pane, view_model: &FrameViewModel) -> Element<Message> {
    let tiles = view_model.active_tiles_for(pane.graphlet_id);
    column![
        iced_aw::Tabs::new(...)
            .push(...)  // one tab per active tile
            .on_select(|idx| Message::ActivateTab { pane_id: pane.pane_id, idx }),
        active_tile_body(tiles, view_model.focused_tile),
    ].into()
}
```

- **Tab bar**: `iced_aw::Tabs` widget. One tab per Active tile in the
  graphlet (per TERMINOLOGY.md Tile Presentation States).
- **Tab content**: viewer pane (`WebViewSurface`, middlenet viewer, wry
  viewer, tool pane, etc.) per the `TileRenderMode` carried in the tile.
- **Active/Inactive toggle**: not rendered here — this is the Navigator's
  Tree Spine surface (per §6.1).
- **Close-tab affordance**: each tab carries a close `×` that emits a
  `Message::CloseTile { tile_id }`. Per TERMINOLOGY.md, closing a tile is
  the deactivate operation (Active → Inactive, presentation state); it
  does not touch graph truth.

Switching the Pane's `graphlet_id` (e.g., from a Navigator action) replaces
the tab set; tiles for the previous graphlet stay Active in any other Pane
bound to that graphlet.

### 4.2 Canvas Pane

A canvas pane renders a **canvas instance** scoped to the Pane's graphlet
(or to the full graph or a query result, depending on Pane configuration).
This uses the same `CanvasBackend<NodeKey>` implementation as the main
canvas and Navigator swatches — see §5.

```rust
fn canvas_pane_content(pane: &Pane, view_model: &FrameViewModel) -> Element<Message> {
    canvas::Canvas::new(
        GraphCanvasProgram::for_pane(pane.pane_id, pane.graphlet_id, RenderProfile::CanvasPane)
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
```

- **Camera / hover / drag state**: lives in the `Program::State` for this
  Pane's `GraphCanvasProgram` instance. Per the iced jump-ship plan
  §12.6, this state must not hoist into `Application`.
- **Pane chrome**: optional zoom-out / focus-fit / lens-picker controls
  rendered by the Pane chrome; lifecycle is Pane-local. Graph-domain
  intents (e.g., create node, create edge) emit through the uphill rule
  (per the iced jump-ship plan §4.9).

### 4.3 Switching Pane type

A Pane's `pane_type` may switch via a Pane-chrome control or a Navigator
intent. Switching does not change `graphlet_id` or any graph truth; it just
changes the render path:

- Tile → Canvas: tiles disappear, canvas instance for the same graphlet
  renders. Tile activation state is preserved (the next time the Pane
  switches back to Tile, the same active set re-renders).
- Canvas → Tile: canvas instance disappears, tab bar over active tiles
  appears.

---

## 5. Canvas Instances (One Code Path, Many Render Profiles)

All graph-canvas surfaces in iced share one `GraphCanvasProgram`
implementation of `iced::widget::canvas::Program<Message, Theme>`. They
differ in **render profile**, not in code path. Per the iced jump-ship
plan §4.8 and TERMINOLOGY.md (Projection Vocabulary § canvas instance),
the surfaces are:

| Surface | Render profile | Hosting |
|---|---|---|
| Main graph canvas | `RenderProfile::MainCanvas` | Canvas Pane in `FrameSplitTree`, full size |
| Canvas base layer | `RenderProfile::MainCanvas` | `FrameSplitTree` slot when `pane_grid` is empty |
| Canvas Pane | `RenderProfile::CanvasPane` | Pane in `FrameSplitTree`, sized by pane_grid |
| Navigator swatch | `RenderProfile::Swatch` | Navigator Swatches bucket (§6.2) |
| Expanded swatch preview | `RenderProfile::ExpandedSwatch` | Hover popover from a swatch |

`canvas::Program` is a trait you implement on a struct; `State` is an
associated type. The skeleton shape:

```rust
pub struct GraphCanvasProgram {
    pub instance_id: CanvasInstanceId,
    pub recipe_id: RecipeId,
    pub render_profile: RenderProfile,
}

pub struct GraphCanvasState {
    pub camera: Camera,                   // pan + zoom
    pub hover: Option<HoverTarget>,       // node/edge under cursor
    pub scaffold: Option<ScaffoldSelection>,
    pub viewport_pixels: Rect,
    pub cached_scene: Option<CachedScene>,
    pub cache_key: CacheKey,              // see §5.1
}

impl canvas::Program<Message, GraphshellTheme> for GraphCanvasProgram {
    type State = GraphCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        // Pointer / wheel / keyboard events mutate widget-local state;
        // when an event implies an authority change (e.g. clicking a
        // node), emit a Message that update() routes uphill.
        // ...
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &GraphshellTheme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        // Build canvas::Geometry from state.cached_scene; rebuild
        // the cache only when state.cache_key changes.
        // For Vello-backed paths, drive the shader widget separately.
        // ...
    }

    fn mouse_interaction(...) -> mouse::Interaction { ... }
}
```

The render profile parameterizes:

- per-frame draw budget (LOD threshold, off-screen culling aggressiveness)
- label rendering density
- viewer-pass aliveness for embedded content (Active vs Cold)
- gesture set (full canvas gestures vs swatch-limited subset)

`Program::State` per instance carries the per-canvas state (camera,
hover, scaffold, viewport, cached scene, cache key). Per the iced
jump-ship plan §12.6: state never lifts into `Application`. Hover on one
swatch must not invalidate other swatches.

Vello / `wgpu` rendering: the heavy graph-scene render runs through a
`shader` widget alongside (or behind) the canvas Program; the canvas
Program owns hit testing, hover affordances, and overlay drawing. The
two widgets share an instance id so their state stays paired.

### 5.1 Generation-based caching

Each canvas instance keys its cached render on the tuple:

```text
(graph_generation, recipe_id, viewport_size, theme_generation)
```

A swatch redraws only when its tuple changes. Hover and viewport gestures
mutate `Program::State` and trigger a per-instance redraw without
invalidating siblings.

`graph_generation` and `theme_generation` come from `graphshell-runtime`
via `FrameViewModel`. `recipe_id` identifies which projection recipe (per
TERMINOLOGY.md Projection Vocabulary) the swatch is rendering — a stable
identifier so that recipe-content changes (e.g., re-derived ego graphlet
membership) bump the swatch's tuple.

### 5.2 Async projection work

Some recipes (semantic clustering, Rapier settle, embedding projection,
corridor analysis) compute on background tasks. The pattern:

- `Subscription` watches a recipe queue maintained by `graphshell-runtime`.
- When the runtime emits a recipe-result event, `Application::update`
  dispatches `Message::RecipeResult { recipe_id, generation, payload }`.
- The receiving canvas instance updates its `Program::State` (or its
  cached scene) and triggers a redraw.
- Stale results (older `generation` than the canvas's current view) are
  dropped silently.

`Command::perform` is used for one-shot recipe requests; long-running
cancellable work uses iced 0.14+'s `Task` API. Per recipe, only the most
recent task is active; recipe ID swaps cancel the prior task.

---

## 6. Navigator Hosts — Three Presentation Buckets

Per [NAVIGATOR.md §8](../navigator/NAVIGATOR.md), the Navigator composes
three canonical Presentation Buckets: Tree Spine, Swatches, Activity Log.
Each Navigator host (NavigatorTop / NavigatorLeft / NavigatorRight /
NavigatorBottom) renders one, two, or all three buckets depending on its
form factor, scope, and available space (per [NAVIGATOR.md §11](../navigator/NAVIGATOR.md)).

### 6.1 Tree Spine bucket

Rendered as `lazy` + `scrollable` over a derived tree shape (frametree,
containment lens, traversal hierarchy, graphlet sections).

```rust
fn tree_spine(view_model: &FrameViewModel, host: &NavigatorHost) -> Element<Message> {
    scrollable(
        lazy(view_model.tree_spine_generation, move |_| {
            column(
                view_model.tree_spine_rows().map(|row| tree_spine_row(row))
            )
        })
    ).into()
}
```

Each row is one of:

- **Frametree node** — a Workbench / Pane / Split entry; click selects/focuses
- **Containment node** — a graphlet section; click expands or focuses
- **Graphlet anchor** — a tile (graph node); click activates/deactivates
- **Lens-driven row** — per `ProjectionLens` variant (Traversal, Arrangement, Containment, Recency)

**Activate/Deactivate UI** for tiles in a graphlet lives here. Each tile row
has a small on/off control:

```text
┌────────────────────────────────────────────────────┐
│  ▼ Graphlet: research-2026-04                     │
│      ●  example.com/intro            [tile shown] │  ← Active tile
│      ○  example.com/related         [tile hidden] │  ← Inactive tile
│      ●  example.com/discussion       [tile shown] │  ← Active tile
│      ○  arxiv.org/2401.00001        [tile hidden] │  ← Inactive tile
└────────────────────────────────────────────────────┘
```

The toggle dispatches `Message::ToggleTilePresentationState { node_key, graphlet_id }`,
which routes uphill to the runtime lifecycle authority per the iced jump-ship
plan §4.9. No graph truth changes; only the per-graphlet presentation
state flips.

### 6.2 Swatches bucket

A virtualized grid of compact `canvas::Program` instances (the same
`GraphCanvasProgram` from §5, with `RenderProfile::Swatch`). One instance
per recipe.

```rust
fn swatches(view_model: &FrameViewModel, host: &NavigatorHost) -> Element<Message> {
    let visible_recipes = view_model.swatches_in_viewport(host.host_id);
    scrollable(
        wrap_horizontally(
            visible_recipes.iter().map(|recipe| swatch_card(recipe))
        )
    ).into()
}

fn swatch_card(recipe: &SwatchRecipe) -> Element<Message> {
    container(
        canvas::Canvas::new(GraphCanvasProgram::for_swatch(recipe.recipe_id, RenderProfile::Swatch))
            .width(Length::Fixed(SWATCH_WIDTH))
            .height(Length::Fixed(SWATCH_HEIGHT))
    )
    .style(swatch_card_style)
    .into()
}
```

- **Virtualization**: only swatches in the visible viewport region are
  rendered; off-screen swatches are not constructed. The `lazy` widget
  bounds tracking gives the visible-viewport list.
- **Per-instance state**: each swatch carries its own `Program::State`
  (camera, hover, scaffold, viewport).
- **Hover preview**: on hover, a swatch may emit
  `Message::SwatchHoverEnter { recipe_id }` which Shell uses to render an
  expanded swatch as a popover (`Modal` + `Stack`) at higher fidelity
  (`RenderProfile::ExpandedSwatch`).
- **Swatch actions**: right-click opens a context menu (`iced_aw::ContextMenu`)
  with Promote / Pin / Open-as-Pane / Save-Recipe actions. These dispatch
  uphill intents per the iced jump-ship plan §4.9.

### 6.3 Activity Log bucket

A `lazy` + `scrollable` over an event stream from `graphshell-runtime` and
SUBSYSTEM_HISTORY.

```rust
fn activity_log(view_model: &FrameViewModel, host: &NavigatorHost) -> Element<Message> {
    scrollable(
        lazy(view_model.activity_log_generation, |_| {
            column(view_model.activity_events().map(|event| activity_row(event)))
        })
    ).into()
}
```

Event types: lifecycle transitions (Active/Warm/Cold), navigation events,
graph mutations, import events, frame-snapshot saves. Each event row is
clickable: clicking navigates to the relevant node / Pane / graphlet.

`Subscription` on `graphshell-runtime`'s event channel keeps the log
warmed; `Application::update` appends new events to the view-model and
triggers a redraw.

---

## 7. CommandBar and Command Surfaces

### 7.1 CommandBar

The CommandBar is a `Container` wrapping a horizontal row:

```rust
fn command_bar(state: &State) -> Element<Message> {
    container(
        row![
            navigator_breadcrumb(state.navigator_context),  // Navigator-projected, read-only
            text_input(state.omnibar_session.draft.as_str(), Message::OmnibarInput)
                .on_submit(Message::OmnibarSubmit),
            command_palette_trigger_button(),
            settings_access_button(),
            sync_status_indicator(),
        ].spacing(8)
    )
    .padding(4)
    .into()
}
```

Authority per [shell_composition_model_spec.md §5](shell_composition_model_spec.md):

- **Shell** owns the `text_input` widget, focus, mode (Display/Input), and
  command dispatch.
- **Navigator** contributes the breadcrumb / scope-badge / graphlet-label
  via `NavigatorContextProjection` (read-only).
- The seam is the struct, not who renders what.

iced specifics:

- `text_input` (iced 0.14) is IME-aware; CJK/Arabic input works without
  additional glue.
- Focus moves via `widget::focus()` `Operation`s (not via a separate
  focus model). Per-widget focus lives in iced; cross-surface focus
  coordination lives in `graphshell-runtime` (per iced jump-ship plan G1
  / Stage A).
- Background completion fetch uses a `Subscription` over a
  Shell-supervised mailbox (per [shell_composition_model_spec.md §5.3](shell_composition_model_spec.md)).

### 7.2 Command Palette

A `Modal` overlay triggered by `Ctrl+P` or the CommandBar trigger button:

```rust
fn command_palette_overlay(state: &State) -> Option<Element<Message>> {
    state.command_palette.is_open.then(|| {
        modal(
            column![
                text_input(state.command_palette.query.as_str(), Message::PaletteQuery),
                scrollable(
                    column(state.command_palette.filtered_actions().map(|a| action_row(a)))
                ),
            ]
        ).into()
    })
}
```

- **Action source**: `ActionRegistry` (atomic registry, per TERMINOLOGY.md);
  Shell dispatches selected actions via `HostIntent`.
- **Filtering**: in-Shell substring + score filter; no background work.
- **Closing**: Escape, click outside, or action selection.

### 7.3 Context Palette

`iced_aw::ContextMenu` triggered on mouse-right anywhere a target is
identifiable. Targets and their available actions:

| Target | Available actions (representative) |
|---|---|
| Tile (in tile pane tab) | Activate / Close / Remove from graphlet / Pin / Tombstone (with confirmation) |
| Canvas node (canvas Pane / main canvas / swatch) | Activate / Open in Pane / Pin / Inspect / Remove from graphlet / Tombstone |
| Edge (canvas surface) | Inspect / Hide in this view / Remove edge |
| Frame border / Split handle | Resize / Reset proportions / Add Pane via drop |
| Navigator row (Tree Spine) | Activate / Deactivate / Reveal in canvas / Open as Pane |
| Activity Log entry | Open referenced node / Open referenced graphlet |
| Swatch | Promote (open as Pane) / Pin / Save recipe / Inspect |
| Empty FrameSplitTree (canvas base layer) | Open Pane (drag a node here also works) / Switch graphlet |

Each action emits a `HostIntent` per the uphill-rule routing in the iced
jump-ship plan §4.9.

### 7.4 Radial Palette

A custom `canvas::Program` overlay for positional radial-menu invocation
(typically gamepad / touch / long-press). Radial geometry is not a
built-in iced widget; the program draws sectors and routes hits to actions
via the same `ActionRegistry` as the command and context palettes.

The radial palette is deferred until the gamepad / input rework lands
(per the iced jump-ship plan, gamepad bindings retired pending Graphshell-native
input design). Spec stub only at this point.

---

## 8. Authority and Uphill-Rule Routing

Every `HostIntent` emitted from an iced widget routes uphill per
[the iced jump-ship plan §4.9](2026-04-28_iced_jump_ship_plan.md) and
[NAVIGATOR.md §4](../navigator/NAVIGATOR.md). The composition skeleton's
contribution is to make sure no widget owns domain state directly.

| State location | What lives there |
|---|---|
| `pane_grid::State<Pane>` (in `Frame`) | Frame split-tree topology; Shell-owned; mutated via Shell intents |
| `canvas::Program::State` (per canvas instance) | Camera, hover, scaffold, viewport; widget-local |
| `text_input` state (in CommandBar / palette) | Draft text; widget-local; submitted via Shell intents |
| `iced_aw::Tabs` state (in tile Panes) | Active-tab index; widget-local; selection routes via Shell |
| `Application::State` | View-model snapshot from `runtime.tick()`; not authoritative graph/workbench/shell state |
| `graphshell-runtime` / `graphshell-core` | All authoritative state |

Test for whether a piece of state belongs in iced widget code:

> If the state survives a window close, it's runtime. If it dies with the
> widget, it's widget-local.

(Per the iced jump-ship plan §4.9.)

---

## 9. Anti-patterns Specific to the Composition Skeleton

In addition to the iced jump-ship plan §5 anti-patterns:

- **Don't hand-roll a split tree.** Use `pane_grid::State<Pane>`. A
  side-structure that mirrors the split tree means two sources of truth.
- **Don't add explicit Split-direction buttons.** The split direction is
  derived from the drop edge per §3.1. A "Split horizontal" button would
  rebuild the egui-era model in iced shape — exactly the failure mode
  this spec rejects.
- **Don't render swatches as static images.** Swatches are live canvas
  instances; static thumbnails are a downgrade.
- **Don't share `Program::State` between canvas instances.** Each
  instance owns its own state; sharing breaks per-instance hover and
  invalidation.
- **Don't put per-Pane camera state in `Application`.** Per the iced jump-ship
  plan §12.6, camera lives in `Program::State`.
- **Don't bypass `pane_grid` for the FrameSplitTree slot.** Custom layout
  for Splits would mean re-implementing what iced gives for free, with
  worse focus-routing and resize semantics.
- **Don't render the canvas base layer as an empty Pane.** The base
  layer is the empty-Frame fallback path; it's a different code branch in
  the composition tree, not a degenerate Pane.
- **Subscribe to runtime events; don't poll inside `view`.** Per the iced
  jump-ship plan §12.6, `view` runs every frame, but the runtime's event
  stream should drive `update` only on real changes. Per-frame polling on
  top of the 60Hz tick Subscription is doubly redundant. Use one
  Subscription per event source (graph mutations, lifecycle transitions,
  recipe results, history events) and let `view` consume the resulting
  view-model.
- **Don't replicate `egui_tiles::Tabs` semantics by hand.** Use
  `iced_aw::Tabs` inside tile Panes. Tab grouping is orthogonal to split
  layout; egui_tiles conflated them. Re-conflating in iced is the
  failure mode.
- **Don't manage per-widget focus from `Application`.** Use
  `widget::focus()` / `widget::Operation` to move focus declaratively.
  Cross-surface focus coordination (the six-track focus model from the
  iced jump-ship plan §11 G1) lives in `graphshell-runtime`; widgets read
  it from `FrameViewModel` and apply via Operations.

---

## 10. Open Items (S2 Follow-Ups)

Items this spec leaves to subsequent S2 deliverables:

- **Omnibar shape detail** (per S2 checklist): exact widget composition,
  completion list shape, mode-transition animation.
- **Browser amenities per surface** (per S2 checklist): each amenity from
  the iced jump-ship plan §4.6 needs its own specification — what surface,
  what data, what intent flow, what `verso://` address.
- ~~**Graph coherence guarantee per surface**~~ — landed 2026-04-29 in
  the iced jump-ship plan §4.10. Twelve surface guarantees covering
  tile pane / canvas pane / canvas base layer / three Navigator buckets
  / omnibar / command palette / context palette / frame switcher /
  settings panes / tool panes / WebViewSurface / drag-to-split drop
  zone.
- **Frametree visualization in Tree Spine**: how the frametree recipe
  renders inside the Tree Spine bucket — collapsible Workbench groups,
  Pane indicators, active-Pane highlight.
- **Frame composition UI**: how a user adds or removes a Workbench from a
  Frame (drag from a "switch workbench" picker? from a Frame-chrome menu?
  from the Activity Log?). Deferred per §3.3.
- **Cross-Workbench Pane drop**: the policy and UX for dragging a Pane
  from one Workbench's region into a Split adjacent to a different
  Workbench's Panes. Deferred per §3.3.
- **Drop-zone indicator design polish**: visual styling, animation, and
  accessibility (focus / keyboard equivalents for drop-zone selection)
  for the drag-to-split interaction in §3.1.
- **Keyboard equivalents for drag-to-split**: while drag is the primary
  path, keyboard accessibility likely needs Pane-move shortcuts (e.g.,
  `Ctrl+Alt+Arrow` to send the focused Pane to a Split direction).
  Deferred to the input subsystem.

These are S2 sub-deliverables, not blockers for this skeleton spec.

---

## 11. Bottom Line

The iced composition skeleton is the Elm triad — one `Application`,
one `update`, one `view`, plus `Subscription`s for tick / runtime
events / async results — instantiated for Graphshell. The Application
holds a `Frame` (whose `pane_grid::State<Pane>` is the split-tree
authority) and a per-canvas-instance state map. The `view` returns a
column-of-rows tree composing the seven slots. One
`GraphCanvasProgram` implementing `canvas::Program` is shared across
main canvas, canvas Panes, swatches, and the canvas base layer,
parameterized by render profile. Splits are created by drag, never by
buttons. Three Navigator Presentation Buckets render through `lazy` +
`scrollable` for tree spine and activity log, plus a virtualized grid
of canvas instances for swatches. Per-instance widget state stays
widget-local; runtime events flow through Subscriptions, not polling;
everything authoritative lives in `graphshell-runtime`.

This skeleton is the Stage A done condition (per the iced jump-ship
plan §12.3) plus the Stage B / C anchors for downstream surface work.
It is the slot-and-authority anchor for S3 (host runtime
closure) and S4 (per-surface bring-up). It is small enough to implement
incrementally and complete enough to evaluate the iced host against the
host-neutral necessities in the iced jump-ship plan §3.2.1.
