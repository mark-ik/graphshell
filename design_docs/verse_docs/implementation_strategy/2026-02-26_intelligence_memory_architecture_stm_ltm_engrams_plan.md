# Intelligence Memory Architecture: STM/LTM, Engrams, and Extractor/Ingestor Plan

**Date**: 2026-02-26
**Status**: Proposed (design-ready)
**Scope**: Intelligence memory management architecture for Graphshell/Verse, including short-term memory (STM), long-term memory (LTM), engram memory storage, ectoplasm streams, and extractor/ingestor workflows.
**Related**:
- `design_docs/verse_docs/implementation_strategy/2026-02-26_model_slots_adapters_udc_personalization_plan.md`
- `design_docs/verse_docs/research/2026-02-24_local_intelligence_research.md`
- `design_docs/TERMINOLOGY.md`

---

## 1. Why This Is a Separate Plan

The model-slots/adapters plan defines:
- slot contracts
- capability declarations
- conformance/evaluation
- adapter compatibility/portability
- archetypes and engrams as transfer concepts

This document defines a **different but related system**: memory management and transfer plumbing.

This separation keeps responsibilities clear:
- **Model slots plan** = what models can do, how they are evaluated, and how features bind to them
- **Memory architecture plan** = how intelligence memories are stored, indexed, promoted, streamed, imported, and exported

---

## 2. Core Concepts (Memory Layer)

### 2.1 Short-Term Memory (STM)

**Short-Term Memory (STM)** is the local, high-churn, automatically managed, user-editable working memory store for intelligence workflows.

Typical STM contents:
- session notes / working summaries
- task state / agent scratchpads
- recent retrieved chunks and relevance annotations
- pending eval runs and partial results
- temporary dataset slices before promotion
- buffered `Ectoplasm` samples/traces

Properties:
- editable
- high write frequency
- TTL/compaction support
- easy rollback/rebuild
- local-first (not automatically shared)

### 2.2 Long-Term Memory (LTM)

**Long-Term Memory (LTM)** is the durable, indexed, versioned memory store for reusable intelligence artifacts and engram memories.

Typical LTM contents:
- `EngramMemory` records
- `TransferProfile` (`Engram`) bundles
- dataset lineage and profiles
- eval profiles and regression receipts
- adapter manifests and compatibility reports
- archetype profiles
- community reports and benchmark receipts
- prompt bundles / task recipes

Properties:
- versioned and provenance-rich
- searchable (keyword + semantic + filtered)
- promotable/shareable via Verse
- suitable for curation and reuse

### 2.3 Engram Memory vs Ectoplasm

- **EngramMemory** = persisted memory unit (portable, indexed, versioned)
- **Ectoplasm** = optional runtime internal signal stream (ephemeral, provider-dependent, policy-gated)

Ectoplasm may be transformed into EngramMemory records, but the two must remain distinct.

---

## 3. Architectural Responsibilities

### 3.1 Memory Subsystems / Components (Proposed)

1. `ShortTermMemoryStore`
- Local DB for working memory and transient records
- Optimized for write-heavy, mutable workflows

2. `LongTermMemoryIndex`
- Durable index/storage for `EngramMemory` and related metadata
- Context-harness-like ingestion/index/search layer (adapted to Graphshell/Verse schemas)

3. `MemoryExtractor`
- Outbound conversion/export surface (LoRA/dataset/engram/ectoplasm/evals/archetypes)

4. `MemoryIngestor`
- Inbound import/normalize/validate/quarantine surface

5. `MemoryPromotionPolicy`
- Rules and workflows for STM -> LTM promotion and LTM -> STM hydration

6. `EctoplasmBridge` (optional capability)
- Runtime adapters for providers that expose internal signal streams

### 3.2 Relationship to ModelRegistry / AgentRegistry

- `ModelRegistry` (from the related plan) remains responsible for model/adaptor capability inventory, slot binding, and conformance references.
- `AgentRegistry` remains responsible for autonomous behavior execution.
- This memory architecture provides the **state substrate** and **transfer plumbing** they use.

---

## 4. Data Model Boundaries

### 4.1 STM Record Categories (Conceptual)

```rust
enum StmRecordKind {
    WorkingNote,
    TaskState,
    RetrievedContext,
    PendingEval,
    TemporaryDatasetSlice,
    EctoplasmBuffer,
    DraftEngram,
}
```

### 4.2 LTM Record Categories (Conceptual)

```rust
enum LtmRecordKind {
    EngramMemory,
    TransferProfile,      // Engram bundle
    AdapterManifest,
    AdapterDatasetProfile,
    AdapterEvalProfile,
    ArchetypeProfile,
    CommunityReport,
    BenchmarkReceipt,
    PortabilityReport,
}
```

### 4.3 Memory Identity and Provenance (Required)

All persisted LTM memory records should carry:
- stable ID
- record type
- creation/update timestamps
- provenance (source system/user/tool)
- version/schema version
- trust/confidence metadata
- privacy/shareability policy

---

## 5. Short-Term Memory (STM) Design

