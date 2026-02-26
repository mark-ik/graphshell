# Model Slots, Adapter Personalization, and UDC-Grounded Evaluation Plan

**Date**: 2026-02-26
**Status**: Proposed (design-ready)
**Scope**: Local intelligence model management, capability-slot binding, LoRA adapter personalization, UDC-grounded dataset/evaluation metadata, and multimodal model slot reuse in Graphshell/Verse.
**Related**:
- `design_docs/verse_docs/research/2026-02-24_local_intelligence_research.md`
- `design_docs/TERMINOLOGY.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md`
- `design_docs/graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md`

---

## 1. Why This Plan Exists

The local intelligence research already defines:
- a 4-model baseline stack (text, embeddings, vision, audio)
- a `ModelRegistry` concept
- capability-based feature routing
- LoRA adapter injection
- Verse distribution for model artifacts and adapters

This plan converts those ideas into a concrete architecture with:
- **slot contracts** (what the app needs)
- **model capability declarations** (what a model claims)
- **conformance/evaluation records** (what a model proves)
- **adapter manifests** (what a LoRA is compatible with)
- **UDC-grounded dataset/eval profiles** (what data shaped it and where it improves)
- **multimodal binding semantics** (one model may satisfy many slots, including single-modality use)

The goal is to make personalization portable, measurable, and feature-gatable instead of opaque.

---

## 2. Core Design Principles

1. **Capabilities, not model names**
Features bind to required capabilities (`embedding`, `text-generation`, `speech-to-text`), not specific model brands.

2. **Slots are runtime contracts**
The app exposes a small set of intelligence slots with stable interfaces. Users/models/adapters may change; slot contracts remain stable.

3. **UDC is a semantic facet, not the whole schema**
UDC classifies domain/topic. It does not replace task, modality, compatibility, safety, or quality metadata.

4. **Personalization must be measurable**
A LoRA without baseline deltas and eval metadata is an untrusted tweak, not a reliable upgrade.

5. **Multimodal models are first-class**
One model may satisfy multiple slots. The runtime must support binding only a subset of its modalities/capabilities to specific slots.

6. **Portability means lineage + evals, not just weights**
The durable asset is: dataset lineage + adapter manifest + eval profile + compatibility matrix. Adapter weights alone are insufficient.

---

## 3. Vocabulary Additions (Intelligence Layer)

These terms are consistent with `TERMINOLOGY.md` capability/conformance/degradation language.

- **Intelligence Slot**: A runtime capability socket with a stable interface contract (e.g. `slot:semantic_indexer`). A slot is not a model.
- **Model Capability Declaration**: A model's claimed support for tasks/modalities/interfaces (for example `embedding`, `text-generation`, `vision-captioning`).
- **Model Conformance**: Evaluated fitness of a model (or model+adapter bundle) for a slot/capability based on tests and metrics.
- **Adapter**: A parameter-efficient delta (e.g. LoRA) applied to a compatible base model.
- **Adapter Bundle**: Base model binding + one or more adapters + merge/composition policy.
- **Modality Projection**: A runtime binding that uses only one modality/capability subset of a multimodal model (for example STT only, vision caption only).
- **Portability Class**: Compatibility status describing whether an adapter can be reused directly, conditionally, or requires retraining.

---

## 4. Intelligence Slots (Canonical v1)

These are the canonical slots for the current local intelligence plan. They align with the existing 4-model minimal stack but do not require four distinct models.

### 4.1 Slot List

1. `slot:text_reasoner`
- **Primary capabilities**: `text-generation`, `classification`, `structured-extraction`
- **Example features**: summaries, workspace chat, edge-label suggestions, field extraction

2. `slot:semantic_indexer`
- **Primary capabilities**: `embedding`, `semantic-similarity`, `semantic-search`
- **Example features**: semantic physics, deduplication, related nodes, concept search

3. `slot:vision_perceptor`
- **Primary capabilities**: `vision-labeling`, `captioning`, optional `region-saliency`
- **Example features**: image tagging, smart icons, saliency crop

4. `slot:audio_perceptor`
- **Primary capabilities**: `speech-to-text`, optional `audio-labeling`
- **Example features**: audio indexing, transcript search

