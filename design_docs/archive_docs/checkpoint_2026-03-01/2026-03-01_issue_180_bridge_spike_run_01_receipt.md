# Issue #180 Receipt — Bridge Spike Run 01

**Date**: 2026-03-01  
**Issue**: `#180`  
**Run ID**: `bridge-spike-1772348489.json`

## Artifact

- `C:/Users/mark_/AppData/Roaming/graphshell/graphs/diagnostics_exports/bridge-spike-1772348489.json`

## Measured Output

- `sample_count`: `5`
- `failed_frame_count`: `5`
- `avg_callback_us`: `198`
- `avg_presentation_us`: `198`
- `bridge_path_used`:
  - `gl.render_to_parent_callback`: `5`

Latest sample snapshot:

- `bridge_path`: `gl.render_to_parent_callback`
- `tile_rect_px`: `{ "x": 0, "y": 0, "width": 2560, "height": 1355 }`
- `render_size_px`: `{ "width": 2560, "height": 1355 }`

## Interpretation

- The bridge-spike export harness is functioning and producing the required #180 measurement-contract fields.
- Current run indicates all sampled frames were flagged as failed (`failed_frame_count == sample_count`).
- This evidence is sufficient to continue #180 analysis but does **not** yet satisfy a clean bridge-viability pass signal.

## Next Step

- Capture at least one additional run after confirming no intentional chaos/probe-forced failure mode is active, and compare failed-frame rates across runs before deciding #180 done-gate readiness.