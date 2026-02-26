# Cross-Cutting Subsystem: Security & Access Control

**Status**: Active / Project Goal
**Subsystem label**: `security`
**Long form**: Security & Access Control Subsystem
**Scope**: Identity integrity, trust boundaries, workspace grant enforcement, cryptographic correctness, and denial-path observability — across local operations and Verse sync
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `diagnostics` (Diagnostics), `accessibility` (Accessibility), `storage` (Persistence & Data Integrity), `history` (Traversal & Temporal Integrity)
**Doc role**: Canonical subsystem implementation guide (summarizes guarantees/roadmap and links to Verse/registry details; avoid duplicating security contract prose across feature plans)
**Sources consolidated**:
- `2026-02-22_registry_layer_plan.md` Phase 5.5 (workspace access control spec)
- `../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md` §§2, 7, 9.5 (identity, trust, encryption, access control)
- `archive_docs/checkpoint_2026-02-24/2026-02-24_step5_5_workspace_access_control.md` (archived Phase 1 implementation status)
**Related**: `PLANNING_REGISTER.md` (P1/P2 priority tasks reference Phase 5.4/5.5 done gates)

---

## 1. Why This Exists

Security is not a feature of Verse. It is a **cross-cutting guarantee** that must hold regardless of which sync backend is active, which mods are loaded, or which protocol paths are exercised.

The dominant failure mode is **silent contract erosion**: a new sync path silently bypasses access checks, a new intent variant isn't covered by the grant matrix, encryption is accidentally skipped for a new persistence keyspace, or a mod loads without capability restriction. None of these produce visible errors. All produce silent security regressions.

Without subsystem-level treatment, every new `GraphIntent` variant, every Verse protocol change, every mod capability declaration, and every persistence path extension becomes an unaudited trust boundary crossing.

---

## 2. Subsystem Model (Four Layers)

| Layer | Security Instantiation |
|---|---|
| **Contracts** | Identity integrity, trust boundary, grant enforcement, cryptographic correctness, mod sandboxing — §3 |
| **Runtime State** | `IdentityRegistry` (keypair, trust store, peer sessions), `SyncWorker` (grant enforcement), `ModRegistry` (capability restrictions) |
| **Diagnostics** | `security.*` + `verse.sync.access_*` channel families — §5 |
| **Validation** | Contract tests (grant matrix, denial paths), harness scenarios (`verse_access_control`), boundary tests — §6 |

---

## 3. Required Invariants / Contracts

### 3.1 Identity Integrity Invariants

1. **Single identity source** — Each Graphshell instance has exactly one P2P identity (Ed25519 keypair). The `NodeId` (public key) is derived deterministically from the `SecretKey`. No second identity source may exist without explicit user action.
2. **Keychain-only storage** — The Ed25519 secret key is stored exclusively in the OS keychain (`keyring` crate). It is never written to disk in plaintext, never logged, and never included in diagnostic output.
3. **Signature verification completeness** — Every inbound signed payload (SyncUnit, pairing confirmation, peer assertion) is verified against the claimed `NodeId` before any state mutation. Verification failures are rejected and logged.
4. **Identity availability degradation** — If the keychain is unavailable (locked, missing, permission denied), the app starts in offline-only mode with explicit diagnostics. No silent fallback to unsigned operations.

### 3.2 Trust Boundary Invariants

1. **Explicit trust only** — No peer is trusted by default. Trust is established only through the pairing ceremony (code exchange, QR scan, or invite link) with explicit user confirmation.
2. **Trust store persistence** — The `TrustedPeer` set is persisted in `user_registries.json`. Load and save are deterministic round-trips. Corruption detection causes trust store reset with diagnostics (not silent fallback).
3. **Peer role enforcement** — `PeerRole::Self_` grants full read/write on all personal workspaces. `PeerRole::Friend` grants are per-workspace, per-permission. Role escalation requires explicit user action.
4. **Revocation completeness** — `ForgetDevice` removes all grants, all connection state, and all cached peer data. No residual trust survives revocation.

### 3.3 Grant Enforcement Invariants

1. **Inbound grant check** — Every inbound `SyncUnit` is checked against the grant matrix before processing. Non-granted workspace sync is rejected with `verse.sync.access_denied` and does not mutate graph state.
2. **Read-only enforcement** — Inbound mutating intents from `ReadOnly` peers are rejected. The rejection is deterministic (not timing-dependent) and logged.
3. **Outbound grant respect** — Outbound sync to a peer only includes workspaces for which the peer has an active grant. No workspace data leaks to ungranteed peers.
4. **Grant matrix completeness** — Every `GraphIntent` variant that modifies graph state has an associated access level requirement. New intent variants without grant classification are compile-time errors (or, at minimum, runtime rejections with Error-severity diagnostics).
5. **No implicit grant inheritance** — Adding a new workspace does not automatically share it with existing peers. Sharing requires explicit `GrantWorkspaceAccess` intent.

### 3.4 Cryptographic Correctness Invariants

