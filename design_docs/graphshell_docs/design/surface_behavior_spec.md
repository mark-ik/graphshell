# Surface Behavior Spec

**Date**: 2026-03-02  
**Status**: Canonical deliverable (D3)  
**Purpose**: Standardize scroll, resize, overflow, and empty/loading/error behavior across Graphshell surfaces.

**Related**:
- `../research/2026-03-02_ux_integration_research.md`
- `../implementation_strategy/aspect_command/command_surface_interaction_spec.md`
- `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md`
- `../implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md`
- `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md`

---

## 1. Scope and surface classes

This spec applies to the following surface classes:

1. Graph Pane
2. Node Pane
3. Tool Pane
4. Floating Window
5. Popup Menu
6. Toast
7. Dialog

This spec defines behavior contracts only; visual styling remains owned by theme/presentation specs.

---

## 2. Scroll defaults

### 2.1 Vertical scroll

- Vertical scroll is allowed on content-bearing surfaces by default.
- Graph Pane uses camera pan/zoom semantics instead of document scroll.
- Dialog bodies and floating-window content regions may scroll vertically when content exceeds viewport.

### 2.2 Horizontal scroll

- Horizontal scroll is disabled by default.
- Horizontal scroll is allowed only for content classes that are inherently wide:
  - URL/address fields
  - tabular diagnostic output
  - preformatted/code-like text blocks
- When horizontal overflow is disabled, truncation policy in §5 applies.

---

## 3. Max-height policy

Use shared max-height computation for floating and overlay surfaces:

- `surface_max_height = viewport_height - margin`
- Default margin constants:
  - Popup Menu: `16px`
  - Command/Context surface shell: `24px`
  - Dialog body: `48px`
  - Floating utility windows: `24px`

Hard rule: no floating/overlay surface may silently clip vertically without either scroll affordance or explicit truncation cue.

---

## 4. Resize behavior and minimum sizes

### 4.1 Resizability matrix

| Surface class | Resizable | Minimum size | Position persistence |
|---|---|---|---|
| Graph Pane | Via tile split/resize handles | `200x150` | Frame/tile layout authority |
| Node Pane | Via tile split/resize handles | `300x200` | Frame/tile layout authority |
| Tool Pane | Via tile split/resize handles | `200x150` | Frame/tile layout authority |
| Floating Window | Yes | `200x150` unless surface-specific override | Session runtime memory |
| Popup Menu | No | Content-fit with max-height policy | None |
| Toast | No | Content-fit | None |
| Dialog | Width/height fixed per dialog type (no freeform drag) | Dialog-specific | None |

### 4.2 Persistence rules

- Tile-hosted panes persist through Frame/Workbench layout state.
- Floating window position persists for the current session runtime only.
- Popup menus, toasts, and dialogs do not persist position.

---

## 5. Truncation and overflow cues

### 5.1 Truncation rules

- Single-line labels/titles truncate with trailing ellipsis.
- Truncation priority order:
  1. Keep interaction affordances visible (close/action controls).
  2. Truncate label text.
  3. Preserve icon/state indicators.

### 5.2 Full-content access

- Hover tooltip must expose full value for truncated text.
- For keyboard-only flow, focused truncated controls must expose full content through accessible name/description.

### 5.3 Overflow signaling

- Any clipped scroll container must show scroll affordance or explicit fade/cut indicator.
- Silent clipping without cue is forbidden.

---

## 6. Empty, loading, and error states

### 6.1 Empty-state contract

- Empty states must include:
  - clear reason text,
  - current scope/context,
  - optional primary recovery action.

### 6.2 Loading-state contract

- Loading indicators must be explicit for async operations.
- Surface class guidance:
  - Pane surfaces: inline spinner/skeleton in content region.
  - Dialogs: inline progress indicator near primary action area.
  - Floating windows: inline loading indicator in body.

### 6.3 Error-state contract

- Errors must include:
  - concise reason,
  - impact/scope,
  - recovery action (retry/open settings/dismiss/fallback).
