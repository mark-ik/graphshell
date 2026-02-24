# Graph Interaction Consistency Plan (2026-02-23)

**Status**: Draft
**Supersedes**: Prior ad-hoc zoom/scroll patches in `render/mod.rs`; absorbs remaining items from `2026-02-19_graph_ux_polish_plan.md` §1.4 (scroll zoom speed) and the "smart fit" + "no-ctrl scroll" feature targets.

---

## Problem Statement

Three categories of UX inconsistency:

1. **Graph navigation is unreliable.** Scroll-to-zoom without Ctrl doesn't work. Fit-to-screen (Z/C keys) doesn't fire. Startup zoom has no visible effect. Multiple iterations have failed because the root cause — input ownership and event routing — was never addressed; patches targeted render-time helpers that execute too late or against stale state.

2. **Tile tree operations are semantically under-explained.** "Horizontal" and "Vertical" appear in tab strips because they are real `Container::Linear` nodes in the tile tree. They can be useful (they expose split structure), but today they lack context and naming guidance, so users interpret them as bugs. The relationship between Graph/WebView panes, container nodes, and Workbench structure is still opaque.

3. **Nodes drift off-screen.** A single node with no edges has no mutual-stabilizing forces. The center gravity locus is fixed at graph-space origin (0,0), not at the viewport center. After panning, gravity pulls nodes away from where the user is looking, and a lone node floats off the visible area.

---

## Terminology Corrections

Per the TERMINOLOGY.md living document, the following renames apply in code comments, logs, and UI strings:

| Old term | Canonical term | Notes |
|---|---|---|
| "fit to screen" | **Camera Fit** | Fits viewport to node bounds. Avoids confusion with display/fullscreen. |
| "zoom to selected" | **Focus Selection** | Fits viewport to selected-node bounds. |
| "scroll zoom" | **Wheel Zoom** | Covers mouse wheel, trackpad scroll, and smooth-scroll. |
| "Horizontal" / "Vertical" (tile containers) | **Split** | User-facing label for `Container::Linear`. Internal code may keep `Linear`. |
| "graph_surface_focused" | **Graph Pane Focused** | Aligns with Pane terminology. |

**Action**: Update `TERMINOLOGY.md` with Camera Fit, Focus Selection, Wheel Zoom, Split.

---

## Root Cause Analysis

### Why scroll-to-zoom without Ctrl doesn't work

egui_graphs `SettingsNavigation::with_zoom_and_pan_enabled(true)` registers an `InputState` callback that consumes scroll events when Ctrl is held. When we set `with_zoom_and_pan_enabled(false)`, egui_graphs stops consuming scroll events — but egui's `ScrollArea` or parent `Ui` widgets may still interpret them as scroll/pan. Our post-render `handle_custom_navigation` reads `smooth_scroll_delta` / `raw_scroll_delta`, but by the time it runs, the scroll events may have been consumed by egui's own scroll handling earlier in the frame.

**Fix**: Intercept scroll events *before* `GraphView` renders by injecting a `ui.input_mut()` call that converts scroll deltas into zoom state, or by using `ui.interact()` with a `Sense::hover()` on the graph rect to claim the input.

### Why fit-to-screen doesn't fire

The custom `apply_pending_fit_to_screen_request` reads `app.fit_to_screen_requested`, but the flag is consumed by `take_pending_fit_to_screen_request()` which was called in an earlier code path. Additionally, the flag must survive until the `MetadataFrame` is available in egui's persisted data — on the first frame after graph init, it may not exist yet.

**Fix**: Use a two-phase approach: set a durable flag that persists across frames until successfully applied, and only clear it after confirming the MetadataFrame write succeeded.

### Why startup zoom has no effect

`pending_initial_zoom` is set in the constructor, but `apply_pending_initial_zoom` fires before the `MetadataFrame` is populated by egui_graphs on its first layout pass. The zoom is attempted, finds no MetadataFrame, does nothing, and the flag is never retried.

**Fix**: Same durable-flag pattern. Additionally, startup should trigger Camera Fit instead of a fixed zoom value, since a fixed zoom can't account for the number or spread of nodes.

### Why nodes drift off-screen

The FR center gravity force (`state.extras.0.params.c = 0.18`) pulls toward graph-space origin `(0, 0)`. After the user pans, the viewport center diverges from `(0, 0)`. Nodes with weak or no edge forces get pulled toward graph-space origin, which is now off-screen.

