<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Canvas Input & Accessibility Follow-on (2026-04-19)

**Status**: Proposed (not started; split out from egui_graphs retirement)
**Scope**: Redesign the graph-canvas input/interaction features that the
earlier `egui_graphs`-based implementations did not meet the project's
standards bar for. These are explicitly *not* part of the egui_graphs
retirement baseline — they need design work, not just plumbing rework.

**Parent**: [../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md](../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md) (archived 2026-04-19)

**Standards bar**: Firefox-consistent where Firefox has a precedent,
WAI-ARIA conformant for composite widgets, ergonomic for both pointer and
keyboard users, modular (each feature independently gated and testable),
effective (resolves a real user need).

---

## 1. Features in this follow-on

Each feature below was in service before M2, was orphaned when M2 landed
the graph-canvas live path, and is being re-landed on graph-canvas + CanvasCamera.
The `egui_graphs`-era implementation does not meet the standards bar above as
drawn. Each entry describes the gap and the redesign direction.

### 1.1 Lasso (rectangle selection)

**Previous shape**: Right-drag rectangle selection, or Shift+Left-drag
(user-preference gate).

**Problems**:

- Right-drag conflicts with Firefox's convention that right-click/right-drag
  surface a context menu. The current code suppresses the context menu when
  the lasso completes a drag, which is an active compensation.
- No keyboard equivalent — fails WAI-ARIA composite-widget selection patterns.
  Users who cannot use a pointer cannot perform multi-selection.

**Redesign**:

- Default pointer gesture: Shift+Left-drag rectangle select. Right-drag
  surfaces the context menu; remove the right-drag lasso default.
- Keep right-drag lasso as a user preference for power users, but not the
  default.
- Add keyboard multi-select: `Shift+Arrow` extends selection in spatial
  direction; `Ctrl+Shift+A` select all visible; `Ctrl+A` already exists.
- Route through `ActionRegistry` so bindings are user-remappable (per the
  project's Control UI/UX principle).
- Build against `graph_canvas::engine::InteractionEngine`'s existing
  `LassoBegin` / `LassoUpdate` / `LassoComplete` state machine.

### 1.2 Tab keyboard traversal

**Previous shape**: `Tab` / `Shift+Tab` cycles nodes in `NodeIndex` order.

**Problems**:

- `NodeIndex` order is arbitrary to the user — not spatial, not temporal, not
  semantic.
- No ARIA role/label on the graph surface; screen readers announce nothing.
- No focus-change live-region announcement.
- Tab cycles *every* node; WAI-ARIA composite-widget convention is that Tab
  enters/exits the widget while arrow keys navigate within.

**Redesign**:

- Graph surface gets `role="application"` (or `role="graphicsdocument"` if
  the assistive tech supports it) with `aria-label` describing the view.
- Arrow keys navigate spatially between visible nodes (nearest neighbor in
  the arrow direction, tie-break by index). This matches how every other
  graph editor works.
- Tab enters the graph surface (if not focused) or leaves it (if focused);
  does not cycle individual nodes.
- Focus change emits an ARIA live-region announcement with the focused
  node's accessibility name.
- Integrate with Graphshell's existing semantic-tagging system
  (`render/semantic_tags.rs`) for richer announcements.

### 1.3 Hover tooltips (node + edge)

**Previous shape**: Hover a node/edge, tooltip appears.

**Problems**:

- Hover-only affordance. Keyboard users never see it. Touch users never
  see it. Screen readers never announce it.
- Fails the project's Control UI/UX principle
  ("Every control surface must be operable with the active input mode
  without requiring the other").
- No dismiss affordance (Esc should close).

**Redesign**:

- Hover and focus both trigger the tooltip (for pointer and keyboard users
  respectively).
- Tooltip content exposed via `aria-describedby` on the focused node element
  (synthetic focus target in the graph surface's DOM-equivalent accessibility
  tree).
- `Escape` dismisses; pointer-leave / focus-leave dismisses.
- Live-region update on focus change (see 1.2).
- Consider integrating with the existing semantic badges in
  `render/semantic_tags.rs` to surface the same information in both the
  tooltip and the accessibility announcement.

### 1.4 Keyboard zoom shortcuts

**Previous shape**: Raw `+` / `-` keys trigger zoom; `0` resets.

**Problems**:

- Raw `+` / `-` conflicts with any text input on the graph surface (future
  inline edit, search-on-graph, etc.)
- Not Firefox-consistent. Firefox uses `Ctrl+=` / `Ctrl+-` / `Ctrl+0`.

**Redesign**:

- Rebind to `Ctrl+=` (zoom in), `Ctrl+-` (zoom out), `Ctrl+0` (reset zoom).
- On macOS also accept `Cmd+=` / `Cmd+-` / `Cmd+0`.
- Route through `ActionRegistry`; user-remappable.
- Apply against `CanvasCamera.zoom`.
- Preserve the existing zoom clamp (`0.1` to `10.0` or per-view min/max).

---

## 2. Dependencies

All four features above can land independently once the baseline retirement
(parent plan) is complete. None blocks the others.

Prerequisites from the baseline:

- `CanvasCamera` is the single camera authority (baseline §1.C)
- `canvas_bridge::collect_canvas_events` handles all portable input
  translation (baseline §1.D)
- `ProjectedScene::hit_proxies` provides node/edge hit testing (already done)

Additional infrastructure to add in this follow-on:

- Semantic accessibility projection over `ProjectedScene` (needed for 1.2,
  1.3)
- ActionRegistry entries for the new bindings (needed for 1.1, 1.4)
- Live-region host adapter (egui + iced) — placeholder trait, real
  implementation depends on each host's accessibility story

---

## 3. Non-goals

- Do not reintroduce the `egui_graphs` dependency for any of these features.
- Do not implement the egui host adapter's accessibility layer under the
  assumption that iced's layer will look the same — keep the portable seam
  minimal and let hosts supply their own AT integration.
- Do not design for Gamepad input in this follow-on; that's covered by the
  Control UI/UX plan.

---

## 4. Progress

### 2026-04-19

- Plan created alongside egui_graphs retirement baseline. Not yet started;
  waiting on baseline completion.
