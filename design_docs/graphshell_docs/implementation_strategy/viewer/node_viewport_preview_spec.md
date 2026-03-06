# Node Viewport Preview Spec

**Date**: 2026-03-05  
**Status**: Draft / implementation-ready direction  
**Scope**: In-canvas node preview/viewport behavior between static thumbnails and full workbench panes

**Related**:
- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `2026-02-23_wry_integration_strategy.md`
- `wry_integration_spec.md`
- `2026-02-26_composited_viewer_pass_contract.md`
- `2026-03-05_node_viewport_preview_minimal_slice_plan.md`

---

## 1. Purpose

Define a robust in-canvas preview model so nodes can be readable and glanceable without opening full workbench panes.

This spec introduces three presentation tiers:

- `Thumbnail`: static fallback image/placeholder.
- `Viewport`: bounded preview window inside node bounds, optionally interactive-lite.
- `Workbench`: full interaction authority in pane-hosted surface.

`Workbench` remains the canonical deep-interaction destination.

---

## 2. Node Viewport Geometry Contract

Each node may expose an internal viewport rectangle:

- `viewport_size`: width/height constrained to node bounds.
- `viewport_margin`: resizable inset padding from node border.
- `viewport_fit_mode`: contain/cover policy for content mapping.

### Invariants

- Viewport must never extend outside node bounds.
- Margin resizing updates viewport rect immediately.
- Node title/badges/chrome reserve layout slots and must not be occluded by viewport content.

---

## 3. Overlap Readability Contract

To preserve readability for enlarged viewport nodes:

- Node-body overlap is discouraged via soft repulsion.
- Repulsion is triggered by viewport-rect overlap, not just node-center distance.
- Dragging a node can temporarily override repulsion for user control; solver re-applies on release.

### Invariants

- Solver must not create infinite jitter loops.
- Repulsion is deterministic for identical initial state and seed.

---

## 4. Anti-Flicker Preview Pipeline

Continuous millisecond capture is prohibited.

Preview refresh must be event-driven with optional bounded cadence:

- Refresh on meaningful events:
  - navigation commit/finish
  - media frame key updates (play/pause/seek)
  - explicit user refresh
  - content metadata/title change
- Optional playback cadence: low-rate capped refresh while media is actively playing.
- Maintain `last_good_frame` and show it during refresh delay/failure.

### Invariants

- Preview refresh path must be non-blocking to frame loop.
- No per-frame capture loop for static pages.
- Failed capture never blanks content if `last_good_frame` exists.

---

## 5. Interaction Tier Model

`Viewport` interaction is capability-gated:

- `Static`: image/text snapshot only, no direct input.
- `InteractiveLite`: limited controls (play/pause, scrub, mute, open-link preview, scroll excerpt).
- `FullInteractive`: reserved for workbench pane (not graph viewport default).

### Policy

- Default graph-canvas mode is `Static` or `InteractiveLite` depending on content type/backend.
- Escalation action (`Open in Workbench`) is always available.

---

## 6. Backend Capability Matrix

| Backend / render mode | Graph viewport capability | Notes |
|---|---|---|
| Servo / `CompositedTexture` | `InteractiveLite` (or `Static`) | Graphshell owns pixels; safe for in-canvas interaction budgeted by policy |
| Wry / `NativeOverlay` | `Static` | Live native overlay remains pane-only; graph viewport uses preview/thumbnail |
| Embedded egui viewers (image/audio/video/docs) | `InteractiveLite` | Prefer lightweight controls in-canvas; full tools in workbench |
| Placeholder/tombstone | `Static` | Explicit fallback state |

### Invariant

`NativeOverlay` backends must not attempt live in-canvas overlay windows inside graph nodes.

---

## 7. Content-Type Preview Policy

- `Web`: snapshot + optional lightweight interaction when composited; Wry remains snapshot in graph.
- `Image`: high-quality fit + zoom-lite.
- `Audio`: cover art/waveform + transport controls.
- `Video`: poster frame + play/pause + short preview loop policy.
- `Document` (PDF/MD/etc.): first-page/section preview + scroll excerpt.

---

## 8. State Model (Planning)

Potential node-level fields:

- `preview_mode: Thumbnail | Viewport`
- `viewport_margin: f32`
- `viewport_interaction_tier: Static | InteractiveLite`
- `preview_last_frame: Option<ImageBlob>`
- `preview_last_updated_at: Option<Timestamp>`
- `preview_refresh_policy: EventDriven { media_cadence_hz: Option<f32> }`

---

## 9. Diagnostics Contract

Suggested channels:

- `viewer.preview.refresh_requested`
- `viewer.preview.refresh_succeeded`
- `viewer.preview.refresh_failed`
- `viewer.preview.stale_frame_displayed`
- `viewer.viewport.overlap_repulsion_applied`
- `viewer.viewport.open_in_workbench_invoked`

---

## 10. Acceptance Targets

- Enlarged viewport nodes remain readable with no viewport-outside-node spill.
- Margin resize updates are stable and immediate.
- Overlap repulsion reduces unreadable stacking without oscillation.
- Static pages do not trigger continuous capture loops.
- Media previews update at bounded cadence without flicker.
- Wry-backed nodes in graph remain preview-only and never spawn live native overlays in-canvas.
- Users can open any viewport node in workbench for full interaction in one action.

---

## 11. Rollout Sequence

1. Land viewport geometry + margin model.
2. Land event-driven preview refresh and `last_good_frame` fallback.
3. Land overlap-repulsion by viewport bounds.
4. Enable content-type `InteractiveLite` policies for embedded/composited paths.
5. Add diagnostics and guard tests; keep Wry graph path preview-only.
