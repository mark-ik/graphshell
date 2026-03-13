# Action Registry Dispatch Contract Spec

**Date:** 2026-03-12  
**Status:** Canonical interaction contract  
**Kind:** Register contract / dispatch boundary

**Related docs:**

- [`action_registry_spec.md`](./action_registry_spec.md)
- [`input_registry_spec.md`](./input_registry_spec.md)
- [`../../aspect_command/command_surface_interaction_spec.md`](../../aspect_command/command_surface_interaction_spec.md)
- [`../../subsystem_focus/focus_and_region_navigation_spec.md`](../../subsystem_focus/focus_and_region_navigation_spec.md)
- [`../../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../../technical_architecture/2026-03-12_specification_coverage_register.md)

---

## 1. Purpose and Scope

This spec defines the concrete dispatch contract for `ActionRegistry`.

It governs:

- action identity and registration rules,
- capability gating and rejection behavior,
- payload validation,
- dispatch lanes and authority boundaries,
- unknown-action and invalid-payload failure behavior,
- the relationship between `ActionRegistry`, command surfaces, and `InputRegistry`.

It does not govern:

- command-surface ranking, layout, or presentation,
- keybinding resolution precedence,
- workbench tile mutation semantics,
- reducer implementation details of emitted intents.

---

## 2. Authority and Ownership

`ActionRegistry` is the canonical semantic command registry.

Ownership rules:

- `ActionRegistry` owns canonical action identity and dispatch semantics.
- command surfaces (`Command Palette`, `Context Palette`, `Radial Palette`, context menus) choose which actions to present, but they must not redefine what an action means.
- `InputRegistry` chooses how hardware input resolves to action invocation, but it must not redefine action meaning.
- reducers, workbench handlers, and runtime consumers own the realization of dispatched carriers after action execution.

Normative rule:

- the same action id must mean the same thing regardless of surface or input source.

---

## 3. Canonical Model

### 3.1 Action identity

An action is identified by a stable, case-insensitive namespaced id.

Contract:

- canonical ids use `namespace:name` form,
- ids are normalized to lowercase on registration and execution,
- ids that do not follow `namespace:name` are tolerated but diagnosable,
- action ids are semantic identities, not presentation labels.

Examples from current code:

- `graph:node_open`
- `workbench:command_palette_open`
- `omnibox:node_search`
- `verse:sync_now`

### 3.2 Registration contract

Registration is explicit.

Current carrier:

- `ActionRegistry::register(action_id, required_capability, handler)`

Current overwrite behavior:

- duplicate registration replaces the previous handler for the same normalized id.

Normative rule:

- registration order is not a user-facing priority mechanism,
- duplicate registration is deterministic last-writer-wins behavior until a richer conflict policy is introduced,
- duplicate replacement should remain diagnosable.

### 3.3 Descriptor model

Each action descriptor currently contains:

- `id`
- `required_capability`
- `handler`

This is intentionally minimal.

It does not yet include:

- surface visibility metadata,
- ranking metadata,
- contextual search keywords,
- localization/display labels,
- richer enablement predicates.

Those are future extensions and must not be inferred from the dispatch contract.

---

## 4. Dispatch Contract

### 4.1 Execution entry point

Current execution entry point:

- `execute(action_id, app, payload) -> ActionOutcome`

Execution sequence:

1. normalize the action id,
2. resolve the descriptor,
3. enforce required capability gating,
4. invoke the registered handler with the current `GraphBrowserApp` snapshot and payload,
5. return either an explicit dispatch bundle or an explicit failure.

### 4.2 Dispatch lanes

`ActionRegistry` does not mutate application state directly.

Current dispatch bundle:

- `GraphIntent`
- `WorkbenchIntent`
- `AppCommand`
- `RuntimeAction`

Normative rule:

- these are parallel dispatch lanes with distinct downstream authorities,
- action handlers may emit one or more items across these lanes,
- handlers must not silently mutate state outside these carriers.

Current downstream ownership:

- `GraphIntent` -> reducer/domain/runtime mutation path
- `WorkbenchIntent` -> workbench/focus/workbench-UI authority path
- `AppCommand` -> higher-level runtime/app orchestration path
- `RuntimeAction` -> runtime-owned side-effect path

### 4.3 Surface relationship

Command surfaces and input bindings are invocation clients, not semantic authorities.

Normative rule:

- a surface may filter which actions it presents,
- a surface may decide not to show an unavailable action,
- a surface must not reinterpret an action id or bypass capability semantics.

If a surface invokes an unavailable action anyway, `ActionRegistry` remains the final gate and must return an explicit rejection.

---

## 5. Capability Gating

### 5.1 Current capability set

Current capabilities:

- `AlwaysAvailable`
- `RequiresActiveNode`
- `RequiresSelection`
- `RequiresWritableWorkspace`

Current availability source:

- availability is evaluated against the current `GraphBrowserApp` state at execution time.

### 5.2 Current semantics

Current gating semantics in code:

- `RequiresActiveNode` -> `app.get_single_selected_node().is_some()`
- `RequiresSelection` -> `!app.focused_selection().is_empty()`
- `RequiresWritableWorkspace` -> currently always available because no explicit read-only mode exists yet

Normative rule:

- capability gating is coarse-grained and global unless a richer context model is added explicitly,
- `RequiresWritableWorkspace` is currently a forward-looking semantic category, not a real lock check,
- surfaces may pre-check capabilities through `describe_action`, but execution-time gating remains authoritative.

### 5.3 Rejection behavior

If a required capability is unavailable:

- execution returns `ActionOutcome::Failure`,
- failure kind is `Rejected`,
- the failure reason must explain the missing capability in plain terms.

This is not a silent no-op.

---

## 6. Payload Validation

Handlers are responsible for validating payload shape.

Current rule:

- every action handler must reject payloads that do not match its expected `ActionPayload` variant.

Failure behavior:

- invalid payload returns `ActionFailureKind::InvalidPayload`,
- the failure reason names the required payload shape.

Normative rule:

- payload mismatch is a caller error,
- handlers must not reinterpret one payload variant as another,
- handlers must not infer missing mandatory parameters from UI state unless that behavior is part of the explicit action contract.

---

## 7. Failure Model

### 7.1 Canonical failure kinds

Current failure kinds:

- `UnknownAction`
- `InvalidPayload`
- `Rejected`

### 7.2 Semantics

- `UnknownAction`: the normalized id is not registered.
- `InvalidPayload`: the action was resolved, but the caller supplied the wrong payload variant or malformed payload data.
- `Rejected`: the action is known, but current state or policy does not permit execution.

Normative rule:

- actions must fail explicitly; they must not silently disappear at execution time.

---

## 8. Context Filtering and Priority

This is the most important non-goal to state clearly:

- `ActionRegistry` does **not** currently own contextual ranking or surface-specific priority.

What it does own:

- coarse capability gating,
- canonical identity,
- dispatch semantics.

What it does **not** currently own:

- palette scoring,
- radial/context menu ordering,
- search ranking,
- per-surface suppression,
- multi-handler priority arbitration.

Therefore:

- any surface-level context filtering must be documented as a client policy layered on top of the registry,
- clients must not treat the current registry as if it already provided rich context availability semantics.

Future extension seam:

- if richer availability/context filtering is added, it must extend the descriptor model explicitly rather than emerge ad hoc in palette or radial code.

---

## 9. Diagnostics Contract

Minimum required diagnostics behavior:

- malformed or non-namespaced registration should be diagnosable,
- duplicate registration/replacement should be diagnosable,
- unknown action execution should be diagnosable,
- rejected execution should be diagnosable at least through explicit failure values,
- invalid payload should be diagnosable through explicit failure values.

Normative rule:

- diagnostics may be emitted by the registry or by calling surfaces,
- but explicit failure values are part of the dispatch contract and must remain authoritative.

---

## 10. Test Contract

Required coverage:

1. id normalization and lookup behavior,
2. non-namespaced id warning behavior,
3. duplicate registration replacement behavior,
4. `describe_action` returns the registered capability,
5. capability gating rejects unavailable actions,
6. invalid payload returns `InvalidPayload`,
7. unknown id returns `UnknownAction`,
8. dispatch bundle may legally contain any of the four output lanes,
9. at least one real command-surface/invocation path exercises registry execution end to end.

---

## 11. Acceptance Criteria

- [ ] `ActionRegistry` remains the canonical semantic authority for action ids and dispatch semantics.
- [ ] All action execution returns either explicit dispatch carriers or explicit failure.
- [ ] Capability gating remains centralized and test-covered.
- [ ] Payload validation remains explicit and test-covered.
- [ ] Command surfaces and input bindings are documented as clients of the registry, not alternate authorities.
- [ ] Future context/ranking extensions do not silently change the current dispatch contract.
