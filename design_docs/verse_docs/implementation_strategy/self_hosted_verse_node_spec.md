# Self-Hosted Verse Node Specification

**Date**: 2026-02-28
**Status**: Proposed (canonical Tier 2 draft)
**Scope**: Defines the operational model, service surfaces, transport boundaries, storage classes, and budget controls for a local private-by-default Verse node.
**Related**:
- `design_docs/verse_docs/technical_architecture/VERSE_AS_NETWORK.md`
- `design_docs/verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md`
- `design_docs/verse_docs/implementation_strategy/proof_of_access_ledger_spec.md`
- `design_docs/verse_docs/implementation_strategy/community_governance_spec.md`

---

## 1. Purpose

A self-hosted Verse node is the user's sovereign local portal into:
- private storage
- bilateral sync
- community storage/indexing
- FLora contribution and consumption
- optional public or semi-public hosted services

It is **private-by-default**. Every outward-facing service must be explicitly enabled.

---

## 2. Core Design Rules

1. **Private by default**
No public service exposure without explicit enablement.

2. **Transport separation**
Keep trusted bilateral flows and community/public flows distinct.

3. **Budgeted services**
Every resource-bearing service should have hard quotas and, when applicable, spend caps.

4. **Local sovereignty**
The user must be able to keep data, adapters, and engrams local while selectively exporting projections.

5. **Service isolation**
Applet hosting, storage serving, FLora contribution, and feed/forum exposure should be independently toggleable.

---

## 3. Transport Boundaries

Recommended transport mapping:

- **iroh**
  - trusted peer pairing
  - workspace sync
  - live presence
  - direct private exchange

- **libp2p**
  - community discovery
  - pubsub announcements
  - content-addressed retrieval
  - public or semi-public services

This keeps the Tier 1 trust boundary simple while letting Tier 2 scale independently.

---

## 4. Service Surfaces

```rust
enum VerseService {
    PrivateStorage,
    CommunityStorage,
    FederatedSearch,
    FloraProvider,
    FloraConsumer,
    AppletHost,
    FeedHost,
    ForumHost,
    SharedProcessGateway,
}
```

### 4.1 Safe Defaults

- `PrivateStorage`: enabled
- all other services: disabled until explicitly configured

---

## 5. Node Modes

```rust
enum VerseNodeMode {
    LocalOnly,
    TrustedPeersOnly,
    CommunityParticipant,
    CommunityHost,
}
```

### 5.1 Recommended Behavior

- `LocalOnly`: no libp2p participation
- `TrustedPeersOnly`: iroh only, no public/community surfaces
- `CommunityParticipant`: reads/contributes to communities under strict quotas
- `CommunityHost`: may serve blobs, applets, or other surfaces publicly or semi-publicly

The user should be able to move between these modes without migrating their data model.

---

## 6. Storage Classes

```rust
enum StorageClass {
    PrivateHot,
    PrivateWarm,
    CommunityCache,
    CommunityPinned,
    Archive,
}
```

Recommended semantics:
- `PrivateHot`: low-latency local working set
- `PrivateWarm`: local retained but not always memory-hot
- `CommunityCache`: opportunistic retained data, evictable
- `CommunityPinned`: explicitly committed to serve under policy
- `Archive`: retained for audit/export, not necessarily served live

---

## 7. Treasury and Budget Controls

```rust
struct NodeTreasuryPolicy {
    payout_enabled: bool,
    max_epoch_spend_units: u64,
    max_single_action_spend_units: u64,
    emergency_stop_enabled: bool,
}
```

### 7.1 Budget Best Practices

- Require explicit budget opt-in before any payout-capable service is enabled.
- Keep per-epoch and per-action caps.
- Provide an emergency stop that halts new payout approvals immediately.
- Separate storage commitment from reward commitment.

This prevents a storage-serving node from silently becoming a spending node.

---

## 8. Exposure Policy

```rust
enum ExposureMode {
    Private,
    TrustedPeers,
    CommunityScoped,
    Public,
}
```

Each service surface should declare its own exposure mode.

Example:
- `AppletHost`: `Private` or `CommunityScoped`
- `CommunityStorage`: `CommunityScoped`
- `ForumHost`: `CommunityScoped` or `Public`

No service should inherit a more public exposure automatically from another service.

---

## 9. Resource Safety

Minimum required protections:
- upload / download rate limits
- connection caps
- storage quota enforcement
- decompression ratio caps
- fetch-on-demand for large objects
- per-service enablement toggles

These align with standard libp2p resource-management practice: bound memory, bandwidth, and concurrent work per subsystem.

---

## 10. Engram and Data Sovereignty

A self-hosted node should support:
- storing full local-private engrams
- exporting sparse or redacted engrams to communities
- preserving private dataset lineage while sharing only derived metadata
- maintaining local adaptation history independent of community checkpoint history

This is the operational expression of the local-first model.

---

## 11. Hosted Service Guardrails

Hosted services such as applets, feeds, forums, or shared process gateways should:
- require explicit service enablement
- declare storage and bandwidth budgets
- have independent moderation policy if community-facing
- never gain implicit access to private engrams or private workspace data

The self-hosted node is a portal, not a single undifferentiated trust zone.

---

## 12. Immediate Defaults (v1)

- Start in `LocalOnly`
- Enable only `PrivateStorage`
- Keep libp2p off until a community join or host action explicitly enables it
- Keep payouts off until treasury policy is configured
- Require per-service exposure mode selection
- Enforce resource limits across every public/community-facing surface

These defaults make the self-hosted node safe enough to be practical for everyday users.
