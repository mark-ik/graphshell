# Distillation Request and Artifact Contract Spec

**Date**: 2026-04-02
**Status**: Canonical contract
**Priority**: Design-ready companion to the Distillery aspect note

**Related**:

- `ASPECT_DISTILLERY.md`
- `semantic_scene_scaffolding_note.md`
- `../subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`
- `../../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`
- `../../../verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `../../../verse_docs/implementation_strategy/self_hosted_model_spec.md`

---

## 1. Purpose and Scope

This spec defines the concrete request, artifact, and promotion contract for the Distillery aspect.

It governs:

- the request object that enters distillation,
- the approved execution sequence after policy evaluation,
- typed artifact classes and minimum metadata,
- promotion boundaries into STM, LTM, and transfer packaging,
- linked provenance across graph, agent, and transfer authorities.

It does not govern:

- raw source-class policy defaults,
- agent supervision lifecycle,
- persistence internals of source domains,
- or remote exchange protocols.

---

## 2. Canonical Runtime Role

The Distillery aspect begins only after the Distillation Boundary has approved a request.

Normative rule:

- policy approval decides whether a request may read source classes at all,
- Distillery decides how approved inputs become typed derived artifacts,
- promotion and export remain separate later steps.

This keeps authorization, transformation, and transfer from collapsing into one service.

---

## 3. Distillation Request Contract

Every distillation flow should begin from a first-class request object.

Suggested shape:

```rust
pub struct DistillationRequest {
    pub request_id: String,
    pub feature_id: String,
    pub contract_id: String,
    pub requested_source_classes: Vec<IntelligenceSourceClass>,
    pub provider_trust_class: ProviderTrustClass,
    pub requested_time_window: Option<TimeWindow>,
    pub output_kind: DistillationOutputKind,
    pub transform_family: DistillationTransformFamily,
}
```

### 3.1 Required request truths

Every request must declare:

- which feature is asking,
- which contract or privacy class it claims,
- which source classes are requested,
- which provider trust class the result is intended for,
- which transform family is being asked for,
- and what output class is expected.

---

## 4. Execution Sequence Contract

Approved distillation must follow this order:

1. receive `DistillationRequest`
2. evaluate privacy and provider policy at the Distillation Boundary
3. resolve source adapters only for approved source classes
4. retrieve bounded source material through owned query surfaces
5. apply redaction, minimization, and normalization
6. execute one explicit transform family
7. classify the resulting artifact or artifacts
8. stamp provenance, privacy class, trust metadata, and exportability
9. optionally route the artifacts into STM, LTM, or transfer packaging under later promotion rules

Normative rule:

- the transform family must be explicit,
- not inferred from a generic "make something useful" runtime entrypoint.

---

## 5. Transform Family Contract

The Distillery aspect should expose a fixed vocabulary of transform families.

Minimum first-class family set:

1. `Summarize`
2. `ExtractFacts`
3. `BuildRetrievalMemory`
4. `BuildBehaviorProfile`
5. `BuildEvalEvidence`
6. `BuildAdapterDataset`
7. `AssembleEngramInputs`
8. `EmitFreshnessSignal`
9. `BuildArrangementScaffold`
10. `BuildSceneSuggestion`

Each family must declare:

- accepted source classes,
- expected artifact classes,
- allowed provider trust classes,
- default local-only versus export-candidate posture,
- and minimum provenance fields.

---

## 6. Artifact Class Contract

Distillery outputs must be typed artifact classes rather than one generic intelligence record.

Minimum artifact vocabulary:

1. `HashSignal`
2. `StructuredFact`
3. `DerivedSummary`
4. `RetrievalMemory`
5. `BehaviorProfile`
6. `EvalReceipt`
7. `AdapterDatasetSlice`
8. `AdapterWeights`
9. `TransferProfileInput`
10. `ArrangementScaffold`
11. `SceneSuggestion`
12. `ProjectionRecommendation`
13. `SpatialHintSignal`

### 6.1 Required metadata on every artifact

Every artifact should carry:

- stable artifact ID,
- transform family,
- source-class set used,
- provenance references,
- privacy classification,
- provider trust classification,
- exportability classification,
- creation timestamp,
- schema version.

Suggested shape:

```rust
pub struct DistilledArtifact {
    pub artifact_id: String,
    pub artifact_class: DistilledArtifactClass,
    pub transform_family: DistillationTransformFamily,
    pub provenance: DistilledArtifactProvenance,
    pub privacy: ArtifactPrivacyClass,
    pub exportability: ArtifactExportability,
    pub trust: ArtifactTrustClass,
    pub payload: DistilledArtifactPayload,
}
```

---

## 7. Artifact Class Rules

### 7.1 `HashSignal`

Low-level derived signal for deduplication, freshness, clustering, and lightweight retrieval hints.

Default rule:

- local derivation first,
- export only as derived signal,
- never treated as semantic understanding on its own.

### 7.2 `StructuredFact`

Compact extracted fact or assertion suitable for retrieval, summarization support, or transfer packaging.

Default rule:

- provenance must point back to source classes or source references,
- confidence and extraction method should be carried explicitly.

### 7.3 `DerivedSummary`

Human- or model-facing condensed explanation built from approved source material.

Default rule:

- source sensitivity matters,
- many summaries should remain local-only unless explicitly approved for export.

### 7.4 `RetrievalMemory`

Indexed memory unit intended for later retrieval rather than direct user display.

Default rule:

- may hydrate STM first,
- promotion to LTM requires provenance and privacy review.

### 7.5 `BehaviorProfile`

Durable description of agent or workflow behavior derived from episodes and outcomes.

Default rule:

- private by default,
- export only when explicitly classified and justified.

### 7.6 `EvalReceipt`

Typed evidence that a behavior, run, or capability was evaluated.

Default rule:

- should be durable and provenance-rich,
- often suitable for LTM or transfer packaging.

### 7.7 `AdapterDatasetSlice`

Approved dataset material suitable for later tuning or adapter creation.

Default rule:

- stricter privacy and source-class review than summary or fact extraction,
- never implied by generic retrieval-memory generation.

### 7.8 `AdapterWeights`

Produced only by downstream training or adaptation flows, not by arbitrary summarization.

Default rule:

- treated as a distinct payload class,
- never used as the sole synonym for engram value.

### 7.9 `TransferProfileInput`

Artifact already shaped to feed `TransferProfile` or adjacent engram assembly.

Default rule:

- assembled only after artifact classification,
- not directly from raw source material.

### 7.10 `ArrangementScaffold`

Typed proposal for semantic spatial structure rather than direct scene mutation.

Examples:

- region proposals
- lane proposals
- anchor or primary-focus suggestions
- graphlet or local-world scope suggestion
- grouping and placement logic

Default rule:

- previewable and rejectable by default,
- routed into projection and view-owned scene/runtime layers rather than applied as graph truth,
- explanation and provenance are required.

### 7.11 `SceneSuggestion`

Typed proposal for how a view should behave once a graph slice or arrangement is active.

Examples:

- scene-mode recommendation
- relation-visibility and reveal policy suggestion
- layout/physics preset suggestion
- emphasis and density recommendation

Default rule:

- may exist with or without an automatically generated arrangement scaffold,
- should cooperate with arrangement scaffolds when present,
- explanation and provenance are required.

### 7.12 `ProjectionRecommendation`

Typed recommendation that a graphlet or other projection form should precede or accompany scene scaffolding.

Default rule:

- does not itself mutate graph truth,
- should carry enough evidence to explain why that projection form was chosen.

### 7.13 `SpatialHintSignal`

Lightweight signal for spatial suggestion rather than full scaffold generation.

Examples:

- likely primary anchor
- likely basin center
- density warning
- relation-peek recommendation

Default rule:

- may be ephemeral,
- should be cheap to generate,
- must still carry provenance and privacy class.

---

## 8. Promotion Boundary Contract

Promotion is a separate decision from distillation.

### 8.1 STM hydration

Artifacts may hydrate STM when they are:

- session-relevant,
- useful for immediate follow-on retrieval or editing,
- and still subject to change or pruning.

### 8.2 LTM promotion

Artifacts may promote to LTM when they are:

- provenance-rich,
- schema-stable enough to index,
- useful beyond the current session,
- and acceptable under privacy and exportability policy.

### 8.3 Transfer packaging

Artifacts may become transfer inputs only when they are:

- explicitly classified as export-candidate or transfer-candidate,
- suitable for engram or FLora-style exchange,
- and linked to enough provenance to explain where they came from.

Normative rule:

- distillation may produce an artifact,
- but it does not by itself imply STM hydration, LTM promotion, or transfer packaging.

---

## 9. Local-Only vs Export-Candidate Contract

Every distilled artifact must carry one explicit exportability class.

Minimum classes:

1. `LocalOnly`
2. `PromotionCandidate`
3. `TransferCandidate`
4. `ExportDenied`

### 9.1 Interpretation

- `LocalOnly`: may be used locally but never exported under current policy.
- `PromotionCandidate`: may be promoted into durable memory but not automatically exported.
- `TransferCandidate`: suitable for later transfer packaging if the next approval steps succeed.
- `ExportDenied`: artifact may exist for local accountability or debugging but is not eligible for outward flow.

---

## 10. Provenance Link Contract

The Distillery aspect must preserve linked provenance across the three truth families:

1. graph truth
2. agent truth
3. transfer truth

That means one distillation or promotion step may link:

- graph/history references,
- `AWAL` episode or action references,
- and lineage or transfer references.

Normative rule:

- linked provenance is required,
- but one giant universal persistence record is forbidden.

---

## 11. Rejection and Non-Promotion Contract

Not every artifact should survive.

Required rule:

- the runtime must be able to record that an artifact was produced but rejected,
- without forcing rejected artifacts into LTM or transfer packaging.

Typical examples:

- agent debugging traces,
- low-confidence extractions,
- summaries from high-risk source material,
- transient retrieval memories that served their immediate purpose.

---

## 12. Acceptance Criteria

The Distillery companion contract is doing its job when:

1. every distillation flow starts from an explicit request object,
2. transform families are named and policy-visible,
3. output classes are typed and provenance-rich,
4. promotion is separate from transformation,
5. local-only and transfer-candidate outputs are clearly distinguishable,
6. graph truth, agent truth, and transfer truth remain linked but separate.
