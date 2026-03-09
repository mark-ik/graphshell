# Unified Security Architecture Plan

**Date**: 2026-03-08  
**Status**: Active / architecture cleanup plan  
**Scope**: Reconcile the canonical Security subsystem contract with the actual runtime split across Verse, persistence, registries, mods, and host protocol boundaries.

**Related**:
- `SUBSYSTEM_SECURITY.md`
- `security_and_access_control_spec.md`
- `../subsystem_storage/storage_and_persistence_integrity_spec.md`
- `../subsystem_mods/SUBSYSTEM_MODS.md`
- `../system/2026-03-08_servoshell_residue_audit.md`

---

## 1. Why This Plan Exists

The current Security subsystem docs correctly describe the desired guarantees, but they flatten several different security concerns into one top-level model. In code, those concerns are split across:

- Verse identity, trust, and sync access control
- persistence encryption and key management
- mod capability loading
- host protocol and content boundary hardening
- emerging Nostr capability enforcement

That split is not itself a problem. The problem is that the authority boundaries between those tracks are still implicit, partially duplicated, or inconsistent with the subsystem docs.

This plan defines the missing top-level structure so implementation work can converge on one coherent security architecture instead of accumulating point fixes.

---

## 2. Security Taxonomy

Security is not one mechanism. For Graphshell, it should be treated as five coordinated tracks:

1. `IdentitySecurity`
2. `TrustAndGrantSecurity`
3. `PersistenceCryptoSecurity`
4. `ModCapabilitySecurity`
5. `HostContentBoundarySecurity`

`NostrSecurity` is currently a provider-specific profile spanning tracks 1, 4, and 5. It should not be modeled as a sixth root track.

---

## 3. Canonical Tracks

### 3.1 `IdentitySecurity`

Owns:

- canonical local identity roots
- signing and verification boundaries
- keychain availability/degradation
- relationship between local P2P identity, provider identities, and future delegated signers

Canonical authorities:

- `IdentityRegistry` is the authority surface
- concrete providers such as Verse P2P identity or future Nostr signers plug into that surface

Required cleanup:

- stop treating the current in-memory fallback signer as a production authority
- make provider-backed signing and verification the canonical path
- define whether Graphshell has one primary local identity with provider-specific projections, or multiple first-class local identities

### 3.2 `TrustAndGrantSecurity`

Owns:

- trusted-peer lifecycle
- pairing and revocation semantics
- workspace grant matrix
- inbound and outbound sync authorization
- remote mutation acceptance rules

Canonical authorities:

- trust store is the durable source of truth
- grant acceptance occurs when remote operations are admitted into durable app/workspace state
- transport workers enforce, but do not define, grant policy

Required cleanup:

- move from Verse-local trust assumptions to an explicit grant-acceptance boundary
- define how `GraphIntent`, runtime events, remote delta application, and future workbench routing participate in authorization
- ensure outbound sync honors grants, not just inbound sync

### 3.3 `PersistenceCryptoSecurity`

Owns:

- at-rest encryption requirements
- persistence-key derivation and storage
- nonce generation and integrity verification
- corruption detection and recovery semantics

Canonical authorities:

- persistence layer owns storage encryption for graph/workspace/state data
- provider-specific journals such as Verse sync logs may use provider-local wrappers, but must conform to subsystem crypto rules

Required cleanup:

- document the difference between graph persistence encryption and Verse sync-log encryption
- replace ad hoc provider-local derivation notes with one canonical key-derivation policy
- align corruption and degraded-mode behavior with the Storage subsystem

### 3.4 `ModCapabilitySecurity`

Owns:

- capability declarations
- grant checks for mod operations
- native versus WASM trust model
- namespace and host-interface enforcement

Canonical authorities:

- Mods subsystem owns lifecycle
- Security subsystem owns capability policy
- mod loader enforces the policy at activation and call boundaries

Required cleanup:

- distinguish “capability availability” from “security approval”
- define the real runtime model for WASM mods instead of assuming `extism` is already present
- make native-mod security review an explicit documented gate, not a narrative expectation

### 3.5 `HostContentBoundarySecurity`

Owns:

- protocol handler restrictions
- WebView/content-to-host privilege boundaries
- origin/referrer assumptions
- clipboard/file/resource bridge limits
- shell-side fallback behaviors that can accidentally bypass intended security posture

Canonical authorities:

- host/runtime boundary code owns enforcement
- security docs define the rules that those host boundaries must satisfy

Required cleanup:

- add this track explicitly to the subsystem model
- stop treating sync/grants as the whole of “security”
- align servoshell-debt cleanup with a host/content threat model

---

## 4. Current Implementation Snapshot

### 4.1 Landed

- Graph/workspace persistence encryption is implemented and tested.
- Verse local identity is keychain-backed.
- Inbound Verse workspace grant enforcement exists.
- Trust add/revoke and workspace grant/revoke flows exist.
- `resource://` path and referrer hardening exists.
- Nostr capability-gated scaffold behavior exists.

### 4.2 Partial / Inconsistent

