# Workflow Registry Spec

**Doc role:** Canonical registry spec for `workflow_registry`.
**Status:** Active / canonical
**Kind:** Future domain registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Defines full session modes as Lens x WorkbenchProfile compositions.

In scope:
- workflow activation and naming contracts
- composition of lens + input + workbench profiles
- session-mode capability metadata

Out of scope:
- individual action semantics
- raw registry provider behavior
- direct reducer mutations

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_workflow(id) -> WorkflowProfile`
- `activate(id) -> WorkflowActivation`
- `describe_workflow(id) -> WorkflowCapability`

## Normative Core

- Workflow is future-facing but should remain a clean composition boundary.
- Workflow composes existing registry outputs instead of inventing a parallel semantics layer.
- Session-mode changes must remain explicit and diagnosable.

## Planned Extensions

- named workflow presets
- workflow-aware resource and input profiles

## Prospective Capabilities

- user-authored workflows
- collaborative/shared session modes

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
