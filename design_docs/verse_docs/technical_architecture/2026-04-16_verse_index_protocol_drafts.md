# Verse Distributed Index Protocol — v0 Draft Spec

> Historical draft compilation. The current canonical consolidation now lives in [2026-04-17_verse_distributed_index_protocol_v0.1.md](2026-04-17_verse_distributed_index_protocol_v0.1.md).

**Status:** Draft. Synthesizes discussion around distributed search for graphshell/verse. Intended as a starting point for implementation, not a final specification. Normative sections use RFC 2119-style language; rationale and deferred work are called out explicitly.

---

## 1. Goals

- Enable peers to selectively index their own browsing and share derived indices with topic-scoped communities.
- Use UDC semantic typing as a first-class community-level relevance filter.
- Exchange content-addressed, signed, independently-verifiable artifacts.
- Achieve sybil resistance through invite chains and reputation, not proof-of-work.
- Decouple the *network protocol unit* from the *local indexing strategy* so the indexer can evolve without breaking the network.

### Non-goals (v0)

- DHT-scale open peer discovery.
- Unbounded public communities.
- Sharing raw HTML snapshots by default.
- Cross-community federated search.

---

## 2. Terminology

| Term | Definition |
|------|------------|
| **ObservationCard** | The primary protocol unit. One URL observed at one point in time, plus derived metadata. |
| **SplitPackage** | Acceleration-layer artifact: a prebuilt, signed tantivy index bundle. Optional. |
| **CommunityManifest** | Replicated community-scoped state: membership, ranking weights, compatibility profile. |
| **IndexCommit** | Signed append-only log entry recording acceptance of ObservationCards or SplitPackages into a community. |
| **Peer / Contributor** | Identity is an ed25519 keypair. |
| **index_profile_hash** | Full compatibility fingerprint for binary segment interop. |

---

## 3. Identity and Signing

- All identities are ed25519 keypairs (`ed25519-dalek`).
- All signed objects are serialized as canonical CBOR (`ciborium`) with deterministic map ordering.
- Signatures cover canonical-CBOR-encoded manifests, never raw compressed bytes.
- All content addresses use raw BLAKE3 (not CIDv1/multihash in v0; reconsider if IPFS interop is ever required).

Rationale: signing manifests (not compressed bytes) means artifacts can be re-compressed or repackaged without invalidating signatures. Raw BLAKE3 aligns with iroh-blobs' native hash so content addresses are directly transferable.

---

## 4. Content Processing Pipeline

Before any observation is recorded, content MUST be canonicalized:

1. **Extraction**: readability-style main-content extraction (e.g., Mozilla readability algorithm port).
2. **Normalization**: strip ads, tracking pixels, session tokens, volatile timestamps in the DOM; normalize whitespace.
3. **Fingerprinting**:
   - `content_hash: [u8; 32]` — BLAKE3 over canonical extracted text. Exact-identity.
   - `minhash: [u64; N]` — MinHash signature over shingled tokens. Near-duplicate detection. N ∈ {64, 128, 256} (profile-dependent).
4. **UDC tagging**: derived from community-selected classifiers or explicit user annotation.

The canonicalization spec is part of `index_profile_hash` — two peers producing different canonical output for the same URL will not interoperate at the exact-match level.

---

## 5. Core Data Types

### 5.1 ObservationCard

The primary protocol unit. Represents one observation of one URL.

```rust
struct ObservationCard {
    url: String,                    // canonical URL
    observed_at: u64,               // unix seconds
    title: Option<String>,
    snippet: Option<String>,        // short extract, for preview
    content_hash: [u8; 32],         // BLAKE3 of canonical text
    minhash: Vec<u64>,              // MinHash signature
    udc_tags: Vec<String>,          // semantic facet codes
    contributor: [u8; 32],          // ed25519 pubkey
    profile_hash: [u8; 32],         // index_profile_hash of producer
}

struct SignedObservation {
    card: ObservationCard,          // canonical CBOR
    signature: [u8; 64],            // ed25519 over canonical bytes
}
```

Design note: signing every card individually costs O(N) verifications. For production, batch-sign via a Merkle root over a chunk of cards with one signature covering the root. v0 may use per-card signing; the batch path is deferred, not foreclosed.

### 5.2 SplitPackageManifest

Acceleration layer. A signed manifest referencing a prebuilt tantivy index bundle as a BLAKE3-addressed blob.

```rust
struct SplitPackageManifest {
    package_id: [u8; 32],           // BLAKE3 of bundle payload
    payload_size: u64,
    profile_hash: [u8; 32],         // index_profile_hash
    min_observed_at: u64,
    max_observed_at: u64,
    observation_count: u64,
    observation_refs: Vec<[u8; 32]>, // content_hash of each card
    udc_summary: Vec<(String, u32)>, // facet histograms
    contributor: [u8; 32],
    supersedes: Vec<[u8; 32]>,       // prior package_ids this replaces
    created_at: u64,
}

struct SignedSplitPackage {
    manifest: SplitPackageManifest,
    signature: [u8; 64],
}
```

The payload (tantivy segment bundle) is zstd-compressed, transferred via iroh-blobs, and addressed by `package_id`. The manifest is what gets gossiped, verified, and indexed; the payload is fetched on demand.

### 5.3 index_profile_hash

```
index_profile_hash = BLAKE3(
    canonical_cbor({
        "schema": tantivy_schema_json,
        "tokenizer": tokenizer_config,
        "analyzer_chain": analyzer_config,
        "canonicalization": canonicalization_spec_version,
        "minhash_params": { "n": N, "seed": SEED },
        "tantivy_major_minor": "0.24",
    })
)
```

Two peers can exchange binary SplitPackages iff their `profile_hash` matches. ObservationCards are profile-agnostic at the data level but consumers must know which profile was used to reproduce the fingerprints.

### 5.4 CommunityManifest

Replicated via iroh-docs. Defines a community.

