# Session Capsule Ledger — Implementation Plan

**Date**: 2026-03-27
**Status**: Approved / Implementation-Ready
**Phase**: Verso Phase 5 extension (Tier 1 portable archive layer)
**Context**: Defines the `SessionCapsule` portable archive format, `SessionLedger` receipt index, UCAN delegation model, and Verso sync integration. This is the concrete deliverable for WASM-safe, cryptographically-owned session portability.

---

## 1. Motivation

Data portability is a first-class usability goal for Graphshell. Users should own their browsing graph, carry it across devices without a central server, encrypt it for privacy, and optionally share it to bootstrap co-op sessions or contribute to Verse communities.

This is explicitly foreshadowed in `PROJECT_DESCRIPTION.md` (Verse section: "selective tokenization of browsing data enabling portability, management, encryption, and distribution"). This plan delivers the Tier 1 portable archive layer — no on-chain tokens, no storage economy. Those are Tier 2 concerns.

---

## 2. Canonical Terminology

| Term | Meaning |
|---|---|
| `SessionCapsule` | One sealed, encrypted, content-addressed snapshot of a graph session. "Capsule" not "archive" — it is portable and shareable, not cold storage. |
| `SessionLedger` | The user's local index of `ArchiveReceipt`s — a cryptographic browsing ledger. |
| `ArchiveReceipt` | A signed ownership claim over one `SessionCapsule`: CID + owner pubkey + Ed25519 signature. |
| `ArchivePrivacyClass` | The declared sharing intent of a capsule (`LocalPrivate`, `OwnDevicesOnly`, `TrustedPeers`, `PublicPortable`). |

---

## 3. Architecture

### 3.1 Layer 0: SessionCapsule (wasm-safe core)

```rust
/// graphshell-core — must compile to wasm32-unknown-unknown
struct SessionCapsule {
    archive_id: Uuid,                      // UUID v4 — stable identity
    graph_snapshot: GraphSnapshot,
    log_tail: Vec<LogEntry>,               // optional mutation log since last snapshot
    created_at_ms: u64,
    device_name: String,
    owner_pubkey: [u8; 32],                // Ed25519 public key (owner's NodeId)
    tags: Vec<String>,
    classification_hint: ArchivePrivacyClass,
}

enum ArchivePrivacyClass {
    LocalPrivate,    // never leaves this device
    OwnDevicesOnly,  // Verso bilateral sync only
    TrustedPeers,    // shared with specific named peers via UCAN
    PublicPortable,  // opt-in Verse community sharing (Tier 2)
}
```

**Serialization pipeline:**

1. rkyv serialize (for local storage) / serde_json (for portability export)
2. zstd compress (already in Cargo.toml)
3. AES-256-GCM encrypt (already in Cargo.toml) — key derived from owner's Ed25519 private key

### 3.2 Layer 1: Content Addressing (wasm-safe)

Encrypted capsule bytes are content-addressed as CIDv1 (sha2-256, per `verseblob_content_addressing_spec.md` defaults). Computed locally using `cid` + `multihash` crates — no IPFS dependency required in Tier 1.

```rust
impl SessionCapsule {
    fn compute_cid(&self) -> Cid { /* sha2-256 of compressed, encrypted bytes */ }
    fn encrypt(&self, key: &[u8; 32]) -> Vec<u8> { /* AES-256-GCM */ }
    fn decrypt(bytes: &[u8], key: &[u8; 32]) -> Result<Self> { /* AES-256-GCM */ }
}
```

### 3.3 Layer 2: Ownership Receipt (wasm-safe)

```rust
struct ArchiveReceipt {
    archive_cid: Cid,
    archive_id: Uuid,
    owner: NodeId,           // Ed25519 public key ("did:key" for the owner)
    created_at_ms: u64,
    tags: Vec<String>,
    privacy_class: ArchivePrivacyClass,
    signature: [u8; 64],     // Ed25519 sig over (cid bytes + archive_id bytes + created_at_ms)
}
```

A collection of `ArchiveReceipt`s is the `SessionLedger`. The signing key lives in the OS keychain (via `keyring`, already used by Verso `P2PIdentitySecret`). On WASM targets, the key is passed in from the host.

### 3.4 Layer 3: UCAN Delegation

UCAN serves two use cases:

