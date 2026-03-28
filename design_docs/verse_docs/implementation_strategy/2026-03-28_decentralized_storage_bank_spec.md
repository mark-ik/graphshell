# Decentralized Storage Bank Specification

**Date**: 2026-03-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the operational layer for decentralized storage in Verse —
how storage is contributed, allocated, used, health-monitored, and accounted
for. Sits between the PoA ledger (accounting) and VerseBlob (addressing).
**Related**:
- `design_docs/verse_docs/implementation_strategy/proof_of_access_ledger_spec.md`
- `design_docs/verse_docs/implementation_strategy/verseblob_content_addressing_spec.md`
- `design_docs/verse_docs/implementation_strategy/self_hosted_verse_node_spec.md`
- `design_docs/verse_docs/implementation_strategy/community_governance_spec.md`
- `design_docs/verse_docs/technical_architecture/2026-03-05_verse_economic_model.md`
- `design_docs/verse_docs/technical_architecture/VERSE_AS_NETWORK.md`
- `design_docs/verso_docs/technical_architecture/VERSO_AS_PEER.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../graphshell_docs/research/2026-03-04_standards_alignment_report.md)):
- **W3C DID Core 1.0** — all identity fields use `did:key`
- **IPFS CIDv1** — blob and fragment addressing
- **RFC 4122 UUID v7** — receipt and announcement IDs (time-ordered)

---

## 1. Purpose

The storage bank is the **operational layer** of Verse's decentralized storage
economy. It answers three questions the existing specs leave open:

1. **Contributing**: How does a peer offer storage capacity to the network, and
   how is that contribution verified and credited?
2. **Using**: How does a peer place data on the network and consume storage
   capacity others have contributed?
3. **Managing**: How is the collective state of the storage bank observed,
   reported, and maintained?

The storage bank is not a server. It is a **distributed accounting view**
reconstructed from signed receipts, health announcements, and challenge
responses. Every participating peer holds a local projection of the bank's
state, synchronized via the same epoch-based ledger model used by the PoA spec.

### 1.1 Relationship to Existing Economic Model

The storage bank extends the **FIL/storage track** of the three-track economic
model (sats for compute, FIL for storage, reputation for governance). It does
not create a new economic track, merge existing tracks, or introduce a new
token. All anti-plutocracy guarantees from the economic model remain in force:
storage credit does not translate into governance authority except through the
existing slow path (reliable service → `BlobServed` receipts → reputation →
governance weight).

---

## 2. The Storage Bank Model

### 2.1 Three Roles

| Role | What it does | Who plays it |
|---|---|---|
| **Contributor** | Offers storage capacity; hosts blobs; responds to availability challenges | Any peer with a `StorageBond` (community) or bilateral agreement (Verso) |
| **Consumer** | Places data in the bank; pays for hosting via credit or pledge | Any peer with data that needs redundant hosting |
| **Manager** | The local ledger + community health aggregation; no central authority | Every participating peer maintains their own view |

A single peer typically plays all three roles simultaneously: contributing
capacity for others while consuming capacity for their own data, and
maintaining their local ledger view.

### 2.2 The Bank Is a View, Not an Entity

There is no "storage bank server." The bank is the emergent state of:
- Signed `StorageAnnouncement`s (who hosts what)
- Signed `StorageHeartbeat`s (periodic health reports)
- Signed `AccessReceipt`s (credit events)
- The community's epoch-aggregated ledger

Each peer reconstructs the bank's state from these signed messages. Consistency
comes from the epoch finalization model already specified in the PoA ledger.

---

## 3. Credit Mechanics

The core design principle: **usage validates storage time**. Storing data is
not valuable in itself — it becomes valuable when someone retrieves data that
has been held reliably over time. However, to prevent the long-tail death
problem (unpopular data losing all storage incentive), credit has two layers.

### 3.1 Base Credit: Availability

```rust
AccessWorkType::BlobAvailabilityEpochHeld
```

A small, steady credit issued at epoch close for each blob the provider passed
availability challenges on during that epoch.

- **Challenge protocol**: A challenger (any community peer, or an automated
  community role) sends a lightweight random byte-range request: "serve me
  bytes `offset..offset+length` of blob `cid`." Provider must respond with the
  correct bytes within a community-configured timeout.
- **Challenge frequency**: Community policy. Recommended: 1 challenge per
  commitment per epoch for active communities; less frequent for small/private
  communities.
- **Failure consequence**: No base credit for that epoch + reputation penalty.
  Repeated failures across epochs trigger `CommitmentStatus::Defaulted` on the
  provider's `StorageBond`.
- **Credit formula**: `base_credit = blob_size_bytes × base_rate_per_byte_epoch`
  where `base_rate_per_byte_epoch` is community-configurable.

Base credit keeps providers honest and incentivized even for cold data that
nobody retrieves for long periods. It is the floor that prevents the long-tail
death problem.

### 3.2 Usage Bonus: Retrieval-Validated Credit

```rust
AccessWorkType::BlobRetrievalServed
```

Additional credit issued when a real user retrieves data the provider has been
holding. The bonus scales with **hold duration** — the longer you held a blob
before serving it, the more the retrieval is worth.

- **Hold-duration weighting**: The core "usage validates storage time"
  primitive. A provider who stored a blob for 6 months and then served it
  provided more value than one who cached it 5 minutes ago. The duration is
  measured from the provider's first `StorageAnnouncement` for that blob to
  the retrieval timestamp.
- **Bonus formula**:
  ```
  usage_bonus = bytes_served × base_rate × min(hold_epochs, cap) × bonus_multiplier
  ```
  where `cap` and `bonus_multiplier` are community-configurable.
- **Anti-gaming**: Only receipts with a valid `requester` signature count.
  Self-retrieval (provider == requester) is excluded from bonus calculation.
  Unique-requester weighting from the existing PoA spec applies.

### 3.3 Repair Credit

```rust
AccessWorkType::BlobRepairCompleted
```

Credit for re-hosting under-replicated blobs. When a blob's replica count
drops below `k_target`, repair workers who fetch and re-host it earn repair
credit. This incentivizes the network to self-heal.

- **Eligibility**: Provider must have an active `StorageBond` and must
  successfully host the blob through at least one challenge cycle.
- **Credit**: Comparable to base credit but may carry a community-configurable
  repair multiplier to prioritize repair work.

### 3.4 Receipt Schema Extension

The existing `AccessReceipt` gains one new optional field:

```rust
struct AccessReceipt {
    receipt_id: Uuid,              // UUID v7
    community_id: CommunityId,
    work_type: AccessWorkType,
    subject_ref: String,           // blob CID
    provider: Did,                 // did:key
    requester: Option<Did>,        // did:key
    declared_units: u64,           // bytes or byte-epochs
    hold_duration_epochs: Option<u64>,  // NEW: epochs held before serving
    epoch_hint: u64,
    created_at_ms: u64,
    signature: Signature,
}
```

`hold_duration_epochs` is populated for `BlobRetrievalServed` and
`BlobRepairCompleted` receipts. It is `None` for `BlobAvailabilityEpochHeld`
(where duration is implicitly 1 epoch).

### 3.5 Base vs. Bonus Ratio

The ratio between base credit (availability) and usage bonus (retrieval) is
**community policy**, not protocol-level. Recommended starting points:

| Community type | Base weight | Bonus weight | Rationale |
|---|---|---|---|
| Archival / cold storage | High (0.7) | Low (0.3) | Data is rarely retrieved; base must sustain providers |
| Active knowledge community | Medium (0.4) | High (0.6) | Retrieval is frequent; usage signal is strong |
| Ephemeral / cache-heavy | Low (0.2) | High (0.8) | Data turns over fast; long-hold bonus is less relevant |

---

## 4. The Fallback Hierarchy

Storage hosting degrades gracefully across three levels. The same data,
addressed by the same CIDv1, can exist at any level. Only the hosting
commitment and credit model change.

```
Community storage bank  (credit-based, k-of-n, health-monitored)
    ↓ demotion: community shrinks / blob under-replicated / user opts out
