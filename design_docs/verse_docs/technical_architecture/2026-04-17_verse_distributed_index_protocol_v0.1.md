# Verse Distributed Index Protocol (VDIP) v0.1

**Status:** Draft v0.1
**Document type:** Core protocol specification with reference profiles and Graphshell mapping
**Audience:** Protocol design, Graphshell implementation planning, future interop work

This document promotes the April 16 draft set into a single canonical spec for the Verse distributed index protocol. It defines the protocol core, keeps transport and ecosystem choices out of the normative layer where possible, and records the current Graphshell implementation boundary explicitly.

## 1. Scope

VDIP defines how peers exchange derived search knowledge as signed, immutable, content-addressed artifacts.

The protocol core covers:

- protocol objects and signing rules
- content canonicalization and compatibility profiles
- community admission and revocation semantics
- search and ranking semantics over accepted local artifacts

The protocol core does not require:

- public DHT-scale discovery
- raw HTML or WARC redistribution by default
- global cross-community federation
- on-chain economics, FLora, or Nostr/DVM integration

Graphshell remains a host and renderer. Verso remains the bilateral peer layer. VDIP belongs to the community-scale Verse layer.

## 2. Design Principles

- Local-first: capture, canonicalization, indexing, and trust evaluation happen locally.
- Immutable artifacts: shared artifacts are append-only and content-addressed.
- Derived-data sharing by default: raw captured content remains local unless explicitly exported.
- Separation of durable truth and acceleration: durable observations stay valid even if the local search engine changes.
- Transport independence: the protocol core defines bytes and semantics, not one mandatory transport stack.

## 3. Normative Language

The key words `MUST`, `SHOULD`, and `MAY` are normative.

## 4. Terminology

| Term | Definition |
|------|------------|
| `ObservationCard` | One canonicalized observation of one URL at one time, plus derived metadata. |
| `SplitPackage` | Optional acceleration artifact containing a prebuilt search index bundle and manifest. |
| `CommunityManifest` | Community-scoped state defining governance, profile compatibility, and the active commit head. |
| `IndexCommit` | Signed append-only commit that adds artifacts and revocation references to a community history. |
| `RevocationRecord` | Immutable tombstone-like record removing an artifact from the active accepted set without rewriting history. |
| `ValidatorReceipt` | Optional signed validation result about a candidate artifact. |
| `index_profile_hash` | BLAKE3 hash of the canonical index compatibility profile. |

## 5. Identity, Serialization, and Addressing

- Identities `MUST` be Ed25519 keypairs.
- Signed objects `MUST` be serialized as canonical CBOR.
- Signatures `MUST` cover canonical manifest bytes, never opaque compressed payload bytes.
- Content identity `MUST` use raw BLAKE3-256.
- Implementations `MAY` expose those digests through a CIDv1 wrapper, but raw BLAKE3 is normative.
- Archive packaging `MUST` be deterministic before hashing.

Rationale: raw BLAKE3 stays close to the underlying blob-transfer model while preserving a clean path to CIDv1 interop later.

## 6. Compatibility Profiles

VDIP defines three compatibility layers. Implementations `MUST` treat these profiles as part of interoperability, not as purely local implementation detail.

### 6.1 CanonicalizationProfile

Defines:

- extraction method
- normalization rules
- volatile-field stripping
- whitespace and token normalization rules

### 6.2 FingerprintProfile

Defines:

- exact-hash algorithm
- near-duplicate algorithm
- shingling rules
- parameterization and seeds

For v0.1, implementations `MUST` support MinHash as the near-duplicate baseline. Parameterization remains profile-defined.

### 6.3 IndexProfile

Defines:

- index schema
- tokenizer and analyzer chain
- canonicalization profile version
- fingerprint profile version
- search engine compatibility markers

The compatibility fingerprint is:

```text
index_profile_hash = BLAKE3(
    canonical_cbor({
        canonicalization_profile,
        fingerprint_profile,
        index_schema,
        tokenizer_config,
        analyzer_config,
        engine_compatibility,
    })
)
```

`SplitPackage`s are binary-compatible only when their `index_profile_hash` matches an accepted profile for the target community.

## 7. Content Canonicalization Pipeline

Before an observation is recorded, content `MUST` be canonicalized:

1. Extraction: readability-style main-content extraction or equivalent profile-defined algorithm.
2. Normalization: remove volatile tokens, session identifiers, ad markup, and profile-declared noise.
3. Exact fingerprinting: compute BLAKE3 over canonical extracted text or fields.
4. Near-duplicate fingerprinting: compute MinHash over profile-defined shingles.
5. UDC tagging: derive semantic tags from explicit user annotation, classifiers, or both.

Raw HTML, WARC payloads, screenshots, and similar captures `SHOULD` remain local by default.

## 8. Core Protocol Objects