### 4.2 Slot Contract Shape (Conceptual)

```rust
struct IntelligenceSlotContract {
    slot_id: SlotId,
    required_capabilities: Vec<CapabilityId>,
    preferred_capabilities: Vec<CapabilityId>,
    interface_version: u32,
    latency_budget_ms: Option<u32>,
    memory_budget_mb: Option<u32>,
}
```

### 4.3 Slot Degradation Rules

Each slot reports:
- `full`: required capabilities available and conformance threshold met
- `partial`: capability available but below target quality/latency, or only a reduced interface supported
- `none`: no compatible binding

This mirrors the folded capability/conformance/degradation pattern used elsewhere in the architecture.

---

## 5. Model Capability Declarations (Including Multimodal)

A model declares what it can do. A slot decides whether that is enough.

### 5.1 Declaration Structure (Conceptual)

```rust
struct ModelCapabilityDeclaration {
    model_id: String,
    architecture_family: String, // qwen, llama, bert, whisper, florence, etc.
    modalities: Vec<ModalityId>, // text, vision, audio
    capabilities: Vec<CapabilityDeclaration>,
    interfaces: Vec<InterfaceDeclaration>,
    resource_profiles: Vec<ResourceProfile>,
}

struct CapabilityDeclaration {
    capability_id: CapabilityId,
    support: SupportLevel, // full | partial | none
    mode: ExecutionMode,   // local-native | local-onnx | remote-verse
    notes: Option<String>,
}
```

### 5.2 Multimodal Model Reuse (Critical Requirement)

A single multimodal model may satisfy multiple slots if it declares compatible capabilities.

Examples:
- A multimodal model with text + vision may bind both `slot:text_reasoner` and `slot:vision_perceptor`.
- A multimodal audio-text model may bind `slot:audio_perceptor` and `slot:text_reasoner`.
- A hypothetical omni model may bind all four slots.

### 5.3 Partial Modality Use (Modality Projection)

The runtime must support using only one modality path from a multimodal model.

Examples:
- Use only speech-to-text from a multimodal audio-text model for `slot:audio_perceptor`.
- Use only captioning from a vision-language model for `slot:vision_perceptor`.
- Disable text generation on the same model if quality is below threshold for `slot:text_reasoner`.

This requires per-capability conformance, not just per-model conformance.

---

## 6. Binding Model(s) to Slot(s)

### 6.1 Slot Binding Record

```rust
struct SlotBinding {
    slot_id: SlotId,
    provider: BoundProvider,
    degradation: DegradationMode,
    conformance_summary: ConformanceSummary,
}

enum BoundProvider {
    BaseModel {
        model_id: String,
        capability_subset: Vec<CapabilityId>,
    },
    ModelWithAdapters {
        model_id: String,
        adapters: Vec<AdapterAttachment>,
        composition: AdapterCompositionPolicy,
        capability_subset: Vec<CapabilityId>,
    },
    RemoteProvider {
        provider_id: String,
        capability_subset: Vec<CapabilityId>,
    },
}
```

### 6.2 Binding Rules

1. A slot may bind to any provider whose declared capabilities satisfy the slot contract.
2. One provider may bind to multiple slots.
3. A provider may bind to only a subset of its declared capabilities.
4. Feature gating uses slot conformance, not model presence alone.
5. If a model binds multiple slots, runtime resource arbitration must be explicit (load once / shared weights / concurrency limits).

### 6.3 Shared-Model Arbitration (When One Model Fills Many Slots)

If one multimodal model is bound to multiple slots:
- load weights once when possible
- expose per-slot concurrency limits
- allow per-slot priority (for example, embeddings/search may preempt chat)
- surface degradation when saturation causes latency to exceed budget

---

## 7. UDC-Grounded Personalization: Data Facets and Evaluation

## 7.1 Why UDC Applies Here

UDC is valuable for adapter personalization because it gives a structured way to describe **what domain knowledge shaped the customization**.

Examples:
- `udc:004` Computer science
- `udc:51` Mathematics
- `udc:621` Engineering

