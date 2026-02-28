# Self-Hosted Model Specification

**Date**: 2026-02-26
**Status**: Proposed (design-ready)
**Scope**: Defines the self-hosted local model environment for Graphshell/Verse: capability contracts, runtime binding, model and engram classification, mini-adapter and FLora tie-ins, cooperative multi-model execution, and UI-facing behavior contracts. Memory storage and extractor/ingestor plumbing live in the companion intelligence memory architecture doc.
**Path note**: This file retains its existing path for continuity, but it replaces the earlier slot-centric plan.
**Related**:
- `design_docs/verse_docs/research/2026-02-24_local_intelligence_research.md`
- `design_docs/verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `design_docs/verse_docs/implementation_strategy/engram_spec.md`
- `design_docs/verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md`
- `design_docs/TERMINOLOGY.md`
- `design_docs/graphshell_docs/technical_architecture/2026-02-27_presentation_provider_and_ai_orchestration.md`

---

## 1. Why This Spec Exists

The earlier framing used fixed "slots" (`text`, `embedding`, `vision`, `audio`) as the primary abstraction. That was useful as a first pass, but it is too rigid.

Graphshell does **not** care how many models a user runs, or which brands they use. It cares whether the local runtime can satisfy the app's contractual requirements:
- what capability is needed
- at what latency / quality / safety level
- with what modality support
- using which local or remote artifacts

This spec therefore pivots from fixed slots to **capability contracts**.

The local self-hosted model environment is where:
- local/private data is processed
- mini-adapter tuning happens
- engrams are assembled and applied
- FLora contributions are prepared
- multiple models can cooperate or divide work by capability

The core metaphor is:
- engrams are the stored food and genetics
- FLora is the garden / community cultivation layer
- models are the fauna that consume different diets and perform useful work

---

## 2. Core Design Rules

1. **Contracts, not named slots**
The runtime should bind models to capability contracts, not to a hard-coded list of sockets.

2. **Capabilities over model identity**
The app evaluates what a model can do, not what it is called.

3. **Self-hosted first**
The local runtime is the sovereign place where raw data, tuning, and high-risk transforms occur.

4. **Engrams are the canonical payload**
Models consume engrams directly or engram-derived subsets. FLora and local tuning both revolve around engrams.

5. **Different models eat different diets**
Not all models consume the same forms of derived knowledge. Some want adapter weights, some want retrieval memory, some want structured facts, and some can combine them.

6. **Cooperation is first-class**
Multiple models may cooperate on the same UI feature, each handling the modalities or transforms it is best at.

7. **UI is contract-gated**
A feature is available only when its contract is satisfiable by the current local model ecology.

---

## 3. Vocabulary

- **Capability Contract**: A runtime requirement describing what the app needs performed, with quality/latency/resource boundaries.
- **Model Provider**: A local or remote model endpoint that declares capabilities and resource constraints.
- **Model Runtime Contract**: The negotiated agreement between a capability request and the provider(s) fulfilling it.
- **Model Diet**: The classes of engram-derived inputs a model can meaningfully consume.
- **Cooperative Pipeline**: A multi-provider execution plan where several models divide one user-facing task.
- **Engram**: The portable customization payload (`TransferProfile` + optional memories).
- **Mini-Adapter Pipeline**: The local process that converts user-controlled data into adapter-ready artifacts and engrams.

---

## 4. Capability Contracts

The app should expose requirements as contracts rather than rigid slots.

```rust
struct CapabilityContract {
    contract_id: String,
    required_capabilities: Vec<CapabilityId>,
    preferred_capabilities: Vec<CapabilityId>,

    accepted_modalities: Vec<ModalityId>,
    accepted_engram_classes: Vec<EngramPayloadClass>,
    accepted_derivation_types: Vec<EngramDerivationType>,

