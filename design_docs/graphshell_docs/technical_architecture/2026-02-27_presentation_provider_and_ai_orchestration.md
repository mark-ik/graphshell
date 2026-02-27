# Presentation Provider Contract, Node Facets, and AI Orchestration (2026-02-27)

**Status**: Architecture proposal (implementation-guiding)
**Audience**: Runtime, registry, and UI/workbench contributors
**Related**:
- `GRAPHSHELL_AS_BROWSER.md`
- `2026-02-18_universal_node_content_model.md`
- `../implementation_strategy/2026-02-24_universal_content_model_plan.md`
- `../implementation_strategy/2026-02-22_registry_layer_plan.md`

---

## 1. Purpose

Define a single integration contract that allows Graphshell to combine:

1. Multiple presentation backends (`viewer:servo`, `viewer:wry`, embedded egui viewers, graph-native views),
2. Node-local facet views (document + schematic + timeline + dependency projections), and
3. A tiered AI runtime (tiny local model + retrieval + optional larger model).

This document does not replace the existing `ViewerRegistry` plan; it extends it with a provider-oriented capability model and explicit AI orchestration boundaries.

---

## 2. Core Direction

Graphshell should treat rendering paths as **complementary providers** behind one runtime contract, not as mutually exclusive engines.

- `egui` / `egui_tiles`: shell orchestration, pane topology, command surfaces.
- `egui_graphs`: semantic graph projections and schematic subgraphs.
- `viewer:servo` / `viewer:wry`: runtime viewer providers for web-native or fallback document fidelity.
- Embedded egui viewers (text/image/pdf/audio/etc.): domain-specific providers for non-web assets.

Primary design principle: choose provider per node/facet based on capabilities and policy, not hard-coded engine preference.

---

## 3. Presentation Provider Contract

### 3.1 Contract Shape

The runtime-facing contract should model capabilities first:

```rust
pub trait PresentationProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn advertised_capabilities(&self) -> ProviderCapabilities;
    fn can_present(&self, request: &PresentationRequest) -> ProviderMatch;

    fn open_session(&self, request: &PresentationRequest) -> Result<PresentationSession, ProviderError>;
    fn render_frame(&self, session: &mut PresentationSession, frame: &FrameContext) -> RenderResult;

    fn hit_test(&self, session: &PresentationSession, point: egui::Pos2) -> HitTestResult;
    fn accessibility(&self, session: &PresentationSession) -> Option<accesskit::TreeUpdate>;
    fn search(&self, session: &PresentationSession, query: &str) -> ProviderSearchResult;

    fn suspend(&self, session: &mut PresentationSession);
    fn resume(&self, session: &mut PresentationSession);
    fn close(&self, session: PresentationSession);
}
```

### 3.2 Capability Descriptor

`ProviderCapabilities` should include at minimum:

- fidelity class (`exact`, `high`, `approximate`),
- interaction class (`read_only`, `interactive`, `editable`),
- geometry model (`free_scroll`, `paged`, `fixed_canvas`, `graph_projection`),
- subsystem declarations (accessibility/security/storage/history support level),
- performance envelope hints (warm-up cost, steady-state frame cost, memory profile).

This aligns with the folded capability declaration model in `TERMINOLOGY.md` and keeps ownership with the surface provider.

---

## 4. Node Facet Taxonomy

Each node may expose multiple facet projections over the same underlying semantic identity.

### 4.1 Canonical Facets

- **Document Facet**: Primary content fidelity view (web page, PDF page stream, image, text body).
- **Schematic Facet**: Graph-native abstraction of the node's internal or local-neighborhood structure.
- **Timeline Facet**: Temporal/traversal projection (changes, visits, events).
- **Dependency Facet**: Linked references/citations/imports/relations projection.
- **Metadata Facet**: Properties, tags, MIME/address info, lifecycle and provenance.

### 4.2 Facet Presentation Rule

Facet selection is independent from node identity and lifecycle. A node stays the same node while facet/provider changes.

`Node + Facet + Provider` is the runtime presentation tuple.

### 4.3 Why Facets Matter

Some content classes are better represented as graph schematics than literal document rendering. Facets make this first-class without replacing high-fidelity providers.

---

## 5. Provider Selection Policy

Selection policy should support explicit and automatic modes:

1. explicit user override (`viewer_id_override` / future `facet_override`),
2. workbench/frame defaults,
3. capability match scoring,
4. policy objective (`best_fidelity`, `best_latency`, `best_graph_explainability`),
5. fallback provider.

Scoring dimensions:

- content compatibility (MIME/address/facet),
- fidelity target,
- interaction requirements,
- subsystem conformance,
- current resource budget.

---

## 6. Tiny Local Model Role ("22MB Class")

Small local models should be treated as a **cognitive coprocessor**, not a final authority.

### 6.1 Good Always-On Tasks

