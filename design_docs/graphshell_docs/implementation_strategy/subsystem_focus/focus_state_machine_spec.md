# Focus State Machine — Interaction Spec

**Date**: 2026-03-12
**Status**: Canonical interaction contract
**Priority**: Implementation-ready (documents existing implementation)

**Related**:

- `SUBSYSTEM_FOCUS.md`
- `focus_and_region_navigation_spec.md`
- `2026-03-08_unified_focus_architecture_plan.md`
- `shell/desktop/ui/gui/focus_state.rs`
- `shell/desktop/ui/gui/focus_realizer.rs`
- `shell/desktop/ui/gui_state.rs` (type definitions)

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Focus state model** — `RuntimeFocusState`, `RuntimeFocusAuthorityState`,
   desired vs. realized focus semantics.
2. **`build_runtime_focus_state` priority order** — the canonical resolution
   function and its precedence rules.
3. **Capture stack** — modal overlay semantics, push/pop contract.
4. **Focus commands** — `FocusCommand` variants and their authority mutations.
5. **Focus realization** — `FocusRealizer` and the intent dispatch contract.
6. **Seeding and sync** — how return targets and realized state are seeded from
   the authority into the app and back.
7. **Diagnostics channels** — required channels and severities.

---

## 2. Core Types

### 2.1 SemanticRegionFocus

The canonical region vocabulary. Only these variants are valid:

```
SemanticRegionFocus =
  | ModalDialog
  | CommandPalette
  | ContextPalette
  | RadialPalette
  | HelpPanel
  | Toolbar
  | GraphSurface  { view_id: Option<GraphViewId> }
  | NodePane      { pane_id: Option<PaneId>, node_key: Option<NodeKey> }
  | ToolPane      { pane_id: Option<PaneId> }
  | Unspecified
```

`Unspecified` is the terminal/default region. It does not mean "no focus" — it
means the active region is not one of the semantic categories above.

### 2.2 RuntimeFocusState (Output)

The resolved, read-only focus state returned by `build_runtime_focus_state`:

```
RuntimeFocusState {
    semantic_region: SemanticRegionFocus,
    pane_activation: Option<PaneId>,
    graph_view_focus: Option<GraphViewId>,
    local_widget_focus: Option<LocalFocusTarget>,
    embedded_content_focus: Option<EmbeddedContentTarget>,
    capture_stack: Vec<FocusCaptureEntry>,
}
```

`overlay_active()` returns true when `semantic_region` is any of
`ModalDialog | CommandPalette | ContextPalette | RadialPalette | HelpPanel`.

### 2.3 RuntimeFocusAuthorityState (Mutable)

The mutable authority record owned by `GuiRuntimeState`:

```
RuntimeFocusAuthorityState {
    semantic_region: Option<SemanticRegionFocus>,
    pane_activation: Option<PaneId>,
    embedded_content_focus: Option<EmbeddedContentTarget>,
    capture_stack: Vec<FocusCaptureEntry>,
    command_surface_return_target: Option<ToolSurfaceReturnTarget>,
    transient_surface_return_target: Option<ToolSurfaceReturnTarget>,
    tool_surface_return_target: Option<ToolSurfaceReturnTarget>,
    local_widget_focus: Option<LocalFocusTarget>,
    realized_focus_state: Option<RuntimeFocusState>,
}
```

### 2.4 FocusCaptureEntry / FocusCaptureSurface

The capture stack entries used for modal overlay tracking:

```
FocusCaptureEntry {
    surface: FocusCaptureSurface,
    return_anchor: Option<ReturnAnchor>,
}

FocusCaptureSurface =
  | ModalDialog
  | CommandPalette
  | ContextPalette
  | RadialPalette
  | HelpPanel
```

### 2.5 Desired vs. Realized Focus

**Desired focus** (`RuntimeFocusAuthorityState.semantic_region`) is the focus
region the authority *intends* to be active. It is set by `FocusCommand`
mutations and by `SetSemanticRegion` overrides.

**Realized focus** (`RuntimeFocusAuthorityState.realized_focus_state`) is
what the tile tree *actually* has active after reconciliation with app state.

