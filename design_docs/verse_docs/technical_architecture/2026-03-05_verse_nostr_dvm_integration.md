<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Verse + Nostr + NIP-90 DVM Integration

**Date**: 2026-03-05
**Status**: Draft / Tier 2 research direction
**Scope**: How Nostr communities, NIP-90 Data Vending Machines, FLora checkpoints, distributed indices, and Proof of Access economics compose into a coherent community knowledge layer.

**Related docs**:

- [`2026-02-23_verse_tier2_architecture.md`](2026-02-23_verse_tier2_architecture.md) — Tier 2 dual-transport and swarm architecture authority
- [`../implementation_strategy/proof_of_access_ledger_spec.md`](../implementation_strategy/proof_of_access_ledger_spec.md) — receipt and reputation model
- [`../implementation_strategy/flora_submission_checkpoint_spec.md`](../implementation_strategy/flora_submission_checkpoint_spec.md) — FLora adapter pipeline
- [`../implementation_strategy/community_governance_spec.md`](../implementation_strategy/community_governance_spec.md) — governance roles and quorum
- [`../../graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md`](../../graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md) — iroh/libp2p/Nostr layer assignments

---

## 1. The Problem This Solves

Verse Tier 2 specifies the economic and data infrastructure for community knowledge pools: distributed index shards, FLora adapter checkpoints, Proof of Access receipts, and governance logs. What it does not specify is:

- How users **discover** Verse communities and their published outputs.
- How community-approved LoRA adapters are **invoked** on behalf of members at query time.
- How **browsing context** (current graph, active nodes, navigation history) feeds into real-time semantic suggestions.
- How **compute compensation** (running inference for a community member) integrates with the Proof of Access economy.
- How the **social layer** (follows, public feed filtering, community announcements) connects to the economic layer.

This document specifies how Nostr and NIP-90 DVMs fill these gaps without duplicating what libp2p + Verse already provides.

---

## 2. Layer Assignment

| Concern | Protocol | Notes |
| --- | --- | --- |
| Community discovery | Nostr NIP-72 | Social surface; anyone can find a Verse by `npub` |
| Private space membership | Nostr NIP-29 (relay-enforced) | Acceptable trust trade-off for private communities |
| Peer discovery within community | libp2p Kademlia DHT | Trustless; does not require Nostr |
| State replication | libp2p gossipsub + Bitswap | Authority: `verse_tier2_architecture.md` |
| LoRA checkpoint storage | Verse FLora + IPFS CIDv1 / Bitswap | Large bytes never inline in pubsub |
| Index shard storage | Verse DHT (tantivy segments as VerseBLOBs) | Authority: `2026-02-23_storage_economy_and_indices.md` |
| Compute job dispatch | Nostr NIP-90 DVMs | Job request/result events; Lightning payment |
| Compute compensation | NIP-57 Lightning Zaps + Proof of Access receipts | Zap = payment; receipt = reputation evidence |
| Feed curation / algo filtering | NIP-90 DVM (feed curation job type) | Community LoRA filters the Nostr community feed |
| Browsing suggestions | NIP-90 DVM (context-aware traversal job) | Graph context → DVM → ranked ghost nodes |
| Governance announcements | Nostr NIP-72 approval events | FLora checkpoint approved → kind 4550 event |
| Social graph / follows | Nostr NIP-02 kind 3 | Follow a Verse community `npub` to track outputs |

Nostr is **never** used for bulk data transfer (index shards, adapter weights, raw blobs). It is a signalling and discovery bus only. All bulk data flows over libp2p Bitswap or iroh-blobs.

---

## 3. NIP-72 as the Verse Community Social Surface

A Verse community has a **canonical `npub`** — the community's Nostr identity, controlled by the initial operator or a multisig equivalent. The community publishes Nostr events under this `npub`:

- **kind 34550** (NIP-72 community definition): community name, description, moderator set (the Verse `CommunityManifest` operator/moderator roles map to NIP-72 moderators), and a reference to the Verse DHT bootstrap addresses.
- **kind 4550** (NIP-72 approval event): each time a FLora checkpoint is approved or a new index epoch is published, the moderator signs a kind 4550 event linking to the VerseBLOB CID. This is the public announcement that a new community output is available.
- **kind 30023** (long-form, optional): community notes, changelogs, research summaries authored by community members.

**Verse-specific event tags** (extending NIP-72):

```json
{
  "kind": 34550,
  "tags": [
    ["d", "<community-id>"],
    ["verse_dht_bootstrap", "<libp2p_multiaddr_1>", "<libp2p_multiaddr_2>"],
    ["verse_community_id", "<hex-community-id>"],
    ["verse_manifest_cid", "<CIDv1 of CommunityManifest blob>"]
  ]
}
```

Users discover the community via Nostr (relay search, follows, shared links). They join the libp2p swarm using the bootstrap addresses in the community definition. Nostr is the **front door**; the swarm is the **interior**.

---

