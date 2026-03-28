<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Verse Economic Model

**Date**: 2026-03-05
**Status**: Draft / Tier 2 research direction
**Scope**: Coherent sketch of the full economic model: storage staking, sats-denominated operational budgets, Filecoin treasury, reputation governance, and the full value loop from browsing to reward.

**Related docs**:

- [`../implementation_strategy/proof_of_access_ledger_spec.md`](../implementation_strategy/proof_of_access_ledger_spec.md) — receipt types, ledger mechanics, epoch accounting
- [`../implementation_strategy/community_governance_spec.md`](../implementation_strategy/community_governance_spec.md) — governance roles, treasury policy, anti-plutocracy rules
- [`../implementation_strategy/flora_submission_checkpoint_spec.md`](../implementation_strategy/flora_submission_checkpoint_spec.md) — FLora adapter pipeline
- [`2026-02-23_verse_tier2_architecture.md`](2026-02-23_verse_tier2_architecture.md) — Tier 2 transport, VerseBlob, community model
- [`2026-03-05_verse_nostr_dvm_integration.md`](2026-03-05_verse_nostr_dvm_integration.md) — NIP-90 DVM compute layer, sats payment flows
- [`../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md`](../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md) — storage bank: contributing, using, managing decentralized storage; credit mechanics, placement, durability, pledge-to-pool

---

## 1. Design Position: No Native Verse Token

A dedicated Verse token is explicitly deferred and may never be necessary. The reasons:

- Token launches require liquidity bootstrapping, exchange listings or AMM pools, regulatory exposure, and ongoing monetary policy governance. This is enormous overhead for a bootstrapping project.
- Two existing tokens already cover the roles a Verse token would play: **sats (Lightning)** for compute-on-demand, **Filecoin (FIL)** for durable storage commitments.
- The third incentive track — **governance weight** — is deliberately non-fungible (reputation, earned by community work, not purchasable). A Verse token would threaten this by making governance weight indirectly buyable.

The three-track model:

| Track | Currency | What it pays for | How earned | Fungible? |
| --- | --- | --- | --- | --- |
| Compute | sats (Lightning) | DVM inference jobs, crawler bounties, ad-hoc compute | Market rate, per-job | Yes |
| Storage | FIL | Long-term blob hosting commitments | Per epoch, via receipt aggregation | Yes |
| Governance | Reputation | Review weight, checkpoint authority, moderation standing | Community work receipts only | No |

These tracks are designed to be non-exploitable against each other: sats volume does not increase reputation, FIL stake does not grant checkpoint authority. See §8 (anti-plutocracy).

A Verse token can be reconsidered as a cross-community coordination mechanism after Tier 2 proves utility at scale. It is not a prerequisite for any feature in this doc.

---

## 2. Storage Staking: Hosting Rights in a Verse

### 2.1 The problem staking solves

In a public Verse community, anonymous peers may volunteer to host blobs (index shards, FLora checkpoints, VerseBlobs). Without collateral, a peer can promise hosting, collect reputation/payout, and then disappear. The community's data availability degrades silently.

**Storage staking** is collateral-backed hosting commitment: a peer pledges FIL proportional to the storage they commit. If they fail to serve, the bond is partially slashed after a dispute window.

### 2.2 Bond mechanics

```rust
pub struct StorageBond {
    provider: Did,              // did:key of the hosting peer
    community_id: CommunityId,
    committed_bytes: u64,       // storage capacity pledged
    bond_amount_fil: u64,       // FIL pledged as collateral (attoFIL units)
    bond_address: FilecoinAddress, // on-chain escrow address
    epoch_start: u64,
    status: BondStatus,
}

pub enum BondStatus {
    Active,
    GracefulExit { exit_epoch: u64 },  // provider announced exit, wind-down period
    Slashed { slash_epoch: u64, reason: SlashReason },
    Released,
}

pub enum SlashReason {
    ServiceLapseConfirmed,  // receipts stopped, dispute window passed
    MaliciousContent,       // served content that failed integrity check
    PolicyViolation,        // community governance voted to slash
}
```

**Bond sizing guidance**: Communities set their own bond-per-byte ratio in `TreasuryPolicy`. A reasonable starting point is the Filecoin storage deal collateral convention: bond ≥ estimated 20-epoch revenue from the commitment.

### 2.3 When staking is required vs. optional