This enables:
- semantic dataset profiling
- per-domain eval deltas vs baseline
- transfer/regression analysis outside the tuned domain

## 7.2 UDC Is One Facet in a Multi-Facet Profile

Required metadata facets for personalization datasets and evaluations:
- **Domain facet**: UDC codes + distribution
- **Task facet**: summarize/extract/classify/embed/caption/transcribe
- **Modality facet**: text/vision/audio/multimodal
- **Compatibility facet**: base model family/revision/tokenizer/modules
- **Quality facet**: benchmark/eval deltas
- **Risk facet**: license, privacy, provenance, safety review state

## 7.3 Adapter Dataset Profile (Conceptual)

```rust
struct AdapterDatasetProfile {
    dataset_id: String,
    source_kind: DatasetSourceKind, // local_reports | verse_reports | curated_mix
    source_lineage: Vec<DatasetSourceRef>, // VerseBlob CIDs, local snapshots, manifests
    sample_count: u64,
    time_window: Option<TimeWindow>,
    modality_distribution: Vec<BucketStat<ModalityId>>,
    task_distribution: Vec<BucketStat<TaskId>>,
    udc_distribution: Vec<UdcBucketStat>,
    privacy_policy: PrivacyPolicy,
    licensing: Vec<LicenseRecord>,
    curation_notes: Option<String>,
}
```

### 7.4 UDC Distribution Rules (v1)

- Store both:
  - exact-code histogram (`udc:519.6`)
  - rolled-up prefix histogram by configurable depth (`udc:5`, `udc:51`, `udc:519`)
- Record unknown/unvalidated UDC tags separately (do not silently coerce).
- Preserve the `KnowledgeRegistry` validation result at export time.

---

## 8. Adapter Manifest and Compatibility

### 8.1 Adapter Manifest (LoRA-focused v1)

```rust
struct AdapterManifest {
    adapter_id: String,
    adapter_method: AdapterMethod, // lora | dora | ia3 (future)
    adapter_format_version: u32,

    base_compatibility: BaseCompatibility,
    target_modules: Vec<String>,
    rank: Option<u32>,
    alpha: Option<f32>,
    dropout: Option<f32>,

    dataset_profile_ref: String,
    eval_profile_ref: String,

    provenance: ProvenanceRecord,
    distribution: DistributionRecord,
}

struct BaseCompatibility {
    architecture_family: String,
    base_model_id: String,
    base_model_revision: Option<String>,
    base_model_hash: String,
    tokenizer_hash: Option<String>,
    tensor_shape_fingerprint: String,
    supported_quantization_modes: Vec<String>,
}
```

### 8.2 Portability Class (Adapter Reuse Status)

Canonical values:
- `exact`: same base model hash/tokenizer/modules
- `family_compatible_unverified`: same architecture family and expected modules, but different revision/weights; requires eval before use
- `projection_compatible`: only some capabilities are compatible/verified (for multimodal or partial adapter scope)
- `retrain_required`: incompatible shapes/modules/tokenizer or unacceptable regressions
- `unsupported`: cannot be applied safely

### 8.3 “Reverse LoRA” Support (Delta Extraction)

Support a tooling path to derive a LoRA-like adapter from a full fine-tune **only when** the base model weights are available.

Requirements:
- original base model weights
- tuned model weights
- compatible architecture mapping
- extraction tool metadata recorded in provenance

Output adapters produced this way must be marked with:
- `provenance.extraction_method = full_delta_low_rank_approximation`
- `portability_class` initially `family_compatible_unverified` (or `exact` only after eval)

---

## 9. Evaluation and Feature Gating (Baseline vs Customized)

### 9.1 Adapter Eval Profile (Conceptual)

```rust
struct AdapterEvalProfile {
    eval_id: String,
    evaluated_on: String, // timestamp / build id
    slot_targets: Vec<SlotId>,
    baseline_refs: Vec<BaselineRef>,
    metrics: Vec<MetricResult>,
    udc_slice_metrics: Vec<UdcSliceMetric>,
    task_slice_metrics: Vec<TaskSliceMetric>,
    regression_flags: Vec<RegressionFlag>,
    conformance_outcome: Vec<SlotConformanceOutcome>,
}
```

