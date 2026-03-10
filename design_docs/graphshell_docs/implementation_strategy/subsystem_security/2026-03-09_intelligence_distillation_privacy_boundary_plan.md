# Intelligence Distillation Privacy Boundary Plan

**Date**: 2026-03-09  
**Status**: Active / design incubation plan  
**Scope**: Define the policy, authority boundaries, and execution model for any feature that reads durable local Graphshell state and turns it into intelligence-facing context for local or remote model providers.

**Related**:
- `SUBSYSTEM_SECURITY.md`
- `2026-03-08_unified_security_architecture_plan.md`
- `../subsystem_storage/2026-03-08_unified_storage_architecture_plan.md`
- `../subsystem_history/2026-03-08_unified_history_architecture_plan.md`
- `../../../verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `../../../verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`

---

## 1. Why This Plan Exists

Graphshell now has enough storage, history, and model-planning surface area that a missing bridge is becoming visible:

- Storage defines how durable state is written, encrypted, recovered, and archived.
- History defines which temporal tracks exist and which ones do not yet exist.
- The self-hosted model spec defines capability contracts and provider privacy requirements.

What is still missing is the boundary between those worlds.

There is currently no canonical answer to:

- which durable local state an intelligence feature may read,
- how that read is filtered or redacted,
- which providers may receive which classes of derived context,
- how local-only, trusted-peer, and remote-provider execution differ,
- and where policy enforcement lives.

This matters before any of the following become real runtime features:

- workspace summary
- chat with graph/history
- automatic node summarization
- clip extraction or semantic condensation
- remote inference using graph-derived context

The good news is that there is no active WAL-to-LLM implementation path today. The boundary can therefore be designed before a bypass exists in production code.

---

## 2. Problem Statement

The missing boundary is not "can the app decrypt its own persistence." It obviously can.

The real problem is that decryption is binary while intelligence consumption must be selective.

Once a local process can read the persistence key and load:

- graph durability state
- traversal archives
- node navigation history
- workspace layouts/settings
- clips or extracted content
- future audit history

it has far more information than most intelligence features should receive.

Without a first-class distillation boundary, future intelligence features will tend to do one of two bad things:

1. read raw persistence state directly because it is convenient,
2. treat existing Verse trust/grant rules as if they also authorize intelligence consumption.

Neither is acceptable.

Sync authorization is not intelligence authorization.  
Encrypted local storage is not a selective privacy policy.  
Replay/read APIs are not automatically model-facing APIs.

---

## 3. Canonical Rule

Every intelligence-facing read of durable Graphshell state must cross an explicit **Distillation Boundary** before any provider-facing execution begins.

That means:

- no model provider reads WAL keyspaces directly,
- no model provider reads snapshot tables directly,
- no model provider reads history archives directly,
- no remote provider receives raw durable state unless an explicit policy allows that source class,
- no future "helpful fallback to remote" may bypass the declared privacy requirement of the feature contract.

The boundary exists even for local-only execution.  
Local execution and remote execution differ in destination risk, but both require the same source-class selection and policy evaluation step.

---

## 4. Canonical Vocabulary

### 4.1 Distillation

**Distillation** is the act of turning durable local state into model-facing context, prompts, summaries, retrieval bundles, structured facts, or exportable derived artifacts.

Distillation is not:

- sync
- replay
- backup/export of raw state
- diagnostics
- direct persistence recovery

### 4.2 Provider Trust Classes

Every intelligence execution target must be classified as one of:

1. `LocalOnly`
2. `LocalPreferred`
3. `TrustedPeerAllowed`
4. `RemoteAllowed`

These are compatible with the self-hosted model spec's privacy language, but this plan applies that language to the storage/history read boundary.

### 4.3 Source Classes

The distillation boundary evaluates durable source classes, not just files or tables.

Canonical source classes:

1. `GraphDurability`
2. `TraversalHistory`
3. `NodeNavigationHistory`
4. `NodeAuditHistory`
5. `WorkspaceLayoutAndSettings`
6. `ClipContent`
7. `NodeBinaryMetadata`
8. `DiagnosticsObservations`
9. `SecurityAndTrustState`
10. `EphemeralRuntimeState`

The storage/history/security subsystems remain the truth owners for these classes. The distillation boundary does not re-own the data; it governs intelligence access to it.

---

## 5. Default Distillation Policy

The default policy should be conservative and asymmetric.

| Source class | Default local policy | Default remote policy | Notes |
| --- | --- | --- | --- |
| `GraphDurability` | Allowed | Denied unless explicitly opted in | URLs/titles/edges may reveal private research graph structure. |
| `TraversalHistory` | Allowed | Denied by default | Behavioral metadata is sensitive even when content is not. |
| `NodeNavigationHistory` | Allowed | Denied by default | Stable node-local browsing lineage is high-sensitivity. |
| `NodeAuditHistory` | Allowed for explicit audit features only | Denied | Future track; should start strict. |
| `WorkspaceLayoutAndSettings` | Denied unless feature explicitly needs it | Denied | Usually irrelevant to model quality. |
| `ClipContent` | Local-only by default | Denied unless explicit per-feature consent + redaction | Raw clipped text/media is the highest-risk source class. |
| `NodeBinaryMetadata` | Denied by default | Denied | Includes thumbnails/favicons and similar payloads with low value, high leakage risk. |
| `DiagnosticsObservations` | Denied unless summarized for local debugging tools | Denied | Diagnostics is observability, not model fuel. |
| `SecurityAndTrustState` | Denied | Denied | Trust stores, keys, grants, and security events are never model inputs by default. |
| `EphemeralRuntimeState` | Denied unless feature owns it explicitly | Denied | Drafts, temporary UI state, and renderer-local caches are not automatic intelligence inputs. |

Two important consequences:

- remote execution should normally consume derived/redacted artifacts, not raw source-class payloads,
- "provider can technically do it" is not enough to authorize access.

---

## 6. Authority Model

This boundary is cross-cutting, so ownership must be explicit.

### 6.1 Security owns policy

The Security subsystem owns:

- source-class policy defaults
- provider trust-class policy
- consent requirements
- redaction requirements for outbound/provider-crossing execution
- denial behavior and audit requirements

### 6.2 Storage and History own truth/query surfaces

Storage and History own:

- persisted truth
- replay/query correctness
- source-class-specific retrieval APIs
- archive and history semantics

They do **not** own provider authorization.

### 6.3 Intelligence runtime owns execution

The model/intelligence runtime owns:

- contract satisfaction
- provider selection
- local vs remote execution routing
- handling of derived artifacts once policy has approved the request

It does **not** define what may be read.

### 6.4 Diagnostics owns observability

Diagnostics owns:

- policy-denied events
- local execution versus remote export events
- redaction-applied signals
- health-summary projections for distillation safety posture

Diagnostics must never carry raw sensitive payloads merely to describe a policy decision.

---

## 7. Canonical Execution Model

Every intelligence feature that consumes durable state should follow this sequence:

1. declare a capability contract and privacy requirement,
2. declare the requested source classes and time/window scope,
3. resolve a candidate provider plan,
4. evaluate a `DistillationPolicy` against source classes, provider trust class, and feature contract,
5. retrieve data through approved query surfaces,
6. apply redaction/aggregation/minimization,
7. execute provider call,
8. emit diagnostics/audit metadata,
9. optionally persist only the approved derived artifact.

This implies a first-class request object.

```rust
struct DistillationRequest {
    request_id: String,
    feature_id: String,
    contract_id: String,
    requested_source_classes: Vec<IntelligenceSourceClass>,
    provider_trust_class: ProviderTrustClass,
    requested_time_window: Option<TimeWindow>,
    output_kind: DistillationOutputKind,
}
```

And a first-class policy decision:

```rust
enum DistillationDecision {
    AllowLocal,
    AllowWithRedaction(RedactionProfile),
    RequireExplicitConsent,
    Deny { reason: String },
}
```

The important architectural point is not the exact Rust type. It is that the decision surface exists as a real authority boundary rather than as scattered `if remote { ... }` checks.

---

## 8. Query-Surface Rule

No intelligence feature should consume persistence internals directly.

Required rule:

- `GraphStore` keyspace iteration is persistence-internal,
- replay APIs are history-facing,
- timeline/archive export is user/export-facing,
- intelligence-facing features use dedicated query surfaces that already normalize source-class semantics.

Examples:

- workspace summary should not iterate raw `mutations` or `traversal_archive`,
- chat-with-graph should not read snapshot tables directly,
- node summarization should request `ClipContent` or node-local derived text through a dedicated source adapter,
- future agent pipelines should not treat diagnostics rings as a retrieval corpus.

This protects the app against convenience-driven bypasses.

---

## 9. Grant Model Separation

Verse trust and workspace grants are not enough.

The app must keep these separate:

1. **collaboration grant**  
   authorizes peer sync or shared workspace behavior

2. **distillation permission**  
   authorizes use of local durable state for an intelligence feature

3. **provider trust approval**  
   authorizes a given provider class to receive local or derived artifacts

A peer with workspace read access does not thereby gain access to model-facing summaries.  
A remote model provider trusted for one feature does not thereby gain blanket access to graph history.  
A local-only contract must not silently fall back to a remote provider.

---

## 10. Remote and Export Boundary

If a distillation output leaves the local device, that is a trust-boundary crossing.

Remote-provider execution and FLora/Verse export should therefore follow the same baseline rule:

- raw/private processing remains local by default,
- remote/export paths receive redacted or derived artifacts by default,
- outbound artifacts carry provenance and privacy metadata,
- source-class policy decides what is exportable, not just the provider's capability.

This aligns with the self-hosted model spec's rule: keep raw/private data processing local and export only redacted or derived artifacts by default.

---

## 11. Diagnostics and Audit

Add a dedicated diagnostic family:

- `intelligence.distillation.requested`
- `intelligence.distillation.denied`
- `intelligence.distillation.local_executed`
- `intelligence.distillation.remote_exported`
- `intelligence.distillation.redaction_applied`
- `intelligence.distillation.policy_violation`

Diagnostics payloads should contain:

- request ID
- feature ID
- provider trust class
- source classes
- decision outcome
- redaction profile ID if used

Diagnostics payloads should **not** contain:

- raw prompt bodies
- raw clip text
- raw URLs unless the diagnostic family explicitly documents them as allowed
- trust-store or key material

---

## 12. Phased Execution Sequence

### 12.1 Phase 1: Local-only policy scaffold

Land first:

- source-class taxonomy
- `DistillationRequest` / decision surface
- local-only enforcement
- diagnostics for allow/deny

Do not add remote fallback in this phase.

### 12.2 Phase 2: Dedicated query surfaces

Before real summarization or chat features:

- add intelligence-facing query adapters for graph, clip, and history data,
- forbid direct persistence internals on feature paths,
- define explicit redaction/minimization profiles.

### 12.3 Phase 3: Remote-provider support

Only after the local boundary is working:

- add provider trust-class routing,
- add explicit user consent flows for remote-permitted source classes,
- add outbound artifact provenance and policy metadata.

### 12.4 Phase 4: Verse/FLora integration

Only after remote-provider policy is stable:

- define which derived artifacts may be exported,
- align engram/export policy with source classes,
- ensure exports do not become an accidental raw-history escape hatch.

---

## 13. Non-Goals

This plan does not:

- replace persistence encryption,
- define model quality/conformance itself,
- replace search/index architecture,
- define the full UI for consent prompts,
- make Verse sync grants equivalent to intelligence permissions,
- authorize exporting raw WAL or archive data to external services.

---

## 14. Done Gate

This plan is architecturally complete when:

1. every intelligence feature declares source classes and privacy requirement,
2. no provider-facing feature reads persistence internals directly,
3. remote execution cannot occur without a real policy decision,
4. local-only contracts cannot silently degrade to remote execution,
5. diagnostics and audit exist for distillation decisions,
6. storage/history/security/model docs use compatible vocabulary for this boundary,
7. exported artifacts are derived/redacted by default rather than raw-history dumps.

Until then, Graphshell should treat WAL/history-to-model features as design-incubation work, not implementation-ready product surface.
