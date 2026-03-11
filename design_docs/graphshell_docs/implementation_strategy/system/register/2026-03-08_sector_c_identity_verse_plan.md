<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector C — Identity & Verse Registry Development Plan

**Doc role:** Implementation plan for the identity and verse registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `IdentityRegistry`, `NostrCoreRegistry`
**Specs:** [identity_registry_spec.md](identity_registry_spec.md), [nostr_core_registry_spec.md](nostr_core_registry_spec.md)
**Also depends on:** `system/2026-03-05_cp4_p2p_sync_plan.md`, `system/2026-03-05_nostr_mod_system.md`

---

## Purpose

`IdentityRegistry` and `NostrCoreRegistry` are co-dependent, but they no longer describe one
cryptographic lane. `IdentityRegistry` owns the transport/device identity (`NodeId`, Ed25519) and
the local user-signing claim surface used to bind session presence. `NostrCoreRegistry` owns the
relay/signer lane for `UserIdentity`, which must eventually become real Nostr-compatible
secp256k1/NIP-46 signing. Device sync requires trusted peer identity managed by the identity
layer; public/user identity is bridged onto that transport identity through a short-lived signed
presence assertion rather than a shared keypair.

The sector proceeds in three tracks:

```
Track 1: IdentityRegistry — real node identity + presence binding assertions
Track 2: NostrCoreRegistry — real relay backend + NIP-46/secp256k1 user signing
Track 3: Cross-registry wiring — bind UserIdentity to NodeId without shared key material
```

---

## Current State

| Registry | Struct | Completeness | Key gaps |
|---|---|---|---|
| `IdentityRegistry` | ✅ | Runtime authority | Real Ed25519 node-signing, persistence, rotation/revocation, Verse trust wiring, and signed presence-binding assertions are landed; NIP-07 remains deferred |
| `NostrCoreRegistry` | ✅ | Runtime authority | Supervised `tokio-tungstenite` relay backend, subscription persistence, relay diagnostics, local secp256k1 user signing, and NIP-46 delegated signing are landed |

### Implementation note — 2026-03-10 correction

The original Sector C plan incorrectly assumed that Nostr signing could reuse the same Ed25519 key
lane as Verse/i roh transport identity. The runtime now explicitly models a two-layer identity
shape:

- `NodeId` / transport identity: Ed25519, owned by `IdentityRegistry`, used for Verse/iroh trust
  and sync payloads.
- `UserIdentity`: public/user signing identity for Nostr and future AT Protocol-style surfaces.
- Binding seam: a short-lived signed presence assertion carried by Verse discovery/presence so the
  two layers can be linked when the user explicitly participates, without collapsing them into one
  persistent keypair.

Current implementation note:

- The presence-binding carrier is landed and signed by the local default user-claim key.
- That local user-claim now uses a dedicated secp256k1 signer, separate from the `NodeId`
  transport key.
- `SignerBackend::Nip46` now routes through the supervised relay worker with an encrypted
  request/response path and a local bunker-mock contract test.
- Bunker URI parsing, session-only secret handling, and local permission memory are now landed on
  top of the delegated signer path.
- Remaining follow-ons are signer-surface integrations such as NIP-07/browser-extension parity and
  richer approval UX, not a registry/runtime correctness blocker.

---

## Phase C1 — IdentityRegistry: Real node identity + binding assertions

**Unlocks:** Transport/node identity, signed presence binding, CP4 peer trust.

### C1.1 — Replace SHA256 stub with `ed25519-dalek` keypair

The `identity_registry_spec.md`'s `crypto-operation` policy requires that local signing uses
real asymmetric cryptography. The current `sign()` implementation hashes payload bytes with
SHA256 — this produces a deterministic but cryptographically meaningless output.

Replace with ed25519:

