# Camera Navigation Fix — Postmortem

**Date**: 2026-03-05
**Status**: Closed (verified working by maintainer)
**Files changed**: `render/mod.rs`, `graph_app.rs`, `registries/domain/layout/canvas.rs`

---

## Problem

Pan, zoom, and free camera movement were completely non-functional in the graph view. The graph was always visible but unmovable. Attempting to drag the background or scroll the wheel had no effect.

## Root Cause (Two Compounding Issues)

### 1. Dead metadata slot

`render/mod.rs` derived the key it used to read/write `MetadataFrame` pan/zoom state as:

```rust
let metadata_id = response.id.with("metadata");
```

`egui_graphs` stores `MetadataFrame` at:

```rust
// inside egui_graphs MetadataFrame::save() / load():
data.insert_persisted(Id::new(self.get_id()), self)
// where get_id() returns Id::new("egui_graphs_metadata_")
// → stored key is Id::new(Id::new("egui_graphs_metadata_")) — double-hashed, None custom_id
```

These two keys are completely different. Every pan/zoom write from graphshell went to a slot `egui_graphs` never reads. All navigation writes had zero visual effect.

### 2. `fit_to_screen_enabled: true` masked the bug

`CanvasNavigationPolicy` had `fit_to_screen_enabled: true` (or was previously passing `true` to `egui_graphs`). This caused `egui_graphs` to call `fit_to_screen()` every frame, resetting pan/zoom to center the graph. Even if graphshell's writes had gone to the right slot, `egui_graphs` would have overwritten them the next frame. The graph appeared functional (always centered) but was in fact permanently locked.

## Regression History

Multiple incremental attempts to fix this bug introduced additional regressions:

1. **`view_custom_id` / `with_id()` introduced**: Added `format!("gv_{}", view_id.as_uuid())` and passed it to `GraphView::with_id()`, `set_layout_state`, `get_layout_state`, and `metadata_id`. This changed the physics layout slot from `None` to `Some("gv_...")`, causing `egui_graphs` to look up physics state in a new empty slot. The Fruchterman-Reingold simulation restarted from random positions every session — nodes scattered off screen and disappeared.

2. **Partial double-hash fix**: `metadata_id` was corrected from `MetadataFrame::new(Some(view_custom_id)).get_id()` (single hash) to `egui::Id::new(MetadataFrame::new(Some(view_custom_id)).get_id())` (double hash). Still broken because `view_custom_id` didn't match the default `GraphView` instance (which uses `None` custom_id).

## Final Fix (3 targeted changes to `render/mod.rs`)

### 1. Revert layout state slot to `None`

```rust
// Before (broken — new empty slot causes physics restart):
let view_custom_id = format!("gv_{}", view_id.as_uuid());
set_layout_state::<FruchtermanReingoldWithCenterGravityState>(ui, physics_state, Some(view_custom_id.clone()));

// After:
set_layout_state::<FruchtermanReingoldWithCenterGravityState>(ui, physics_state, None);
```

Same for `get_layout_state` — reverted to `None`. `GraphView::with_id()` call removed entirely.

### 2. Fix `metadata_id` to match `egui_graphs`' actual storage key

```rust
// Before (wrong key — dead slot):
let metadata_id = response.id.with("metadata");
// or later (wrong custom_id):
let metadata_id = egui::Id::new(MetadataFrame::new(Some(view_custom_id.clone())).get_id());

// After (matches egui_graphs double-hash with None custom_id):
let metadata_id = egui::Id::new(MetadataFrame::new(None).get_id());
```

### 3. Keep `fit_to_screen_enabled: false`

`canvas_navigation_settings()` always returns `with_fit_to_screen_enabled(false)`. `egui_graphs` still performs a single first-frame fit via `first_frame_pending: true`, which gives a sensible initial view. After that, graphshell's `CameraCommand::Fit` owns all subsequent fit operations.

## Cleanup

- Removed `StartupFit` variant from `CameraCommand` enum (was never queued; design redundancy).
- Removed `fit_to_screen_enabled` field from `CanvasNavigationPolicy` (was the root cause of every-frame refit).
- Simplified `canvas_navigation_settings()` to zero parameters.

## Key Insight for Future Work

**`egui_graphs` coordinate space**: `meta.pan` is stored in widget-local coordinates (origin = widget top-left). At draw time only, `resp.rect.left_top()` is added to convert to screen-space for rendering. Graphshell's `apply_background_pan`, `apply_pending_wheel_zoom`, and `apply_pending_camera_command` operate in widget-local space and write directly to `MetadataFrame.pan` / `MetadataFrame.zoom` — this is correct and no coordinate conversion is needed at the write site.

**Multi-view caveat**: Reverting to `None` means all graph views share one `MetadataFrame` slot (pre-existing limitation before this PR). Per-view isolation requires `GraphView::with_id(Some(view_id))` AND all consumers (`set_layout_state`, `get_layout_state`, `metadata_id`) using the exact same id. Deferred to future work.

## Verification

- `cargo check --package graphshell` — no errors
- Runtime verified by maintainer: node appears, drag pans, scroll zooms, fit command works
