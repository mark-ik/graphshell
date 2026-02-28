# Verse: Concrete Research Agenda (2026-02-28)

**Status**: Active research agenda
**Scope**: Six concrete research threads needed to advance Verse from its current Tier 1 baseline toward Tier 2. Each section names the open question, what the docs currently say, what must be learned, and what a useful research deliverable looks like.

---

## 1. Local Model Feasibility on Realistic Hardware Tiers

### What the Docs Say

The local intelligence research (`2026-02-24_local_intelligence_research.md`) selects `burn` with WGPU backend for inference, recommends a four-model stack (MiniLM + Qwen2.5-0.5B + Florence-2 + Whisper-Tiny), and gives a total working RAM target of ~680 MB. It asserts that `burn`'s WebGPU backend runs on integrated GPUs and older AMD/Intel hardware — but does not back this with measured latency or memory-resident footprint figures on real consumer devices.

The engram spec and FLora pipeline assume that local contributors can run a "mini-adapter pass" to produce LoRA deltas. The self-hosted model spec allows LoRA injection at runtime. Neither document characterizes what hardware is actually required to make that pipeline practical.

### Open Questions

1. On a machine with no discrete GPU (e.g. Intel Iris Xe, 8 GB RAM, mid-2021 laptop): what is the latency for a single embedding pass, a summarization call, and a LoRA merge? Is it interactive (< 300 ms) or batch-only?
2. What is the minimum RAM available to the inference subsystem when Servo + the graph compositor + the OS are also resident? Does the 680 MB stack fit in practice, or does it compete with Servo's content processes?
3. LoRA fine-tuning (even for small adapters) requires gradient computation and optimizer state — typically 4–10× the forward-pass memory. Which consumer hardware tiers can run even a single-epoch local adapter pass on a few hundred examples in a reasonable session window (< 20 minutes)?
4. What quantization levels (INT8, INT4, GGUF Q4) are compatible with `burn`/WGPU, and how do they change the quality/speed trade-off for the use cases identified (embeddings, summarization, extraction)?

### Hardware Tier Classification Needed

A practical tier taxonomy for the docs:

| Tier | Profile | Example hardware |
|------|---------|-----------------|
| T-Min | CPU-only, 8 GB RAM | Budget laptop, no GPU |
| T-Mid | iGPU (Vulkan/DX12), 16 GB RAM | Mid-range laptop 2021–2024 |
| T-Full | dGPU 4–8 GB VRAM, 16–32 GB RAM | Desktop / gaming laptop |
| T-Server | dGPU 16+ GB VRAM, 32+ GB RAM | Workstation / contributor node |

### Research Deliverable

A `2026-xx-xx_local_model_hardware_benchmarks.md` file containing:
- Measured latency and peak RSS for each model on each tier (embedding, summarization, LoRA merge).
- Which features are feasible at each tier (background embedding = T-Min; interactive summarization = T-Mid; local adapter pass = T-Full).
- A recommended feature-flag → hardware tier mapping so the self-hosted model spec can reference it.
- WGPU/`burn` quantization support matrix.

---

## 2. Engram Derivation Types and Model Diet Compatibility

### What the Docs Say

The engram spec (`engram_spec.md`) defines `EngramDerivationType` (AdapterWeights, SoftPrompt, EmbeddingVector, HashFingerprint, PerceptualHash, LocalitySensitiveHash, StructuredFact, SchemaRecord, DerivedSummary, EvalMetric) and `ModelDietKind` (AdapterTunable, RetrievalAugmented, SymbolicAugmented, PromptConditioned, MultiDiet). It states that hashes and fingerprints are not substitutes for adapter weights, and that each model/slot should declare which derivation types it accepts.

The intelligence memory architecture plan (`2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`) defines how these derivation types flow through STM/LTM/Extractor/Ingestor, but doesn't validate which base models actually benefit from which derivation types in practice.

### Open Questions

