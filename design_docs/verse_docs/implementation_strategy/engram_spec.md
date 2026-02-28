# Engram Specification (TransferProfile v1)

**Date**: 2026-02-28
**Status**: Proposed (canonical schema draft)
**Scope**: Defines the canonical `Engram` transport object for local model customization and Verse FLora exchange. This spec standardizes the `TransferProfile` envelope, `EngramMemory` inventory, validation classes, redaction rules, and merge/provenance expectations.
**Related**:
- `design_docs/verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `design_docs/verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`
- `design_docs/verse_docs/technical_architecture/VERSE_AS_NETWORK.md`

---

## 1. Purpose

An **Engram** is the canonical, portable customization payload used by:
- local Graphshell model workflows
- local/private Verse nodes
- Verse FLora community exchanges

An Engram is not just a LoRA file. It is an envelope (`TransferProfile`) plus zero or more typed `EngramMemory` items. A LoRA delta/checkpoint is a primary memory for trainable adaptation, but the envelope and sibling memories carry the context required for compatibility checks, trust, ranking, payout logic, redaction, and merge policy.

---

## 2. Core Design Rules

1. **Canonical transport**
`TransferProfile` is the canonical transport object for customization exchange.

2. **Sparse is valid**
An Engram may omit many memory classes. Partial export is a first-class behavior.

3. **Weights are not enough**
If `adapter_weights` are present without compatibility and provenance context, the payload is valid but low-trust.

4. **Local-first**
Raw source data does not need to leave the local node. Engrams may carry derived summaries or redacted lineage instead.

5. **Policy-aware**
Every Engram must express privacy/shareability intent, even if the payload is local-only.

6. **Composable**
A Verse may merge, annotate, reject, quarantine, or re-emit Engrams without requiring mutation of the original payload bytes.

---

## 3. Validation Classes

Every Engram should declare its intended validation class:

```rust
enum EngramValidationClass {
    LocalPrivate,      // For local-only use; may contain richer private context
    LocalExportable,   // Safe for manual export, but not necessarily community-ready
    VerseSubmission,   // Intended for verse/FLora contribution
    VerseCheckpoint,   // Curated/accepted verse output
    ArchiveOnly,       // Historical or audit record; not intended for direct runtime use
}
```

This class does not guarantee acceptance. It declares author intent and determines which validation profile to apply.

---

## 4. Envelope Schema

The canonical envelope remains `TransferProfile`.

```rust
struct TransferProfile {
    engram_id: String,
    display_name: String,
    version: u32, // schema version for the envelope

    validation_class: EngramValidationClass,
    privacy: EngramPrivacyPolicy,
    redaction: RedactionProfile,

    engram_memories: Vec<EngramMemoryRef>,

    diet_profile_ref: Option<String>,
    extractability_profile_ref: Option<String>,
    dataset_profile_ref: Option<String>,
    eval_profile_ref: Option<String>,
    adapter_manifest_ref: Option<String>,

    portability_class: PortabilityClass,
    conformance_summary: Vec<SlotConformanceOutcome>,
    udc_profile: UdcProfileSummary,

    provenance: ProvenanceRecord,
    trust: TrustEnvelope,
}
```

### 4.1 Required Envelope Fields

Minimum required fields for a valid Engram:
- `engram_id`
- `display_name`
- `version`
- `validation_class`
- `privacy`
- `redaction`
- `engram_memories` (may be empty only for envelope-only audit stubs)
- `portability_class`
- `provenance`
- `trust`

### 4.2 Strongly Recommended Fields

These are not strictly required for all classes, but should be present for any reusable Engram:
- `adapter_manifest_ref` when adapter memory is present
- `udc_profile` when the payload represents domain adaptation
- `eval_profile_ref` for Verse submission or checkpoint use
- `dataset_profile_ref` unless explicitly redacted

---

## 5. Memory Inventory

Each referenced memory is a typed record with its own integrity and policy metadata.

```rust
struct EngramMemoryRef {
    memory_id: String,
    kind: EngramMemoryKind,
    location: MemoryLocation,
    hash: String,
    required_for_application: bool,
    redaction_state: RedactionState,
}

