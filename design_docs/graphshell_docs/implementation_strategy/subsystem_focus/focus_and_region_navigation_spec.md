# Focus and Region Navigation Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `../2026-02-28_ux_contract_register.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `../research/2026-02-24_spatial_accessibility_research.md`
- `../subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`

**Adopted standards** (see [standards report](../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6):

- **WCAG 2.2 Level AA** — SC 2.4.3 (focus order), SC 2.4.11/2.4.12 (focus appearance), SC 2.1.2 (no keyboard trap), SC 2.4.7 (focus visible); §4.7 deterministic contract is the normative implementation path for these criteria
- **OpenTelemetry Semantic Conventions** — focus-owner change, blocked transfer, and fallback diagnostics

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- file tree = graph-backed hierarchical projection.
- workbench = arrangement boundary.

Focus routing respects these ownership boundaries and must not reassign semantic ownership across them.

## Contract template (inherits UX Contract Register §2A)

Normative focus contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- File tree is not content truth authority.
- Physics presets are not camera modes.

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
- In canonical Graphshell routing, hover alone must not retarget keyboard/camera command ownership; semantic-owner transfer requires explicit activation (`click`/`tap`/region-cycle/command handoff) through the focus router.

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

### 4.3A Shared Modal Isolation + Focus Return Contract Table (normative)

This table is canonical and mirrored verbatim in:

- `aspect_input/input_interaction_spec.md`
- `subsystem_ux_semantics/ux_tree_and_probe_spec.md`

| Transition / surface state | Capture owner while active | Required escape path(s) | Deterministic focus return target |
|---|---|---|---|
| Modal opened from any host region | Modal surface (`Modal` context) | `Escape` or explicit dismiss action | Stored pre-modal semantic region if still valid; otherwise next valid visible region |
| Modal dismissed/confirmed | Focus router on pop from `Modal` context | Dismiss action completion | Same region/control anchor captured on modal open (or deterministic fallback as above) |
| Command palette or radial opened | Command surface (`CommandPalette` context) | `Escape`, click-away dismiss, or explicit close action | Prior semantic region/control captured at open |
| Command palette or radial dismissed | Focus router on pop from `CommandPalette` context | Dismiss action completion | Prior captured region/control; must not default to omnibar |
| Omnibar/search explicit focus acquisition | Text-entry control (`TextEntry` context) | `Escape`, explicit unfocus, or region-cycle command | Prior semantic region/control captured before text-entry capture |
| Embedded content focused | Embedded viewer (`EmbeddedContent` context) with host escape guarantee | Host-focus-reclaim binding (`Escape` or configured equivalent) | Last host semantic region before embedded capture |
| Region-cycle command (`F6`) while not modal-captured | Focus router | Repeated region-cycle / reverse cycle binding | Next/previous visible landmark in deterministic order; wraps predictably |

Any transition that violates this table is a correctness bug and must emit
`ux:navigation_violation` or `ux:contract_warning` with enough context to identify transition
source and failed return target.

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

### 4.7 Deterministic Focus/Selection Interaction Contract

This section is the canonical deterministic mapping used for predictability closure (`#300`) and deliverable D2 alignment.

#### 4.7.1 Selection scope model

- **Selection truth owner** is the active Graph View (`GraphViewId`) within the active Frame.
- Workbench/global surfaces may inspect selection state but must not mutate selection without routing through Graph/Workbench authority intents.
- Frame switches preserve runtime focus context (`active region`, `active pane`, `last focused control`) and per-view selection/camera as runtime memory for the target Frame.
- Frame Snapshot persistence remains a storage concern; runtime focus/selection restoration is a focus-router concern and must be deterministic even when no persistence write occurs.

#### 4.7.2 Focus ownership map

| Region | Semantic owner | Local focus owner | Selection authority | Return-path anchor |
|---|---|---|---|---|
| Workbench Chrome | Focus router | Workbar/toolbar widgets | None | Active Frame root |
| Active Graph Pane | Focus router | Graph canvas interaction target | Active `GraphViewId` | Last focused graph pane |
| Node/Content Pane | Focus router | Viewer widget/webview local focus | Node-pane bound graph context | Parent tile slot |
| Tool Pane | Focus router | Tool pane controls | Tool-defined (non-graph) | Previously active region |
| Command Surface | Focus router | Palette/radial list cursor | Reads context; does not own selection truth | Captured return target |
| Omnibar/Search Surface | Focus router | Text entry control | Reads selection context | Captured return target |
| Modal/Blocking Surface | Focus router | Modal controls | None | Captured return target |

#### 4.7.3 Deterministic handoff algorithms

1. **Spawn/open**
   - Resolve destination region from invocation source and payload.
   - Store current region as return target.
   - Set semantic owner to destination region.
   - Set local focus to destination primary control.
   - Emit focus-transition diagnostic event.

2. **Close/dismiss**
   - If explicit return target exists and is visible/valid, restore it.
   - Else restore last valid visible region in this order: Active Graph Pane -> Node/Content Pane -> Workbench Chrome.
   - Restore local focus token for that region if still valid, else fallback to region primary control.
   - Emit restore diagnostic event.

3. **Frame switch**
   - Persist outgoing Frame runtime focus token (`region`, `pane`, `local control`) and active view selection/camera in runtime memory.
   - Activate target Frame.
   - Rehydrate target Frame runtime token if valid.
   - If invalid/missing, default to target Frame primary graph region.
   - Emit frame-switch focus event with fallback reason when applicable.

4. **Modal capture enter/exit**
   - Enter: push current semantic owner as capture return target and grant modal exclusive semantic focus.
   - Exit: pop and validate capture return target; if invalid, use close/dismiss fallback order.

#### 4.7.4 Input arbitration (pointer vs keyboard)

- Keyboard commands target semantic owner region, not hovered pointer region.
- Pointer hover may update local hover state only.
- Pointer click may transfer semantic focus only when clicking an interactable in a focus-owning region.
- If keyboard target and pointer hover disagree, semantic owner wins until explicit pointer activation.

#### 4.7.5 Active-pane indicator contract

- Exactly one pane region renders active-pane indicator at a time.
- Indicator must be legible in all `TileRenderMode` affordance paths.
- Indicator transitions occur in the same frame as semantic-owner change.
- When command surfaces are open, command-surface focus indicator is primary and pane indicator is retained as contextual secondary state.

#### 4.7.6 Failure handling

- Unresolvable return target must emit diagnostics and fallback via deterministic order.
- Focus traps are correctness violations and must be observable in UX probes/harness scenarios.
- Any fallback that changes intended return target must include explicit reason metadata in diagnostics.

### 4.8 Implementation checklist and test references

The deterministic interaction contract is considered implementation-aligned when the checklist below remains true.

- [x] Selection truth source is documented as active `GraphViewId` in active Frame.
- [x] Semantic focus owner to active-pane mapping is explicit by region.
- [x] Return-path algorithm is explicit for modal close, pane close, and frame switch.
- [x] Pointer-vs-keyboard arbitration is explicit and deterministic.
- [x] Failure and fallback diagnostics expectations are explicit.

Reference tests (current runtime evidence):

- `shell/desktop/ui/gui.rs`
   - `close_settings_tool_pane_restores_previous_graph_focus`
   - `node_focus_state_clears_graph_surface_focus`
   - `graph_surface_focus_state_clears_node_hint_and_syncs_focused_view`
- `shell/desktop/ui/gui_orchestration_tests.rs`
   - `close_settings_tool_pane_restores_previous_graph_focus_via_orchestration`
- `shell/desktop/workbench/tile_view_ops.rs`
   - `cycle_focus_region` path and `FocusCycleRegion` ordering (`Graph -> Node -> Tool` when present)

---

## 5. Planned Extensions

- per-region focus memory,
- configurable region-cycle order,
- richer focus scopes for multi-pane graph contexts,
- explicit focus breadcrumbs in diagnostics surfaces,
- per-domain Accessibility settings page: configurable region cycle order, per-region focus memory, skip-link visibility — exposed via the **Accessibility** settings category in `aspect_control/settings_and_control_surfaces_spec.md §4.2`.

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


