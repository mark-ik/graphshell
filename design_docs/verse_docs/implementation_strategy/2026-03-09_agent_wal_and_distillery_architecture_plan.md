# Agent WAL and Distillery Architecture Plan

**Date**: 2026-03-09
**Status**: Proposed (design incubation)
**Scope**: Defines the missing intelligence-native durability layer between Graphshell's graph/history storage and Verse/FLora-facing intelligence artifacts: the agent-owned write-ahead log (`AWAL`), the canonical experience unit, and the `Distillery` pipeline that turns local graph and agent traces into typed memory, retrieval, evaluation, and engram outputs.
**Related**:
- `self_hosted_model_spec.md`
- `2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
- `engram_spec.md`
- `flora_submission_checkpoint_spec.md`
- `../../graphshell_docs/implementation_strategy/subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`
- `../../graphshell_docs/implementation_strategy/system/register/2026-03-08_sector_g_mod_agent_plan.md`
- `../../graphshell_docs/implementation_strategy/subsystem_history/2026-03-08_unified_history_architecture_plan.md`
- `../../graphshell_docs/implementation_strategy/subsystem_storage/2026-03-08_unified_storage_architecture_plan.md`

---

## 1. Why This Plan Exists

The current architecture has three important pieces already:

- Graphshell persists graph truth, workspace truth, and history truth.
- the intelligence memory plan defines STM/LTM, engram memory, extractors, and ingestors.
- the self-hosted model spec defines capability contracts, provider declarations, and privacy constraints.

What is still missing is the intelligence-native durability model between them.

Today there is no canonical answer to:

- what the unit of an intelligence "experience" is,
- what an agent durably remembers about its own observations and actions,
- how graph/history state and agent activity become typed intelligence artifacts,
- what should become retrieval memory versus adapter data versus eval evidence versus FLora-ready engrams,
- and how to keep "graph truth" separate from "agent truth."

Without that missing middle layer, everything tends to collapse into one of two bad shortcuts:

1. treat the graph WAL as if it were the complete intelligence corpus,
2. treat engrams as if they were only weights or only dataset blobs.

Both are too coarse.

This plan introduces two missing concepts:

- `AWAL` — the agent-owned write-ahead log
- `Distillery` — the policy-gated transform pipeline that produces typed intelligence artifacts

---

## 2. Problem Statement

Graphshell's WAL answers "what durable graph state changed."

It does **not** answer:

- what an agent noticed,
- what hypotheses it formed,
- what tools it invoked,
- what outputs were accepted or rejected,
- what feedback shaped later behavior,
- or what sequence of observations and actions should count as a reusable intelligence experience.

Likewise, the existing intelligence docs correctly say that engrams are more than LoRA files, but they still need an upstream pipeline that answers:

- where engram memories come from,
- what gets promoted,
- what remains local-only,
- what is exportable,
- and what granularity is meaningful.

The missing primitive is not another storage table. It is a new architectural layer with its own truth model.

---

## 3. Canonical Rule

Graphshell should treat graph durability, agent durability, and transfer durability as three distinct authorities:

1. **Graph truth**  
   nodes, edges, graph-backed content, graph history, workspace state

2. **Agent truth**  
   an agent's own observation/action/evaluation trail

3. **Transfer truth**  
   typed artifacts promoted out of local graph and agent traces for reuse, retrieval, tuning, export, or FLora submission

These must not be flattened into one log.

The graph WAL remains the authority for graph truth.  
`AWAL` becomes the authority for agent truth.  
Engrams and memory records remain the authority for transfer truth.

The `Distillery` is the bridge between those authorities. It does not replace them.

---

## 4. Canonical Vocabulary

### 4.1 `AWAL`

**Agent Write-Ahead Log**: the append-only durable journal for a supervised agent's own state-bearing observations, tool actions, outputs, and outcomes.

`AWAL` is:

- durable
- inspectable
- replayable
- partitioned by agent identity
- policy-gated for export and promotion

`AWAL` is not:

- the graph WAL
- raw chain-of-thought persistence
- a replacement for STM/LTM
- a synonym for engram

### 4.2 Distillery

**Distillery**: the cross-cutting transform layer that consumes approved local source classes and emits typed intelligence artifacts.

User-facing shorthand can call this a subsystem. Architecturally it is probably best modeled as an **Aspect** constrained by security/storage/history policy, not as a raw persistence service.

### 4.3 Experience Unit

The missing atomic unit should be an **Episode**, not a node and not a whole session.

Recommended hierarchy:

1. `Observation`
2. `ActionStep`
3. `Episode`
4. `Session`
5. `Engram`

Where:

- `Observation` = one raw or derived intake event
- `ActionStep` = one agent/tool decision and result
- `Episode` = the smallest promotable intelligence unit
- `Session` = an ordered collection of episodes
- `Engram` = a portable transfer object built from one or more promoted episodes and related memories

This keeps the atom small enough to inspect and large enough to be meaningful.

---

## 5. Canonical Experience Model

### 5.1 `Observation`

An observation is the smallest recorded input the agent can cite.

Examples:

- a graph node reference
- a clip-content reference
- a traversal or navigation-history slice
- a DOM/content extraction result
- a text chunk retrieved from local memory
- a visual hash or thumbnail signature
- a user correction or explicit rating

Observations may be raw, derived, or referenced-by-ID. They do not need to inline payload bytes.

### 5.2 `ActionStep`

An action step is a durable record that the agent:

- selected a tool or transform,
- ran it against some observations,
- produced an output,
- and received an outcome.

Examples:

- "summarized nodes 3, 7, 9"
- "proposed three tags and one was accepted"
- "generated a workspace summary and user rejected it"
- "ran visual-fingerprint extraction on a rendered page"

### 5.3 `Episode`

An episode is the smallest reusable intelligence experience.

It should contain:

- purpose or task class
- bounded input set
- ordered action steps
- outputs
- outcome/evaluation
- provenance and privacy metadata

An episode is where "experience" becomes more than a trace.

### 5.4 `Session`

A session is a temporal container over episodes.

It is useful for:

- local replay
- episode grouping
- temporal context
- diagnostics and analysis

But sessions are too large and noisy to be the primary transfer atom.

---

## 6. `AWAL` Architecture

### 6.1 Ownership

`AWAL` belongs to agent identity, not to graph identity.

That means:

- each supervised agent has its own journal partition,
- journals may be additionally scoped by workspace, profile, or user,
- and deleting or editing graph state does not implicitly rewrite agent history.

### 6.2 What `AWAL` should record

Recommended durable entry classes:

1. `ObservationRecorded`
2. `ActionPlanned`
3. `ToolInvoked`
4. `OutputProduced`
5. `OutputAccepted`
6. `OutputRejected`
7. `FeedbackApplied`
8. `EvalRecorded`
9. `EpisodeClosed`

Each entry should prefer references and typed summaries over raw payload duplication.

### 6.3 What `AWAL` should not record by default

By default, `AWAL` should avoid:

- raw hidden chain-of-thought
- persistence keys or trust material
- unrestricted copies of clip payloads
- unrestricted copies of prompt bodies when references or redacted forms are enough
- renderer-private transient data

The point of `AWAL` is durable accountability and reusable experience, not opaque thought dumping.

### 6.4 Relationship to STM/LTM

`AWAL` is neither STM nor LTM.

- STM is mutable working memory.
- LTM is promoted, indexed long-term memory.
- `AWAL` is an append-only accountability and provenance trail for agent experience.

The distillery may read `AWAL` into STM, promote derivatives into LTM, or package results into engrams, but the log itself remains a separate truth source.

---

## 7. Distillery Architecture

### 7.1 Role

The distillery consumes policy-approved local source classes and emits typed outputs.

Inputs may include:

- graph durability state
- traversal history
- node navigation history
- knowledge/ontology tags
- clips and extracted content
- agent `AWAL` traces
- STM/LTM retrieval results
- local evaluation and feedback signals

The distillery should never be an unbounded "read everything and train on it" surface.

### 7.2 The distillery is not one transform

It should expose distinct transform families:

1. `Summarize`
2. `ExtractFacts`
3. `BuildRetrievalMemory`
4. `BuildBehaviorProfile`
5. `BuildEvalEvidence`
6. `BuildAdapterDataset`
7. `AssembleEngram`
8. `EmitFreshnessSignal`

Each family has different privacy, trust, and promotion requirements.

### 7.3 Output classes, not rarity tiers

Do not model the output space as "rarities."

Use typed artifact classes with:

- derivation type
- trust level
- privacy policy
- model-diet relevance
- exportability

Recommended output classes:

1. `HashSignal`
2. `StructuredFact`
3. `DerivedSummary`
4. `RetrievalMemory`
5. `BehaviorProfile`
6. `EvalReceipt`
7. `AdapterDatasetSlice`
8. `AdapterWeights`
9. `TransferProfile`

This is more actionable than a rarity metaphor and aligns with the existing engram vocabulary.

---

## 8. Hashes, Thumbnails, and Visual Signatures

The hash idea is worth preserving, but it should sit low in the stack as a derived signal class rather than as the primary training payload.

Useful candidate signal types:

- perceptual hash of rendered page thumbnails
- locality-sensitive hash for visual similarity
- DOM/content fingerprint
- favicon/brand signature
- freshness/change signature over repeated visits

Likely uses:

- deduplication
- clustering
- site freshness/change detection
- community staleness reports
- lightweight visual retrieval hints

Default stance:

- local derivation first
- export only as derived signal
- do not treat visual hashes as equivalent to content understanding
- use them as support metadata for distillation, not as the main corpus

Implementation detail such as "Servo texture access is easier than Wry overlay capture" belongs in source-adapter planning, not in the top-level architecture.

---

## 9. Relationship to Engrams and FLora

This plan should tighten the current engram story rather than replace it.

### 9.1 Engrams are still the portable envelope

`TransferProfile` remains the canonical transport object.

The distillery does not replace engrams. It produces the memories and artifacts that engrams package.

### 9.2 Not every output becomes an engram

Many distillery outputs should remain local-only:

- temporary retrieval memories
- rejected outputs
- agent debugging traces
- private behavior profiles
- high-risk source-derived summaries

Only selected promoted artifacts should become engram memories or `TransferProfile` payloads.

### 9.3 FLora should consume typed artifacts, not just weights

FLora should be fed by:

- eval evidence
- dataset lineage
- derived summaries
- structured facts
- compatibility metadata
- adapter weights when present

This avoids collapsing the ecosystem into "LoRA equals the only thing that matters."

---

## 10. Relationship to the Intelligence Privacy Boundary

The privacy-boundary plan defines what may cross from durable app state into intelligence features.

This plan defines what durable intelligence-native state exists once that crossing is allowed.

Required rule:

- graph/history/clip/security data cross the **Distillation Boundary** first,
- approved reads may participate in episodes and `AWAL`-adjacent transforms,
- distillery outputs may later be promoted into memory or engrams,
- remote/export paths use derived artifacts, not raw local truth, by default.

So the dependency direction is:

1. privacy boundary first
2. `AWAL` and episode model
3. distillery transforms
4. memory promotion
5. engram/FLora export

---

## 11. Relationship to Graphshell History

The distillery and `AWAL` plans should align with Graphshell History, but not collapse into it.

The useful connection is structural:

- History traversal walks graph/content state over time.
- `AWAL` replay walks agent observation/action state over time.
- lineage traversal walks provenance and derivation state over ancestry.

These are three different truths with one family resemblance: they all need cursor-based, policy-bounded traversal over append-only records.

That means:

- History should continue to own local graph temporal truth.
- `AWAL` should continue to own local agent temporal truth.
- lineage DAGs should continue to own provenance truth for engrams and FLora checkpoints.

What should be shared is the traversal/cursor mental model, not the storage authority.

### 11.1 Canonical boundary event

When a distillation or promotion step crosses from graph/agent activity into intelligence artifacts, the system should emit a linked event across the relevant authorities.

One underlying operation may therefore create:

1. a node audit-history event in Graphshell history
2. one or more `AWAL` records (`OutputProduced`, `OutputAccepted`, `EpisodeClosed`, etc.)
3. one or more lineage-DAG nodes or edges in engram/FLora space

The event should be **linked** across systems, not represented as one giant universal node shared by all of them.

This is the missing cross-system event type:

- history needs it for node audit/provenance
- `AWAL` needs it for durable agent experience
- lineage needs it for ancestry of promoted artifacts

Until that boundary event is defined, these systems can reference each other, but they cannot cleanly agree on provenance across the handoff.

---

## 12. Relationship to `AgentRegistry`

`AgentRegistry` remains the supervision and lifecycle surface for agents.

It should not also become the intelligence memory architecture.

Recommended split:

- `AgentRegistry` owns declaration, supervision, lifecycle, capabilities, and signal wiring
- `AWAL` owns agent durability
- `Distillery` owns typed transforms
- STM/LTM own working and promoted memory

This keeps agent execution separate from agent memory and transfer semantics.

---

## 13. Phased Execution

### 12.1 Phase A — Define episode and `AWAL` schema

Land first:

- episode vocabulary
- `AWAL` entry classes
- ownership and partitioning rules
- local inspect/replay surface

### 12.2 Phase B — Local-only distillery

Before any remote/provider export:

- local-only transforms from graph/history/clip/`AWAL`
- typed artifact outputs
- policy-gated promotion into STM/LTM

### 12.3 Phase C — Visual/hash signals

Add low-risk derived signals:

- freshness/change fingerprints
- visual clustering signals
- optional local retrieval hints

### 12.4 Phase D — Engram assembly

Only after typed artifacts exist:

- map promoted outputs to `EngramMemoryKind`
- assemble `TransferProfile` envelopes
- classify local-only versus exportable memory kinds

### 12.5 Phase E — FLora and Verse exchange

Only after privacy and engram classification are stable:

- permit selected distillery outputs into FLora submission flows
- preserve provenance from episode -> artifact -> engram -> submission

### 12.6 Phase F — Agent perspective profiles

If Graphshell later wants a "model ghost" or agent perspective surface, it should emerge here as a derived profile over `AWAL` and episode data, not as raw hidden-thought persistence.

That keeps the "ghost" concept inspectable and policy-governed.

---

## 14. Open Questions

1. Is `Episode` enough, or do some workflows need a smaller promotable unit than an episode?
2. Should `AWAL` be per agent definition, per running agent instance, or both?
3. What minimum metadata is required for a promotable `BehaviorProfile`?
4. Which visual/hash signal classes are useful enough to standardize early?
5. How much rejected output history should remain durable?
6. Which `EngramMemoryKind` extensions are needed for behavior/eval/profile artifacts?
7. How should local user edits and explicit feedback alter later episode weighting?

---

## 15. Done Gate

This plan is architecturally successful when:

1. Graphshell has a canonical promotable intelligence atom (`Episode` or an explicitly revised replacement).
2. supervised agents can keep durable, inspectable journals without polluting graph truth.
3. the distillery emits typed artifacts instead of a generic "training blob."
4. engrams are fed by typed derived artifacts rather than being treated as synonym for weights.
5. local-only, trusted-peer, and exportable paths are distinct and policy-governed.
6. "agent memory," "graph memory," and "transfer memory" are no longer conflated in planning docs.

Until then, agent experience, graph history, and engram assembly should be treated as related but separate tracks rather than folded into one persistence story.
