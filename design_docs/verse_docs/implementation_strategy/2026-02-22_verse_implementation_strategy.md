# Verse Implementation Strategy: Quick Reference

**Date**: 2026-02-22 (original), 2026-02-23 (split)
**Status**: Index / Orientation Document
**Context**: This document has been split into two focused files to separate implementation-ready engineering plans from long-horizon research architecture.

---

## Document Split Rationale

The original Verse strategy combined **Tier 1** (concrete Phase 5 deliverables: iroh-based bilateral sync, identity, pairing, conflict resolution) with **Tier 2** (speculative architecture: libp2p swarms, VerseBlob content addressing, Proof of Access economics, federated search). This created cognitive overload for engineers executing Phase 5 and prevented Tier 2 from evolving independently.

**New structure**:

1. **[Tier 1 Implementation Plan](2026-02-23_verse_tier1_sync_plan.md)** (~800 lines)  
   Complete specification for Registry Phase 5: iroh transport, Ed25519 identity, pairing ceremonies, delta sync protocol, SyncWorker control plane integration, workspace access grants, UX designs, security model, and step-by-step execution plan. **This is the concrete Phase 5 deliverable.**

2. **[Tier 2 Architecture](../technical_architecture/2026-02-23_verse_tier2_architecture.md)** (~600 lines)  
   Long-horizon research: dual-transport model (iroh + libp2p), VerseBlob content format, community swarms with GossipSub, federated search, Proof of Access economic layer, Nostr signaling, content pipeline, crawler economy. **This is exploratory — not a Phase 5 dependency.**

Engineers implementing Phase 5 should focus on **Tier 1 only**. Tier 2 provides architectural context for future evolution but makes no immediate claims on the implementation roadmap.

---

## Quick Start

- **I'm implementing Verse sync for Phase 5**: Read [Tier 1 Implementation Plan](2026-02-23_verse_tier1_sync_plan.md). Follow the execution plan in §9 (Steps 5.1–5.5). All dependencies, UX mockups, and done gates are defined.

- **I'm researching federated knowledge protocols**: Read [Tier 2 Architecture](../technical_architecture/2026-02-23_verse_tier2_architecture.md). This explores community-scale swarms, economic incentives, and search infrastructure. Treat it as a design space, not a requirement.

- **I'm reviewing Verse holistically**: Skim Tier 1 §1–5 (identity, transport, sync protocol, conflict resolution) for the core model, then read Tier 2 §1–2 (dual-transport rationale, identity bridge) to understand how Tier 1 extends to public swarms.

---

## What Changed

### Tier 1 (Implementation Plan)
- **§1**: Overview — Tier 1 characteristics (iroh, bilateral, pairing, LWW conflicts)
- **§2**: Identity & Pairing — Ed25519 keypair in OS keychain, trust store, pairing flows (code/QR, mDNS, invite links)
- **§3**: Transport (iroh) — QUIC, Magic Sockets, NAT traversal, connection model
- **§4**: Sync Protocol — SyncUnit wire format, version vectors, delta computation, conflict resolution strategies
- **§5**: SyncWorker — Control plane integration, accept loop, intent pipeline, backpressure
- **§6**: UX Design — Sync status indicator, Sync Panel, pairing flows, workspace sharing, conflict resolution UI
- **§7**: Security & Encryption — Noise transport auth, at-rest AES-256-GCM, trust boundaries
- **§8**: Registry Integration — ModManifest, initialization sequence, ActionRegistry extensions, diagnostics channels, offline graceful degradation
- **§9**: Phase 5 Execution Plan — 5 thin vertical slices with done gates (iroh scaffold, trust store, pairing UI, delta sync, access control)
- **§10**: Crate Dependencies — iroh, keyring, mdns-sd, qrcode, rkyv, zstd, aes-gcm
- **§11**: Open Questions (Tier 1 only) — Identity scope, relay infrastructure, sync triggers, conflict accumulation, VV pruning, workspace granularity

### Tier 2 (Architecture)
- **§1**: Dual-Transport Model — iroh (bilateral) + libp2p (community swarms)
- **§2**: Identity Bridge — Same Ed25519 keypair derives both iroh NodeId and libp2p PeerId
- **§3**: VerseBlob — Content-addressed universal format (replaces delta-based SyncUnit for public swarms)
- **§4**: Community Model — Governance, rebroadcast levels, GossipSub pubsub, Bitswap content retrieval
- **§5**: Search Architecture — Sharded indexes, IndexSegment as VerseBlob, federated query model
- **§6**: Proof of Access — Receipt model, aggregation, reputation vs token settlement, "no-receipt" flag for Tier 1
- **§7**: Research Agenda — DHT scalability, moderation at scale, index freshness, economic model validation
- **§8**: Nostr Signaling — Optional bootstrap layer for peer discovery and announcements
- **§9**: Content Pipeline — Ingest → Enrich → Curate → Publish; WARC archives, CRDT integration (speculative)
- **§10**: Protocol Ecosystem Mapping — iroh, libp2p, IPFS, Nostr, ActivityPub, Filecoin, Lightning
- **§11**: Crawler Economy — Bounty model for external web content ingestion, anti-spam, platform bridges
- **§12**: Open Questions (Tier 2 only) — VerseBlob vs IPFS CID, libp2p vs iroh consolidation, token vs reputation, community bootstrapping
- **§13**: Relationship to Tier 1 — Additive design, opt-in communities, no impact on bilateral sync
- **§14**: Research Roadmap — Q3 2026 validation, Q4 2026 pilot, 2027 economic layer + spec stabilization
- **§15**: Alignment with Graphshell's Mission — Personal tool vs protocol layer pivot; long-horizon bet

---

## Rationale for Split

From `2026-02-22_registry_interaction_design_notes.md` §On Document Length:

> Tier 1 (~770 lines) is **implementation-ready**: engineers can execute Phase 5.1–5.5 without ambiguity. Tier 2 (~555 lines) is **long-horizon research**: it explores architectural space but makes no immediate engineering claims. Combining them into one 1320-line document forces engineers to wade through speculative design (libp2p, VerseBlob, Proof of Access) when they only need the iroh sync contract.

**Split benefits**:
- Phase 5 engineers can focus on Tier 1 without cognitive load from Tier 2's economic/community models
- Tier 2 can evolve independently via separate research doc (new alternatives, abandoned ideas) without destabilizing the Phase 5 plan
- Clear "done gate" boundary: Phase 5 is complete when Tier 1 works; Tier 2 validation happens later (Q3 2026+)

---

## Legacy Note

This file previously contained the full Verse strategy (§1–12, 1353 lines). It has been refactored into:
- `2026-02-23_verse_tier1_sync_plan.md` (§1–10 + Tier 1 open questions)
- `../technical_architecture/2026-02-23_verse_tier2_architecture.md` (§11 + Tier 2 open questions + research roadmap)

Original content preserved in git history (`git show HEAD~1:design_docs/verse_docs/implementation_strategy/2026-02-22_verse_implementation_strategy.md`).