Bilateral peer hosting   (trust-based, visibility-reported, informal reciprocity)
    ↓ demotion: peer offline / relationship ends / no willing peers
Self-hosting             (always works, full control, no external redundancy)
```

### 4.1 Promotion

A blob moves up the hierarchy when more hosting becomes available:

| Transition | Trigger | Mechanism |
|---|---|---|
| Self → Bilateral | User agrees with a peer to host each other's data | Verso bilateral agreement; blob appears in `PeerStorageReport` |
| Self → Community | User submits blob to community replication queue | `StorageRequest` posted; providers pull and announce |
| Bilateral → Community | Bilateral-hosted blob gets community-level hosting | Additional providers announce; community health tracking begins |

### 4.2 Demotion

A blob moves down when hosting is lost:

| Transition | Trigger | User experience |
|---|---|---|
| Community → Bilateral | `actual_k` drops to 0 at community level but bilateral peer still holds it | Health indicator changes from "community-replicated" to "peer-hosted" |
| Community → Self | `actual_k` drops to 0 and no bilateral peer holds it | Health indicator changes to "self-hosted only"; user prompted to take action |
| Bilateral → Self | Peer goes permanently offline or withdraws | Health indicator changes; blob is now local-only |

### 4.3 Addressing Continuity

CIDv1 addressing is the same at all levels. A blob's identity does not change
when it moves between hosting levels. The only thing that changes is the set of
providers and the credit/trust model governing them.

---

## 5. Bilateral Storage Budgeting (The Microcosm)

Two peers sharing storage over Verso is the simplest storage bank: n=2, trust
substitutes for bonds and reputation, direct reciprocity replaces credit
intermediation.

### 5.1 Visibility Model

Verso reports per-peer storage usage without enforcing limits:

```rust
struct PeerStorageReport {
    peer_id: Did,
    bytes_i_hold_for_peer: u64,
    bytes_peer_holds_for_me: u64,
    held_blob_cids: Vec<Cid>,
    last_verified_at_ms: u64,
}
```

Each peer sees:
- How much of their data the other peer is holding
- How much of the other peer's data they are holding
- The imbalance (if any)

### 5.2 No Enforcement

Verso does not enforce bilateral storage quotas. Peers see the imbalance and
negotiate informally. Trust handles free-riding at n=2 — if your friend is
taking advantage, you stop hosting their data. This is the same social
enforcement that makes BitTorrent private trackers work.

### 5.3 Compatibility with Community-Scale Structures

The bilateral data model is intentionally compatible with the community-scale
storage bank. A `PeerStorageReport` is a degenerate storage bank view with n=2
and no credit intermediary. If both peers later join a community, their
bilateral hosting can be promoted to community-level hosting without re-placing
the data.

---

## 6. Placement and Assignment

### 6.1 No Global Placement Engine

The storage bank does not include a centralized or algorithmic placement engine.
Placement is driven by **provider self-selection** from a community-managed
**replication queue**, combined with community priority signaling.

This follows the BitTorrent model (peers choose what to seed) rather than the
Ceph/Filecoin model (system assigns placement). It is simpler, requires no
coordinator, and preserves provider autonomy.

### 6.2 The Replication Queue

When a blob needs hosting, it enters the community's replication queue:

```rust
struct ReplicationQueueEntry {
    blob_cid: Cid,
    size_bytes: u64,
    priority_class: PlacementPriority,
    current_k: u32,
    k_target: u32,
    requester: Did,
    entered_at_epoch: u64,
}

