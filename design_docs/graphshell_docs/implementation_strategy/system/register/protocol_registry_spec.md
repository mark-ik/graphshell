# Protocol Registry Spec

**Doc role:** Canonical registry spec for `protocol_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)
- [../../../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md](../../../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md) (canonical protocol packaging and host-capability model)

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
- non-engine social/collaboration/storage systems unless they are explicitly
  exposed as receivable protocol handlers

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve(uri) -> Result<ContentStream>`
- `can_handle(scheme) -> bool`
- `describe_capability(id) -> ProtocolCapability`

`ProtocolCapability` is the future source of truth for protocol packaging and
host availability. At minimum, each capability description must document:

- `scheme_or_family`
- `lane_kind` (`document`, `discovery`, `mutation`, `collaboration`, `storage_replication`)
- `packaging_class` (`core_builtin`, `default_portable`, `optional_portable`, `native_only`, `non_engine_layer`)
- `host_support_profile`
- `requires_capabilities`
- `degradation_mode`

## Normative Core

- Protocol resolution is deterministic and explicit; unknown schemes fail with diagnostics, not silent fallback.
- Core seeds keep the app functional offline without mods.
- Protocol handlers produce content streams or structured load failures; they do not choose viewers.
- Not every protocol in the Graphshell ecosystem belongs in the middlenet
  engine or in this registry as a first-class handler. Identity fabrics,
  collaboration fabrics, and storage/replication systems remain outside this
  registry unless a specific receivable handler contract is adopted.

## Planned Extensions

- Explicit host-support and degradation metadata on `ProtocolCapability`
- Receivable peer/content handlers where explicitly adopted (`ipfs://`,
  `ipns://`, `magnet:`, `verse://`, `did:` are examples of possible future
  candidates, not automatic middlenet-engine members)
- richer capability diagnostics and trust policy hooks

## Prospective Capabilities

- protocol-scoped access policies
- streaming content transforms and staged verification

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
