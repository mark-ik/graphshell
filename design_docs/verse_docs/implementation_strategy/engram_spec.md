# Engram Specification (TransferProfile v1)

**Date**: 2026-02-28
**Status**: Proposed (canonical schema draft)
**Scope**: Defines the canonical `Engram` transport object for local model customization and Verse FLora exchange. This spec standardizes the `TransferProfile` envelope, `EngramMemory` inventory, validation classes, redaction rules, ranking-policy artifacts, and merge/provenance expectations.
**Related**:

- `design_docs/verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `design_docs/verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `design_docs/verse_docs/implementation_strategy/lineage_dag_spec.md`
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`
- `design_docs/verse_docs/technical_architecture/VERSE_AS_NETWORK.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.1, 3.3, 3.10, 3.11, 3.12)):
- **IPFS CIDv1** — `EngramMemoryRef.hash` is a `Cid` (CIDv1, BLAKE3), not a plain `String`; `ContentAddressed.cid` is CIDv1
- **RFC 4122 UUID v4** — `engram_id` and `memory_id` are UUID v4 stable identifiers
- **W3C DID Core 1.0** — contributor identity in `ContributorAttestation` uses `did:key`
- **W3C VC Data Model 2.0** — `GovernanceReceipt` and `ContributorAttestation` are Verifiable Credential envelopes

---

## 1. Purpose

An **Engram** is the canonical, portable customization payload used by:

- local Graphshell model workflows
- local/private Verse nodes
- Verse FLora community exchanges

An Engram is not just a LoRA file. It is an envelope (`TransferProfile`) plus zero or more typed `EngramMemory` items. A LoRA delta/checkpoint is a primary memory for trainable adaptation, but the envelope and sibling memories carry the context required for compatibility checks, trust, ranking, payout logic, redaction, merge policy, and portable discovery behavior.

In FLora terms, an Engram may carry not only trainable adapters but also the artifacts that make a verse's ranking policy legible and portable: source preferences, topic affinities, trust modifiers, embeddings, compact statistical parameters, and evaluation context.

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

7. **Inspectable ranking surfaces**
If an Engram materially affects discovery or ranking, it should carry enough policy context for another party to review what it is trying to boost, suppress, or prioritize.

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
    engram_id: Uuid,     // UUID v4 (RFC 4122) — stable content identity
    display_name: String,
    version: u32,        // schema version for the envelope

    validation_class: EngramValidationClass,
    privacy: EngramPrivacyPolicy,
    redaction: RedactionProfile,

    engram_memories: Vec<EngramMemoryRef>,

    diet_profile_ref: Option<String>,
    extractability_profile_ref: Option<String>,
    dataset_profile_ref: Option<String>,
    eval_profile_ref: Option<String>,
    adapter_manifest_ref: Option<String>,
    ranking_policy_ref: Option<String>,
    embedding_profile_ref: Option<String>,

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
- `ranking_policy_ref` when the payload materially changes discovery or ranking behavior
- `embedding_profile_ref` when embeddings or other retrieval-oriented learned artifacts are present
- `udc_profile` when the payload represents domain adaptation
- `eval_profile_ref` for Verse submission or checkpoint use
- `dataset_profile_ref` unless explicitly redacted

---

## 5. Memory Inventory

Each referenced memory is a typed record with its own integrity and policy metadata.