```rust
struct CommunityManifest {
    community_id: [u8; 32],         // BLAKE3 of founding manifest
    name: String,
    description: String,
    udc_scope: Vec<String>,          // facet codes this community covers
    profile_hash: [u8; 32],          // accepted index profile
    admin_keys: Vec<[u8; 32]>,
    moderator_keys: Vec<[u8; 32]>,
    ranking_weights: RankingWeights, // per-axis weights for ranking
    invite_policy: InvitePolicy,     // open / invite-chain / contribution-gated
    relay_url: Option<String>,       // optional community-operated iroh relay
}

struct RankingWeights {
    bm25: f32,
    udc_match: f32,
    trust: f32,
    freshness: f32,
    novelty: f32,
}
```

Mutations are CRDT merges via iroh-docs with write access gated by admin/moderator keys.

### 5.5 IndexCommit

Append-only log of accepted contributions. Each commit is signed by an admin or moderator.

```rust
struct IndexCommit {
    commit_id: [u8; 32],            // BLAKE3 of canonical body
    parent: Option<[u8; 32]>,       // prior commit in chain
    community_id: [u8; 32],
    added_observations: Vec<[u8; 32]>,    // content_hashes
    added_packages: Vec<[u8; 32]>,        // package_ids
    revoked_observations: Vec<[u8; 32]>,
    revoked_packages: Vec<[u8; 32]>,
    timestamp: u64,
    author: [u8; 32],
}

struct SignedCommit {
    commit: IndexCommit,
    signature: [u8; 64],
}
```

The commit graph is a DAG (forking supported), not strictly linear. **Head selection** is the principal open question — see §9.

---

## 6. Network Layer

Single transport dependency in v0: **iroh**.

| Role | Crate | Purpose |
|------|-------|---------|
| Data plane | `iroh-blobs` | Transfer SplitPackage payloads by BLAKE3 hash with verified streaming |
| Announcements | `iroh-gossip` | One pub/sub topic per community for new commits, manifests, observations |
| Community state | `iroh-docs` | CommunityManifest replication with gated writes |
| Connectivity | `iroh` core | QUIC + relay fallback, ed25519 pubkey dialing |

### Relays

Communities MAY operate their own iroh relay. Members joining a community automatically add the community relay as a home relay alongside public defaults. Relay metadata (which pubkeys connect to whom) stays within community infrastructure when a community relay is used.

### Deferred

- **libp2p**: open DHT-scale community discovery. Reconsider when public unbounded communities are a real use case.
- **p2panda-auth / p2panda-encryption**: group encryption with post-compromise security for private communities. Adopt piecemeal if/when community-wide E2E encryption is required.

---

## 7. Trust and Ranking

### 7.1 Contribution graph

Maintained locally via `petgraph`:

- Nodes: contributor pubkeys.
- Edges: trust signals with typed weights.
  - `Invite(a → b)`: a vouched for b joining a community.
  - `Endorse(a → b, community_id)`: a positively evaluated b's contributions.
  - `Flag(a → b, community_id)`: a flagged b's contributions as low-quality or malicious.

### 7.2 EigenTrust iteration

Global trust scores are computed per-community via iterated normalized matrix-vector multiplication over the trust graph, anchored by the community's admin keys as pre-trusted peers. Convergence typically <20 iterations for typical community sizes. Implementation does not require the deprecated `eigen-trust` crate — the math is straightforward over `petgraph`.

### 7.3 Multi-axis local signals

Local trust feeding into EigenTrust is derived from:

- **Utilization**: how often a contributor's cards appear in local search results the user interacts with.
- **Novelty**: MinHash similarity of contributed content to the existing community index. High-novelty contributions score higher.
- **Moderator endorsements**: direct positive signals.
- **Flags**: direct negative signals, with flag-weight proportional to flagger's own trust score.
- **Identity age**: probation discount for keys younger than community threshold.

### 7.4 Mount-time gate

Trust is enforced at segment-mount time, not at query time:

- When a new SplitPackage is accepted into a commit, Graphshell checks the contributor's global trust score against the community's threshold.
- Below threshold: the package is **not mounted** into the local tantivy reader set.
- Above threshold: mounted and available to all subsequent queries.

This keeps the search hot path fast — untrusted segments never enter the reader set.

### 7.5 Sybil resistance (policy, not crypto)

- **Invite chain**: new identities require a voucher from an existing member. Voucher's trust is at stake for vouchee's behavior.
- **Probation**: identities younger than N days are down-weighted in ranking regardless of trust score.
- **Rate limits**: per-identity caps on contributions per time window.
- **Stake**: optional per-community — vouchers forfeit reputation when their invitees are flagged.

---

## 8. Search

1. User issues a query scoped to one or more communities.
2. Graphshell fans out across all locally-mounted tantivy readers for those communities.
3. Hits are deduplicated by `content_hash` (exact) and clustered by MinHash Jaccard similarity (near-dup) within a threshold.
4. Composite ranking:

   ```
   score = w_bm25    * bm25
         + w_udc     * udc_match
         + w_trust   * contributor_trust
         + w_fresh   * freshness_decay(observed_at)
         + w_novelty * novelty_vs_community
   ```

   Weights come from the community's `RankingWeights`.

5. Top-K results returned to the UI, with provenance (contributor pubkey, commit id, community).

No cross-index merging. Fan-out is the mechanism.

---

## 9. Open Questions

- **IndexCommit head selection.** Who publishes the head? Does every member track their own head and reconcile? Is there a community-wide canonical head maintained by admins? Needs a concrete answer before the commit DAG is useful.
- **Batch signing.** Per-card signatures don't scale past ~10^6 cards per index. Merkle-root batch signing is the standard answer; needs a specific scheme chosen.
- **Private community discovery.** How do invite-only communities exist without leaking their existence? Private Set Intersection (as in p2panda-discovery) is the known approach; requires evaluation.
- **Revocation semantics.** When a contributor is revoked, are prior commits rewritten? Marked? Left as historical record?
- **Schema evolution.** How do communities upgrade `profile_hash` without fragmenting? Likely a dual-profile transition period with explicit migration commits.

---

## 10. Explicitly Deferred

- libp2p / DHT-scale discovery
- automerge for human-edited metadata (likely adopt later for moderation logs and policy docs, not binary index artifacts)
- p2panda auth/encryption primitives
- CIDv1 / multihash wrapping
- Cross-community search federation
- WARC-style raw snapshot sharing (legally higher-risk; opt-in only)