### 8.1 ObservationCard

```rust
struct ObservationCard {
    url: String,
    observed_at: u64,
    title: Option<String>,
    snippet: Option<String>,
    content_hash: [u8; 32],
    minhash: Vec<u64>,
    udc_tags: Vec<String>,
    contributor: [u8; 32],
    profile_hash: [u8; 32],
}

struct SignedObservation {
    card: ObservationCard,
    signature: [u8; 64],
}
```

Per-card signatures are normative in v0.1. Batch signing is deferred to a later extension.

### 8.2 SplitPackageManifest

```rust
struct SplitPackageManifest {
    package_id: [u8; 32],
    payload_size: u64,
    profile_hash: [u8; 32],
    min_observed_at: u64,
    max_observed_at: u64,
    observation_count: u64,
    observation_refs: Vec<[u8; 32]>,
    udc_summary: Vec<(String, u32)>,
    contributor: [u8; 32],
    supersedes: Vec<[u8; 32]>,
    created_at: u64,
}

struct SignedSplitPackage {
    manifest: SplitPackageManifest,
    signature: [u8; 64],
}
```

The payload is an implementation-defined, deterministic archive of search-engine artifacts. The manifest is the signed and gossiped unit.

### 8.3 CommunityManifest

```rust
struct CommunityManifest {
    community_id: [u8; 32],
    name: String,
    description: String,
    udc_scope: Vec<String>,
    primary_profile: [u8; 32],
    accepted_profiles: Vec<[u8; 32]>,
    admin_keys: Vec<[u8; 32]>,
    moderator_keys: Vec<[u8; 32]>,
    ranking_weights: RankingWeights,
    invite_policy: InvitePolicy,
    preferred_head: Option<[u8; 32]>,
    head_epoch: u64,
}

struct RankingWeights {
    bm25: f32,
    udc_match: f32,
    trust: f32,
    freshness: f32,
    novelty: f32,
}
```

`accepted_profiles` allows explicit dual-profile migration periods. `preferred_head` resolves the active commit-set question for v0.1.

### 8.4 IndexCommit

```rust
struct IndexCommit {
    commit_id: [u8; 32],
    parents: Vec<[u8; 32]>,
    community_id: [u8; 32],
    added_observations: Vec<[u8; 32]>,
    added_packages: Vec<[u8; 32]>,
    added_receipts: Vec<[u8; 32]>,
    revocations: Vec<[u8; 32]>,
    timestamp: u64,
    author: [u8; 32],
}

struct SignedCommit {
    commit: IndexCommit,
    signature: [u8; 64],
}
```

The commit graph is a DAG. Consumers `MUST` treat the active accepted set as the closure of commits reachable from the community's `preferred_head`.

### 8.5 RevocationRecord

```rust
enum ArtifactKind {
    Observation,
    SplitPackage,
    ValidatorReceipt,
}

struct RevocationRecord {
    revocation_id: [u8; 32],
    community_id: [u8; 32],
    target_kind: ArtifactKind,
    target_id: [u8; 32],
    reason_code: String,
    note: Option<String>,
    revoked_at: u64,
    revoked_by: [u8; 32],
}

struct SignedRevocation {
    record: RevocationRecord,
    signature: [u8; 64],
}
```

Revocation removes an artifact from the active set but does not rewrite historical records.

### 8.6 ValidatorReceipt

```rust
enum ValidationDecision {
    Accept,
    Reject,
    Warn,
}

struct ValidatorReceipt {
    receipt_id: [u8; 32],
    community_id: [u8; 32],
    subject_kind: ArtifactKind,
    subject_id: [u8; 32],
    validator: [u8; 32],
    decision: ValidationDecision,
    reason_code: Option<String>,
    observed_at: u64,
    expires_at: Option<u64>,
}

struct SignedValidatorReceipt {
    receipt: ValidatorReceipt,
    signature: [u8; 64],
}
```

Validator receipts are optional in v0.1. Communities `MAY` require them in local admission policy, but the core protocol does not require them for basic interoperability.

## 9. Community State and Head Selection

VDIP resolves commit-head selection minimally in v0.1:

- The authoritative active head is `CommunityManifest.preferred_head`.
- Only admin keys `MUST` be allowed to advance `preferred_head`.
- Consumers `MUST` apply only commits reachable from `preferred_head` when computing the active accepted set.
- Consumers `MAY` maintain local provisional heads for staging or review, but those heads `MUST NOT` be treated as canonical community state unless published through an admin-authorized manifest update.
- `head_epoch` `MUST` increase monotonically when `preferred_head` changes.

This keeps DAG history available without leaving active-state selection undefined.

## 10. Admission, Revocation, and Profile Migration

