Parent: #90 (`lane:embedder-debt`)
Related: #88 (`lane:stabilization`), #89 (`lane:control-ui-settings`)

## Summary

Right-click / ctrl-click link actions inside webpage content (e.g. `servo.org`) can invoke a short legacy context-menu/open-new-view path that appears to bypass Graphshell node/tile creation semantics.

Observed symptoms include:
- legacy/short context menu appears instead of Graphshell command surface
- a tile may open without a mapped graph node
- behavior makes split/frame/workbench testing harder due to inconsistent focus/path routing

## Why this belongs in embedder debt

This looks like a servoshell-era host/webview path still handling link/context actions outside the Graphshell command/tile model.

It is not just UI polish:
- it breaks graph authority
- it causes tile/node mapping ambiguity
- it creates focus/render behavior differences that are hard to test

## Likely hotspots

- `shell/desktop/ui/gui.rs`
- `shell/desktop/host/*`
- `shell/desktop/lifecycle/webview_controller.rs`
- `shell/desktop/workbench/tile_runtime.rs`
- webview context menu / open-new-view handlers

## Scope

- Trace web content context-menu and open-new-tile/split flows
- Identify where Graphshell node/tile semantics are bypassed
- Bridge or retire legacy path(s)
- Document deferred limitations if any path must remain temporarily

## Done gate

- Web content open-in-new-view flows route through Graphshell node/tile creation semantics.
- Legacy fallback context-menu path is removed, bridged into command surface, or explicitly constrained with documented limitations.
- Tile/node mapping invariants hold for these flows.