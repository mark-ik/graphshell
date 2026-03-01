# 2026-03-01 Embedder-Debt Host/UI Boundary Ownership Receipt

Date window: 2026-03-01
Lane: `lane:embedder-debt` (`#90`)
Primary issue closed by this receipt: `#120`

## Purpose

Record the current host/UI ownership boundaries after recent embedder-debt slices and define an explicit next extraction sequence.

This receipt is architectural mapping + sequencing only. It does not change runtime behavior.

## Current ownership map (authoritative snapshot)

### Host / Embedder boundary owners

- `shell/desktop/host/running_app_state.rs`
  - Owns runtime host state + embedder integration seams.
  - Owns lifecycle/event ingress from host/embedder side into UI frame loop inputs.
- `shell/desktop/host/window.rs`
  - Owns `EmbedderWindow` event-source responsibilities.
  - Must remain event-source only for Graphshell semantics.

### UI state + coordinator owners

- `shell/desktop/ui/gui.rs`
  - Owns `Gui` state and public UI-facing methods.
  - Owns state holder composition (`ToolbarState`, `GuiRuntimeState`) and rendering context handles.
- `shell/desktop/ui/gui/gui_update_coordinator.rs`
  - Owns update-frame coordination pipeline staging (prelude/pre-frame, input dispatch, semantic/post-render finalize).
  - No standalone semantic authority; coordinates existing authority owners.
- `shell/desktop/ui/gui_orchestration.rs`
  - Owns orchestration dispatch helpers for frame phases and workbench-authority intent interception.
  - GUI semantic mutations should be queued through intents; direct reducer-owned semantic writes are not allowed here.
- `shell/desktop/ui/gui_frame.rs`
  - Owns frame-phase utility operations and render/lifecycle scaffolding called by coordinator/orchestration modules.

### Workbench authority owners

- `shell/desktop/workbench/tile_view_ops.rs`
  - Owns tile-tree mutation operations (open/focus/split/close behavior).
- `shell/desktop/workbench/tile_runtime.rs`
  - Owns runtime viewer lifecycle helpers attached to tile topology.

## Boundary status summary

- `gui.rs` no longer contains the entire update-frame coordinator path in one file; coordinator boundary is explicit via `gui/gui_update_coordinator.rs`.
- Pending-open GUI selection path is intent-funneled (`GraphIntent::SelectNode`) and reducer-owned for semantic mutation application.
- Workbench-authority intent interception remains centralized in `gui_orchestration.rs`.
- Remaining pressure points are concentrated in `gui_frame.rs` responsibility breadth and host/content-open routing paths.

## Next extraction sequence (explicit)

1. **`#119`** — Split `gui_frame.rs` responsibilities (frame orchestration utility vs feature-specific logic) with in-module ownership docs.
2. **`#175`** — Constrain content-originating open/context flows to Graphshell semantic routing (remove/contain bypass paths).
3. **`#121`** — Non-functional naming/comment cleanup in hotspot modules to remove stale servoshell-era assumptions.
4. **`#171`** — Compositor diagnostics hardening/failure-mode visibility where boundary regressions still hide signal.
5. Re-run quick smoke matrix + targeted boundary regressions after each slice.

## Hotspot modules for upcoming slices

- `shell/desktop/ui/gui_frame.rs`
- `shell/desktop/ui/gui_orchestration.rs`
- `shell/desktop/ui/gui.rs`
- `shell/desktop/host/*`
- `shell/desktop/lifecycle/webview_controller.rs`
- `shell/desktop/workbench/tile_runtime.rs`

## Validation protocol for this sequence

For each child slice in this sequence:

- Compile gate: `cargo check`
- Targeted boundary tests for touched seam(s)
- Quick smoke gate: `Graphshell: Windows quick`
- Hub sync comment on `#90` with commit + acceptance mapping

## Linking

- Parent hub: `#90`
- Receipt-closing issue: `#120`
