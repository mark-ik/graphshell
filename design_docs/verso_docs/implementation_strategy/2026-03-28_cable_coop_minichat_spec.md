<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Cable as Co-op Minichat Substrate

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: Adopt the Cable wire protocol as the chat substrate for co-op session minichat (deferred in coop_session_spec.md §9.3) and as a lightweight persistent cabal store for small-group (≲100 person) durable chat
**Out of scope**: Cable replacing Matrix for large-group / federated room chat or WebRTC signaling; Cable for Verse community-scale messaging

**Related docs**:

- [`coop_session_spec.md`](coop_session_spec.md) — co-op session authority model; §9.3 defers minichat
- [`../technical_architecture/VERSO_AS_PEER.md`](../technical_architecture/VERSO_AS_PEER.md) — Verso bilateral layer, iroh transport, Ed25519 identity
- [`../../graphshell_docs/implementation_strategy/social/comms/COMMS_AS_APPLETS.md`](../../graphshell_docs/implementation_strategy/social/comms/COMMS_AS_APPLETS.md) — Comms as a hosted surface family composing multiple chat lanes
- [`../research/2026-03-28_permacomputing_alignment.md`](../research/2026-03-28_permacomputing_alignment.md) — permacomputing alignment research; Cable identified as strongest actionable connection
- [Cable protocol spec](https://github.com/cabal-club/cable) — `cable.md`, `wire.md`, `handshake.md`, `moderation.md` (1.0-draft8)
- [cable.rs](https://github.com/cabal-club/cable.rs) — Rust implementation (5-crate workspace)

---

## 1. Why Cable

Co-op session minichat (coop_session_spec.md §9.3) needs a chat protocol. The options are: invent one, adopt an existing one, or defer indefinitely. Cable is a strong candidate for adoption because of deep structural alignment with the co-op bilateral layer:

| Property | Cable | Co-op / Verso |
|----------|-------|---------------|
| Identity primitive | Ed25519 keypair | Ed25519 keypair (Verso keychain) |
| Group secret | 32-byte cabal key (PSK) | `CoopSessionId` (UUID, extendable to 32-byte secret) |
| Transport assumption | Full-duplex encrypted binary stream | iroh streams (encrypted, full-duplex) |
| Message ordering | Causal DAG (`links` field → BLAKE2b hashes) | Not yet defined for chat |
| Deletion | `post/delete` by author | Not yet defined; aligns with ephemeral session model |
| Moderation | Subjective (admin/mod/user, local overrides) | Host-led authority (§4.2) — composable |
| Wire format | Binary, varint-framed, compact | iroh uses varint-framed streams natively |
| Sync model | Pull-based (hash request → post request) | iroh pull/push over streams |
| Rust crate | `cable.rs` (alpha, actively maintained) | Direct dependency candidate |
| Implementability | Designed for under-resourced teams; 7 post types, 7 message types | Matches Graphshell's minimal-dependency philosophy |
| Permacomputing alignment | Listed on permacomputing.net/projects; no corporate dependency | Graphshell design values (see research doc) |

Cable was explicitly designed as a ground-up replacement for hypercore-based Cabal, motivated by the same problems Graphshell faces: append-only log limitations, tight coupling to Node.js, inability to delete, low implementability across languages.

---

## 2. Protocol Mapping

### 2.1 Transport: Cable Wire Protocol over iroh Streams

Cable's Noise handshake (`Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b`) establishes an encrypted channel and verifies both peers know the cabal key. Since iroh already provides:

- Authenticated, encrypted QUIC streams (peers identified by `NodeId`)
- Per-connection ALPN protocol negotiation

...the Cable Noise handshake is **redundant for transport encryption** but **still useful for cabal key verification**. Two options:

**Option A — Skip Noise, verify cabal key at application layer.** Run the Cable wire protocol directly over iroh streams. The co-op session join flow already verifies session membership (the host admits guests). Add a cabal-key-derived challenge-response at join time to confirm the guest holds the session secret.

**Option B — Run full Noise handshake inside iroh stream.** Treat the iroh stream as a raw transport pipe and run Cable's Noise handshake over it. Double encryption (iroh QUIC + Noise ChaChaPoly), but protocol-correct and uses the `handshake` crate unmodified.

**Recommendation: Option A.** Skip Noise, use iroh's existing encryption, verify the cabal key via a lightweight application-layer challenge. This avoids double encryption overhead and simplifies the dependency surface. The `cable` encoding/decoding crate is still used for the wire protocol; only the `handshake` crate is bypassed.

### 2.2 Identity: Verso Keypair → Cable Per-Cabal Keypair

Cable requires a **unique Ed25519 keypair per cabal** to prevent cross-cabal replay attacks. Verso holds a single master Ed25519 keypair in the OS keychain.

**Derivation**: For each co-op session, derive a session-scoped signing keypair:

```
session_seed = BLAKE2b(verso_master_secret || session_cabal_key)
session_keypair = Ed25519::from_seed(session_seed)
```

This produces a deterministic, unique keypair per session that:
- Prevents cross-session replay (different cabal key → different keypair).
- Does not require storing per-session keys (re-derivable from master key + session identifier).
- Is compatible with Cable's per-cabal uniqueness requirement.

The session public key is what other peers see as the Cable identity for this session.

### 2.3 Cabal Key: Derived from CoopSessionId

`CoopSessionId` is currently a UUID. For Cable integration, derive a 32-byte cabal key:

```
cabal_key = BLAKE2b-256(session_id.as_bytes() || host_public_key)
```

Including the host's public key ensures that even if two sessions have the same UUID (collision or reuse), their cabal keys differ. The cabal key is distributed to guests as part of the session invite (alongside the iroh `NodeId` and invite token already specified in coop_session_spec.md).

### 2.4 Post Type Mapping

| Cable post type | Co-op minichat usage |
|-----------------|---------------------|
| `post/text` | Chat messages in the session minichat panel |
| `post/delete` | Author-initiated message deletion |
| `post/info` | Display name and avatar metadata for chat presence |
| `post/topic` | Session topic / description (set by host) |
| `post/join` | Announced when a participant enters the chat channel |
| `post/leave` | Announced when a participant leaves or disconnects |

All six post types map directly. No custom post types are needed for the initial minichat feature.

### 2.5 Channel Mapping

Cable organizes messages into named channels within a cabal. For co-op minichat:

- **Single channel**: `"session"` — the default and only channel for the initial rollout.
- **Future**: per-topic channels within a co-op session (e.g., `"links"` for URL sharing, `"notes"` for collaborative notes). Cable's channel model supports this natively.

### 2.6 Request/Response Sync

Cable's pull-based sync is well-suited to co-op's session lifecycle:

1. **On join**: Guest sends a `Channel Time Range Request` for the `"session"` channel from session start time to now (with `time_end = 0` for live subscription). Receives hashes, requests missing posts.
2. **Live**: The live subscription delivers new post hashes as they are created. Guest requests full posts for unknown hashes.
3. **On reconnect**: Guest sends a new time-range request from their last-known timestamp. Fills gaps without re-fetching everything.
4. **On session end**: No special teardown needed. The in-memory store is discarded (or snapshot-archived per coop_session_spec.md §8).

---

## 3. Moderation Integration

### 3.1 The Tension

Cable's moderation is **subjective**: every peer is their own ultimate authority. Co-op is **host-led**: the host has non-negotiable supremacy (coop_session_spec.md §4.2).

### 3.2 Resolution: Host as Admin Seed + Subjective Layer on Top

Cable's moderation spec supports a **moderation seed**: a set of public keys provided at cabal join time as initial trusted authorities. Map the co-op host's session public key as the sole admin seed.

This means:
- **From the host's perspective**: They are admin (always true in Cable). Their moderation actions (hide user, drop post, block) are authoritative for their own view.
- **From a guest's perspective**: The host is their initial admin seed. By default, the host's moderation decisions propagate to all guests (Cable's transitivity rule for admins). Guests can locally override if they choose (Cable's local-user-supremacy rule).

**Effect**: The host's authority is respected by default (matching co-op §4.2), but guests retain the Cable-native ability to make local moderation decisions (hiding messages, muting users) without host involvement. This is **more permissive** than co-op's current spec, but in a good way: it gives guests agency over their own view without undermining host control over shared content.

### 3.3 Moderation Actions Relevant to Co-op

| Cable action | Co-op meaning |
|-------------|---------------|
| Hide user | Locally mute a participant's chat messages (guest-local; does not affect shared graph view) |
| Hide post | Locally hide a specific message |
| Drop post | Remove a message from local storage AND display |
| Block user | Stop syncing chat posts with/from a participant. **Note**: This is chat-layer only; it does not affect co-op graph authority (role gating handles that) |

### 3.4 Private Moderation

Cable allows moderation actions to be marked **local-only** (encrypted, never synced). This means a guest can mute another participant without anyone knowing — including the host. This is appropriate for the chat layer; graph-level authority remains under host control via the role model.

---

## 4. Storage Model

Cable in Graphshell operates in two distinct storage modes depending on use context.

### 4.1 Mode A — Ephemeral (Co-op Session Minichat)

Co-op sessions are ephemeral. Cable's in-memory store is the correct default for the minichat lane. Chat history lives in RAM for the duration of the session and is not automatically persisted.

**Snapshot archival**: at session end (coop_session_spec.md §8), if the guest takes a snapshot, the chat log can optionally be included:

- Serialize all `post/text` posts in causal order.
- Include in the `SessionCapsule` as a sidecar artifact (e.g., `chat_log.gemtext` or `chat_log.json`).
- Respect `post/delete`: deleted posts are excluded.
- Respect moderation: posts hidden/dropped by the user's local moderation view are excluded.

### 4.2 Mode B — Persistent (Named Cable Cabal)

A **named Cable cabal** is a persistent, named group that outlives any individual co-op session. It is synced peer-to-peer whenever members are online and stored locally in a redb or fjall partition.

**Scale rationale**: Cable cabals are not designed for Matrix-scale rooms. In practice a cabal is unlikely to exceed ~100 participants. The storage burden is trivially small:

| Group size | Activity | Post size (avg) | Storage/year |
|-----------|----------|-----------------|-------------|
| 10 people | 20 msgs/day/person | ~200 bytes | ~15 MB |
| 50 people | 20 msgs/day/person | ~200 bytes | ~73 MB |
| 100 people | 20 msgs/day/person | ~200 bytes | ~146 MB |

These figures are comfortably within local device storage budgets. A full year of active chat from 100 users is smaller than a single high-resolution photograph.

**Store schema** (redb/fjall):

| Table | Key | Value | Purpose |
|-------|-----|-------|---------|
| `posts` | `Blake2bHash` (32 bytes) | encoded `Post` | Primary post store; deduplication |
| `channel_timeline` | `(channel_name, timestamp_ms, Blake2bHash)` | `()` | Time-range request index |
| `channel_heads` | `channel_name` | `Vec<Blake2bHash>` | Current DAG heads per channel |
| `channel_state` | `channel_name` | `ChannelStateProjection` | Materialized topic + member info |
| `peer_vectors` | `PublicKey` (32 bytes) | `VersionVector` | Per-peer sync state |

**Garbage collection**: Cable recommends discarding posts older than a rolling window (default: 1 week). For persistent cabals, this default is **disabled** — history is kept indefinitely unless the user explicitly applies a retention policy. A configurable per-cabal TTL is the recommended UX knob.

**Identity**: Each named cabal has:

- A cabal key (32 bytes, randomly generated at creation, shared out-of-band to new members)
- A human-readable name (local metadata only, not part of the Cable protocol)
- A derived per-user keypair per cabal (same derivation as §2.2, but seeded from the cabal key rather than a `CoopSessionId`)

**Discovery**: Cable has no built-in discovery mechanism. Named cabals are joined by receiving the cabal key out-of-band — the same model as Verso's existing pairing flows (QR code, invite link, mDNS). A cabal invite can be encoded as a `verse://cabal/{cabal_key_hex}/{display_name}` URI alongside the host's `NodeId`.

**Relationship to co-op sessions**: A co-op session can optionally be backed by a named cabal rather than a fresh ephemeral cabal:

- Host creates or selects an existing cabal when starting the session.
- Chat history from previous sessions with the same cabal is visible.
- Post-session, new messages are retained in the persistent store automatically — no explicit snapshot needed.

### 4.3 Mode Selection

| Context | Mode | Cabal key source | Store |
|---------|------|-----------------|-------|
| Co-op session (default) | Ephemeral (A) | Derived from `CoopSessionId` | In-memory |
| Co-op session (opt-in) | Persistent (B) | User-selected named cabal | redb/fjall |
| Named group cabal | Persistent (B) | Randomly generated at creation | redb/fjall |

---

## 5. Dependency Surface

### 5.1 Crates from cable.rs

| Crate | Use | Size / deps |
|-------|-----|-------------|
| `cable` | Binary encoding/decoding of posts and messages | `sodiumoxide`, `desert` (vendored) |
| `cable_core` | Peer manager, in-memory store, stream handling | `async-std`, `futures`, `sodiumoxide` |
| `handshake` | Noise handshake | `snow` — **skip if Option A (§2.1)** |
| `desert` | Serialization (vendored in cable.rs workspace) | Minimal |
| `length_prefixed_stream` | Varint framing | Minimal |

### 5.2 Dependency Considerations

- **`sodiumoxide`**: Wraps libsodium. Provides Ed25519 and BLAKE2b. Graphshell already uses `ed25519-dalek` for Verso's keypair and `blake3` for content hashing. Options:
  - Accept `sodiumoxide` as an additional dependency (simplest; maintains cable.rs compatibility).
  - Fork `cable` crate to use `ed25519-dalek` + `blake2` (pure Rust, no C dependency, but diverges from upstream).
  - Contribute pure-Rust crypto backend to cable.rs upstream (ideal long-term).
- **`async-std`**: `cable_core` uses `async-std`. Graphshell uses `tokio`. Options:
  - Use `cable` (encoding only) without `cable_core`; implement own peer manager on tokio.
  - Use `tokio-compat` shim for `cable_core`.
  - Contribute tokio backend to cable.rs upstream.

**Recommendation**: Start with `cable` crate only (encoding/decoding). Implement the peer manager and store directly in Verso's `SyncWorker` / co-op handler using tokio. This minimizes dependency surface and avoids the async-std/tokio tension. The `cable` crate's only significant dependency is `sodiumoxide` for crypto.

### 5.3 Feature Gate

All Cable functionality behind `--features cable`:

```toml
[features]
cable = ["dep:cable"]
```

Graphshell without `cable` has no minichat. Co-op still functions (presence, graph sharing, roles) — just no chat panel.

---

## 6. Intent Surface

### 6.1 New GraphIntent Variants

```rust
// Chat message operations (reducer-owned, durable within session)
SendCoopChatMessage { session_id: CoopSessionId, content: String }
DeleteCoopChatMessage { session_id: CoopSessionId, post_hash: Blake2bHash }
SetCoopChatTopic { session_id: CoopSessionId, topic: String }
```

### 6.2 New WorkbenchIntent Variants

```rust
// Chat panel UI (workbench-owned, pane/tile mutations)
OpenCoopChatPanel
CloseCoopChatPanel
FocusCoopChatInput
```

### 6.3 Signal/Channel Events (Non-Durable)

- Incoming `post/text`, `post/delete`, `post/info`, `post/join`, `post/leave` from Cable sync → rendered directly in the chat panel without entering the undo/WAL pipeline.
- Chat messages are **not** graph semantic events. They do not create nodes or edges. They exist only in the Cable in-memory store for the session lifetime.

### 6.4 Boundary Rule

Chat intents do not cross into graph authority. `SendCoopChatMessage` does not create a graph node. If a user wants to promote a chat message to a graph node, that is a separate explicit action (`ClipCoopChatToNode`), conceptually equivalent to a web clip.

---

## 7. Relationship to Comms Lanes

COMMS_AS_APPLETS.md defines Comms as a hosted surface family composing multiple communication lanes. Cable fits as the **bilateral session chat lane**:

| Lane | Protocol | Scope | Persistence | Scale |
|------|----------|-------|-------------|-------|
| Session chat | **Cable** (over iroh) | Co-op session lifetime | Ephemeral (in-memory) | 2–20 |
| Small-group cabal | **Cable** (over iroh) | Named cabal lifetime | Persistent (redb/fjall) | ≲100 |
| Durable room chat | Matrix | Community / room lifetime | Persistent (homeserver) | 5–1000+ |
| Public/relay social | Nostr | Global / relay-scoped | Relay-dependent | Unbounded |

Cable covers two tiers: ephemeral session chat and persistent small-group cabals. Matrix takes over at scales where homeserver infrastructure and federation are justified — particularly when WebRTC group signaling (MSC3401) or interoperability with existing Matrix deployments is needed. A future unified Comms surface presents these lanes as tabs within one panel; the Comms applet abstracts over the structural differences between Cable posts and Matrix events.

---

## 8. Rollout Plan

### Phase 1 — Wire Format Integration

1. Add `cable` crate as an optional dependency (`--features cable`).
2. Implement session-scoped keypair derivation (§2.2).
3. Implement cabal key derivation from `CoopSessionId` (§2.3).
4. Implement Cable post encoding/decoding for the six post types (§2.4).
5. Implement Cable request/response message encoding/decoding.
6. Unit tests: round-trip encoding, keypair derivation determinism, cabal key derivation.

### Phase 2 — Chat Channel over iroh

1. Register a new ALPN for co-op minichat alongside the existing co-op presence ALPN.
2. Implement cabal-key challenge-response at session join (Option A, §2.1).
3. Implement in-memory Cable post store (or adapt `cable_core`'s).
4. Implement pull-based sync: `Channel Time Range Request` on join, live subscription for new posts.
5. Wire incoming posts to the chat panel renderer.
6. Wire outgoing messages from chat input to Cable post creation + broadcast.

### Phase 3 — Moderation and UX

1. Wire host as admin seed (§3.2).
2. Implement local hide/mute actions in the chat panel.
3. Implement `post/delete` (author-initiated).
4. Implement chat topic display (from `post/topic`).
5. Implement chat log snapshot archival (§4.2).

### Phase 4 — Polish

1. Chat panel integration with co-op presence overlay (§9.1 of coop_session_spec.md).
2. Role-sensitive chat input: `ViewOnly` guests can read chat but input is disabled (or enabled — design decision; chat may be permitted even for view-only participants).
3. Performance: ensure Cable sync does not compete with co-op presence datagram channel for iroh bandwidth.

### Phase 5 — Persistent Named Cabals

1. Implement redb/fjall Cable post store with the schema defined in §4.2.
2. Implement per-cabal keypair derivation from randomly generated cabal key (§4.2 Identity).
3. Implement cabal invite URI encoding/decoding (`verse://cabal/…`).
4. Wire cabal creation, join, and leave flows into the intent surface (see §6 extensions below).
5. Expose cabal selection when starting a co-op session (§4.3 opt-in persistent mode).
6. Implement configurable per-cabal TTL / retention policy UI.
7. Implement `Channel Time Range Request` responses from the persistent store (allows peers who were offline to catch up on history).

---

## 9. Open Questions

1. **Should `ViewOnly` guests be able to chat?** Chat is not a graph mutation. It is reasonable to allow view-only guests to participate in text chat while restricting graph operations. This would be a divergence from the current coop_session_spec.md role model, which applies roles to "the shared co-op surface" broadly. Decision: likely yes (chat is social, not editorial), but needs explicit sign-off.

2. **Crypto backend convergence.** `sodiumoxide` (C libsodium) vs `ed25519-dalek` + `blake2` (pure Rust). Short-term: accept both. Long-term: contribute pure-Rust backend to cable.rs or maintain a thin fork.

3. **Cable upstream contribution.** If Graphshell implements a tokio-native peer manager and/or persistent store for Cable, contributing these upstream benefits both projects. The cable.rs README explicitly lists "networking layer not designed yet" and "no persistent storage" as known gaps.

4. **Multi-channel in a session.** Cable supports multiple named channels per cabal. Initial rollout uses a single `"session"` channel. Should additional channels (e.g., `"links"`, `"notes"`) be specced now or deferred?

5. **Chat message size limit.** Cable enforces a 4 KiB max for `post/text`. This is generous for chat but may be restrictive if chat messages carry rich content (e.g., pasted code blocks). Acceptable for initial rollout; revisit if users hit the limit.

6. **Named cabal membership list.** Cable has no explicit membership concept beyond peers who have posted `post/join`. For a persistent cabal, how is membership managed when a member has been offline for a long time? Is the `channel_state` projection sufficient, or does the persistent store need an explicit membership table separate from post history?

7. **Cabal → Verse promotion.** A named cabal that outgrows ~100 people, or whose members want community-scale replication and discovery, should be promotable to Verse. What does that migration path look like? Verse could seed the existing cabal key as a community identifier, or members could re-key into a Verse-native identity. This is the natural upper bound for Cable's scale tier.