These may transiently diverge. `sync_runtime_semantic_region_from_workbench`
reconciles them by reading the current tile tree and updating
`realized_focus_state`.

Invariant: **Desired focus is never overwritten during sync.** Sync only
writes to `realized_focus_state`; it never modifies `semantic_region`.

---

## 3. `build_runtime_focus_state` Priority Order

`build_runtime_focus_state(RuntimeFocusInputs)` is the single canonical resolver.
All focus state computation must go through it. Priority order (highest wins):

| Priority | Condition | Resolved region |
|---|---|---|
| 1 | `show_clear_data_confirm == true` | `ModalDialog` (capture pushed) |
| 2 | `show_command_palette && !contextual_mode` | `CommandPalette` (capture pushed) |
| 2 | `show_command_palette && contextual_mode` | `ContextPalette` (capture pushed) |
| 3 | `show_radial_menu == true` | `RadialPalette` (capture pushed) |
| 4 | `show_help_panel == true` | `HelpPanel` (capture pushed) |
| 5 | `semantic_region_override == Some(r)` | `r` (no capture change) |
| 6 | `local_widget_focus == ToolbarLocation { .. }` | `Toolbar` |
| 7 | Active tile is a Graph pane | `GraphSurface { view_id }` |
| 8 | Active tile is a Node pane (via `pane_region_hint`) | `NodePane { pane_id, node_key }` |
| 9 | Active tile is a Tool pane (via `pane_region_hint`) | `ToolPane { pane_id }` |
| 10 | Nothing matches | `Unspecified` |

Capture stack entries are only pushed for priorities 1–4 (overlay surfaces).
Priorities 5–10 do not modify the capture stack.

### 3.1 Capture Stack Invariant

The capture stack in the returned `RuntimeFocusState` contains exactly one
entry per active overlay surface (1–4 above). Entries are pushed in priority
order. Only surfaces that are active contribute entries.

Example: `show_command_palette && show_help_panel` → capture stack contains
`[CommandPalette, HelpPanel]` (both active simultaneously is valid; command
palette wins semantic region).

---

## 4. Focus Builder Entry Points

Three public convenience builders exist, each wrapping `build_runtime_focus_state`:

### 4.1 `workspace_runtime_focus_state`

Builds from `GraphBrowserApp` workspace fields (show_command_palette,
show_help_panel, etc.) plus the focused_view and optional overrides.

**Use case**: Frame loop reads; places where app state is authoritative.

### 4.2 `workbench_runtime_focus_state`

Builds from the live `Tree<TileKind>` — determines region from actual active
tile kind.

**Use case**: Realizing semantic region from current workbench layout.

### 4.3 `desired_runtime_focus_state`

Builds from an explicit `RuntimeFocusAuthorityState` — the desired (intended)
focus, not the realized tile state.

**Use case**: Diagnostics, focus inspector, rendering decisions that should
reflect intent rather than tile actuality.

---

## 5. FocusCommand Contract

`apply_focus_command(authority, command)` mutates `RuntimeFocusAuthorityState`
in place. It must not touch the tile tree or app state.

| Command | Authority mutation |
|---|---|
| `EnterCommandPalette { contextual_mode, return_target }` | Sets `semantic_region = CommandPalette or ContextPalette`; stores `return_target` in `command_surface_return_target`; pushes capture entry |
| `ExitCommandPalette` | Removes command/context entries from capture stack; restores `semantic_region` from stack top or return target |
| `EnterTransientSurface { surface, return_target }` | Pushes capture entry; sets `semantic_region`; stores return target |
| `ExitTransientSurface { surface, restore_target }` | Removes capture entry for `surface`; restores `semantic_region` from stack top or `restore_target`; updates `transient_surface_return_target` |
| `SetEmbeddedContentFocus { target }` | Sets `embedded_content_focus = target`; no region change |
| `EnterToolPane { return_target }` | Sets `semantic_region = ToolPane { pane_id: None }`; stores return target |
| `ExitToolPane { restore_target }` | Restores `semantic_region` from `restore_target`; updates `tool_surface_return_target` |
| `SetSemanticRegion { region }` | Directly sets `semantic_region = Some(region)` |
| `Capture { surface, return_anchor }` | Pushes capture entry; sets `semantic_region` |
| `RestoreCapturedFocus { surface }` | Removes entry for surface; restores from stack top or sets `semantic_region = None` |

