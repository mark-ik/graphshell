# DISTILLERY — Aspect

**Date**: 2026-04-02
**Status**: Architectural aspect note
**Priority**: Design incubation clarification

**Related**:

- `distillation_request_and_artifact_contract_spec.md`
- `semantic_scene_scaffolding_note.md`
- `../subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`
- `../../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`
- `../../../verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `../../../verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `../system/register/2026-03-08_sector_g_mod_agent_plan.md`

**Policy authority**: This file is the canonical policy authority for the Distillery aspect within Graphshell's architecture taxonomy. Detailed intelligence-memory, `AWAL`, and engram plans refine contracts and execution sequencing and must defer policy authority to this file for aspect classification and ownership boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

---

## 1. Purpose

This note defines the **Distillery aspect** as the architectural owner of policy-gated transformation from approved local graph and agent sources into typed intelligence artifacts.

It exists to keep one boundary explicit:

- durable local state is not automatically intelligence-facing input,
- agent traces are not the same as graph truth,
- and typed retrieval, eval, behavior, adapter, and transfer artifacts must not collapse into one undifferentiated memory blob.

---

## 2. What The Distillery Aspect Owns

- distillation request and transform-family orchestration after policy approval
- typed artifact-class emission (`StructuredFact`, `DerivedSummary`, `RetrievalMemory`, `BehaviorProfile`, `EvalReceipt`, `AdapterDatasetSlice`, `TransferProfile`, and adjacent classes)
- typed scene-intelligence emission such as scaffold proposals, projection recommendations, and spatial hint artifacts where approved local source classes support them
- transform-family contracts such as summarize, extract-facts, retrieval-memory build, behavior-profile build, eval-evidence build, adapter-dataset build, engram assembly handoff, and freshness-signal emission
- source-adapter choreography across approved durable graph, history, clip, runtime, and `AWAL` inputs
- provenance-ready handoff from approved source classes to STM/LTM promotion and transfer packaging
- local-only versus export-candidate distinction at the artifact-class level

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

The Distillery aspect does not own policy authorization or source truth.

- **Security** owns source-class policy, redaction, consent, and provider trust rules.
- **Storage** and **History** own durable truth and approved query surfaces.
- **AgentRegistry** owns agent declaration, supervision, and lifecycle.
- **AWAL** owns agent durability.
- **STM/LTM** own working and promoted memory.
- **Engram / FLora** flows own transfer packaging and exchange.

The Distillery aspect is therefore the typed transform layer that sits between approved reads and promoted or portable outputs.

---

## 4. Bridges

- Security -> Distillery: approved source classes, trust class, consent state, redaction profile
- Storage / History -> Distillery: source-class query surfaces for durable local truth
- AgentRegistry / `AWAL` -> Distillery: supervised agent trace and episode material
- Distillery -> STM / LTM: approved derived artifact hydration and promotion
- Distillery -> Engram / FLora: selected typed artifacts become portable transfer inputs rather than raw source dumps

---

## 5. Architectural Rule

If a behavior answers "how does approved local graph or agent state become a typed intelligence artifact without collapsing graph truth, agent truth, and transfer truth into one authority?" it belongs to the **Distillery aspect**.

---

## 6. Canonical Constraints

The Distillery aspect must obey these rules:

1. No distillation path bypasses the explicit Distillation Boundary.
2. No provider reads durable local state directly from persistence internals.
3. Local-only and export-capable outputs remain distinct.
4. Artifact classes stay typed and provenance-rich.
5. Graph truth, agent truth, and transfer truth remain separate authorities.

---

## 7. Execution Posture

The canonical execution posture is:

1. privacy boundary first
2. approved source-class read
3. typed transform-family execution
4. artifact classification and provenance stamping
5. STM/LTM promotion or transfer packaging only where policy allows it

This is a local-first aspect. Remote or federated flows must consume derived artifacts, not raw local truth, by default.

---

## 8. Design Status

This aspect is real enough to name now, but it remains an incubation area.

What is mature:

- the need for a distinct aspect,
- the separation between graph truth, agent truth, and transfer truth,
- the privacy-boundary dependency,
- the typed-artifact direction.

What remains open:

- exact `AWAL` carrier shapes,
- episode granularity refinements,
- artifact promotion criteria,
- local-only versus exportable class boundaries for some future outputs,
- how much of agent-perspective modeling should surface in the product.

---

## 9. Architectural Outcome Sought

The Distillery aspect is successful when Graphshell can say all of the following clearly:

- which source classes an intelligence feature may read,
- which transform family turned those inputs into derived artifacts,
- which artifacts remain local,
- which artifacts may be promoted or exported,
- and which authority still owns the underlying truth.

Until that is true, distillation should be treated as a first-class architecture track, not as a convenience helper inside storage, history, or agent runtime.
