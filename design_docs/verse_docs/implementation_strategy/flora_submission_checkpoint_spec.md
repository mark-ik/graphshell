# FLora Submission and Checkpoint Specification

**Date**: 2026-02-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the canonical submission, review, checkpoint, and payout surfaces for Verse FLora communities. This spec builds on the Engram spec and treats engrams as the submission payload, with adapter memories as the primary trainable component and ranking-policy metadata as a first-class review surface.
**Related**:

- `design_docs/verse_docs/implementation_strategy/engram_spec.md`
- `design_docs/verse_docs/implementation_strategy/lineage_dag_spec.md`
- `design_docs/verse_docs/implementation_strategy/proof_of_access_ledger_spec.md`
- `design_docs/verse_docs/implementation_strategy/community_governance_spec.md`
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`

---

## 1. Purpose

FLora is the Verse adaptation pipeline for community-managed model customization and shared discovery policy.

Its core responsibilities are:

- accept engram-based submissions from contributors
- validate and review those submissions under community policy
- produce immutable accepted checkpoints
- distribute rewards, reputation, or both

In practice, FLora should be understood as a **signed, shared, forkable ranking policy package** for a verse.

That package may include:

- source weights
- topic affinities
- trust graph modifiers
- recency, novelty, and depth preferences
- community-specific ranking knobs
- evaluation and provenance metadata
- optional local model adapters or other learned components

This lets a verse publish portable discovery behavior such as:

- boosting primary sources
- downweighting ragebait or controversy-seeking material
- preferring trusted domains or trusted contributors
- surfacing certain hubs, clusters, or feeds ahead of others

The goal is not to force every verse into one central recommender. The goal is to let communities publish, audit, compare, remix, fork, sell, or privately run distinct ranking policies against local graphs, local models, and shared community surfaces.

The design is deliberately:

- **append-only** for auditability
- **content-addressed** for deduplication and replay safety
- **metadata-first** so sparse submissions can still be reviewed

## 1.1 Framing

FLora is not best understood as "the community LoRA" in the narrow sense.

The more accurate framing is:

- the canonical payload is still an Engram
- adapter weights are still an important trainable memory
- but the thing a verse is really sharing is a forkable relevance policy with explicit provenance

Some FLora packages may be mostly heuristic and metadata-driven.
Others may include embeddings, statistical parameters, or compact local adapters.
Others may stay entirely on the safe end of the spectrum and only express reviewable ranking preferences without any learned component.

This framing keeps model tuning optional and plural rather than treating FLora as one mandatory, opaque recommender layer.

## 1.2 Non-Goals and Failure Modes to Avoid

FLora should explicitly avoid becoming any of the following:

- a giant, opaque personalization engine whose behavior cannot be inspected or contested
- a checkpoint lineage with unclear training provenance or unverifiable derivation claims
- a recommendation system that produces charming but inexplicable weirdness with no debuggable cause
- a ranking surface captured by high-volume actors simply because they can out-produce everyone else
- a privacy failure where shared weights or attached memories encode sensitive personal behavior too directly

The intended standard is **predictable, inspectable weirdness** rather than inscrutable personalization.
Communities should be able to explain what a FLora package is trying to optimize, what inputs shaped it, what it boosts or downweights, and why one checkpoint differs from another.

This implies several practical expectations:

- provenance must be explicit enough for review and audit
- ranking behavior should be decomposable into intelligible policy inputs where possible
- high-volume contribution alone should not determine acceptance or prominence
- privacy review must treat weights and learned artifacts as potential carriers of sensitive behavioral traces

---

## 2. Core Design Rules

1. **The submission payload is an Engram**
Every FLora submission points to an `Engram` (`TransferProfile`) and optional attached `EngramMemory` items.

2. **Weights are primary, but not mandatory**
Submissions may contain `AdapterWeights`, but a community may also accept evidence-only or eval-only engrams into a review queue.

3. **Checkpoints are immutable**
Accepted outputs are immutable checkpoint records. Subsequent changes produce new checkpoints.

4. **Review is explicit**
No checkpoint should be promoted directly from a raw contributor submission in curated or moderated communities.

5. **Large payloads are fetched, not broadcast**
Pubsub should carry manifests and references only. Large adapter bytes stay behind content-addressed retrieval.

6. **Ranking policy is first-class**
FLora checkpoints should be reviewable as ranking-policy packages, not only as trainable weight bundles.

7. **Inspectability over opacity**
Communities should prefer submissions whose behavior, provenance, and intended effects can be meaningfully reviewed.

8. **Plurality over centralization**
FLora should support many forkable local or community policies rather than converging on one global recommender.

---

## 3. Core Entities

```rust
struct FloraSubmission {
    submission_id: String,
    community_id: CommunityId,
    adapter_id: AdapterId,
    contributor: PeerId,

