# Focus and Region Navigation Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `2026-02-28_ux_contract_register.md`
- `2026-02-27_workbench_frame_tile_interaction_spec.md`
- `2026-02-28_graph_node_edge_interaction_spec.md`
- `2026-02-28_command_surface_interaction_spec.md`
- `../research/2026-02-24_spatial_accessibility_research.md`
- `2026-02-27_ux_baseline_done_definition.md`

---

## 1. Purpose and Scope

This spec defines how focus moves between Graphshell regions and who owns focus at any moment.

It explains:

- what app regions exist,
- what focus means semantically in each region,
- who owns focus transitions,
- what state transitions focus changes imply,
- what visual feedback must accompany focus,
- what fallback behavior must happen when focus resolution is unclear,
- which region-navigation controls are core, planned, and exploratory.

---

## 2. Canonical Region Model

### 2.1 Primary regions

Graphshell has these primary navigable regions:

1. **Workbench Chrome**
2. **Active Graph Pane**
3. **Node/Content Pane**
4. **Tool Pane**
5. **Command Surface**
6. **Omnibar/Search Surface**
7. **Settings or History Surface**
8. **Modal or Blocking Surface**

### 2.2 Ownership model

- Graphshell focus router owns semantic focus state and region handoff.
- The framework may expose widget-local focus within a region.
- Widget-local focus must not become the global authority for region focus.

---

## 3. Canonical Interaction Model

### 3.1 Focus categories

1. **Region Focus**
   - which app region currently owns semantic input
2. **Local Focus**
   - which control inside a region currently owns widget-local input
3. **Capture**
   - temporary exclusive ownership by modal or text-entry flow
4. **Return Path**
   - deterministic restoration of the prior valid region

### 3.2 Canonical guarantees

- there is always one semantic focus owner,
- opening a new surface produces deterministic focus handoff,
- closing a surface returns focus to a visible valid successor,
- users must not get trapped in a region without an escape path,
- focus changes must be visible and diagnosable.

---

## 4. Normative Core

### 4.1 Region Focus Ownership

**What this domain is for**

- Determine which region owns keyboard, command, and non-pointer semantic input.

**Core rule**

- Exactly one region owns semantic focus at a time.
- Hover may influence pointer targeting, but it does not silently replace semantic focus unless the owning interaction model explicitly permits it.

**Who owns it**

- Graphshell focus router.
- The framework may expose local focus state only.

**State transitions**

- Opening a region may transfer semantic focus to that region.
- Selecting within a region may update local focus without changing region focus.

**Visual feedback**

- Focused region state must be legible.
- Focused control within that region must also be legible.

**Fallback / degraded behavior**

- If focus ownership cannot be resolved, Graphshell must fall back to the last valid visible region and emit diagnostics.

### 4.2 Spawn, Open, and Close Handoffs

**What this domain is for**

- Keep new surfaces usable on first activation and keep closing behavior predictable.

**Core controls**

- Opening a new pane or surface transfers focus to the new primary interactive element.
- Closing a focused surface returns focus to the next visible valid context.

**Who owns it**

- Graphshell workbench and focus controllers.

**State transitions**

- Spawn enters the new region and sets local focus to its primary control or canvas.
- Close removes the region and restores focus to the successor region.

**Visual feedback**

- The newly focused region must visibly read as active on first render.
- Focus return after close must be visible and immediate.

**Fallback / degraded behavior**

- Blank first-frame or ambiguous focus return is forbidden.

### 4.3 Capture, Modals, and Text Entry

**What this domain is for**

- Handle temporary exclusive focus without breaking return paths.

**Core controls**

- Text-entry surfaces capture text input while active.
- Modal or blocking surfaces capture semantic focus until resolved or dismissed.
- Escape or explicit dismissal returns focus through a deterministic path.

**Who owns it**

- Graphshell focus router defines capture rules.
- Individual surfaces may request capture; they do not own cross-app return semantics.

**State transitions**

- Entering capture stores a return target.
- Exiting capture restores the stored valid region if it still exists.

**Visual feedback**

- Capturing surfaces must clearly read as modal or active.
- Suppressed global commands should be explainable.

**Fallback / degraded behavior**

- If the saved return target no longer exists, Graphshell must restore focus to the next valid visible region.

### 4.4 Region Cycling and Escape Hatches

**What this domain is for**

- Ensure non-pointer users can move across the application deliberately.

**Core controls**

- Region-cycling shortcuts (for example `F6`) move across major app regions.
- Skip-link semantics must exist for toolbar, graph, and content regions.

**Who owns it**

- Graphshell accessibility and focus controllers.

**State transitions**

- Region cycling changes semantic focus, not graph or content meaning.
- Cycling wraps predictably through the visible region order.

**Visual feedback**

- The newly focused region must announce and display focus.

**Fallback / degraded behavior**

- Regions that are absent or disabled are skipped explicitly rather than trapping the user.

### 4.5 Cross-Surface Focus Rules

**What this domain is for**

- Keep graph, workbench, search, and command surfaces from fighting over ownership.

**Core rules**

- Command surfaces take semantic focus while open.
- Omnibar text entry takes text focus and may temporarily suppress unrelated global bindings.
- Graph pane resumes command targeting when it regains semantic focus.
- Web content focus must remain escapable back into host regions.

**Who owns it**

- Graphshell focus router; Servo/webview focus is subordinate to host-region routing.

**State transitions**

- Surface open, confirm, dismiss, and close all update the active region owner.

**Visual feedback**

- The user must be able to tell whether they are in host UI, graph canvas, or embedded content.

**Fallback / degraded behavior**

- Focus traps are forbidden.
- If embedded content cannot yield focus cleanly, Graphshell must expose an explicit host-side escape path.

### 4.6 Diagnostics and Accessibility

**What this domain is for**

- Make focus bugs observable and non-pointer navigation viable.

**Diagnostics**

- Focus-owner changes and blocked focus transfers should be observable in diagnostics.
- Ambiguous or dropped focus transitions are correctness bugs.

**Accessibility**

- Focus order must be deterministic.
- Screen-reader and keyboard users must be able to recover to a known region without pointer input.

---

## 5. Planned Extensions

- per-region focus memory,
- configurable region-cycle order,
- richer focus scopes for multi-pane graph contexts,
- explicit focus breadcrumbs in diagnostics surfaces.

---

## 6. Prospective Capabilities

- voice-driven region switching,
- predictive focus restoration based on task history,
- mod-defined focus regions,
- richer spatial focus navigation for multi-canvas workspaces.

---

## 7. Acceptance Criteria

1. Exactly one semantic focus owner exists at all times.
2. Spawn and close handoffs are deterministic and visible.
3. Modal and text-entry capture have explicit return paths.
4. Region cycling and escape hatches exist for non-pointer users.
5. Host UI and embedded content focus boundaries are explicit.
6. Focus failures are diagnosable rather than silent.