- tag and badge suggestion from local node metadata/snippets,
- candidate edge ranking for user confirmation,
- command and pane routing suggestions,
- local cluster/focus hints for graph layout,
- concise per-node notes for schematic labels.

### 6.2 Avoid Assigning to Tiny Models

- long-form factual synthesis without retrieval,
- high-stakes correctness decisions,
- deep multi-hop reasoning over large corpora.

### 6.3 Safety Boundary

Tiny-model outputs should default to `suggestion` intents requiring confirmation unless confidence and policy explicitly allow autonomous actions.

---

## 7. Tiered AI Orchestration

Graphshell should run a tiered path:

1. **Tier A (Local Tiny Model)**: low-latency interaction glue.
2. **Tier B (Retrieval/Index Grounding)**: factual context assembly from local graph/index.
3. **Tier C (Optional Larger Model)**: escalated reasoning/synthesis only when needed.

Escalation triggers:

- confidence below threshold,
- request complexity above token/reasoning bounds,
- user explicitly requests deep analysis.

Demotion rule: when Tier C is unavailable, system falls back to Tier A+B with explicit degraded-mode messaging.

### 7.1 Model Capacity and Concurrency Policy

There is no required hard cap on registered model providers, but runtime should enforce an operational cap:

- **N installed, M hot** model policy where `M << N`,
- many providers may be registered, but only a bounded set can be resident/active concurrently,
- hot-set management should be policy-driven (memory/latency budget + user mode + workload class).

This keeps multi-model flexibility without forcing all users into high resource usage.

### 7.2 AI Optionality and Capability-Derived Reduction

AI must be optional at product level and configurable per user profile.

Canonical operating modes:

- **AI Disabled**: no model providers active; all AI-powered features hidden or hard-disabled.
- **Multi-Model Adaptive**: multiple providers enabled; router chooses by capability/policy.

When capability coverage is partial (regardless of model count), the system naturally runs in a **capability-derived reduced surface**:

- features with satisfied required capabilities remain enabled,
- features missing required capabilities are unavailable,
- features missing optional capabilities run in declared degraded mode.

Mode selection must be user-controllable and persistence-backed.

---

## 8. Capability-Gated Feature Activation

Features must declare required capabilities and activate only when those capabilities are present.

### 8.1 Feature Capability Contract

Each AI-backed feature declares:

- required capability set,
- optional capability set,
- minimum conformance level,
- allowed degradation behavior.

Example shape:

```rust
struct FeatureCapabilityRequirement {
    feature_id: &'static str,
    required: Vec<ModelCapability>,
    optional: Vec<ModelCapability>,
    min_conformance: ConformanceLevel,
    degrade_to: FeatureDegradationMode,
}
```

### 8.2 Activation Rules

- If required capabilities are absent, feature does not activate.
- If only optional capabilities are absent, feature may run in declared degraded mode.
- UI must reflect activation state (`enabled`, `degraded`, `unavailable`) and reason.

### 8.3 No-AI and Partial-Capability Guarantees

- No-AI mode must remain fully usable for core graph/workbench workflows.
- Partial capability coverage must never imply hidden escalation to unavailable providers.
- Capability checks must run before feature invocation, not after failure.

---

## 9. Registry Integration Guidance

- Keep provider registrations in `ViewerRegistry`-adjacent surfaces, with capability metadata declared at registration.
- Route AI-coprocessor outputs through intent boundaries (`GraphIntent` and workbench intent routing class), not direct state mutation.
- Expose provider/facet/AI health via Diagnostics subsystem channels (degradation must be observable).

---

## 10. Incremental Adoption Plan

1. Introduce capability descriptor additions and provider scoring hooks.
2. Add `FacetKind` to node presentation state (runtime first, persistence optional later).
3. Implement first schematic facet for selected node neighborhood.
4. Add tiny-model assistant lane as suggestion-only.
5. Add feature capability declarations and activation-state reporting.
6. Add explicit No-AI mode and capability-derived reduced-surface behavior.
7. Add Tier B retrieval grounding and escalation routing.
8. Gate Tier C integrations behind explicit feature flags and policy.

---

## 11. Acceptance Criteria (Architecture-Level)

- Provider choice is policy-driven and capability-aware, not hard-coded by content type only.
- A single node can switch between at least two facets without identity churn.
- Tiny-model assistant can propose suggestions without violating intent authority boundaries.
- Degradation mode (`full`/`partial`/`unavailable`) is declared and visible for provider and AI lanes.
- AI Disabled mode runs without model-provider side effects or hidden fallback calls.
- Partial capability coverage enables only capability-satisfied features and surfaces clear unavailable/degraded states.

---

## 12. Non-Goals (This Iteration)

- Replacing existing viewer implementations.
- Committing to a single universal renderer for all formats.
- Removing dedicated engines where fidelity requirements are strict.

The design target is a hybrid system: specialized providers where needed, graph-native and approximation facets where they are advantageous.
