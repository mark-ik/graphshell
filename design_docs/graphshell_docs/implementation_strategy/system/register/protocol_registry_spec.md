# Protocol Registry Spec

**Doc role:** Canonical registry spec for `protocol_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `protocol_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md)):
- **RFC 3986** — all URI scheme resolution follows RFC 3986 syntax; `resolve(uri)` must parse per RFC 3986 §3
- **OSGi R8** — capability registration, handler lookup, and conflict-resolution vocabulary
- **OpenTelemetry Semantic Conventions** — diagnostic channel naming/severity for resolution failures

**Referenced as prior art** (no conformance obligation):
- **ActivityPub** — not an adopted Verse protocol; `activitypub://` is not a planned handler scheme (see §Planned Extensions below)

## Registry Policies

1. **Protocol-resolution policy**: URI/protocol resolution follows explicit handler lookup and fallback contracts.
2. **Non-blocking-resolution policy**: Protocol paths should preserve responsiveness and avoid hidden blocking behavior.
3. **Capability-boundary policy**: Protocol providers operate within declared capability/trust boundaries.
4. **Failure-visibility policy**: Resolve failures and fallback paths must be explicit and diagnosable.

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

- Verse/peer protocol providers (`ipfs://`, `verse://`, `did:` scheme handling)
- Note: `activitypub://` is not a planned handler — ActivityPub is reference-only, not an adopted Verse standard
- richer capability diagnostics and trust policy hooks

## Prospective Capabilities

- protocol-scoped access policies
- streaming content transforms and staged verification

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