1. **Transport encryption** — All Verse connections use Noise protocol via iroh QUIC. No plaintext transport path exists.
2. **At-rest encryption** — The `SyncLog` (per-workspace intent journal) is encrypted with AES-256-GCM. The encryption key is derived from the persistence key stored in the OS keychain.
3. **Nonce uniqueness** — AES-GCM nonces are never reused. Nonce generation uses `OsRng` or a deterministic counter that is never reset.
4. **Ciphertext integrity** — All decryption verifies the GCM authentication tag. Decryption failures produce explicit errors, not silent truncation or empty output.

### 3.5 Mod Sandboxing Invariants

1. **WASM capability restriction** — WASM mods run in `extism` sandbox with capability-restricted access. No filesystem, no network, no keychain access unless explicitly declared and granted.
2. **Native mod audit requirement** — Native mods (compiled into binary) are not sandboxed. Any new native mod requires explicit documentation of its security surface.
3. **Mod channel namespace enforcement** — Mods can only emit diagnostic channels in their declared namespace. Cross-namespace emission is rejected.
4. **Mod capability declaration** — `ModManifest.requires` declares all capabilities. Loading a mod that requires undeclared capabilities fails with `registry.mod.security_violation`.

---

## 4. Surface Capability Declarations (Folded Approach)

Security capability declarations are folded into the relevant registry entries:

### 4.1 Viewer/Surface Security Capabilities

Each viewer/surface declares:

```
transport_encryption: full | partial | none
payload_signing: full | partial | none
grant_awareness: full | partial | none   // Does the surface respect workspace grants?
sandbox_level: native | wasm | none
notes: String
```

### 4.2 Registry Integration

- `ViewerRegistry` entries: Servo (native, full transport via Noise), Wry (native, OS-level TLS), plaintext (no network).
- `ProtocolRegistry` entries: protocol handlers declare transport security properties.
- `ModRegistry` entries: sandbox level, declared requires/provides, capability verification status.

---

## 5. Diagnostics Integration

### 5.1 Required Diagnostic Channels

| Channel | Severity | Description |
|---|---|---|
| `security.identity.key_loaded` | Info | P2P keypair loaded from keychain |
| `security.identity.key_generated` | Info | New P2P keypair generated (first launch) |
| `security.identity.key_unavailable` | Error | Keychain access failed |
| `security.identity.sign_succeeded` | Info | Payload signed successfully |
| `security.identity.sign_failed` | Error | Signing failed |
| `security.identity.verify_succeeded` | Info | Peer signature verified |
| `security.identity.verify_failed` | Error | Peer signature verification failed |
| `security.trust.peer_added` | Info | New trusted peer added via pairing |
| `security.trust.peer_revoked` | Info | Peer trust revoked |
| `security.trust.store_load_failed` | Error | Trust store deserialization/integrity failure |
| `verse.sync.access_denied` | Error | Inbound sync for non-granted workspace rejected |
| `verse.sync.readonly_mutation_rejected` | Warn | ReadOnly peer attempted mutating intent |
| `verse.sync.grant_created` | Info | Workspace access granted to peer |
| `verse.sync.grant_revoked` | Info | Workspace access revoked from peer |
| `security.crypto.nonce_collision_prevented` | Error | Nonce reuse detected and prevented |
| `security.crypto.decryption_failed` | Error | AES-GCM decryption or tag verification failed |
| `security.mod.capability_violation` | Error | Mod attempted undeclared capability |
| `security.mod.sandbox_escape_prevented` | Error | WASM mod attempted unauthorized operation |

### 5.2 Security Health Summary (Diagnostic Inspector)

- Identity status: `active` / `degraded (keychain unavailable)` / `missing`
- Trust store: peer count, last sync per peer, pending pairing sessions
- Grant matrix summary: workspace → peer → access level
- Access denial counters (recent window)
- Mod security: any capability violations in session
- Cryptographic status: encryption active/degraded

### 5.3 Invariant Watchdogs

Required watchdog invariants (start → terminal pairs):
- `security.identity.sign_started` → `sign_succeeded | sign_failed` (200ms)
- `security.identity.verify_started` → `verify_succeeded | verify_failed` (200ms)
- `security.trust.store_load_started` → `store_load_succeeded | store_load_failed` (1000ms)
- `security.mod.load_started` → `load_succeeded | capability_violation | load_failed` (2000ms)

---

## 6. Validation Strategy

### 6.1 Test Categories

