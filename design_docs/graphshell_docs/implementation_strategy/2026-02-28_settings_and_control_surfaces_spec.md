# Settings and Control Surfaces Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `2026-02-28_ux_contract_register.md`
- `2026-02-20_settings_architecture_plan.md`
- `2026-02-24_control_ui_ux_plan.md`
- `2026-02-27_workbench_frame_tile_interaction_spec.md`
- `../design/KEYBINDINGS.md`

---

## 1. Purpose and Scope

This spec defines how settings, history, diagnostics, and control pages behave as app-owned surfaces.

It explains:

- what control surfaces are for,
- what settings and related tool pages mean semantically,
- who owns their navigation and persistence behavior,
- what state transitions opening, applying, and exiting imply,
- what visual feedback must accompany changes,
- what fallback behavior must happen when preferred presentation is unavailable,
- which control-surface behaviors are core, planned, and exploratory.

---

## 2. Canonical Surface Model

### 2.1 Core control-surface classes

1. **Settings Pages**
2. **History Pages**
3. **Diagnostics Pages**
4. **Import and Persistence Pages**
5. **Transient Control Menus**

### 2.2 Canonical architectural rule

- Settings are nodes, not dialogs.
- Internal routes such as `graphshell://settings/...` are page-backed, pane-composable app surfaces.

### 2.3 Ownership model

- Graphshell owns route resolution, apply semantics, persistence timing, and return paths.
- The UI framework renders forms, lists, and pages but does not define settings meaning.

---

## 3. Canonical Interaction Model

### 3.1 Interaction categories

1. **Open**
2. **Inspect**
3. **Edit**
4. **Apply**
5. **Exit and Return**

### 3.2 Canonical guarantees

- control surfaces behave like real app pages,
- settings changes have explicit scope and persistence rules,
- exiting a control surface preserves a deterministic return path,
- control surfaces do not bypass Graphshell authority,
- placeholder or missing control pages are explicit rather than silent.

---

## 4. Normative Core

### 4.1 Route and Surface Identity

**What this domain is for**

- Keep settings and other tool pages addressable and composable with the workbench.

**Core rules**

- Internal control pages resolve through explicit Graphshell routes.
- Opening a settings or history page creates or focuses an app-owned pane destination.

**Who owns it**

- Graphshell routing and tool-surface controllers.

**State transitions**

- Route open resolves the target page and presentation mode.
- The workbench hosts the resulting pane like any other user-facing surface.

**Visual feedback**

- The surface must clearly identify which control page is active.

**Fallback / degraded behavior**

- Missing pages must surface as explicit unsupported or deferred states, not blank panes.

### 4.2 Page-Based Settings Model

**What this domain is for**

- Replace scattered floating panels with coherent settings pages.

**Core rules**

- Settings categories should be page-based and navigable.
- Category pages may include persistence, keybindings, appearance, physics, downloads, bookmarks, history, workspaces, notifications, and about.

**Who owns it**

- Graphshell settings controller and preference model.

**State transitions**

- Navigating between settings categories changes the active page, not the user's broader app context.
- Editing settings stages or immediately applies changes according to the page's policy.

**Visual feedback**

- Users must be able to tell which category they are editing.
- Page navigation and current scope must be obvious.

**Fallback / degraded behavior**

- Unimplemented settings categories must be marked as such, not hidden by silent omission.

### 4.3 Apply, Persist, and Revert Semantics

**What this domain is for**

- Make settings changes trustworthy.

**Core rules**

- Each setting must have a clear apply policy:
  - immediate,
  - staged-until-confirm,
  - or destructive-confirmation.
- Persistence timing must be explicit.

**Who owns it**

- Graphshell preference and settings authorities.

**State transitions**

- Editing updates staged or live state.
- Applying commits the change to the relevant authority.
- Reverting restores the prior persisted or staged value.

**Visual feedback**

- Pending vs applied state must be legible.
- Destructive or high-impact changes require explicit confirmation surfaces.

**Fallback / degraded behavior**

- If a setting cannot be applied, the user must see why and retain a recoverable state.

### 4.4 Return Paths and Pane Semantics

**What this domain is for**

- Keep control surfaces integrated with normal app navigation.

**Core rules**

- Closing or exiting a control page returns focus to a deterministic prior context.
- Control pages can be pinned, split, and reopened through normal workbench semantics.

**Who owns it**

- Graphshell workbench, focus, and tool-surface controllers.

**State transitions**

- Open, close, and switch operations affect pane and focus state, not graph identity.

**Visual feedback**

- Entry and exit should visibly preserve context.

**Fallback / degraded behavior**

- Closing a control page must not drop the user into an ambiguous blank region.

### 4.5 Accessibility, Diagnostics, and Import Surfaces

**What this domain is for**

- Keep control surfaces usable and observable.

**Accessibility**

- Ordered lists and tables should support standard range and list navigation conventions.
- Control pages must remain keyboard-usable.

**Diagnostics**

- Failed applies, invalid values, and missing pages must be observable.

**Import surfaces**

- Import actions may be surfaced through settings or related tool pages, but they still route through the same action authority as the command system.

---

## 5. Planned Extensions

- richer page-level IA and navigation,
- page-specific previews for appearance and physics,
- import dashboards and source summaries,
- better staged-apply support for complex preferences.

---

## 6. Prospective Capabilities

- mod-defined settings pages,
- workspace-scoped settings profiles,
- multi-user preference contexts,
- HTML-backed internal settings pages replacing some embedded egui pages.

---

## 7. Acceptance Criteria

1. Settings and related tool pages are app-owned, page-based surfaces.
2. Internal routes resolve through Graphshell rather than ad hoc dialogs.
3. Apply, persist, and revert semantics are explicit.
4. Exit and return paths are deterministic.
5. Missing or deferred pages are explicit.
6. Control surfaces remain accessible and diagnosable.
