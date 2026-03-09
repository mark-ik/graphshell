<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector B — Input & Dispatch Registry Development Plan

**Doc role:** Implementation plan for the input and dispatch registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `InputRegistry`, `ActionRegistry`, `RendererRegistry` (new)
**Specs:** [input_registry_spec.md](input_registry_spec.md), [action_registry_spec.md](action_registry_spec.md)
**Execution note:** `RendererRegistry` Phase B1 is executed as part of [../2026-03-08_servoshell_debtclear_plan.md](../2026-03-08_servoshell_debtclear_plan.md) Phases 1–2. Sector B2/B3 remain follow-on registry work.

---

## Purpose

Sector B owns the complete dispatch pipeline from raw user input to authoritative state mutation:

```
InputEvent (key / gamepad / mouse)
 └─► InputRegistry          binding × context → ActionId
      └─► ActionRegistry    ActionId × payload → Vec<GraphIntent> | WorkbenchIntent
           └─► Reducer / Workbench Authority

NodeKey + PaneId
 └─► RendererRegistry       pane attachment table → accept/reject
      └─► reconcile_webview_lifecycle()   renderer created only after acceptance
```

`RendererRegistry` is the most urgent new registry in the codebase: it is the boundary object
required by debtclear Phase 1 and blocks servoshell structural inversion.

For sequencing, treat only Phase B1 as a debt-clear prerequisite. Completing
`InputRegistry` and `ActionRegistry` modernization is important, but it is not a
reason to pause debt-clear once the renderer boundary is landing through the
debt-clear plan itself.

---

## Current State

| Registry | Struct | Completeness | Key gaps |
|---|---|---|---|
| `InputRegistry` | ✅ | Partial | Only 4 keyboard bindings; no gamepad; no context; no chords; no runtime rebind |
| `ActionRegistry` | ✅ | Partial | Only 7 actions; no namespace enforcement; no capability guard; sync-only handlers |
| `RendererRegistry` | ❌ | Not started | Required by debtclear Phase 1 |

---

## Phase B1 — RendererRegistry (most urgent)

**Unlocks:** Servoshell debtclear Phase 1 done gate; creation inversion (Phase 2).

**Sequencing note:** This phase is intentionally duplicated with the debt-clear
plan's Phase 1B / Phase 2 work because it is the shared boundary slice. When in
doubt, implement it under the debt-clear plan and treat this section as the
registry-specific acceptance detail for the same work.

### B1.1 — Define `RendererRegistry` struct

The `RendererRegistry` maintains a bijective map: each `PaneId` has at most one active
`RendererId`; each `RendererId` is attached to exactly one `PaneId`.

```rust
pub struct PaneAttachment {
    pub pane_id: PaneId,
    pub renderer_id: RendererId,
    pub attached_at: Instant,
    pub node_key: Option<NodeKey>,  // which node this renderer is serving
}

pub struct RendererRegistry {
    by_pane: HashMap<PaneId, PaneAttachment>,
    by_renderer: HashMap<RendererId, PaneId>,
}

impl RendererRegistry {
    /// Accept a renderer for a pane. Returns Err if pane already has a renderer.
    pub fn accept(&mut self, pane: PaneId, renderer: RendererId, node: Option<NodeKey>)
        -> Result<(), RendererRegistryError>;

    /// Detach renderer from its pane. Must be called before the renderer is destroyed.
    pub fn detach(&mut self, renderer: RendererId) -> Option<PaneAttachment>;

    /// Look up which renderer is currently serving a pane.
    pub fn renderer_for_pane(&self, pane: &PaneId) -> Option<&PaneAttachment>;

    /// Look up which pane a renderer is attached to.
    pub fn pane_for_renderer(&self, renderer: &RendererId) -> Option<PaneId>;
}
```

**Done gates:**
- [ ] `RendererRegistry` struct in `shell/desktop/runtime/registries/renderer.rs`.
- [ ] `accept()`, `detach()`, `renderer_for_pane()`, `pane_for_renderer()` implemented.
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_RENDERER_ATTACH` and `DIAG_RENDERER_DETACH` channels registered (Severity::Info).
- [ ] Unit tests: accept, detach, double-accept error, lookup consistency.

### B1.2 — Gate renderer creation on `RendererRegistry::accept()`

Per debtclear plan Phase 2A, no renderer may be created in `reconcile_webview_lifecycle()` unless
the registry has accepted the pane attachment first. The acceptance must come from the workbench
authority's handling of an open request, not from shell code.

Flow:
1. Workbench authority receives `WorkbenchIntent::OpenPane { node_key }`.
2. It calls `registries.renderer.accept(pane_id, renderer_id, Some(node_key))`.
3. Only after `accept()` succeeds does `reconcile_webview_lifecycle()` create the renderer.
4. If `accept()` fails (duplicate), `reconcile_webview_lifecycle()` does not create and emits
   `DIAG_RENDERER_ATTACH` at `Warn` severity.

**Done gates:**
- [ ] `reconcile_webview_lifecycle()` checks `RendererRegistry::renderer_for_pane()` before any
  renderer creation.
- [ ] Debtclear Phase 1 acceptance criterion: no shell code creates renderers outside
  `reconcile_webview_lifecycle()`.
- [ ] `log::warn!` emitted on any attempt to create without prior acceptance.
- [ ] Scenario test: Ctrl+T open flow creates renderer only after registry accepts.

### B1.3 — Detach on close and wire diagnostics

When a pane is closed:
1. `WorkbenchIntent::ClosePane` triggers `RendererRegistry::detach()`.
2. `reconcile_webview_lifecycle()` destroys the renderer.
3. `DIAG_RENDERER_DETACH` emits.

**Done gates:**
- [ ] Detach called from workbench close path before reconcile destroys renderer.
- [ ] No orphaned `PaneAttachment` entries remain after all panes are closed.
- [ ] Test: open + close round-trip leaves registry empty.

---

## Phase B2 — InputRegistry: Gamepad, context, and chords

**Unlocks:** Gamepad radial menu dispatch (CLAUDE.md directive); full input parity.

### B2.1 — Typed `InputBinding` and modifier representation

Replace the flat string key with a typed binding:

```rust
pub enum InputBinding {
    Key { modifiers: ModifierMask, keycode: Keycode },
    Gamepad { button: GamepadButton, modifier: Option<GamepadButton> },
    Chord(Vec<InputBinding>),       // sequential input sequence
}