**Fix**: Update the gravity locus to track the viewport center in graph space. This means the gravity target shifts as the user pans, keeping nodes attracted toward what the user is actually looking at.

### Why tile operations feel confusing

egui_tiles exposes `Container::Linear(LinearLayout { dir: LinearDir::Horizontal | Vertical })` as a visible tab title when the container appears in the tab strip. This is architecturally correct: container tiles are first-class nodes that can appear anywhere tabs can appear. In Graphshell, `all_panes_must_have_tabs: true` intentionally wraps panes in `Tabs`, so split/merge flows often surface container nodes. The current rendering path falls through to `format!("{:?}", container.kind())` without structural cues, so valid architecture is presented with ambiguous UX.

**Fix**: Keep container visibility, but make it explicit and teachable. Override `tab_title_for_tile` (not just `tab_title_for_pane`) to render semantic labels and optionally directional glyphs (e.g., `Split ↔`, `Split ↕`, `Tabs`, `Grid`) plus lightweight affordances that explain what selecting that container means.

---

## Implementation Phases

### Phase 1: Input Ownership (fixes scroll-to-zoom)

**Goal**: Scroll wheel over graph pane = zoom. No Ctrl required. Configurable.

**Approach**: Pre-render input interception.

1. Before `GraphView::new()` renders, call `ui.input_mut(|i| ...)` to:
   - Read `smooth_scroll_delta.y` and `raw_scroll_delta.y`.
   - If the graph rect is hovered (check via stored response or `ui.rect_contains_pointer`), zero out the scroll deltas so egui/egui_graphs won't interpret them as scroll.
   - Store the consumed scroll delta in an app-owned field (`app.pending_wheel_zoom_delta`).
2. In `handle_custom_navigation` (post-render), read `app.pending_wheel_zoom_delta` and apply the zoom transform to `MetadataFrame`.
3. The `scroll_zoom_requires_ctrl` setting gates step 1: if true, only consume scroll when Ctrl is held.

**Why this works**: By zeroing the scroll delta *before* the GraphView widget runs, no other widget can consume it. The zoom application happens post-render against the now-populated MetadataFrame.

**Files**: `render/mod.rs`

**Tasks**:
- [ ] Add `pending_wheel_zoom_delta: f32` field to `GraphBrowserApp`.
- [ ] In `render_graph_in_ui_collect_actions`, before `GraphView` render: `ui.input_mut()` to intercept and zero scroll deltas when graph is hovered.
- [ ] In `handle_custom_navigation`, consume `pending_wheel_zoom_delta` and apply zoom with pointer-relative pivot.
- [ ] Remove old `apply_scroll_zoom_without_ctrl` function entirely.
- [ ] Verify `scroll_zoom_requires_ctrl` setting is respected.

---

### Phase 2: Durable Camera Commands (fixes fit + startup zoom)

**Goal**: Camera Fit (Z/C), Focus Selection, and startup zoom always succeed.

**Approach**: Replace one-shot booleans with durable command enums that retry until the MetadataFrame is ready.

1. Replace `fit_to_screen_requested: bool` and `pending_initial_zoom: Option<f32>` with a single `pending_camera_command: Option<CameraCommand>` enum:
   ```rust
   enum CameraCommand {
       Fit,                          // Fit all nodes with relax factor
       FitSelection,                 // Fit selected nodes (tighter)
       SetZoom(f32),                 // Absolute zoom
       StartupFit,                   // First-frame fit (same as Fit but triggered on init)
   }
   ```
2. `handle_custom_navigation` attempts to apply the pending command. If the MetadataFrame doesn't exist yet, it leaves the command in place for the next frame.
3. On successful application, clear the command.
4. Startup: set `pending_camera_command = Some(CameraCommand::StartupFit)` in the constructor. Remove `DEFAULT_STARTUP_ZOOM` constant and `pending_initial_zoom` field.
5. Z key: if 2+ selected, `CameraCommand::FitSelection`; else `CameraCommand::Fit`.
6. C key: always `CameraCommand::Fit`.

**Tuning constants** (named, top of `render/mod.rs`):
- `CAMERA_FIT_PADDING: f32 = 1.1` — bounding-box padding multiplier.
- `CAMERA_FIT_RELAX: f32 = 0.5` — zoom-back factor (0.5 = 50% as tight as mathematical fit).
- `CAMERA_FOCUS_SELECTION_PADDING: f32 = 1.2` — tighter padding for selection fit.