```rust
struct EngramMemoryRef {
    memory_id: Uuid,   // UUID v4 (RFC 4122)
    kind: EngramMemoryKind,
    location: MemoryLocation,
    hash: Cid,         // CIDv1, BLAKE3 (IPFS CIDv1 adopted standard)
    required_for_application: bool,
    redaction_state: RedactionState,
}

enum EngramMemoryKind {
    DatasetLineage,
    EvalBehavior,
    AdapterWeights,
    AdapterManifest,
    RankingPolicy,
    TrustPolicy,
    EmbeddingProfile,
    FeatureCalibration,
    PolicyDiff,
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
- `RankingPolicy`: declarative ranking parameters such as source weights, topic affinities, recency/depth preferences, or feature-weight manifests.
- `TrustPolicy`: explicit trust modifiers, trusted-domain preferences, suppressions, or contributor-weighting rules.
- `EmbeddingProfile`: embeddings or related retrieval/clustering artifacts intended to shape search, grouping, or recommendation behavior.
- `FeatureCalibration`: compact statistical parameters, thresholds, or scoring calibrations that influence ranking behavior without requiring a full adapter.
- `PolicyDiff`: canonical machine-readable comparison artifact describing how a ranking or trust policy changed between two versions.
- `CompatibilityReport`: explicit fit/fail reasoning against base models or slots.
- `DietProfile`: evidence-backed adaptation tendencies for a model family/checkpoint.
- `ExtractabilityProfile`: what can be recovered or exported from the source model.
- `PromptBundle`: optional prompts, templates, or control hints.
- `SyntheticExamples`: optional distilled examples used for eval or replay.
- `MergeLineage`: ancestry and merge history when the payload is derived from prior Engrams.
- `GovernanceReceipt`: moderation, payout, approval, or checkpoint inclusion receipts.
- `ContributorAttestation`: optional contributor identity/reputation claims or signatures.

### 5.1A Policy Artifact Schemas (v1)

FLora policy artifacts should prefer compact, reviewable structures before opaque learned components.
For v1, communities should treat `RankingPolicy`, `TrustPolicy`, `EmbeddingProfile`, and `FeatureCalibration` as distinct memory families rather than collapsing them into one generic blob.

#### Ranking Policy

Use `RankingPolicy` for declarative discovery behavior: source weights, topic affinities, recency/depth tradeoffs, diversity boosts, and similar heuristics.

```rust
struct RankingPolicyMemory {
        policy_id: Uuid,
        display_name: String,
        objective: String,
        feature_weights: Vec<RankingFeatureWeight>,
        blend_strategy: RankingBlendStrategy,
        explanation_summary: Option<String>,
        eval_refs: Vec<String>,
}

struct RankingFeatureWeight {
        feature: RankingFeature,
        weight: f32,
        clamp_min: Option<f32>,
        clamp_max: Option<f32>,
        notes: Option<String>,
}

enum RankingFeature {
        SourceAuthority,
        TopicAffinity,
        GraphProximity,
        Recency,
        Novelty,
        Depth,
        Diversity,
        CommunityPreference,
}

enum RankingBlendStrategy {
        Linear,
        WeightedCascade,
        ThresholdGate,
}
```

Example heuristic-oriented payload:

```json
{
    "display_name": "primary-sources-flora",
    "objective": "Prefer primary reporting and archival sources for civic discovery.",
    "feature_weights": [
        { "feature": "SourceAuthority", "weight": 1.4 },
        { "feature": "Depth", "weight": 0.7 },
        { "feature": "Novelty", "weight": 0.2 },
        { "feature": "CommunityPreference", "weight": 0.8, "notes": "boost public-interest reporting" }
    ],
    "blend_strategy": "WeightedCascade",
    "explanation_summary": "Primary sources and high-context reporting outrank fast-twitch reposts."
}
```

#### Trust Policy

Use `TrustPolicy` for explicit trust modifiers, suppressions, or weighting rules tied to domains, contributor classes, or attested relationships.

```rust
struct TrustPolicyMemory {
        policy_id: Uuid,
        default_trust_score: f32,
        domain_rules: Vec<DomainTrustRule>,
        contributor_rules: Vec<ContributorTrustRule>,
        suppression_rules: Vec<SuppressionRule>,
        explanation_summary: Option<String>,
}

struct DomainTrustRule {
        domain_pattern: String,
        score_delta: f32,
        rationale: Option<String>,
}

struct ContributorTrustRule {
        attestation_kind: String,
        score_delta: f32,
        rationale: Option<String>,
}

struct SuppressionRule {
        label: String,
        applies_to: String,
        action: SuppressionAction,
        rationale: Option<String>,
}

enum SuppressionAction {
        Downrank,
        HideByDefault,
        RequireContext,
}
```

Example trust-modifier payload:

```json
{
    "default_trust_score": 0.0,
    "domain_rules": [
        {
            "domain_pattern": "*.gov",
            "score_delta": 1.1,
            "rationale": "Primary institutional sources for public records."
        },
        {
            "domain_pattern": "*.ragebait.example",
            "score_delta": -1.6,
            "rationale": "Known outrage-optimization behavior."
        }
    ],
    "suppression_rules": [
        {
            "label": "decontextualized-clip",
            "applies_to": "media excerpts without provenance",
            "action": "RequireContext"
        }
    ],
    "explanation_summary": "Boost public-record domains and require context for clipped material."
}
```

#### Embedding Profile

Use `EmbeddingProfile` when the artifact affects retrieval, clustering, or label suggestion via vector representations rather than direct adapter tuning.

```rust
struct EmbeddingProfileMemory {
        profile_id: Uuid,
        display_name: String,
        model_family: String,
        vector_dim: u32,
        intended_uses: Vec<EmbeddingUse>,
        training_scope_summary: Option<String>,
        privacy_notes: Option<String>,
        eval_refs: Vec<String>,
}

