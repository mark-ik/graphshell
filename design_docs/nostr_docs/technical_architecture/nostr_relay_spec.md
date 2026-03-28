<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graphshell Embedded Nostr Relay

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: Embedded NIP-01-compliant relay server running inside Graphshell as a NostrCore capability. Covers operating modes, storage model, NIP requirements, intent surface, and relationship to external relay software.

**Related docs**:

- [`../implementation_strategy/nostr_runtime_behavior_spec.md`](../implementation_strategy/nostr_runtime_behavior_spec.md) — NostrCore client-side runtime contract; relay policy profiles
- [`../implementation_strategy/nostr_core_registry_spec.md`](../implementation_strategy/nostr_core_registry_spec.md) — NostrCore capability-provider boundary; ModManifest
- [`../../graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md`](../../graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md) — Network layer assignments; §6 "Do we need to run our own relay?"
- [`../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Capsule server pattern (Gemini/Gopher/Finger); structural analog for the relay
- [`../../verso_docs/implementation_strategy/coop_session_spec.md`](../../verso_docs/implementation_strategy/coop_session_spec.md) — Flock model (§14); flock members are the user set for Flock relay mode

---

## 1. What This Is

Graphshell can run a NIP-01-compliant Nostr relay server as an **embedded service** within the app — a NostrCore capability alongside the existing client-side relay subscriber/publisher. The relay:

- Listens on a local WebSocket endpoint (default: `ws://localhost:4869`, configurable)
- Optionally exposes an external address for inbound connections from the network
- Stores Nostr events in a fjall partition (not SQLite — keeping the dependency surface consistent with the rest of Graphshell)
- Is fully owned and supervised by `NostrCore` / `ControlPanel`
- Is toggled off by default; requires explicit user activation

This follows the exact same architectural pattern as the Gemini, Gopher, and Finger capsule servers in Verso ([VERSO_AS_PEER.md §Gemini Capsule Server](../../verso_docs/technical_architecture/VERSO_AS_PEER.md)): a small protocol server co-located with the app, serving content the user owns, activated by a `GraphIntent`.

The relay is **not** a replacement for dedicated relay software (`strfry`, `nostr-rs-relay`, `rnostr`) for high-traffic or production use cases. See §9 for the boundary.

---

## 2. Why Run a Relay

The network_architecture.md §6 already establishes that running a relay becomes attractive in three scenarios. Expanded here with the full benefit surface:

| Benefit | Mechanism | Mode |
|---------|-----------|------|
| **NIP-65 outbox reliability** | Publish your own events to your relay as one of your write relays (NIP-65 kind 10002). Relay churn on public relays is ~20% downtime in practice; your relay guarantees your events are retrievable | Personal |
| **Flock event cache** | Store events from your flock (coop_session_spec.md §14) locally. Access their notes/posts offline without polling public relays on every open | Flock |
| **NIP-46 bunker transport** | A localhost relay eliminates the third-party relay dependency from the NIP-46 signing flow entirely. See §2.1. | Personal |
| **Wallet export blob persistence** | Guaranteed availability of your NIP-44-encrypted workspace snapshots, independent of public relay availability | Personal |
| **Private NIP-29 group spaces** | NIP-29 requires a relay that enforces membership rules. Without your own relay, you depend on a third party's enforcement — and their uptime | Community |
| **Verse community relay** | A Verse space can offer its members a relay as a community service, hosted by members who are online | Community |
| **Permacomputing alignment** | You become a peer contributing to the network, not just a consumer extracting from public infrastructure | All |

### 2.1 NIP-46 Bunker Transport — Why Localhost Changes Everything

NIP-46 (Nostr Connect) is the protocol for remote signing: your `nsec` lives on one device or app (the "bunker"), and other clients request signatures from it by exchanging messages through a shared relay. The relay is just a transport — it never sees the key. But it is a *required* intermediary, and its reliability directly determines whether signing works.

**The problem with public relays as bunker transport:**

The NIP-46 signing handshake has a tight timeout window. A client sends a `connect` or `sign_event` request to the relay; the bunker must pick it up, sign, and return the response before the client gives up. In practice this means:

- If the relay has transient latency (very common — public relays are not low-latency infrastructure), the handshake silently times out. The user sees a spinner that eventually dies with a generic "connection failed" error.
- If the relay is down for maintenance or overloaded, NIP-46 is completely non-functional. The user cannot sign anything or set up a new identity until the relay recovers.
- If the relay drops the WebSocket while the bunker is mid-session, the session breaks and has to be re-established from scratch.
- Public relays increasingly rate-limit or require payment; a bunker session requires a persistent subscription, which some relays deprioritize.

This is not a theoretical concern. Users setting up `npub` identities via NIP-46 routinely encounter relay failures at exactly the moment they are trying to establish a new signing session — the highest-friction point in the whole onboarding flow.

**Why a localhost relay fixes it:**

When the embedded relay is running, the bunker URL becomes:

```
bunker://npub1...?relay=ws://localhost:4869
```

Both the signing client (Graphshell's NostrCore client) and the bunker app connect to `ws://localhost:4869`. The relay is a loopback TCP socket on the same machine. Round-trip latency is measured in microseconds, not hundreds of milliseconds. There is no network path to fail. The relay is available as long as Graphshell is running.

The NIP-46 handshake that timed out on a congested public relay now completes before any human-perceptible delay.

**Graphshell as the bunker itself:**

Graphshell already holds the user's `nsec` equivalent — the Verso Ed25519 keypair in the OS keychain, and the Nostr secp256k1 signing key in `NostrCore`. A natural extension is to expose these as a NIP-46 bunker: other NIP-46-capable clients on the same machine (or local network in Flock mode) point their bunker URL at Graphshell's relay, and Graphshell handles signing requests via `NostrCore`'s existing signer boundary (§5 of nostr_runtime_behavior_spec.md — no raw key exposure to the requesting client).

This makes Graphshell a zero-configuration key manager for any NIP-46-aware Nostr client on the user's machine, without ever exposing the `nsec` or requiring a third-party signing service.

---

## 3. Operating Modes

The relay runs in one of three modes. Mode determines storage policy, connection policy, and NIP requirements.

### 3.1 Personal Mode

Stores only the user's own events (matching the user's `npub` as `pubkey`).

- **Who can connect**: localhost only by default; no external inbound connections
- **Who can write**: only the local user (via `NostrCore` client publishing to `ws://localhost:4869`)
- **Who can read**: any local process or localhost client
- **Storage policy**: keep all events indefinitely; optionally apply per-kind retention limits
- **Use case**: NIP-65 outbox, NIP-46 bunker transport, wallet blob persistence

### 3.2 Flock Mode

Stores the user's own events plus events from flock members (coop_session_spec.md §14 `FlockEntry`).

- **Who can connect**: localhost + flock members by IP or Nostr AUTH (NIP-42)
- **Who can write**: local user (unrestricted); flock members (their own events only, enforced by AUTH)
- **Who can read**: local user (all stored events); flock members (their own + user's events)
- **Storage policy**: user's events kept indefinitely; flock member events subject to a configurable rolling window (default: 30 days) or explicit keep-list
- **Use case**: flock event cache, offline access to collaborators' posts, shared session archive

**Storage budget**: At flock scale (~20 people, 50 events/day/person), storage is ~10–50 MB/year. Trivially manageable.

### 3.3 Community Mode

Open or membership-gated relay for a Verse space or other group.

- **Who can connect**: configurable — open (any client) or restricted (NIP-29 group members via AUTH)
- **Who can write**: configurable per community policy; NIP-29 enforces membership
- **Who can read**: configurable; public or member-only
- **Storage policy**: community-configured; rotating window or explicit archive policy
- **Use case**: Verse community relay, NIP-29 private group, organisational relay

**Note**: Community mode is the only mode where the relay is exposed externally by default. The user should understand they are operating infrastructure for others. ControlPanel surfaces this explicitly at activation.

---

## 4. Storage Model

### 4.1 Store Layout (fjall)

One fjall partition tree per relay instance (`nostr_relay_{mode}` or `nostr_relay_{cabal_id}` for named community relays):

| Partition | Key | Value | Purpose |
|-----------|-----|-------|---------|
| `events` | `event_id` (32 bytes, SHA-256) | encoded `NostrEvent` | Primary event store; deduplication |
| `by_pubkey_time` | `(pubkey, created_at_desc, event_id)` | `()` | Author + time-range filter evaluation |
| `by_kind_time` | `(kind, created_at_desc, event_id)` | `()` | Kind filter evaluation |
| `by_tag` | `(tag_name, tag_value, event_id)` | `()` | Tag filter evaluation (e-tags, p-tags, etc.) |
| `deleted_ids` | `event_id` | `deleted_at` | NIP-09 deletion tombstones |
| `relay_meta` | `"info"` | `RelayInfoDocument` | NIP-11 self-description |
| `auth_tokens` | `session_token` (ephemeral) | `(pubkey, expiry)` | NIP-42 AUTH session tracking |

### 4.2 Filter Evaluation

NIP-01 `REQ` filters evaluate against the three index partitions:

1. If `ids` filter is present: direct `events` lookup by ID.
2. If `authors` filter is present: scan `by_pubkey_time` for matching pubkeys within `since`/`until`.
3. If `kinds` filter is present: scan `by_kind_time` for matching kinds within `since`/`until`.
4. Tag filters: scan `by_tag` for matching `(tag_name, tag_value)` pairs.
5. Intersection of all filter dimensions applied in memory over the candidate set.

### 4.3 Deletion (NIP-09)

When a kind-5 deletion event is received from the event's original author:

- The referenced event IDs are added to `deleted_ids`.
- The original events are removed from all index partitions.
- The kind-5 event itself is stored (so peers can learn about the deletion).
- Future `REQ` filters skip `deleted_ids` entries.

### 4.4 Storage Budgeting

The relay reports storage usage through the same `PeerStorageReport`-style diagnostics as the bilateral storage model in VERSO_AS_PEER.md. The ControlPanel can show "relay is using X MB" alongside other storage consumers.

---

## 5. NIP Requirements by Mode

| NIP | Description | Personal | Flock | Community |
|-----|-------------|----------|-------|-----------|
| NIP-01 | Basic protocol: EVENT, REQ, CLOSE, NOTICE | Required | Required | Required |
| NIP-09 | Event deletion | Required | Required | Required |
| NIP-11 | Relay information document (HTTP GET) | Required | Required | Required |
| NIP-42 | AUTH: relay authentication | Optional | Required | Required |
| NIP-29 | Relay-based groups (membership enforcement) | No | No | Optional |
| NIP-70 | Protected events (relay-only distribution) | Optional | Optional | Optional |

### 5.1 NIP-11 Relay Information Document

The relay serves its NIP-11 document at `GET /` with `Accept: application/nostr+json`. The document is constructed from `RelayInfoDocument` in the store and includes:

- `name`: user-configured relay name (default: `"{display_name}'s Graphshell relay"`)
- `description`: user-configured description
- `pubkey`: the user's `npub` (relay operator identity)
- `supported_nips`: populated from the active NIP set for the current mode
- `software`: `"graphshell-nostr-relay"`
- `version`: Graphshell version string

### 5.2 NIP-42 AUTH

For Flock and Community modes, the relay uses NIP-42 challenge-response to authenticate connecting clients:

1. On WebSocket connect, relay sends `["AUTH", "<challenge>"]`.
2. Client signs and returns `["AUTH", <signed-kind-22242-event>]`.
3. Relay verifies signature and checks pubkey against the allowed set (flock members or community members).
4. Unauthenticated clients in restricted modes receive `NOTICE "restricted: authentication required"` on write attempts and on restricted reads.

### 5.3 NIP-29 Community Groups

In Community mode with NIP-29 enabled, the relay enforces group membership via `kind:9000`-`kind:9009` admin events. Group admins can add/remove members; the relay rejects writes from non-members. This is the correct substrate for private Verse spaces that want relay-enforced membership without exposing events to public relays.

---

## 6. NostrCore Ownership

The relay follows the same ownership model as all other NostrCore services:

- **Lifecycle**: owned by `NostrCore` / `ControlPanel`. Start/stop via `GraphIntent`. No ad hoc background tasks.
- **WebSocket server**: a `tokio`-native WebSocket listener (using `tokio-tungstenite` or equivalent). The server task is supervised by `ControlPanel` alongside the existing relay worker.
- **Secret boundary**: the relay never has access to the user's `nsec` or signing key. It stores and serves public events. Auth verification uses public key signature checking only.
- **Graph boundary**: the relay does not mutate graph state. Events stored in the relay are not automatically projected into the semantic graph — that is a separate explicit action via `NostrCore`'s event-to-intent pipeline.

### 6.1 Relay Worker Shape

```rust
pub struct NostrRelayWorker {
    mode: NostrRelayMode,
    store: NostrRelayStore,       // fjall-backed
    listener: TcpListener,
    auth_sessions: HashMap<SessionToken, AuthRecord>,
    flock_pubkeys: BTreeSet<Pubkey>,     // populated from flock store in Flock mode
    community_members: BTreeSet<Pubkey>, // populated from Verse community in Community mode
    command_rx: mpsc::Receiver<NostrRelayCommand>,
    output_tx: mpsc::Sender<NostrRelayOutput>,
    diagnostics: DiagnosticsWriteHandle,
}
```

### 6.2 Worker Commands and Outputs

```rust
pub enum NostrRelayCommand {
    Stop,
    UpdateFlockMembers(BTreeSet<Pubkey>),
    UpdateCommunityMembers(BTreeSet<Pubkey>),
    SetPolicy(NostrRelayPolicy),
}

pub enum NostrRelayOutput {
    Started { addr: SocketAddr },
    Stopped,
    ClientConnected { pubkey: Option<Pubkey>, addr: SocketAddr },
    ClientDisconnected { pubkey: Option<Pubkey> },
    EventStored { event_id: EventId, kind: u32, pubkey: Pubkey },
    EventRejected { reason: String },
    StorageWarning { used_bytes: u64, limit_bytes: u64 },
}
```

---

## 7. GraphIntent Wiring

```rust
// Relay lifecycle (reducer-owned)
StartNostrRelay { mode: NostrRelayMode, port: Option<u16> }
StopNostrRelay
SetNostrRelayMode { mode: NostrRelayMode }
SetNostrRelayPolicy { policy: NostrRelayPolicy }
SetNostrRelayInfo { name: Option<String>, description: Option<String> }
```

Where:

```rust
pub enum NostrRelayMode {
    Personal,
    Flock,
    Community { nip29_enabled: bool, open: bool },
}

pub struct NostrRelayPolicy {
    pub max_event_size_bytes: u32,      // default: 65536 (64 KiB)
    pub max_subscriptions_per_client: u32,
    pub allowed_kinds: Option<BTreeSet<u32>>,   // None = all kinds
    pub blocked_pubkeys: BTreeSet<Pubkey>,
    pub retention_window_days: Option<u32>,     // None = keep indefinitely
}
```

WorkbenchIntent (pane/UI only):

```rust
OpenNostrRelayPanel
CloseNostrRelayPanel
```

---

## 8. ModManifest Extension

`NostrCore`'s `ModManifest.provides` gains one new capability:

```rust
// Add to existing provides list:
"nostr:relay-serve"
```

This capability is:
- Denied by default (relay is off unless user activates it)
- Required for `StartNostrRelay` to succeed
- Separate from `nostr:relay-subscribe` and `nostr:relay-publish` (client-side capabilities)

The relay capability requires:
- `network` (if external mode; Personal localhost-only does not)
- `storage:write` (fjall partition)

---

## 9. Relationship to External Relay Software

The embedded relay is not a replacement for dedicated relay software in all cases.

| Use case | Embedded relay | External relay (strfry / nostr-rs-relay / rnostr) |
|----------|---------------|--------------------------------------------------|
| Personal NIP-65 outbox | Good fit | Overkill |
| Flock event cache | Good fit | Unnecessary complexity |
| NIP-46 bunker transport | Good fit | Workable but requires separate process management |
| Small private group (< ~100 members) | Good fit | Workable |
| High-traffic public relay | Not suitable | Correct tool |
| Production community relay (always-on server) | Not suitable (desktop app) | Correct tool |
| Relay on a VPS / always-on machine | Not suitable | Correct tool |

**The embedded relay is designed for desktop use** — it runs when Graphshell is running and goes offline when the user closes the app. For 24/7 availability, a VPS-hosted relay (`strfry` + systemd) is the right choice. Graphshell's relay policy settings (`Strict`/`Community`/`Open` in `nostr_runtime_behavior_spec.md`) support pointing the client at any external relay alongside or instead of the embedded one.

**Practical interoperability**: A user might run the embedded relay for personal mode (always available on localhost while the app is open) and also configure an external VPS relay for NIP-65 write redundancy. The client-side relay pool handles both simultaneously.

---

## 10. Diagnostics

```
nostr:relay:started         — Info   — relay is listening (addr, mode)
nostr:relay:stopped         — Info   — relay has shut down cleanly
nostr:relay:client_rejected — Warn   — client failed AUTH or is on blocklist
nostr:relay:event_rejected  — Warn   — event failed validation or policy check
nostr:relay:storage_warning — Warn   — storage usage approaching configured limit
nostr:relay:storage_error   — Error  — fjall write failure
nostr:relay:worker_crashed  — Error  — relay worker exited unexpectedly
```

---

## 11. Rust Implementation Notes

### 11.1 WebSocket Layer

`tokio-tungstenite` provides async WebSocket upgrade over `tokio::net::TcpListener`. The relay worker accepts TCP connections, upgrades to WebSocket, and handles the NIP-01 message loop per client in a spawned task.

### 11.2 Event Encoding

Nostr events are JSON-encoded on the wire (per NIP-01). Internal storage can use a more compact representation (e.g., `rkyv`-serialized struct) to avoid repeated JSON parsing. The `nostr` crate (`rust-nostr` / `nostr-sdk`) provides well-tested event types, signature verification, and filter evaluation — prefer using it rather than rolling custom event types.

### 11.3 Storage Backend

fjall replaces SQLite as the event store backend. This keeps the dependency surface consistent (Graphshell already uses fjall for the semantic graph) and avoids a separate SQLite dependency. The index schema in §4.1 maps naturally onto fjall's ordered key-value semantics.

### 11.4 Prior Art

`rnostr` (GitHub: `rnostr/rnostr`) is a pure-Rust Nostr relay with a plugin architecture and configurable storage backends. Its event filter evaluation and NIP-42 AUTH implementation are worth studying for the index traversal and AUTH handshake patterns, even if the Graphshell relay is implemented from scratch against the `nostr` crate types.

---

## 12. Rollout Plan

### Phase R1 — Personal relay (localhost only)

1. Create `NostrRelayWorker` with fjall store and NIP-01 message loop.
2. Implement EVENT, REQ, CLOSE, NOTICE handling.
3. Implement NIP-09 deletion.
4. Serve NIP-11 document via HTTP GET on same port.
5. Wire `StartNostrRelay`, `StopNostrRelay`, `SetNostrRelayPolicy` intents.
6. Add diagnostics channels.
7. Add relay panel to Settings → Sync (start/stop toggle, mode selector, storage usage).

Done gate: local Nostr client (e.g., `nak` CLI tool) can connect to `ws://localhost:4869`, publish events, and retrieve them with `REQ` filters.

### Phase R2 — Flock relay + NIP-42 AUTH

1. Implement NIP-42 challenge-response AUTH.
2. Wire flock pubkey list from `FlockEntry` store (coop_session_spec.md §14).
3. Implement per-pubkey write restrictions (flock members can write own events only).
4. Expose external address option (configurable bind address).
5. Add `UpdateFlockMembers` command for live flock sync.

### Phase R3 — Community relay + NIP-29

1. Implement NIP-29 group admin event handling (kind 9000–9009).
2. Wire community member set from Verse community membership.
3. Expose Community mode in the relay panel with NIP-29 toggle.
4. Add storage warning thresholds and rotation policy UI.

---

## 13. Open Questions

1. **Port default**: 4869 is unused and memorable. Should it be configurable only, or should there be a stable default that users can rely on for their NIP-65 relay list?

2. **Relay when Graphshell is minimized to tray**: should the relay continue serving when the main window is hidden but the process is alive? Probably yes — this is the right behavior for NIP-46 bunker use.

3. **TLS for external mode**: `wss://` requires a certificate. Options: self-signed (TOFU), Let's Encrypt via `rcgen` + ACME, or user-provided cert. The Gemini capsule server uses self-signed with ephemeral certs (VERSO_AS_PEER.md §TLS). The same approach works here for Flock mode; Community mode with public clients may want a real cert.

4. **Relay discovery via NIP-65**: Should Graphshell automatically add `ws://localhost:4869` to the user's kind-10002 relay list when the Personal relay is started? This is the natural thing to do but should be an opt-in, not automatic.

5. **Event projection into graph**: Events stored in the relay are not automatically graph nodes. But a user might want "everything in my relay that matches filter X → create graph nodes." This is a separate intent and out of scope for this spec, but worth noting as a natural follow-on.
