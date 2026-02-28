# Input Registry Spec

**Doc role:** Canonical registry spec for `input_registry`.
**Status:** Active / canonical
**Kind:** Domain registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Maps input events to canonical actions and profiles.

In scope:
- keybind/mousebind profile registration
- input event to `ActionId` mapping
- input profile capability metadata

Out of scope:
- action semantics
- widget-local fallback behavior
- graph/workbench authority rules

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve(event) -> Option<ActionId>`
- `describe_profile(id) -> InputCapability`
- `list_bindings(profile) -> Iterator<Binding>`

## Normative Core

- Input maps to canonical actions, not ad hoc widget behavior.
- One semantic authority per behavior remains the governing rule.
- Profile lookup and conflicts are explicit.

## Planned Extensions

- mode-specific input profiles
- user-remappable binding sets

## Prospective Capabilities

- context-adaptive profiles
- device-specific input packs

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