enum EngramMemoryKind {
    DatasetLineage,
    EvalBehavior,
    AdapterWeights,
    AdapterManifest,
    CompatibilityReport,
    DietProfile,
    ExtractabilityProfile,
    PromptBundle,
    SyntheticExamples,
    MergeLineage,
    GovernanceReceipt,
    ContributorAttestation,
}
```

### 5.0 Memory Location (v1)

`MemoryLocation` is explicit in v1. It distinguishes embedded bytes from externally addressed or local-only references.

```rust
enum MemoryLocation {
    Embedded {
        media_type: String,
        byte_len: u64,
    },
    ContentAddressed {
        cid: Cid,
        media_type: String,
        byte_len: u64,
    },
    LocalOnlyRef {
        local_ref: String,
        media_type: String,
    },
}
```

Rules:
- `Embedded` is preferred for small metadata-oriented memories.
- `ContentAddressed` is preferred for larger or shareable memories, especially `AdapterWeights`.
- `LocalOnlyRef` is valid only for `LocalPrivate` or `ArchiveOnly` exports and must not appear in `VerseSubmission`.

Recommended guidance:
- small metadata: embed directly
- large binaries or reusable bundles: content-address by CID

This resolves the v1 storage boundary while preserving local-only workflows.

### 5.1 Memory Classes

- `DatasetLineage`: provenance of the data sources or their redacted summaries.
- `EvalBehavior`: performance deltas, benchmark slices, regression flags.
- `AdapterWeights`: LoRA delta, checkpoint, or equivalent trainable parameter-efficient artifact.
- `AdapterManifest`: compatibility and adapter-method declaration.
- `CompatibilityReport`: explicit fit/fail reasoning against base models or slots.
- `DietProfile`: evidence-backed adaptation tendencies for a model family/checkpoint.
- `ExtractabilityProfile`: what can be recovered or exported from the source model.
- `PromptBundle`: optional prompts, templates, or control hints.
- `SyntheticExamples`: optional distilled examples used for eval or replay.
- `MergeLineage`: ancestry and merge history when the payload is derived from prior Engrams.
- `GovernanceReceipt`: moderation, payout, approval, or checkpoint inclusion receipts.
- `ContributorAttestation`: optional contributor identity/reputation claims or signatures.

### 5.2 Minimum Memory Requirements by Use

`LocalPrivate`:
- At least one memory of any kind.

`LocalExportable`:
- At least one reusable memory plus `provenance`.

`VerseSubmission`:
- At least one of:
  - `AdapterWeights`
  - `EvalBehavior`
  - `CompatibilityReport`
- Plus enough metadata to make moderation meaningful.

`VerseCheckpoint`:
- Must include either `AdapterWeights` or a stable pointer to accepted upstream `AdapterWeights`
- Should include `MergeLineage` and `GovernanceReceipt`

---

## 6. Privacy and Redaction

Engrams are explicitly designed to support selective disclosure.

```rust
enum EngramPrivacyPolicy {
    PrivateOnly,
    TrustedPeersOnly,
    VerseCommunityScoped,
    PublicPortable,
}

struct RedactionProfile {
    mode: RedactionMode,
    hidden_memory_kinds: Vec<EngramMemoryKind>,
    summary_only_memory_kinds: Vec<EngramMemoryKind>,
    notes: Option<String>,
}

