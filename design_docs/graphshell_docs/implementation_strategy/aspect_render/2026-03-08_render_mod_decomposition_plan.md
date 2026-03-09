# `render/mod.rs` Decomposition Plan

**Date**: 2026-03-08  
**Status**: Active; Stages 1-2 landed  
**Primary hotspot**: `render/mod.rs`  
**Related**:
- `../technical_architecture/ARCHITECTURAL_CONCERNS.md`
- `frame_assembly_and_compositor_spec.md`
- `render_backend_contract_spec.md`

---

## 1. Problem Statement

`render/mod.rs` had grown to 5,617 lines and mixed three different classes of work:

- graph canvas interaction/rendering
- operator/tool panels and settings UI
- reducer-bridge helper calls used by render-side tool flows

That makes render-path changes collide with panel/tool work and hides clean seams that already exist in the code.

---

## 2. Target Shape

`render/mod.rs` should remain the graph canvas/render-pass owner, while adjacent UI surfaces move into focused child modules.

Intended layout:

```text
render/mod.rs            graph canvas render + interaction orchestration
render/panels.rs         help/history/settings panels
render/reducer_bridge.rs render-side reducer dispatch helpers
render/command_palette.rs
render/radial_menu.rs
render/spatial_index.rs
```

Later extractions should follow the same rule: move coherent panel/tool surfaces out, keep canvas ownership in `render/mod.rs`.

---

## 3. Ordered Stages

### Stage 1. Extract panel surfaces and reducer bridge

Move already-clustered panel logic out first:

- physics settings panel
- camera controls settings panel
- keyboard shortcut help panel
- history manager panel and row rendering
- reducer-dispatch helper wrappers used by render-side flows

**Done gate**:

- `render/mod.rs` keeps calling the same functions through child modules
- no behavior change
- canvas code is easier to scan because panel code is gone

**Status**: Landed on 2026-03-08.

Implemented files:

- `render/panels.rs`
- `render/reducer_bridge.rs`

### Stage 2. Extract non-canvas tool panes

Move tool-pane UI that is not graph-canvas-specific into dedicated children.

Candidates:

- file tree tool pane
- settings tool pane sections
- sync tool pane sections
- command-surface context assembly/disabled-reason formatting that belongs with `render/command_palette.rs` or `render/radial_menu.rs`, not the canvas owner

**Done gate**:

- `render/mod.rs` no longer hosts long stretches of form/panel UI unrelated to graph drawing

**Status**: Landed on 2026-03-08.

### Stage 3. Isolate graph interaction helpers by concern

Within the remaining canvas file, group helpers by stable concern:

- camera/navigation helpers
- lasso/selection helpers
- metadata-slot / per-view graph-state keying helpers
- search/filter/culling helpers
- semantic-physics helpers

This stage should only happen after panel/tool extraction so the canvas seam is clearer.

**Done gate**:

- no single helper cluster dominates the file
- canvas render path is readable top-to-bottom
- multi-pane metadata access no longer depends on hardcoded or duplicated key construction inside unrelated canvas helpers

### Stage 4. Reduce owner module to canvas orchestration

Final desired owner file contents:

- graph canvas orchestration entry points
- graph-specific interaction/action collection
- graph-specific render overlays

**Exit target**:

- `render/mod.rs` under ~4k lines without scattering graph canvas logic across arbitrary files

---

## 4. Non-Goals

- no change to graph interaction semantics
- no movement of command palette / radial menu ownership beyond their existing modules in this slice
- no render backend contract change

---

## 5. 2026-03-08 Implementation Receipt

Landed extraction slices:

- `render/panels.rs` now owns panel/UI clusters that were already natural seams
- `render/panels.rs` now also owns the file-tree, settings, and sync tool-pane flows that had still been hosted in the owner file
- `render/reducer_bridge.rs` now owns render-side reducer dispatch helpers
- `render/mod.rs` remains the graph canvas owner

Measured result after landing:

- `render/mod.rs`: 5,617 -> 3,764 lines

Remaining highest-value follow-on:

- graph helper clustering by camera/selection/search/culling
- per-view metadata/keying helper extraction for lasso/selection multi-pane correctness
- command-surface invocation/context cleanup so `render/mod.rs` only requests a palette/menu render, not assembles command-surface policy inline