    interface_version: u32,
    latency_budget_ms: Option<u32>,
    memory_budget_mb: Option<u32>,
    quality_floor: Option<QualityTier>,
    privacy_requirement: PrivacyRequirement,
}
```

### 4.1 Contract Examples

`contract:workspace_summary`
- capabilities: `text-generation`, `summarization`
- accepts: text-derived summaries, retrieval memories, adapter weights
- privacy: local-preferred

`contract:semantic_neighbor_search`
- capabilities: `embedding`, `semantic-similarity`
- accepts: retrieval engrams, vector memories, symbolic side-data
- privacy: local-only or trusted-peer only

`contract:image_caption_and_tag`
- capabilities: `vision-captioning`, `vision-labeling`
- accepts: image-derived adapters, visual embeddings, perceptual fingerprints as support metadata

`contract:audio_transcript_index`
- capabilities: `speech-to-text`, optional `audio-labeling`
- accepts: audio adapters, transcript summaries, retrieval memories

### 4.2 Contract Satisfaction

A contract is satisfiable if the runtime can produce a valid plan that:
- meets all required capabilities
- respects privacy constraints
- stays within resource budgets
- has conformance above the configured floor

The plan may use one provider or several.

---

## 5. Provider Declarations

Each model provider declares capabilities, modalities, resource profile, and diet compatibility.

```rust
struct ModelProviderDeclaration {
    provider_id: String,
    model_family: String,
    architecture_family: String,

    capabilities: Vec<CapabilityDeclaration>,
    modalities: Vec<ModalityId>,
    diet_profile: ModelDietProfile,
    consumption_profile: ModelConsumptionProfile,

    execution_mode: ExecutionMode, // local-native | local-onnx | local-service | remote-verse
    resource_profile: ResourceProfile,
}
```

### 5.1 Important Point

One provider may:
- satisfy many contracts
- satisfy only part of one contract
- satisfy different contracts with different modality subsets

This avoids forcing a one-model-per-purpose architecture.

---

## 6. Model Classification

The local runtime needs a practical classification scheme for models.

### 6.1 Primary Axes

```rust
struct SelfHostedModelClass {
    model_family: String,
    architecture_family: String,
    modalities: Vec<ModalityId>,
    preferred_diets: Vec<ModelDietKind>,
    accepted_derivations: Vec<EngramDerivationType>,
    adaptation_methods_supported: Vec<AdapterMethod>,
}
```

### 6.2 Recommended Categories

- **Adapter-tunable text model**
  - accepts `AdapterWeights`, `SoftPrompt`, `DerivedSummary`
  - good for summary, extraction, chat, labeling

- **Retrieval-first semantic model**
  - accepts `EmbeddingVector`, `HashFingerprint`, `StructuredFact`
  - good for search, clustering, dedup, routing

- **Vision-capable model**
  - accepts image/visual adapters, visual embeddings, perceptual fingerprints as support signals
  - good for captioning, labeling, region analysis

- **Audio-capable model**
  - accepts audio-derived adapters, transcript summaries, retrieval context
  - good for STT and audio indexing

- **Multi-diet multimodal model**
  - accepts several derivation types
  - can combine retrieval + symbolic + adapter conditioning

### 6.3 What Hashes Are For

Hashes, perceptual hashes, and locality-sensitive hashes are useful for:
- deduplication
- provenance
- similarity lookup
- routing and retrieval

They are **not** direct substitutes for adapter weights.

---

## 7. Engrams as Fuel

Engrams are the canonical fuel and transfer object for the local model environment.

Models may consume:
- the full engram
- only the runtime-loadable subset
- derived memories extracted from the engram

### 7.1 Useful Engram Classes for Runtime

- `Adapter`: direct behavioral tuning
- `Retrieval`: external memory for search or augmentation
- `Symbolic`: structured fact and schema support
- `Evaluation`: conformance and trust gating
- `Provenance`: trust, legal-risk, and policy context
- `Hybrid`: combined packages

### 7.2 Runtime-Minimal vs Runtime-Rich

Minimal application often requires only:
- `AdapterWeights`
- `AdapterManifest`
- compatibility and conformance checks

Richer application may also use:
- retrieval memories
- symbolic facts
- eval/profile guidance
- provenance and attestation for trust-sensitive features

---

## 8. Local Mini-Adapter Pipeline

The self-hosted model environment owns the local mini-adapter path.

### 8.1 Flow

1. Gather local user-controlled material:
   - notes
   - graph-linked content
   - imported corpora
   - extracted metadata
   - local clips or analysis artifacts
2. Convert raw/private material into safer derived forms.
3. Classify the material with UDC and other required metadata.
4. Produce adapter deltas, retrieval memories, symbolic records, summaries, and evals as appropriate.
5. Package the result as an engram.
6. Keep it local, merge it into local history, or export a redacted version to FLora.

### 8.2 FLora Tie-In

FLora is not separate from this; it is the community layer built on top of the same local conversion pipeline.

Local node:
- grows and tests engrams privately
- exports selected engrams to verses
- imports community checkpoints back into local use

That is why model capability, engram classification, and FLora policy must line up.

---

## 9. Cooperative Multi-Model Execution

A user-facing task may be fulfilled by one provider or several cooperating providers.

```rust
struct CooperativeRuntimePlan {
    plan_id: String,
    contract_id: String,
    stages: Vec<ExecutionStage>,
    final_outputs: Vec<OutputKind>,
}