pub struct ModifierMask(u8);  // bit flags for Ctrl/Shift/Alt/Super

pub enum Keycode {
    Named(NamedKey),    // Enter, Escape, Arrow*, F1–F12, etc.
    Char(char),
}
```

Existing 4 bindings (`TOOLBAR_SUBMIT`, `NAV_BACK`, `NAV_FORWARD`, `NAV_RELOAD`) are re-expressed
as `InputBinding::Key` values with appropriate modifier masks.

**Done gates:**
- [ ] `InputBinding` enum defined.
- [ ] Existing bindings migrated; no regressions in unit tests.
- [ ] `register_binding(binding: InputBinding, action_id: ActionId, context: InputContext)`.

### B2.2 — Context-aware resolution

The `input_registry_spec.md`'s `context-resolution` policy requires that the same physical input
can resolve to different actions depending on active context.

```rust
pub enum InputContext {
    GraphView,
    DetailView,
    OmnibarOpen,
    RadialMenuOpen,
    DialogOpen,
}

pub fn resolve(&self, binding: &InputBinding, context: InputContext)
    -> InputBindingResolution
```

**Done gates:**
- [ ] `resolve()` signature updated to include `InputContext`.
- [ ] `Enter` in `OmnibarOpen` → omnibar submit; `Enter` in `GraphView` → graph node confirm.
- [ ] Conflict detection: two actions bound to same (binding, context) pair emits `Warn` diagnostic.
- [ ] Unit tests for each context variant.

### B2.3 — Gamepad bindings

Per CLAUDE.md: radial menu default in Gamepad mode; D-pad/stick navigation; 8-sector; no
concentric rings; both menus work in both input modes; all routes through ActionRegistry.

Register built-in gamepad bindings:

| Gamepad input | Context | Action |
|---|---|---|
| Right shoulder | GraphView | `radial_menu:open` |
| D-pad directions (×8) | RadialMenuOpen | `radial_menu:select_sector_{0–7}` |
| Left stick press | RadialMenuOpen | `radial_menu:confirm` |
| B / Circle | RadialMenuOpen | `radial_menu:cancel` |
| Left bumper | GraphView | `graph:navigate_back` |
| Right bumper | GraphView | `graph:navigate_forward` |
| Start | GraphView | `workbench:command_palette_open` |

**Done gates:**
- [ ] All gamepad bindings registered in `InputRegistry::new()`.
- [ ] Gamepad events from `EmbedderWindow` input handler are converted to `InputBinding::Gamepad`
  and routed through `InputRegistry::resolve()`.
- [ ] Unit tests: each gamepad binding resolves to expected action in expected context.

### B2.4 — Runtime rebinding

The spec requires that bindings can be remapped at runtime (user preferences, mod-provided profiles).

```rust
pub fn remap_binding(
    &mut self,
    old: InputBinding,
    new: InputBinding,
    context: InputContext,
) -> Result<(), InputConflict>
```

Persisted to user preferences via a `GraphIntent` carrier after rebind.

**Done gates:**
- [ ] `remap_binding()` implemented with conflict detection.
- [ ] Rebind emits `DIAG_INPUT_BINDING` at Info severity.
- [ ] User preferences round-trip: rebind → persist → restore on restart.

---

## Phase B3 — ActionRegistry: Namespace, capability, and action families

**Unlocks:** Correct `namespace:name` key discipline (CLAUDE.md); undoable actions; graph and
workbench action families.

### B3.1 — Enforce `namespace:name` key format

Per CLAUDE.md: "New registry keys must follow the `namespace:name` pattern."

```rust
pub fn register_action(&mut self, id: &str, handler: ActionHandler) {
    if !id.contains(':') {
        log::warn!("action_registry: key {:?} does not follow namespace:name format", id);
    }
    // ...
}
```

Existing action IDs are already `namespace:name` format but should be validated consistently.

**Done gates:**
- [ ] `register_action()` emits `log::warn!` for non-conforming keys.
- [ ] All existing action ID constants conform (audit + fix).

### B3.2 — Actions emit intents, not direct state mutation

The `action_registry_spec.md`'s `intent-emission` policy: actions return `Vec<GraphIntent>` or
`WorkbenchIntent`, they do not mutate state directly.

Current `execute_graph_view_submit_action()` and similar handlers call into `graph_app` state
directly in some paths. Refactor all handlers to return intents:

```rust
pub fn execute(&self, action_id: &str, payload: ActionPayload, context: &ActionContext)
    -> ActionOutcome

