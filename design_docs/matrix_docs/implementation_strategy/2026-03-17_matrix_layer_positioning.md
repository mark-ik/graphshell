<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Matrix Layer Positioning — Durable Shared Spaces in the Hosting Gradient

**Date**: 2026-03-17
**Updated**: 2026-03-17 — revised to three-context + two-fabric model; added room hosting gradient, cross-carrying rules, concept resurfacing
**Status**: Draft / architectural positioning note
**Scope**: Places Matrix relative to iroh, libp2p, Verse, Nostr, and WebRTC within Graphshell's hosting gradient and host/mod ownership model.

**Related docs**:

- [`2026-03-05_network_architecture.md`](2026-03-05_network_architecture.md) - Current protocol layer assignments for iroh, libp2p, Nostr, and WebRTC.
- [`2026-03-05_nostr_mod_system.md`](2026-03-05_nostr_mod_system.md) - Host-owned relay/signing model for Nostr mods.
- [`2026-03-17_multi_identity_binding_rules.md`](2026-03-17_multi_identity_binding_rules.md) - Three-identity model and explicit binding rules for `NodeId`, `npub`, and Matrix IDs.
- [`2026-03-17_matrix_core_adoption_plan.md`](2026-03-17_matrix_core_adoption_plan.md) - Execution plan for `MatrixCore` session lifecycle, room projection, and allowlisted graph-intent mapping.
- [`2026-03-17_matrix_event_schema.md`](2026-03-17_matrix_event_schema.md) - Initial `graphshell.room.*` Matrix event namespace, payload shapes, and allowed routing behavior.
- [`register/matrix_core_registry_spec.md`](register/matrix_core_registry_spec.md) - `MatrixCore` capability-provider boundary for room sync and membership-governed spaces.
- [`../../verso_docs/implementation_strategy/coop_session_spec.md`](../../verso_docs/implementation_strategy/coop_session_spec.md) - Co-op transport, identity, and wallet boundaries.
- [`../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md`](../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md) - Verse/Nostr boundary and "front door vs interior" split.

---

## 1. Decision Summary

Graphshell's network model is organised as **three contextual substrates** with a
hosting gradient, plus **two cross-cutting capability fabrics**.

### 1.1 Three Contextual Substrates

| Context | Metaphor | Transport | Hosting |
| --- | --- | --- | --- |
| **Bilateral** | "come to my home" | iroh (QUIC) | Your device — ephemeral, live while you're online |
| **Room** | "let's meet in this room" | Matrix | Self-hosted, member-seeded, or Verse-hosted — durable |
| **Community** | "this is our community's center" | Verse (libp2p) | Self-hosted or decentralized storage hosters — durable + replicated |

Each context answers a different question:

- **iroh** answers "which trusted peers/devices are live with me now?"
- **Matrix** answers "which durable shared spaces am I a member of?"
- **Verse** answers "which subject communities and knowledge worlds do I participate in?"

### 1.2 Two Cross-Cutting Capability Fabrics

- **Nostr** is a set of social capabilities (identity, messaging, publication, discovery)
  available at every substrate. Nostr is like a post office with terminals at your home,
  the group's room, and the community's center. It provides identity cards, relayed
  messaging, highlights, follows, and publication — reusable across all three contexts.

- **WebRTC** is a media-piping capability invocable from any substrate that has a signaling
  path. It provides audio, video, and screen share. It is not a standalone protocol family —
  it depends on a signaling substrate (iroh for bilateral, Matrix for rooms).

Matrix should **not** replace:

- `iroh` for trusted-peer sync, direct blob transfer, or low-latency Coop state
- `libp2p` for Verse swarm discovery and community replication
- `Nostr` for portable public identity, relay publication, and Nostr-native social presence

---

## 2. Layer Placement

