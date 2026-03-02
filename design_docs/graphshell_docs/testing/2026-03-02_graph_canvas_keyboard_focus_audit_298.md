# #298 Graph Canvas Keyboard Focus + Naming Audit

**Date**: 2026-03-02  
**Issue**: `#298`  
**Scope**: Graph-pane keyboard focus traversal baseline and accessible naming model hardening.

---

## 1. Implemented behavior

### 1.1 Keyboard traversal model (`G-A-1`, `G-A-4`)

- Graph canvas now participates in keyboard traversal when the graph widget has focus.
- `Tab` advances to the next graph node in deterministic `NodeKey` order.
- `Shift+Tab` moves to the previous graph node (with wrap-around).
- Traversal emits the same reducer-owned selection intent path as pointer selection (`GraphAction::SelectNode` with single-select semantics).

### 1.2 Accessible naming model (`G-A-3`)

- Graph canvas now reports an explicit accessibility label via `widget_info`.
- Label includes focused node context when available:
  - `Graph canvas. Focused node: {title-or-url}. Press Tab or Shift+Tab to move between nodes.`
- Fallback label when nothing is focused:
  - `Graph canvas. No node focused. Press Tab to focus the first node.`

Naming source policy:

1. Node title when non-empty.
2. Node URL when title is empty.
3. `Untitled node` fallback when both are empty.

---

## 2. Verification evidence

### 2.1 Unit tests added

- `render::tests::keyboard_traversal_advances_and_wraps_in_deterministic_order`
- `render::tests::keyboard_traversal_reverse_wraps_to_last_when_unfocused`
- `render::tests::graph_canvas_accessibility_label_includes_focused_node_name`

### 2.2 Command receipts

Executed targeted tests:

- `cargo test --lib keyboard_traversal_`
- `cargo test --lib graph_canvas_accessibility_label_includes_focused_node_name`

Observed result:

- All targeted `#298` tests passed.

---

## 3. Done-gate mapping (`#298`)

- [x] At least one keyboard traversal flow implemented and tested (`Tab` / `Shift+Tab` wrap traversal).
- [x] Accessibility checklist status delta updated to reflect graph keyboard focus + naming baseline closure.
- [x] Focus/naming behavior remains reducer-owned and deterministic.

---

## 4. Notes

This closure establishes baseline keyboard traversal and naming exposure for the graph canvas interaction model. Follow-on accessibility work can extend from this baseline to richer per-node AccessKit subtree exposure and reader-mode traversal without changing the core deterministic traversal contract.