# Scaffold Registry

**Date**: 2026-03-02  
**Status**: Canonical active scaffold index  
**Marker format**: `[SCAFFOLD:<id>]`

This file tracks implementation scaffolds that are intentionally partial: core contracts or reducer paths exist, but one or more integration paths are still open.

## Active Scaffolds

| Marker | Area | Current state | Primary evidence | Closure criteria |
| --- | --- | --- | --- | --- |
| `[SCAFFOLD:view-dimension-ui-wiring]` | Graph view dimension control | Closed (2026-04-07): explicit `SetViewDimension` dispatch is now exposed from the graph-scoped host controls and the legacy panels surface; the reversible UDC depth preset remains available as a separate `ToggleSemanticDepthView` control. | `graph_app.rs` (`GraphIntent::SetViewDimension` apply path); `shell/desktop/ui/workbench_host.rs` dimension picker + `SetViewDimension` dispatch; `render/panels.rs` dimension buttons; focused reducer/host tests. | ✅ Closed. |
| `[SCAFFOLD:viewer-wry-runtime-registration]` | Wry viewer backend integration | All three integration gaps closed (2026-03-20): (1) URL navigation: `WryManager::navigate_webview` + `last_url` tracking added; `navigate_wry_overlay_for_node` wired into `lifecycle_reconcile` so the live overlay follows node URL changes including omnibar submissions; (2) Occlusion/resize: tile-splitter drag suppression added via `InteractionUiState::tile_drag_active` / `OverlaySuppressionReason::TileDrag`, diagnostic channel `compositor.overlay.native.suppressed.tile_drag` registered; resize already handled by per-frame `sync_native_overlay_for_tile`; (3) Input passthrough: in-pane `render_node_viewer_backend_selector` suppressed when `TileRenderMode::NativeOverlay` — control delegated to graph bar "Compat"/"Servo" button. Scaffold closed. | `mods/native/verso/wry_manager.rs`; `mods/native/verso/mod.rs`; `shell/desktop/lifecycle/lifecycle_reconcile.rs`; `shell/desktop/workbench/interaction_policy.rs`; `shell/desktop/workbench/tile_compositor.rs`; `shell/desktop/workbench/tile_behavior/node_pane_ui.rs`. | ✅ Closed. |
| `[SCAFFOLD:verse-protocol-handler]` | Verse protocol provider | Closed (2026-04-08): Verse now registers `verse` into the protocol provider/runtime surfaces and the runtime resolves `verse://` through live mod-owned extension records. | `mods/native/verse/mod.rs` real `register_protocol_handlers`; `shell/desktop/runtime/registries/mod.rs` Verse extension wiring; focused protocol/provider/runtime tests. | ✅ Closed. |
| `[SCAFFOLD:wasm-mod-loader-runtime]` | WASM mod lifecycle | Partial (2026-04-08): loader now admits path-backed `.wasm` modules via sidecar manifests, tracks `WasmModSource`, validates the minimal headless guest ABI (`init`, `render`, `on_event`, optional `update`), runs `init` during activation, exposes callable headless `render` / `on_event` host paths, and activates a minimal Extism runtime with deny-by-default capabilities plus rollback/quarantine handling for failed activation or unload. Richer guest ABI, capability grants beyond deny-by-default, and hot-reload remain open. | `registries/infrastructure/mod_loader.rs` `load_mod(path)` + rollback/quarantine diagnostics; `mods/wasm/mod.rs` headless Extism activation + guest-surface validation + host-call path; `shell/desktop/runtime/registries/mod.rs` WASM runtime extension records and unload path. | Extend from path-admission/headless activation to the fuller WASM plugin ABI and capability-grant model. |

## Update Rules

- Add a new marker row when scaffolded code is introduced.
- Keep marker IDs stable across sessions.
- Remove or mark resolved in the same session as closure commit.
- Keep this file and `DOC_README.md` synchronized.
