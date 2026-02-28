# Action Registry Spec

**Doc role:** Canonical registry spec for `action_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Registers canonical executable actions and their handler contracts.

In scope:
- action definition and lookup
- action handler capability metadata
- command execution entry points

Out of scope:
- command surface presentation
- input bindings
- tile-tree ownership

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `execute(context) -> Vec<GraphIntent>`
- `describe_action(id) -> ActionCapability`
- `list_actions(scope) -> Iterator<ActionId>`

## Normative Core

- The same action means the same thing across keyboard, palette, radial, and context surfaces.
- Actions emit intents or explicit failures; they do not silently noop.
- Action identity is stable and canonical.

## Planned Extensions

- parameterized action descriptors
- richer enablement and availability metadata

## Prospective Capabilities

- transactional multi-action bundles
- user-defined action macros

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