1. **Session portability**: grant a peer read access to a specific `SessionCapsule` to bootstrap a co-op session with full graph history (not a blank canvas)
2. **Co-op hosting (future)**: the `workspace/write` capability extends this mechanism to live intent delegation

**Own-device sync**: no UCAN needed. Both devices share the same root Ed25519 keypair. Decryption key is derived from the shared private key; verification is a bare signature check.

**Friend/peer sharing**:

A per-capsule AES-256-GCM key is wrapped in a UCAN capability token:

```text
UCAN {
    issuer: did:key:<owner-pubkey>,
    audience: did:key:<friend-pubkey>,
    capabilities: [{ with: "graphshell:archive:<cid>", can: "archive/read" }],
    expiry: Option<u64>,
    proof: <Ed25519 signature>,
    facts: { encrypted_archive_key: <base64(NaCl-box(aes_key, friend_pubkey, owner_privkey))> }
}
```

The recipient uses their private key to unbox the AES key, then decrypts the capsule. `rs-ucan` handles the UCAN envelope; `crypto_box` (NaCl box, WASM-compatible) handles key encapsulation.

### 3.5 Layer 4: Verso Sync Integration (native only)

The existing Verso iroh/QUIC sync protocol carries `ArchiveReceipt`s as a new intent variant:

```rust
VersoIntent::SyncCapsule { cid: Cid }
```

On receiving a sync event, the device:

1. Verifies the `ArchiveReceipt` signature
2. Fetches encrypted capsule bytes over iroh (already established transport)
3. Derives the AES key from the shared Ed25519 keypair (own-device) or unboxes via UCAN (peer)
4. Decrypts and calls `Graph::from_snapshot()` to restore the session

### 3.6 WASM Boundary

| Layer | Location | WASM-safe |
|---|---|---|
| `SessionCapsule` + serialization | `graphshell-core` | Yes |
| `ArchiveReceipt` + signing | `graphshell-core` | Yes |
| `ArchivePrivacyClass` | `graphshell-core` | Yes |
| CID computation | `graphshell-core` | Yes |
| AES-256-GCM encrypt/decrypt | `graphshell-core` | Yes |
| UCAN envelope construction | `graphshell-core` (rs-ucan) | Yes |
| `crypto_box` key encapsulation | `graphshell-core` | Yes |
| `SessionLedger` (disk I/O, `wallet.redb`) | `mods/native/verse/archive_wallet.rs` | No (disk) |
| Verso sync (`VersoIntent::SyncCapsule`) | `mods/native/verse/mod.rs` | No (iroh) |

---

## 4. Transport and Mobile Note

iroh uses QUIC + hole-punching (Magic Sockets). Both devices must be **online simultaneously** for Tier 1 bilateral sync. When hole-punching fails, iroh falls back to `relay.iroh.network` (self-hostable).

For async mobile access (device not online): IPFS pinning or a self-hosted Verse storage node is required. That is a Tier 2 concern. The `SessionCapsule` format is designed to support this path without modification.

---

## 5. New Dependencies

| Crate | Version | Purpose | WASM-safe | Already present |
|---|---|---|---|---|
| `cid` | 0.11 | CIDv1 content addressing | Yes | No |
| `multihash` | 0.19 | Multihash (sha2-256 wrapper) | Yes | No |
| `rs-ucan` | 0.5+ | UCAN capability token format | Yes | No |
| `crypto_box` | 0.9 | NaCl box for key encapsulation | Yes | No |

Already present: `sha2`, `aes-gcm`, `zstd`, `rkyv`, `serde`, `serde_json`, `ed25519-dalek`, `redb`.

---

## 6. New GraphIntent Variants

Per CLAUDE.md: all new `GraphIntent` variants must be handled in `apply_intents()`.

```rust
// app/intents.rs
GraphIntent::ExportSessionCapsule {
    privacy_class: ArchivePrivacyClass,
    tags: Vec<String>,
},
GraphIntent::ImportSessionCapsule {
    capsule_cid: String,   // CIDv1 string
},
```

---

## 7. Files to Create / Modify

### New files

- `model/archive.rs` — `SessionCapsule`, `ArchiveReceipt`, `ArchivePrivacyClass` (wasm-safe)
- `mods/native/verse/archive_wallet.rs` — `SessionLedger` receipt index (`wallet.redb`, native only)

### Modified files

