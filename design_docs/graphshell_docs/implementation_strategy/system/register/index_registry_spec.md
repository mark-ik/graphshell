# Index Registry Spec

**Doc role:** Canonical registry spec for `index_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Defines search backends for local lookup, retrieval, and timeline/history queries.

In scope:
- search provider registration
- query fanout and result contracts
- history/timeline retrieval backends

Out of scope:
- command surface UX
- viewer rendering
- storage formats beyond provider contracts

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `query(text) -> Iterator<Result>`
- `describe_backend(id) -> IndexCapability`
- `suggest(scope, text) -> Iterator<Suggestion>`

## Normative Core

- Index queries return structured results with explicit source metadata.
- Local search must function without network providers.
- History retrieval is a first-class index capability, not a side channel.

## Planned Extensions

- federated and peer-backed providers
- result ranking policy profiles

## Prospective Capabilities

- semantic retrieval fusion
- background index health scoring

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
