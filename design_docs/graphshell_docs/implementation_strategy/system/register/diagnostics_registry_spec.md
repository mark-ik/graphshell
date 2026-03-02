# Diagnostics Registry Spec

**Doc role:** Canonical registry spec for `diagnostics_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `diagnostics_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Schema-authority policy**: Channel schema, severity, and invariants are declared registry contracts.
2. **Non-silent-orphan policy**: Unknown/orphan channel registration must be observable and auditable.
3. **Invariant-visibility policy**: Watchdog violations and pending states must be surfaced to diagnostics state/consumers.
4. **Config-roundtrip policy**: Runtime channel configuration changes must be explicit, persistent-capable, and testable.

## Policy-to-Contract Traceability

| Policy | Contract anchors in this doc | Stability implication |
|---|---|---|
| Schema-authority policy | **Normative Core**: "Diagnostics channels are first-class contracts with versioned payloads."; **Purpose and Scope**: schema/version contracts; **Acceptance Criteria**: contract tests + register diagnostics contract | Prevents drift in channel shape/severity/invariant semantics over time |
| Non-silent-orphan policy | **Normative Core**: explicit blocked/deferred/failure signaling; **Purpose and Scope**: channel registration/configuration ownership; **Acceptance Criteria**: harness/scenario coverage of real behavior | Prevents silent channel fallthrough and undiscoverable observability gaps |
| Invariant-visibility policy | **Normative Core**: compatibility managed deliberately + explicit failure signaling; **Acceptance Criteria**: tests covering registration/lookup/fallback behavior | Keeps watchdog/invariant failures actionable rather than latent |
| Config-roundtrip policy | **Canonical interfaces**: `get_config` / `set_config`; **Purpose and Scope**: sampling/retention configuration; **Acceptance Criteria**: contract tests for behavior correctness | Ensures runtime config changes remain safe, reversible, and test-backed |

## Purpose and Scope

Defines channel names, payload schemas, sampling, and retention rules for diagnostics.

In scope:
- diagnostic channel registration
- schema/version contracts
- sampling and retention configuration

Out of scope:
- feature-specific business logic
- UI presentation of diagnostics
- test harness orchestration beyond schema contracts

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `register_channel(def)`
- `get_config(channel_id)`
- `set_config(channel_id, config)`

## Normative Core

- Diagnostics channels are first-class contracts with versioned payloads.
- Every registry emits explicit blocked/deferred/failure signals, not silent fallback.
- Schema compatibility is managed deliberately.

## Planned Extensions

- diagnostics UI integration surfaces
- aggregated counter/profile contracts

## Prospective Capabilities

- adaptive sampling policies
- remote diagnostics streams

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