## 4. NIP-90 Data Vending Machines as the Compute Layer

NIP-90 defines a job marketplace: clients publish job requests (kind 5000–5999), DVMs publish results (kind 6000–6999), status updates arrive as kind 7000. Payment is via Lightning Zap attached to the result.

### 4.1 Job types for Verse communities

**Feed curation** (kind 5300 — existing NIP-90 type):

```json
{
  "kind": 5300,
  "tags": [
    ["i", "<nostr_relay_url>", "url"],
    ["param", "max_results", "20"],
    ["param", "verse_community_id", "<community-id>"],
    ["param", "flora_checkpoint_cid", "<CIDv1>"]
  ]
}
```

The DVM fetches the community's public Nostr feed, runs the community LoRA checkpoint for relevance ranking, returns a ranked list of event IDs. Members see a curated feed rather than a raw firehose.

**Semantic traversal suggestion** (new kind, proposed 5400):

```json
{
  "kind": 5400,
  "tags": [
    ["i", "<current_node_url>", "url"],
    ["i", "<graph_context_hash>", "verse_graph_snapshot"],
    ["param", "verse_community_id", "<community-id>"],
    ["param", "flora_checkpoint_cid", "<CIDv1>"],
    ["param", "index_epoch", "<epoch-id>"],
    ["param", "max_suggestions", "8"]
  ]
}
```

The client provides its current node URL and a lightweight graph context snapshot (a hash of the active node set and recent navigation history). The DVM:

1. Fetches the relevant index shards from the Verse DHT for the current node's domain/topic.
2. Loads the community LoRA checkpoint.
3. Runs semantic similarity against the index to produce ranked traversal suggestions.
4. Returns a list of `(url, title, relevance_score, rationale)` tuples.

Results surface in the graph as **suggested edge ghost nodes** — visually distinct, dismissable, attributed to the Verse community that produced them.

**Graph node summarisation** (new kind, proposed 5401):

```json
{
  "kind": 5401,
  "tags": [
    ["i", "<node_content_hash>", "verse_blob"],
    ["param", "verse_community_id", "<community-id>"],
    ["param", "flora_checkpoint_cid", "<CIDv1>"],
    ["param", "output_format", "annotation"]
  ]
}
```

Produces a short annotation for a graph node using the community's domain-specialised model. Stored as a node annotation on the local graph; not shared unless the user explicitly publishes it.

### 4.2 DVM operator economics

A DVM operator within a Verse community:

- Holds the community's FLora checkpoint locally (fetched via Bitswap on checkpoint approval).
- Monitors the community's NIP-90 job feed (subscribed via the community relay or a shared Nostr relay pool).
- Processes jobs and publishes results.
- Receives **Lightning Zap payment** from the requesting client (NIP-57 zap receipt attached to the result event).
- Generates a **Proof of Access receipt** (type `ComputeCompleted`) which accumulates community reputation.

This means compute compensation has two tracks: immediate Lightning payment (real money, instant) and deferred reputation accumulation (community standing, influences governance weight). These are complementary — not either/or.

### 4.3 Who runs DVMs?

