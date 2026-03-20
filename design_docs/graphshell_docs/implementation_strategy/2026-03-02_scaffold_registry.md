# Scaffold Registry

**Date**: 2026-03-02  
**Status**: Canonical active scaffold index  
**Marker format**: `[SCAFFOLD:<id>]`

This file tracks implementation scaffolds that are intentionally partial: core contracts or reducer paths exist, but one or more integration paths are still open.

## Active Scaffolds

| Marker | Area | Current state | Primary evidence | Closure criteria |
| --- | --- | --- | --- | --- |
| `[SCAFFOLD:view-dimension-ui-wiring]` | Graph view dimension control | `SetViewDimension` exists in reducer but no active surface dispatch path. | `graph_app.rs` (`GraphIntent::SetViewDimension` apply path); no corresponding dispatch in `tile_behavior.rs` controls. | Add explicit UI/control-surface dispatch + diagnostics/tests for 2D↔3D mode transitions. |
| `[SCAFFOLD:viewer-wry-runtime-registration]` | Wry viewer backend integration | All three integration gaps closed (2026-03-20): (1) URL navigation: `WryManager::navigate_webview` + `last_url` tracking added; `navigate_wry_overlay_for_node` wired into `lifecycle_reconcile` so the live overlay follows node URL changes including omnibar submissions; (2) Occlusion/resize: tile-splitter drag suppression added via `InteractionUiState::tile_drag_active` / `OverlaySuppressionReason::TileDrag`, diagnostic channel `compositor.overlay.native.suppressed.tile_drag` registered; resize already handled by per-frame `sync_native_overlay_for_tile`; (3) Input passthrough: in-pane `render_node_viewer_backend_selector` suppressed when `TileRenderMode::NativeOverlay` — control delegated to graph bar "Compat"/"Servo" button. Scaffold closed. | `mods/native/verso/wry_manager.rs`; `mods/native/verso/mod.rs`; `shell/desktop/lifecycle/lifecycle_reconcile.rs`; `shell/desktop/workbench/interaction_policy.rs`; `shell/desktop/workbench/tile_compositor.rs`; `shell/desktop/workbench/tile_behavior/node_pane_ui.rs`. | ✅ Closed. |
| `[SCAFFOLD:verse-protocol-handler]` | Verse protocol provider | Manifest advertises `protocol:verse`, runtime calls registration hook, but provider registration remains stubbed. | `mods/native/verse/mod.rs` TODO in `register_protocol_handlers`; runtime invokes hook in registry startup. | Register real `protocol:verse` handler(s) and validate resolution through protocol registry tests. |
| `[SCAFFOLD:wasm-mod-loader-runtime]` | WASM mod lifecycle | `ModType::Wasm` exists, but discovery/load activation path is native-mod-only. | `mod_loader.rs` defines `ModType::Wasm`; discovery uses inventory native registrations only. | Implement dynamic WASM discovery/load/activation pipeline (Extism) with capability enforcement tests. |

## Update Rules

- Add a new marker row when scaffolded code is introduced.
- Keep marker IDs stable across sessions.
- Remove or mark resolved in the same session as closure commit.
- Keep this file and `DOC_README.md` synchronized.
