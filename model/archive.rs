/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Session Capsule Ledger — core portable archive types (Slice A.1).
//!
//! These types are the WASM-safe foundation of the session portability layer.
//! No disk I/O, no async, no platform-specific deps.
//!
//! Types:
//! - [`SessionCapsule`] — sealed, encrypted snapshot of a graph session
//! - [`ArchiveReceipt`] — Ed25519-signed ownership claim over one capsule
//! - [`ArchivePrivacyClass`] — user-declared sharing policy for a capsule
//!
//! ### Serialization strategy
//! `GraphSnapshot` derives rkyv traits, not serde. `SessionCapsule` stores the
//! snapshot as pre-serialized rkyv bytes (`snapshot_bytes`) so the capsule
//! itself can be carried over serde-based transports (JSON for portability,
//! or base64-encoded in the receipt). The raw bytes are validated on decode.
//!
//! Disk I/O (`SessionLedger`, `wallet.redb`) lives in
//! `mods/native/verse/archive_wallet.rs` (native-only).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ArchivePrivacyClass
// ---------------------------------------------------------------------------

/// User-declared sharing policy for a [`SessionCapsule`].
///
/// Advisory classification stored in the capsule and receipt. It does not
/// enforce access at the protocol level — that is UCAN's responsibility.
/// It informs the runtime which sync paths are permitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchivePrivacyClass {
    /// Never leaves this device. Not synced via Verso, not shared.
    LocalPrivate,
    /// Synced to the user's own devices only (Verso bilateral sync,
    /// same root Ed25519 keypair). Not shared with third parties.
    OwnDevicesOnly,
    /// Shared with specific named peers via UCAN delegation.
    TrustedPeers,
    /// Opt-in Verse community sharing (Tier 2).
    PublicPortable,
}

// ---------------------------------------------------------------------------
// SessionCapsule
// ---------------------------------------------------------------------------

/// A sealed, content-addressed snapshot of a graph session.
///
/// The `snapshot_bytes` field holds a rkyv-serialized `GraphSnapshot`.
/// This indirection lets the capsule be serde-portable without requiring
/// `GraphSnapshot` to derive serde traits.
///
/// ### Lifecycle
/// ```text
/// GraphSnapshot  →  rkyv::to_bytes()  →  SessionCapsule::new()
///     ↓
/// serde_json or raw bytes transport
///     ↓
/// compress (zstd)          ← Slice A.2
///     ↓
/// encrypt (AES-256-GCM)    ← Slice A.2
///     ↓
/// compute CID (sha2-256)   ← Slice A.2
///     ↓
/// sign → ArchiveReceipt    ← Slice A.3
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCapsule {
    /// Stable UUID v4 identity for this capsule.
    pub archive_id: Uuid,

    /// rkyv-serialized `GraphSnapshot` bytes. Use
    /// `SessionCapsule::decode_snapshot()` to recover the snapshot (Slice A.1
    /// callers that need the struct back must do so in a context where
    /// `graphshell-core` persistence types are available).
    #[serde(with = "serde_bytes_base64")]
    pub snapshot_bytes: Vec<u8>,

    /// Optional rkyv-serialized `LogEntry` mutation tail since the last
    /// periodic snapshot. Empty if created from a clean snapshot.
    #[serde(with = "serde_bytes_base64")]
    pub log_tail_bytes: Vec<u8>,

    /// Wall-clock creation time (milliseconds since UNIX epoch).
    pub created_at_ms: u64,

    /// Human-readable name of the device that created this capsule.
    pub device_name: String,

    /// Ed25519 public key of the owner (raw 32-byte verifying key).
    pub owner_pubkey: [u8; 32],

    /// User-visible labels for organization and search.
    pub tags: Vec<String>,

    /// User-declared sharing policy.
    pub privacy_class: ArchivePrivacyClass,
}

impl SessionCapsule {
    /// Construct a capsule from pre-serialized snapshot bytes.
    ///
    /// `snapshot_bytes` must be the output of `rkyv::to_bytes::<GraphSnapshot>()`.
    /// `log_tail_bytes` may be empty.
    pub fn new(
        snapshot_bytes: Vec<u8>,
        log_tail_bytes: Vec<u8>,
        owner_pubkey: [u8; 32],
        device_name: String,
        privacy_class: ArchivePrivacyClass,
        tags: Vec<String>,
    ) -> Self {
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            archive_id: Uuid::new_v4(),
            snapshot_bytes,
            log_tail_bytes,
            created_at_ms,
            device_name,
            owner_pubkey,
            tags,
            privacy_class,
        }
    }
}

// ---------------------------------------------------------------------------
// ArchiveReceipt
// ---------------------------------------------------------------------------