enum PlacementPriority {
    Critical,       // FLora checkpoints, governance records
    CommunityPinned,// explicitly pinned by community policy
    Standard,       // shared workspaces, rooms
    Cached,         // opportunistic; evictable
}
```

Queue ordering: `(priority_class DESC, under_replication_score DESC, age ASC)`.
Under-replication score = `k_target - current_k`.

### 6.3 Provider Pull Model

Providers with active `StorageBond`s poll the replication queue and choose
blobs to host. After fetching and verifying a blob, the provider publishes a
signed `StorageAnnouncement`:

```rust
struct StorageAnnouncement {
    announcement_id: Uuid,     // UUID v7
    provider: Did,
    community_id: CommunityId,
    blob_cid: Cid,
    fragment_index: u32,       // for erasure coding; 0 for full-copy
    announced_at_epoch: u64,
    signature: Signature,
}
```

The community aggregates announcements to update the health view. A blob's
`current_k` increments when a new provider announces hosting.

### 6.4 Community Priority Signaling

Communities can signal priority (e.g., "checkpoints before raw submissions")
but cannot force placement. Providers retain the right to choose which blobs
they host, subject to their `StorageContributionBudget` constraints.

---

## 7. Data Durability

### 7.1 Redundancy Target

Each community sets a `k_target` (default 3): the desired number of
independent providers hosting each blob. The health monitor tracks `actual_k`
per blob.

### 7.2 Fragment Model (Erasure-Coding-Ready)

Every blob has a `FragmentManifest` that lists how its data is split for
redundancy purposes:

```rust
struct FragmentManifest {
    blob_cid: Cid,                    // the original blob CID
    coding_scheme: CodingScheme,
    fragments: Vec<FragmentEntry>,
    k_required: u32,                  // fragments needed to reconstruct
    m_total: u32,                     // total fragments produced
}