---

## 11. Recommended v0 Implementation Order

1. `ObservationCard` + canonical CBOR + ed25519 signing.
2. Canonicalization pipeline (readability port, MinHash, BLAKE3).
3. `CommunityManifest` stored in iroh-docs; local-only (single-peer) path working end-to-end.
4. Fan-out tantivy search across local observations grouped by community.
5. `SplitPackage` production and mounting.
6. iroh-gossip announcements; two-peer sync working.
7. Trust graph in petgraph; EigenTrust iteration; mount-time gate.
8. `IndexCommit` DAG + head selection (once §9 resolved).

Each step validates the previous one independently. The protocol should be functional at step 4 for a single peer and at step 6 for a federation.

---

## References

- Tantivy: https://github.com/quickwit-oss/tantivy
- iroh: https://www.iroh.computer/docs/overview
- iroh-blobs: https://docs.rs/iroh-blobs
- iroh-docs: https://docs.rs/iroh-docs
- iroh-gossip: https://docs.rs/iroh-gossip
- EigenTrust paper: Kamvar, Schlosser, Garcia-Molina (2003)
- MinHash: Broder (1997)
- BLAKE3: https://github.com/BLAKE3-team/BLAKE3
- ed25519-dalek: https://docs.rs/ed25519-dalek
- ciborium: https://docs.rs/ciborium
- petgraph: https://docs.rs/petgraph

---

# Draft 1 — Graphshell Decentralized Search Architecture (Verse Tier 2)

Term: Sector C Supplement / Verse Intelligence Incubation
Status: Architecture Draft
Goal: A peer-to-peer, community-curated search engine where users selectively broadcast derived semantic indices of their browsing history without relying on centralized crawlers or storage.

## 1. Core Data Structures

The system architecture separates the "durable truth" (the Observation log) from the "acceleration structure" (the Search Index).

### 1.1 The Observation Card

The ObservationCard is the atomic unit of the network. It represents a single, user-verified snapshot of a web resource.

Format: canonical CBOR (DAG-CBOR).
Content Address: CIDv1 using multihash(BLAKE3).
Fields:
- url: The canonicalized URL.
- canonical_hash: BLAKE3 hash of the normalized, stripped HTML body.
- simhash: A 64-bit SimHash (or custom MinHash sketch) over the extracted text for near-duplicate screening.
- udc_tags: Array of canonical semantic tags.
- title & snippet: Derived metadata for search display.
- timestamp: ISO-8601 timestamp of the fetch.
- contributor_pubkey: Ed25519 public key of the authoring peer.
- signature: Ed25519 signature over the CBOR bytes.

Note: Raw WARC/HTML captures are strictly local and not gossiped by default to minimize copyright liability and bandwidth.

### 1.2 The Community Manifest (IndexCommit)

A CommunityManifest is an append-only, signed Log or CRDT (e.g., via p2panda) that tracks the state of a community's index.

Form: Replicated log / Merkle DAG.
Fields:
- added_observations: Array of ObservationCard CIDs.
- revoked_observations: Array of CIDs explicitly tombstoned by the community.
- trust_edges: Endorsements or flags (Key A + Weight + Key B) used for calculating sybil resistance.
- published_splits: Optional array of SplitPackage CIDs (acceleration layer).

### 1.3 The Acceleration Layer (SplitPackage)

For large communities, rebuilding the index from millions of ObservationCards is slow. Trusted peers periodically compile and sign SplitPackage blobs.

Format: Zstd-compressed tarball of a sealed Tantivy segment.
Metadata Profile: Requires an exact match of the index_profile_hash (Tantivy version, schema hash, tokenizer config) for safe local mounting.
Range Data: min_snapshot_at and max_snapshot_at for cheap routing.

## 2. Network Topology

The P2P layer is explicitly decomposed by responsibility rather than using a single monolithic framework.

### 2.1 Blob Transfer and Transport (iroh)

Role: The data plane.
Mechanics: QUIC transport, NAT traversal, and iroh-blobs for verified BLAKE3 blob/range transfer.
Usage: Fetching ObservationCards, SplitPackages, and handling bilateral trusted connections.

### 2.2 Community State and Access Control (p2panda)

Role: The control plane for moderated groups.
Mechanics: Replicated DAGs, eventual consistency, and invite-gated access control.
Usage: Syncing the CommunityManifest, tracking ACLs, moderation logs, and maintaining the index chain.

### 2.3 Public Discovery (libp2p) Optional

Role: The open swarm control plane.
Mechanics: Kademlia DHT, GossipSub.
Usage: If the community is public, this allows peers to discover the community manifest bootstrap nodes. If invite-only, libp2p can be omitted in favor of direct peer dials.

## 3. Trust and Relevance Scoring

Search relevance is not merely textual; it is inextricably linked to the social trust graph of the community.

### 3.1 Local EigenTrust Integration (petgraph)

Rather than a generic reputation crate, Graphshell runs an iterative, PageRank-style EigenTrust algorithm locally using petgraph.
Inputs: Invite lineages, explicit endorsements, moderation approvals, and flag rates extracted from the CommunityManifest.
Sybil Resistance: New identities are naturally partitioned or heavily discounted until vouched for. Vouchers stake their own reputation on their invitees.

### 3.2 The Composite Ranking Function

When a query is executed, the Search UI ranks hits using a composite score defined by the community:

- Textual TF-IDF/BM25 (via Tantivy).
- Freshness (via ObservationCard timestamp).
- UDC Relevance (exact or hierarchical facet matches).
- Novelty (SimHash distance penalty for heavily duplicated content).
- Contributor Trust (The local EigenTrust scalar).

## 4. Execution Lifecycle

Path 1: Generating Knowledge
- Capture: User browser strips the DOM, extracts text, computes BLAKE3 and SimHash.
- Seal: Graphshell mints an ObservationCard, signs it, and writes it to local storage.
- Gossip: If the user opted to share with Community X, Graphshell appends the CID to the p2panda Community Manifest and seeds the CBOR blob via iroh.