enum EmbeddingUse {
        Retrieval,
        Clustering,
        LabelSuggestion,
        SimilaritySearch,
}
```

Example embedding-oriented payload:

```json
{
    "display_name": "civics-cluster-embed-v1",
    "model_family": "bge-small-en-community",
    "vector_dim": 768,
    "intended_uses": ["Clustering", "LabelSuggestion", "SimilaritySearch"],
    "training_scope_summary": "Derived from accepted public-interest civic corpus summaries and checkpoint lineage.",
    "privacy_notes": "No per-user browsing sequence should be recoverable from published vectors.",
    "eval_refs": ["cid:cluster-eval-001"]
}
```

#### Feature Calibration

Use `FeatureCalibration` for compact thresholds, score transforms, or feature normalizations that shape ranking behavior without shipping a full adapter.

```rust
struct FeatureCalibrationMemory {
        calibration_id: Uuid,
        feature_name: String,
        transform: CalibrationTransform,
        parameters: BTreeMap<String, f32>,
        notes: Option<String>,
}

enum CalibrationTransform {
        LinearScale,
        Sigmoid,
        Piecewise,
        QuantileBucket,
}
```

These artifacts are intentionally separable.
A verse might checkpoint only heuristics, or heuristics plus trust modifiers, or embeddings plus calibration, without requiring `AdapterWeights` at all.

### 5.1B Canonical Policy Diff Artifacts (v1)

When a verse updates a `RankingPolicy` or `TrustPolicy`, the preferred comparison format is a separate machine-readable `PolicyDiff` artifact rather than a prose-only changelog.

```rust
struct PolicyDiffMemory {
    diff_id: Uuid,
    kind: PolicyDiffKind,
    from_memory_ref: Option<Cid>,
    to_memory_ref: Cid,
    generated_at_ms: u64,
    generated_by: DiffOperator,
    summary: String,
    expected_effects: Vec<ExpectedEffect>,
    change_count: u32,
}

enum PolicyDiffKind {
    RankingPolicy,
    TrustPolicy,
}

enum DiffOperator {
    LocalTool,
    VerseReviewer,
    VerseModerator,
    AutomatedPolicy,
}

struct ExpectedEffect {
    surface: DiscoverySurface,
    direction: EffectDirection,
    description: String,
    confidence: ConfidenceLevel,
}

enum DiscoverySurface {
    FeedRanking,
    SearchRanking,
    ClusterFormation,
    LabelSuggestion,
    SourcePrioritization,
}

enum EffectDirection {
    Broaden,
    Narrow,
    Rebalance,
    Suppress,
    Boost,
    Neutral,
}
```

#### Ranking Policy Diff

```rust
struct RankingPolicyDiffBody {
    objective_change: Option<TextValueChange>,
    blend_strategy_change: Option<EnumValueChange<RankingBlendStrategy>>,
    feature_weight_changes: Vec<FeatureWeightChange>,
    added_features: Vec<RankingFeatureWeight>,
    removed_features: Vec<RankingFeatureWeight>,
    eval_ref_changes: Vec<SetMembershipChange<String>>,
}

struct FeatureWeightChange {
    feature: RankingFeature,
    old_weight: f32,
    new_weight: f32,
    old_clamp_min: Option<f32>,
    new_clamp_min: Option<f32>,
    old_clamp_max: Option<f32>,
    new_clamp_max: Option<f32>,
    notes: Option<String>,
}
```

#### Trust Policy Diff

```rust
struct TrustPolicyDiffBody {
    default_trust_score_change: Option<ScalarValueChange>,
    domain_rule_changes: Vec<DomainRuleChange>,
    contributor_rule_changes: Vec<ContributorRuleChange>,
    suppression_rule_changes: Vec<SuppressionRuleChange>,
}

enum RuleChangeKind {
    Added,
    Removed,
    Modified,
}

struct DomainRuleChange {
    rule_key: String,
    change_kind: RuleChangeKind,
    old_score_delta: Option<f32>,
    new_score_delta: Option<f32>,
    rationale: Option<String>,
}

struct ContributorRuleChange {
    rule_key: String,
    change_kind: RuleChangeKind,
    old_score_delta: Option<f32>,
    new_score_delta: Option<f32>,
    rationale: Option<String>,
}

