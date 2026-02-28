# Security and Access Control Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_SECURITY.md`
- `../subsystem_storage/storage_and_persistence_integrity_spec.md`
- `../subsystem_diagnostics/diagnostics_observability_and_harness_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **Security & Access Control subsystem**.

It governs:

- identity integrity
- trust establishment and revocation
- grant enforcement
- cryptographic correctness
- mod capability restrictions

---

## 2. Canonical Model

Security is a cross-cutting runtime guarantee.

It is not owned by a single transport or sync backend. It must remain valid across:

- local behavior
- Verse sync
- mod loading
- persistence and key management

---

## 3. Normative Core

### 3.1 Identity Integrity

- There is one canonical local identity source unless explicit user action changes it.
- Secret material remains keychain-only.
- Signed payloads are verified before state mutation.
- Identity degradation must be explicit.

### 3.2 Trust Boundaries

- No peer is trusted implicitly.
- Trust store persistence must be deterministic.
- Role and grant escalation require explicit user action.
- Revocation must fully remove trust effects.

### 3.3 Grant Enforcement

- Inbound and outbound workspace access must respect the grant matrix.
- Read-only peers must not mutate graph state.
- New mutating intents must not bypass access classification.

### 3.4 Cryptographic Correctness

- Transport and at-rest encryption requirements must remain explicit.
- Nonce reuse is forbidden.
- Verification and decryption failures must be explicit.

### 3.5 Mod Boundaries

- WASM mods remain capability-restricted.
- Native mods require explicit review and documentation of security surface.
- Namespace and capability declarations must be enforced.

---

## 4. Planned Extensions

- stronger grant-matrix compile-time enforcement
- richer trust and revocation UI
- deeper mod capability inspection and reporting

---

## 5. Prospective Capabilities

- finer-grained workspace sharing policies
- richer peer trust models
- policy-driven security posture presets

---

## 6. Acceptance Criteria

- Core identity, trust, and grant invariants are explicit and tested or diagnosed.
- Security failures do not degrade silently.
- Security boundaries remain independent of any single feature path or backend.