| Concern | Primary substrate | Matrix role |
| --- | --- | --- |
| Trusted device sync | iroh | None |
| Bilateral live collaboration (Coop) | iroh (+ WebRTC for media, signaled over iroh) | Optional durable-promotion target |
| Durable private/shared spaces | Matrix | Primary — with self-hosted, member-seeded, or Verse-hosted rooms |
| Public/private subject communities | Verse (libp2p + selected Nostr layers) | Service offered by Verse infrastructure; not the community substrate itself |
| Public identity / follows / relay publication | Nostr (cross-cutting) | Nostr capabilities are available within rooms via widgets/applets |
| Real-time media | WebRTC | Matrix rooms are the signaling plane for room-based calls (MSC3401) |

Matrix is a **durable room/state substrate** and the natural **signaling plane for
WebRTC** in room contexts — analogous to how iroh and libp2p are a transport pair
at different peer-count scales.

---

## 3. Relationship to Existing Graphshell Protocols

### 3.1 iroh (Bilateral Context)

iroh remains the peer-scoped transport for:

- bilateral device sync
- trusted small-group Coop sessions
- direct peer blob transfer
- low-latency cursor/presence/event channels

Matrix must not reopen the decision that these flows are P2P-first. If a
Matrix-backed graph room exists, it is a different collaboration mode with
different durability and trust assumptions.

**Promotion path**: An ephemeral iroh Coop session between two people can
optionally "promote" into a durable Matrix room if the participants decide the
conversation should outlive the session. This follows the same ephemeral-to-durable
pattern as pane promotion in the tile tree.

**iroh as P2P replication for member-hosted rooms**: Member-seeded Matrix rooms
(§4) may use iroh-docs for P2P state replication among participating members,
applying iroh's content-addressed, eventually-consistent sync without routing
through a homeserver.

### 3.2 Verse (Community Context)

Verse remains the subject-oriented community layer. Its canonical concerns are:

- community identity and participation around a topic/domain
- community replication and discovery
- long-horizon shared knowledge growth
- FLora/community artifact stewardship

**Verse as room infrastructure provider**: A Verse community can host Matrix rooms
as a service for its members, analogous to how a Verse hosts knowledge stores. Room
provisioning and governance may be managed through Verse community governance. A
Verse community should be able to outlive any single Matrix homeserver or room —
the community is the durable entity, the rooms are services within it.

### 3.3 Nostr (Cross-Cutting Capability Fabric)

Nostr is not a layer in the same sense as iroh, Matrix, or libp2p. It is a set
of social capabilities reusable across all three contexts:

| Nostr capability | Bilateral (iroh) | Room (Matrix) | Community (Verse) |
| --- | --- | --- | --- |
| Identity (npub, kind 0) | Coop peer identity | Room member identity cards | Community member profiles |
| DMs (NIP-17) | Coop invites, peer chat | Out-of-room member messaging | Community member private messages |
| Follows (NIP-02) | Bilateral trust signals | Room member discovery | Community author follows |
| Highlights (NIP-84) | Clip sharing between peers | Clip publication from room context | Community knowledge curation |
| DVMs (NIP-90) | — | Room-initiated compute jobs | Community compute marketplace |
| Publication (kind 30000+) | Published bilateral snapshots | Room-published artifacts | Community knowledge publications |

Matrix rooms can publish to Nostr relays; Nostr widgets/applets can operate within
Matrix room surfaces. These are capabilities manifesting in a context, not competing
layers.

### 3.4 WebRTC (Cross-Cutting Media Capability)

WebRTC is a media-piping capability, not a standalone protocol family. It is
invocable from any context that provides a signaling path:

| Context | Signaling path | Use case |
| --- | --- | --- |
| Bilateral (iroh Coop) | iroh session stream (SDP exchange over QUIC) | Screen share, video in Coop sessions |
| Room (Matrix) | Matrix room events (MSC3401 / m.call.* family) | Room calls, screen share in Matrix-backed spaces |
| Community (Verse) | Community-hosted Matrix room or libp2p signaling | Community live events (future) |

