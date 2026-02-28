# Verse Tier 2: Global Sync Architecture

**Date**: 2026-02-23
**Status**: Research Architecture (Long-Horizon Design)
**Prerequisite**: Tier 1 (iroh direct sync) validated in production
**Context**: Explores extension to public community swarms, dual-transport model, economic primitives, and search infrastructure for Verse as a protocol. This is **not** a Phase 5 deliverable — it defines the long-term architectural space for research and experimentation.

---

## 1. The Dual-Transport Model

Tier 1 uses **iroh** for bilateral, session-oriented sync between trusted peers. Tier 2 adds **libp2p** for community-scale swarms with content addressing, DHT discovery, and open participation.

The two transports are **complementary**, not competing:

| | iroh (Tier 1) | libp2p (Tier 2) |
| --- | --- | --- |
| Use case | Personal devices, bilateral trust | Public communities, ephemeral membership |
| Discovery | Manual pairing, mDNS | DHT, rendezvous, peer exchange |
| Connection | Session-oriented (QUIC stream lifecycle) | Ephemeral, gossip/pubsub |
| Content format | `SyncUnit` (private deltalog) | `VerseBlob` (universal content-addressed) |
| Bandwidth econ | None (zero-cost sync) | Proof of Access (optional) |
| Privacy model | Private by default (Noise auth) | Public by default (content hashes are observable) |

### 1.1 Architectural Layering

```
            Application
            ────────────
            │
            ▼
      [VerseBlob API]  ← Unified content abstraction
      /            \
     /              \
[iroh SyncUnit]   [libp2p VerseBlob]
   │                   │
   │                   │
[Bilateral Sync]   [Community Swarms]
   │                   │
[TrustedPeer]      [PubSub/Bitswap]
```

