<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Matrix Layer Positioning - Durable Shared Spaces Above the P2P Stack

**Date**: 2026-03-17
**Status**: Draft / architectural positioning note
**Scope**: Places Matrix relative to iroh, libp2p, Verse, Nostr, and Graphshell's host/mod ownership model.

**Related docs**:

- [`2026-03-05_network_architecture.md`](2026-03-05_network_architecture.md) - Current protocol layer assignments for iroh, libp2p, Nostr, and WebRTC.
- [`2026-03-05_nostr_mod_system.md`](2026-03-05_nostr_mod_system.md) - Host-owned relay/signing model for Nostr mods.
- [`2026-03-17_multi_identity_binding_rules.md`](2026-03-17_multi_identity_binding_rules.md) - Three-identity model and explicit binding rules for `NodeId`, `npub`, and Matrix IDs.
- [`2026-03-17_matrix_core_adoption_plan.md`](2026-03-17_matrix_core_adoption_plan.md) - Execution plan for `MatrixCore` session lifecycle, room projection, and allowlisted graph-intent mapping.
- [`2026-03-17_matrix_event_schema.md`](2026-03-17_matrix_event_schema.md) - Initial `graphshell.room.*` Matrix event namespace, payload shapes, and allowed routing behavior.
- [`register/matrix_core_registry_spec.md`](register/matrix_core_registry_spec.md) - `MatrixCore` capability-provider boundary for room sync and membership-governed spaces.
- [`coop_session_spec.md`](coop_session_spec.md) - Coop transport, identity, and wallet boundaries.
- [`../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md`](../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md) - Verse/Nostr boundary and "front door vs interior" split.

---

## 1. Decision Summary

If Graphshell adopts Matrix, it should sit **above** the P2P transport stack as a
durable shared-space and messaging substrate. It should **not** replace:

- `iroh` for trusted-peer sync, direct blob transfer, or low-latency Coop state
- `libp2p` for Verse swarm discovery and community replication
- `WebRTC` for real-time media
- `Nostr` for portable public identity, relay publication, and Nostr-native social presence

Matrix's best-fit role is:

- durable private/shared graph rooms
- membership and moderation semantics
- room-backed message history
- organisation/team collaboration surfaces
- bridge-friendly federation with an existing ecosystem

In short:

- `iroh` answers "which trusted peers/devices are live with me now?"
- `Matrix` answers "which durable shared spaces am I a member of?"
- `Verse` answers "which subject communities and knowledge worlds do I participate in?"
- `Nostr` answers "how do I publish, discover, and carry portable public/social identity?"

---

## 2. Layer Placement

| Concern | Primary substrate | Matrix role |
| --- | --- | --- |
| Trusted device sync | `iroh` | None |
| Small-n live collaboration | `iroh` (+ `WebRTC` for media) | Optional mirror/invite layer only |
| Durable private/shared spaces | `Matrix` | Primary, if adopted |
| Public/private subject communities | `Verse` (`libp2p` + selected Nostr layers) | Not the canonical community substrate |
| Public identity / follows / relay publication | `Nostr` | Optional bridge target, not replacement |
| Real-time media | `WebRTC` | None |

Matrix is therefore a **federated room/state layer**, not a transport layer.

---

## 3. Relationship to Existing Graphshell Protocols

### 3.1 iroh

`iroh` remains the peer-scoped transport for:

- bilateral device sync
- trusted small-group Coop sessions
- direct peer blob transfer
- low-latency cursor/presence/event channels

Matrix must not be allowed to reopen the decision that these flows are P2P-first.
If a Matrix-backed graph room exists, it is a different collaboration mode with
different durability and trust assumptions.

### 3.2 Verse

Verse remains the subject-oriented community layer. Its canonical concerns are:

- community identity and participation around a topic/domain
- community replication and discovery
- long-horizon shared knowledge growth
- FLora/community artifact stewardship

Matrix rooms may be useful as a coordination surface for Verse operators or
private teams, but they should not become the canonical Verse substrate. A Verse
community should be able to outlive any single Matrix homeserver or room.

### 3.3 Nostr

Nostr remains the cross-cutting public/social layer:

- portable public identity and presence
- posts, highlights, follows, DMs, and relay publication
- public discovery and linkability across ecosystems
- embedded webapps/mods that already target Nostr

Matrix and Nostr can interoperate at the application layer, but they should be
treated as separate ecosystems with separate native identities.

---

## 4. Matrix and Nostr Interop Boundary

Matrix interoperability with Nostr is plausible and useful, but it should be
implemented as a **bridge/adapter**, not as protocol-level identity unification.

Rules:

1. Keep `NodeId`, Matrix user ID, and Nostr `npub` as distinct identities.
2. Allow explicit signed bindings between those identities.
3. Permit bridge services to mirror selected events between Matrix rooms and
   Nostr relays when the user or room policy allows it.
4. Do not assume raw key reuse, native protocol compatibility, or automatic
   trust transfer between Matrix moderation state and Nostr relay state.

Practical examples:

- A Matrix room may expose a "publish selected events to Nostr" action.
- A Nostr identity card in Graphshell may show linked Matrix handles.
- A bridge may mirror room announcements or highlights into Nostr, while the
  durable private room history stays Matrix-native.

Non-goal:

- "Matrix replaces Nostr identity" is not a valid architecture claim.

---

## 5. Ownership Model Inside Graphshell

If implemented, Matrix should follow the same host-owned networking principles as
Nostr:

- host-owned homeserver client/session lifecycle
- host-owned crypto/session secret handling
- host-owned sync worker and event projection
- mods/surfaces receive capabilities, not raw sockets or unmanaged secrets
- graph mutations still flow through host reducers/intents

A likely shape is a `MatrixCore` native service or native mod providing:

- room sync and membership projection
- send/receive operations for approved room/event types
- bounded event-to-graph mapping hooks
- optional bridge hooks for Nostr and Verse surfaces

This keeps Graphshell's "single mutation authority" invariant intact.

---

## 6. When Matrix Is the Right Tool

Matrix is a good fit when the user wants:

- durable shared spaces with persistent membership
- moderation roles and room governance
- organisation/team collaboration with history
- federation with existing Matrix deployments
- a messaging substrate that is stronger than ad hoc invites/DMs

Matrix is the wrong fit when the requirement is:

- direct trusted-device sync
- low-latency peer session transport
- swarm discovery / DHT routing
- public publish-once relay distribution
- canonical Verse community replication

---

## 7. Planning Implication

Matrix should be treated as an **optional future fifth network family** with a
clear, non-overlapping role:

- `iroh` = trusted peer/session transport
- `libp2p` = community swarm transport
- `Nostr` = public/social identity and event bus
- `WebRTC` = media transport
- `Matrix` = durable federated shared-space substrate

If a future lane is opened, it should start with:

1. a `MatrixCore` boundary/spec
2. identity-binding rules (`NodeId` <-> Matrix ID <-> `npub`)
3. room-event to graph-intent mapping rules
4. explicit non-goals forbidding Matrix from replacing `iroh`/`libp2p`

That sequence preserves the current network architecture instead of blurring it.