Path 2: Consuming Knowledge
- Sync: Graphshell syncs the p2panda log for Community X.
- Filter: The local petgraph algorithm discounts dropped/flagged contributors.
- Fetch: Valid, missing ObservationCard CIDs are pulled via iroh-blobs.
- Index/Compile: The local Tantivy runner consumes the cards, adding them directly to the local search index, compacting segments freely without breaking network hashes.
- Accelerate: If a trusted community peer publishes a SplitPackage replacing 50,000 cards, Graphshell can download the BLAKE3 tarball via iroh-blobs and hot-mount it into the Tantivy Searcher.

---

# Draft 2 — Graphshell Verse Federated Search Spec v0.1

## Status

- Draft
- Intended scope: local-first indexing, signed community artifact exchange, and federated search

## Normative Language

- MUST, SHOULD, and MAY are normative.

## 1. Goal

- Graphshell MUST let users build a local search index from selectively captured browsing data.
- Users MUST be able to share derived index artifacts with communities without requiring raw browsing history disclosure.
- Communities MUST be able to accept, reject, rank, and revoke contributed artifacts.
- Search MUST work without mandatory global index merging.

## 2. Non-Goals

- The protocol does not require raw HTML redistribution.
- The protocol does not require a global public network in v0.1.
- The protocol does not require economic incentives or on-chain settlement.

## 3. Architecture

- The system MUST be local-first.
- Local capture, canonicalization, indexing, and trust policy evaluation MUST happen on the user’s device.
- Shared artifacts MUST be immutable.
- Community state MUST be expressed as signed append-only records over immutable artifacts.
- Transport and storage SHOULD be separated from trust and ranking.

## 4. Core Objects

- ObservationRecord: one captured page or snapshot-derived unit.
- SplitPackage: one immutable compressed artifact containing compatible Tantivy segment files plus manifest metadata.
- IndexCommit: one signed record that adds, removes, supersedes, or revokes shared artifacts for a community.
- ValidatorReceipt: one signed attestation that a package was checked against community policy.
- CommunityManifest: one signed definition of community scope, access rules, and trust policy.

## 5. Index Profile

- Shared packages MUST declare an index_profile_hash.
- index_profile_hash MUST cover schema, tokenizers, analyzers, canonicalization rules, fingerprint profile, and Tantivy/index-format compatibility.
- Packages with incompatible index_profile_hash values MUST NOT be mounted into the same compatible search view without explicit translation.

## 6. ObservationRecord

- Each record MUST include a stable content address, source URL, capture timestamp, canonical text or extracted fields, and provenance fields needed for ranking.
- Each record SHOULD include canonical UDC tags, site/domain metadata, and optional language metadata.
- Each record MUST retain its individual timestamp even when package-level timestamp ranges are present.

## 7. Similarity Signals

- Exact identity MUST use BLAKE3 over canonical bytes.
- Near-duplicate detection SHOULD use text-derived SimHash or MinHash.
- Implementations MAY add DOM-structure hashes or perceptual hashes for rendered/media cases.
- Canonicalization MUST be deterministic enough that materially equivalent pages usually produce equivalent or near-equivalent fingerprints.

## 8. SplitPackage Format

A SplitPackage MUST contain:
- manifest
- one or more immutable Tantivy segment files
- optional sidecar fingerprint data
- optional package-local summaries for discovery

The package manifest MUST include:
- package_hash
- index_profile_hash
- contributor_pubkey
- signature
- min_snapshot_at
- max_snapshot_at
- document count
- UDC summary
- fingerprint summary
- referenced artifact hashes

Package archives MUST be deterministic before hashing and signing.
Compression SHOULD use zstd.

## 9. Signing and Addressing

- Canonical manifest bytes MUST be signed with ed25519-dalek.
- Artifacts SHOULD use content addressing based on BLAKE3.
- If multiformat interoperability is desired, artifact identifiers SHOULD be representable as CIDv1 with a BLAKE3 multihash.

## 10. Community State

- Community state MUST be represented as signed append-only IndexCommit records.
- IndexCommit MUST support:
- add package
- remove package
- supersede package
- revoke package
- attach validator receipt
- fork lineage

CRDTs are optional here; a signed append-only Merkle-style log is sufficient for v0.1.

## 11. Search Semantics

- Implementations MUST support search across multiple accepted local packages.
- Global merge is NOT required.
- Local compaction or merge MAY be performed as an optimization.
- Search results SHOULD collapse exact and near-duplicate hits using package fingerprints and trust-aware grouping logic.

## 12. Trust and Admission

- Communities MUST define an admission policy.
- Admission policy MAY be invite-only, validator-gated, contribution-gated, or open with trust thresholds.
- Implementations SHOULD maintain a trust graph over contributor identities.
- Trust inputs SHOULD include package utilization, novelty, validator outcomes, endorsements, and identity age.
- New identities SHOULD be down-weighted until they establish history.

## 13. Revocation and Moderation

- Revocation MUST be first-class.
- A revoked package MUST remain immutable but MUST be removable from the active accepted search set.
- Communities SHOULD support denylisted hashes, supersession, and validator-issued policy rejections.

## 14. Networking

- Private and trusted artifact transfer SHOULD use iroh plus iroh-blobs.
- Community announcement and lightweight coordination SHOULD use iroh topic/doc primitives in early versions.
- Larger or more adversarial public swarms MAY add libp2p later for broader discovery and routing.
- chitchat is NOT sufficient as the primary community network substrate.

## 15. Privacy

- Raw captured content SHOULD remain local by default.
- Shared artifacts SHOULD prefer derived search data over raw page archives.
- Query privacy MUST be considered separately from artifact privacy.
- Implementations SHOULD support local-only search over fetched packages to avoid mandatory remote query disclosure.

## 16. Rollout

- Phase 0: local indexing only
- Phase 1: signed package export/import between trusted peers
- Phase 2: community manifests, validator receipts, trust-aware multi-package search
- Phase 3: optional public/community transport expansion and stronger sybil resistance

## 17. Open Questions

- Exact fingerprint profile selection
- Validator quorum rules
- Trust-score algorithm details
- Public-swarm discovery defaults
- Cross-version package compatibility and translation

Reference assumptions: Tantivy architecture, Iroh blobs, Iroh overview, libp2p overview, p2panda overview

---