### 5.1 Exit Restoration Rules

When exiting a command/transient surface:

1. If the capture stack is non-empty after removing the exited surface's
   entry, restore `semantic_region` to the top of the stack.
2. If the capture stack is empty, restore `semantic_region` from the
   appropriate return target field.
3. Return target restoration maps `ToolSurfaceReturnTarget` to
   `SemanticRegionFocus` via `semantic_region_for_tool_surface_target`.

---

## 6. Focus Realization Contract (FocusRealizer)

`FocusRealizer` is the bridge between the authority (desired focus) and the
tile tree / app state (realized focus). It is created per-frame-loop pass and
consumes `WorkbenchIntent` values.

### 6.1 Intent Interception

`realize_workbench_intent(authority, intent) -> Option<WorkbenchIntent>` either:

- Returns `None` after handling the intent itself (consumed).
- Returns `Some(intent)` to pass it through to the normal workbench dispatcher.

Intercepted intents:

| Intent | Realizer behavior |
|---|---|
| `OpenCommandPalette` | Seeds return target; opens command or context palette based on authority region |
| `ToggleCommandPalette` | Opens if closed; closes + restores focus if open |
| `ToggleHelpPanel` | Opens if closed; closes + restores transient focus if open |
| `ToggleRadialMenu` | Opens if closed; closes + restores transient focus if open |
| `CycleFocusRegion` | Realizes the authority's desired semantic region in the tile tree |
| `OpenToolPane { kind }` | Seeds tool surface return target for Settings/HistoryManager; dispatches |
| `CloseToolPane { kind, restore }` | Seeds return target if `restore`; dispatches |

All other intents pass through to `dispatch_workbench_authority_intent`.

### 6.2 `realize_semantic_region_from_focus_authority`

When `CycleFocusRegion` is received, the realizer maps the authority's
`semantic_region` to the tile tree via `make_active`:

- `GraphSurface { view_id: Some(id) }` → activate tile with matching `graph_view_id`
- `GraphSurface { view_id: None }` → activate any Graph tile
- `NodePane { pane_id: Some(id) }` → activate tile with matching `pane_id`
- `NodePane { node_key: Some(k) }` → activate tile with matching `node`
- `NodePane { None, None }` → activate any Node tile
- `ToolPane { pane_id: Some(id) }` → activate tile with matching tool `pane_id`
- `ToolPane { pane_id: None }` → activate any Tool tile
- Any other region → no-op (returns false)

### 6.3 `restore_pending_transient_surface_focus`

Called after closing a transient surface (HelpPanel, RadialPalette) to restore
focus to the saved return target.

Guard: if any overlay is still active (`show_command_palette || show_help_panel
|| show_radial_menu`), returns immediately — focus is not restored while another
overlay is up.

After restoration, compares focus-before and focus-after. Emits:

- `CHANNEL_UX_FOCUS_RETURN_FALLBACK` (`Warn`) if restoration did not produce a
  focus transition, or the restored target does not match the saved target.
- `CHANNEL_UX_FOCUS_REALIZATION_MISMATCH` (`Warn`) if the realized semantic
  region does not match the desired region after restoration.
- `CHANNEL_UX_NAVIGATION_VIOLATION` (`Warn`) if no focus transition occurred
  and no active return target is present.

---

## 7. Seeding and Sync Contract

### 7.1 Return Target Seeding

Three seed functions copy return targets from authority → app pending fields,
but only if the app field is currently `None` (first-write-wins):

- `seed_command_surface_return_target_from_authority`
- `seed_tool_surface_return_target_from_authority`
- `seed_transient_surface_return_target_from_authority`

This prevents the authority's return target from being overwritten by a stale
app pending value when the overlay is already open.