/// An Ed25519-signed ownership claim over one [`SessionCapsule`].
///
/// The receipt is the wallet entry. A `SessionLedger` is an index of
/// `ArchiveReceipt`s.
///
/// ### Signature coverage
/// The Ed25519 signature (populated in Slice A.3) covers:
/// `cid_bytes || archive_id.as_bytes() || created_at_ms.to_le_bytes()`
///
/// where `cid_bytes` is the raw multihash bytes of the CIDv1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveReceipt {
    /// CIDv1 (sha2-256, raw codec) of the compressed+encrypted capsule bytes.
    /// Stored as the multibase string (base32 lower, "b" prefix).
    /// Empty string until Slice A.2 CID computation is wired.
    pub archive_cid: String,

    /// Matches `SessionCapsule::archive_id`.
    pub archive_id: Uuid,

    /// Ed25519 public key of the owner (raw 32-byte verifying key).
    pub owner_pubkey: [u8; 32],

    /// Wall-clock creation time (milliseconds since UNIX epoch).
    pub created_at_ms: u64,

    /// User-visible labels (copied from the capsule at receipt creation time).
    pub tags: Vec<String>,

    /// User-declared sharing policy.
    pub privacy_class: ArchivePrivacyClass,

    /// Ed25519 signature (raw 64 bytes) over the canonical byte sequence.
    /// All-zeros until Slice A.3 signing is wired.
    #[serde(with = "serde_sig_bytes")]
    pub signature: [u8; 64],
}

impl ArchiveReceipt {
    /// Construct an unsigned receipt stub from a capsule.
    ///
    /// `archive_cid` is empty and `signature` is zeroed until Slice A.2/A.3.
    pub fn stub(capsule: &SessionCapsule) -> Self {
        Self {
            archive_cid: String::new(),
            archive_id: capsule.archive_id,
            owner_pubkey: capsule.owner_pubkey,
            created_at_ms: capsule.created_at_ms,
            tags: capsule.tags.clone(),
            privacy_class: capsule.privacy_class.clone(),
            signature: [0u8; 64],
        }
    }
}

// ---------------------------------------------------------------------------
// Serde helpers for byte arrays
// ---------------------------------------------------------------------------

/// Serde helper: serialize `Vec<u8>` as base64 in JSON, passthrough in binary.
mod serde_bytes_base64 {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(&STANDARD.encode(bytes))
        } else {
            s.serialize_bytes(bytes)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        if d.is_human_readable() {
            let s = String::deserialize(d)?;
            STANDARD.decode(&s).map_err(D::Error::custom)
        } else {
            Vec::<u8>::deserialize(d)
        }
    }
}

/// Serde helper: serialize `[u8; 64]` as base64 in JSON.
mod serde_sig_bytes {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(&STANDARD.encode(bytes))
        } else {
            s.serialize_bytes(bytes)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        if d.is_human_readable() {
            let s = String::deserialize(d)?;
            let v = STANDARD.decode(&s).map_err(D::Error::custom)?;
            v.try_into()
                .map_err(|_| D::Error::custom("expected 64-byte signature"))
        } else {
            let v = Vec::<u8>::deserialize(d)?;
            v.try_into()
                .map_err(|_| D::Error::custom("expected 64-byte signature"))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_capsule() -> SessionCapsule {
        SessionCapsule::new(
            vec![0u8, 1, 2, 3],
            vec![],
            [1u8; 32],
            "test-device".to_string(),
            ArchivePrivacyClass::OwnDevicesOnly,
            vec!["test".to_string()],
        )
    }

    #[test]
    fn session_capsule_round_trip_json() {
        let capsule = dummy_capsule();
        let json = serde_json::to_string(&capsule).expect("serialize");
        let back: SessionCapsule = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(capsule.archive_id, back.archive_id);
        assert_eq!(capsule.owner_pubkey, back.owner_pubkey);
        assert_eq!(capsule.snapshot_bytes, back.snapshot_bytes);
        assert_eq!(back.privacy_class, ArchivePrivacyClass::OwnDevicesOnly);
    }

    #[test]
    fn archive_receipt_stub_matches_capsule() {
        let capsule = dummy_capsule();
        let receipt = ArchiveReceipt::stub(&capsule);
        assert_eq!(receipt.archive_id, capsule.archive_id);
        assert_eq!(receipt.owner_pubkey, capsule.owner_pubkey);
        assert!(receipt.archive_cid.is_empty());
        assert_eq!(receipt.signature, [0u8; 64]);
    }

    #[test]
    fn archive_receipt_round_trip_json() {
        let receipt = ArchiveReceipt::stub(&dummy_capsule());
        let json = serde_json::to_string(&receipt).expect("serialize");
        let back: ArchiveReceipt = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(receipt.archive_id, back.archive_id);
        assert_eq!(receipt.signature, back.signature);
    }

    #[test]
    fn privacy_class_serde_round_trip() {
        for cls in [
            ArchivePrivacyClass::LocalPrivate,
            ArchivePrivacyClass::OwnDevicesOnly,
            ArchivePrivacyClass::TrustedPeers,
            ArchivePrivacyClass::PublicPortable,
        ] {
            let json = serde_json::to_string(&cls).expect("serialize");
            let back: ArchivePrivacyClass = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cls, back);
        }
    }
}

