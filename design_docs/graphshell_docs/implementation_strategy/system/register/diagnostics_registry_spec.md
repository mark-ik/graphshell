# Diagnostics Registry Spec

**Doc role:** Canonical registry spec for `diagnostics_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

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