# Draft 4 — Synthesized Spec: Verse Distributed Index Protocol (VDIP) v0.2

**Status**
- Draft
- Core protocol plus implementation profiles

**Normative Language**
- `MUST`, `SHOULD`, `MAY`

## 1. Goals

- Peers MUST be able to selectively share derived search knowledge from their own browsing.
- Shared knowledge MUST be independently verifiable, immutable, and content-addressed.
- Communities MUST be able to accept, rank, supersede, and revoke contributions.
- Search MUST work without mandatory global segment merging.
- The protocol MUST separate durable shared truth from local acceleration structures.

## 2. Non-Goals

- Raw HTML/WARC redistribution by default
- Mandatory public DHT-scale discovery
- Mandatory global federation across all communities
- On-chain economics in v0.x

## 3. Protocol Model

The protocol defines three layers:

1. Durable truth:
- `ObservationCard`

2. Community truth:
- `CommunityManifest`
- `IndexCommit`
- `ValidatorReceipt`
- `RevocationRecord`

3. Acceleration:
- `SplitPackage`

Durable truth MUST remain valid even if the local indexing engine changes.
Acceleration artifacts MAY be discarded and rebuilt locally.

## 4. Identity, Serialization, and Addressing

- Identities MUST be `Ed25519` keypairs.
- Signed objects MUST be serialized as canonical CBOR.
- Signatures MUST cover canonical manifest bytes, not opaque compressed payload bytes.
- Content identity MUST be `BLAKE3-256`.
- Implementations MAY expose content IDs as raw 32-byte BLAKE3 digests or as `CIDv1` wrappers using BLAKE3 multihash.
- Archive packaging MUST be deterministic before hashing.

## 5. Compatibility Profiles

The spec MUST define explicit profiles.

### 5.1 Canonicalization Profile

Defines:
- extraction method
- normalization rules
- volatile-field stripping
- tokenization basis for duplicate detection

### 5.2 Fingerprint Profile

Defines:
- exact hash algorithm
- near-duplicate algorithm
- parameters and seeds

### 5.3 Index Profile

Defines:
- index schema
- tokenizer/analyzer chain
- canonicalization profile version
- fingerprint profile version
- local search engine compatibility version

`index_profile_hash` MUST commit to all of the above.

## 6. Core Objects

### 6.1 ObservationCard

Atomic shared observation of one resource at one time.

Required fields:
- canonical URL
- `observed_at`
- title
- snippet
- canonical-content exact hash
- near-duplicate sketch
- UDC tags
- contributor pubkey
- profile/version metadata
- signature

Optional fields:
- language
- site/domain
- MIME hint
- source provenance fields

Raw captures SHOULD remain local by default.

### 6.2 SplitPackage

Optional acceleration artifact containing prebuilt index data.

Required manifest fields:
- `package_id`
- `index_profile_hash`
- contributor pubkey
- signature
- `min_observed_at`
- `max_observed_at`
- observation count
- referenced observation IDs
- UDC summary
- fingerprint summary
- `supersedes[]`

Payload:
- one or more immutable local-search-engine segment files
- optional sidecar metadata

A `SplitPackage` MUST NOT be the sole canonical source of truth.

### 6.3 CommunityManifest

Defines one community.

Required fields:
- `community_id`
- name
- description
- UDC scope
- accepted index profiles
- admission policy
- ranking weights
- admin/moderator identities
- discovery hints
- transport hints

### 6.4 IndexCommit

Append-only signed community record.

Must support:
- add observations
- add packages
- revoke observations
- revoke packages
- supersede packages
- attach validator receipts
- fork lineage

The commit graph MAY be a DAG.

### 6.5 ValidatorReceipt

Signed attestation that an artifact was checked against community policy.

### 6.6 RevocationRecord

Signed record removing an artifact from the active accepted set without deleting history.

## 7. Similarity and Deduplication

- Exact identity MUST use `BLAKE3` over canonicalized content.
- Near-duplicate detection SHOULD use `SimHash`, `MinHash`, or another declared profile.
- Implementations MAY add DOM-structure or perceptual hashes.
- Search results SHOULD collapse exact matches and cluster near-duplicates.
- Similarity logic MUST be profile-declared so peers know what they are comparing.

## 8. Trust and Admission

Communities MUST define an admission policy:
- invite-only
- validator-gated
- contribution-gated
- open with thresholds

Implementations SHOULD maintain a local trust graph.

Trust inputs SHOULD include:
- invite lineage
- endorsements
- flags
- validator outcomes
- utilization
- novelty
- identity age

An EigenTrust/PageRank-style local computation is RECOMMENDED but not mandated by the protocol.

## 9. Search Semantics

- Implementations MUST support fan-out search across accepted local artifacts.
- Global merge is NOT required.
- Local compaction or reindexing MAY be performed as an optimization.
- Trust MAY be enforced at mount time, query time, or both.
- Result ranking SHOULD combine:
- textual relevance
- UDC relevance
- freshness
- novelty
- contributor trust

## 10. Revocation and Moderation

- Revocation MUST be first-class.
- Revoked artifacts MUST remain historically referenceable but MUST be removable from the active search set.
- Communities SHOULD support denylists, supersession, and policy-rejection receipts.

## 11. Privacy

- Raw content SHOULD remain local by default.
- Shared artifacts SHOULD prefer derived metadata and search structures over redistributable page archives.
- Query privacy MUST be treated separately from artifact privacy.
- Implementations SHOULD support local-only search over fetched artifacts.

## 12. Transport and State Profiles

This is where crates belong.

### Profile A: Trusted/Midscale Rust Profile

Recommended now:
- transport: `iroh` + `iroh-blobs`
- announcements: `iroh-gossip`
- replicated state: `iroh-docs`
- local search: `tantivy`
- compression: `zstd`
- serialization: `ciborium` or equivalent canonical CBOR
- trust graph: `petgraph`

### Profile B: Moderated Local-First Governance Profile

Optional:
- replicated community state / ACLs: `p2panda`

### Profile C: Public Swarm Discovery Profile

Deferred:
- `libp2p` for larger/open/adversarial discovery and routing

The core protocol MUST remain valid without any one of these profiles.

## 13. Rollout

