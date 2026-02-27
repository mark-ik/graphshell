# Codebase Guide

**Last Updated**: 2026-02-27  
**Status**: Active  
**Purpose**: Fast orientation for where behavior lives in the Graphshell codebase.

**See also**:
- `ARCHITECTURAL_OVERVIEW.md`
- `GRAPHSHELL_AS_BROWSER.md`
- `../implementation_strategy/2026-02-27_workbench_frame_tile_interaction_spec.md`

---

## 1) Core Boundaries

Graphshell runtime behavior is split across three boundaries:

1. **Graph reducer authority** (`app.rs`)  
   Semantic state and intent-driven mutation.
2. **Workbench/tile authority** (`shell/desktop/ui` + `shell/desktop/workbench`)  
   Frame/tile arrangement, focus, and pane open/switch semantics.
3. **Runtime rendering authority** (`render/mod.rs` + compositor paths)  
   Graph rendering, viewer rendering paths, and render-mode policy.

---

## 2) High-Value Entry Points

- `app.rs`  
  `GraphIntent` handling, lifecycle transitions, persistence hooks, undo/redo foundations.

- `render/mod.rs`  
  Graph view rendering and interaction, camera commands, zoom/fit behavior.

- `shell/desktop/ui/gui.rs`  
  Frame-loop orchestration and workbench-authority intent interception.

- `shell/desktop/ui/gui_frame.rs`  
  Per-frame execution sequencing and panel orchestration.

- `shell/desktop/workbench/tile_view_ops.rs`  
  Tile open/focus/split helpers and tile-tree structural operations.

- `shell/desktop/workbench/tile_behavior.rs`  
  Tile-level user interactions and pending-open/split behavior.

- `shell/desktop/workbench/tile_render_pass.rs`  
  Tile render dispatch for graph/node/tool pane paths.

- `shell/desktop/workbench/tile_post_render.rs`  
  Post-render tile reconciliation and deferred operations.

- `shell/desktop/workbench/tile_runtime.rs`  
  Tile runtime state handling and viewer/runtime wiring helpers.

---

## 3) Related Runtime Systems

- `shell/desktop/lifecycle/*`  
  Webview lifecycle reconciliation and runtime mapping helpers.

- `shell/desktop/protocols/*`  
  Protocol routing and in-app protocol handlers.

- `persistence/*` and `shell/desktop/ui/persistence_ops.rs`  
  Snapshot/log persistence and save/load operations.

- `registries/*`  
  Registry/domain contracts and runtime capability wiring.

---

## 4) Practical Debug Routing

- **Node open/split/focus issue** → `gui.rs`, `gui_frame.rs`, `tile_view_ops.rs`, `tile_behavior.rs`
- **Graph camera/zoom issue** → `render/mod.rs`, `registries/domain/layout/canvas.rs`
- **Viewer composition/z-order issue** → `tile_render_pass.rs`, `tile_compositor.rs`
- **Lifecycle mismatch issue** → `app.rs`, `shell/desktop/lifecycle/*`
- **Persistence restore/routing mismatch** → `persistence/*`, `persistence_ops.rs`, `app.rs`

---

## 5) Editing Guardrails

- Keep graph-authority mutations in reducer paths.
- Keep tile/frame structure mutations in workbench-authority paths.
- Keep render-order fixes aligned with composited pass contract.
- Prefer updating active strategy docs when behavior contracts change.
