# Issue #180 Receipt — Bridge Spike Run 02

**Date**: 2026-03-01  
**Issue**: `#180`  
**Run ID**: `bridge-spike-1772349117.json`

## Artifact

- `C:/Users/mark_/AppData/Roaming/graphshell/graphs/diagnostics_exports/bridge-spike-1772349117.json`

## Measured Output

- `sample_count`: `3`
- `failed_frame_count`: `3`
- `avg_callback_us`: `722`
- `avg_presentation_us`: `722`
- `chaos_enabled_sample_count`: `0`
- `restore_verification_fail_count`: `0`
- `failed_by_reason`:
  - `scissor`: `3`

## Interpretation

- Failure attribution is now explicit and consistent across this run: all failures map to the `scissor` invariant.
- Chaos mode was not active and restore verification remained successful, which narrows likely cause to non-chaos scissor-state mutation/leak in the guarded callback flow.
- #180 remains open pending at least one run with reduced or zero failed-frame rate after investigating scissor-state behavior in the bridge path.