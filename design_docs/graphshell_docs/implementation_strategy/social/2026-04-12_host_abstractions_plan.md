<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Protocol-Agnostic P2P Graph Views & Host Abstractions Plan

**Date**: 2026-04-12
**Status**: Active / Implementation Plan
**Scope**: Strategy for Graphshell transition from a unified semantic protocol to a host-first browser for decentralized networks. The shell provides a resilient, peer-to-peer synced canvas (via Verso/iroh), a unified Identity Ring, generic persistence, and the host-side capability boundary for decentralized storage. Communication layers (Matrix, Nostr, IRC) become hosted applets that plug into this canvas without leaking their network semantics into the core architecture.

**Related docs**:
- [`comms/COMMS_AS_APPLETS.md`](comms/COMMS_AS_APPLETS.md) — Comms as hosted applets boundary declaration
- [`../../technical_architecture/GRAPHSHELL_AS_BROWSER.md`](../../technical_architecture/GRAPHSHELL_AS_BROWSER.md) — Semantic framework for Graphshell as host
- [`../../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Verso bilateral and P2P transport primitives
- [`../../../verse_docs/implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md`](../../../verse_docs/implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md) — Canonical Verse storage-bank operational model
- [`../../../verse_docs/research/2026-04-13_storage_system_comparison_for_verse.md`](../../../verse_docs/research/2026-04-13_storage_system_comparison_for_verse.md) — Comparative storage-system synthesis and recommended Verse hybrid

---

## 1. Strategy Overview

Graphshell transitions from a unified semantic protocol to a host-first browser for decentralized networks. By treating protocols as generic modular primitives, Graphshell can host a "Tildeverse" of diverse protocol rooms, channels, and feeds (Nostr, Matrix, IRC, Scuttlebutt, etc.) on a unified canvas. 

Instead of managing bespoke embedded databases and persistent websockets for every protocol, the Graphshell Host provides:
1. **Delegated Identity (Identity Ring)**
2. **Git-like State Replication (Persistence Facade)**
3. **P2P Collaborative Canvas Layout (Verso/iroh)**
4. **Storage Capability Boundaries** for private replication, shared-service durability, and applet/service allocation without collapsing those concerns into one undifferentiated storage economy

Graphshell does **not** own community-scale storage incentives or storage-bank settlement. Verse owns that layer. The host owns the capability seams that make those systems usable:

- local storage visibility and audit surfaces
- service-class declarations and retention classes
- applet package vs. service-instance separation
- safe defaults for what is private, shared, pinned, or incentive-eligible

---

## 2. Host Storage Boundary

The host must model three storage trust zones explicitly instead of pretending all replicated data is the same thing.

| Trust zone | Typical examples | Host obligation | Default credit posture |
| --- | --- | --- | --- |
| **Personal / bilateral** | my devices, trusted-peer backup, private pair/group sync | track, audit, show imbalance, preserve accountability | no public credit by default |
| **Shared service** | Matrix room state, shared workspace, capsule content, shared files | expose service object identity, retention policy, health, and allocation | credit-eligible if Verse/community policy allows |
| **Open public market** | anonymous or pseudonymous third-party durability | expose stronger warnings, budget controls, and policy state | optional later |

Private should not mean invisible. The host should still surface:

- what data is held where
- challenge or verification state
- who is storing for whom
- whether a storage relationship is merely bilateral or part of a shared-service pool

The distinction is that the host must not assume private replication implies public reward.

### 2.1 Applet Package vs Service Instance

The host should treat installed protocol/app runtime and durable shared objects as different kinds of things.

- **Applet package**: host capability or software/runtime artifact
- **Service instance**: a particular room, workspace, capsule bundle, file set, or other durable shared object

The storage-bank question is usually about the service instance, not the installed applet. Graphshell should therefore model persistent storage allocation primarily around durable shared objects rather than around "streaming applets to peers."

---

## 3. Execution Phases

### Phase 1: Host-Owned Core Abstractions

1. **Identity Ring**
   - **Goal:** Implement a unified cryptographic and session identity manager. 
   - **Mechanism:** Instead of each protocol applet managing user auth independently, Graphshell owns the keys/tokens (potentially backed by `ucan` or native OS `keyring`). It delegates context-specific identities (e.g., a Nostr secp256k1 key, Matrix homeserver token, Verso ed25519 identity) seamlessly when an applet is invoked.

2. **Git-Like Persistence Facade (Diff-Driven Sync)**
   - **Goal:** Unify the network data loop under a replication abstraction.
   - **Mechanism:** Extend the Graphshell storage engine around a content-addressable or log-based model using Rust ecosystem primitives (e.g., `automerge` or `iroh-sync`).
   - Instead of applets maintaining long-polling websockets, state is modeled as a tree/document. Applets sync social updates by comparing the local "HEAD" to a remote state, drawing down only the diffs. This model implicitly yields offline-first, local-first social feeds and reduces bandwidth waste.
   - The Persistence Facade must expose separate storage classes for personal replication, shared-service durability, and public/community storage so that audit, retention, and incentive policy are not conflated.

3. **Host Storage Capability Surface**
   - **Goal:** Make storage relationships visible and policy-aware at the host layer without turning Graphshell into the owner of Verse economics.
   - **Mechanism:** Expose service-class metadata, retention class, allocation priority, and health state for durable objects. Graphshell should know that a Matrix room archive is a shared service object, while a Matrix runtime package is a host capability package.
   - **Mechanism:** Record bilateral/private storage relationships for audit and user visibility, but treat them as non-credit-bearing unless explicitly promoted into Verse/community policy.

### Phase 2: The Applet Integration Seam

4. **Protocol Mod API**
   - **Goal:** Define a strict generic interface for communication applets (Nostr, Matrix). 
   - **Mechanism:** An applet requests rendering space (panes/nodes) and registers its state sync mechanisms with the Persistence Facade. It explicitly lacks the authority to mutate the core `GraphTree` semantic domain. (For stricter isolation, WASM-sandboxed guests via `wasmtime` or `extism` should be considered for future untrusted mods).
   - **Mechanism:** Applets must declare which durable objects they create or depend on: room state, workspace snapshot, feed cache, capsule bundle, attachment set, and similar service objects. That declaration is the host seam for storage policy and later Verse integration.

5. **Applet Restructure**
   - **Goal:** Port existing mods to the new API.
   - **Mechanism:** Move Nostr relays and Matrix room clients behind this new API. Their network I/O becomes "syncing my slice of the Persistence Facade document" rather than detached daemon processes directly writing to their own standalone databases.
   - **Mechanism:** Where durable shared state exists, restructure around service-instance carriers rather than around applet-global storage assumptions.

### Phase 3: Persistent P2P Spatial Graph

6. **Spatial Layout Synchronization**
   - **Goal:** P2P multi-user synchronized workspace layouts.
   - **Mechanism:** Synchronize the physical arrangement of the "Verse" using the same git-like (CRDT) diffing capability (e.g., `automerge` or `yrs`). Authorized peers can collaboratively arrange the Host layer over Verso transport—Node A moved to `(x, y)` is treated as a CRDT diff to the shared workspace document.

7. **Abstract Node Mapping**
   - **Goal:** Map inbound network diffs to frontend redraws cleanly.
   - **Mechanism:** Map inbound network diffs (a new Nostr note, a Matrix message) into generic visual UI triggers. Graphshell natively re-renders the node when its underlying document state changes. The UI relies strictly on the Host's diffs rather than applet-specific event loops.

8. **Shared-Service Storage Alignment**
   - **Goal:** Keep the host's storage model aligned with Verse's storage-bank model without importing Verse economics into the shell.
   - **Mechanism:** Let Graphshell mark service objects as private-only, bilateral, shared-service, or Verse-backed. Promotion from one tier to another should be explicit and auditable.
   - **Mechanism:** Keep opaque encrypted hosting and fragment durability as Verse-side concerns while ensuring the host can present meaningful UX about health, retention, and storage relationships.

---

## 4. Crate Ecosystem Integration Targets

To execute the host capabilities, Graphshell relies on the following established Rust crates:

- **`automerge` / `yrs`**: For modeling the `GraphTree` layout as a conflict-free replicated data type (CRDT), enabling concurrent multi-device layout editing.
- **`iroh-sync` / `iroh`**: For the content-addressed, multi-author replicable log beneath the Persistence Facade, handling "fetch and merge diffs" networking automatically.
- **`ucan`**: For peer-to-peer delegation of identity capabilities to the hosted applets without leaking master keys.
- **Capability-oriented encrypted storage patterns** inspired by Tahoe-LAFS / Storj should inform the storage seam, even if Graphshell itself does not own Verse's final storage-bank implementation.

---

## 5. Verification Limits and Gates

- **Identity Delegation Test**: Verify the `IdentityRing` can successfully supply a Nostr keypair and a Matrix auth token in the same user session without the UI explicitly requiring a "global" login blocking the application.
- **Applet Isolation Check**: Ensure Nostr nodes and Matrix nodes can exist in the same spatial graph concurrently without their inner message state structures leaking into the global `WorkspaceState`.
- **Canvas CRDT Test**: Launch two local Graphshell clients connected via Verso (iroh), manipulate the position of a Matrix room node, and verify its new coordinates propagate to the peer's graph view as a differential sync.
- **Offline Tolerance**: Simulate an offline period: create local social-layer updates, regain connectivity, and verify the Persistence Facade correctly computes and pushes the delta via iroh/automerge.
- **Private Storage Visibility Test**: Verify that personal-device or trusted-peer storage relationships are visible, auditable, and challengeable without being surfaced as public credit-bearing service.
- **Service-Instance Classification Test**: Verify that the host distinguishes an installed applet package from a durable shared object such as a room archive, workspace snapshot, or capsule bundle.
- **Tier Promotion Test**: Verify that a durable object can move from local-only to bilateral to shared-service/Verse-backed classification without changing object identity or silently changing incentive policy.
