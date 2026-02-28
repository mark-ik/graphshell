# VerseBlob Content Addressing Specification

**Date**: 2026-02-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the canonical `VerseBlob` envelope, content addressing rules, transport split, size classes, and retrieval expectations for Tier 2 Verse communities.
**Related**:
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`
- `design_docs/verse_docs/implementation_strategy/engram_spec.md`
- `design_docs/verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md`

---

## 1. Purpose

`VerseBlob` is the canonical content-addressed transport unit for Tier 2.

It exists to:
- advertise and retrieve immutable content by hash
- separate small pubsub control messages from large binary payloads
- support reuse across FLora, search, storage receipts, applets, and community metadata

---

## 2. Core Design Rules

1. **CID-first**
Every `VerseBlob` should be identified by a CID-compatible content address.

2. **CIDv1 base32 default**
Use CIDv1 with base32 text encoding as the portable canonical representation.

3. **Pubsub carries manifests, not bulk bytes**
GossipSub messages should be compact announcements or manifests only.

4. **Immutable payloads**
Once addressed, a blob is immutable. New state means a new blob.

5. **Bounded decoding**
Receivers must enforce hard limits on decompression, nesting, and attachment expansion before fully decoding.

---

## 3. Canonical Envelope

```rust
struct VerseBlob {
    cid: Cid,                    // canonical CIDv1 identifier
    schema_version: u32,
    kind: VerseBlobKind,
    codec: VerseBlobCodec,

    author: PeerId,
    created_at_ms: u64,
    signature: Signature,

    body: VerseBlobBody,
}

enum VerseBlobKind {
    IntentDelta,
    IndexSegment,
    EngramEnvelope,
    ReceiptBatch,
    CommunityManifest,
    GovernanceEvent,
    AppletPackage,
    FeedDelta,
    Opaque,
}

enum VerseBlobCodec {
    DagCbor,
    Raw,
    CarV1,
}
```

### 3.1 Body Classes

```rust
enum VerseBlobBody {
    InlineBytes(Vec<u8>),            // only for small control payloads
    AttachmentManifest(BlobManifest),// primary mode for larger content
}

struct BlobManifest {
    root_ref: BlobRef,
    attachments: Vec<BlobRef>,
    total_declared_bytes: u64,
}

struct BlobRef {
    cid: Cid,
    declared_bytes: u64,
    media_type: String,
    role: BlobRole,
}

enum BlobRole {
    Root,
    AdapterWeights,
    EvalBundle,
    DatasetSummary,
    PromptBundle,
    SignatureBundle,
    Ancillary,
}
```

---

## 4. Content Addressing Rules

### 4.1 Hashing

Recommended default:
- multihash: `sha2-256`
- CID version: `v1`
- textual form: base32 lower-case

This aligns with current IPFS interoperability norms and avoids base58btc-only legacy assumptions.

### 4.2 Codec Selection

- `DagCbor`: structured envelopes and manifests
- `Raw`: direct binary attachments
- `CarV1`: bundled, multi-block transport archives when a submission or checkpoint needs a package of content-addressed blocks

### 4.3 Relationship to IPFS

Verse uses CID-compatible addressing, but a `VerseBlob` is not required to be globally pinned on IPFS.

Practical rule:
- if a node can resolve by local cache, trusted peer provider, or community provider table, that is sufficient
- optional IPFS pinning is allowed for portability and archival

---

## 5. Size Classes and Transport Policy

```rust
enum BlobSizeClass {
    InlineControl,   // <= 64 KiB
    NormalFetch,     // <= 8 MiB
    LargeFetch,      // <= 256 MiB
    ArchiveOnly,     // > 256 MiB or policy-restricted
}
```

### 5.1 Recommended Limits

- `InlineControl`: may travel directly over pubsub if schema-validated
- `NormalFetch`: advertised on pubsub, fetched separately
- `LargeFetch`: never in pubsub; request-response or provider fetch only
- `ArchiveOnly`: off the live swarm path by default; require explicit opt-in fetch

This keeps pubsub traffic bounded and reduces amplification and memory-pressure risk.

---

## 6. Retrieval Model

Recommended retrieval order:

1. local content-addressed cache
2. explicitly connected peer provider
3. known community provider set
4. DHT/provider lookup
5. optional external IPFS pinning gateway or local IPFS node

### 6.1 Best-Practice Fetch Rules

- Never auto-fetch large attachments solely because a pubsub announcement was received.
- Validate the manifest first.
- Enforce max bytes and accepted media types before retrieval.
- Fetch on demand or under explicit policy.

---

## 7. Validation and Safety

Before accepting or forwarding a blob:

1. Verify CID matches the declared body bytes.
2. Verify signature over `(cid, schema_version, kind, created_at_ms)`.
3. Verify schema and kind-specific constraints.
4. Verify attachment list is bounded.
5. Verify declared byte counts are within policy.

### 7.1 Required Guards

- maximum nesting depth for manifests
- decompression ratio cap
- maximum attachment fan-out
- maximum number of unresolved missing refs before dropping
- duplicate CID suppression

These are straightforward anti-abuse controls used widely in content-addressed and pubsub systems.

---

## 8. Kind-Specific Conventions

### 8.1 `EngramEnvelope`

- root should be the serialized `TransferProfile`
- attachments may contain `AdapterWeights`, eval bundles, lineage summaries, or receipts

### 8.2 `IndexSegment`

- root should be the queryable segment manifest
- large segment blocks should remain external attachments

### 8.3 `ReceiptBatch`

- root should be a compact CBOR structure
- attachment use should be rare

### 8.4 `AppletPackage`

- prefer `CarV1` packaging with explicit manifest and signature bundle
- no implicit execution on fetch

---

## 9. Replication and Retention

Nodes should track retention independently from addressing.

Suggested retention classes:
- `ephemeral`
- `cached`
- `pinned-local`
- `pinned-community`
- `archival`

Addressing says what the content is.
Retention says how long the node keeps serving it.

---

## 10. Immediate Defaults (v1)

- Use CIDv1 base32 with `sha2-256`.
- Use `DagCbor` for envelopes and manifests.
- Inline only small control data.
- Use `CarV1` for multi-object bundled exports.
- Treat pubsub as announcement/manifest distribution, not large payload transport.
- Require strict validation and bounded decoding before forwarding.

These defaults make VerseBlob practical, interoperable, and resilient without forcing a full IPFS dependency model.
