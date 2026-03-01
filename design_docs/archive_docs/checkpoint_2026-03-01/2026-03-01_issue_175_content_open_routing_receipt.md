# Issue #175 Receipt — Content-Originating Open Routing

**Date**: 2026-03-01  
**Issue**: `#175`  
**Domain**: Content Opening and Routing (`Workbench / Frame / Tile`)  
**Owner boundary**: Graphshell routing + lifecycle authority

## Contract Summary

Content-originating open actions must route through Graphshell node/tile semantics and must not use legacy bypass shortcuts.

## Implementation Evidence

### 1) Reducer-side shortcut removed

- File: `graph_app.rs`
- Change: `GraphIntent::WebViewCreated` no longer directly mutates selection via `select_node(child_node, false)`.
- Result: child creation remains semantic-only (node creation, mapping, lifecycle promotion, optional parent edge), without forcing focus/selection in the reducer path.

### 2) Routed open path asserted

- File: `shell/desktop/ui/gui_orchestration_tests.rs`
- New test: `webview_created_child_open_routes_through_frame_routed_intent`
- Assertions:
  - `WebViewCreated` does not directly change selected node.
  - Pending child webview open emits `GraphIntent::OpenNodeFrameRouted { prefer_frame: None }` for mapped child nodes.

### 3) Existing retry/open route remains valid

- File: `shell/desktop/ui/gui_orchestration_tests.rs`
- Existing test: `deferred_child_webview_retries_and_opens_via_frame_routed_intent`
- Confirms deferred child webview IDs are retried and eventually opened through frame-routed semantics after mapping appears.

## Invariants Covered

- Tile/node mapping is preserved through `webview_id -> node_key` mapping before routed open.
- Open path authority remains in Graphshell orchestration (`OpenNodeFrameRouted`), not web content integration.
- Framework role remains event source; pane/tile creation semantics remain Graphshell-owned.

## Verification Commands

- `cargo test webview_created_child_open_routes_through_frame_routed_intent -- --nocapture`
- `cargo test test_intent_webview_created_links_parent_without_direct_selection_mutation -- --nocapture`
- `cargo test deferred_child_webview_retries_and_opens_via_frame_routed_intent -- --nocapture`
- `cargo check`

## Related Strategy Docs Updated

- `design_docs/graphshell_docs/implementation_strategy/2026-02-28_ux_contract_register.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-28_ux_issue_domain_map.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-28_current_milestone_ux_contract_checklist.md`