```rust
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};

pub struct IdentityKey {
    signing_key: SigningKey,
}

impl IdentityKey {
    pub fn generate() -> Self {
        Self { signing_key: SigningKey::generate(&mut OsRng) }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, IdentityKeyError> {
        SigningKey::from_bytes(bytes).map(|k| Self { signing_key: k })
            .map_err(|_| IdentityKeyError::InvalidKeyMaterial)
    }

    pub fn sign(&self, payload: &[u8]) -> Vec<u8> {
        self.signing_key.sign(payload).to_bytes().to_vec()
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }
}
```

`IdentityRegistry::sign()` delegates to `IdentityKey::sign()`.
`IdentityRegistry::verify()` (new, required by spec) uses `VerifyingKey::verify()`.

**Done gates:**
- [x] `ed25519-dalek` added to `Cargo.toml` with `rand_core` + `getrandom` features.
- [x] `IdentityKey` struct replaces raw bytes in the identity store.
- [x] `sign()` uses ed25519; `verify()` implemented.
- [x] `IDENTITY_ID_DEFAULT` and `IDENTITY_ID_P2P` keys generated at first run and persisted to user data dir.
- [x] Unit tests: sign + verify round-trip; verify with wrong key returns Err.

### C1.2 — Key persistence

Identity keys must survive restart. Keys are persisted to the platform user data directory, not
the workspace (they are device-scoped, not workspace-scoped).

```rust
pub fn load_or_generate(key_id: &IdentityId, store_path: &Path)
    -> Result<IdentityKey, IdentityKeyError>
```

Keys are stored as raw 32-byte ed25519 seed files, protected by filesystem permissions. No
passphrase encryption in the initial implementation; a `KeyProtection::Unprotected` annotation
marks this as a known gap until a keychain integration phase.

**Done gates:**
- [x] `load_or_generate()` implemented for the default and P2P identity slots.
- [x] Key files are not checked into version control (`.gitignore` rule).
- [x] `DIAG_IDENTITY_SIGN` emits at `Warn` if key file is missing and a new key is generated.

### C1.3 — Key rotation and revocation

```rust
pub fn rotate_key(&mut self, key_id: &IdentityId) -> Result<VerifyingKey, IdentityKeyError>
pub fn revoke_key(&mut self, key_id: &IdentityId) -> Result<(), IdentityKeyError>
```

Rotation generates a new keypair and archives the old verifying key (for verifying historical
signatures). Revocation removes the signing key but retains the verifying key for audit.
Both operations emit `DIAG_IDENTITY_SIGN` at `Info` severity.

**Done gates:**
- [x] `rotate_key()` and `revoke_key()` implemented.
- [x] Rotated keys are archived, not discarded.
- [ ] Diagnostics emit on rotation and revocation.

### C1.4 — Presence binding assertion carrier

`IdentityRegistry` now needs an explicit cross-layer binding carrier so Verse discovery/presence can
link a `UserIdentity` claim to a transport `NodeId` without sharing key material.

```rust
pub struct PresenceBindingAssertion {
    pub node_id: String,
    pub user_identity: UserIdentityClaim,
    pub issued_at_secs: u64,
    pub expires_at_secs: u64,
    pub audience: String,
    pub signature: String,
}
```

**Done gates:**
- [x] `IdentityRegistry::create_presence_binding_assertion()` implemented.
- [x] `IdentityRegistry::verify_presence_binding_assertion()` implemented.
- [x] Verse mDNS discovery/presence carries the binding assertion.
- [x] Discovery surfaces whether the binding verified successfully.

---

## Phase C2 — NostrCoreRegistry: Real relay backend

**Unlocks:** Actual Nostr event publishing and subscription; Verse device sync over Nostr.

### C2.1 — `TungsteniteRelayService` implementation

`InProcessRelayService` is a trait defined in `nostr_core.rs`. Replace the test-only mock with a
real WebSocket relay backend using `tokio-tungstenite`:

```rust
pub struct TungsteniteRelayService {
    connections: HashMap<Url, RelayConnection>,
    policy: NostrRelayPolicy,
    cancel: CancellationToken,
}

impl InProcessRelayService for TungsteniteRelayService {
    async fn subscribe(&mut self, filters: NostrFilterSet, handle: NostrSubscriptionHandle)
        -> Result<(), NostrCoreError>;
    async fn unsubscribe(&mut self, handle: NostrSubscriptionHandle)
        -> Result<(), NostrCoreError>;
    async fn publish(&mut self, event: NostrSignedEvent)
        -> Result<NostrPublishReceipt, NostrCoreError>;
}
```

The relay service runs as a supervised worker under `ControlPanel`, not as a standalone thread.
It multiplexes subscriptions over a connection pool governed by `NostrRelayPolicy`.

`ws://` (non-TLS) connections are permitted only in dev/test mode (feature flag or explicit policy
override). Production default is `wss://` only (existing normalization preserved).

**Done gates:**
- [x] `TungsteniteRelayService` struct with basic connect/disconnect/subscribe/publish.
- [x] Relay service spawned as supervised worker in `ControlPanel`.
- [x] `Community` relay policy (default) connects to the configured relay list.
- [ ] `DIAG_NOSTR_RELAY` channels emit on connection state changes.
- [x] Integration test: publish/subscribe/close commands are emitted over a local relay websocket.

### C2.2 — Subscription persistence across restarts

Active subscription filter sets are part of workspace state. When the relay service restarts
(e.g. app restart), subscriptions must be re-established from persisted state.

Subscriptions are persisted via `GraphIntent::PersistNostrSubscriptions` (new intent) which
writes the active filter set to workspace state through the WAL.

**Done gates:**
- [x] `GraphIntent::PersistNostrSubscriptions` variant defined and handled.
- [x] On startup, persisted subscriptions are re-submitted to `TungsteniteRelayService`.
- [x] Test: restart with active subscription re-establishes it automatically.

### C2.3 — NIP-46 remote signer

`SignerBackend::Nip46` is typed in `nostr_core.rs` but has no implementation. NIP-46 (Nostr
Connect / "bunker") delegates signing to a remote signer process via a Nostr relay. This is also
the cleanest way to finish the `UserIdentity` lane without collapsing it back into the Ed25519
transport identity.

```rust
pub struct Nip46Signer {
    bunker_url: Url,
    session_key: IdentityKey,   // ephemeral session keypair
    relay_service: Arc<Mutex<dyn InProcessRelayService>>,
}

impl Nip46Signer {
    pub async fn sign_event(&self, unsigned: NostrUnsignedEvent)
        -> Result<NostrSignedEvent, NostrCoreError>;
}
```

This is a medium-complexity async RPC over Nostr relay. It enables hardware signer integration
and NIP-07 browser extension bridges.

**Done gates:**
- [x] `Nip46Signer`-equivalent relay RPC path with `sign_event()` implementation.
- [x] `SignerBackend::Nip46` variant wired into `NostrCoreRegistry::sign_event()`.
- [x] Session key is generated from the user-identity lane without reusing the P2P `NodeId` key store.
- [x] Integration test: NIP-46 sign round-trip with a local bunker mock.

Current implementation note:
- Local signing now uses canonical Nostr event hashes with `created_at`, and the relay backend is a supervised worker under `ControlPanel`.
- `SignerBackend::Nip46` is now implemented over the relay worker using encrypted NIP-46 RPC.
- Bunker URI parsing, session-only bunker secret handling, and local pending/allow/deny permission
  memory now exist on the Sync settings surface and persist non-secret policy state across restart.
- NIP-07/browser-extension parity remains follow-on work.

---

## Phase C3 — Unified keypair ownership and CP4 wiring

**Unlocks:** No duplicated node key material; CP4 peer trust; explicit cross-layer identity binding.

### C3.1 — Single node-key owner: `IdentityRegistry`