- **Community members**: altruistic or reputation-motivated. A researcher in a plant biology Verse might run a DVM because high reputation gives them more governance weight.
- **Paid operators**: anyone who wants to earn sats. No community membership required to operate a DVM — the community LoRA is public (within the community's access policy); the DVM just needs to fetch it.
- **Self-hosted**: a user can run their own DVM locally and submit jobs to themselves — zero-latency, zero-payment, full privacy. The local inference stack from `self_hosted_model_spec.md` is the runtime.

---

## 5. FLora Checkpoint → Nostr Announcement Pipeline

When a FLora checkpoint is approved via community quorum:

1. The approving moderator/curator publishes a **kind 4550** (NIP-72 approval event) referencing the checkpoint CIDv1.
2. The checkpoint CID is included in the community's DHT as a pinned VerseBLOB.
3. DVM operators subscribed to the community feed see the approval event and fetch the new checkpoint via Bitswap.
4. Subsequent NIP-90 job requests can reference the new checkpoint CID.

This pipeline means checkpoint distribution is **pull-based** (operators fetch when they see the announcement) rather than push-based (no broadcast of large adapter bytes). The Nostr event is tiny; the bytes move over Bitswap only to operators who want them.

---

## 6. Distributed Index + Semantic Suggestions

The Verse distributed tantivy index (authority: `2026-02-23_storage_economy_and_indices.md`) provides the retrieval layer. The FLora LoRA provides the semantic ranking layer. Together:

```
User navigates to node N
  → client submits kind 5400 job (current node + graph context)
  → DVM fetches index shards for N's domain from DHT
  → DVM runs LoRA checkpoint for semantic similarity ranking
  → DVM returns ranked URL list + rationale
  → client renders results as ghost nodes with "suggested by <community>" attribution
  → user accepts (creates edge) or dismisses
```

**Privacy properties**:
- The client sends the current node URL and a graph context hash — not a full browsing history dump.
- The DVM sees the query but not the user's identity (NIP-90 jobs can be published with a random one-time keypair; the Zap payment can be routed anonymously via Lightning).
- Raw browsing history never leaves the device; only the current traversal context is shared per-query.

**Multiple communities**:
A user can be a member of multiple Verse communities simultaneously. Traversal suggestions from each community are rendered with distinct attribution, allowing the user to see whose knowledge graph informed each suggestion. Communities with overlapping topic coverage will produce different suggestions based on their respective training data and LoRA checkpoints.

---

## 7. Algorithmic Feed Filtering

A Verse community's public Nostr presence (announcements, member posts, approved content links) is a feed. Without filtering, a large community's feed is noise. NIP-90 feed curation DVMs (kind 5300) solve this:

- Member subscribes to community `npub` feed on Nostr.
- Client submits a kind 5300 curation job: "filter this feed using community LoRA checkpoint X."
- DVM returns ranked event IDs.
- Client renders the curated view rather than the raw chronological feed.

This is **algorithmic curation without a centralised algorithm**: the ranking model is community-owned (approved via FLora governance), operator-run (competitive, pays via Lightning), and member-controllable (members choose which community's LoRA they trust for curation, or run it locally).

---

## 8. Tokenomics Composition

The existing Proof of Access model (receipt-based reputation, optional Lightning settlement) composes with NIP-90 as follows:

| Economic event | Receipt type | Nostr event | Lightning |
| --- | --- | --- | --- |
| Serve an index shard | `RetrievalServed` | — | Optional tip |
| Store a VerseBLOB | `StorageServed` | — | Optional |
| Review a FLora submission | `ReviewCompleted` | — | — |
| Approve a checkpoint | `ModerationCompleted` | kind 4550 | — |
| Run a DVM compute job | `ComputeCompleted` | kind 6000–6999 + kind 9735 zap receipt | Required (NIP-57 zap) |
| Curate a feed | `ComputeCompleted` | kind 6300 + kind 9735 | Required |

Key design decision: **DVM compute is always Lightning-paid** (real money, immediate), while storage/retrieval/review are reputation-tracked (receipts, deferred settlement optional in v2). This creates a natural division: commodity compute is priced by the market; community contribution (review, moderation, indexing) is incentivised by governance weight.

Anti-plutocracy rule from `community_governance_spec.md` applies: Lightning payment history does not translate to governance weight. A well-funded actor who pays for many DVM jobs does not gain moderation authority. Governance weight comes from review/moderation receipts only.

---

## 9. Open Problems

1. **LoRA access control**: If a community's FLora checkpoint is access-restricted (not public), DVM operators need to prove community membership before fetching. The NIP-29 relay-enforced group model could gate checkpoint access, but adds relay trust dependency. Alternative: encrypt the checkpoint with a community-derived key (threshold encryption from moderator key set). No finalized design yet.

2. **DVM sybil resistance**: A malicious actor could run a DVM, collect Lightning payments, and return garbage results. Mitigation: client rates DVM results (kind 7001 feedback events); community reputation system weights DVM operators by result quality over time. Finalized feedback loop design is deferred.

3. **Index freshness vs. DVM latency**: Stale index shards produce stale suggestions. High-volume communities need fresh indices; volunteer indexers may lag. Dynamic shard incentives (higher receipt weight for fresher shards) are noted in `2026-02-23_storage_economy_and_indices.md` as an open problem. DVM operators can mitigate by maintaining local index replicas rather than fetching from DHT per-query.

4. **Cross-community suggestion conflicts**: When multiple communities suggest traversals for the same node with conflicting rankings, the client needs a tie-breaking policy. Proposed default: rank by community relevance score (how closely the community's topic domain matches the current node), then by DVM operator reputation. No spec yet.

5. **Privacy of graph context in DVM jobs**: The kind 5400 job includes a graph context hash. Even a hash leaks that the user visited certain nodes (hash reversal if the URL space is small). Mitigation: add noise (include k random nodes in the context hash), or use private information retrieval techniques for index queries. Deferred research problem.

---

## 10. Rollout Sequence

This is Tier 2 territory — none of this ships before Tier 1 (Device Sync, CP4) is stable. Suggested sequence when Tier 2 begins:

1. NIP-72 community definition + kind 4550 checkpoint announcements (Nostr signalling only, no DVM yet).
2. Basic NIP-90 DVM for feed curation (kind 5300) using community LoRA checkpoint — prove the pipeline end-to-end.
3. Traversal suggestion DVM (kind 5400) integrated with graph ghost node UI.
4. Graph node summarisation DVM (kind 5401) integrated with node annotation panel.
5. Proof of Access `ComputeCompleted` receipt type — connect DVM runs to reputation economy.
6. Local DVM mode — user runs inference locally, submits jobs to themselves. Zero-payment, full privacy.
7. Close open problems (§9) as research matures.
