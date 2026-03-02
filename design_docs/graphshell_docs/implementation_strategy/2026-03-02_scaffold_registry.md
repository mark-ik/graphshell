# Scaffold Registry

**Date**: 2026-03-02  
**Status**: Canonical active scaffold index  
**Marker format**: `[SCAFFOLD:<id>]`

This file tracks implementation scaffolds that are intentionally partial: core contracts or reducer paths exist, but one or more integration paths are still open.

## Active Scaffolds

| Marker | Area | Current state | Primary evidence | Closure criteria |
| --- | --- | --- | --- | --- |
| `[SCAFFOLD:view-dimension-ui-wiring]` | Graph view dimension control | `SetViewDimension` exists in reducer but no active surface dispatch path. | `graph_app.rs` (`GraphIntent::SetViewDimension` apply path); no corresponding dispatch in `tile_behavior.rs` controls. | Add explicit UI/control-surface dispatch + diagnostics/tests for 2D↔3D mode transitions. |
| `[SCAFFOLD:divergent-layout-commit]` | Divergent layout writeback | Commit path is present but currently stubbed/no-op behavior. | `tile_behavior.rs` button label indicates stub; reducer logs only in `GraphIntent::CommitDivergentLayout`. | Implement canonical writeback semantics from divergent local simulation to shared canonical state with tests. |
| `[SCAFFOLD:viewer-wry-runtime-registration]` | Wry viewer backend integration | NativeOverlay mode plumbing exists, but active viewer registration and manager flow are partial/deferred. | `tile_runtime.rs` maps `viewer:wry` → `NativeOverlay`; `ViewerRegistry`/Verso registration currently centers on `viewer:webview`. | Wire `viewer:wry` registration + manager lifecycle under feature gate and add integration tests. |
| `[SCAFFOLD:verse-protocol-handler]` | Verse protocol provider | Manifest advertises `protocol:verse`, runtime calls registration hook, but provider registration remains stubbed. | `mods/native/verse/mod.rs` TODO in `register_protocol_handlers`; runtime invokes hook in registry startup. | Register real `protocol:verse` handler(s) and validate resolution through protocol registry tests. |
| `[SCAFFOLD:wasm-mod-loader-runtime]` | WASM mod lifecycle | `ModType::Wasm` exists, but discovery/load activation path is native-mod-only. | `mod_loader.rs` defines `ModType::Wasm`; discovery uses inventory native registrations only. | Implement dynamic WASM discovery/load/activation pipeline (Extism) with capability enforcement tests. |

## Update Rules

- Add a new marker row when scaffolded code is introduced.
- Keep marker IDs stable across sessions.
- Remove or mark resolved in the same session as closure commit.
- Keep this file and `DOC_README.md` synchronized.