    engram_id: String,
    engram_blob_cid: Cid,
    adapter_memory_ref: Option<String>,

    parent_checkpoint: Option<String>,
    declared_udc_profile: UdcProfileSummary,
    requested_reward_class: RewardClass,

    created_at_ms: u64,
    signature: Signature,
}

struct FloraReviewRecord {
    review_id: String,
    submission_id: String,
    reviewer: PeerId,
    verdict: ReviewVerdict,
    score: Option<f32>,
    notes_ref: Option<Cid>,
    created_at_ms: u64,
    signature: Signature,
}

struct FloraCheckpoint {
    checkpoint_id: String,
    community_id: CommunityId,
    adapter_id: AdapterId,

    source_submissions: Vec<String>,
    source_engrams: Vec<String>,
    output_engram_id: String,
    output_engram_cid: Cid,
    policy_diff_refs: Vec<Cid>,

    strategy: CheckpointStrategy,
    policy_snapshot_ref: Option<Cid>,

    created_at_ms: u64,
    curator_signature: Signature,
}

enum ReviewVerdict {
    Accept,
    AcceptLowConfidence,
    RequestMoreContext,
    RejectIncompatible,
    RejectLowEvidence,
    Quarantine,
}
```

---

## 4. Submission Classes

```rust
enum RewardClass {
    Unpaid,
    ReputationOnly,
    FixedBounty,
    PerformanceWeighted,
    CuratorDiscretion,
}

