<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Network Architecture — iroh / libp2p / Nostr / WebRTC Layer Assignment

**Date**: 2026-03-05
**Updated**: 2026-03-07 — added WebRTC layer (§2.4, §3.7, §8)
**Status**: Draft / canonical direction
**Scope**: Protocol layer assignments for Coop, Device Sync, Verse, identity, and real-time media features.

**Related docs**:

- [`coop_session_spec.md`](coop_session_spec.md) — Coop session authority (§3 transport, §15 identity, §16 wallet)
- [`2026-03-05_cp4_p2p_sync_plan.md`](2026-03-05_cp4_p2p_sync_plan.md) — Device Sync (iroh transport, ControlPanel boundary)
- [`2026-02-23_verse_tier1_sync_plan.md`](../../../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — Verso mod and iroh endpoint authority

---

## 1. Layer Assignment Summary

Graphshell uses four network protocol families with distinct, non-overlapping roles:

| Layer | Protocol | Role | Features |
| --- | --- | --- | --- |
| **Transport** | iroh (QUIC) | Direct peer connections, reliable data | Device Sync, Coop cursor/presence, blob transfer |
| **Transport (swarm)** | libp2p | Multi-peer swarm topology, DHT routing | Verse (rotating hosts, large-n spaces) |
| **Application bus** | Nostr | Identity, event publication, social graph | User profiles, follows, DMs, relay-persisted events |
| **Media transport** | WebRTC | Real-time audio/video/screen share | Coop screen share, synchronized video playback |

These are complementary, not competing. No two protocols in this table overlap in function:

- iroh and libp2p both use QUIC but serve different scales: iroh is session-scoped (2–5 named peers, low latency), libp2p is swarm-scoped (open mesh, DHT, content routing).
- Nostr is not a transport — it carries small signed events only, never bulk data or streams.
- WebRTC is media-only — its SCTP data channels have worse ordering guarantees than QUIC streams for document sync; it is used only where native media handling is required.

iroh ships a `libp2p-iroh` crate that bridges the two transport layers. Nostr sits above both.

---

## 2. Protocol Roles in Detail

### 2.1 iroh (QUIC)

iroh provides QUIC-based direct peer connections with NAT hole-punching and encrypted channels. It is the primary transport for:

- **Device Sync** (CP4): continuous state replication between trusted devices. `iroh-docs` (eventually-consistent KV) and `iroh-blobs` (BLAKE3 content-addressed transfer) are the data sync primitives.
- **Coop real-time channel**: cursor position streaming (iroh datagram, unreliable, low-latency), durable session events (iroh stream, reliable, ordered).
- **Blob transfer for graph media nodes**: content-addressed file transfer between peers using `iroh-blobs`.

iroh relay servers (DERP) are NAT traversal helpers only — they see encrypted traffic and step back once a hole is punched. They are not data stores.

### 2.2 libp2p

libp2p adds:

- **Kademlia DHT**: content routing by hash — find peers who hold a given piece of content without knowing where it is ahead of time.
- **gossipsub**: epidemic broadcast trees for pub-sub across a swarm. More scalable than iroh-gossip for large-n topologies.
- **mDNS local discovery**: automatic peer discovery on a local network.
- **Multiple transports**: TCP, QUIC, WebRTC, WebSocket with protocol negotiation.

libp2p is the right transport for **Verse** (see §4), where sessions involve larger peer sets, rotating hosts, and swarm replication semantics.

### 2.4 WebRTC

WebRTC is the browser-native real-time communication stack (ICE/STUN/TURN for NAT traversal, DTLS for encryption, SRTP for media, SCTP for data channels). Its role in Graphshell is strictly **real-time media** inside Coop sessions:

- **Screen share**: host captures a Servo webview tile as a `MediaStream` and sends it to guests via WebRTC media tracks. Latency: 100–300ms typical. No iroh involvement — media is point-to-point via WebRTC.
- **Synchronized video playback**: when the active node is a video URL, each peer plays their own local copy; playback state (`play`, `pause`, `seek`) is synchronized over a WebRTC data channel or (simpler) the existing iroh Coop event stream. WebRTC is not strictly required for sync messages — iroh handles them — but is required if the host is streaming video directly rather than each peer loading independently.
- **WASM / browser fallback transport** (Tier 2+): if Graphshell ever runs in a browser tab, iroh's native QUIC is unavailable. WebRTC data channels (via `str0m`, a pure-Rust WebRTC stack) become the fallback P2P transport for Coop. Not in scope for any current phase.

**NAT traversal**: WebRTC uses ICE (STUN + TURN). iroh uses DERP relays. Both solve the same NAT problem independently. When WebRTC is added to Coop for media, iroh handles document sync and WebRTC handles media — each uses its own hole-punching stack. The iroh connection established for the Coop session can supply the signalling channel for WebRTC SDP exchange, avoiding a separate signalling server.

**Rust implementation**: `str0m` (pure Rust, no C deps) is the preferred WebRTC stack. `webrtc-rs` is an alternative but heavier. Neither is in the codebase today.

### 2.3 Nostr

Nostr is a signed-event pub-sub bus over WebSocket relays. Its role in Graphshell:

- **User identity** (`npub` / `UserIdentity`): cross-device identity anchor for Coop, flock, and wallet signing (see `coop_session_spec.md §15`).
- **Event publication**: public graph views, published snapshots, public profile (kind 0), follows list (kind 3).
- **DMs** (NIP-17 sealed): private out-of-band communication — Coop invite delivery, peer messaging.
- **Social graph** (NIP-02): follows list as a curated discovery mechanism for public graph content.
- **Relay-persisted events**: encrypted wallet export blobs (§3.5), public graph view announcements.

Nostr does **not** replace iroh or libp2p. It has no peer-to-peer path, no NAT traversal, and is unsuitable for sub-100ms streaming (TCP/WebSocket, 50–500ms relay round-trip). The real-time session channel is always iroh; Nostr provides identity and event history.

---

## 3. Nostr Features in Scope

### 3.1 Key management — the critical risk

Nostr identity loss is permanent: there is no key recovery mechanism in the base protocol. This is the primary UX risk for any Nostr-dependent feature.

Mitigations, in order of preference:

1. **Hardware wallet as primary signer** (§16.2 of coop_session_spec): the `nsec` never touches the app. The hardware device is the key store. Loss of the device = lost key, but hardware wallets are designed to be backed up via seed phrase.
2. **Seed phrase backup**: user is shown their BIP-39 seed phrase on key generation and prompted to record it offline. Standard wallet UX.
3. **NIP-46 remote signing**: the key lives in a separate signing app (Amber on Android, Alby on desktop). Graphshell never holds the `nsec` — it requests signatures via the NIP-46 protocol over a local or remote channel.
4. **Key rotation** (no finalized NIP as of 2025): not yet implementable in a standard-compatible way. Track NIP progress.

Graphshell **must not** store the raw `nsec` in the app's local profile store without encryption. Acceptable storage: encrypted with a device-level secret (OS keychain / secure enclave), or not stored at all (delegate to NIP-46 signer).

### 3.2 Public profile and follows (NIP-01 kind 0, NIP-02 kind 3)

A user's Nostr public profile (kind 0) contains: display name, bio, avatar, NIP-05 identifier (user@domain), Lightning address. In Graphshell context this also surfaces:

- A list of public graph views the user has shared.
- A link to their public node collections or published snapshots (via addressable events, kind 30000–39999).

The follows list (NIP-02 kind 3) serves as a **curated discovery layer**: following someone means you can subscribe to their public graph publications. This is the "follow this person's research graph" primitive — genuinely Graphshell-native, not just a clone of Twitter follows.

**Management**: profile and follows are published as Nostr events to the user's chosen relays. Graphshell provides a profile settings panel. On save, a new kind 0 (or kind 3) event is signed and published, replacing the prior version on relays.

**Scope boundary**: Graphshell does not implement a general Nostr social timeline (kind 1 notes, replies, reposts). The social layer is limited to profile discovery and public graph view follows. Users who want full Nostr social features use a Nostr client.

### 3.3 Private DMs (NIP-17 sealed)

NIP-17 gift-wrapped DMs provide metadata-blind private messaging: relay operators cannot determine sender, recipient, or message time. Use cases in Graphshell:

- Coop session invite delivery (out-of-band, before iroh connection is established).
- Peer-to-peer messaging in the flock panel.
- Future: minichat history persistence (messages persisted on user's DM relay, loaded on next session).

DMs are delivered to the recipient's preferred DM relay (kind 10050 event). No relay required on the sender side beyond publishing the gift-wrapped event.

### 3.4 Blossom (NIP-B7) for graph media

Blossom is content-addressed file storage (SHA-256) over HTTP. Graphshell-native use:

- Media nodes (images, PDFs, video) are stored as Blossom blobs.
- The graph stores the content hash, not a URL — the content is verifiable and retrievable from any Blossom server holding that hash.
- Complements iroh-blobs for peer-to-peer transfer: iroh for direct peer exchange, Blossom for public availability on HTTP servers.

### 3.5 Wallet export / encrypted relay blobs (NIP-44)

Covered in `coop_session_spec.md §16.3`. Encrypted workspace snapshots published as Nostr addressable events (kind 30000+), NIP-44 encrypted. Key = passport, relay = storage. No infrastructure to run if using public relays.

### 3.7 NIP-84 Highlights (kind 9802) — clip publication

Covered in `viewer/2026-02-11_clipping_dom_extraction_plan.md §5`. When a user clips a DOM element and chooses to publish it, Graphshell signs a kind 9802 highlight event with the canonical source URL (`r` tag) and publishes to the user's relay set. This is an explicit user action — never automatic. The `nostr` mod (`mods/native/nostr`) handles signing and publication without a Lantern dependency.

### 3.6 Data Vending Machines (NIP-90) — future

NIP-90 DVMs are a compute marketplace: publish a job request (kind 5000–5999), receive results (kind 6000–6999), pay via Lightning Zap. Relevant future use cases:

- Graph summarisation or topic extraction as a DVM job.
- AI-assisted node annotation or link suggestion.
- Feed curation (algorithmic follows suggestions based on graph content).

Not in scope for any current phase. Noted as the integration point if compute features are added.

---

## 4. Verse and NIP-72 / NIP-29

Verse is the persistent communal hosting model — shared graph spaces with rotating host responsibilities and multiple long-term participants. This is distinct from Coop (ephemeral, single-host, small-n).

### 4.1 Why Verse needs libp2p, not just iroh

A Verse space with 10–50+ participants benefits from:

- **DHT-based peer discovery**: find other participants without a central coordinator.
- **gossipsub**: efficient epidemic broadcast for shared graph state updates across a swarm.
- **Content routing**: retrieve a graph node or blob by hash from any peer who has it, without knowing who.

iroh is optimised for direct point-to-point connections (2–5 peers). libp2p's Kademlia + gossipsub scales to the swarm topology Verse requires.

### 4.2 NIP-72 as Verse community layer

NIP-72 moderated communities map onto Verse semantics:

| NIP-72 concept | Verse equivalent |
| --- | --- |
| Community definition (kind 34550) | Verse space definition (name, hosts, policy) |
| Moderator `npub` set | Host set (may rotate) |
| Member post submission | Proposed shared graph mutation |
| Moderator approval (kind 4550) | Host approval → committed to shared graph |
| Community feed | Canonical shared graph view |

NIP-72 gives Verse a Nostr-native social layer for free: community membership is implicit (anyone can find and follow a community by its `npub`), approval history is public and verifiable, and the moderator set is on-chain in the community definition event.

### 4.3 NIP-29 as private Verse spaces

NIP-29 relay-enforced groups add access control: only members receive events, enforced by the relay. This maps onto private Verse spaces where the participant list is not public. The trade-off is that the relay becomes a trust point — this is acceptable for Verse spaces where participants trust a shared relay operator.

NIP-29 group ID format: `relay_host'group_id`. The relay's access control model replaces the full DHT-based peer discovery; it's simpler but less censorship-resistant.

### 4.4 Layer assignment for Verse

| Concern | Protocol |
| --- | --- |
| Peer discovery | libp2p Kademlia DHT |
| State replication across swarm | libp2p gossipsub |
| Direct blob transfer between peers | iroh-blobs (via libp2p-iroh bridge) |
| Host identity and community definition | Nostr (NIP-72 kind 34550) |
| Private space membership enforcement | Nostr NIP-29 (relay-enforced) |
| Host approval of mutations | Nostr (NIP-72 kind 4550 approval events) |
| Real-time presence within a Verse session | iroh datagram (same as Coop) |

---

## 5. Nostr Mod Plugin Surface (Future)

A Nostr mod would expose the Nostr social layer inside Graphshell as a first-class panel, rather than requiring users to switch to a separate Nostr client. The mod API surface it needs:

- **Signing capability**: access to the user's `UserIdentity` signing function (never the raw `nsec`) — implemented via NIP-46 remote signing protocol or a local signing interface.
- **Relay pool**: a managed set of WebSocket connections to the user's configured relays, shared across mods (not one connection pool per mod).
- **Event emit / subscribe**: publish events and subscribe to filters, scoped to the relay pool.
- **Graph integration hook**: when a Nostr event references a URL or a known `npub`, the mod can propose a graph node creation via the current reducer bridge carrier path (`GraphIntent` today; future command/planner entry may wrap it).

The interesting native integration is **Nostr→graph**: a kind 1 note referencing a URL becomes a clippable graph node. A NIP-23 long-form article becomes a content node. A kind 9802 highlight (NIP-84) annotating a webpage maps to a Graphshell annotation node. These are richer than embedding a foreign Nostr client — they make the graph the primary surface.

**Existing Nostr apps as mods**: possible but not the priority. A WebView-based mod could embed a Nostr web client (e.g. Snort, Nostrudel) with `window.nostr` injected via NIP-07 (browser extension signing API). Graphshell provides the NIP-07 signing bridge. The user gets a full Nostr client in a pane. This is a one-evening mod, not a major feature.

---

## 6. Relay Infrastructure Posture

### What requires relays

| Feature | Relay required? | Who runs it? |
| --- | --- | --- |
| `npub` signing only | No | — |
| Coop session signaling (invite delivery) | Yes | Any public relay |
| Profile (kind 0) + follows (kind 3) | Yes | Any public relay |
| NIP-17 sealed DMs | Yes (DM relay) | Any relay the recipient advertises |
| Wallet export blobs | Yes | Public relay acceptable (payload encrypted) |
| NIP-72 community / Verse space | Yes | Any public relay |
| NIP-29 private group | Yes | Trusted relay (enforces membership) |

### Do we need to run our own relay?

Not required. Public relays (dozens available, free) cover identity, profile, DMs, and community events. For reliability of your own events (relay churn is real — ~20% downtime in practice), publishing to 3–5 relays is the mitigation, not running your own.

Running a relay becomes attractive for:

- **Private NIP-29 Verse spaces**: you want to control membership enforcement.
- **Reliable wallet export blob persistence**: you want guaranteed availability of your own encrypted snapshots.
- **Organisational relay**: a Graphshell-native relay for users of a shared Verse space.

A self-hosted relay is a single process with SQLite backing (strfry, nostr-rs-relay, rnostr). It is not serious infrastructure — it can run on a VPS or even a home server. No ops burden beyond a process supervisor.

**Graphshell does not need to operate relay infrastructure** to ship any feature in the current roadmap. Running a relay is a user/operator option, not a platform dependency.

---

## 7. Protocol Interoperability Notes

### Curve mismatch (Nostr secp256k1 vs. iroh Ed25519)

These cannot be unified into a single keypair without a protocol change. The two-layer model (`UserIdentity` / `NodeId`) is the correct response — bind them via a signed assertion in the presence broadcast rather than trying to use one keypair for both purposes.

Implementation note as of 2026-03-10:

- Graphshell now carries a short-lived signed presence-binding assertion on the Verse discovery path.
- That assertion binds the transport `NodeId` to a user-identity claim for a scoped audience/TTL.
- The current local user-identity signer is now a dedicated secp256k1 lane, separate from the
  Ed25519 transport identity.
- Full closure still requires NIP-46 delegated signing so the `UserIdentity` lane can move out of
  local key custody while keeping the `NodeId` lane separate.

### libp2p-iroh bridge

The `libp2p-iroh` crate allows iroh's QUIC transport and NAT traversal to be used by a libp2p host. This means Verse's libp2p swarm can use iroh's superior hole-punching without reimplementing it. Use this bridge when implementing Verse — do not run iroh and libp2p as completely separate stacks.

### WebRTC signalling over iroh

WebRTC requires a signalling channel to exchange SDP offer/answer before the peer connection is established. In Graphshell, the existing iroh Coop session stream serves as the signalling channel — no separate signalling server needed. Flow:

1. Host sends SDP offer as a Coop session event over iroh.
2. Guest responds with SDP answer over the same iroh stream.
3. ICE candidates are exchanged over iroh.
4. WebRTC peer connection is established; media flows directly peer-to-peer (or via TURN if ICE fails).

This means WebRTC in Coop requires iroh to already be connected — WebRTC is an add-on for media, not a replacement for the session transport.

### Nostr and AT Protocol coexistence

`UserIdentity::DidPlc` (see `coop_session_spec.md §15.6`) is the hook for AT Protocol integration. If Graphshell ever adds public graph view sharing with Bluesky-style global discovery, `did:plc` identity enables federation with the AT Protocol AppView layer. The two identity systems do not conflict — a user can have both an `npub` and a `did:plc`, with Graphshell preferring whichever the user has configured.

---

## 8. WebRTC in Coop Sessions (Future / Tier 2)

WebRTC media features are not in scope for any current roadmap phase. This section documents the intended design when they are added.

### 8.1 Screen share

The host captures one or more Servo webview tiles as a `MediaStream` (platform screen-capture API or Servo offscreen render) and transmits it to guests via WebRTC video tracks. Guests see a live render of the host's view without needing a local Servo instance for that tile. Useful for: demos, pair browsing where the guest is on low-bandwidth, presenting a node to the group.

### 8.2 Synchronized video playback

When the active Coop node is a video URL (YouTube, direct MP4, etc.), each peer loads the URL independently in their own local Servo webview. Playback state is synchronized over the iroh Coop event stream — not via WebRTC data channels — because iroh is already present and reliable. `CoopContribution` variants `SeekVideo { position_secs }`, `PlayVideo`, `PauseVideo` carry the playback cursor. The host's webview is the authoritative playback cursor; guests follow.

WebRTC is only needed here if the host is directly streaming video frames to guests (e.g. DRM content the guest cannot load independently). In the common case (same public URL), iroh sync messages are sufficient and WebRTC is not required.

### 8.3 Rust implementation path

- `str0m` (pure Rust, no C deps): preferred. Handles ICE, DTLS, SRTP, SCTP. Maintained and production-tested.
- Signalling: iroh Coop session stream (see §7 above — no separate signalling server).
- TURN fallback: public TURN servers (Cloudflare, Twilio) or self-hosted `coturn`. Required only when ICE direct punch-through fails (~15% of connections on restricted networks).

### 8.4 Scope boundary

WebRTC in Graphshell is **Coop-only and media-only**. It does not replace iroh for document sync, does not replace libp2p for Verse swarm topology, and does not replace Nostr for signalling/identity. Adding WebRTC for media does not change any other layer assignment in this document.