| Community type | Staking requirement |
| --- | --- |
| Private / personal Verse | Not required. Host bears costs voluntarily. No anonymous peers. |
| Small trusted group | Optional. Members may choose to stake to signal commitment. |
| Public Verse, open membership | Required for peers claiming storage receipts and payout. |
| Public Verse, reputation-only | No payout → no staking required; hosting is altruistic. |

Staking is **not** required for Tier 1 (Device Sync). It is a Tier 2 concept that applies only when anonymous public hosting is involved.

### 2.4 Graceful exit

A bonded provider announces exit by setting `GracefulExit { exit_epoch }`. During the exit window:
- They continue serving data (receipts continue accumulating).
- Other providers have time to replicate their shards.
- At `exit_epoch`, if shard coverage is confirmed by at least k-1 other providers, the bond is released in full.
- If coverage is not confirmed, a partial slash is applied before release.

### 2.5 Relationship to Filecoin deals

Verse storage bonds are **not** Filecoin storage deals — they do not use Filecoin's Proof of Replication or Proof of Spacetime machinery. They are a simpler receipt-based system: providers earn receipts by serving data; the bond is collateral against non-performance detected via receipt absence.

If a community wants Filecoin-level cryptographic storage proofs, it can run a Filecoin storage deal alongside the Verse bond. This is a policy option, not a requirement.

---

## 3. Reputation: the Third Track

### 3.1 Why reputation is a vector, not a scalar

A single reputation score is gameable and opaque: high storage volume can obscure poor review quality; historical contribution can permanently entrench early members. A **vector of domain-specific scores** is harder to game (you cannot substitute storage reputation for review authority) and more informative (communities can weight dimensions differently).

```rust
pub struct ReputationVector {
    storage:  f32,  // BlobServed + IndexServed receipts, unique-requester weighted
    review:   f32,  // ReviewCompleted + ModerationCompleted receipts, outcome-adjusted
    flora:    f32,  // FloraSubmissionAccepted + CheckpointIncluded, lineage-weighted
    compute:  f32,  // ComputeCompleted receipts, client-feedback-adjusted
    longev:   f32,  // rolling tenure signal — consistent presence over time
}
```

Each dimension feeds only from its own receipt type. Doing more storage work does not inflate `review`. The dimensions are computed independently per epoch and composed into governance weight only at the point of a specific action.

### 3.2 Quality, not just quantity

Quantity alone is gameable. Each dimension incorporates a quality signal:

**Storage quality** (`storage`): unique requester count weights more than repeat traffic. Serving 1 TB to the same peer scores nearly the same as serving them once. Serving 100 GB to 500 unique peers scores much higher. This is already in the Proof of Access ledger spec; the reputation vector makes it explicit.

**Review quality** (`review`): a reviewer's score is retroactively adjusted by the downstream fate of their decisions. If a checkpoint you approved is later revoked via appeal, your `review` score takes a hit. If your approvals consistently hold across epochs, your score rises. The governance log is append-only and provides the audit trail for this feedback.

**FLora quality** (`flora`): a submission earns base points on acceptance. It earns **lineage bonus points** if a later checkpoint lists it in `merged_from` — i.e. the community built on your contribution. Accepted submissions that are never built upon fade with decay. This rewards durable knowledge contributions over one-off submissions.

**Compute quality** (`compute`): DVM result quality is rated by requesting clients via NIP-90 kind 7001 feedback events. Sustained negative feedback reduces compute reputation. Operators with high ratings are preferred in community-subsidised job routing.

**Longevity** (`longev`): a slow-accumulating, slow-decaying signal representing consistent long-term presence. Cannot be boosted by burst activity. Rewards reliability over time without letting it dominate newer contributors.

### 3.3 Decay

Each dimension decays independently per epoch using a configurable factor:

```
new_score = old_score * decay_factor + new_receipts_weighted
```

Recommended defaults (community-configurable in `ReputationSpec`):

| Dimension | Decay per epoch | Rationale |
| --- | --- | --- |
| `storage` | 0.92 | Fast-moving — reward current availability |
| `review` | 0.95 | Moderate — reward sustained engagement |
| `flora` | 0.97 | Slow — knowledge contributions have long value |
| `compute` | 0.90 | Fast — reward recent DVM quality |
| `longev` | 0.99 | Very slow — presence signal should persist |

