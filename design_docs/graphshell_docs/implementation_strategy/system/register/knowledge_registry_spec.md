# Knowledge Registry Spec

**Doc role:** Canonical registry spec for `knowledge_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `knowledge_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Semantic-validation policy**: Tag/taxonomy validation must be explicit and provider-routed.
2. **Seed-floor policy**: Core semantic seed floor remains available offline.
3. **Advisory-semantic policy**: Knowledge hints inform other systems but do not override their mutation authority.
4. **Schema-evolution policy**: Provider/schema changes require explicit compatibility behavior and diagnostics.

## Purpose and Scope

Owns semantic tagging, taxonomy validation, and ontology-derived hints.

In scope:
- taxonomy/tag provider routing
- semantic validation and distance contracts
- label and color hint resolution

Out of scope:
- graph layout or rendering policy
- viewer choice
- search backend ranking

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `validate(tag)`
- `distance(a, b)`
- `get_label(code)`
- `get_color_hint(code)`

## Normative Core

- UDC defaults form the offline seed floor.
- Knowledge validation is explicit and provider-routed.
- Semantic hints inform other systems but do not override their authority.

## Planned Extensions

- additional schema providers
- stronger semantic indexing hooks

## Prospective Capabilities

- cross-schema reconciliation
- knowledge-driven workflow suggestions

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
