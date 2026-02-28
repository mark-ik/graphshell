# Community Governance and Moderation Specification

**Date**: 2026-02-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the governance, membership, moderation, review quorum, and dispute-handling rules for Verse communities, including FLora and storage/indexing policy surfaces.
**Related**:
- `design_docs/verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md`
- `design_docs/verse_docs/implementation_strategy/proof_of_access_ledger_spec.md`
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`

---

## 1. Purpose

Verse communities need explicit policy surfaces so that:
- contribution review is predictable
- access control is enforceable
- moderation is auditable
- stake and reputation affect governance without becoming arbitrary

This spec favors narrow, explicit capabilities and append-only moderation logs over informal social convention.

---

## 2. Core Design Rules

1. **Policy is explicit**
Every community should publish a machine-readable policy manifest.

2. **Moderation is append-only**
Accept/reject/quarantine actions are recorded as immutable governance events.

3. **Capabilities are scoped**
Membership, review, payout approval, and service hosting should be separate permissions.

4. **Stake is bounded by governance rules**
Stake may gate eligibility or budget authority, but should not be the only source of content truth.

5. **Reputation and quorum matter**
Higher-trust actions should require reviewer agreement, not just unilateral authority.

---

## 3. Community Manifest

```rust
struct CommunityManifest {
    community_id: CommunityId,
    display_name: String,

    governance_mode: GovernanceMode,
    access_mode: AccessMode,
    receipt_policy: ReceiptPolicy,

    role_policy: RolePolicy,
    review_policy: ReviewPolicy,
    treasury_policy: TreasuryPolicy,
    moderation_policy: ModerationPolicy,

    created_at_ms: u64,
    version: u32,
}
```

---

## 4. Roles and Capabilities

```rust
enum CommunityRole {
    Member,
    Contributor,
    Reviewer,
    Moderator,
    Curator,
    Treasurer,
    Operator,
}

enum CommunityCapability {
    SubmitEngrams,
    ReviewSubmissions,
    ApproveCheckpoints,
    IssueReceipts,
    ApprovePayouts,
    ChangePolicy,
    BanMembers,
    HostPublicServices,
}
```

### 4.1 Best-Practice Role Separation

- `Reviewer` and `Treasurer` should be separable.
- `Moderator` should not automatically control treasury.
- `Operator` should control node/service policy, not content acceptance by default.
- `Curator` may approve checkpoints, but that action should still be auditable.

This avoids collapsing the whole community into a single “admin does everything” trust model.

---

## 5. Membership and Access

```rust
enum GovernanceMode {
    Open,
    Curated,
    Moderated,
}

enum AccessMode {
    PublicRead,
    PublicReadContributeReviewed,
    InviteOnly,
    StakeGated,
    Hybrid,
}
```

### 5.1 Recommended Defaults

- `PublicReadContributeReviewed` is the safest flexible default for public communities.
- `InviteOnly` is the safest default for high-trust or sensitive FLora domains.
- `StakeGated` should gate queue access or payout eligibility, not replace review.

---

## 6. Review and Quorum Policy

```rust
struct ReviewPolicy {
    min_reviews_for_accept: u32,
    min_reviews_for_checkpoint: u32,
    require_moderator_for_checkpoint: bool,
    allow_sparse_submission_acceptance: bool,
    first_time_contributor_requires_quarantine: bool,
}
```

### 6.1 Recommended Quorum Defaults

- Open low-risk community:
  - accept: 1 review
  - checkpoint: 2 reviews or curator approval

- Curated community:
  - accept: 2 reviews
  - checkpoint: 2 reviews + curator signature

- Moderated high-trust community:
  - accept: 2 reviews
  - checkpoint: 3 reviews + moderator/curator signature

---

## 7. Treasury Policy

```rust
struct TreasuryPolicy {
    payout_enabled: bool,
    epoch_budget_units: u64,
    max_single_payout_units: u64,
    require_multi_sig_above_units: Option<u64>,
}
```

### 7.1 Recommended Best Practice

- Use budgets and caps.
- Require multi-signature or multi-approver release above a configured threshold.
- Keep treasury policy changes on a slower path than day-to-day review decisions.

This reduces abuse and mirrors basic treasury controls from mature decentralized systems.

---

## 8. Appraisal and Legal-Risk Policy

Community policy should explicitly define how the verse appraises engrams and what legal-risk classes it accepts.

```rust
struct AppraisalPolicy {
    accepted_payload_classes: Vec<EngramPayloadClass>,
    accepted_derivation_types: Vec<EngramDerivationType>,
    preferred_udc_prefixes: Vec<String>,
    require_evals_for_payout: bool,
    require_attestation_for_high_risk: bool,
    legal_risk_policy: LegalRiskPolicy,
}

enum LegalRiskPolicy {
    DerivedOnly,
    DerivedPlusApprovedPublicSource,
    ExplicitOptInArchival,
}
```

### 8.1 Recommended Default

The safest general default is `DerivedOnly`.

That means the community accepts derived artifacts such as:
- adapter weights
- hashes and fingerprints
- extracted metadata
- UDC classification
- evals
- derived summaries

And rejects, by default:
- raw clips
- screenshots
- copied snippets
- direct personal corpora

### 8.2 Explicit Higher-Risk Publication

If a contributor intentionally publishes higher-risk source material, the community should treat that as an explicit publication choice with separate moderation and legal-risk handling.

Communities should not infer permission merely because a source was processable locally.

---

## 9. Moderation Events

```rust
struct GovernanceEvent {
    event_id: String,
    community_id: CommunityId,
    actor: PeerId,
    action: GovernanceAction,
    target_ref: String,
    reason_ref: Option<Cid>,
    created_at_ms: u64,
    signature: Signature,
}

enum GovernanceAction {
    MemberAdmitted,
    MemberSuspended,
    MemberBanned,
    SubmissionQuarantined,
    SubmissionAccepted,
    SubmissionRejected,
    CheckpointApproved,
    CheckpointRevoked,
    PayoutApproved,
    PolicyUpdated,
}
```

All moderation actions should be durable and reviewable.

---

## 10. Disputes and Appeals

```rust
struct AppealRequest {
    appeal_id: String,
    target_event_id: String,
    requester: PeerId,
    reason_ref: Option<Cid>,
    created_at_ms: u64,
}
```

Recommended process:
- allow appeal against bans, quarantines, major payout denials, and checkpoint revocations
- require the appeal to create a new governance event
- do not rewrite prior events; supersede them

---

## 11. Reputation, Stake, and Anti-Plutocracy

Recommended policy:
- stake may unlock queue access, treasury participation, or service commitments
- reputation should affect reviewer weight and trust
- checkpoint acceptance should still require explicit review rules

Avoid pure token-weighted acceptance of content.

Reason:
- stake measures collateral or budget commitment
- it does not prove content quality

This is the simplest way to avoid turning FLora into a pay-to-merge system.

---

## 12. Service Hosting Policy

Communities may expose:
- storage
- index segments
- FLora adapters
- applets
- feeds/forums
- shared process access points

Each service surface should have explicit enablement and moderation scope. Hosting rights should not be assumed for all members.

---

## 13. Immediate Defaults (v1)

- Require a published `CommunityManifest`
- Keep moderation append-only
- Separate review, treasury, and hosting capabilities
- Use reviewer quorum for checkpoints
- Use stake as gating/collateral, not sole truth
- Require explicit policy for bans, appeals, and treasury caps
- Require explicit appraisal policy and legal-risk policy
- Default legal-risk handling to `DerivedOnly`

These defaults are strict enough to be safe without requiring full on-chain governance machinery.