Matrix has the most mature WebRTC signaling implementation (Element Call / MSC3401).
If `MatrixCore` owns WebRTC signaling as a capability, it could potentially be
offered to other contexts when a Matrix session is available, with iroh-native
signaling as the zero-dependency fallback for P2P-only Coop sessions.

---

## 4. Room Hosting Gradient

Matrix rooms in Graphshell support three hosting modes, forming a gradient from
fully self-controlled to community-governed:

| Mode | Host | Durability | Trust model |
| --- | --- | --- | --- |
| **Self-hosted** | User's own homeserver | Durable (persists while server runs) | Full self-sovereignty |
| **Member-seeded** | P2P replication among room members | Durable (persists while members seed) | Distributed among members; higher-priority members carry more network responsibility |
| **Verse-hosted** | Verse community's decentralized storage layer | Durable + replicated | Delegated to prescribed hosters (private) or public hosters; governed by Verse community policy |

### 4.1 Self-hosted

The user runs or controls a Matrix homeserver. Standard Matrix federation applies.
This is the baseline mode and requires no Graphshell-specific infrastructure.

### 4.2 Member-seeded

Room members replicate room state among themselves without routing through a
central homeserver. This is conceptually iroh-docs applied to room state: content-
addressed, eventually-consistent, NAT-traversal-included.

Member priority determines network responsibility: higher-priority members keep
more state warm and relay to less-available members. Priority may be implicit
(based on uptime/availability) or explicit (assigned by room governance).

When enough members are online, the room is available. When no members are online,
the room is offline but state is preserved on members' devices.

For fan-out beyond iroh's comfort zone (~5 peers), libp2p gossipsub may be used
for state distribution across a larger member set.

### 4.3 Verse-hosted

A Verse community provisions room infrastructure as a service for its members,
the same way it provisions knowledge storage. The room's existence and governance
are community-managed facts. Members discover Verse-hosted rooms through the
community's libp2p discovery infrastructure.

This mode does not require individual users to run homeservers or maintain
constant uptime — the community's decentralized storage layer handles durability.

---

## 5. Interop Boundaries

### 5.1 Matrix and Nostr Interop

Matrix interoperability with Nostr is implemented as **capabilities manifesting
in context**, not protocol-level identity unification.

Rules:

1. Keep `NodeId`, Matrix user ID, and Nostr `npub` as distinct identities.
2. Allow explicit signed bindings between those identities.
3. Nostr capabilities (publish, DM, identity, highlights) are available as
   applets/widgets within Matrix room surfaces.
4. Do not assume raw key reuse, native protocol compatibility, or automatic
   trust transfer between Matrix moderation state and Nostr relay state.

Practical examples:

- A Matrix room may expose a "publish selected events to Nostr" action.
- A Nostr identity card in Graphshell may show linked Matrix handles.
- A bridge may mirror room announcements or highlights into Nostr, while the
  durable private room history stays Matrix-native.
- Coop chat at the bilateral layer may be facilitated through a Nostr DM
  implementation (NIP-17).

Non-goal:

- "Matrix replaces Nostr identity" is not a valid architecture claim.

### 5.2 Cross-Carrying Rules

Protocols should carry water for each other where there is a natural fit:

| Carrier | Carried for | Direction | Description |
| --- | --- | --- | --- |
| iroh | Matrix | iroh → Matrix | P2P replication for member-seeded rooms |
| Matrix | iroh Coop | Matrix → iroh | Coop session "promotion" to a durable room |
| Matrix | WebRTC | Matrix → WebRTC | Room-based WebRTC signaling (MSC3401) |
| iroh | WebRTC | iroh → WebRTC | P2P WebRTC signaling (SDP over QUIC) for Coop media |
| Verse/libp2p | Matrix | libp2p → Matrix | Verse-hosted room discovery and provisioning |
| libp2p | Matrix | libp2p → Matrix | gossipsub fan-out for large member-seeded rooms |
| Nostr | all contexts | cross-cutting | Identity, DMs, publication, discovery available everywhere |
| Verse | iroh | libp2p → iroh | Community introduces two peers who establish bilateral iroh session |

