# Node Viewport Preview — Minimal Slice Plan

**Date**: 2026-03-05  
**Status**: Implementation-ready  
**Scope**: First shippable slice of in-canvas viewport previews with anti-flicker behavior

**Related**:

- `node_viewport_preview_spec.md`
- `viewer_presentation_and_fallback_spec.md`
- `2026-02-23_wry_integration_strategy.md`

---

## 1. Goal

Ship a minimal, stable node viewport preview system that:

- keeps preview content inside node bounds with resizable margin,
- avoids flicker by replacing continuous capture with event-driven refresh,
- preserves backend boundary (`NativeOverlay` remains preview-only in graph view),
- does not change workbench authority semantics.

---

## 2. Non-Goals (Slice 1)

- No full in-canvas editing for web documents.
- No cross-user/collaborative preview sync.
- No advanced overlap solver tuning beyond deterministic soft repulsion.
- No backend hot-swap automation.

---

## 3. Slice Breakdown

### Slice A — Data Model + Defaults

Add minimal preview state to node/view runtime:

- `preview_mode: Thumbnail | Viewport`
- `viewport_margin: f32`
- `preview_last_frame: Option<...>`
- `preview_last_updated_at: Option<...>`
- `preview_dirty: bool`

Default policy:

- New nodes start as `Thumbnail`.
- Existing nodes migrate with default values.
- `viewer:wry` in graph canvas remains preview-only.

Done gate:

- `cargo check` clean, persistence round-trip includes new fields with backward-compatible defaults.

### Slice B — Geometry + Render Hook

Implement viewport rect computation in graph node render path:

- Compute node chrome rect (title/badge exclusions).
- Apply `viewport_margin` inset.
- Clip preview render to viewport rect.

Done gate:

- Preview never paints outside node bounds.
- Margin changes visibly update viewport rect in same frame.

### Slice C — Event-Driven Preview Refresh

Replace high-frequency capture loop with event-driven invalidation:

- Mark `preview_dirty = true` on navigation/content/media key events.
- Refresh preview only when dirty (or bounded media cadence if enabled).
- Preserve/display `preview_last_frame` on refresh failure.

Done gate:

- Static pages do not continuously refresh preview.
- No blank/flicker if refresh fails and prior frame exists.

### Slice D — Minimal Interaction + Escalation

Support lightweight interaction policy:

- `Thumbnail`: non-interactive.
- `Viewport`: optional `InteractiveLite` controls for non-overlay embedded viewers.
- Always show/open escalation action: `Open in Workbench`.

Done gate:

- Single action opens focused node content in workbench pane.
- Wry graph nodes remain non-live preview and route to workbench for full interaction.

### Slice E — Overlap Repulsion (Deterministic Basic)

Add soft repulsion when viewport rectangles overlap:

- Apply after drag release and during physics settle.
- Use bounded nudge per tick to avoid oscillation.

Done gate:

- Overlap incidence decreases under dense viewport clusters.
- Solver deterministic under fixed seed.

---

## 4. Code Touchpoints (Expected)

- `graph_app.rs`
  - preview state fields
  - (optional) preview-related intents
- `model/graph/mod.rs`
  - persisted node metadata additions
- `services/persistence/mod.rs`
  - load/save defaults and migration
- `render/mod.rs`
  - viewport rect calc, clip, draw path
  - preview dirty checks and fallback frame usage
- `shell/desktop/ui/*` (if margin/preview-mode controls are exposed in slice 1)
  - simple toggle and slider entry points

---

## 5. Proposed Intents (Minimal)

- `SetNodePreviewMode { node, mode }`
- `SetNodeViewportMargin { node, margin }`
- `MarkNodePreviewDirty { node, reason }`
- `RefreshNodePreview { node }` (if refresh explicitly intent-routed)

If refresh stays render-driven, keep only mode/margin intents and mark-dirty via semantic events.

---

## 6. Guard Tests

- `node_viewport_rect_respects_node_bounds_and_margin`
- `node_preview_refresh_is_event_driven_not_continuous`
- `node_preview_uses_last_good_frame_on_refresh_failure`
- `wry_graph_view_node_remains_preview_only`
- `open_in_workbench_from_viewport_routes_correctly`
- `viewport_overlap_repulsion_is_deterministic`
- `preview_state_persistence_roundtrip_with_defaults`

---

## 7. Diagnostics (Minimal)

- `viewer:preview_refresh_requested` (Info)
- `viewer:preview_refresh_succeeded` (Info)
- `viewer:preview_refresh_failed` (Warn)
- `viewer:preview_last_good_frame_used` (Warn)
- `viewer:viewport_open_in_workbench` (Info)

---

## 8. Rollout Order

1. Slice A (state/persistence)  
2. Slice B (geometry/render clip)  
3. Slice C (event-driven refresh)  
4. Slice D (interaction + workbench escalation)  
5. Slice E (basic overlap repulsion)

Release after Slice C if stability target is met; D/E can follow as incremental closures.
