# Agent Registry Spec

**Doc role:** Canonical registry spec for `agent_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Defines background/autonomous tasks and their scheduling contracts.

In scope:
- agent registration
- spawn and schedule contracts
- agent capability metadata

Out of scope:
- control panel worker supervision itself
- core reducer semantics
- command surface UI

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `spawn(context)`
- `schedule(cron)`
- `describe_agent(id) -> AgentCapability`

## Normative Core

- Agents are explicit background capabilities, not hidden threads.
- Agent work crosses into app state through supervised intents.
- Failures and backpressure surface through diagnostics.

## Planned Extensions

- policy-driven scheduling classes
- resource budgeting and pausing

## Prospective Capabilities

- user-authored agents
- cross-mod coordination policies

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