The application sees a unified "VerseBlob" API. The runtime routes to iroh for bilateral sync (peers in my trust store) and libp2p for community content (public swarms I've joined).

---

## 2. Identity Bridge

Both iroh and libp2p use Ed25519 keypairs for peer identity. Tier 2 derives **both** identities from the same `P2PIdentitySecret`.

```rust
/// Same secret key used for both transports
struct P2PIdentitySecret {
    secret_key: SecretKey,  // Ed25519 (32 bytes)
}

// Derivations:
fn iroh_node_id(secret: &SecretKey) -> iroh::NodeId {
    secret.public() // 32-byte public key used as NodeId
}

fn libp2p_peer_id(secret: &SecretKey) -> libp2p::PeerId {
    let keypair = Keypair::ed25519_from_bytes(secret.to_bytes()).unwrap();
    PeerId::from(keypair.public())
}
```

This allows the same device to operate in both ecosystems without splitting identity:
- In Tier 1 communities: peers recognize me by my iroh `NodeId`
- In Tier 2 swarms: peers recognize me by my libp2p `PeerId`
- Both are derived from the same root secret → single pairing ceremony can authorize both

---

## 3. VerseBlob: The Universal Content Format

Tier 2 content must be **content-addressed** to enable caching, DHT storage, and censorship resistance. The `VerseBlob` is the atomic unit of transfer in public swarms.

### 3.1 Structure

```rust
#[derive(Archive, Serialize, Deserialize)]
struct VerseBlob {
    /// SHA-256 hash of payload (used as DHT key)
    content_hash: Hash256,
    /// MIME type hint
    content_type: String,
    /// Author's peer identity (libp2p PeerId or iroh NodeId)
    author: PeerId,
    /// Ed25519 signature over (content_hash || authored_at)
    signature: Signature,
    /// Wall-clock timestamp (not trusted for ordering, only provenance)
    authored_at: SystemTime,
    /// The actual content (intents, index segments, media, etc.)
    payload: BlobPayload,
}

enum BlobPayload {
    /// A batch of graph intents (ported from SyncUnit)
    IntentDelta(Vec<SyncedIntent>),
    /// A tantivy index segment (node titles + full-text search)
    IndexSegment(Vec<u8>),
    /// Arbitrary binary (attached media, WARC archives, etc.)
    Opaque {
        format: String,
        data: Vec<u8>,
    },
}
```

The `content_hash` is the DHT key. Peers retrieve blobs by hash and validate the signature before trusting the content.

### 3.2 Content Addressing vs Delta Sync

Tier 1's `SyncUnit` carries deltas (version vectors + new intents since last sync). This is efficient for bilateral sessions but incompatible with content addressing — the delta depends on the receiver's state.

Tier 2's `VerseBlob` is **self-contained**: no version vectors, no receiver-specific state. This has cost implications:

- Pro: Blobs can be cached, gossiped, archived — DHT-native
- Con: No automatic deduplication of intents across blobs (community members may receive redundant data)

Mitigation: The Tier 2 runtime maintains a local "seen set" (Bloom filter over `(author, sequence)` tuples) to discard duplicate intents.

---

## 4. Community Model

Tier 2 introduces **communities** — public or semi-public swarms where multiple participants exchange graph content without explicit bilateral trust.

### 4.1 Community Definition

```rust
struct Community {
    id: CommunityId,           // Hash of (name, genesis_block)
    name: String,
    governance: GovernanceModel,
    rebroadcast_level: RebroadcastLevel,
    access_control: AccessControl,
}

enum GovernanceModel {
    /// Anyone can publish, anyone can seed
    Open,
    /// Curators sign approved content
    Curated { curators: Vec<PeerId> },
    /// Admin controls membership and moderation
    Moderated { admin: PeerId, moderators: Vec<PeerId> },
}

enum RebroadcastLevel {
    /// Members relay all content seen (high bandwidth, high availability)
    Full,
    /// Members relay only high-vote content (reputation-weighted)
    Selective,
    /// No relay — direct fetch from author only
    None,
}

enum AccessControl {
    Public,
    InviteOnly { approved_peers: Vec<PeerId> },
    TokenGated { required_balance: u64 },
}
```

### 4.2 Joining a Community

```
User: "Join community: r/graphtheory"
 ↓
App queries DHT for CommunityId (hash of name + genesis block)
 ↓
Connect to known rendezvous points (bootstrap peers)
 ↓
Subscribe to pubsub topic: /verse/community/<CommunityId>
 ↓
Advertise self as provider for (CommunityId, my_peer_id)
 ↓
Sync: request recent blobs from peers (IntentDelta payloads)
```

Communities use libp2p's **GossipSub** for real-time pub/sub and **Bitswap** for bulk content retrieval.

### 4.3 Content Lifecycle

```
Author publishes:
  GraphIntent batch → VerseBlob → sign → publish to pubsub topic
   ↓
Community members receive blob
   ↓
Validate signature + check against local moderation rules
   ↓
Apply intents to local graph + add blob to DHT
  ↓
(If RebroadcastLevel::Full) rebroadcast to GossipSub
```

### 4.4 Federated Adaptation Layer (FLora)

Tier 2 communities may optionally operate a **federated LoRA (FLora)** pipeline alongside graph and index exchange. In this model, a community maintains one or more domain-specific LoRA adapters that members can use as portable knowledge overlays for local AI tooling.

**Core properties:**
- Raw training data remains local to the contributor.
- Contributors run a local mini-adapter pass and publish only weight deltas, candidate LoRA checkpoints, and evaluation metadata.
- A community treasury (for example, Filecoin-backed stake) funds rewards for accepted updates.
- Access to the resulting adapters can be open, contribution-gated, reputation-gated, or private to a closed membership list.

```rust
struct FloraSubmission {
    community_id: CommunityId,
    adapter_id: AdapterId,
    contributor: PeerId,
    parent_checkpoint: Hash256,
    weight_delta_blob: Hash256,
    evaluation: EvaluationReceipt,
    signature: Signature,
}

struct FloraCheckpoint {
    adapter_id: AdapterId,
    checkpoint_hash: Hash256,
    parent: Option<Hash256>,
    merged_from: Vec<Hash256>,
    policy: MergePolicy,
}
```

**Operational flow:**
1. A community defines an adapter domain and stakes treasury funds behind it.
2. Contributors train locally on the data they control and publish a `FloraSubmission`.
3. Moderators, stakers, or trusted reviewers evaluate the candidate in a confirmation buffer.
4. Accepted submissions become a new checkpoint or are merged into a curated checkpoint line.
5. Community members fetch approved checkpoints and mount them in their own AI stack.

This gives a Verse community a portable "skill chip" that can evolve over time without forcing members to disclose their raw private corpora.

---

## 5. Search Architecture (Local vs Remote)

Tier 1 search is **local-only** — tantivy indexes the current device's graph. Tier 2 enables **federated search** across community swarms.

### 5.1 Distributed Index Model

Each community has a **sharded index**. Participants volunteer to index portions of the community's content and publish index segments as VerseBLOBs:

```rust
struct IndexSegment {
    community_id: CommunityId,
    shard: u64,              // Hash(node_id) % num_shards
    entries: Vec<IndexEntry>,
    segment_hash: Hash256,    // Published as VerseBlob
}

struct IndexEntry {
    node_id: UUID,
    title: String,
    tags: Vec<String>,
    content_preview: String, // First 200 chars of node body
}
```

Participants query by:
1. Hash the search term → determine target shards
2. Fetch index segments for those shards from DHT
3. Merge results locally (no central search server)

### 5.2 Index Replication

To avoid single points of failure:
- Each shard has **k replicas** (default k=3)
- Indexers sign their segments → consumers can verify authenticity
- "Stale segment" detection: if segment is >24h old with no updates and community activity is high, request a fresh index from another peer

---

## 6. Proof of Access (Economic Layer)

Tier 2 optionally introduces an economic layer to incentivize hosting, seeding, and indexing.

### 6.1 The Receipt Model

When Peer A retrieves a VerseBlob from Peer B, B can optionally request a **receipt**:

```rust
struct Receipt {
    blob_hash: Hash256,
    bytes_transferred: u64,
    requester: PeerId,
    provider: PeerId,
    timestamp: SystemTime,
    signature: Signature,
}
```

Receipts are **micro-promises**: "I, PeerId A, acknowledge receiving N bytes of blob X from PeerId B." These receipts aggregate into reputation scores or token claims (see §6.3).

### 6.2 Receipt Aggregation

Participants submit receipts to a **ledger** (on-chain or off-chain, TBD):

```
Ledger tracks:
  Provider B:
    Total bytes served: 120 GB
    Unique requesters: 340
    Last 30 days: 12 GB
```

Providers with high aggregate scores earn **community reputation** (used for search ranking, curation eligibility, moderation authority, etc.).

### 6.3 Token Settlement (Speculative)

Phase 1: Receipts are **reputation-only** (no money, no tokens). High-reputation peers are preferred for search queries and get priority in request queues.

Phase 2 (long-term): Communities can opt into **token settlement** — receipts convert to microtransactions. Requires:
- Integration with a payment layer (Lightning, rollup, or dedicated Verse token)
- Price discovery mechanism (bandwidth marketplaces)
- Anti-gaming safeguards (Sybil resistance, collateral bonds)

This is deferred until Tier 1 proves utility at scale.

### 6.4 The "No-Receipt" Flag

Bilateral Tier 1 sync always sets `skip_receipt = true`. The receipt model only applies to community (Tier 2) transfers. Personal device sync remains zero-cost.

---

## 7. Research Agenda

Tier 2 is a multi-year research space. Key open problems:

### 7.1 DHT Scalability

libp2p's Kademlia DHT works well for small-to-medium swarms (<10,000 nodes). Beyond that, query latency degrades. Alternatives:
- **Hybrid routing**: Hierarchical DHTs with per-community routing tables
- **Rendezvous servers**: Centralized bootstrap → decentralized mesh after initial join
- **S/Kademlia extensions**: Authenticated routing, eclipse attack resistance

### 7.2 Moderation at Scale

Open communities need tools to handle spam, abuse, and illegal content:
- **Curator signatures**: Only rebroadcast blobs signed by trusted curators
- **Reputation filters**: Hide content from low-reputation authors
- **Decentralized reports**: Peer-to-peer "flag" propagation with threshold-based auto-hide

No single solution. Need experimentation.

### 7.3 Index Freshness

Sharded indexes work when community activity is stable. Burst traffic (viral node, mass migration) can overwhelm volunteer indexers. Potential solutions:
- **Index incentives**: Proof of Access receipts for indexing work
- **Dynamic sharding**: Repartition shards when load spikes
- **Lazy indexing**: Only index high-vote content

### 7.4 Economic Model Validation

Proof of Access assumes receipts are forgery-resistant and auditable. Attack vectors:
- **Self-serving**: Peer A runs fake Peer B, transfers to itself, claims receipts
- **Collusion**: Peers A and B exchange receipts without real transfers
- **Sybil**: Attacker creates thousands of identities to inflate reputation

Mitigations: Require collateral bonds (stake tokens to participate), reputation decay, cross-validation from third parties. But these introduce complexity and centralization risks. Need real-world testing before committing to a design.

---

## 8. Nostr Signaling (Protocol Bridge)

Nostr is a simple pub/sub protocol with existing traction in decentralized social apps. Tier 2 can use Nostr as a **signaling layer** without adopting its data model:

```
Use Nostr for:
  - Community announcements (new index segments available, governance votes)
  - Peer discovery (find libp2p multiaddrs for community members)
  - Pairing hints (bootstrap Tier 1 pairing via shared Nostr DM)

Do NOT use Nostr for:
  - Primary content storage (Nostr events are ephemeral and relay-dependent)
  - Graph intent transfer (too high-bandwidth for typical relays)
```

Integration:
- Verse publishes Nostr events with kind `30078` (custom kind for Verse announcements)
- Event content: JSON-encoded `{ verse_version, community_id, libp2p_multiaddr, index_segment_hashes }`
- Participants follow these events to bootstrap libp2p connections without hardcoded relay addresses

Nostr is a convenience layer, not a dependency. Verse works without it (via DHT-only discovery).

---

## 9. Content Pipeline (Ingest → Enrich → Curate → Publish)

Tier 2 communities benefit from structured content ingestion workflows:

### 9.1 The Pipeline

```
┌─ Ingest ──┐    ┌─ Enrich ──┐    ┌─ Curate ──┐    ┌─ Publish ──┐
│ Raw URLs  │ -> │ Fetch +   │ -> │ Vote +    │ -> │ Publish as │
│ or files  │    │ extract   │    │ tag +     │    │ VerseBlob  │
│           │    │ metadata  │    │ review    │    │ to swarm   │
└───────────┘    └───────────┘    └───────────┘    └────────────┘
```

Each stage can be handled by different community roles:
- **Ingesters**: Submit raw links (anyone)
- **Enrichers**: Run scrapers/extractors to generate previews (volunteer bots or curators)
- **Curators**: Vote on quality, add tags, write summaries (trusted members)
- **Publishers**: Batch approved content into VerseBLOBs and publish to DHT (automated or manual)

### 9.2 WARC Archives

For web content (articles, papers, videos), use **WARC** (Web ARChive format) to preserve full context:

```rust
BlobPayload::Opaque {
    format: "application/warc".to_string(),
    data: warc_bytes,
}
```

WARC captures:
- Original HTTP headers
- Full HTML + embedded resources (CSS, images)
- Provenance metadata (timestamp, URL, IP)

This allows offline replay of content even if the source disappears. Community members can run local WARC players (pywb, replayweb.page) to view archived nodes.

### 9.3 CRDT Integration (Speculative)

For collaborative editing of community nodes (e.g., Wikipedia-style articles), integrate a CRDT library (automerge, yjs):

```rust
BlobPayload::CRDTDelta {
    doc_id: UUID,
    crdt_ops: Vec<CRDTOp>,
}
```

Community members apply CRDT operations to a shared document. Conflict-free merging happens locally. The latest state is periodically published as a VerseBlob.

This requires significant complexity (CRDT state management, garbage collection, schema versioning). Deferred until demand is proven.

---

## 10. Protocol Ecosystem Mapping

Tier 2 positions Verse within the broader decentralized protocol space:

| Protocol | Role in Verse Tier 2 |
| --- | --- |
| **iroh** | Bilateral sync (Tier 1 foundation) |
| **libp2p** | Community swarms, DHT, GossipSub |
| **IPFS** | Content addressing inspiration; VerseBlob is IPFS-compatible but uses custom format |
| **Nostr** | Optional signaling layer for peer discovery and announcements |
| **ActivityPub** | Future: Publish community summaries to Mastodon/Bluesky (see §11.3) |
| **Dat/Hypercore** | Alternative sync protocol; evaluate if iroh proves insufficient |
| **Filecoin** | Optional: Pay for long-term archival of high-value community indexes |
| **Lightning/Rollups** | Long-term: Microtransaction layer for Proof of Access settlement |

Verse is **protocol-agnostic at the transport layer** — the VerseBlob abstraction allows swapping substrates without changing application logic.

---

## 11. Crawler Economy (Speculative)

Communities may want external web content indexed and ingested automatically. Tier 2 introduces a **crawler economy** where participants run bots to fetch, extract, and enrich URLs submitted by community members.

### 11.1 The Crawler Bounty Model

```
1. User submits URL to community (via special GraphIntent: AddExternalLink)
2. Community members vote on priority (upvote = "I want this crawled")
3. Crawler bots monitor the community's "pending crawl" queue (libp2p pubsub topic)
4. Bot claims URL, fetches content, extracts metadata, uploads WARC VerseBlob
5. Bot publishes receipt: "I crawled <URL>, here is blob hash <X>, I spent Y bandwidth"
6. Community curators validate: content matches URL, no spam → approve receipt
7. Crawler earns reputation (and optionally tokens, if community has economic layer)
```

### 11.2 Anti-Spam Safeguards

- **Require deposit**: Crawlers post collateral to join the economy (forfeit if they publish spam)
- **Rate limits**: Max N crawls per crawler per day (prevents flooding)
- **Curator veto**: Curators can reject low-quality crawls → crawler loses reputation

### 11.3 Integration with External Platforms

Crawlers can also:
- Monitor Mastodon/Bluesky for links shared by trusted accounts → auto-ingest into community
- Subscribe to RSS feeds and publish new entries as VerseBLOBs
- Run headless browsers to capture JavaScript-heavy sites as WARCs

This positions Graphshell as a **protocol-level aggregator** — content flows in from the open web and decentralized social networks, gets curated by the community, and becomes searchable/linkable in the local graph.

---

## 12. Open Questions (Tier 2)

1. **VerseBlob vs IPFS**: Should VerseBlob use IPFS's CID format (compatible with existing IPFS tooling) or a custom hash? Trade-off: IPFS compatibility vs tighter integration with Verse's signature + versioning model.

2. **libp2p vs iroh for Communities**: iroh is optimized for bilateral sync. libp2p is the standard for decentralized swarms. Should Tier 2 use **both** (dual-transport) or consolidate on one? Recommendation: Evaluate iroh's gossip protocol (if it matures) before committing to dual-transport complexity.

3. **Proof of Access Economics**: Should the economic layer use a **dedicated Verse token**, integrate with an existing token (e.g., Filecoin, Arweave), or remain reputation-only forever? Each option has vastly different development and governance implications.

4. **Community Bootstrapping**: How do the first 100 users find each other when there is no DHT yet? Options: hardcoded rendezvous servers (centralization risk), Nostr signaling (adds dependency), invite-only launch (slow growth). Need experimentation.

---

## 13. Relationship to Tier 1

Tier 2 is **additive, not replacement**:

- Users who only sync personal devices (Tier 1) are unaffected by Tier 2 complexity
- Tier 2 features (community swarms, search, receipts) are **opt-in** via joining a community
- The same device can participate in both: bilateral sync via iroh for personal workspaces, libp2p for public communities
- If Tier 2 proves infeasible or undesirable, Tier 1 remains a fully functional product

This design philosophy de-risks the research agenda. Tier 1 ships in Phase 5 (Q2 2026); Tier 2 is a multi-year exploration.

---

## 14. Next Steps (Research Roadmap)

### Q3 2026: Validation Phase
- Prototype VerseBlob on libp2p testnet (10-20 nodes)
- Measure DHT lookup latency and gossip bandwidth at scale
- Implement IndexSegment publishing + federated search query (no economic layer yet)

### Q4 2026: Community Pilot
- Launch one "reference community" (`/graphshell-research`) as testbed
- Recruit 50-100 participants for real-world usage
- Gather data: bandwidth consumption, moderation burden, index freshness

### 2027 H1: Economic Layer Design
- If community pilot succeeds, design Proof of Access ledger (on-chain vs off-chain)
- Prototype receipt aggregation + token settlement
- Threat model: Sybil resistance, collusion attacks, griefing

### 2027 H2: Protocol Stabilization
- Publish Verse protocol spec (VerseBlob format, DHT schema, GossipSub topics)
- Reference implementations in Rust (primary) + TypeScript (web client)
- Interop testing with other libp2p-based systems

---

## 15. Alignment with Graphshell's Mission

Tier 2 is **not required** for Graphshell to be a valuable tool. The personal knowledge graph + search + AI integration (Phase 1-5) are sufficient for a standalone product.

Tier 2 is the answer to: **"What if Graphshell became infrastructure for decentralized knowledge commons?"**

It's a long-horizon bet that:
- Users want to participate in public communities (not just private graphs)
- Decentralized protocols can compete with centralized platforms (Reddit, Notion, Roam) on convenience
- Economic incentives can solve the "who pays for hosting?" problem without sacrificing openness

If these assumptions hold, Tier 2 transforms Graphshell from **personal tool** to **protocol layer** for collaborative knowledge. If they don't, Tier 1 remains a competitive, self-contained product.

---

**End of Tier 2 Architecture Document**