### 7.2 `sync_runtime_focus_authority_state`

One-way sync: writes the current workspace focus state into
`authority.realized_focus_state`. Does not touch `semantic_region` or any
other desired-focus field.

This is called by the frame loop to keep the realized cache current.

### 7.3 `sync_runtime_semantic_region_from_workbench`

Calls `refresh_realized_runtime_focus_state` (workbench-aware sync) to update
the authority's `semantic_region` from the current tile tree's active tile.

**Important**: This overwrites `semantic_region` with the tile-derived value.
This must only be called in contexts where tile state should override the
desired authority — typically after tile layout changes, not after explicit
focus commands.

### 7.4 `capture_tool_surface_return_target_in_authority`

Captures the currently active tool surface return target from the tile tree
into `authority.tool_surface_return_target`, but only if it is not a control
surface (palettes, dialogs). This prevents palette overlay targets from
being saved as return points.

---

## 8. Helper Functions

### 8.1 `apply_graph_search_local_focus_state`

Sets `local_widget_focus = LocalFocusTarget::GraphSearch` when opening;
clears it only if it was `GraphSearch` when closing (prevents clearing
unrelated focus).

### 8.2 `apply_toolbar_location_local_focus_state`

Sets `local_widget_focus = ToolbarLocation { pane_id }` and `semantic_region =
Toolbar` when focused. On blur: clears `local_widget_focus` only if it was
`ToolbarLocation`, and clears `semantic_region` only if it was `Toolbar`.

### 8.3 `ui_overlay_active_from_flags`

Pure predicate: returns `build_runtime_focus_state(...).overlay_active()` from
raw show-flags without requiring a live authority. Used in rendering paths where
only overlay-active status is needed.

---

## 9. Diagnostics Contract

| Channel | Severity | Intent |
|---|---|---|
| `ux:focus_return_fallback` | `Warn` | Focus restoration did not produce expected transition |
| `ux:focus_realization_mismatch` | `Warn` | Desired and realized semantic regions diverged after restoration |
| `ux:navigation_violation` | `Warn` | No focus transition and no active return target after restore attempt |

---

## 10. Acceptance Criteria

| Criterion | Verification |
|---|---|
| `build_runtime_focus_state` with `show_clear_data_confirm` → `ModalDialog` with capture entry | Test: flag set → `semantic_region == ModalDialog && capture_stack.len() == 1` |
| Command palette with return target → capture entry has correct `ReturnAnchor` | Test: `EnterCommandPalette { return_target: Graph(v) }` → `capture_stack[0].return_anchor == ToolSurface(Graph(v))` |
| Context palette when `command_palette_contextual_mode == true` | Test: flag + `contextual_mode` → `ContextPalette` |
| `overlay_active()` is true for all overlay surfaces | Test: each overlay flag → `overlay_active() == true`; no flag → false |
| Exit command palette restores semantic region to return target | Test: enter + exit → region is `GraphSurface { view_id }` matching return target |
| Transient surface exit restores focus via `restore_target` | Test: enter transient + exit → region restored from `restore_target` |
| `sync_runtime_focus_authority_state` does not overwrite desired `semantic_region` | Test: authority has `CommandPalette`; sync called → `semantic_region` still `CommandPalette` |
| `workspace_runtime_focus_state` tracks `show_command_palette` with capture | Test: `show_command_palette = true` + `command_surface_return_target` set → capture stack has `ContextPalette` entry with `ReturnAnchor` |
| `workbench_runtime_focus_state` tracks active node pane region | Test: active node pane → `NodePane { node_key: Some(..) }` |
| `realize_semantic_region_from_focus_authority` activates correct tile for each region type | Test: authority set to `NodePane { node_key }` → `CycleFocusRegion` activates the node tile |
| `restore_pending_transient_surface_focus` emits `focus_return_fallback` when restoration fails | Test: broken return path → `Warn` channel emitted |
| Desired vs. realized are independently tracked | Test: authority has `NodePane`; realize yields `GraphSurface`; inspector reports both correctly |