struct SuppressionRuleChange {
    rule_key: String,
    change_kind: RuleChangeKind,
    old_action: Option<SuppressionAction>,
    new_action: Option<SuppressionAction>,
    rationale: Option<String>,
}
```

Common helper shapes:

```rust
struct TextValueChange {
    old_value: Option<String>,
    new_value: Option<String>,
}

struct ScalarValueChange {
    old_value: f32,
    new_value: f32,
}

struct EnumValueChange<T> {
    old_value: T,
    new_value: T,
}

struct SetMembershipChange<T> {
    value: T,
    change_kind: RuleChangeKind,
}
```

Example ranking-policy diff artifact:

```json
{
  "diff_id": "7f8d2684-d10d-4d91-aecf-88269248425f",
  "kind": "RankingPolicy",
  "from_memory_ref": "bafyrei-rankingpolicy-v2",
  "to_memory_ref": "bafyrei-rankingpolicy-v3",
  "generated_at_ms": 1775337600000,
  "generated_by": "VerseReviewer",
  "summary": "Boosts source authority and depth while reducing novelty bias.",
  "expected_effects": [
    {
      "surface": "SearchRanking",
      "direction": "Rebalance",
      "description": "Long-form primary reporting should appear earlier in results.",
      "confidence": "High"
    }
  ],
  "change_count": 3,
  "body": {
    "blend_strategy_change": {
      "old_value": "Linear",
      "new_value": "WeightedCascade"
    },
    "feature_weight_changes": [
      {
    "feature": "SourceAuthority",
    "old_weight": 1.0,
    "new_weight": 1.4,
    "old_clamp_min": null,
    "new_clamp_min": null,
    "old_clamp_max": null,
    "new_clamp_max": null
      },
      {
    "feature": "Novelty",
    "old_weight": 0.6,
    "new_weight": 0.2,
    "old_clamp_min": null,
    "new_clamp_min": null,
    "old_clamp_max": null,
    "new_clamp_max": null
      }
    ],
    "added_features": [
      {
    "feature": "Depth",
    "weight": 0.7,
    "clamp_min": null,
    "clamp_max": null,
    "notes": "Prefer high-context explainers."
      }
    ],
    "removed_features": [],
    "eval_ref_changes": []
  }
}
```

Canonical expectations:
- diffs should identify the exact prior and next policy artifact by CID
- diffs should express semantic changes at the feature or rule level, not only byte-level replacement
- diffs should include expected effects so reviewers can compare intent to observed behavior
- diffs should be content-addressed and publishable alongside checkpoint review receipts

### 5.2 Minimum Memory Requirements by Use

`LocalPrivate`:
- At least one memory of any kind.

`LocalExportable`:
- At least one reusable memory plus `provenance`.

`VerseSubmission`:
- At least one of:
  - `AdapterWeights`
  - `EvalBehavior`
    - `RankingPolicy`
    - `TrustPolicy`
    - `EmbeddingProfile`
    - `FeatureCalibration`
  - `CompatibilityReport`
- Plus enough metadata to make moderation meaningful.

`VerseCheckpoint`:
- Must include either `AdapterWeights`, `RankingPolicy`, `TrustPolicy`, `EmbeddingProfile`, `FeatureCalibration`, or a stable pointer to accepted upstream artifacts of those kinds
- Should include `PolicyDiff` when ranking or trust policy changes materially from the prior accepted checkpoint
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
- `RankingPolicy`, `TrustPolicy`, `EmbeddingProfile`, and `FeatureCalibration` should be treated as potentially sensitive if they were derived from personal behavioral traces.
- `PromptBundle` should default to redacted unless explicitly shareable.
- `ContributorAttestation` may be anonymous, pseudonymous, or signed.

Redaction must never silently remove required compatibility constraints if `AdapterWeights` remain present.
Redaction should also avoid publishing ranking artifacts whose granularity would expose private user habits more directly than the verse's privacy policy allows.

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

### 10.2 Example Hybrid Verse Submission

The following example shows a single `TransferProfile` carrying the kinds of policy artifacts a verse could checkpoint without requiring a pure adapter-only interpretation.

```json
{
    "engram_id": "6d4a2c2a-38c2-4d59-9c8d-76f46d5ef2c1",
    "display_name": "civics-flora-v3",
    "version": 1,
    "validation_class": "VerseSubmission",
    "privacy": "VerseCommunityScoped",
    "redaction": {
        "mode": "SummaryOnly",
        "hidden_memory_kinds": ["PromptBundle"],
        "summary_only_memory_kinds": ["DatasetLineage"],
        "notes": "Derived-only release for civic discovery verse."
    },
    "engram_memories": [
        {
            "memory_id": "b2b0f604-8417-49d3-9cb1-d6a4824bd43d",
            "kind": "RankingPolicy",
            "location": {
                "Embedded": {
                    "media_type": "application/json",
                    "byte_len": 2048
                }
            },
            "hash": "bafyrei-rankingpolicycid",
            "required_for_application": true,
            "redaction_state": "None"
        },
        {
            "memory_id": "33a6038d-aa04-4921-95ab-b9f27f684523",
            "kind": "TrustPolicy",
            "location": {
                "Embedded": {
                    "media_type": "application/json",
                    "byte_len": 1536
                }
            },
            "hash": "bafyrei-trustpolicycid",
            "required_for_application": true,
            "redaction_state": "None"
        },
        {
            "memory_id": "5d091505-4618-4555-b58c-83fd69f0ce3d",
            "kind": "EmbeddingProfile",
            "location": {
                "ContentAddressed": {
                    "cid": "bafyrei-embeddingprofilecid",
                    "media_type": "application/json",
                    "byte_len": 8192
                }
            },
            "hash": "bafyrei-embeddingprofilecid",
            "required_for_application": false,
            "redaction_state": "SummaryOnly"
        },
        {
            "memory_id": "ef7a9f60-1d72-41c7-89aa-75858a3d8d0f",
            "kind": "FeatureCalibration",
            "location": {
                "Embedded": {
                    "media_type": "application/json",
                    "byte_len": 896
                }
            },
            "hash": "bafyrei-featurecalibrationcid",
            "required_for_application": false,
            "redaction_state": "None"
        },
        {
            "memory_id": "3f0d34f9-d815-4fc1-a7c6-3ba91bf6a8d9",
            "kind": "EvalBehavior",
            "location": {
                "ContentAddressed": {
                    "cid": "bafyrei-evalbundlecid",
                    "media_type": "application/json",
                    "byte_len": 12044
                }
            },
            "hash": "bafyrei-evalbundlecid",
            "required_for_application": false,
            "redaction_state": "None"
        },
        {
            "memory_id": "a5be8336-86aa-4a96-af77-468122fe1226",
            "kind": "CompatibilityReport",
            "location": {
                "Embedded": {
                    "media_type": "application/json",
                    "byte_len": 1400
                }
            },
            "hash": "bafyrei-compatreportcid",
            "required_for_application": true,
            "redaction_state": "None"
        },
        {
            "memory_id": "1ef7ea29-10ee-4bab-85b4-f40955fb079e",
            "kind": "DatasetLineage",
            "location": {
                "Embedded": {
                    "media_type": "application/json",
                    "byte_len": 1100
                }
            },
            "hash": "bafyrei-lineagecid",
            "required_for_application": false,
            "redaction_state": "SummaryOnly"
        }
    ],
    "dataset_profile_ref": "cid:dataset-summary-civic-v3",
    "eval_profile_ref": "cid:eval-profile-civic-v3",
    "ranking_policy_ref": "cid:ranking-policy-civic-v3",
    "embedding_profile_ref": "cid:embedding-profile-civic-v3",
    "portability_class": "PortableWithReview",
    "conformance_summary": [],
    "udc_profile": {
        "exact_codes": ["32", "321", "342"],
        "rolled_up_codes": ["3"],
        "dominant_codes": ["32"],
        "confidence": "High"
    },
    "provenance": {
        "notes": "Derived from accepted public-interest civic corpus summaries and prior civics-flora checkpoints."
    },
    "trust": {
        "trust_level": "CommunityReviewed",
        "signatures": [],
        "evidence_refs": ["cid:review-receipt-001", "cid:review-receipt-002"],
        "moderation_state": "Accepted"
    }
}
```

Interpretation:
- the verse can apply the `RankingPolicy` and `TrustPolicy` directly for feed, discovery, and source prioritization
- the `EmbeddingProfile` can support clustering or label suggestion without being mandatory for every runtime
- the `FeatureCalibration` tunes score transforms without hiding the whole policy inside a larger opaque artifact
- `EvalBehavior`, `CompatibilityReport`, and `DatasetLineage` provide the evidence needed for review and checkpoint comparison

### 10.3 Verse Acceptance Outcomes

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