1. Local `ObservationCard` generation and signing
2. Local community manifests and fan-out search
3. Signed artifact exchange between trusted peers
4. `SplitPackage` acceleration
5. Trust graph and validator receipts
6. Community commit DAG
7. Optional broader discovery profile

## 14. Open Questions

- commit-head selection
- batch signing vs per-card signing
- exact fingerprint profile
- profile migration rules
- private-community discovery leakage
- remote provider query privacy

## 15. Design Notes

- `ObservationCard` is the network truth, not Tantivy.
- Binary index artifacts are acceleration structures, not the canonical protocol unit.
- The protocol keeps transport, governance, and search-engine choice separable.
- Implementation profiles are intentionally modular so Graphshell can start with a narrow Rust-native stack and expand later if community scale or threat models demand it.

---

## Synthesis Takeaways (To be merged into final spec)

### 1. The "Truth vs. Acceleration" Framing
The system strictly separates the **Durable Truth** (the immutable `ObservationCard` log) from the **Acceleration Structure** (the `SplitPackage` Tantivy index). This decoupling ensures the network protocol remains stable even if the local indexing strategy or Tantivy file formats evolve.

### 2. Resolving the Head Selection Open Question
The open question of *IndexCommit head selection* is resolved by leveraging `iroh-docs`:
* The `CommunityManifest` and commit log live in an `iroh-docs` document.
* Because `iroh-docs` uses a multi-writer CRDT (with write-access gated by admin ed25519 keys), the "head" is simply the current synced state of the document.
* Admins don't need a complex consensus algorithm for the DAG; they just write the new state/commit_id to the `iroh-docs` key-value store, and `iroh` automatically handles the eventual consistency and conflict resolution across the community.

### 3. Modular Implementation Profiles
The protocol defines modular implementation profiles (Trusted/Midscale Rust, Moderated Local-First Governance, Public Swarm Discovery) to allow Graphshell to start with a minimal Rust-native stack and incrementally adopt more complex transport, governance, and discovery mechanisms as needed. This modularity ensures flexibility and scalability without compromising the core protocol.

---

# Verse Distributed Index Protocol — v0.2 Draft Spec

**Status:** Draft. Merges and supersedes three prior drafts. Separates protocol-level normative requirements from reference-implementation guidance.

**Normative language:** MUST, MUST NOT, SHOULD, SHOULD NOT, MAY follow RFC 2119.

**Document structure:** §§1–11 are protocol-level and transport-agnostic. §12 is reference-implementation guidance and may be replaced by alternative implementations that satisfy §§1–11. §§13–14 are phasing and open questions.

---

## 1. Goals

- Peers build local search indices from selectively captured browsing data.
- Peers share derived index artifacts with topic-scoped communities without disclosing raw browsing history.
- Communities accept, reject, rank, and revoke contributed artifacts under explicit policy.
- Search functions without mandatory global index merging.
- Protocol-level data units decouple from any specific local indexing strategy.

### Non-goals (v0)

- DHT-scale open peer discovery.
- Unbounded public communities as a v0 target.
- Economic incentives or on-chain settlement.
- Cross-community federated search.
- Default sharing of raw HTML or WARC captures.

---

## 2. Terminology

| Term | Definition |
|------|------------|
| **ObservationCard** | Atomic protocol unit: one URL observed at one time, with derived metadata. |
| **SplitPackage** | Acceleration artifact: signed bundle of compatible tantivy segments covering a set of observations. |
| **IndexCommit** | Signed append-only record adding, superseding, or revoking artifacts within a community. |
| **ValidatorReceipt** | Signed attestation that an artifact was checked against a community's policy. |
| **CommunityManifest** | Signed definition of a community's scope, profile, access rules, and trust policy. |
| **Contributor** | An identity; an ed25519 keypair. |
| **index_profile_hash** | Compatibility fingerprint for binary segment interop. |

---

## 3. Architecture (Normative)

- The system MUST be local-first: capture, canonicalization, indexing, and trust evaluation MUST occur on the user's device.
- Shared artifacts MUST be immutable. Any change MUST be expressed as a new artifact.
- Community state MUST be expressed as signed append-only records over immutable artifacts.
- Transport and storage concerns MUST be separated from trust and ranking concerns at the protocol level.
- The wire protocol unit MUST be independent of the local indexing implementation. A peer MUST be able to consume ObservationCards from another peer without running the same local indexer.

---

## 4. Identity and Signing

- Identities MUST be ed25519 keypairs.
- All signed objects MUST be serialized as canonical CBOR with deterministic map ordering.
- Signatures MUST cover the canonical-CBOR-encoded manifest, not compressed or packaged bytes.
- Content addresses SHOULD use raw BLAKE3. Implementations MAY wrap BLAKE3 in CIDv1+multihash for multiformat interop; the protocol does not require it.

**Rationale:** Signing manifests (not packaged bytes) allows artifacts to be re-compressed or repackaged without invalidating signatures. Raw BLAKE3 aligns with the reference transport's native hash for zero-translation content addressing.

---

## 5. Content Processing Pipeline

Before an observation is recorded, content MUST be canonicalized by a pipeline that:

1. Extracts main content (readability-style algorithm; specific implementation identified in `index_profile_hash`).
2. Normalizes volatile elements (strip ads, tracking pixels, session tokens, timestamps embedded in DOM; normalize whitespace).
3. Fingerprints:
   - Exact identity: BLAKE3 over canonical extracted text.
   - Near-duplicate: MinHash signature over shingled tokens (implementations MAY additionally use SimHash; MinHash is the primary signal).
4. Derives UDC tags via community-selected classifier or explicit annotation.

Canonicalization MUST be deterministic such that materially equivalent pages produce equivalent exact hashes. Two peers disagreeing on canonicalization WILL diverge at the exact-match layer; near-duplicate similarity SHOULD still converge.

---

## 6. Core Protocol Objects

Rust types below are reference schemas. Wire format is canonical CBOR.

### 6.1 ObservationCard