**Files**: `app.rs`, `render/mod.rs`, `input/mod.rs`

**Tasks**:
- [ ] Define `CameraCommand` enum in `app.rs`.
- [ ] Replace `fit_to_screen_requested`, `pending_initial_zoom`, `pending_zoom_to_selected_request` with `pending_camera_command: Option<CameraCommand>`.
- [ ] Update `request_fit_to_screen()` → `request_camera_command(CameraCommand::Fit)`.
- [ ] Update Z/C key handlers in `input/mod.rs` to emit the correct `CameraCommand`.
- [ ] In `handle_custom_navigation`: single dispatch site for `CameraCommand` with retry-on-missing-metadata.
- [ ] Remove `apply_pending_initial_zoom`, `apply_pending_fit_to_screen_request`, `apply_pending_zoom_to_selected_request` (consolidated into one function).
- [ ] Constructor: initialize with `CameraCommand::StartupFit`.

---

### Phase 3: Viewport-Tracking Gravity (fixes node drift)

**Goal**: Center gravity pulls nodes toward the viewport center, not graph-space origin.

**Approach**: Each frame, compute the viewport center in graph space from the current `MetadataFrame` (pan + zoom), and pass it to the physics simulation as the gravity target.

1. After `GraphView` renders and `MetadataFrame` is populated, compute:
   ```
   viewport_center_graph = (viewport_center_screen - meta.pan) / meta.zoom
   ```
2. Write this to the FR state's gravity target: `state.extras.0.params.target = viewport_center_graph`.
3. This requires extending `FruchtermanReingoldWithCenterGravityState` (or its params) to accept a target point instead of defaulting to `(0, 0)`. If the upstream crate doesn't expose this, apply the gravity force manually in `apply_semantic_clustering_forces` or a new `apply_viewport_gravity` helper.