### 5.1 Behavior

STM is intended to behave like a working notebook plus runtime cache:
- auto-populated by agents/tools during active sessions
- editable by the user (not append-only)
- aggressively compacted to avoid bloat
- capable of promotion into durable memory

### 5.2 Required Features

- record editing and annotation
- TTL policies (record-class specific)
- pinning (prevent auto-eviction)
- provenance tracking
- local snapshots/checkpoints
- manual promotion action ("Promote to Engram Memory")

### 5.3 Ectoplasm in STM

Ectoplasm should land in STM first:
- raw or semi-normalized stream capture
- rate-limited / sampling-controlled
- optional summarization before persistence

Never persist raw ectoplasm indefinitely by default.

---

## 6. Long-Term Memory (LTM) Design

### 6.1 Context-Harness-Like Pattern (Adapted)

Use a context-harness-style architecture for LTM management:
- connector/ingestion layer
- normalization/chunking layer (where applicable)
- index/storage layer (SQLite/FTS + optional embeddings)
- retrieval API (local and MCP-style if desired)

But extend the schema beyond generic text chunks to include typed intelligence records (`EngramMemory`, evals, manifests, archetypes, reports).

### 6.2 Retrieval Modes

LTM retrieval should support:
- keyword search
- semantic search
- hybrid search
- faceted filtering (slot/capability/UDC/model family/privacy/trust)
- graph-aware retrieval (workspace/source provenance links)

### 6.3 Promotion Targets

Canonical promotion targets include:
- `EngramMemory` (dataset lineage, eval behavior, compatibility report)
- `TransferProfile` (assembled engram bundle)
- `ArchetypeProfile`
- `CommunityReport`

---

## 7. Extractor (Outbound)

### 7.1 Purpose

`MemoryExtractor` is the canonical outbound conversion/export boundary for intelligence memory and customization artifacts.

### 7.2 Export Types (v1)

- `LoRA` adapter weights + `AdapterManifest`
- dataset slice + dataset profile
- `TransferProfile` (`Engram`)
- `ArchetypeProfile`
- eval bundle (profiles + receipts)
- portability report
- `Ectoplasm` session capture (if supported and permitted)

### 7.3 Export Modes

- **Snapshot export**: produce a static bundle at a point in time
- **Derived export**: compute export from local state (e.g. extracted adapter from full delta)
- **Streaming export**: `EctoplasmStream` live feed to a local tool or external system

### 7.4 Export Safety / Policy

Before export, enforce:
- privacy policy checks
- license/shareability checks
- redaction rules (URLs, prompts, sensitive traces)
- provenance stamping

---

## 8. Ingestor (Inbound)

### 8.1 Purpose

`MemoryIngestor` is the canonical inbound boundary for importing external or local intelligence artifacts into STM/LTM and runtime workflows.

### 8.2 Ingest Types (v1)

- adapter + manifest
- dataset/profile bundles
- `TransferProfile` (Engram)
- `ArchetypeProfile`
- community reports / benchmark receipts
- `Ectoplasm` captures or live streams

### 8.3 Ingest Pipeline

1. **Receive** (file, Verse, local stream, HTTP, IPC)
2. **Identify** (type/schema version)
3. **Quarantine** (untrusted staging)
4. **Validate** (schema, integrity, signatures, policy)
5. **Normalize** (map to internal record forms)
6. **Index** (STM and/or LTM)
7. **Attach** (optional runtime binding, subject to conformance/compatibility)

### 8.4 Quarantine Is Required

Imported memory/customization artifacts must not be assumed trustworthy.

Quarantine checks should include:
- signature/hash verification
- schema version support
- policy compatibility (privacy/license)
- compatibility checks for adapters/engrams
- optional eval requirement before runtime enablement

---

## 9. Promotion and Hydration Flows

### 9.1 STM -> LTM Promotion

Promotion converts transient working records into durable memory.

Examples:
- summarized ectoplasm session -> `EngramMemory(eval_behavior)`
- temporary dataset slice -> `AdapterDatasetProfile` + lineage memory
- working tuning notes -> `ArchetypeProfile` draft -> finalized archetype

Promotion should be:
- explicit (manual) or rule-driven (policy)
- provenance-preserving
- reversible (with tombstones/receipts if deleted)

### 9.2 LTM -> STM Hydration

Hydration loads selected durable memories into active working context.

Examples:
- hydrate a `TransferProfile` for upgrade/reuse
- hydrate benchmark receipts and community reports before model selection
- hydrate archetype + prior evals before new adapter tuning

### 9.3 Ectoplasm -> Memory Derivation

If ectoplasm is available:
- buffer in STM
- summarize/derive structured observations
- store derived records as `EngramMemory` or reports in LTM
- optionally discard raw stream after retention window

---

## 10. Evidence and Community Reports in Memory Architecture

This memory architecture is the storage/processing layer that makes the Model Index Verse evidence idea practical.

### 10.1 Evidence Sources (Stored as Memories or Linked Records)

