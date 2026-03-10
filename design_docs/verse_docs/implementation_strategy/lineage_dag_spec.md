# Lineage DAG Specification

**Date**: 2026-03-09
**Status**: Proposed (shared structural primitive)
**Scope**: Defines the canonical lineage graph structure shared by Engrams and FLora checkpoints, including ancestry links, append-only chain commitments, traversal policy, trust filtering, revocation semantics, and lineage-shaping operations.
**Related**:
- `engram_spec.md`
- `flora_submission_checkpoint_spec.md`
- `community_governance_spec.md`
- `proof_of_access_ledger_spec.md`
- `2026-03-09_agent_wal_and_distillery_architecture_plan.md`

---

## 1. Purpose

Both Engrams and FLora checkpoints already carry lineage-like information:

- content-addressed payloads
- parent or input references
- merge ancestry
- contributor and governance attestations

But the current model is still split and weaker than it needs to be:

- Engrams treat lineage mainly as a flat merge-memory record
- FLora checkpoints treat lineage mainly as a single-parent or source-list relationship
- neither exposes a shared traversable structure with explicit policy operations
- neither defines lineage as a configurable trust surface

This spec introduces one shared primitive:

- **Lineage DAG** for structure
- **TraversalPolicy** for interpretation

Engrams and FLora checkpoints then become different payload-bearing node types inside the same structural model.

---

## 2. Core Design Rules

1. **Lineage is a DAG, not a list**
Lineage may have multiple parents. A single-parent chain is only a special case.

2. **Content address the payload, not the policy**
The payload node is content-addressed and immutable. Traversal and trust policies are separate overlays.

3. **Use one structure for both Engrams and FLora**
The payload type differs. The lineage mechanics do not.

4. **Lineage is more than audit**
Lineage must support active operations such as prune, cutoff, filter, merge, and weight.

5. **Append-only ordering and DAG ancestry are distinct**
A node may have many ancestry parents but only one append-only predecessor within a given stream or authority sequence.

6. **Community and personal policy may differ**
The same lineage graph may be traversed under a community-default policy or a local personal policy.

---

## 3. Canonical Vocabulary

### 3.1 Lineage DAG

A **Lineage DAG** is the complete ancestry graph for a lineage-bearing object family.

It contains:

- one or more roots
- immutable payload-bearing nodes
- directed parent edges
- optional append-only stream commitments
- trust and revocation metadata

### 3.2 Lineage Node

A **Lineage Node** is one immutable payload plus its ancestry metadata.

It is not necessarily the same thing as an Engram or a checkpoint body. It wraps a payload reference and the structural metadata required for lineage traversal.

### 3.3 Stream Commitment

A **Stream Commitment** is the append-only hashchain ordering for one named authority stream.

Examples:

- one FLora community's checkpoint stream
- one local user's engram publication stream
- one curated verse's official lineage sequence

This is separate from DAG parentage. DAG parentage answers "what informed this node." Stream commitment answers "what did this authority publish before this node."

### 3.4 Traversal Policy

A **Traversal Policy** is the set of rules used to evaluate and traverse a lineage DAG for a specific purpose.

Examples:

- community default checkpoint policy
- personal local tuning policy
- trust-depth-limited federation policy
- audit-only full provenance policy

---

## 4. Canonical Structure

### 4.1 Conceptual Model

```rust
struct LineageDag {
    dag_id: Cid,
    node_roots: Vec<Cid>,
    nodes: HashMap<Cid, LineageNode>,
}

struct LineageNode {
    node_cid: Cid,
    payload: LineagePayloadRef,
    parent_nodes: Vec<Cid>,

    created_at_ms: u64,
    origin_scope: OriginScope,

    stream_commitment: Option<StreamCommitment>,
    trust: TrustEnvelope,
    revocation: RevocationState,
}

enum LineagePayloadRef {
    Engram {
        engram_id: String,
        engram_cid: Cid,
    },
    FloraCheckpoint {
        checkpoint_id: String,
        checkpoint_cid: Cid,
        community_id: CommunityId,
    },
}
```

### 4.2 Stream Commitment

```rust
struct StreamCommitment {
    stream_id: String,
    previous_stream_node: Option<Cid>,
    previous_chain_hash: Option<Blake3Hash>,
    chain_hash: Blake3Hash,
    signer: DidKey,
    signature: Signature,
}
```

Rules:

- `chain_hash` commits to the stream's prior published state and the current node CID
- a node may belong to zero or one official append-only stream in v1
- multi-stream publication may be added later, but should not complicate v1

Recommended v1 chain rule:

`chain_hash = BLAKE3(previous_chain_hash || node_cid || created_at_ms || stream_id)`

The exact canonical bytestring format belongs in implementation details, but the ordering commitment must be deterministic.

### 4.3 Revocation State

```rust
enum RevocationState {
    Active,
    Superseded { replacement: Option<Cid> },
    Revoked { reason_ref: Option<Cid> },
    Quarantined { reason_ref: Option<Cid> },
}
```

Revocation is metadata about acceptance and trust, not mutation of lineage history. The node remains present in the DAG.

---

## 5. Canonical Operations

The lineage DAG should expose a small shared operation surface.

### 5.1 `prune`

`prune(dag, predicate)`

Exclude subtrees or branches that match a condition.

Example uses:

- remove all branches passing through a compromised checkpoint
- exclude one verse from local tuning
- remove low-trust contributor branches

### 5.2 `cutoff`

`cutoff(dag, timestamp_or_height)`

Ignore ancestors before a chosen chronological or stream height boundary.

Example uses:

- trust only recent lineage
- freeze historical baseline
- cap audit surface for runtime inference

### 5.3 `filter`

`filter(dag, trust_policy)`

Traverse only nodes that satisfy policy rules.

Example uses:

- direct-trust only
- path length <= 2
- exclude quarantined nodes
- require minimum governance class

### 5.4 `merge`

`merge(dag_a, dag_b, merge_policy)`

Combine lineage graphs or subgraphs under a defined merge strategy.

Example uses:

- merge two engram branches
- incorporate one verse's accepted lineage into another verse's checkpoint lineage

### 5.5 `weight`

`weight(dag, weighting_policy)`

Assign influence or relevance weights to nodes or paths before downstream transforms.

Example uses:

- upweight recent lineage
- downweight low-confidence branches
- emphasize personal local branches over community defaults

---

## 6. Traversal Policy

### 6.1 Conceptual Model

```rust
struct TraversalPolicy {
    max_depth: Option<u32>,
    cutoff_before_ms: Option<u64>,
    excluded_roots: Vec<Cid>,
    required_roots: Option<Vec<Cid>>,

    trust_filter: TrustFilter,
    revocation_filter: RevocationFilter,
    weighting: WeightFunction,
}
```

`TraversalPolicy` is intentionally separate from the DAG. It is configuration over shared structure, not part of payload identity.

### 6.2 Community vs Personal Policy

Two policy scopes are first-class:

1. **Community policy**
   defines the official lineage view for checkpoint production and shared governance

2. **Personal policy**
   defines the individual's local traversal of the same DAG for tuning, retrieval, ranking, or exclusion

This means:

- the DAG is shared
- policy is local or community-specific
- different traversals can produce different effective outputs without forking the underlying graph

### 6.3 Relationship to Graphshell History Traversal

This spec uses the word "traversal" in a way that is structurally similar to Graphshell's History subsystem, but the truth source is different.

The distinction must remain explicit:

- **history traversal** = movement through graph/content state over time
- **lineage traversal** = movement through provenance/derivation state over ancestry

Both are path-selection problems over append-only graph-like structures.

Both need:

- cursor position
- cutoff and filtering rules
- replay or traversal policy
- selective reconstruction of state from prior records

But they do **not** share the same stored truth.

The shared primitive is therefore not one universal history store. It is a reusable cursor/policy model for walking append-only DAG-like structures.

Conceptually:

```rust
struct DagCursor {
    position: Cid,
    policy: TraversalPolicy,
    visited: HashSet<Cid>,
}
```

Graphshell History may use this idea for:

- temporal replay over snapshots + WAL
- node navigation history over `NavigateNode`-style records
- future audit-history queries

This spec uses the same idea for:

- engram ancestry walks
- verse checkpoint ancestry walks
- trust- and time-bounded lineage selection

### 6.4 Shared Semantics, Separate Authorities

The architectural rule is:

- History owns graph temporal truth.
- `AWAL` owns agent temporal truth.
- Lineage DAG owns provenance truth.

They may share traversal semantics, but they must not be flattened into one universal DAG authority.

---

## 7. Engram Mapping

### 7.1 Current gap

The Engram spec already has `MergeLineage` and `EngramMergeRecord`, but those are flat provenance memories rather than a first-class traversable structure.