enum CheckpointStrategy {
    SingleSubmissionPromote,
    LinearMerge,
    WeightedMerge,
    DomainSelectiveMerge,
    MetadataOnlyPromotion,
}
```

### 4.1 Recommended Submission Profiles

`EvidenceOnly`
- no `AdapterWeights`
- includes evals, compatibility, or dataset summaries
- used to influence review, bounty targeting, or future merges

`RankingPolicyCandidate`
- expresses ranking weights, trust modifiers, policy heuristics, or other reviewable discovery preferences
- may include evals, provenance, and comparison notes without shipping a new adapter
- used when a verse wants portable ranking behavior without requiring model retraining

`AdapterCandidate`
- contains `AdapterWeights`
- may be sparse
- eligible for checkpoint promotion if community rules permit

`CheckpointCandidate`
- references one or more already-reviewed submissions
- usually emitted by curators, moderators, or automated policy

---

## 5. Submission Validation

Before a submission enters review:

1. Verify signature and contributor membership.
2. Verify `engram_blob_cid` exists and resolves.
3. Parse the engram envelope and validate the declared `EngramValidationClass`.
4. Enforce adapter compatibility if `AdapterWeights` are present.
5. Enforce policy limits:
   - max submission manifest size
   - max attached memory count
   - max decompressed adapter size
   - per-contributor outstanding review queue depth
6. If the submission materially affects ranking or discovery behavior, require enough metadata to explain intended boosts, suppressions, or prioritization changes.

### 5.1 Safe Defaults

- GossipSub publishes only a compact submission manifest (< 64 KiB target).
- Adapter weights and large evidence blobs are fetched separately by CID.
- Receiver-side resource limits must reject oversized or highly compressible payloads before full materialization.
- Communities should apply strict schema validation before forwarding manifests to other peers.

These defaults mirror established pubsub and content-addressed best practices: advertise small control envelopes, retrieve bulk data on demand, and validate early.

---

## 6. Review and Moderation Flow

### 6.1 Recommended Review Pipeline

1. `Submitted`
2. `Validated`
3. `QueuedForReview`
4. `Accepted`, `Rejected`, or `Quarantined`
5. `Checkpointed` (optional)
6. `RewardSettled` (optional)

### 6.2 Quorum Defaults

- `Open` communities may allow single-review promotion for low-risk adapters.
- `Curated` communities should require at least 2 matching `Accept`-class reviews for checkpoint promotion.
- `Moderated` communities should require policy-defined quorum plus one authorized moderator/curator signature on the checkpoint.

### 6.3 Review Inputs

Reviewers should consider:
- compatibility with target base models
- UDC/domain fit
- eval deltas versus baseline and current checkpoint
- safety or policy flags
- contributor trust and prior receipts
- whether the submission is sufficiently contextualized for payout
- whether the ranking effects are legible enough to audit and compare
- whether the submission creates undue capture risk for high-volume actors or favored domains
- whether learned components could encode sensitive behavioral traces beyond the verse's privacy tolerance

### 6.4 Verse-Specific Appraisal Policy

Engram quality is not universal. Each verse should be able to appraise the same submission differently.

Recommended appraisal inputs:
- domain relevance (`udc_profile` alignment)
- accepted derivation types for that verse's target models
- compatibility with the verse's preferred adapter families or model diets
- contributor trust and prior accepted work
- evidence richness (evals, lineage, compatibility, attestation)
- legal-risk class and redaction profile
- storage and review cost relative to community budget
- explainability of ranking effects
- concentration risk in whose content or behavior is being privileged
- privacy leakage risk from attached memories or learned artifacts

This means a submission may be:
- high-value in one verse
- accepted with low payout in another
- rejected in a stricter verse

### 6.5 Derived-Only Submission Default

FLora should treat personal or privately gathered material as **derived-only by default**.

Preferred shared materials:
- adapter weights
- ranking parameters and heuristic manifests
- compatibility metadata
- provenance and attestation
- UDC/domain tags
- evals
- derived summaries
- hashes, structural signatures, and fingerprints

Not shared by default:
- raw clips
- screenshots
- snippets
- private notes
- direct personal corpora

If a contributor publishes higher-risk source material intentionally, that should be treated as an explicit social/community publication choice and governed by the verse's legal-risk policy.

### 6.6 Operator Review Checklist

Before accepting or checkpointing a FLora submission, operators and reviewers should be able to answer the following:

1. What does this submission try to change?
    Can the reviewer describe, in plain language, which sources, topics, behaviors, or signals it boosts, suppresses, or reprioritizes?
2. Where did it come from?
    Is the provenance clear enough to understand what data, judgments, or prior checkpoints shaped it?
3. Is the behavior inspectable enough to debug?
    If the checkpoint behaves oddly, does the verse have enough metadata, eval context, and policy description to compare it against prior checkpoints and explain the difference?
4. Does it create capture risk?
    Does the submission primarily privilege whoever can produce the most volume, the most engagement bait, or the most self-reinforcing trust edges?
5. Does it leak too much?
    Could the weights, embeddings, heuristics, or attached memories encode sensitive personal behavior more directly than the community intends to publish?
6. Is the evidence proportional to the claim?
    If the submission makes large ranking or adaptation claims, does it include enough eval, compatibility, and lineage context to justify them?

If a reviewer cannot answer these questions with reasonable confidence, the safe default should be `RequestMoreContext`, `AcceptLowConfidence`, or `Quarantine` rather than silent promotion.

---

## 7. Checkpoint Production

Checkpoint generation creates a new immutable community-approved output.

### 7.1 Rules

- A checkpoint must always produce a new `checkpoint_id`.
- Source submissions remain immutable and addressable.
- Checkpoint records should reference the policy snapshot and review receipts that justified acceptance.
- If weights are merged, the output engram must include `MergeLineage`.
- Metadata-only promotion is allowed when a community wants to preserve evidence or review context without merging weights.
- When a checkpoint materially changes discovery behavior, the community should publish enough metadata to compare it against the prior checkpoint in human terms, not only by hash.
- When `RankingPolicy` or `TrustPolicy` artifacts change materially, the checkpoint should publish canonical `PolicyDiff` artifacts and include their CIDs in `policy_diff_refs`.

### 7.1A Checkpoint Comparison Semantics

When a verse compares one FLora checkpoint to another, the review should not stop at "different CID, different output."
The meaningful comparison is behavioral and policy-oriented.

At minimum, reviewers should compare:

1. Lineage change.
    Which submissions, upstream checkpoints, or merge inputs are new, removed, or reweighted?
2. Ranking-policy change.
    Which features, source weights, topic affinities, recency settings, novelty boosts, or depth preferences changed?
3. Trust and suppression change.
    Which domains, contributor classes, trust modifiers, or suppression rules were added, removed, or tightened?
4. Learned-artifact change.
    Did the checkpoint add, remove, or replace embeddings, calibrations, or adapter weights that could materially change retrieval, clustering, or recommendation behavior?
5. Evidence change.
    What new evals, compatibility reports, or lineage summaries justify the update relative to the prior checkpoint?
6. Privacy and capture-risk change.
    Does the new checkpoint increase the chance of leaking sensitive behavior or of overprivileging high-volume actors, dominant domains, or self-reinforcing trust cliques?

Best-practice output for checkpoint comparison should include:

- a short human-readable change summary
- canonical machine-readable `PolicyDiff` artifacts for changed ranking or trust policies
- links or CIDs for the eval and review receipts that justify the change
- an explicit statement of whether the update is expected to broaden discovery, narrow it, rebalance trust, or mainly improve explanation and calibration

If a verse cannot explain a checkpoint delta in these terms, it should treat the checkpoint as under-contextualized even if the bytes are valid.

Recommended minimum publication rule:

- if `RankingPolicy` changed, publish at least one `RankingPolicy` `PolicyDiff`
- if `TrustPolicy` changed, publish at least one `TrustPolicy` `PolicyDiff`
- if no material policy changed, `policy_diff_refs` may be empty

### 7.2 Rollback

Rollbacks do not mutate or delete prior checkpoints.

Instead:
- mark a checkpoint `Superseded` or `Revoked`
- emit a new checkpoint pointing to the prior safe ancestor
- retain the old object for audit history

This preserves append-only auditability and avoids ambiguous state.

---

## 8. Reward and Payout Hooks

FLora does not settle payouts directly. It emits reward-eligible events that the Proof of Access / reward ledger can consume.

```rust
struct FloraRewardReceipt {
    receipt_id: String,
    submission_id: String,
    checkpoint_id: Option<String>,
    beneficiary: PeerId,
    reward_class: RewardClass,
    reward_units: u64,
    reason: RewardReason,
    created_at_ms: u64,
}