1. For retrieval-augmented use cases (the most accessible diet since they don't require fine-tuning): which base models in the four-model stack can effectively consume an `EmbeddingVector` memory at inference time via RAG? Is a 0.5B model RAG-capable at acceptable quality?
2. `StructuredFact` and `SchemaRecord` memories are described as useful for symbolic/tool-augmented systems. Which practical tools or retrieval backends (tantivy, a local SQLite FTS, a simple vector store) can consume these, and what schema format is most portable (JSON-LD, RDF, custom)?
3. For `AdapterWeights` (the main trainable diet): what base model families support PEFT/LoRA in the Verse stack, and what rank/alpha configurations are appropriate for the adaptation domains Verse is targeting (browsing history, UDC classification, domain knowledge)?
4. `DerivedSummary` is mentioned as a "compressed evidence" type. Can a derived summary from one model be consumed as a few-shot prompt by a different model family, and does the quality remain useful after this level of indirection?
5. `HashFingerprint` / `PerceptualHash` / `LocalitySensitiveHash` are useful for dedup and routing — but what is the practical collision rate and false-positive rate on the content types Verse handles (web pages, text nodes, image nodes)? Which LSH algorithm is appropriate per content type?

### Research Deliverable

A `2026-xx-xx_engram_diet_compatibility_matrix.md` file containing:
- Per-model (from the four-model stack): which derivation types it can consume, in which mode (RAG, prompt injection, weight merge), and at what quality threshold.
- A worked example of a Verse submission round-trip for each diet type: what the contributor generates locally, what crosses the wire, and what the consumer can do with it.
- Recommended default diet declarations for each model in `self_hosted_model_spec.md`.
- A recommended minimum viable engram for each `EngramValidationClass` (LocalPrivate, LocalExportable, VerseSubmission, VerseCheckpoint) expressed in terms of which derivation types must be present.

---

## 3. Extraction-to-Engram Transforms Useful Without Sharing Raw Data

### What the Docs Say

The engram spec (§8.4, §11.3) explicitly establishes that the default is: local raw data is used privately; Verse-facing engrams contain only derived artifacts and metadata. Preferred low-risk submission materials include hashes, fingerprints, DOM fingerprints, extracted facts, UDC classifications, eval bundles, adapter weights, and derived summaries. Raw clips, screenshots, and private notes must remain local by default.

The intelligence memory plan defines the `MemoryExtractor` interface (`export_snapshot`, `export_stream`) and its export safety/policy checks (privacy, license, redaction, provenance stamping). The self-hosted model spec addresses capability contracts but not the specifics of which extraction transforms are practical for a browser use case.

### Open Questions

1. **DOM fingerprinting**: What structural features of a webpage produce a fingerprint that is useful for dedup/routing without reconstructing the page? What is the right algorithm: simhash on rendered text, structural hash of tag sequences, or a perceptual hash on a screenshot thumbnail? How stable are these across minor page updates?
2. **Readability + embedding as a privacy-preserving proxy**: If you extract article text via readability, embed it with MiniLM, and keep only the embedding vector, how much semantic information is retained for community routing vs. how much private content is at risk from embedding inversion attacks?
3. **UDC auto-classification**: What is the practical accuracy of running Qwen2.5-0.5B or a fine-tuned MiniLM against a page's readability text to produce a top-3 UDC classification? What precision/recall is needed before UDC codes become useful as community routing signals?
4. **Synthetic examples**: The engram spec includes `SyntheticExamples` as a memory type. For which kinds of browsing behavior can you produce useful synthetic training examples without revealing the source URL or raw content (e.g., classification labels, instruction-following pairs, summarization inputs)?
5. **Adapter weight privacy**: If a contributor trains a LoRA on private documents, how much of the private content is recoverable from the delta weights alone? This is a well-studied problem in membership inference attacks — what mitigations are practical at the contributor's local node (differential privacy noise, weight clipping, eval-only submission)?

### Research Deliverable

A `2026-xx-xx_extraction_transform_catalog.md` file containing:
- A catalog of practical extraction transforms, each with: input type, output derivation type, estimated local compute cost, privacy risk class, and usefulness class for community routing.
- For the highest-value transforms (DOM fingerprint, embedding vector, UDC classification): a recommended algorithm, implementation candidate in the Rust ecosystem, and known failure modes.
- A recommended default `RedactionProfile` template for each extraction type.
- A note on membership inference risk for LoRA submissions with a pointer to the relevant literature.

---

## 4. Legal-Risk Classes for Verse Content and Reconstruction Policy

### What the Docs Say

The engram spec names higher-risk submission materials (raw clips, screenshots, snippets, private notes) and says they should remain local by default. The self-hosted node spec (§10) requires that private dataset lineage be preserved locally while only derived metadata is shared. The Tier 2 architecture (§9.2) describes WARC archiving as an optional blob payload for community content.

No document yet defines a legal risk taxonomy for Verse content, reconstruction policy for archived pages, copyright implications of LoRA weight sharing, or jurisdiction-specific considerations.

### Open Questions

1. **Copyright and database rights in submitted content**: If a user clips a news article, runs readability extraction, and submits a derived summary as an engram, who holds rights to the summary? Does this differ if the summary is machine-generated vs. hand-edited? Does the UDC classification of the source constitute a database right claim in EU jurisdictions?
2. **WARC archives and Section 512/DMCA safe harbor**: If a Verse community stores WARC blobs of web pages and serves them to peers, is the hosting node a "service provider" eligible for safe harbor? What notice-and-takedown process is required at the protocol level?
3. **LoRA weight sharing and derivative works**: Under current US copyright law, do trained model weights constitute a derivative work of the training corpus? The legal status is unsettled — but what is the current best practice from the ML research community (e.g., model cards, dataset provenance disclosures) that Verse should adopt defensively?
4. **EU AI Act and GDPR interaction**: If a Verse community collects engrams that include behavioral metadata (browsing history summaries, traversal logs), does this constitute profiling under GDPR? What data minimization requirements apply? Does the EU AI Act's classification of "AI systems" apply to FLora's federated LoRA training?
5. **Reconstruction risk**: If a community blob index retains enough index entries (title + 200-char preview + UDC + URL) to substantially reconstruct the content of a web page, does this create independent liability distinct from the underlying WARC archive?

### Research Deliverable

A `2026-xx-xx_verse_legal_risk_taxonomy.md` file containing:
- A tiered risk taxonomy for Verse content types: Green (low risk: hashes, embeddings, metadata), Yellow (medium risk: derived summaries, index entries, synthetic examples), Red (high risk: raw clips, full WARC archives, scraped training corpora).
- Per-tier: recommended handling policy, what the self-hosted node spec should say about defaults, and what community governance rules should say about hosting and serving.
- A reconstruction policy recommendation for community index segments (e.g., no more than N chars preview, no URL reconstruction from index alone).
- A recommended minimum provenance disclosure format for LoRA submissions that satisfies current ML community best practice.
- A note on jurisdictions where additional legal review is required before Tier 2 community hosting is practical.

---

## 5. libp2p Operational Defaults for Safe Tier 2 Deployment

### What the Docs Say

The Tier 2 architecture (`2026-02-23_verse_tier2_architecture.md`) specifies libp2p with GossipSub for community broadcast and Bitswap for content retrieval. It notes libp2p Kademlia DHT works well for <10,000 nodes and degrades beyond that, mentions S/Kademlia extensions for eclipse attack resistance, and lists resource safety requirements for the self-hosted node spec (upload/download rate limits, connection caps, storage quota, decompression ratio caps). The self-hosted node spec (§9) lists these as minimum protections but does not give concrete values.

The Freenet takeaways (`2026-02-27_freenet_takeaways_for_verse.md`) recommend avoiding over-centralized routing paths and keeping transport/protocol specs explicitly labeled as `implemented` vs. `proposed`.

### Open Questions

1. **GossipSub D parameters**: What are the right `D`, `D_low`, `D_high`, `D_lazy` mesh degree parameters for a Verse community of 10–500 nodes? Too low = poor availability; too high = bandwidth waste. What is the expected fan-out cost per published `VerseBlob` at these scales?
2. **DHT bootstrap**: Kademlia requires bootstrap peers. For Verse, this means either hardcoded rendezvous servers (centralization risk) or Nostr-signaled multiaddrs (adds dependency). What is the recommended bootstrap strategy that preserves decentralization while being practical at initial launch?
3. **Resource manager defaults**: The libp2p resource manager (`go-libp2p-resource-manager` / `rust-libp2p` equivalents) controls memory, streams, connections, and file descriptors per peer and globally. What are the right default values for a Verse node running alongside Servo (a memory-intensive browser engine) on a T-Min or T-Mid device?
4. **GossipSub message size limits and backpressure**: `VerseBlob` payloads can include `IndexSegment` (arbitrary size) and `Engram` (could be large with embedded weights). GossipSub imposes maximum message sizes. What is the right size cap for GossipSub messages, and how should large payloads be split (announce-then-fetch via Bitswap, or multipart streaming)?
5. **Peer scoring and spam resistance**: GossipSub's peer scoring mechanism allows down-scoring peers that publish invalid or spam messages. What scoring parameters are appropriate for Verse's community model, and how should the scoring interact with the moderation rules in `community_governance_spec.md`?
6. **NAT traversal fallback**: iroh Magic Sockets handle NAT traversal for Tier 1 via relay. For Tier 2 libp2p, what is the right NAT hole-punching strategy (DCUtR, AutoNAT, relay nodes)? What fraction of typical consumer ISP configurations fail all hole-punching attempts and require a relay?

### Research Deliverable

A `2026-xx-xx_libp2p_operational_defaults.md` file containing:
- Recommended libp2p configuration table for a Verse community node: GossipSub parameters, DHT parameters, resource manager limits, connection manager limits — with a column for each hardware tier (T-Min, T-Mid, T-Full).
- Bootstrap strategy recommendation with fallback hierarchy: mDNS (LAN) → Nostr-signaled multiaddrs → hardcoded community rendezvous.
- GossipSub message size policy: max inline payload size, Bitswap CID-announce pattern for larger objects.
- Peer scoring baseline configuration for a Verse community.
- NAT traversal failure rate estimate for consumer ISPs and recommended relay configuration.

---

## 6. iroh and libp2p Operational Scaling: Telescoping from Tier 1 to Tier 2

### What the Docs Say

This is the most open question in the Verse network architecture. The Tier 2 architecture document (§13) states that Tier 2 is additive and that both transports share the same Ed25519 identity (§2). The dual-transport model (§1) positions iroh for bilateral/trusted sync and libp2p for community swarms, with a unified `VerseBlob` API routing to each.

But there is no analysis of how these two networking strategies telescope into each other — i.e., how a user naturally moves from a purely local-scale Tier 1 instance (two trusted devices, LAN mDNS) to a local-to-global Tier 2 scope, and what the operational mechanics of that expansion look like.

The user's questions identify several specific sub-problems: local peer hosting limits, the seed/peer model for expanding hosting as more people join and rebroadcast, how discoverability works for IPFS-style content addressing, and to what extent iroh and libp2p telescope into each other.

### Open Questions

#### A. iroh: local peer hosting limit

1. iroh is designed for bilateral sync over QUIC. What is the practical ceiling on simultaneous bilateral sessions on a T-Mid device before iroh's resource consumption conflicts with Servo? Is there a documented connection count limit in the iroh codebase?
2. iroh's relay infrastructure is currently operated by n0 (the iroh maintainers). What is the policy and availability of these relays? Can Verse run its own relay for a community, and what are the resource requirements?
3. For Tier 1 "constellation" scenarios (a user has 5 trusted devices, or a small team of 10 peers all syncing with each other), does bilateral iroh scale adequately, or does it become O(n²) connection overhead at some threshold?

#### B. Seed/peer model for expanding hosting

4. The VERSE.md doc defines "Seeders/rebroadcasters" as a peer role. The Tier 2 architecture defines `RebroadcastLevel` (Full / Selective / None). As a community grows, how does hosting workload distribute? Is the model: a small core of Full rebroadcasters subsidize a larger population of None/Selective participants? At what community size does this break down?
5. For VerseBLOBs stored on the DHT, what is the expected replication factor (k=3 is mentioned) and the expected retrieval latency as a function of community size and online-peer fraction? Is Kademlia's routing sufficient, or does Verse need a separate "well-known seeders" list per community for high-availability blobs?
6. Can a Verse instance start as a Tier 1 iroh peer, contribute blobs to a small community via iroh's blob transfer (without joining the full libp2p swarm), and then optionally upgrade to a full libp2p community participant? What is the migration path?

#### C. Discoverability: IPFS content addressing vs. Verse DHT

7. IPFS's Kademlia DHT provides global content discoverability by hash. Verse's VerseBlob also uses content hashing. Are these interoperable? Can a VerseBlob be served from an IPFS node by content hash, or does the envelope format differ enough to prevent this?
8. If Verse uses its own DHT (libp2p Kademlia with a Verse-specific namespace), how does a new user discover existing communities? Without prior knowledge of bootstrap peers or community IDs, what is the discovery path? (Currently: UDC tags guide routing; community IDs are hashes of name + genesis block — but this presupposes the user knows the name.)
9. The Tier 2 architecture mentions Nostr as an optional signaling layer for peer discovery (kind 30078 events). Is this sufficient for bootstrapping discovery at Tier 2 scale, or does Verse need a more durable directory mechanism (e.g., a well-known IPNS record, or a community registry published as a VerseBlob itself)?

#### D. Telescope model: how Tier 1 and Tier 2 extend each other

10. The unified `VerseBlob` API (§1.1 of Tier 2 architecture) routes iroh for bilateral sync and libp2p for community content. In practice, a blob created during Tier 1 sync (a `SyncUnit`) is different from a community-published `VerseBlob` — the former carries version vectors and is receiver-specific. What is the practical relationship? Can a Tier 1 `SyncUnit` be re-packaged as a `VerseBlob` for community sharing, and what information is lost or needs to be added?
11. If a self-hosted node is operating as `CommunityHost` and is also a Tier 1 bilateral peer with several friends, are the two transport stacks independent (different ports, different resource pools) or can they share state (e.g., a blob already fetched for a Tier 1 sync can be served to a Tier 2 requester without re-download)?

### Research Deliverable

A `2026-xx-xx_verse_network_telescope_model.md` file containing:
- An iroh connection scaling table: max practical bilateral connections per hardware tier, iroh relay policy and self-hosted relay setup.
- A Tier 1 → Tier 2 migration path: the sequence of steps a node takes to go from local-only iroh to a full libp2p community participant, with the transport and identity continuity guarantees at each step.
- A community hosting economy model: the seed/rebroadcast balance at various community sizes (10, 100, 1000 peers), expected DHT lookup latency, and the minimum "infrastructure class" of nodes needed to sustain a community.
- A Verse vs. IPFS interoperability assessment: are VerseBLOBs retrievable from an IPFS gateway by CID, and what is the trade-off of IPFS compatibility vs. tighter Verse envelope integration?
- A community discoverability recommendation: the bootstrap hierarchy (mDNS → iroh Magic Sockets → Nostr signaling → hardcoded community relay), with the conditions under which each step is invoked and what a user sees at each step.
- A concrete recommendation on whether the Tier 1 / Tier 2 transports should share a blob cache, and if so, what the cache coherence and privacy boundary rules are.

---

## Cross-Cutting Notes

### Priority ordering

Research areas 1 (hardware feasibility) and 5 (libp2p defaults) are prerequisites for safe Tier 2 deployment. Areas 2 (diet compatibility) and 3 (extraction transforms) are prerequisites for the FLora pipeline being useful to contributors. Area 4 (legal risk) is a prerequisite for any public-facing community hosting. Area 6 (telescope model) is the longest-horizon but is architecturally foundational.

Recommended sequencing: 1 → 3 → 2 → 5 → 6 → 4 (legal review can proceed in parallel from Q3 2026 but does not block Tier 1).

### Relationship to existing open questions in the docs

The Tier 2 architecture (§12) lists four open questions: VerseBlob vs IPFS CID format (addressed in area 6), libp2p vs iroh consolidation (addressed in areas 5 and 6), Proof of Access economics (not addressed here — deferred to post-Tier-2-pilot), and community bootstrapping (addressed in area 6). The engram spec (§14) open questions on payout policy representation and checkpoint signing requirements are not addressed here — they belong to governance/FLora research.

### What this agenda does not cover

- Proof of Access ledger design and token economics (deferred to post-Tier-2-pilot per VERSE_AS_NETWORK.md).
- Community governance spec implementation details (addressed in `community_governance_spec.md`).
- Full-text index replication freshness problem (partially addressed in Tier 2 architecture §7.3; more research needed but scoped to a separate search architecture track).
- CRDT integration for collaborative editing (explicitly out of scope for Verse Tier 1/2 baseline).
