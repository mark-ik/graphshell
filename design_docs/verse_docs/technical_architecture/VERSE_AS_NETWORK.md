# VERSE AS NETWORK

**Purpose**: Specification for Verse as Graphshell's optional community-scale network layer, and for the boundary between Graphshell as host, Verso as bilateral peer agent, and Verse as shared-community infrastructure.

**Document Type**: Network and protocol specification (not implementation status)
**Status**: Verso bilateral sync/co-op is the active Phase 5 implementation lane; Verse community swarms, federated search, Proof of Access, and FLora/engram exchange remain long-horizon research (Q3 2026+)
**See**: [VERSO_AS_PEER.md](../../verso_docs/technical_architecture/VERSO_AS_PEER.md) for how Graphshell's Verso mod participates; [2026-02-23_verse_tier1_sync_plan.md](../../verso_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) for the active bilateral sync implementation plan; [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md) for the long-horizon swarm architecture; [COMMS_AS_APPLETS.md](../../graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md) for the hosted communication-surface boundary

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../graphshell_docs/research/2026-03-04_standards_alignment_report.md) §§3.10–3.14 for full rationale):

- **libp2p specs (via iroh)** — Tier 1: QUIC transport, Noise XX handshake, PeerID (Ed25519), Identify protocol. Tier 2: GossipSub 1.1, Kademlia DHT.
- **W3C DID Core 1.0** — Verse peer identity as `did:key` DID derived from Ed25519 public key. iroh `NodeId` is the internal form; `did:key` is the Verse-layer wire identity URI.
- **W3C VC Data Model 2.0** — Verse knowledge object envelopes (reports, graph slices, engrams). Issuer is the authoring peer's `did:key`; proof is an Ed25519 signature. Replaces ActivityPub as Verse's knowledge vocabulary.
- **IPFS CIDv1** — Content addressing for VerseBlobs (base32 canonical text, dag-cbor or raw codec, BLAKE3 hash).
- **CRDT semantics** — Concurrent sync model: OR-Set for node/edge sets; Last-Write-Wins per metadata field (timestamp from UUID v7 operation tokens). RFC 6902 JSON Patch is not adopted (no merge semantics; see standards report §4.3).

**Referenced as prior art** (no conformance obligation):
- **ActivityPub / AT Protocol** — federated identity patterns; neither adopted. ActivityPub interop via a bridge server remains a long-horizon possibility but is not the Verse backbone.
- **Filecoin FIP mechanics** — FLora staking and on-chain storage deal mechanics (Tier 2, Q3 2026+). Separate from iroh/libp2p transport; requires a Filecoin light client decision at design time.

---

## Architectural Position

Graphshell is the **host and renderer**, not the network itself. It must remain fully useful as a local-first browser, file/media organizer, and knowledge graph even when no networking capability is loaded.

The ownership split is:

- **Graphshell** hosts surfaces and renders network-backed applets, but does not require networking to function.
- **Verso** owns bilateral and session-scoped peer behavior: web capability, Device Sync, and co-op over iroh.
- **Verse** owns the optional community-scale layer: durable shared knowledge spaces, participant-governed replication, and larger-n discovery/search flows.
- [**Comms**](../../graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md) is best understood as an optional hosted applet/capability family that may run inside Graphshell surfaces; it is not a core Graphshell semantic domain.

This means Graphshell can host Verse- or Comms-backed surfaces without collapsing their semantics into the shell itself. Hosting a surface is not the same thing as owning the underlying network domain.

---

## What the Verse Is

The Verse is a **decentralized, peer-to-peer knowledge network** for community-scale participation. It transforms Graphshell from a purely local browser/organizer into a host for shared, participant-governed knowledge spaces when the user opts into that layer.

Its core building blocks are **community primitives**: community manifests, content-addressed VerseBlobs, governance and announcement records, distributed index shards, FLora checkpoints, and receipt-ledger artifacts such as Proof of Access.

The Verse is not a server. It is not a platform. It is a set of protocols and data formats that Graphshell peers speak to each other. Every user running Graphshell with Verso enabled is a Verse peer.

Each local Verse instance is best understood as a **private-by-default, self-hosted portal**: a sovereign node that can keep data and model customizations private, or selectively participate in consensual storage, indexing, and adaptation economies.

Historically the docs described Verse as having two tiers. The current architectural split is narrower and cleaner:

- **Verso** owns bilateral sync and co-op over iroh. That is the active implementation lane.
- **Verse** owns community swarms and community-scale knowledge exchange over libp2p/Nostr/Matrix-aligned fabrics where appropriate.

When older documents refer to "Verse Tier 1," read that as the bilateral capability now owned by **Verso** rather than by Verse as a standalone network domain.

In the long-horizon Tier 2 framing, that local Verse node can also act as:
- a wallet-adjacent treasury manager for stake-backed storage and bounty budgets
- a host for persistent graphs, applets, feeds, forums, and access points to shared web processes
- a private engram library and FLora contributor/consumer

This document describes both tiers as a conceptual whole. For implementation, read the tier-specific documents.

---

## Why the Verse Exists

The current web has a structural problem: search engines decide what you see, and the decision criteria are engagement (ads) rather than relevance or trust. The Verse inverts this:

- **Your index, your graph**: you decide what to save and how to organize it.
- **Your trust network curates discovery**: when you search, you search your own graph first, then your trusted peers' graphs, then the broader community — in that order.
- **Resilience**: if a website goes offline, the Verse (via content addressing and peer replication) may still have a snapshot of it.
- **Portable identity**: your graph relationships and reputation travel with you. Blocking or platform bans cannot sever your connections.

The Verse does not replace the web. It adds a memory and trust layer over it.

---

## Bilateral Context Boundary

Bilateral peer sync and live co-op are not the Verse proper. They are the **Verso-side foundation** that Verse can build on, promote from, or interoperate with.

Practical rule:

- If the interaction is two-party, named-peer, relationship-scoped, and iroh-backed, it belongs to **Verso**.
- If the interaction is community-scoped, participant-governed, or intended to outlive any single bilateral session, it belongs to **Verse**.

That split preserves local-first behavior and keeps community networking optional rather than making it a prerequisite of the shell.

---

## Verso Bilateral Foundation

### The Model

Two Graphshell instances that have paired with each other sync their graphs bilaterally through **Verso**. Think git between two machines: operations batch at the WAL level, sync happens when peers connect, conflicts are resolved deterministically (last-write-wins on metadata; additive merge for nodes and edges).

**Properties:**

- **Local-first**: works perfectly offline; sync is opportunistic.
- **No mandatory server**: peers connect directly via iroh Magic Sockets (QUIC with NAT traversal). No relay required for most network configurations.
- **End-to-end encrypted**: iroh uses the Noise protocol for transport authentication; at-rest data uses AES-256-GCM.
- **Selective**: each workspace can be shared with specific peers at `ReadOnly` or `ReadWrite` access level.

This section remains here because the bilateral model is the substrate Verse grows out of, but its implementation authority lives in Verso and its implementation plan remains [2026-02-23_verse_tier1_sync_plan.md](../implementation_strategy/2026-02-23_verse_tier1_sync_plan.md).

### What Gets Synced

Tier 1 syncs the **semantic graph**:

- Nodes (identity, URL, title, tags, mime_hint, viewer preferences)
- Edges (type, traversal log)
- Tags and metadata mutations

It does **not** sync:

- Layout state (tile tree, node positions) — device-local spatial preferences.
- Renderer runtime state (active viewers, scroll positions) — ephemeral.
- Workspace tab semantics (which pane tabs are grouped) — device-local.

This boundary keeps sync meaningful: peers share *what exists and what it means*, not *how each device displays it*.

### The Wire Format: SyncUnit

A `SyncUnit` is the atomic unit of sync exchange:

- A rkyv-serialized, zstd-compressed batch of fjall WAL log entries.
- Scoped to a single workspace.
- Tagged with the sender's version vector (one clock per peer) so the receiver knows exactly which operations are new.
- Signed with the sender's Ed25519 key so the receiver can verify authenticity.

Delta computation: when peer A connects to peer B, they exchange version vectors. A sends only the WAL entries B hasn't seen yet (the delta). B applies them as `GraphIntent` events through the normal reducer pipeline — Verso never writes directly to `GraphWorkspace`.

### Identity and Trust

Each Verso instance generates one Ed25519 keypair on first use, stored in the OS keychain. The public key is the Verse identity. Pairing is the act of two peers exchanging and storing each other's public keys:

- **QR code**: display → scan → mutual trust.
- **Invite link**: `verse://pair/{NodeId}/{token}` — shareable; opens Graphshell and triggers pairing.
- **mDNS discovery**: local network auto-discovery for devices on the same network.

