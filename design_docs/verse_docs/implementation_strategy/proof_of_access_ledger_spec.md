# Proof of Access Ledger Specification

**Date**: 2026-02-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the recommended receipt, aggregation, epoch accounting, reputation, and optional payout model for storage, indexing, review, and FLora reward accounting in Verse communities.
**Related**:
- `design_docs/verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md`
- `design_docs/verse_docs/implementation_strategy/community_governance_spec.md`
- `design_docs/verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md`

---

## 1. Purpose

Proof of Access is the Verse accounting layer for:
- storage service
- retrieval service
- indexing service
- review/moderation work
- FLora contribution rewards

It should support:
- reputation-only operation
- off-chain ledger accounting
- optional token settlement later

The default recommendation is **off-chain-first, payout-second**.

---

## 2. Core Design Rules

1. **Receipts are evidence, not money**
Receipts prove service claims. They do not settle funds directly.

2. **Epoch batching**
Aggregate and settle in epochs rather than per-request.

3. **Off-chain default**
Keep v1 accounting in signed append-only ledgers, not live chain transactions.

4. **Payment channels over per-action settlement**
If token payouts are enabled, use escrowed channel-style settlement rather than on-chain transfer per event.

5. **Reputation always exists**
Even if financial payout is disabled, the ledger still computes reputation.

---

## 3. Receipt Types

```rust
enum AccessWorkType {
    BlobServed,
    IndexServed,
    FloraSubmissionAccepted,
    FloraCheckpointIncluded,
    ReviewCompleted,
    ModerationCompleted,
}

struct AccessReceipt {
    receipt_id: String,
    community_id: CommunityId,
    work_type: AccessWorkType,

    subject_ref: String,      // blob cid, submission id, checkpoint id, review id
    provider: PeerId,
    requester: Option<PeerId>,

    declared_units: u64,      // bytes, points, or policy-defined work units
    epoch_hint: u64,
    created_at_ms: u64,

    signature: Signature,
}
```

### 3.1 Unit Semantics

- `BlobServed`: bytes transferred
- `IndexServed`: bytes or query-weighted points
- `FloraSubmissionAccepted`: policy-weighted points
- `FloraCheckpointIncluded`: higher policy-weighted points
- `ReviewCompleted`: review points
- `ModerationCompleted`: moderation points

Each community defines the exact weighting schedule, but the receipt shape stays stable.

---

## 4. Ledger Model

```rust
struct LedgerEpoch {
    community_id: CommunityId,
    epoch_id: u64,
    opens_at_ms: u64,
    closes_at_ms: u64,

    receipts_root: Cid,
    settlements_root: Option<Cid>,
    finalized: bool,
}

struct LedgerEntry {
    entry_id: String,
    epoch_id: u64,
    beneficiary: PeerId,
    work_type: AccessWorkType,
    weighted_units: u64,
    reputation_delta: i64,
    payout_delta: Option<u64>,
}
```

### 4.1 Recommended Epoch Cadence

- small/private communities: daily or weekly
- active public communities: hourly to daily

Short enough for responsive accounting, long enough to avoid hot-path settlement.

---

## 5. Reputation Computation

Reputation should be computed from weighted, policy-filtered receipts.

Recommended factors:
- validated unique requesters
- successful service volume
- freshness (recent activity weighted more)
- dispute / invalid receipt rate
- moderator overrides for fraud or abuse

### 5.1 Reputation Decay

Use mild rolling decay rather than permanent accumulation.

Rationale:
- reduces stale oligarchies
- rewards recent useful participation
- aligns with community quality and responsiveness

---

## 6. Payout Model

### 6.1 Recommended v1 Default

- Keep payout **disabled by default**
- Keep full receipt and reputation accounting active
- Allow communities to enable explicit bounty or budget pools later

### 6.2 Optional Payout Path

If payout is enabled:
- community treasury escrows a fixed budget
- epoch finalization computes payout allocations
- beneficiaries claim against an escrowed channel or batched settlement record

This mirrors established decentralized-storage practice: negotiate and account off-chain, settle in coarse batches.

### 6.3 Filecoin-Aligned Guidance

For Filecoin-oriented communities:
- use Filecoin-backed treasury accounting as the budget source
- prefer payment-channel-like claim semantics instead of direct chain transactions per contribution
- treat on-chain interaction as settlement/audit, not live service coordination

---

## 7. Fraud and Abuse Controls

Required protections:
- duplicate receipt suppression
- nonce / unique receipt IDs
- signed requester acknowledgment where applicable
- epoch cutoffs
- claim dispute window
- per-peer rate limiting

Recommended anti-gaming rules:
- cap repeated receipts from the same requester in a short window
- weight unique requesters more heavily than repeated self-loop traffic
- allow communities to mark peers as colluding or low-trust
- separate “receipt accepted” from “payout eligible”

---

## 8. No-Receipt and Privacy Modes

Communities should allow work that does not generate economic receipts.

Examples:
- private peer exchange
- anonymous public caching
- reputation-disabled communities

```rust
enum ReceiptPolicy {
    RequiredForPayout,
    OptionalReputationOnly,
    Disabled,
}
```

This keeps the economic layer optional and prevents money logic from becoming mandatory for basic network use.

---

## 9. Disputes and Reversals

The ledger should support post-hoc correction without rewriting history.

```rust
enum LedgerDisputeOutcome {
    ConfirmedValid,
    Invalidated,
    ReducedWeight,
    Reassigned,
}
```

Rules:
- never delete original receipts
- append dispute and correction records
- future epochs may claw back reputation or payouts if policy allows

---

## 10. Immediate Defaults (v1)

- Signed append-only off-chain ledger
- Epoch-based aggregation
- Reputation on by default
- Token payout off by default
- Payment-channel-style settlement if payouts are enabled
- Fraud controls and dispute windows mandatory

These defaults keep the economic layer useful without dragging the Verse protocol into premature chain complexity.
