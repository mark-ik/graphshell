# WorkbenchProfile and Workflow Composition Spec

**Date**: 2026-03-06
**Status**: Canonical profile contract
**Priority**: Implementation-ready

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md`

---

## 1. Scope

This spec defines the canonical `WorkbenchProfile` object used to compose workspace behavior from one named profile.

It canonicalizes:

1. Interaction preference composition (`InputProfile`, focus/open policies).
2. Pane behavior defaults (open/duplicate/close/degrade policies).
3. Command-surface preferences (palette/radial/omnibar behavior defaults).
4. Navigator host preferences (scope, edge, form factor, and host layout).
5. Workflow presets (named bundles for repeatable task flows).
6. Persistence boundaries and settings-surface ownership.

This spec does not redefine Graph truth or reducer semantics.

---

## 2. Canonical Profile Object

### 2.1 Identity and schema

`WorkbenchProfile` is a named, versioned object with stable ID and explicit ownership scope.

```json
WorkbenchProfile {
  id: WorkbenchProfileId,
  version: u32,
  label: String,
  interaction: InteractionPreferences,
  pane_defaults: PaneBehaviorDefaults,
  command_surface: CommandSurfacePreferences,
  navigator_hosts: Vec<NavigatorHostPreferences>,
  workflow_presets: Vec<WorkflowPreset>
}
```

`WorkbenchProfileId` must follow `namespace:name` format.

### 2.2 Ownership boundary

- Workbench subsystem owns `WorkbenchProfile` selection and application.
- Input aspect owns `InputProfile` resolution and event routing.
- Command aspect owns action meaning and command execution semantics.
- Settings/control surfaces own edit/apply/persist UX for profile objects.

---

## 3. Composition Domains

### 3.1 Interaction preferences

```json
InteractionPreferences {
  input_profile_id: String,
  per_context_input_overrides: Map<InputContext, String>,
  focus_return_policy: FocusReturnPolicy,
  open_routing_policy: OpenRoutingPolicy
}
```

Rules:

- `input_profile_id` references an `InputProfile` registered in `InputRegistry`.
- Missing profile references must degrade to the active default profile with diagnostic warning.
- `focus_return_policy` must comply with deterministic return contracts in the Focus subsystem.

### 3.2 Pane behavior defaults

```json
PaneBehaviorDefaults {
  open_mode_default: OpenMode,
  duplicate_policy: DuplicatePolicy,
  close_handoff_policy: CloseHandoffPolicy,
  inactive_pane_degrade_policy: InactivePaneDegradePolicy
}
```

Rules:

- Defaults apply only when an action does not provide explicit overrides.
- Graph identity and reducer ownership are unchanged by pane-default selection.
- `close_handoff_policy` must remain deterministic and focus-safe.

### 3.3 Command-surface preferences

```json
CommandSurfacePreferences {
  palette_enabled: bool,
  radial_enabled: bool,
  omnibar_enabled: bool,
  omnibar_focus_policy: OmnibarFocusPolicy,
  category_order_policy: CategoryOrderPolicy,
  pinned_action_ids: Vec<ActionId>
}
```

Rules:

- Command execution authority remains `ActionRegistry`.
- `omnibar_focus_policy` must preserve explicit-focus ownership requirements.
- Pinned actions must be treated as hints, not permission bypasses.

### 3.4 Navigator host preferences

```json
NavigatorHostPreferences {
  host_id: NavigatorHostId,
  enabled: bool,
  scope: NavigatorScope,
  form_factor: NavigatorFormFactor,
  anchor_edge: AnchorEdge,
  size_fraction: f32,
  cross_axis_margin_start_px: f32,
  cross_axis_margin_end_px: f32
}
```

Rules:

- `host_id` is stable and unique within one `WorkbenchProfile`.
- At most one Navigator host may occupy a given edge at a time.
- Multiple Navigator hosts may be enabled simultaneously so long as their
  resolved bounds do not overlap.
- `Top` / `Bottom` hosts default to `Toolbar` form factor.
- `Left` / `Right` hosts default to `Sidebar` form factor.
- Dragging a host across axes may change its default form factor, but the
  chosen form factor is still persisted explicitly in the profile.
- `scope` is per host; different hosts may project different scopes at the
  same time.
- Cross-axis margins are part of the remembered layout contract and must be
  restored with the host.

Suggested enums:

```json
NavigatorHostId = Top | Bottom | Left | Right
NavigatorScope = Both | GraphOnly | WorkbenchOnly | Auto
NavigatorFormFactor = Sidebar | Toolbar
AnchorEdge = Top | Bottom | Left | Right
```

### 3.5 Workflow presets

```json
WorkflowPreset {
  id: WorkflowPresetId,
  label: String,
  target_workbench_layout: Option<WorkbenchLayoutTemplateId>,
  startup_routes: Vec<String>,
  startup_actions: Vec<ActionId>
}
```

Rules:

- Presets are declarative bundles for repeatable workflows (for example review, research, triage).
- Preset application may open panes and trigger actions through existing authorities only.
- Presets must not directly mutate graph state outside reducer-approved intents.

---

## 4. Persistence and Resolution Rules

### 4.1 Scope boundaries

- Profile catalog persistence is user-scoped.
- Active profile selection is workspace-scoped.
- Optional per-workbench override is workbench-scoped.
- Navigator host enablement, scope, edge, form factor, and host margins are
  profile-owned settings that may be overridden per workbench only through the
  active `WorkbenchProfile` resolution chain.

### 4.2 Resolution chain

At runtime, resolve profile by this deterministic chain:

1. Explicit per-workbench `profile_id` override.
2. Workspace active `profile_id`.
3. User default `profile_id`.
4. Built-in fallback `workbench_profile:default`.

### 4.3 Apply and failure behavior

- Profile application is atomic at the profile object boundary.
- Invalid field references degrade by domain (input, pane, command) and emit diagnostics.
- Partial apply is allowed only with explicit diagnostics and domain-safe fallbacks.
- Persistence write failures must preserve runtime-applied state and surface explicit warning.

---

## 5. Settings Surface Integration

Settings ownership is defined in `../aspect_control/settings_and_control_surfaces_spec.md`.

Required settings routes:

- `verso://settings/workspaces/profiles`
- `verso://settings/workspaces/workflows`

Required operations:

- Create, clone, rename, delete profile.
- Set workspace active profile.
- Set or clear per-workbench profile override.
- Enable/disable Navigator hosts.
- Change Navigator host scope, anchor edge, form factor, and margins.
- Apply workflow preset and preview affected domains.

---

## 6. Diagnostics Contract

| Channel | Severity | Condition |
| --- | --- | --- |
| `ux:contract_warning` | `Warn` | Profile references missing/invalid component and fallback is applied |
| `ux:navigation_transition` | `Info` | Profile or preset application triggers route/open transitions |
| `ux:navigation_violation` | `Warn` | Profile application requests unsupported route/open/focus handoff |

---

## 7. Acceptance Criteria

1. A canonical `WorkbenchProfile` schema exists and is cross-linked from workbench and settings specs.
2. Profile resolution chain is deterministic and documented.
3. Interaction, pane, command-surface, and workflow domains are represented in one profile object.
4. Navigator host settings are represented in the profile with stable host IDs.
5. Settings routes for profile and workflow editing are explicit.
6. Persistence boundaries (user/workspace/workbench) are explicit and non-overlapping.
7. Fallback and failure behavior is diagnostics-backed.