- requirements-driven eval results
- benchmark receipts
- structured community reports
- regression receipts
- portability verification results
- archetype tuning outcomes

### 10.2 Why This Belongs in LTM

Diet/extractability/archetype guidance requires accumulated evidence over time. LTM is the natural home for that evidence because it supports:
- indexing and retrieval
- provenance and trust metadata
- repeatable reuse across upgrades and devices

---

## 11. Interfaces (Conceptual)

```rust
trait MemoryExtractor {
    fn export_snapshot(&self, req: ExportRequest) -> Result<ExportBundle>;
    fn export_stream(&self, req: StreamExportRequest) -> Result<ExportStreamHandle>; // ectoplasm
}

trait MemoryIngestor {
    fn ingest(&mut self, source: IngestSource) -> Result<IngestReceipt>;
    fn ingest_stream(&mut self, source: StreamIngestSource) -> Result<IngestStreamHandle>; // ectoplasm
}

trait MemoryPromotionPolicy {
    fn should_promote(&self, stm_record: &StmRecord) -> PromotionDecision;
    fn promote(&self, stm_record: StmRecord) -> Result<Vec<LtmRecord>>;
}
```

---

## 12. Security, Privacy, and Trust

### 12.1 Default Privacy Posture

- STM defaults to local-only
- LTM records are local by default unless explicitly marked shareable
- Ectoplasm export is opt-in and capability-gated

### 12.2 Sensitive Content Classes

Special handling may be required for:
- raw prompts and prompt histories
- proprietary documents in dataset lineage
- activation/internal traces (`Ectoplasm`)
- user identifiers and workspace references

### 12.3 Trust Metadata

Every imported/promoted memory should support trust/confidence markers such as:
- self-authored
- trusted peer
- signed community maintainer
- unsigned/unverified
- machine-derived (low confidence until reviewed)

---

## 13. Scope Split with the Model Slots Plan

To keep the design set maintainable:

This document owns:
- STM/LTM stores
- extractor/ingestor boundaries
- promotion/hydration flows
- ectoplasm runtime streaming concepts
- memory indexing/retrieval patterns

The model slots plan owns:
- slot definitions
- capability declarations
- conformance thresholds
- adapter compatibility/portability rules
- archetype semantics (as customization targets)
- engram bundle composition semantics

Cross-link policy:
- memory architecture may store and transport model-slot artifacts
- model-slot systems should reference this document for persistence/ingest/export mechanics

---

## 14. Minimal Implementation Sequence (Recommended)

### Phase M1 — STM Foundation

1. Implement `ShortTermMemoryStore` with editable records and TTL support
2. Add STM record categories for notes/task state/pending evals/draft engrams
3. Add manual promotion UI actions (draft -> durable)

### Phase M2 — LTM Index Foundation

1. Implement `LongTermMemoryIndex` (keyword + faceted search)
2. Add typed record support for `EngramMemory`, `TransferProfile`, `ArchetypeProfile`, reports
3. Add provenance/privacy/trust metadata fields

### Phase M3 — Extractor/Ingestor

1. Implement `MemoryExtractor` snapshot export for engrams/archetypes/eval bundles
2. Implement `MemoryIngestor` quarantine + validation pipeline
3. Add Verse transport hooks for imports/exports

### Phase M4 — Ectoplasm (Optional)

1. Define `EctoplasmCapabilities` + `EctoplasmPolicy`
2. Add provider capability declaration path for ectoplasm support
3. Add `EctoplasmStream` buffering into STM and derived memory promotion

### Phase M5 — Context-Harness-Style Retrieval Integration

1. Add hybrid search / connector-style ingestion for LTM
2. Expose local retrieval API (and optional MCP-compatible surface)
3. Link LTM retrieval to agent context assembly workflows

---

## 15. Open Questions

1. Should STM and LTM use the same storage engine with separate tables/policies, or separate stores entirely?
2. What minimum metadata is required for a valid `EngramMemory` promotion?
3. How much raw ectoplasm (if any) is retained by default before summarization/deletion?
4. Do we permit remote live ectoplasm ingestion, or only local loopback in v1?
5. Which LTM retrieval APIs are first-class (in-process only, local HTTP, MCP, Verse queries)?

---

## 16. Non-Goals (v1)

- Full cognitive architecture / AGI-style memory theory
- Universal introspection protocol across all model providers
- Long-term storage economics/token incentives for the Model Index Verse
- Automatic privacy-safe sharing of all promoted memories

---

## 17. Summary

This plan defines a dedicated intelligence memory architecture for Graphshell/Verse:
- **STM** for editable, high-churn working memory
- **LTM** for durable, indexed `EngramMemory` and related records
- **Extractor/Ingestor** boundaries for moving LoRA/dataset/engram/ectoplasm artifacts in and out
- **Promotion/Hydration** flows between working memory and durable memory
- **Ectoplasm** as an optional runtime signal stream that can be observed and transformed, but is not the same as persisted memory

It complements (rather than replaces) the model slots/adapters plan by providing the persistence and transfer mechanics that make engrams, archetypes, and evidence-backed customization practical.