```rust
struct ObservationCard {
    url: String,
    observed_at: u64,
    title: Option<String>,
    snippet: Option<String>,
    content_hash: [u8; 32],
    minhash: Vec<u64>,
    udc_tags: Vec<String>,
    language: Option<String>,
    contributor: [u8; 32],
    profile_hash: [u8; 32],
}

struct SignedObservation {
    card: ObservationCard,
    signature: [u8; 64],
}
```

Each card MUST retain its individual `observed_at` even when embedded in a package with its own range metadata.

### 6.2 SplitPackage

A SplitPackage MUST contain:
- a signed manifest
- one or more immutable tantivy segment files
- optional sidecar fingerprint data
- optional package-local summaries for discovery

Package archives MUST be deterministic before hashing and signing. Compression SHOULD use zstd.

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
```

Packages with differing `profile_hash` MUST NOT be mounted into the same compatible search view without explicit translation.

### 6.3 IndexCommit

```rust
struct IndexCommit {
    commit_id: [u8; 32],
    parents: Vec<[u8; 32]>,
    community_id: [u8; 32],
    added_observations: Vec<[u8; 32]>,
    added_packages: Vec<[u8; 32]>,
    revoked_observations: Vec<[u8; 32]>,
    revoked_packages: Vec<[u8; 32]>,
    superseded_by: Vec<([u8; 32], [u8; 32])>,
    attached_receipts: Vec<[u8; 32]>,
    timestamp: u64,
    author: [u8; 32],
}
```

The commit graph is a DAG with multi-parent merge support. Forks MUST be representable; community-level canonical head selection is policy, not protocol (see §13).

### 6.4 ValidatorReceipt

```rust
struct ValidatorReceipt {
    receipt_id: [u8; 32],
    artifact_id: [u8; 32],      // observation or package hash
    artifact_kind: ArtifactKind,
    community_id: [u8; 32],
    policy_version: [u8; 32],
    outcome: ValidationOutcome, // Passed | Failed(reason) | Flagged(reason)
    validator: [u8; 32],
    validated_at: u64,
}

enum ValidationOutcome {
    Passed,
    Failed { reason: String },
    Flagged { reason: String },
}
```

Receipts MUST be signed by the validator. Communities MAY require receipts from one or more validators before an artifact is accepted into a commit.

### 6.5 CommunityManifest

```rust
struct CommunityManifest {
    community_id: [u8; 32],
    name: String,
    description: String,
    udc_scope: Vec<String>,
    profile_hash: [u8; 32],
    admin_keys: Vec<[u8; 32]>,
    moderator_keys: Vec<[u8; 32]>,
    validator_keys: Vec<[u8; 32]>,
    admission_policy: AdmissionPolicy,
    validation_policy: ValidationPolicy,
    ranking_weights: RankingWeights,
    relay_urls: Vec<String>,
}
```

Mutations MUST be signed by an admin. Changes to `profile_hash` SHOULD be staged with a transition period (see §13).

### 6.6 index_profile_hash

```
index_profile_hash = BLAKE3(canonical_cbor({
    "schema": <tantivy schema>,
    "tokenizer": <tokenizer config>,
    "analyzer_chain": <analyzer config>,
    "canonicalization_spec": <version + algorithm id>,
    "minhash_params": { "n": <N>, "seed": <SEED>, "shingle_size": <K> },
    "index_format": <tantivy major.minor>,
}))
```

Two peers can exchange and mount binary SplitPackages iff `profile_hash` matches exactly. ObservationCards carry their producer's `profile_hash` so consumers can interpret fingerprint fields correctly.

---

## 7. Trust, Admission, and Ranking

### 7.1 Trust graph

Implementations MUST maintain a local trust graph over contributor identities. Nodes are pubkeys. Edges include at minimum:
- `Invite`: voucher-for-invitee
- `Endorse`: positive evaluation in community context
- `Flag`: negative evaluation in community context

### 7.2 Trust signals

Trust scoring SHOULD consider:
- Package utilization (search-result interaction frequency)
- Content novelty (MinHash distance from existing community content)
- Validator outcomes
- Moderator endorsements and flags
- Identity age and invite-chain depth

### 7.3 Trust computation

Implementations SHOULD compute global trust scores via EigenTrust-style iteration over the local trust graph, anchored at community admin keys as pre-trusted peers. The protocol does not mandate a specific algorithm; it mandates that a contributor's trust score SHOULD be derivable and auditable from community-visible signals.

### 7.4 Mount-time gate

Trust SHOULD be enforced at segment-mount time, not at query time. Packages from contributors below a community-defined threshold MUST NOT be mounted into the active local search view. This keeps the query hot path independent of the trust computation.

### 7.5 Sybil resistance

Communities MUST declare an `AdmissionPolicy`:
- `Open { trust_threshold }`
- `InviteOnly { invite_chain_root }`
- `ContributionGated { validator_quorum }`

Probation: new identities SHOULD be down-weighted for a community-defined period regardless of other signals. Vouchers SHOULD forfeit reputation when their invitees are flagged.

### 7.6 Ranking

Query ranking SHOULD be a composite of community-weighted axes:

```
score = w_bm25    * bm25
      + w_udc     * udc_match
      + w_trust   * contributor_trust
      + w_fresh   * freshness_decay(observed_at)
      + w_novelty * novelty_vs_community