struct ExecutionStage {
    provider_id: String,
    capability_subset: Vec<CapabilityId>,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
}
```

### 9.1 Example Cooperation Patterns

**Audio + text split**
- audio-capable provider performs STT
- text model summarizes and extracts tasks from the transcript

**Vision + text split**
- vision model captions/images/regions
- text model converts those captions into semantic tags, edge labels, or summaries

**Retrieval + text split**
- embedding model finds relevant context
- text model answers or summarizes using retrieved context

**Multimodal single-provider**
- one provider handles image, audio, and text if it can satisfy the contract efficiently

### 9.2 Arbitration Rule

Prefer:
- the fewest providers needed to satisfy the contract
- unless specialization materially improves quality, safety, or latency

This keeps the runtime pragmatic instead of over-orchestrated.

---

## 10. UI-Facing Model Behaviors

The self-hosted model environment exists to enable UI behaviors, not model inventory for its own sake.

### 10.1 Likely UI Capabilities

- workspace chat / Q&A
- page and node summaries
- semantic search and related-node discovery
- edge-label suggestions
- extraction into structured fields
- image captioning and media tagging
- audio transcription and transcript search
- moderation or trust hints for imported/community engrams
- capability-aware command suggestions

### 10.2 UI Contract Principle

A UI feature should declare:
- required contract
- acceptable degradation
- privacy requirement
- whether local-only is mandatory

Example:
- transcript search may degrade gracefully if only transcript text exists
- invoice extraction should not degrade into unreliable freeform guessing

---

## 11. Runtime Degradation and Trust

Each contract should resolve to one of:
- `full`
- `partial`
- `none`

But degradation must also consider:
- trust of the active engrams
- legal-risk class
- privacy constraints
- whether the current provider is local or remote

A technically capable provider may still be rejected for a contract if:
- its privacy boundary is too weak
- its engram inputs are too low-trust
- its diet does not match the available engram forms

---

## 12. Recommended Default Local Ecology

The system should not require a single monolithic "best" model.

A practical default ecology is:
- one local text-capable provider
- one local retrieval/embedding provider
- one local or optional vision-capable provider
- one local or optional audio-capable provider

But this is not a hard contract. A single multimodal provider may collapse these roles, or specialized providers may divide them more finely.

The app only cares whether the active contracts are satisfiable.

---

## 13. Scope Boundary

This spec owns:
- capability contracts
- provider declarations
- runtime binding and cooperation
- model classification and diets
- engram consumption semantics for local models
- mini-adapter and FLora-facing model behavior
- UI-facing feature contracts

This spec does **not** own:
- long-term memory storage layout
- extractor/ingestor persistence plumbing
- Verse economic policy
- community governance

Those remain in the companion Verse docs.

---

## 14. Immediate Implementation Guidance

1. Replace fixed slot assumptions with contract-based routing in future model-facing APIs.
2. Require providers to declare accepted derivation types and preferred diets.
3. Keep raw/private data processing local; export only redacted or derived engrams by default.
4. Let one contract be satisfied by multiple cooperating providers.
5. Gate UI affordances by contract satisfaction, conformance, trust, and privacy, not by model presence alone.
6. Treat FLora as an extension of the same local engram pipeline, not a separate model world.

This keeps the self-hosted model layer flexible, privacy-respecting, and aligned with the broader Verse architecture.
