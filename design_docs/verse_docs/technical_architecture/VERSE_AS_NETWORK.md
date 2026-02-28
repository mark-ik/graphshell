# VERSE AS NETWORK

**Purpose**: Specification for the Verse network — what it is, what it does, and how Graphshell participates in it through the Verso peer agent.

**Document Type**: Network and protocol specification (not implementation status)
**Status**: Tier 1 (bilateral iroh sync) in Phase 5 implementation; Tier 2 (community swarms, federated search, Proof of Access, FLora/engram exchange) is long-horizon research (Q3 2026+)
**See**: [VERSO_AS_PEER.md](../../graphshell_docs/technical_architecture/VERSO_AS_PEER.md) for how Graphshell's Verso mod participates; [2026-02-23_verse_tier1_sync_plan.md](../implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) for the Tier 1 implementation plan; [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md) for the long-horizon swarm architecture

---

## What the Verse Is

The Verse is a **decentralized, peer-to-peer knowledge network** that Graphshell instances participate in. It transforms the browser from a consumer of the web to a participant in a new layer on top of it.

The Verse is not a server. It is not a platform. It is a set of protocols and data formats that Graphshell peers speak to each other. Every user running Graphshell with Verso enabled is a Verse peer.

Each local Verse instance is best understood as a **private-by-default, self-hosted portal**: a sovereign node that can keep data and model customizations private, or selectively participate in consensual storage, indexing, and adaptation economies.

**The Verse has two tiers:**

- **Tier 1** — Bilateral sync: two trusted peers synchronize their graph state directly over iroh (QUIC-based, NAT-traversing transport). This is the Phase 5 deliverable. It works offline-first; peers sync when they connect.
- **Tier 2** — Community swarms: larger groups of peers form communities around shared knowledge domains, exchanging curated index segments and content-addressed blobs via libp2p GossipSub. This is long-horizon research.

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

## Tier 1: Bilateral Peer Sync

### The Model

Two Graphshell instances that have paired with each other sync their graphs bilaterally. Think git between two machines: operations batch at the WAL level, sync happens when peers connect, conflicts are resolved deterministically (last-write-wins on metadata; additive merge for nodes and edges).

**Properties:**

- **Local-first**: works perfectly offline; sync is opportunistic.
- **No mandatory server**: peers connect directly via iroh Magic Sockets (QUIC with NAT traversal). No relay required for most network configurations.
- **End-to-end encrypted**: iroh uses the Noise protocol for transport authentication; at-rest data uses AES-256-GCM.
- **Selective**: each workspace can be shared with specific peers at `ReadOnly` or `ReadWrite` access level.

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

## Tier 2: Community Swarms (Long-Horizon Research)

Tier 2 extends the bilateral model to larger groups of peers who share knowledge in a domain, without requiring bilateral trust with every participant.

**Key concepts** (not Phase 5 dependencies; documented fully in [2026-02-23_verse_tier2_architecture.md](2026-02-23_verse_tier2_architecture.md)):

- **Dual transport**: iroh for bilateral trusted sync (Tier 1); libp2p GossipSub for community-scale broadcast (Tier 2).
- **VerseBlob**: a content-addressed, self-describing data unit for publishing curated graph subsets to a community. Unlike a `SyncUnit` (bilateral delta), a `VerseBlob` is addressed by its content hash and can be retrieved by any community member.
- **Community model**: communities form around shared knowledge domains (a topic, a workspace template, a research group). Membership is opt-in. A community has rebroadcast levels (Core → Extended → Public) governing who relays content.
- **Federated search**: community members share sharded tantivy index segments as `VerseBlob`s. Searching a community means querying peers' indexes, not a central server.
- **Proof of Access**: a lightweight economic layer where peers earn reputation (or credits) by storing and serving `VerseBlob`s for others. Deferred to post-Tier-1 research.
- **Federated adaptation (FLora)**: communities can also maintain shared domain-specific LoRA adapters, where contributors keep raw data local and publish engram payloads containing adapter memories plus contextual metadata, letting members load community-trained knowledge into their own AI tooling.

Tier 2 validation begins Q3 2026 after Tier 1 is proven in production. Tier 2 is additive — it does not change Tier 1's bilateral sync model.

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

## What the Verse Is Not

- **Not a search engine**: Verse does not crawl the web. It indexes what you and your peers have visited and curated.
- **Not a social network**: Verse has no feeds, no follows, no public timelines in Tier 1. Community swarms (Tier 2) add opt-in group discovery, but not social media mechanics.
- **Not a cloud service**: there is no Verse server. Peers sync directly. A relay may be used for NAT traversal, but it sees only encrypted bytes and holds no state.
- **Not a blockchain**: Verse does not use a chain data structure. Proof of Access (Tier 2) uses reputation credits, not tokens, in the current research direction.

---

## Participation Levels

A Graphshell user can participate in the Verse at any level:

| Level | Requires | What you get |
| ----- | -------- | ------------ |
| None | — | Full Graphshell without sync; local-only knowledge graph |
| Tier 1 (bilateral) | Verso + iroh | Sync your graph with specific trusted peers (friends, devices) |
| Tier 1 (workspace sharing) | Verso + iroh | Share specific workspaces in read-only or read-write mode |
| Tier 2 (community) | Verso + libp2p | Participate in topic communities; share index segments; search across community |
| Tier 2 (storage contributor) | Verso + libp2p + storage quota | Earn reputation by hosting blobs for the community |
| Tier 2 (FLora contributor) | Verso + libp2p + local model runtime | Submit local engrams with adapter memories to community FLora pipelines; consume approved domain adapters |
| Tier 2 (self-hosted verse operator) | Verso + libp2p + local storage/treasury policy | Run a private-by-default verse node, set storage and bounty policy, and selectively expose services or communities |

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
- [VERSO_AS_PEER.md](../../graphshell_docs/technical_architecture/VERSO_AS_PEER.md) — Verso mod: web capability + Verse peer agent; ModManifest, SyncWorker, pairing

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

**Graphshell context:**
- [GRAPHSHELL_AS_BROWSER.md](../../graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) — browser model; how knowledge is created and organized before it enters the Verse
- [../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md](../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md) — Phase 5 registry integration; Verso's `ModManifest` and `VerseMod` registration