The trust store is per-device. There is no global identity registry. Trust is bilateral and explicit.

### Conflict Resolution

Tier 1 uses a simple, deterministic conflict resolution model:

| Conflict type | Resolution |
| ------------- | ---------- |
| Concurrent metadata edits (title, tags) | Last-write-wins by wall-clock timestamp |
| Concurrent node creation at same UUID | Impossible — UUIDs are unique per device |
| Concurrent edge creation | Additive — both edges kept |
| Node deleted on one peer, edited on other | Deletion wins; edits are dropped |
| Position updates (physics) | Not synced; device-local |

For Tier 1, user-facing conflict UI is minimal: the rare case where deletion conflicts with an edit results in a toast notification ("A node you edited was deleted by a peer"). No complex merge UI is needed.

---

## Verse Community Layer (Long-Horizon Research)

Verse extends the bilateral model to larger groups of peers who share knowledge in a domain, without requiring bilateral trust with every participant.

**Key concepts** (not Phase 5 dependencies; documented fully in [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md)):

- **Dual transport**: iroh for bilateral trusted sync (Tier 1); libp2p GossipSub for community-scale broadcast (Tier 2).
- **VerseBlob**: a content-addressed, self-describing data unit for publishing curated graph subsets to a community. Unlike a `SyncUnit` (bilateral delta), a `VerseBlob` is addressed by its content hash and can be retrieved by any community member.
- **Community model**: communities form around shared knowledge domains (a topic, a workspace template, a research group). Membership is opt-in. A community has rebroadcast levels (Core → Extended → Public) governing who relays content.
- **Federated search**: community members share sharded tantivy index segments as `VerseBlob`s. Searching a community means querying peers' indexes, not a central server.
- **Proof of Access**: a lightweight economic layer where peers earn reputation (or credits) by storing and serving `VerseBlob`s for others. Deferred to post-Tier-1 research.
- **Federated adaptation (FLora)**: communities can also maintain shared domain-specific LoRA adapters, where contributors keep raw data local and publish engram payloads containing adapter memories plus contextual metadata, letting members load community-trained knowledge into their own AI tooling.

Tier 2 validation begins Q3 2026 after Tier 1 is proven in production. Tier 2 is additive — it does not change Tier 1's bilateral sync model.

### Decentralized Storage Bank

The storage bank is the **operational layer** of decentralized storage in
Verse — it covers how storage is contributed, allocated, verified, and
health-monitored. It sits between the PoA ledger (accounting) and VerseBlob
(addressing).

Key properties:

- **Two-layer credit model**: base credit for passing periodic availability
  challenges (prevents long-tail death for unpopular data) + usage-validated
  bonus on real retrieval (scaled by hold duration — "usage validates storage
  time").
- **Provider self-selection**: no global placement engine. Providers pull from
  a community replication queue and choose which blobs to host.
- **k-of-n redundancy targeting**: community sets a replication target
  (default k=3). Health monitoring tracks actual replica count per blob.
  Under-replicated blobs enter the queue at elevated priority.
- **Erasure-coding-ready**: v1 uses naive k-replication; interfaces are
  designed for future Reed-Solomon k-of-m fragment coding.
- **Fallback hierarchy**: community storage bank → bilateral peer hosting →
  self-hosting. Blobs promote up as hosting grows, demote down as hosting
  shrinks. CIDv1 addressing is the same at all levels.
- **Pledge-to-pool**: peers pledge non-transferable storage credits to a
  community pool that backs shared services (rooms, workspaces, checkpoints).
  No trading, no cross-track fungibility.

Full specification:
[2026-03-28_decentralized_storage_bank_spec.md](../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md).

---

## The Knowledge Asset Pipeline

Whether in Tier 1 or Tier 2, the Verse is most useful when nodes carry rich metadata. The pipeline from raw browsing to Verse-ready knowledge:

```
Ingest          Enrich                  Curate              Share
──────          ──────                  ──────              ─────
Browse/crawl →  MIME detection      →   UDC tags        →   Sync to peers (T1)
File drop       Schema.org / JSON-LD    #starred / #clip    Publish VerseBlob (T2)
Import          readability extract     manual annotation   Export WARC archive
                LLM summarization (opt) workspace grouping
```

The `KnowledgeRegistry` (UDC semantic tags) is the curate layer that makes browsing history into a structured personal library. A node without tags is a bookmark; a node with `udc:51` ("Mathematics") and a verified URL is a citation.

---

## Participation Levels

A Graphshell user can participate in the Verse at any level:

| Level | Requires | What you get |
| ----- | -------- | ------------ |
| None | — | Full Graphshell without sync; local-only knowledge graph |
| Verso bilateral sync | Verso + iroh | Sync your graph with specific trusted peers (friends, devices) |
| Verso workspace sharing | Verso + iroh | Share specific workspaces in read-only or read-write mode |
| Verse community participation | Verso + libp2p | Participate in topic communities; share index segments; search across community |
| Verse storage contributor | Verso + libp2p + storage quota | Earn reputation by hosting blobs for the community |
| Verse FLora contributor | Verso + libp2p + local model runtime | Submit local engrams with adapter memories to community FLora pipelines; consume approved domain adapters |
| Self-hosted Verse operator | Verso + libp2p + local storage/treasury policy | Run a private-by-default Verse node, set storage and bounty policy, and selectively expose services or communities |

Participation is always opt-in and can be revoked. Revoking access to a workspace removes the peer from the trust store and stops syncing; it does not delete data already on the peer's device.

---

## Network Architecture

### Tier 1 (iroh)

```
Peer A (Graphshell + Verso)              Peer B (Graphshell + Verso)
┌──────────────────────────┐            ┌──────────────────────────┐
│  GraphWorkspace (WAL)    │            │  GraphWorkspace (WAL)    │
│  SyncWorker              │            │  SyncWorker              │
│  Trust Store             │            │  Trust Store             │
└────────────┬─────────────┘            └─────────────┬────────────┘
             │  iroh QUIC connection                  │
             │  (Noise auth, NAT traversal)           │
             └────────────────────────────────────────┘
               SyncUnit (delta WAL entries, signed, compressed)
```

### Tier 2 (libp2p, future)

```
┌──────┐   GossipSub   ┌──────┐   GossipSub   ┌──────┐
│Peer A│ ─────────────>│Peer B│<─────────────  │Peer C│
└──────┘               └──────┘               └──────┘
  VerseBlob pub/sub        VerseBlob retrieval (Bitswap)
  Index segment sharing    Federated query routing
```

Both transport layers share the same Ed25519 identity (the same keypair derives both iroh NodeId and libp2p PeerId). Adding Tier 2 does not require re-pairing.

---

## Related Documentation

**Peer agent (Verso):**
- [../../verso_docs/technical_architecture/VERSO_AS_PEER.md](../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Verso mod: web capability + bilateral peer agent; ModManifest, SyncWorker, pairing, co-op boundary

**Tier 1 implementation:**
- [../implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](../implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — iroh scaffold, Ed25519 identity, pairing ceremonies, SyncUnit wire format, SyncWorker control plane, workspace access grants, Phase 5 execution plan

**Tier 2 research:**
- [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md) — dual transport, VerseBlob, community swarms, federated search, Proof of Access, research roadmap
- [../implementation_strategy/engram_spec.md](../implementation_strategy/engram_spec.md) — canonical `Engram` / `TransferProfile` schema for local exchange and FLora submissions
- [../implementation_strategy/verseblob_content_addressing_spec.md](../implementation_strategy/verseblob_content_addressing_spec.md) — canonical `VerseBlob` envelope, CID rules, transport split, and retrieval policy
- [../implementation_strategy/flora_submission_checkpoint_spec.md](../implementation_strategy/flora_submission_checkpoint_spec.md) — FLora submission, review, checkpoint, and reward hook specification
- [../implementation_strategy/proof_of_access_ledger_spec.md](../implementation_strategy/proof_of_access_ledger_spec.md) — receipt, reputation, epoch accounting, and optional payout model
- [../implementation_strategy/community_governance_spec.md](../implementation_strategy/community_governance_spec.md) — governance roles, quorum, moderation, treasury, and dispute rules
- [../implementation_strategy/self_hosted_verse_node_spec.md](../implementation_strategy/self_hosted_verse_node_spec.md) — private-by-default local Verse node operating model and service guardrails
- [../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md](../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md) — decentralized storage bank: contributing, using, managing storage; credit mechanics, placement, durability, pledge-to-pool

**Graphshell context:**
- [GRAPHSHELL_AS_BROWSER.md](../../graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) — browser model; how knowledge is created and organized before it enters the Verse
- [../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md](../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md) — Phase 5 registry integration; Verso's `ModManifest` and `VerseMod` registration