```

Weights come from the community's `RankingWeights`. Additional axes MAY be added per community.

---

## 8. Revocation and Moderation

- Revocation MUST be first-class and MUST support at minimum:
  - `add`, `supersede`, `revoke` of observations and packages
  - `attach_receipt` for validator outcomes
  - `deny_hash` for blanket community-wide hash denials
- A revoked artifact MUST remain immutable and retrievable for audit; it MUST be removable from the active accepted search set.
- Revocation MUST be expressible as a new `IndexCommit`, not by mutating prior commits.
- Communities SHOULD maintain a moderation log as an auditable sequence of commits with attached receipts and rationale.

---

## 9. Privacy

- Raw captured content (HTML, DOM snapshots, WARC) SHOULD remain local by default.
- Shared artifacts SHOULD be derived data (canonical text, fingerprints, tags), not raw captures. Raw snapshot sharing MUST be explicit opt-in per community.
- Contributor pubkeys are public within a community by necessity. Communities MAY support key rotation and pseudonymous identities; the protocol does not mandate an unlinkability mechanism in v0.
- **Query privacy** MUST be considered separately from artifact privacy. Implementations SHOULD support local-only search over fetched packages such that a query is not disclosed to any remote peer. Remote query protocols MAY be layered on later and MUST be opt-in.

---

## 10. Search Semantics

Implementations MUST support search across multiple locally-mounted packages without global index merge. Fan-out across readers is the required mechanism; local compaction is an optimization that MAY be performed but MUST NOT produce artifacts that are shared as if they were original contributions.

Results SHOULD be deduplicated by exact `content_hash` and clustered by MinHash Jaccard similarity within a community-configured threshold.

Provenance metadata (contributor pubkey, commit id, community) MUST be available for each result.

---

## 11. Execution Lifecycle

### Path 1: Generating

1. **Capture.** Browser strips DOM, runs canonicalization pipeline, computes `content_hash` and `minhash`.
2. **Seal.** `ObservationCard` is constructed, canonical-CBOR-encoded, signed. Stored locally.
3. **Publish.** User opts card into community X. The card (or a `SplitPackage` containing it) is announced and its bytes seeded for fetch.
4. **Commit.** A community-authorized writer produces an `IndexCommit` adding the artifact. Validators MAY attach receipts before or after.

### Path 2: Consuming

1. **Sync.** Peer fetches latest community commits and manifest.
2. **Filter.** Local trust computation evaluates contributor scores; low-trust contributors' commits are filtered at mount time.
3. **Fetch.** Missing artifacts referenced in trusted commits are pulled by content address.
4. **Mount.** `SplitPackage` bundles are hot-mounted into the local tantivy reader set. Loose `ObservationCard`s are indexed locally into an index using the community's `profile_hash`.
5. **Search.** Fan-out query across mounted readers; rank by §7.6; render with provenance.

---

## 12. Reference Implementation

This section is non-normative. It describes the current recommended implementation stack.

### 12.1 Networking

| Role | Crate | Purpose |
|------|-------|---------|
| Transport | `iroh` | ed25519-keyed QUIC endpoints, relay fallback, NAT traversal |
| Data plane | `iroh-blobs` | BLAKE3-addressed verified blob and range transfer |
| Announcements | `iroh-gossip` | Community-topic pub/sub for commits and manifests |
| Community state | `iroh-docs` | Replicated CommunityManifest with gated writes |

Communities MAY operate a dedicated iroh relay, advertised via `relay_urls` in the CommunityManifest. Members configure the relay as a home relay for improved connectivity and privileged metadata handling.

### 12.2 Local index

- **Index library:** `tantivy` 0.24+.
- **Storage:** `redb` for local state and trust graph; filesystem for segment storage.
- **Serialization:** `ciborium` for canonical CBOR.
- **Crypto:** `ed25519-dalek` for signing, `blake3` for hashing.
- **Compression:** `zstd`.
- **Trust graph:** `petgraph`.
- **Fingerprinting:** MinHash implemented over a small custom sketch; consider `simhash` as a sidecar for SEO screening.

### 12.3 Deferred alternatives

- **`libp2p`**: reconsider if unbounded public community discovery becomes a requirement.
- **`p2panda`**: adopt `p2panda-auth` or `p2panda-encryption` piecewise if group E2E encryption or offline-first access control becomes required. The full framework is not recommended as a substrate in v0.
- **`automerge`**: reasonable for human-edited community metadata (moderation rationale, policy discussion) but not for binary index artifacts.
- **`chitchat`**: NOT suitable as the primary network substrate for verse.

---

## 13. Phased Rollout

Each phase MUST validate before the next is attempted.

### Phase 0: Local indexing
- Capture, canonicalize, index locally into tantivy.
- **Validation:** a user can browse, have their content canonicalized and indexed, and query the local index without any networking.

### Phase 1: Signed artifact exchange
- `ObservationCard` and `SplitPackage` production, signing, export, and import between two trusted peers by direct dial.
- **Validation:** peer A exports, peer B imports and can query A's contributions locally.

### Phase 2: Communities
- `CommunityManifest`, `IndexCommit`, `ValidatorReceipt`. Trust graph and mount-time gate. Fan-out multi-package search.
- **Validation:** a three-peer community with an admin, a contributor, and a consumer can accept, reject, and revoke contributions with correct downstream effects on the consumer's search results.

### Phase 3: Scale and adversarial conditions
- Broader transport expansion, stronger sybil resistance, batch signing, private community discovery.
- **Validation:** community of 50+ peers with at least one adversarial contributor and one sybil attempt can be operated without quality degradation.

---

## 14. Open Questions

- **IndexCommit head selection.** Community-wide canonical head vs. per-peer head reconciliation. The DAG is representable; the policy layer is undecided.
- **Batch signing scheme.** Merkle root + single signature across N observations is the standard approach; specific tree structure and inclusion-proof format not chosen.
- **Private community discovery.** Private Set Intersection (as in `p2panda-discovery`) is the known primitive. Requires evaluation.
- **Key rotation and identity continuity.** A mechanism is required for rotating keys without losing contribution history.
- **Profile migration.** Dual-profile transition when a community upgrades `index_profile_hash`. Likely a moratorium period with explicit re-indexing commits; not yet specified.
- **Query federation.** When to allow remote query evaluation (search-time disclosure) vs. requiring local fetch-then-query.
- **Validator quorum rules.** `ValidationPolicy` shape for N-of-M validator requirements, conflict resolution between validators.

---

## 15. References

- Tantivy: https://github.com/quickwit-oss/tantivy
- iroh: https://www.iroh.computer/docs/overview
- iroh-blobs: https://docs.rs/iroh-blobs
- iroh-docs: https://docs.rs/iroh-docs
- iroh-gossip: https://docs.rs/iroh-gossip
- p2panda: https://p2panda.org
- EigenTrust: Kamvar, Schlosser, Garcia-Molina (2003)
- MinHash: Broder (1997)
- BLAKE3: https://github.com/BLAKE3-team/BLAKE3
- ed25519-dalek: https://docs.rs/ed25519-dalek
- ciborium: https://docs.rs/ciborium
- petgraph: https://docs.rs/petgraph
- zstd: https://github.com/facebook/zstd