- identity authority is split between a registry scaffold and Verse runtime
- diagnostics naming is split between `registry.*`, `verse.sync.*`, and provider-specific namespaces
- trust-store persistence shape does not match canonical docs
- security degradation exists operationally but is not surfaced as a coherent subsystem health model
- Nostr capability enforcement is real, but provider transport and signer delegation are scaffold-only

### 4.3 Missing

- signature verification on the actual inbound remote-sync application path
- outbound grant filtering
- canonical trust-store corruption recovery flow
- compile-time or authoritative runtime grant classification coverage for mutating operations
- real WASM capability enforcement path
- unified host/content boundary contract

---

## 5. Architectural Corrections

### 5.1 Collapse to One Identity Authority Surface

The app should expose one canonical identity authority surface, with provider adapters underneath it.

This means:

- `IdentityRegistry` becomes a real provider-backed facade, not a separate fake authority
- Verse P2P identity becomes an identity provider, not a parallel root
- future Nostr signing becomes another provider on the same authority model

### 5.2 Separate Authorization From Transport

Grant checks should not be defined by transport workers alone.

The model should be:

1. transport receives remote proposal
2. trust/grant policy evaluates proposal against durable authorization state
3. accepted remote changes are admitted into app/workspace state
4. reducers/reconcile apply effects

This is the missing bridge between the current Verse-centric checks and the broader app intent model.

### 5.3 Unify Diagnostics Taxonomy

Adopt one explicit mapping:

- subsystem-level channels: `security.identity.*`, `security.trust.*`, `security.crypto.*`, `security.mod.*`, `security.host.*`
- provider-specific detail channels remain allowed, but must map cleanly to subsystem categories

Existing `registry.*` and `verse.sync.*` channels can remain during transition, but the plan should treat them as migration surfaces, not final taxonomy.

### 5.4 Add Host/Content Security As A First-Class Security Track

Security planning should explicitly include:

- protocol allow/deny rules
- bridge permissions
- clipboard and filesystem exposure
- content-triggered host actions
- renderer/process fallback semantics

Without this, the subsystem remains skewed toward sync and crypto while ignoring browser-shell risks.

---

## 6. Sequencing Plan

### Phase A. Authority And Taxonomy Cleanup

1. Write a short canonical addendum defining the five security tracks and their authority boundaries.
2. Define the canonical identity authority surface and provider model.
3. Define the diagnostics taxonomy and migration mapping.

Done-gate:

- subsystem docs no longer imply one flat security mechanism
- every existing security doc can be placed under one of the five tracks

### Phase B. Identity And Trust Normalization

1. Rework `IdentityRegistry` into a provider-backed authority surface.
2. Move Verse identity and trust-store behavior behind that authority.
3. Define trust-store persistence shape and corruption recovery contract.

Done-gate:

- no duplicate production identity authority remains
- trust-store load failure semantics are explicit and diagnosed

### Phase C. Authorization Closure

1. Define grant classification ownership for mutating operations and remote deltas.
2. Enforce outbound as well as inbound grant filtering.
3. Wire signature verification into the actual inbound remote-application path.

Done-gate:

- remote sync admission is verified, authorized, and diagnosable end-to-end

### Phase D. Mod And Provider Security Closure

1. Document the current native-mod trust model precisely.
2. Either implement the real WASM sandbox path or downgrade the subsystem docs so they stop implying it is already active.
3. Align Nostr provider capability checks with the subsystem taxonomy.

Done-gate:

- mod capability policy matches the actual loader/runtime model

### Phase E. Host/Content Boundary Hardening

1. Write a host/content security contract doc.
2. Audit protocol handlers, WebView bridges, clipboard/file/resource boundaries, and shell fallbacks against that contract.
3. Link this phase to servoshell-debt cleanup so authority and security moves reinforce each other.

Done-gate:

- host/content risks are part of subsystem security, not undocumented shell behavior

---

## 7. Cross-Plan Dependencies

- `SUBSYSTEM_STORAGE.md` and storage integrity specs must align with `PersistenceCryptoSecurity`.
- `SUBSYSTEM_MODS.md` and register/mod lifecycle specs must align with `ModCapabilitySecurity`.
- servoshell debt-clearing and host authority cleanup must align with `HostContentBoundarySecurity`.
- `graphshell_core` extraction should not freeze identity/grant semantics before phases B and C are clarified.

This is not a separate side plan. It is a boundary-setting plan that should inform security-related slices across storage, mods, sync, and shell cleanup.

---

## 8. Recommended Immediate Actions

1. Update `SUBSYSTEM_SECURITY.md` so it explicitly references this plan and stops overstating current cohesion.
2. Add a short `Identity authority gap` section to the security subsystem guide.
3. Add a short `Host/content boundary gap` section to the security subsystem guide.
4. Open implementation follow-ons for:
   - outbound grant filtering
   - inbound signature verification on sync application
   - trust-store corruption detection/recovery
   - diagnostics taxonomy migration

---

## 9. Done Definition

The unified security architecture is in place when:

- all security work can be placed under one of the five canonical tracks
- one canonical identity authority surface exists
- trust/grant acceptance boundaries are explicit and used on real runtime paths
- persistence and provider crypto rules are aligned
- mod capability policy matches actual runtime enforcement
- host/content boundary rules are documented and audited
- subsystem diagnostics describe security posture coherently instead of by incidental implementation namespace