**Fallback** (if upstream doesn't support target point): After the FR layout pass, apply a manual force toward the viewport center to all nodes. This is less elegant but achieves the same result.

**Files**: `render/mod.rs`, possibly `egui_graphs` fork

**Tasks**:
- [ ] Check if `FruchtermanReingoldWithCenterGravity` params support a configurable gravity target.
- [ ] If yes: set target each frame from MetadataFrame.
- [ ] If no: add `apply_viewport_gravity` helper that applies a small force toward viewport center after layout.
- [ ] Verify single nodes stay on-screen after panning.

---

### Phase 4: Tile Tree Semantics & Discoverability (fixes user confusion)

**Goal**: Preserve true architecture in the UI while making it predictable and understandable. Users should understand why container tabs exist and what actions they represent.

**Approach**:

1. **Container semantic labels**: In `tile_behavior.rs`, container fallback currently does `format!("{:?}", container.kind())`. Replace this with explicit container labels in `tab_title_for_tile`:
   - `ContainerKind::Horizontal` → `Split ↔`
   - `ContainerKind::Vertical` → `Split ↕`
   - `ContainerKind::Tabs` → `Tab Group`
   - `ContainerKind::Grid` → `Grid`
   Keep names short and stable for persistence screenshots and user guidance.

2. **Clarify architecture in-product**: Add a concise tooltip/help text for container tabs:
   - "Split tabs represent layout groups, not content panes."
   - Include one sentence on how to collapse them (close tabs on one side, or merge by drag).

3. **Simplification invariants**: Keep `all_panes_must_have_tabs: true` (required for local tab strips), and explicitly verify simplification behavior:
   - single-child `Linear` collapses,
   - same-direction nested linears join,
   - cross-direction nesting is preserved,
   - lone pane tabs remain wrapped.

4. **Split UX contract**: When a user drags a tab outside the strip (current "detach to split" behavior), the resulting split should:
   - Show the graph pane on one side and the detached webview on the other.
   - Pane tabs show pane titles; container tabs show semantic split/group labels.
   - If the split is later collapsed (all tabs closed on one side), the layout should simplify back to a single pane.

5. **Documentation sync**: Keep `design_docs/TERMINOLOGY.md` as source-of-truth and add a brief "Workbench Layout" section to the help panel explaining Tile, Pane, Container, Split, and Tab Group semantics.

**Files**: `desktop/tile_behavior.rs`, `desktop/tile_view_ops.rs`

**Tasks**:
- [ ] Replace `format!("{:?}", container.kind())` fallback with semantic labels in `tab_title_for_tile`.
- [ ] Add tooltip/help affordance for container tabs.
- [ ] Verify simplification invariants with targeted tests (including same-direction join + cross-direction preserve).
- [ ] Test split → close → simplify flow and tab-detach predictability.
- [ ] Ensure terminology alignment between UI strings and `TERMINOLOGY.md`.

---

## Validation Checklist

### Wheel Zoom
- [ ] Mouse wheel over graph pane zooms without Ctrl (default setting).
- [ ] Mouse wheel over graph pane with `scroll_zoom_requires_ctrl = true` requires Ctrl.
- [ ] Trackpad two-finger scroll over graph pane zooms.
- [ ] Mouse wheel over webview pane scrolls page content (does not zoom graph).
- [ ] Zoom is pointer-relative (zooms toward cursor position).

### Camera Fit
- [ ] Z key with 0 or 1 selected nodes: fits all nodes with relaxed zoom.
- [ ] Z key with 2+ selected nodes: fits selected nodes with tighter zoom.
- [ ] C key: always fits all nodes.
- [ ] On startup with existing graph: camera fits to nodes automatically.
- [ ] On startup with empty graph: no crash, camera at default position.
- [ ] Camera fit with a single node does not zoom in so far that the node fills the pane.

### Node Stability
- [ ] Single node with no edges stays on-screen after 5 seconds.
- [ ] After panning, nodes drift toward new viewport center (not back to origin).
- [ ] Multiple disconnected nodes cluster near viewport center.

### Tile Tree
- [ ] Container tabs use semantic labels (`Split ↔`, `Split ↕`, `Tab Group`, `Grid`) instead of raw enum debug strings.
- [ ] Pane tabs remain content-centric (`Graph`, webview title, diagnostics title).
- [ ] Closing last tab in a split collapses the split.
- [ ] Dragging a tab out of the strip creates a clean split.
- [ ] Users can distinguish content panes vs layout-group tabs without ambiguity.

---

## Architecture Notes

### Why post-render helpers fail for scroll zoom

The egui frame lifecycle is: `input → layout/render → post-render`. Scroll events are consumed during `layout/render` by whatever widget first claims them. By the time our post-render helpers run, the deltas read from `ui.input()` may already be zero because another widget consumed them. The only reliable interception point is `ui.input_mut()` *before* the widget that would consume them.

### Why one-shot flags fail for camera commands

egui_graphs creates `MetadataFrame` lazily on its first layout pass. Any camera command that fires before that pass finds no MetadataFrame and silently fails. One-shot flags (`bool` → `false` after first attempt) don't survive this race. Durable commands (`Option<Enum>` → `None` only after successful MetadataFrame write) do.

### Consistency principle

Every pane in the Workbench should follow the same focus model: **hover to activate, scroll to interact with pane content**. For webview panes, "scroll to interact" means page scrolling. For graph panes, "scroll to interact" means zoom. This is the same principle used by VS Code's editor tabs and terminal panels. The `scroll_zoom_requires_ctrl` setting is an escape hatch for users who prefer the Ctrl convention, not the default.

### Captured decisions from architecture deep-dive (2026-02-23)

These points are now treated as design constraints for future UX changes:

1. **Container nodes are first-class and should not be treated as rendering bugs.**
   `Horizontal`/`Vertical` originate from real `Container::Linear` nodes in the tree.

2. **Nested containers are expected and valuable.**
   The model intentionally supports arbitrary composition (`Tabs` ↔ `Linear` ↔ `Grid`) with simplification, not strict one-level pane grouping.

3. **Multiple tab bars are an intended capability.**
   Because panes are wrapped in `Tabs` (`all_panes_must_have_tabs: true`), each split region can own an independent local tab strip.

4. **UX should expose structure semantically, not hide structure categorically.**
   Default policy is rename/reframe container labels for clarity, not blanket suppression.

5. **Terminology must track actual code architecture.**
   `TERMINOLOGY.md` remains authoritative and must be updated whenever tile model/UI language changes.

---

## Dependency Map

```
Phase 1 (Input Ownership) ─── no dependencies
Phase 2 (Camera Commands)  ─── no dependencies (can parallelize with Phase 1)
Phase 3 (Viewport Gravity) ─── depends on Phase 2 (needs MetadataFrame access pattern)
Phase 4 (Tile Clarity)     ─── no dependencies (can parallelize with everything)
```

Phases 1, 2, and 4 can be implemented in parallel. Phase 3 should follow Phase 2.