enum CodingScheme {
    FullCopy,                         // v1: each fragment = the full blob
    ReedSolomon { data: u32, parity: u32 },  // future: k-of-m erasure coding
}

struct FragmentEntry {
    fragment_cid: Cid,
    fragment_index: u32,
    size_bytes: u64,
}
```

**v1 implementation**: `CodingScheme::FullCopy`. Each fragment is a complete
copy of the blob. `k_required = 1`, `m_total = k_target`. This is naive
k-replication, but the interfaces are designed so that switching to
Reed-Solomon later requires no structural changes to announcements, health
reporting, or repair protocols.

**Future**: `CodingScheme::ReedSolomon { data: k, parity: m-k }`. A blob is
split into `m` fragments where any `k` suffice to reconstruct. This gives
`m`-provider redundancy at `m/k` storage overhead instead of `m×` overhead.
For example, 4-of-8 coding gives 8-provider redundancy at 2× storage cost.

### 7.3 Health Reporting

Providers periodically publish signed heartbeats:

```rust
struct StorageHeartbeat {
    heartbeat_id: Uuid,            // UUID v7
    provider: Did,
    community_id: CommunityId,
    held_blob_cids: Vec<Cid>,      // or held fragment CIDs
    available_bytes: u64,
    uptime_epochs: u64,
    heartbeat_epoch: u64,
    signature: Signature,
}
```

The community aggregates heartbeats into a health view (see §8). Heartbeat
frequency is community policy; recommended: once per epoch.

### 7.4 Repair Protocol

When health reporting shows a blob with `actual_k < k_target`:

1. Blob enters the replication queue at **elevated priority** (its
   `under_replication_score` increases).
2. Any provider can act as a **repair worker**: fetch the blob (or
   reconstruct it from fragments), re-host it, and announce.
3. Repair workers earn `BlobRepairCompleted` credit (§3.3).
4. When `actual_k >= k_target`, the blob exits the elevated-priority state.

Repair is pull-based (providers self-select) rather than push-based (no
coordinator assigns repair work). The replication queue and credit incentive
drive repair organically.

### 7.5 Eviction and Withdrawal

When a provider wants to stop hosting a blob:

```rust
struct StorageWithdrawal {
    withdrawal_id: Uuid,           // UUID v7
    provider: Did,
    community_id: CommunityId,
    blob_cid: Cid,
    withdrawal_epoch: u64,         // effective epoch
    signature: Signature,
}
```

- Publishing a `StorageWithdrawal` triggers the blob to re-enter the
  replication queue (its `actual_k` will drop by 1 at `withdrawal_epoch`).
- A **grace period** (community policy, recommended: 3 epochs) gives other
  providers time to pick up the blob before the withdrawal takes effect.
- The withdrawing provider is not penalized if the grace period is respected.
  Immediate disappearance without withdrawal announcement is treated as a
  failed challenge and triggers reputation penalty.

---

## 8. Storage Bank Visibility

The "precisely reporting realtime state" of the storage bank is surfaced at
three levels.

### 8.1 Per-Blob Health

| Field | Source |
|---|---|
| Replica count (`actual_k`) | Aggregated from `StorageAnnouncement`s |
| Provider list | Active announcers for this blob |
| Last challenge time | Most recent `BlobAvailabilityEpochHeld` receipt |
| Health status | `Healthy` (k >= k_target), `Degraded` (0 < k < k_target), `AtRisk` (k = 0 at community, bilateral only), `LocalOnly` (k = 0 everywhere) |

### 8.2 Per-Community Health

| Field | Source |
|---|---|
| Total committed capacity | Sum of active `StorageBond` committed bytes |
| Total used capacity | Sum of hosted blob sizes across providers |
| Under-replicated blob count | Blobs with `actual_k < k_target` |
| Provider count | Active providers with heartbeats in the last N epochs |
| Average uptime | Mean `uptime_epochs` across active providers |
| Pool utilization | `CommunityStoragePool.total_consumed / total_pledged` |

### 8.3 Per-Peer View

| Field | Source |
|---|---|
| My contributed capacity | Bytes I'm hosting for others |
| My used capacity | Bytes others are hosting for me |
| My credit balance | Base + bonus credits earned (from local ledger) |
| My pledge commitments | Credits pledged to community pools |
| My blobs' health | Per-blob health for data I've placed |

---

## 9. Pledging Credits to Community Pools

Credits earned in the storage bank are **non-transferable** between peers —
there is no trading, no exchange rate, no token. However, a peer can **pledge**
portions of their earned credit to a community storage pool that backs shared
services.

### 9.1 The Pledge Model

```rust
struct StoragePledge {
    pledge_id: Uuid,                  // UUID v7
    pledger: Did,
    community_id: CommunityId,
    pledged_credit_units: u64,        // non-transferable, directed allocation
    effective_epoch: u64,
    expiry_epoch: Option<u64>,        // None = open-ended
    signature: Signature,
}
```

A pledge is a **directed allocation**, not a transfer. The pledger retains
ownership of the credit but commits it to a specific purpose. Pledges can be
withdrawn (with a grace period, like `StorageWithdrawal`) but cannot be
redirected to another peer.

### 9.2 Community Storage Pool

```rust
struct CommunityStoragePool {
    community_id: CommunityId,
    total_pledged_units: u64,
    total_consumed_units: u64,
    active_pledges: Vec<StoragePledge>,
    service_allocations: Vec<ServiceAllocation>,
}