enum RewardReason {
    AcceptedSubmission,
    IncludedInCheckpoint,
    ReviewWork,
    ModerationWork,
    EvaluationWork,
}
```

### 8.1 Best-Practice Default

- Keep reward accounting **off the hot path** of submission acceptance.
- Accept or reject the submission first.
- Compute payouts in epoch batches.

This avoids making checkpoint creation depend on live payment success.

---

## 9. Anti-Abuse and Resource Policy

Recommended minimum protections:
- per-peer submission rate limits
- per-adapter queue caps
- per-community maximum outstanding unreviewed bytes
- decompression ratio caps
- duplicate-engram suppression by CID
- reviewer rate limits and audit trails

Communities should also be able to:
- require a minimum local proof-of-work or reputation threshold for new contributors
- require a small refundable stake for high-cost review queues
- quarantine all first-time contributors until an explicit reviewer opts in

---

## 10. Transport Guidance

Recommended transport split:
- **GossipSub**: submit compact manifests, review notices, checkpoint announcements
- **Bitswap / request-response / provider fetch**: retrieve engram bodies, adapter weights, and larger evidence bundles

Rationale:
- keeps pubsub bandwidth bounded
- reduces amplification risk
- improves replayability and deduplication

---

## 11. Immediate Defaults (v1)

- Use engrams as the only accepted top-level payload.
- Permit sparse submissions.
- Require explicit review before checkpointing in any non-open community.
- Keep review and payout decoupled.
- Keep checkpoints append-only and immutable.
- Fetch large adapter bytes by CID, never inline in pubsub announcements.
- Default to derived-only submission policy for privately gathered material.
- Let each verse define its own appraisal weighting instead of pretending quality is universal.

These defaults make FLora practical without requiring an overbuilt settlement or governance stack on day one.