enum RedactionMode {
    None,
    MetadataOnly,
    SummaryOnly,
    PointerOnly,
}
```

### 6.1 Redaction Rules

- `DatasetLineage` may be replaced with aggregate counts, UDC histograms, or source-class summaries.
- `AdapterWeights` may be omitted entirely while preserving eval/provenance context.
- `PromptBundle` should default to redacted unless explicitly shareable.
- `ContributorAttestation` may be anonymous, pseudonymous, or signed.

Redaction must never silently remove required compatibility constraints if `AdapterWeights` remain present.

---

## 7. UDC Characterization

Every domain-oriented Engram should carry a compact UDC summary, even when full dataset lineage is withheld.

```rust
struct UdcProfileSummary {
    exact_codes: Vec<String>,
    rolled_up_codes: Vec<String>,
    dominant_codes: Vec<String>,
    confidence: ConfidenceLevel,
}
```

### 7.1 UDC Use in Verse

Verse nodes may use `udc_profile` to:
- route submissions to relevant FLora pipelines
- weight moderation/review queues
- compute contribution relevance for bounty programs
- filter or search historical Engrams
- guide merge policies between similar or conflicting domain adapters

---

## 8. Classification and Compatibility

Engrams and models need an explicit compatibility language so the runtime can answer:
- what this engram contains
- what this model can consume
- which transformations are suitable for this pairing

### 8.1 Engram Content Classification

Not all engram memories serve the same role.

Critical distinction:
- `AdapterWeights` change model behavior directly.
- Hashes and fingerprints do **not** replace adapter weights. They support identity, deduplication, similarity, provenance, and routing.
- Embeddings, symbolic facts, and summaries may be useful model inputs or retrieval assets, but they are not interchangeable with trainable adapter deltas.

Recommended engram content classification:

```rust
enum EngramPayloadClass {
    Adapter,        // trainable parameter-efficient deltas (LoRA/DoRA/IA3/etc.)
    Retrieval,      // embeddings, vector memories, retrieval-oriented indices
    Symbolic,       // structured facts, triples, schemas, typed semantic records
    Evaluation,     // eval traces, benchmark slices, conformance outputs
    Provenance,     // lineage, attestations, signatures, governance receipts
    Hybrid,         // multiple classes with no single dominant role
}

enum EngramDerivationType {
    AdapterWeights,
    SoftPrompt,
    EmbeddingVector,
    HashFingerprint,
    PerceptualHash,
    LocalitySensitiveHash,
    StructuredFact,
    SchemaRecord,
    DerivedSummary,
    EvalMetric,
}
```

### 8.2 Model Diet and Consumption Requirements

Models should be classified by what kinds of engram-derived material they can consume effectively.

```rust
enum ModelDietKind {
    AdapterTunable,      // expects parameter-efficient deltas
    RetrievalAugmented,  // prefers embeddings / external memory
    SymbolicAugmented,   // prefers structured facts / schemas / tools
    PromptConditioned,   // uses soft prompts or prompt bundles
    MultiDiet,           // can combine multiple classes
}

struct ModelConsumptionProfile {
    accepted_derivations: Vec<EngramDerivationType>,
    preferred_diets: Vec<ModelDietKind>,
    required_compatibility_refs: Vec<String>,
}
```

This gives the runtime and Verse communities a concrete answer to "what needs what diet."

### 8.3 Compatibility Rules

- `HashFingerprint`, `PerceptualHash`, and `LocalitySensitiveHash` are useful for retrieval, dedup, and provenance; they are not direct substitutes for `AdapterWeights`.
- `EmbeddingVector` memories can support retrieval-oriented or semantic-indexing models but do not, by themselves, replace trainable adapters.
- `StructuredFact` and `SchemaRecord` memories are most useful for symbolic, tool-augmented, or retrieval-augmented systems.
- `DerivedSummary` memories may be suitable as compressed evidence, review context, or future adaptation input, but they are not guaranteed to be runtime-loadable artifacts.
- A model or slot should declare the derivation types it accepts before the runtime attempts to attach or route an engram.

### 8.4 Derived-Only Sharing Default

For Verse exchange, the default expectation is:
- local raw data is used privately for extraction and tuning
- Verse-facing engrams contain derived artifacts and metadata
- raw clips, screenshots, snippets, and private corpora are not shared by default

This keeps the transport aligned with local-first privacy and reduces copyright/privacy risk.

---

## 9. Trust and Provenance

Every Engram must carry enough trust metadata to explain where it came from and how much confidence to assign it.

```rust
struct TrustEnvelope {
    trust_level: TrustLevel,
    signatures: Vec<SignatureRef>,
    evidence_refs: Vec<String>,
    moderation_state: ModerationState,
}

enum TrustLevel {
    SelfAsserted,
    PeerAttested,
    CommunityReviewed,
    CheckpointAccepted,
}

enum ModerationState {
    Unreviewed,
    Quarantined,
    Accepted,
    Rejected,
    Superseded,
}
```

### 8.1 Provenance Requirements

`ProvenanceRecord` should be able to answer:
- who or what produced this payload
- which upstream engrams or datasets informed it
- what tooling version produced it
- whether it was merged, derived, or directly emitted

For Verse use, provenance should also support payout and governance auditing.

---

## 10. Merge Semantics

FLora communities and local users both need deterministic merge rules.

```rust
struct EngramMergeRecord {
    merge_id: String,
    inputs: Vec<String>,   // engram_ids
    output: String,        // engram_id
    strategy: MergeStrategy,
    operator: MergeOperator,
}