At a daily epoch cadence, `storage` at 0.92 means ~60-day half-life; `longev` at 0.99 means ~1.9-year half-life. These are starting points, not fixed values.

### 3.4 Governance weight derivation

From the reputation vector, governance weight for a specific action type is computed by a community-configured weight matrix:

```rust
pub struct ReputationSpec {
    decay_per_epoch: HashMap<ReputationDimension, f32>,
    governance_weight_matrices: HashMap<GovernanceAction, WeightRow>,
    new_identity_quarantine_epochs: u32,
    unique_requester_discount_threshold: u32,
}

pub struct WeightRow {
    storage: f32,
    review:  f32,
    flora:   f32,
    compute: f32,
    longev:  f32,
}
```

Example matrices (community-adjustable defaults):

| Action | storage | review | flora | compute | longev |
| --- | --- | --- | --- | --- | --- |
| Checkpoint approval | 0.05 | 0.60 | 0.25 | 0.00 | 0.10 |
| Moderation decision | 0.15 | 0.40 | 0.05 | 0.00 | 0.40 |
| Storage payout priority | 0.70 | 0.00 | 0.00 | 0.00 | 0.30 |
| DVM operator trust rank | 0.00 | 0.05 | 0.05 | 0.80 | 0.10 |
| Community policy vote | 0.10 | 0.30 | 0.20 | 0.10 | 0.30 |

A community that values long-term members over recent burst contributors can increase `longev` weights. A high-throughput indexing community can increase `storage` weight for payout priority. The matrix is published in `CommunityManifest` and changes via governance vote.

### 3.5 Sybil resistance

New identities start in a **quarantine period** (configurable epochs, e.g. 3) during which their receipts generate reduced reputation (e.g. ×0.1 weight). They can participate but cannot immediately influence governance. This creates a time cost for Sybil attacks.

Additional mitigations:

- **Unique requester discount**: reputation gain is discounted when receipts come from a small, repetitive requester set (below `unique_requester_discount_threshold`). A Sybil cluster exchanging receipts internally earns little.
- **Cross-validation visibility**: the governance log records which identities approved which submissions. Clusters of new identities approving each other's work are visible in the log and disputable.
- **Bootstrap stake** (§6.3): communities that require FIL stake for reputation-generating participation raise the economic cost of Sybil attacks proportionally.

### 3.6 Reputation is local to a community

A member's `ReputationVector` is computed per-community from that community's receipt ledger. High reputation in one Verse does not transfer to another. This prevents reputation laundering across communities and keeps governance weight meaningful to the community that earned it.

Cross-community reputation portability (e.g. a "known contributor" badge from one community influencing quarantine duration in another) is a deferred research problem — not in scope for initial Tier 2.

---

## 4. Operational Budget: Sats for a Verse Community

### 3.1 The community Lightning wallet