Currently `NostrCoreRegistry` delegates its local host signing path into `IdentityRegistry`, but
that path is still transitional and not a real Nostr key. The invariant here is narrower: the
transport/node key owner must stay singular, and user-signing must not silently reuse it once
secp256k1/NIP-46 lands.

Refactor `NostrCoreRegistry::sign_event()` to delegate to `IdentityRegistry`:

```rust
// in NostrCoreRegistry::sign_event()
let verifying_key = registries.identity.verifying_key_for(&self.signer_config.identity_id)?;
let signature = registries.identity.sign(&event_hash, &self.signer_config.identity_id)?;
```

`IdentityRegistry` is the only `NodeId`/transport key owner. `NostrCoreRegistry` may reference a
user-signing handle, but it must not store or mint a second transport key.

**Done gates:**
- [x] `NostrCoreRegistry` transport key store removed; only identity references remain.
- [x] `NostrCoreRegistry::sign_event()` calls `IdentityRegistry::sign()` on the current transitional local-host path.
- [x] Unit test: event signed via NostrCore validates against IdentityRegistry verifying key.
- [x] No raw transport key bytes stored in `NostrCoreRegistry`.

### C3.2 — CP4 peer identity wiring

When CP4 P2P sync lands, peer trust requires `IDENTITY_ID_P2P` to sign sync payloads and
verify remote peer signatures. The `P2PIdentityExt` trait on `IdentityRegistry` already stubs
this delegation to `crate::mods::native::verse::*`. Replace with direct `IdentityKey` usage:

```rust
impl IdentityRegistry {
    pub fn p2p_sign(&self, payload: &[u8]) -> Result<Vec<u8>, IdentityKeyError> {
        self.sign(payload, &IDENTITY_ID_P2P)
    }

    pub fn p2p_verify(&self, payload: &[u8], sig: &[u8], peer_key: &[u8; 32])
        -> Result<bool, IdentityKeyError>
}
```

**Done gates (deferred to CP4 active phase):**
- [ ] `p2p_sign()` uses real ed25519 from `IDENTITY_ID_P2P` key.
- [ ] `p2p_verify()` implemented; peer public key comes from `PeerRegistry` (CP4).
- [ ] Verse verse module calls replaced with direct `IdentityRegistry` calls.

### C3.3 — NIP-07 bridge (deferred, prospective)

The `nostr_core_registry_spec.md` lists NIP-07 (browser extension signer) as a prospective
capability. This involves injecting a signing bridge into embedded web content contexts.
Track as a prospective capability; no implementation in this sector.

**Done gate:**
- [ ] Spec entry in `nostr_core_registry_spec.md` updated to mark NIP-07 as deferred.

---

## Acceptance Criteria (Sector C complete)

- [ ] `IdentityRegistry` uses real ed25519 signing; `sign()` + `verify()` round-trip tested.
- [ ] Identity keys are persisted to platform user data directory and survive restart.
- [ ] `NostrCoreRegistry` has no local key store; all signing delegates to `IdentityRegistry`.
- [x] `TungsteniteRelayService` enables real Nostr event publish/subscribe.
- [x] Nostr subscriptions persist across app restarts.
- [x] NIP-46 remote signer is implemented and wired to `SignerBackend::Nip46`.
- [ ] `DIAG_IDENTITY_SIGN` and `DIAG_NOSTR_RELAY` channels emit with correct severity.
- [ ] No duplicate key material exists anywhere in the codebase.

---

## Related Documents

- [identity_registry_spec.md](identity_registry_spec.md)
- [nostr_core_registry_spec.md](nostr_core_registry_spec.md)
- [../2026-03-05_cp4_p2p_sync_plan.md](../2026-03-05_cp4_p2p_sync_plan.md) — CP4 P2P sync dependency
- [../2026-03-05_nostr_mod_system.md](../2026-03-05_nostr_mod_system.md) — Nostr mod system
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — register routing policy
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
