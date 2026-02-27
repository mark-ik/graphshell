Parent: #89 (`lane:control-ui-settings`)
Related: #90 (`lane:embedder-debt`) for legacy webview context-menu paths

## Summary

Current command surfaces are fragmented:
- `F2` summons a menu labeled `Edge Commands`
- right-click often shows a short legacy context menu
- context availability is too hover-target-specific (node-biased)

We need one command-surface model that supports:
- a larger global palette variant (`F2`)
- a compact contextual variant (right-click near pointer)

## Desired behavior (from field report)

- `F2`: global command palette with broader scope
- right-click: compact contextual command palette
- both backed by the same registry-driven command model
- available across core contexts (canvas, nodes, edges, tiles, panes, and workbench chrome)

## Likely hotspots

- `render/command_palette.rs`
- `render/mod.rs`
- `input/mod.rs`
- action registry / runtime registries integration
- webview context menu bridge paths

## Scope

- Unify invocation model and dispatch path
- Retire or narrow `Edge Commands` naming
- Ensure context-sensitive enablement without hard-disappearing relevant categories unexpectedly

## Non-goals

- Full command taxonomy/pinning customization in first slice
- Every context category implemented at once

## Done gate

- One command-surface backend model supports both F2 and contextual invocation variants.
- Labeling no longer says `Edge Commands` unless truly edge-specific.
- Palette can be invoked from canvas, nodes, edges, panes, and workbench chrome with context-appropriate enablement.