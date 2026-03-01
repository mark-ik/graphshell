# Issue #180 Receipt — Bridge Spike Run 03

**Date**: 2026-03-01  
**Issue**: `#180`  
**Run ID**: `bridge-spike-1772350583.json`

## Artifact

- `C:/Users/mark_/AppData/Roaming/graphshell/graphs/diagnostics_exports/bridge-spike-1772350583.json`

## Measured Output

- `sample_count`: `64`
- `failed_frame_count`: `0`
- `avg_callback_us`: `48`
- `avg_presentation_us`: `48`
- `chaos_enabled_sample_count`: `0`
- `restore_verification_fail_count`: `0`
- `failed_by_reason`: `{}`

## Delta vs prior clean run

Compared against `bridge-spike-1772350293.json`:

- `sample_count`: `3 -> 64`
- `avg_callback_us`: `685 -> 48`
- `avg_presentation_us`: `685 -> 48`
- `failed_frame_count`: `0 -> 0`
- `failed_by_reason`: `{}` (unchanged)

## Interpretation

- Runtime symptom matches diagnostics: black overdraw/flicker is no longer reproducing.
- The bridge path remains invariant-clean while running a substantially longer sample window.
- The higher sample count with lower average callback/presentation timing indicates the per-frame content-pass registration path is now stable under continued focus/interaction.