A Verse community holds a Lightning wallet addressed by its canonical keypair (the same keypair as the community's Nostr `npub`, if Nostr is in use). This is the operational budget — a pool of sats spent on community services.

Managed via NIP-47 (Nostr Wallet Connect) for communities using Nostr-integrated tooling, or directly via the community operator's Lightning node for simpler setups.

### 3.2 What the budget pays for

| Expense | Mechanism | Typical size |
| --- | --- | --- |
| DVM compute jobs (inference, curation, summarisation) | NIP-57 Zap to DVM operator per job result | Small (a few sats per query) |
| Crawler bounties (web content ingestion) | Direct Lightning payment on receipt validation | Medium (proportional to content size + bandwidth) |
| FLora reviewer tips (optional, above reputation) | Epoch payout from treasury | Small to medium |
| Nostr relay hosting (if community runs its own) | Ongoing relay operator payment | Fixed monthly |
| Rendezvous / bootstrap node operation | Ongoing node operator payment | Fixed monthly |

DVM jobs are **always paid at point of service** — the requesting client pays the DVM directly. The community treasury can optionally subsidise DVM jobs for members (e.g., the first N queries per member per epoch are paid by the treasury).

### 3.3 Budget funding sources

| Source | Mechanism |
| --- | --- |
| Member contributions | Members send sats to the community Lightning address (voluntary or required for membership) |
| Access fees | Joining the community, or fetching the community LoRA, costs a small fee |
| Index query revenue | External queriers pay sats for DVM jobs that use the community's index/LoRA |
| Crawler economy | Community's index is popular → other communities pay for access → revenue flows in |
| Donations / grants | External funders send sats to the community address |

### 3.4 Multi-sig treasury controls

Above a configured threshold (`TreasuryPolicy.require_multi_sig_above_units`), any expenditure requires multi-approver sign-off from the Treasurer role set. This mirrors the existing governance spec requirement.

For small communities (< a few hundred sats threshold), single-approver treasury management is acceptable. For communities with significant FIL + sats reserves, multi-sig is mandatory.

---

## 5. FIL Treasury: Long-Term Storage Commitment

### 4.1 Why FIL, not sats

Sats (Lightning) are optimised for fast, cheap, per-job payments. They are not suitable for long-term storage commitments: a Lightning channel requires both parties to be online for settlement, and channel capacity is limited.

Filecoin is designed for durable storage economics: time-locked deals, on-chain escrow, cryptographic proofs of continued storage. A community's FIL treasury is the backstop for:
- Storage bond escrow (§2)
- Epoch-based storage payout to bonded providers
- Long-term archival of community VerseBLOBs, FLora checkpoints, and index epochs

### 4.2 FIL treasury mechanics

```rust
pub struct FilecoinTreasury {
    community_id: CommunityId,
    on_chain_address: FilecoinAddress,
    current_balance_attoFIL: u64,
    epoch_budget_attoFIL: u64,      // max payout per epoch
    active_bonds: Vec<StorageBond>, // currently bonded providers
    epoch_settlements: Vec<EpochSettlement>,
}

pub struct EpochSettlement {
    epoch_id: u64,
    total_receipts_weighted: u64,
    total_payout_attoFIL: u64,
    beneficiary_payouts: Vec<(Did, u64)>, // (provider, attoFIL)
    finalized: bool,
    receipts_root: Cid,
}
```

### 4.3 Funding the FIL treasury

| Source | Notes |
| --- | --- |
| Initial community stake | Founding members seed the treasury to bootstrap hosting |
| Ongoing member dues | Optional: members pay FIL to maintain access to community data |
| Crawler economy FIL revenue | If community sells access to its index via FIL-denominated deals |
| Grants / external funding | External parties fund communities of public interest |

### 4.4 Payout disabled by default

Per the Proof of Access ledger spec: payout is **disabled by default**. Communities start with reputation-only accounting. FIL payout is enabled explicitly in `TreasuryPolicy.payout_enabled` when the community is ready to manage treasury operations. This prevents premature economic complexity from blocking community formation.

---

## 6. The Full Value Loop

```
┌─ Member browses ──────────────────────────────────────────────┐
│                                                               │
│  Graph nodes generated from navigation                        │
│  Browsing history stays local (never shared raw)              │
│                                                               │
└──────────────────────┬────────────────────────────────────────┘
                       │ local mini-adapter pass
                       ▼
┌─ Local model adaptation ──────────────────────────────────────┐
│                                                               │
│  TransferProfile generated (weights + metadata, no raw data)  │
│  Member decides to submit as FLora contribution               │
│                                                               │
└──────────────────────┬────────────────────────────────────────┘
                       │ FloraSubmission published (Bitswap)
                       ▼
┌─ Community review ────────────────────────────────────────────┐
│                                                               │
│  Reviewers evaluate (quorum per GovernanceMode)               │
│  ReviewCompleted receipts → reputation delta for reviewers    │
│  Accepted → FloraCheckpoint approved                          │
│  kind 4550 Nostr announcement published                       │
│                                                               │
└──────────────────────┬────────────────────────────────────────┘
                       │ checkpoint available in DHT
                       ▼
┌─ Storage provision ───────────────────────────────────────────┐
│                                                               │
│  Bonded providers fetch checkpoint blob via Bitswap           │
│  BlobServed receipts → reputation + FIL payout at epoch close │
│  Bond collateral guarantees continued availability            │
│                                                               │
└──────────────────────┬────────────────────────────────────────┘
                       │ member queries community
                       ▼
┌─ Compute on demand ───────────────────────────────────────────┐
│                                                               │
│  Member submits NIP-90 DVM job (traversal suggestion,         │
│  feed curation, node summarisation)                           │
│  DVM fetches checkpoint → runs inference → returns result     │
│  DVM earns sats (Lightning Zap) immediately                   │
│  ComputeCompleted receipt → DVM reputation delta              │
│  Member sees results: ghost nodes, curated feed, annotations  │
│                                                               │
└──────────────────────┬────────────────────────────────────────┘
                       │ epoch closes
                       ▼
┌─ Epoch settlement ────────────────────────────────────────────┐
│                                                               │
│  Storage receipts aggregated → FIL payout (if enabled)        │
│  Reviewer receipts aggregated → reputation updated            │
│  Governance weight recalculated from rolling reputation       │
│  Reputation decay applied (prevents stale oligarchy)          │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

---

## 7. Additional Staking Types

Beyond storage bonds, two other staking types are worth noting:

### 6.1 Contributor stake (quality signal)

A FLora contributor can optionally attach a stake to their submission — a small FIL amount held in escrow during the review window. If the submission is accepted, the stake is returned plus a small bonus from the community treasury (if payout is enabled). If rejected for low quality (not policy violation), the stake is burned or redistributed to reviewers.

This is **opt-in and additive**: the review quorum process always determines acceptance. Stake is a quality signal and economic skin-in-the-game, not a bypass.

Rule: contributor stake ≠ acceptance guarantee. Anti-plutocracy principle preserved.

### 6.2 Reviewer stake (accountability)

Reviewers can optionally stake against their reviews. If a review is later overturned via appeal (GovernanceAction outcomes), a portion of the reviewer's stake is slashed. This creates accountability for reviewers who approve low-quality work or reject high-quality work for non-policy reasons.

Again, opt-in. Unstaked reviewing is always permitted; staked reviewing earns additional reputation weight per review.

### 6.3 Bootstrap stake (community formation)

When a new Verse community is created, founding members collectively stake FIL to signal commitment and seed the treasury. This stake is held for a minimum epoch window before withdrawal rights unlock, preventing pump-and-dump community formation.

Bootstrap stake parameters are in `TreasuryPolicy`. A community can set bootstrap stake to zero (fully open formation) — the tradeoff is lower initial credibility for anonymous joining peers.

---

## 8. Anti-Plutocracy Guarantees

These rules are non-negotiable and must survive any future token or economic layer additions:

1. **Sats volume ≠ reputation.** Paying for more DVM jobs does not increase a member's governance weight.
2. **FIL bond size ≠ checkpoint authority.** A larger storage bond does not grant the right to approve FLora submissions.
3. **Stake ≠ acceptance.** No amount of contributor or reviewer stake can substitute for the review quorum process.
4. **Treasury balance ≠ policy control.** A member who funds the treasury generously does not thereby gain `Operator` or `Moderator` role.
5. **Reputation decays.** Historical contribution does not permanently lock in governance authority. Recent participation matters.

Governance weight comes from doing community work — reviewing, moderating, indexing, hosting — not from financial position. The economic layer compensates providers; it does not grant authority over content or policy.

---

## 9. Relationship Between Tracks

```
sats (Lightning)
  ↓ pays
DVM operators, crawlers, relay operators
  (no reputation effect, no governance weight)

FIL (Filecoin)
  ↓ bonds / pays
Storage providers (bonded)
  ↓ receipt accumulated
Reputation delta (BlobServed, IndexServed)
  ↓ over time
Governance weight (search ranking, curation eligibility)

Reputation (no money)
  ↓ earned by
Review work, moderation, FLora contribution acceptance
  ↓ over time
Governance weight (checkpoint authority, moderation standing)
```

The only path from money to governance authority is:
`FIL bond → serve data reliably → BlobServed receipts → reputation → governance weight`

This path is slow (multiple epoch cycles), active (requires ongoing work), and decay-limited (historical reputation fades). It cannot be shortcut by buying more FIL.

---

## 10. Decentralized Storage Bank

The storage bank is the **operational layer** of the storage track — it answers
how storage is contributed, allocated, verified, and health-monitored. The full
specification is in
[`2026-03-28_decentralized_storage_bank_spec.md`](../implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md).
This section summarizes how the storage bank integrates with the economic model.

### 10.1 Credit Mechanics: Base + Bonus

The storage bank introduces two new `AccessWorkType` variants that extend the
PoA ledger's storage track:

- **`BlobAvailabilityEpochHeld`**: Small, steady base credit for passing
  periodic availability challenges. Prevents the long-tail death problem where
  unpopular (but valuable) data loses all storage incentive.
- **`BlobRetrievalServed`**: Usage-validated bonus credit on real retrieval,
  scaled by **hold duration** — the longer you held a blob before serving it,
  the more the retrieval is worth. This is the "usage validates storage time"
  primitive.

Both feed into the `storage` dimension of `ReputationVector`. The base-vs-bonus
ratio is community policy (see storage bank spec §3.5).

### 10.2 Bilateral Storage Visibility

At the Verso (bilateral) level, peers report storage usage to each other
without enforcement:

```rust
struct PeerStorageReport {
    peer_id: Did,
    bytes_i_hold_for_peer: u64,
    bytes_peer_holds_for_me: u64,
    held_blob_cids: Vec<Cid>,
    last_verified_at_ms: u64,
}
```

This is the storage bank at n=2, where trust substitutes for bonds and
reputation. No credit intermediary — peers see the imbalance and negotiate
informally. The bilateral model is the foundation from which community-scale
storage banking grows.

### 10.3 Pledge-to-Pool

Storage credits are **non-transferable** — no trading, no exchange rate. But
peers can **pledge** portions of earned credit to a community storage pool that
backs shared services (Matrix rooms, shared workspaces, FLora checkpoints).

Pledging is a directed allocation, not a transfer. It does not create
cross-track fungibility: storage credits cannot be converted to sats, FIL, or
reputation. The three-track separation is preserved.

See storage bank spec §9 for `StoragePledge` and `CommunityStoragePool`.

### 10.4 Fallback Hierarchy

Storage hosting degrades gracefully:

```
Community storage bank → Bilateral peer hosting → Self-hosting
```

The same CIDv1-addressed data can exist at any level. Blobs promote up the
hierarchy as more hosting becomes available and demote down as hosting is lost.
Health indicators show the user where their data stands.

---

## 11. Open Problems

> Note: the storage bank spec introduces its own open questions around
> challenge protocol tuning, erasure coding rollout, and repair incentive
> calibration — see storage bank spec §13.

1. **Filecoin light client**: Verse nodes need to interact with the Filecoin chain (bond escrow, epoch settlement). A full Filecoin node is impractical on a desktop. A light client or gateway approach is needed — no design yet.

2. **Sybil resistance for storage bonds**: An attacker can create many `did:key` identities, each posting a small bond, collectively controlling a large share of community data. Mitigation options: minimum bond size, rate limiting per identity per epoch, community-operator allowlist for bonded providers. No finalized design.

3. **FIL/sats exchange rate volatility**: Treasury budgets denominated in FIL fluctuate against sats. A community that budgets 1 FIL/epoch for payouts may find that FIL appreciating sharply reduces the number of providers willing to accept FIL at the old rate. Mitigation: denominate budgets in both FIL and sats, let the community vote to rebalance. Complex; deferred.

4. **Bootstrap chicken-and-egg**: A new community with no index, no LoRA, and no bonded providers is not useful. Members must contribute before they can benefit. Mitigation: founding members can contribute pre-seeded FLora checkpoints and index shards; bootstrap stake gives early members economic incentive to show up. But this still requires motivated founders.

5. **Privacy of contributor stake**: If a contributor stakes FIL against a specific FLora submission, and the submission is linked to their `npub` / `did:key`, their financial position is partially revealed on-chain. For contributors who care about pseudonymity, this is a concern. Possible mitigation: anonymous submission with delayed stake reveal (ZK proof that the submitter has stake). Significant complexity.

---

## 12. Rollout Sequence

This is all Tier 2, after Tier 1 (Device Sync) is stable:

1. **Reputation-only** (no money): Proof of Access receipts, epoch ledger, governance weight. No FIL, no sats. Validates the feedback loop without economic risk.
2. **Sats compute** (NIP-90 DVMs): DVM operators paid in sats per job. Community treasury optionally subsidises member queries. This requires only Lightning — no Filecoin.
3. **FIL treasury + storage payout**: Community enables `payout_enabled`, seeds FIL treasury, bonded providers earn FIL at epoch close. Requires Filecoin light client decision.
4. **Storage staking**: Bonded providers post FIL collateral. Enables slash mechanics and graceful exit. Higher trust, higher accountability.
5. **Contributor/reviewer staking** (optional, community-level config): Staked reviews and submissions for communities that want stronger accountability signals.
6. **Bootstrap stake** (optional, for new community formation): Founding member commitment stake.