- Communities `MUST` define an invite or access policy.
- Communities `MAY` apply additional admission checks using validator receipts, trust scores, or local moderation rules.
- Revoked artifacts `MUST NOT` remain in the active accepted set after the relevant `RevocationRecord` becomes reachable from the preferred head.
- Communities `SHOULD` use `accepted_profiles` to support controlled profile migration.
- During migration, communities `MAY` accept observations from more than one profile, but `SplitPackage` mounting still requires exact binary compatibility with one accepted profile.

## 11. Trust and Ranking

Trust is community-local and implementation-defined, but the protocol assumes these inputs are available to local policy:

- invite lineage
- endorsements and flags
- identity age
- utilization in local search
- novelty relative to existing community content

Implementations `MAY` compute trust using EigenTrust-like or other graph-based algorithms.

Recommended enforcement rule for v0.1:

- trust gates `SplitPackage` mounting and optional artifact admission
- query-time ranking combines textual score with UDC relevance, trust, freshness, and novelty

## 12. Search Semantics

Search execution `MUST` be local over the consumer's accepted artifacts.

1. Scope the query to one or more communities.
2. Gather locally accepted observations and mounted split packages reachable from each community's preferred head.
3. Query across the local search engine's readers.
4. Deduplicate exact matches by `content_hash`.
5. Cluster near-duplicates using MinHash similarity.
6. Rank with community-defined weights.
7. Return results with provenance.

The composite score is community-defined but typically has the form:

```text
score = w_bm25    * bm25
      + w_udc     * udc_match
      + w_trust   * contributor_trust
      + w_fresh   * freshness_decay(observed_at)
      + w_novelty * novelty_vs_community
```

Global mandatory merge is out of scope for v0.1. Fan-out across accepted local artifacts is sufficient.

## 13. Privacy Baseline

- Raw capture data `SHOULD` remain local by default.
- Shared artifacts `SHOULD` prefer derived search data over raw page archives.
- Query privacy is separate from artifact privacy and is deferred beyond v0.1.
- Implementations `SHOULD` support local-only search over already-fetched artifacts so remote query disclosure is not mandatory.

## 14. Reference Profiles

These profiles are reference implementation guidance, not mandatory protocol rules.

### 14.1 Profile A: Iroh-First Trusted Exchange

- blob transfer: `iroh-blobs`
- community announcements: `iroh-gossip`
- replicated community state: `iroh-docs`
- connectivity and relays: `iroh`

This is the most direct fit for early Graphshell experimentation.

### 14.2 Profile B: Broader Discovery Overlay

Public discovery overlays such as libp2p-based routing or other announcement fabrics may be added later. They are not required for v0.1 conformance.

## 15. Graphshell Implementation Mapping

The protocol is ahead of the current Graphshell implementation. These reuse points and gaps are the current boundary:

- [../../../mods/native/verse/mod.rs](../../../mods/native/verse/mod.rs) already provides Ed25519-backed iroh identity, trusted-peer storage, and workspace-grant concepts that can seed community identity and admission work.
- [../../../model/archive.rs](../../../model/archive.rs) already contains signed portable archive objects and privacy classes; that is a useful precedent for signed artifact envelopes, but it is not yet VDIP's object model.
- [../../../app/clip_capture.rs](../../../app/clip_capture.rs) already exposes structured capture data from web content; this is a precursor to canonicalization, not the canonicalization pipeline itself.
- [../../../services/query/mod.rs](../../../services/query/mod.rs) and [../../../services/facts/mod.rs](../../../services/facts/mod.rs) provide local structured querying over projected history facts, but they do not yet implement distributed observations, split-package search, or community ranking.

Missing pieces before Graphshell can claim a VDIP implementation:

- canonicalization profile machinery
- BLAKE3 and MinHash artifact fingerprinting for observations
- Tantivy or equivalent split-package production and mounting
- community manifest replication beyond bilateral sync
- commit and revocation application logic
- trust-graph computation for admission or mount-time gating

## 16. Rollout Order

1. ObservationCard serialization, canonical CBOR, and per-card signing.
2. Canonicalization profiles plus BLAKE3 and MinHash generation.
3. Local-only community manifest and preferred-head handling.
4. Local search across accepted observations grouped by community.
5. SplitPackage production and mounting.
6. Two-peer artifact exchange using a reference transport.
7. Community trust and optional validator receipts.
8. Broader discovery overlays and advanced privacy features.

## 17. Deferred Work

- batch-signing extensions
- private community discovery with existence-hiding properties
- advanced query privacy
- mandatory public discovery overlays
- cross-community search federation
- raw snapshot redistribution by default
- economics, FLora, and Nostr/DVM integration

## 18. References

- [2026-04-16_verse_index_protocol_drafts.md](2026-04-16_verse_index_protocol_drafts.md)
- [VERSE_AS_NETWORK.md](VERSE_AS_NETWORK.md)
- [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md)