- `Cargo.toml` — add `cid`, `multihash`, `rs-ucan`, `crypto_box`
- `app/intents.rs` — add `ExportSessionCapsule`, `ImportSessionCapsule`
- `app/intent_phases.rs` — handle new variants in `handle_domain_graph_intent()`
- `graph_app.rs` — wire export/import handlers
- `mods/native/verse/mod.rs` — register ledger capability, add `VersoIntent::SyncCapsule` path

---

## 8. Implementation Slices

### A.1 — Core types + serialization (wasm-safe, no crypto)

- Define `SessionCapsule`, `ArchiveReceipt`, `ArchivePrivacyClass` in `model/archive.rs`
- Implement `SessionCapsule::from_snapshot(snapshot: GraphSnapshot) -> Self`
- rkyv + serde_json round-trip

**Done gate**: `cargo check -p graphshell-core --target wasm32-unknown-unknown` passes with zero errors.

### A.2 — CID computation + encryption

- Add `cid` + `multihash` to Cargo.toml
- `SessionCapsule::compute_cid(&self) -> Cid`
- `SessionCapsule::encrypt(&self, key: &[u8; 32]) -> Vec<u8>`
- `SessionCapsule::decrypt(bytes: &[u8], key: &[u8; 32]) -> Result<Self>`

**Done gate**: encrypt/decrypt round-trip unit test passes; WASM check still clean.

### A.3 — Receipt signing + SessionLedger

- `ArchiveReceipt::sign(capsule: &SessionCapsule, secret: &SecretKey) -> Self`
- `ArchiveReceipt::verify(&self) -> bool`
- `SessionLedger` backed by a separate `wallet.redb` file (native only)

**Done gate**: sign → verify round-trip test; ledger stores and retrieves receipts.

### A.4 — GraphIntent integration

- Add `ExportSessionCapsule` / `ImportSessionCapsule` to `GraphIntent`
- Handle in `handle_domain_graph_intent()` in `app/intent_phases.rs`
- Export: `Graph::to_snapshot()` → `SessionCapsule::from_snapshot()` → encrypt → ledger entry
- Import: decrypt → `Graph::from_snapshot()` → apply to domain

**Done gate**: round-trip export → import produces identical graph state.

### A.5 — Verso sync (own-device capsule transfer) + UCAN delegation

- Add `rs-ucan` + `crypto_box` to Cargo.toml
- `VersoIntent::SyncCapsule { cid: Cid }` in Verso sync protocol
- UCAN construction for peer sharing (`ArchivePrivacyClass::TrustedPeers`)
- On sync: compare ledger receipt indices, fetch missing capsules over iroh

**Done gate**: two simulated instances exchange a `SessionCapsule`; UCAN token round-trip verifies.

---

## 9. Verification

1. `cargo check -p graphshell-core --target wasm32-unknown-unknown` — zero errors (A.1)
2. Unit test: `SessionCapsule` encrypt → decrypt round-trip (A.2)
3. Unit test: `ArchiveReceipt` sign → verify round-trip (A.3)
4. Integration test: `ExportSessionCapsule` → `ImportSessionCapsule` produces graph state equality (A.4)
5. Unit test: UCAN token construction → verification round-trip (A.5)
6. Diagnostics channel events confirm capsule write/read in the diagnostics panel (no separate wallet UI required for this phase)

A full wallet UI (web client or browser extension) is a follow-on milestone.

---

## 10. Relationship to Existing Specs

- **`verseblob_content_addressing_spec.md`**: `SessionCapsule` CID format follows the VerseBlob defaults (CIDv1, sha2-256, base32). Capsules are a valid `VerseBlobKind::Opaque` payload in Tier 2.
- **`engram_spec.md`**: A `SessionCapsule` is not an Engram (it carries graph state, not model adaptation deltas). However, `ArchiveReceipt`s share the same trust/provenance pattern and can be wrapped in a `TransferProfile` for Verse community sharing (Tier 2, future).
- **`2026-02-23_verse_tier1_sync_plan.md`**: `SessionLedger` sync reuses the existing iroh transport and `TrustedPeer` trust store. No new identity infrastructure needed.

---

## 11. Open Questions

1. Should `VersoIntent::SyncCapsule` trigger automatically on peer connect (full ledger reconciliation) or only on explicit user action?
2. For `ArchivePrivacyClass::PublicPortable` capsules destined for Verse Tier 2: should the `SessionCapsule` format carry a `VerseSubmissionProfile` stub now (for forward compatibility), or add it later?
