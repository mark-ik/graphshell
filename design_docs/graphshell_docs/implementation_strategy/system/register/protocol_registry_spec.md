# Protocol Registry Spec

**Doc role:** Canonical registry spec for `protocol_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Maps URI schemes to content handlers and load adapters.

In scope:
- scheme-to-handler resolution contracts
- core seed protocol floor (`file://`, `about:`)
- provider capability boundaries for local, web, and future P2P protocols

Out of scope:
- viewer selection
- document presentation policy
- tile-tree routing

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve(uri) -> Result<ContentStream>`
- `can_handle(scheme) -> bool`
- `describe_capability(id) -> ProtocolCapability`

## Normative Core

- Protocol resolution is deterministic and explicit; unknown schemes fail with diagnostics, not silent fallback.
- Core seeds keep the app functional offline without mods.
- Protocol handlers produce content streams or structured load failures; they do not choose viewers.

## Planned Extensions

- Verse/peer protocol providers (`ipfs://`, `activitypub://`)
- richer capability diagnostics and trust policy hooks

## Prospective Capabilities

- protocol-scoped access policies
- streaming content transforms and staged verification

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
