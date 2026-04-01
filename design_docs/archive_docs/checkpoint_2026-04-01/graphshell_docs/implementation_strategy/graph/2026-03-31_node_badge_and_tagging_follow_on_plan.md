# Node Badge and Tagging Follow-On Plan (2026-03-31)

**Status**: Closed / Archived 2026-04-01

**Related**:

- `2026-02-20_node_badge_and_tagging_plan.md`
- `node_badge_and_tagging_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../system/register/action_registry_spec.md`

The original node badge and tagging plan was mostly landed when this follow-on was created. This remainder is now closed: keyboard routing is intentionally bound through `Ctrl+T`, the panel host lives in `shell/desktop/ui/tag_panel.rs`, close semantics are explicit, the icon picker is searchable emoji-only, and the focused routing/interaction tests are present.

---

## Plan

### Feature Target 1: Deliberate Panel Routing

**Context**: The tag panel is currently reachable from the Selected Node inspector and the graph toolbar. The original interaction contract also called for deliberate keyboard and graph-context entry points, but `T` is still bound to physics and the graph context menu does not yet expose `Tags…`.

**Tasks**:

- Decide the canonical keyboard route through the input/action stack instead of silently stealing the current physics shortcut.
- Open the tag panel for the selected graph node through that routed action.
- Open the tag panel for the focused node pane when the workbench surface owns focus.
- Add a graph context-menu `Tags…` action that routes through the same open path.
- Keep the existing inspector and toolbar entry points as redundant affordances, not parallel implementations.

**Validation Tests**:

- `test_tag_panel_opens_from_bound_action_for_selected_graph_node`
- `test_tag_panel_opens_from_bound_action_for_focused_node_pane`
- `test_graph_context_menu_exposes_tags_action`

### Feature Target 2: Finish the Panel Contract

**Context**: The panel itself exists, but it still lives in render-layer code as a generic egui window. The original contract called for node-anchored placement plus explicit close semantics.

**Tasks**:

- Move the panel host out of `render/semantic_tags.rs` into a dedicated shell UI module.
- Preserve the current reducer-intent flow and node-owned tag truth; do not reintroduce a second state carrier.
- Anchor the panel near the selected node or focused node-pane target when geometry is available.
- Implement explicit `Esc`, outside-click, and deselect close semantics.
- Keep icon-preview/pending state local to `TagPanelState` only.

**Validation Tests**:

- `test_tag_panel_closes_on_escape`
- `test_tag_panel_closes_on_outside_click`
- `test_tag_panel_closes_when_selection_moves_away`

### Feature Target 3: Rich Icon Picker

**Context**: Durable icon overrides are already implemented. What is still missing is the richer searchable picker that the original plan described.

**Tasks**:

- Replace the current preset-row picker with a searchable emoji catalog.
- Decide whether Lucide still earns its asset/dependency cost. If yes, add a curated subset and search path. If no, explicitly narrow the scope to emoji-only and update the spec/plan together.
- Keep system-tag icons immutable.
- Reuse `NodeTagPresentationState` as the only durable carrier for user-tag icon choices.

**Validation Tests**:

- `test_user_tag_icon_override_persists_after_reload`
- `test_system_tag_icon_override_is_rejected`
- `test_icon_picker_search_returns_matching_emoji`
- `test_lucide_picker_search_returns_matching_slug` (only if Lucide remains in scope)

### Feature Target 4: UI-Path Coverage

**Context**: Model coverage exists for ordering/icon metadata and render-side coverage exists for suggestion ranking, but the interactive panel flows are not covered at the same level.

**Tasks**:

- Add UI-path tests for panel open, add, remove, and close flows.
- Add coverage for icon-preview selection and persisted icon writeback.
- Add at least one regression test that keeps canvas badges and tab-header badges in sync for the same node state.

**Validation Tests**:

- `test_tag_panel_add_flow_emits_tag_intent`
- `test_tag_panel_remove_flow_emits_untag_intent`
- `test_tag_panel_icon_selection_writes_presentation_metadata`
- `test_canvas_and_tab_badges_share_badge_resolution_order`

---

## Findings

- Do not reopen the `Node.tags` ownership migration or the `NodeTagPresentationState` carrier. Those are already the correct durable homes for canonical membership, ordering, and user-tag icon overrides.
- The biggest remaining UX gap is not the badge renderer; it is the missing deliberate trigger and close contract around the panel.
- The rich icon-picker slice should be treated as a scope decision, not assumed work. If Lucide does not provide enough value over emoji, the smaller and cleaner path is to formalize an emoji-only picker and close the lane.

### Issue-Ready Backlog

1. Route the tag panel through a canonical input/action path and add graph context-menu access.
2. Extract the tag panel into a dedicated shell UI module and finish close semantics.
3. Replace the preset-row icon picker with searchable emoji, plus Lucide only if explicitly retained.
4. Add UI-path tests for open/close/add/remove/icon-persistence flows.

---

## Progress

### 2026-03-31

- Created as the reduced remainder after auditing `2026-02-20_node_badge_and_tagging_plan.md` against the current implementation.
- Confirmed that badge resolution, presentation metadata, node-owned tag truth, orbit rendering, clip-border treatment, and tab-header badges are already landed.
- Isolated the actual unfinished work to routing, panel extraction/close semantics, richer icon picker/search scope, and UI-path coverage.

### 2026-04-01

- Closed after the remaining routing, shell-hosting, searchable emoji picker, and interaction-test work landed in code.
- Scope decision finalized: Lucide remains out of the current picker surface; emoji-only search is the active contract.
- Archived alongside the broader 2026-02-20 historical plan.
