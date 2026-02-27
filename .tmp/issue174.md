Parent: #88 (`lane:stabilization`)
Related: #90 (`lane:embedder-debt`) for host/frame/focus ownership debt

## Summary

Newly opened tiles/panes sometimes do not render content immediately and can appear blank until extra clicks or tile switches.

Behavior varies with focus state:
- some panes appear after a click/switch
- some panes do not render on first click but appear on subsequent clicks
- deleting another focused pane can leave the graph pane unfocused in an empty-looking workbench region

## Repro (current field report)

- Open tiles/panes from node/link flows.
- Observe inconsistent focus on spawn.
- Observe blank viewport that appears later after focus changes.
- In pane deletion flows, observe graph pane not regaining focus/render as expected.

## Likely hotspots

- `shell/desktop/ui/gui.rs`
- `shell/desktop/ui/gui_frame.rs`
- `shell/desktop/workbench/*` focus/tile activation paths
- `shell/desktop/lifecycle/webview_controller.rs`
- `shell/desktop/workbench/tile_runtime.rs`

## Architectural context

This looks like focus ownership + render activation ordering debt, not only a local paint bug.
It likely overlaps servoshell-derived host/frame assumptions and should be cross-linked to `lane:embedder-debt`.

## Done gate

- New tiles/panes consistently render on first spawn when intended to be focused.
- Pane-deletion focus handoff is deterministic and renders immediately.
- Repro captured in a scenario test or diagnostics receipt.