### 9.2 Baselines to Compare

For a personalized adapter, compare against at least:
1. base model alone
2. base + default community adapter (if one exists)
3. prior active personalized adapter (if replacing)

### 9.3 UDC-Sliced Evaluation (Core Requirement)

Measure gains and losses by UDC slice, not only aggregate score.

Example questions:
- Did the adapter improve `udc:004` extraction accuracy?
- Did it regress `udc:51` summarization quality?
- Did off-domain hallucinations increase in non-target UDC families?

### 9.4 Feature Gating Policy

A feature is enabled only if:
- required slot is bound
- slot conformance >= threshold for the required capability
- no blocking regression flags are active for the feature's risk profile

Example:
- `AutoEdgeLabeling` may require `slot:text_reasoner` + `structured-extraction` with `partial` allowed.
- `StructuredExtraction` for receipts/invoices may require `full` conformance and no schema-break regression flags.

---

## 10. Multimodal Models: One Model for Many Sockets

This section addresses the additional requirement explicitly.

### 10.1 Supported Binding Patterns

1. **One-model-per-slot** (simple baseline)
- Four separate models fill four slots.

2. **One-model-multi-slot** (shared multimodal)
- One multimodal model fills two or more slots.
- Example: VLM fills `slot:text_reasoner` + `slot:vision_perceptor`.

3. **Omni-model-all-slots**
- One model claims text, embedding, vision, and audio capabilities.
- Runtime binds all four slots to one provider if conformance passes.

4. **Partial-modality projection**
- A multimodal model is bound to only one slot/capability subset.
- Example: use audio transcription only; do not expose its weak text-generation path.

### 10.2 Capability Subsetting Is Mandatory

Model binding must support `capability_subset` because a model can be:
- strong at captioning but weak at extraction
- good at speech-to-text but poor at summarization
- usable for embeddings only under a specific projection/head

The runtime must not infer “all capabilities enabled” from “model loaded.”

### 10.3 Adapter Scope for Multimodal Models

Adapters may target:
- shared backbone modules (affects multiple slots)
- modality-specific towers/heads (affects one slot)
- cross-modal fusion layers (affects multimodal reasoning only)

Manifest/eval metadata must declare adapter scope so the runtime can predict blast radius.

Example scope fields:
- `scope = shared_backbone`
- `scope = modality_audio`
- `scope = modality_vision`
- `scope = cross_modal_fusion`

### 10.4 Safety/Quality Implication

If a single adapter changes shared multimodal layers, re-run conformance on every slot bound to that model. A “text improvement” adapter may accidentally degrade captioning or transcription behavior.

---

## 11. Verse Distribution Artifacts (Model + Adapter + Eval)

### 11.1 Artifact Types (v1)

Define separate artifacts for distribution and trust:
- `ModelManifest`
- `AdapterManifest`
- `AdapterDatasetProfile`
- `AdapterEvalProfile`
- `EvalSuiteManifest` (optional but recommended)

### 11.2 Verse Packaging Recommendation

Publish adapters as a bundle of:
- adapter weights blob
- `AdapterManifest`
- dataset profile reference or embedded summary
- eval profile
- signature + provenance metadata

This makes community adapters searchable and trustworthy.

### 11.3 Search/Discovery Facets

Index at least:
- slot compatibility (`slot:text_reasoner`, etc.)
- capability IDs
- architecture family/base model
- UDC target domains
- privacy/license flags
- portability class
- conformance summary

---

## 12. Upgrade Paths

### 12.1 Direct Reuse (Best Case)

Use existing adapter unchanged when compatibility is `exact` and conformance remains above threshold.

### 12.2 Conditional Reuse (Same Family)

Attempt adapter on a newer revision only when compatibility is `family_compatible_unverified`.

Required steps:
1. bind in quarantine mode
2. run eval suite
3. publish/update `AdapterEvalProfile`
4. upgrade only if conformance passes

### 12.3 Rebuild/Retune (Common Case)

When portability is `retrain_required`, reuse the durable assets:
- dataset lineage (`Reports`, UDC/task slices)
- eval suites
- prior conformance results
- adapter config hints (rank/target modules)