- Viewer fallback/degraded states remain governed by viewer fallback spec; this spec requires the error presentation to be explicit and actionable.

---

## 7. Floating surface lifecycle

### 7.1 Open/close behavior

- Opening a floating surface must not produce ambiguous z-order.
- Closing returns semantic focus through canonical focus return-path rules.

### 7.2 Z-order rules

- Dialogs are top-most within app-owned UI layers.
- Popup menus render above their invoking surface, below blocking dialogs.
- Toasts render in non-blocking overlay layer and must not capture semantic focus.

### 7.3 Motion and reduced-motion

- Motion is optional and must remain non-blocking.
- Reduced-motion preference disables non-essential open/close animation.

---

## 8. Surface behavior matrix

| Surface class | Scroll | Overflow policy | Empty state | Loading state | Error state |
|---|---|---|---|---|---|
| Graph Pane | Camera pan/zoom (not document scroll) | Truncate UI labels; never clip controls silently | Explicit no-content graph message + create/open action | Inline lightweight loading cue during async graph operations | Inline graph-scope error + recovery action |
| Node Pane | Vertical content scroll | Horizontal scroll only for wide content classes; otherwise truncate + tooltip | Explicit node-content unavailable message + open/route action | Inline spinner/skeleton in content region | Inline error card + retry/fallback action |
| Tool Pane | Vertical scroll by default | Horizontal for diagnostic tables/code-like text; otherwise truncate + tooltip | Explicit empty tool-state message + primary action | Inline loading indicator in pane body | Inline tool error + recovery action |
| Floating Window | Vertical body scroll | Max-height policy + explicit scroll cue | Explicit empty body message | Inline loading indicator | Inline error block + action |
| Popup Menu | Vertical scroll when long | Max-height policy; no horizontal overflow without truncation cues | Explicit “no actions available” state | N/A (open should be immediate) | Inline unavailable/blocked reason |
| Toast | No scroll | Truncate long text with tooltip/accessibility description | N/A | N/A | Use toast variant with explicit action when recoverable |
| Dialog | Vertical scroll in body region as needed | Max-height policy + body scroll | Explicit no-content message when applicable | Inline progress indicator | Structured error + primary/secondary recovery actions |

---

## 9. Initial implementation checklist

- [x] Canonical D3 artifact exists at `design_docs/graphshell_docs/design/surface_behavior_spec.md`.
- [x] Covers Graph Pane, Node Pane, Tool Pane, Floating Window, Popup Menu, Toast, Dialog.
- [x] Scroll, max-height, resize/min-size, truncation, empty/loading/error, and floating lifecycle rules are explicit.
- [x] Control-plane and parity docs are linked/updated for D3 tracking.

## 10. Discoverability addendum (`#297`)

### 10.1 Empty-state inventory (priority surfaces)

- Graph empty state: explicit guidance and first-action path in command palette (`Create First Node`).
- Frame empty state: explicit actionable fallback remains required by workbench interaction contract.
- Pane empty states: covered by §6 empty-state contract and §8 matrix (`Node Pane`, `Tool Pane`).
- Search empty state: command/search surfaces must show explicit “no results” rather than blank list.
- History empty state: tool pane must show explicit no-history guidance with next action.
- Diagnostics empty state: tool pane must show explicit no-samples/no-events guidance.

### 10.2 Disabled-action explanation policy

- Palette surfaces must show disabled actions with explicit unmet-precondition reasons.
- Reason format: `"[Action] requires [precondition]. [How to satisfy it]."`
- Radial surfaces may omit disabled entries when spatial density constraints prevent readable explanations.

### 10.3 Implementation and regression linkage

- Implementation path: `render/command_palette.rs`
  - disabled-action reason tooltips for disabled entries,
  - empty-graph discoverability message + `Create First Node` action.
- Regression checks:
  - `disabled_node_delete_exposes_precondition_reason`
  - `empty_graph_message_present_when_graph_has_no_nodes`

Maintenance rule: any change to surface overflow/empty/loading/error semantics must update this spec and UX parity trackers in the same PR.