Anti-patterns:

- Matrix absorbing iroh's bilateral transport role
- Matrix absorbing Verse's community replication role
- Nostr replacing Matrix for durable room state
- WebRTC signaling without a clear substrate owner

---

## 6. Concept Resurfacing

Architectural concepts are not locked to their native context. They resurface
across layers through composition, not reimplementation:

| Concept | Native context | Resurfaced in | How |
| --- | --- | --- | --- |
| **Coop** (live collaboration) | Bilateral (iroh) | Room — two room members go live | iroh session initiated from room member discovery |
| | | Community — two Verse members go live | iroh session initiated from Verse member discovery |
| **Room** (durable shared space) | Matrix | Bilateral — Coop promoted to durable | Explicit promotion creates room for two |
| | | Community — Verse-hosted rooms | Community provisions room infrastructure |
| **Verse** (knowledge community) | Community (libp2p) | Bilateral — micro-verse between two peers | iroh-carried Verse conventions, bilateral knowledge exchange |
| | | Room — room-scoped knowledge curation | Room members curate/replicate knowledge within room context |
| **Nostr applet** (social capability) | Cross-cutting | All three contexts | Same `npub`, same NIP standards, different host context |
| **WebRTC** (media piping) | Cross-cutting | Bilateral (iroh signaling) and Room (Matrix signaling) | Invocable from any context with signaling |

The underlying protocol implementations stay the same; the contexts compose them.
A "Coop from a Verse" is still an iroh session — the Verse provided the introduction.
A "room in a Verse" is still a Matrix room — the Verse community hosts the
infrastructure.

---

## 7. Ownership Model Inside Graphshell

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
- optional WebRTC signaling service for room-based calls

This keeps Graphshell's "single mutation authority" invariant intact.

---

## 8. When Matrix Is the Right Tool

Matrix is a good fit when the user wants:

- durable shared spaces with persistent membership
- moderation roles and room governance
- organisation/team collaboration with history
- federation with existing Matrix deployments
- a messaging substrate that is stronger than ad hoc invites/DMs
- WebRTC signaling for room-based calls
- a room that can be hosted by themselves, their peers, or their community

Matrix is the wrong fit when the requirement is:

- direct trusted-device sync
- low-latency peer session transport
- swarm discovery / DHT routing
- public publish-once relay distribution
- canonical Verse community replication

---

## 9. Planning Implication

The network model comprises three contextual substrates and two cross-cutting
capability fabrics. Four native mods carry the implementation:

**Contextual substrates (three)**:

- iroh = bilateral trusted peer/session transport
- Matrix = durable room/state substrate (self-hosted, member-seeded, or Verse-hosted)
- Verse/libp2p = community swarm transport and governance

**Cross-cutting capability fabrics (two)**:

- Nostr = social capabilities (identity, messaging, publication, discovery) available at every substrate
- WebRTC = media piping invocable from any substrate with a signaling path

**Native mods (four)**:

- Verso (Servo/Wry + iroh)
- Verse (libp2p communities)
- Nostr (social capability fabric)
- Matrix (durable rooms + WebRTC signaling)

If a future lane is opened for Matrix, it should start with:

1. a `MatrixCore` boundary/spec
2. identity-binding rules (`NodeId` <-> Matrix ID <-> `npub`)
3. room-hosting-mode definitions (self-hosted, member-seeded, Verse-hosted)
4. room-event to graph-intent mapping rules
5. explicit non-goals forbidding Matrix from replacing `iroh`/`libp2p`

That sequence preserves the three-context architecture instead of blurring it.