Then train a new adapter for the upgraded base model.

### 12.4 Extracted Adapter ("Reverse LoRA")

If a user has a full tuned checkpoint and the original base checkpoint, support extracting a low-rank adapter as a portability aid. This is a convenience path, not a guarantee of quality; extracted adapters still require eval and conformance gating.

---

## 13. Runtime Architecture Placement (Graphshell / Verse)

### 13.1 Registry Placement (Proposed)

This plan extends the research `ModelRegistry` concept as an **atomic registry** (intelligence runtime capability inventory), alongside `AgentRegistry` and `IndexRegistry`.

Proposed responsibilities:
- model/adaptor manifests
- slot bindings
- compatibility checks
- local artifact resolution (disk/Verse/HTTP)
- conformance cache lookup
- degradation reporting

`AgentRegistry` remains the owner of autonomous behaviors (summarize, classify, suggest tags). Agents query the model/slot binding layer rather than hardcoding model IDs.

### 13.2 Diagnostics Integration

Introduce diagnostics namespaces such as:
- `intelligence.slot.bound`
- `intelligence.slot.degraded`
- `intelligence.adapter.compatibility_failed`
- `intelligence.adapter.eval_missing`
- `intelligence.adapter.regression_flag`
- `intelligence.model.resource_saturated`

This keeps intelligence feature gating observable and debuggable.

---

## 14. Minimal Implementation Sequence (Recommended)

### Phase A — Slot Contracts + Binding (No Training Yet)

1. Implement canonical slot definitions (`text`, `embedding`, `vision`, `audio`)
2. Implement capability declarations and slot binding records
3. Add feature gating by slot conformance/degradation
4. Support multimodal `capability_subset` binding in runtime model selection

### Phase B — Adapter Metadata + Compatibility

1. Implement `AdapterManifest` + `BaseCompatibility`
2. Implement portability class checks
3. Add UI/diagnostics surfacing for compatibility and degradation
4. Support adapter bundle attachment to slot bindings

### Phase C — UDC Dataset/Eval Profiles

1. Implement `AdapterDatasetProfile` schema with UDC/task/modality facets
2. Implement `AdapterEvalProfile` schema with UDC-sliced metrics
3. Require baseline comparison before enabling adapter-backed feature gates above low-risk level
4. Index these artifacts in Verse model search

### Phase D — Tooling and Upgrade Paths

1. Add quarantine bind + eval workflow for family-compatible upgrades
2. Add extracted-adapter import path ("reverse LoRA" support metadata)
3. Add shared-model arbitration for multi-slot multimodal bindings
4. Add adapter scope fields for multimodal towers/backbone/fusion layers

---

## 15. Open Questions (Design, Not Blockers)

1. **Embedding from multimodal models**: Do we accept pooled hidden states from a multimodal model for `slot:semantic_indexer`, or require explicit embedding head conformance?
2. **Adapter composition policy**: How many concurrent adapters do we support per bound provider before predictability drops?
3. **Privacy defaults**: Are `AdapterDatasetProfile` lineage refs shareable by default, or summarized/redacted unless explicit consent is granted?
4. **Eval standardization**: Which minimum eval suites become required for `full` conformance per slot?
5. **Remote inference parity**: Do remote Verse providers publish the same conformance schema, or a reduced trust declaration?

---

## 16. Non-Goals (v1)

- Training pipeline implementation details (optimizer configs, schedulers, GPU cluster orchestration)
- Federated gradient aggregation protocols
- Marketplace economics/pricing for community inference providers
- Automatic adapter extraction from black-box hosted APIs
- Universal cross-architecture adapter translation without retraining

---

## 17. Summary

This plan makes the "curated assistants + personalization" idea operational by treating intelligence as:
- **slots** (stable runtime contracts)
- **capabilities** (declared support)
- **conformance** (measured quality)
- **adapters** (portable deltas with compatibility metadata)
- **UDC-grounded dataset/eval facets** (semantic lineage and domain-specific performance)

It also explicitly supports the multimodal case: one model can fill many sockets, and a single modality path of a multimodal model can be bound independently when only that capability is good enough.