struct ServiceAllocation {
    service_ref: String,              // e.g., "room:abc123", "workspace:def456"
    allocated_units: u64,
    priority: AllocationPriority,
}

enum AllocationPriority {
    Critical,    // FLora checkpoints, governance records
    Standard,    // shared workspaces, rooms
    BestEffort,  // cached content, optional archives
}
```

Services that need persistent storage (Matrix rooms, shared workspaces, FLora
checkpoints) draw from the community pool. When the pool runs low:

1. Under-replicated blobs are flagged in the health dashboard.
2. Community governance decides: recruit more contributors, reduce retention
   policy, or prune low-value content (`BestEffort` allocations evicted first).

### 9.3 Cross-Track Isolation

Pledging does **not** create cross-track fungibility:
- Storage credits cannot be converted to sats or FIL
- Storage credits cannot be converted to reputation
- Pledging does not grant governance weight
- Large pledges do not confer policy control

The three-track economic model's separation (sats/FIL/reputation) remains
intact. Pledging is resource allocation within the storage track, not a bridge
between tracks.

---

## 10. Node Contribution Budget

Each peer configures how much storage capacity they contribute:

```rust
struct StorageContributionBudget {
    max_contributed_bytes: u64,
    max_bandwidth_bytes_per_epoch: u64,
    service_hours: Option<(u8, u8)>,  // active hours (e.g., 08:00–22:00); None = always
    auto_repair: bool,                // participate in repair work automatically
}
```

This sits alongside the existing `NodeTreasuryPolicy` in the self-hosted node
spec. It governs the supply side: how much of my resources I'm willing to
contribute to the storage bank.

**Defaults**:
- `max_contributed_bytes`: 0 (no contribution until explicitly configured)
- `max_bandwidth_bytes_per_epoch`: 0
- `service_hours`: None
- `auto_repair`: false

Contributing to the storage bank is always opt-in.

---

## 11. Integration Summary

### 11.1 PoA Ledger Extensions

Add to `AccessWorkType`:
- `BlobAvailabilityEpochHeld` — base credit for passing availability challenge
- `BlobRetrievalServed` — usage bonus with hold-duration weighting
- `BlobRepairCompleted` — repair credit for re-hosting under-replicated blobs

Add to `AccessReceipt`:
- `hold_duration_epochs: Option<u64>` — epochs held before serving

### 11.2 Economic Model Extensions

- **Bilateral storage visibility**: `PeerStorageReport` in the Verso section
- **Pledge-to-pool**: `StoragePledge` and `CommunityStoragePool` as storage
  track mechanisms (not new tracks)
- **Storage bank health**: per-blob, per-community, per-peer health views

### 11.3 Self-Hosted Node Extensions

- `StorageContributionBudget` alongside `NodeTreasuryPolicy`

### 11.4 VerseBlob Extensions

- `FragmentManifest` (with `CodingScheme::FullCopy` for v1)
- `StorageAnnouncement`, `StorageHeartbeat`, `StorageWithdrawal` message types

### 11.5 VERSE_AS_NETWORK Extensions

- Storage Bank section under Verse Community Layer

### 11.6 VERSO_AS_PEER Extensions

- Bilateral storage visibility section

---

## 12. What This Does NOT Change

- **Anti-plutocracy guarantees**: Storage credit ≠ governance authority. The
  only path from storage to governance weight remains: reliable service →
  `BlobServed` receipts → reputation → governance weight (slow, active,
  decay-limited).
- **Payment-last principle**: Reputation and credit accounting come first.
  FIL payout remains off by default. Payment channels are the last thing
  enabled.
- **Local-first default**: Graphshell works without any storage bank. A peer
  with no network connection loses nothing — their data is self-hosted.
- **Three-track separation**: Sats (compute), FIL (storage), reputation
  (governance) remain independent. Pledge-to-pool is resource allocation
  within the storage track, not cross-track fungibility.

---

## 13. v1 Defaults

- `CodingScheme::FullCopy` (naive k-replication)
- `k_target = 3`
- Base + bonus credit with community-configurable ratio
- Challenge frequency: 1 per commitment per epoch
- Withdrawal grace period: 3 epochs
- Heartbeat frequency: 1 per epoch
- Pledge-to-pool: enabled but no automatic pledging
- `StorageContributionBudget`: all zeros (opt-in)
- FIL payout: off by default (reputation-only accounting)