pub enum ActionOutcome {
    Intents(Vec<GraphIntent>),
    WorkbenchIntent(WorkbenchIntent),
    SignalEmit(SignalEnvelope),
    Failure(ActionFailure),    // never silent noop
}
```

**Done gates:**
- [ ] All 7 existing handlers refactored to return `ActionOutcome`.
- [ ] No handler directly mutates `GraphBrowserApp` fields.
- [ ] `ActionOutcome::Failure` emits `DIAG_ACTION_EXECUTE` at `Error` severity.
- [ ] Unit tests confirm handler return shapes.

### B3.3 — Graph action family

Register canonical graph actions:

| Action ID | Payload | Emits |
|---|---|---|
| `graph:node_open` | `{ node_key, pane_id? }` | `GraphIntent::ActivateNode` |
| `graph:node_close` | `{ node_key }` | `GraphIntent::DeactivateNode` |
| `graph:edge_create` | `{ from, to, label? }` | `GraphIntent::AddEdge` |
| `graph:navigate_back` | — | `GraphIntent::TraverseBack` |
| `graph:navigate_forward` | — | `GraphIntent::TraverseForward` |
| `graph:select_node` | `{ node_key }` | `GraphIntent::SelectNode` |
| `graph:deselect_all` | — | `GraphIntent::DeselectAll` |

**Done gates:**
- [ ] All 7 graph actions registered with intent-returning handlers.
- [ ] `graph:navigate_back` / `forward` replace any hardcoded navigation calls.

### B3.4 — Workbench action family

Register canonical workbench actions:

| Action ID | Payload | Emits |
|---|---|---|
| `workbench:split_horizontal` | `{ pane_id }` | `WorkbenchIntent::SplitPane(Horizontal)` |
| `workbench:split_vertical` | `{ pane_id }` | `WorkbenchIntent::SplitPane(Vertical)` |
| `workbench:close_pane` | `{ pane_id }` | `WorkbenchIntent::ClosePane` |
| `workbench:command_palette_open` | — | `WorkbenchIntent::OpenToolPane(CommandPalette)` |
| `workbench:settings_open` | — | `WorkbenchIntent::OpenToolPane(Settings)` |

**Done gates:**
- [ ] All 5 workbench actions registered.
- [ ] Workbench intents are routed to the workbench authority, not the graph reducer.
- [ ] `log::warn!` emitted if a workbench intent is mistakenly sent to `apply_reducer_intents()`.
  (This is the SYSTEM_REGISTER "silent no-op" gap fix.)

### B3.5 — Capability guard

Each action descriptor carries a capability requirement. `execute()` checks the caller's capability
token before dispatching.

```rust
pub struct ActionDescriptor {
    pub id: String,
    pub required_capability: Option<ActionCapability>,
    pub handler: ActionHandler,
}

pub enum ActionCapability {
    AlwaysAvailable,
    RequiresActiveNode,
    RequiresSelection,
    RequiresWritableWorkspace,
}
```

`describe_action(id) -> ActionCapability` is exposed through `RegistryRuntime`.

**Done gates:**
- [ ] `ActionDescriptor` defined; all existing actions annotated with capability.
- [ ] `execute()` checks capability; unavailable action returns `ActionOutcome::Failure`.
- [ ] `describe_action()` exposed on `RegistryRuntime`.

---

## Acceptance Criteria (Sector B complete)

- [ ] `RendererRegistry` enforces the creation boundary: debtclear Phase 1 acceptance criterion met.
- [ ] `InputRegistry` resolves gamepad bindings with same contract as keyboard bindings.
- [ ] Radial menu sectors are bound to `radial_menu:select_sector_N` actions in gamepad context.
- [ ] All action IDs follow `namespace:name` format.
- [ ] Actions return intents, not direct mutations; workbench intents route to workbench authority.
- [ ] `log::warn!` fires when workbench-authority intents reach `apply_reducer_intents()`.
- [ ] Graph and workbench action families are registered and tested.
- [ ] Runtime rebinding works and persists through user preferences.

---

## Related Documents

- [input_registry_spec.md](input_registry_spec.md)
- [action_registry_spec.md](action_registry_spec.md)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — two-authority model
- [../2026-03-08_servoshell_debtclear_plan.md](../2026-03-08_servoshell_debtclear_plan.md) — RendererRegistry requirement
- [../2026-02-24_control_ui_ux_plan.md](../2026-02-24_control_ui_ux_plan.md) — gamepad/radial menu spec
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
