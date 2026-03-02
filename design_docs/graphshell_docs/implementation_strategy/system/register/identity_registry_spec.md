# Identity Registry Spec

**Doc role:** Canonical registry spec for `identity_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `identity_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Identity-integrity policy**: Persona/key resolution must be explicit and deterministic.
2. **Trust-boundary policy**: Trust decisions and key availability failures must be explicit and diagnosable.
3. **Crypto-operation policy**: Signing/verification paths are contract-first and cannot be silently bypassed.
4. **Fallback-safety policy**: Any fallback behavior must preserve security posture and emit explicit diagnostics.

## Purpose and Scope

Owns persona, signing, and local/peer identity capability contracts.

In scope:
- persona/key registration
- signing and verification interfaces
- identity capability metadata

Out of scope:
- protocol transport
- mod lifecycle
- UI command semantics

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `sign(payload, persona)`
- `verify(payload, signature)`
- `describe_identity(id) -> IdentityCapability`

## Normative Core

- Local identity must function offline.
- Security-root operations are explicit and diagnosable.
- Identity providers expose trust boundaries clearly.

## Planned Extensions

- peer trust providers
- Verse-aligned distributed identity integrations

## Prospective Capabilities

- delegated personas and scoped credentials
- cross-device identity federation

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