1. **Contract tests (deterministic)** — Grant matrix (every `GraphIntent` variant's access level requirement), trust store round-trip, keypair load/generate/sign/verify, revocation completeness, nonce uniqueness.
2. **Integration tests** — `verse_access_control` harness: ReadOnly peer receives updates but local mutations are rejected; non-granted workspace sync emits `access_denied`.
3. **Denial-path tests** — Every access control branch has an explicit test that exercises the denial path and asserts the diagnostic channel fires.
4. **Mod sandbox tests** — WASM mod capability restriction tests (attempted filesystem access denied, attempted network access denied).
5. **Boundary tests** — No module outside `SyncWorker` can mutate trust store; no module outside `apply_intents()` can apply remote intents.

### 6.2 CI Gates

Required checks for PRs touching:
- `mods/native/verse/` (sync worker, trust store, pairing)
- `registries/atomic/knowledge.rs`, `registries/atomic/diagnostics.rs` (security channels)
- `registries/infrastructure/mod_loader.rs` (mod capability enforcement)
- `services/persistence/` (encryption paths)
- Any file adding new `GraphIntent` variants (must include grant classification)

### 6.3 Audit Trail

Security-relevant events should be reviewable in a dedicated Diagnostic Inspector section for forensic analysis: who connected, what was granted/denied, when identity operations occurred.

---

## 7. Degradation Policy

### 7.1 Required States

- **Full**: Keychain available, all crypto active, trust store loaded, grant enforcement active.
- **Degraded (offline-only)**: Keychain unavailable or corrupted. App functions as offline-only graph organizer. No Verse operations. Explicit diagnostics emitted.
- **Degraded (trust-reset)**: Trust store corrupted. All peers untrusted. User must re-pair. Explicit notification.

### 7.2 Required Signals

- Degradation states emit to `security.*` channels.
- Diagnostic Inspector reflects degraded security status prominently.
- User-visible indicators for: keychain locked, trust store reset, no encryption.
- No silent fallback to unencrypted or unauthenticated operations.

---

## 8. Ownership Boundaries

| Owner | Guarantees |
|---|---|
| **`IdentityRegistry`** | Keypair lifecycle, signing, verification, persona resolution. The security root. |
| **`SyncWorker`** | Transport encryption (Noise via iroh), inbound grant enforcement, access denial emission. |
| **Trust Store** (in `user_registries.json`) | Peer persistence, grant matrix, revocation completeness. |
| **`ModRegistry` / `ModLoader`** | Capability restriction, namespace enforcement, sandbox isolation. |
| **`GraphStore` / Persistence** | At-rest encryption (AES-256-GCM), nonce management; see Storage Subsystem. |

---

## 9. Implementation Roadmap (Subsystem-Local)

1. **Formalize grant matrix** — Document access level requirement for every `GraphIntent` variant. Add a compile-time or runtime check that new variants specify their grant level.
2. **Fill identity diagnostic channels** — Ensure all sign/verify/key operations emit to `security.identity.*` channels.
3. **Harden denial paths** — For every access control branch, add explicit denial-path test + diagnostic assertion.
4. **Add trust store integrity check** — Verify trust store deserialization succeeds; on failure, emit error diagnostic and reset with user notification.
5. **Wire security health summary** — Expose identity/trust/grant/crypto status in diagnostics pane.
6. **Mod capability audit** — Verify all native mods document their security surface; add capability violation tests for WASM sandbox.
7. **Grant matrix CI gate** — Add CI check that new `GraphIntent` variants include grant classification.

---

## 10. Current Status & Gaps

**What exists**:
- Security/access-control requirements are documented across Verse Tier 1 and registry-layer plans, with implementation slices already landed for parts of identity and sync access enforcement.
- Phase 5.4/5.5 done-gate closures are tracked in the planning register and partially represented in code/tests/diagnostics.
- Security subsystem contracts, diagnostics expectations, and degradation modes are now centralized in this guide.

**What's missing / open**:
- Full grant-matrix coverage and CI gating for new `GraphIntent` variants.
- Deterministic denial-path diagnostics validation across all sync branches.
- Trust-store integrity checks/recovery and comprehensive `security.identity.*` channel coverage.

### 10.1 Immediate Next Actions

Based on the Phase 5.4/5.5 done-gate closures and current code:

1. Verify `verse.sync.access_denied` is emitted deterministically on all denial paths (not just the primary one).
2. Add trust store round-trip integrity test (serialize → corrupt → deserialize → detect → recover).
3. Audit all `GraphIntent` variants for grant classification coverage.
4. Add `security.identity.*` channel family to diagnostics channel phase contracts.

---

## 11. Dependencies / Blockers

- Depends on Verse sync path stabilization and completion of Phase 5.4/5.5 done gates.
- Some guarantees require shared `GraphIntent` classification metadata patterns coordinated with reducers and registries.
- Crypto/keychain overlap with `storage` requires aligned degradation semantics and diagnostics severity conventions.

## 12. Linked Docs

- `2026-02-22_registry_layer_plan.md` (Phase 5.5 workspace access control)
- `../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md` (identity/trust/encryption/access control)
- `PLANNING_REGISTER.md` (P1/P2 and subsystem sequencing)
- `SUBSYSTEM_STORAGE.md` (at-rest encryption and keychain overlap)
- `SUBSYSTEM_DIAGNOSTICS.md` (security diagnostic channels/health summaries)

## 13. Done Definition

Security is a guaranteed system property when:

- Every `GraphIntent` variant has an associated grant classification.
- Trust boundaries are enforced at the `SyncWorker` level with no bypass paths.
- All denial paths are tested and emit diagnostics.
- Identity operations are fully observable via security diagnostic channels.
- Trust store corruption is detected and recovered with user notification.
- At-rest encryption is verified (nonce uniqueness, tag verification).
- Mod capability restrictions are enforced and tested.
- New sync paths, intent variants, and mod capabilities require security review (CI-gated or documented audit).
