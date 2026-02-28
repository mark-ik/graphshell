# RENDER — Aspect

**Date**: 2026-02-28
**Status**: Architectural aspect note
**Priority**: Immediate architecture clarification

**Related**:

- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../viewer/2026-02-26_composited_viewer_pass_contract.md`
- `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`
- `../2026-02-28_ux_contract_register.md`

---

## 1. Purpose

This note defines the **Render aspect** as the architectural owner of frame assembly and GPU surface lifecycle.

It exists to keep one boundary explicit:

- what content is visible is owned by Canvas, Workbench, and Viewer,
- how each frame is assembled, composited, and committed to the GPU is owned by the Render aspect,
- and the GUI shell (`gui.rs`, `gui_frame.rs`) is a host — not a semantic owner of rendering policy.

---

## 2. What The Render Aspect Owns

- egui frame loop coordination (begin frame, layout pass, paint)
- composition pass ordering (UI chrome → content → overlay affordance)
- `CompositorAdapter` lifecycle and GL/wgpu state isolation
- `TileRenderMode` resolution and per-tile render dispatch
- thumbnail pipeline (`thumbnail_pipeline.rs`)
- render callback registration and execution ordering
- GPU surface lifecycle (surface creation, resize, present)
- egui → wgpu migration boundary (tracking what has crossed and what has not)

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

The Render aspect does not own what is rendered — it owns the pipeline that commits frames.

- **Viewer** owns fallback state and content presentation policy.
- **Canvas** and **Workbench** own what surfaces exist and which are active.
- **Render** owns when and how those surfaces are assembled into a committed frame.

The planned GUI decomposition will make this boundary explicit in code: `gui.rs` is currently a monolith hosting both frame orchestration (Render) and workbench layout driving (Workbench). The decomposition extracts Render aspect concerns into their own module boundary.

---

## 4. Bridges

- Render -> Viewer: invokes per-tile composition passes in Viewer-defined order
- Render -> Workbench: reads tile tree for render traversal; does not mutate layout
- Render -> Canvas: invokes graph canvas render callback within the tile frame
- Registry -> Render: `ViewerRegistry` supplies `TileRenderMode` per viewer; `ThemeRegistry` supplies visual tokens for UI chrome pass

---

## 5. GUI Decomposition Note

`shell/desktop/ui/gui.rs` currently owns frame orchestration, workbench driving, and composition dispatch as a single monolith. Plans exist to decompose it:

- frame loop and GPU surface lifecycle → Render aspect module
- workbench tile tree driving and focus handoff → Workbench domain
- per-tile composition dispatch → CompositorAdapter (already partially extracted)

This decomposition is deferred pending the `egui_graphs` custom canvas migration (see `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`). Plans belong in this folder; they are not abandoned.

---

## 6. Architectural Rule

If a behavior answers "how is a frame assembled and committed to the GPU?" it belongs to the **Render aspect**.