### 7.2 Required adoption

Engrams should:

- reference lineage DAG nodes instead of only flat merge records
- allow multi-parent ancestry explicitly
- treat merge strategy as one transform over lineage, not the lineage model itself

### 7.3 Result

"Tuning an Engram" becomes lineage-policy selection plus downstream merge behavior, rather than a black-box merge with provenance attached after the fact.

That unlocks:

- cutoff by time
- prune by trust source
- filter by governance status
- re-weight by recency or domain fit

---

## 8. FLora Mapping

### 8.1 Current gap

FLora checkpoints already point to source submissions and source engrams, and submissions already have `parent_checkpoint`, but this is not yet a shared multi-parent lineage model with explicit stream commitments.

### 8.2 Required adoption

FLora should:

- treat checkpoints as lineage nodes with multi-parent ancestry
- add stream commitments for official checkpoint ordering within one community
- carry cross-verse incorporation anchors when one community incorporates another's accepted lineage

### 8.3 Federation

For federation, one verse does not need to absorb another verse's whole append-only stream as its own. It only needs to anchor the exact upstream lineage state it incorporated.

That means:

- checkpoint incorporation references upstream node CID
- if available, it also references upstream `chain_hash`
- downstream verifiers can prove which upstream published state informed the downstream checkpoint

This makes inter-verse provenance auditable without introducing global consensus.

---

## 9. Trust, Revocation, and Audit

### 9.1 Trust is local policy over shared lineage

Trust is not baked into the DAG topology itself.

The DAG records:

- who published what
- which nodes derive from which ancestors
- what revocation/supersession metadata exists

Traversal policy decides what to trust.

### 9.2 Revocation propagation

This spec does not define the full gossip protocol, but it requires that revocation events be first-class lineage metadata, not out-of-band footnotes.

Minimum requirement:

- a lineage-aware consumer must be able to learn that a node became `Superseded`, `Revoked`, or `Quarantined`
- traversal policy must be able to exclude or downgrade those branches deterministically

### 9.3 Trust depth

Because lineage may cross multiple verses, the model should support explicit trust-depth limits.

Examples:

- direct only
- depth <= 2
- unrestricted full provenance

This is the lineage analogue of certificate path-length constraints.

---

## 10. Limits and Scaling

### 10.1 Cryptographic layer

No hard practical limit. Hashing and signature checks remain local per node or per stream step.

### 10.2 Retrieval layer

Audit cost grows with path length and DAG breadth. Consumers should be allowed to:

- request full provenance
- request summarized lineage
- request policy-reduced lineage

### 10.3 Runtime layer

Very large lineage DAGs should support summarized or windowed traversal views rather than forcing full materialization in latency-sensitive paths.

This means lineage should be:

- traversable in full for audit
- summarizable for UI and runtime decisions
- policy-reducible for practical use

---

## 11. Minimal Required Fields for a Valid Lineage Node

Every lineage node must have:

- stable payload reference
- node CID
- parent node list
- creation time
- trust envelope
- revocation state

For stream-published nodes, it must additionally have:

- `stream_id`
- `previous_stream_node` or stream root marker
- `chain_hash`
- signer identity and signature

---

## 12. Adoption Plan

### 12.1 Phase L1

Land this spec and make it the shared structural reference for lineage-bearing docs.

### 12.2 Phase L2

Patch `engram_spec.md`:

- reframe `MergeLineage`
- reference lineage DAG nodes
- distinguish flat memory payload from traversable lineage structure

### 12.3 Phase L3

Patch `flora_submission_checkpoint_spec.md`:

- add multi-parent checkpoint lineage
- add stream commitment fields
- add federation anchor rules

### 12.4 Phase L4

Patch governance/trust docs:

- community traversal policy
- personal traversal policy
- revocation propagation expectations

---

## 13. Non-Goals

This spec does not:

- define a blockchain or global consensus mechanism
- define the exact transport gossip protocol for revocations
- replace Engrams or checkpoints as payload objects
- require every runtime path to load full ancestry eagerly

---

## 14. Done Gate

This spec is successful when:

1. Engram lineage and FLora lineage no longer define separate weaker ancestry structures.
2. stream ordering and DAG ancestry are explicitly distinguished.
3. prune/cutoff/filter/merge/weight are canonical operations rather than ad hoc downstream logic.
4. personal and community lineage policies can differ without duplicating the underlying DAG.
5. lineage is usable as a programmable trust surface, not just an audit trail.