enum MergeStrategy {
    KeepSeparate,
    LinearCheckpoint,
    WeightedMerge,
    DomainSelectiveMerge,
    MetadataOnlyAggregation,
}

enum MergeOperator {
    LocalUser,
    TrustedPeer,
    VerseModerator,
    AutomatedPolicy,
}
```

### 9.1 Merge Rules

- The original source Engrams remain immutable.
- Merged outputs create new `engram_id` values.
- `MergeLineage` should record input ancestry.
- Weighted or selective merges should reference the policy or evaluation basis used.
- A Verse may merge metadata without merging weights.

---

## 11. Verse Submission Profile

For FLora, the envelope is the same, but verses evaluate against a stricter submission profile.

### 10.1 Recommended Minimum for `VerseSubmission`

- Envelope with `validation_class = VerseSubmission`
- `udc_profile`
- `trust`
- `provenance`
- One or more of:
  - `AdapterWeights`
  - `EvalBehavior`
  - `CompatibilityReport`
- Enough non-weight context to let the verse rank or reject the payload

### 10.2 Verse Acceptance Outcomes

```rust
enum VerseSubmissionOutcome {
    Accepted,
    AcceptedNeedsMoreContext,
    QuarantinedForReview,
    RejectedInsufficientEvidence,
    RejectedIncompatible,
}
```

Verses should be allowed to:
- accept sparse payloads but assign lower trust or lower payout
- require richer payloads for payout eligibility
- retain rejected submissions as local audit records if policy allows

### 11.3 Low-Risk Verse Submission Guidance

Preferred low-risk submission materials:
- hashes and fingerprints
- DOM fingerprints and structural signatures
- extracted facts and typed metadata
- UDC classification
- eval bundles
- adapter weights
- derived summaries

Higher-risk materials such as raw clips, screenshots, snippets, or private notes should remain local by default unless the contributor explicitly publishes them under a policy that permits that risk.

---

## 12. Local Runtime Application

Local model application does not need the full payload at runtime.

### 11.1 Runtime-Minimal Application Set

For direct model application, the practical minimum is usually:
- `AdapterWeights`
- `AdapterManifest`
- enough compatibility information to verify safe attachment

Everything else is still valuable:
- `EvalBehavior` informs feature gating
- `DatasetLineage` and `UdcProfileSummary` explain what the adaptation is for
- `MergeLineage` explains how the current state was assembled

The runtime may therefore apply only a subset of memories while retaining the whole Engram in storage.

---

## 13. Versioning and Compatibility

### 12.1 Schema Versioning

- `TransferProfile.version` versions the envelope schema.
- Memory-specific formats should version themselves independently.
- Unknown memory kinds must be ignored, not treated as fatal, unless they are marked `required_for_application`.

### 12.2 Forward Compatibility Rule

Receivers should:
- preserve unknown memories when re-exporting, if possible
- reject only when missing a required memory for the requested operation
- separate storage validity from runtime applicability

This keeps the format extensible without forcing every node to understand every future memory class.

---

## 14. Open Questions

1. Should Verse payout policy ever be represented directly in the engram, or remain strictly verse-local except for emitted receipts?
2. Should `VerseCheckpoint` require signed moderation receipts from multiple reviewers for higher-trust communities by default, or only when community policy explicitly demands it?

---

## 15. Immediate Implementation Guidance

Short-term practical rules:
- Treat `TransferProfile` as the stable top-level envelope name.
- Keep `Engram` as the design term shown to users and in architecture docs.
- Default all local exports to `LocalPrivate` or `LocalExportable`.
- Require explicit promotion to `VerseSubmission`.
- Use explicit `MemoryLocation` classes: `Embedded`, `ContentAddressed`, and `LocalOnlyRef`.
- For FLora, accept sparse submissions, but make payout and merge eligibility depend on richer context.

This keeps the spec aligned with local-first privacy while giving Verse communities enough structure to coordinate adaptation and compensation safely.
