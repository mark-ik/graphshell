<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Nostr NIP Completion Plan

**Date**: 2026-03-10
**Status**: Active / planning
**Scope**: NIP coverage gaps in `NostrCoreRegistry` required for a functional Nostr client
**Parent**: [register/2026-03-08_sector_c_identity_verse_plan.md](register/2026-03-08_sector_c_identity_verse_plan.md)

**Related**:
- [register/nostr_core_registry_spec.md](register/nostr_core_registry_spec.md)
- [2026-03-05_nostr_mod_system.md](2026-03-05_nostr_mod_system.md)
- [register/2026-03-08_sector_c_identity_verse_plan.md](register/2026-03-08_sector_c_identity_verse_plan.md)

---

## 1. Current State

Landed in Sector C:

- **NIP-01**: full relay protocol — WebSocket connect/subscribe/publish via `TungsteniteRelayService`,
  subscription persistence, `NostrSignedEvent` / `NostrUnsignedEvent` / `NostrFilterSet` types
- **NIP-44**: encryption primitive — `nostr::nips::nip44` imported and used for NIP-46 envelope
- **NIP-46**: delegated remote signer — bunker URI parse, session key, encrypted RPC, permission memory
- **NIP-07**: host-owned bridge — `window.nostr` injection, `getPublicKey`/`signEvent`/`getRelays`,
  per-origin permission memory

### 1.1 NIP-01 protocol-gap closure receipt (2026-03-10)

Runtime gap closure landed in `NostrCoreRegistry` relay transport path:

- Added inbound frame handling for subscription and publish confirmation paths:
    - subscription observation now inspects `EOSE`, `NOTICE`, and `CLOSED` frames after `REQ`
    - publish acknowledgment now inspects `OK` and `NOTICE` frames after `EVENT`
- Publish receipts now reflect relay acknowledgment state (`accepted`/`rejected`) instead of
    send-only optimistic success.

Code path:

- `shell/desktop/runtime/registries/nostr_core.rs`
    - `observe_subscription_confirmation(...)`
    - `await_publish_ack(...)`
    - `recv_json(...)`

Targeted validation landed:

- `nostr_relay_worker_publish_observes_ok_ack`
- `nostr_relay_worker_publish_notice_marks_receipt_rejected`
- existing regression guard still green: `nostr_relay_worker_emits_req_event_and_close_over_websocket`

Residual NIP-01 follow-on (still open):

- General relay event-stream ingestion/dispatch for non-NIP-46 subscription payloads remains
    follow-on work (current closure is message-type acknowledgment and receipt semantics in the
    relay transport contract).

**Not landed — confirmed by grep across codebase:**

- No `bech32` encode/decode, no `npub`/`nsec`/`nprofile`/`nevent`/`naddr` codec (NIP-19)
- No `nostr:` URI scheme handler (NIP-21)
- No kind `3` contact list publish/fetch (NIP-02)
- No kind `10002` relay list metadata (NIP-65)
- No kind `7` reactions (NIP-25)
- No kind `30000`-range lists (NIP-51)
- No relay info document fetch (NIP-11)
- No NIP-05 identifier resolution
- No NIP-17 sealed DMs
- `npub1example` strings in tests are literal placeholder strings, not real bech32

---

## 2. NIP Priority Classification

### Tier 1 — Blocking (client is broken without these)

**NIP-19: `bech32`-encoded identifiers**

Every user-facing identity representation in Nostr uses bech32. Without it:
- `authors` filter fields cannot accept user input (they must be raw hex pubkeys)
- Public keys cannot be displayed in any standard Nostr format
- Events cannot be referenced by `nevent` links
- The `getPublicKey` NIP-07 response returns raw hex, but web content expects `npub` on display

Entities: `npub1` (public key), `nsec1` (secret — display-only decode for import),
`note1` (event ID), `nprofile1` (pubkey + relay hints), `nevent1` (event + relay hints),
`naddr1` (replaceable event address).

**NIP-21: `nostr:` URI scheme**

For a browser, this is as fundamental as `http://`. Nostr-aware web content embeds
`nostr:npub1...` and `nostr:nevent1...` links. Without a `nostr:` protocol handler,
those links fail silently or open nothing. Graphshell's `ProtocolRegistry` already
handles custom schemes — `nostr:` must be registered there.

### Tier 2 — Required for social graph functionality

**NIP-65: Relay list metadata (kind `10002`)**

A user's canonical list of read/write relays, published as a replaceable event.
Without it, Graphshell can only find events on hardcoded relays. With it, relay
selection becomes portable — you follow someone and automatically know where their
events appear. This is how the Nostr social graph works at scale.

**NIP-02: Follow lists (kind `3`)**

The contact list. A set of pubkeys (and optional relay hints and petnames) the user
follows. Without NIP-02, there is no social graph — you cannot subscribe to a
user's feed or know who they follow. This is required before any social timeline
surface can exist.

**NIP-11: Relay information document**

HTTP `GET` to a relay URL returns JSON: supported NIPs, name, description, pubkey,
limitations. Required for intelligent relay selection — before connecting to a relay
you should know what it supports. Also needed to warn users when a relay doesn't
support a required NIP.

### Tier 3 — Required for content interaction

**NIP-25: Reactions (kind `7`)**

Likes, `+`/`-`, emoji reactions. Users expect to react to notes. Without NIP-25
the client is fully read-only from a social perspective.

**NIP-05: Nostr address (internet identifier)**

Maps `user@domain` to a pubkey via a `.well-known/nostr.json` HTTP endpoint.
For a browser, DNS-based identity is a natural fit. Users expect to type a human
address rather than a 64-char hex string.

**NIP-51: Lists (kind `30000`-range)**

Bookmark lists, mute lists, pinned notes, categorized follow sets. Required for
any content organization feature. Mute lists in particular are a basic trust/safety
tool — without them users cannot suppress spam or unwanted content.

### Tier 4 — Required for private messaging

**NIP-44: Versioned encryption** — already landed as a primitive (used in NIP-46).
The encryption function itself exists; it just needs wiring to DM send/receive.

**NIP-17: Private direct messages (sealed, gift-wrapped)**

The modern DM standard using NIP-44 + kind `1059` gift wrap. Preferable over
NIP-04 for new implementation since metadata leakage is much lower. NIP-44 being
already landed means the crypto is done; the message kind handling and thread model
are the remaining work.

---

## 3. Implementation Phases

### Phase N1 — Codec foundation (unblocks everything)

**N1.1 — NIP-19 bech32 codec in `NostrCoreRegistry`**

The `nostr` crate (already a dependency via NIP-44 import) includes NIP-19 types.
Expose them through `NostrCoreRegistry` as a codec surface rather than re-implementing.

```rust
// In nostr_core.rs — new codec module
pub(crate) fn encode_npub(pubkey_hex: &str) -> Result<String, NostrCoreError>
pub(crate) fn encode_note(event_id_hex: &str) -> Result<String, NostrCoreError>
pub(crate) fn encode_nprofile(pubkey_hex: &str, relays: &[String]) -> Result<String, NostrCoreError>
pub(crate) fn encode_nevent(event_id_hex: &str, relays: &[String], kind: Option<u16>, author: Option<&str>) -> Result<String, NostrCoreError>
pub(crate) fn encode_naddr(identifier: &str, pubkey_hex: &str, kind: u16, relays: &[String]) -> Result<String, NostrCoreError>
pub(crate) fn decode_bech32(bech32_str: &str) -> Result<NostrEntityRef, NostrCoreError>

pub(crate) enum NostrEntityRef {
    PublicKey(String),       // hex pubkey
    Note(String),            // hex event id
    Profile { pubkey: String, relays: Vec<String> },
    Event { event_id: String, relays: Vec<String>, kind: Option<u16>, author: Option<String> },
    Addr { identifier: String, pubkey: String, kind: u16, relays: Vec<String> },
}
```

Also: `NostrFilterSet::authors` currently takes raw hex strings from test code. The
filter construction path should accept bech32 and decode internally before sending
`REQ` messages (NIP-01 requires hex on the wire).

**Done gates:**
- [ ] `encode_npub` / `encode_note` / `encode_nprofile` / `encode_nevent` / `encode_naddr` implemented
- [ ] `decode_bech32` decodes all five entity types to `NostrEntityRef`
- [ ] `NostrFilterSet` builder accepts bech32 authors and decodes to hex
- [ ] NIP-07 `getPublicKey` response available in both hex and npub forms
- [ ] Unit tests: encode/decode round-trip for all five entity types; invalid input returns `Err`

**N1.2 — NIP-21 `nostr:` URI protocol handler**

Register `nostr:` in `ProtocolRegistry`. The handler decodes the entity using N1.1,
then routes to the appropriate graph action:

- `nostr:npub1...` → open profile node or navigate to pubkey
- `nostr:nevent1...` → open event node or navigate to event
- `nostr:nprofile1...` → open profile with relay hints
- `nostr:naddr1...` → open addressable event
- `nostr:note1...` → open note event

The routing target is a `GraphIntent` proposal (per the mod system architecture:
Nostr-originated changes are proposals, not direct mutations). Unknown or
unsupported entity types return an explicit unsupported state, not a silent failure.

**Done gates:**
- [ ] `nostr:` registered in `ProtocolRegistry` as a handled scheme
- [ ] Handler decodes bech32 entity and emits appropriate graph intent proposal
- [ ] Unsupported entity types return explicit error node, not blank pane
- [ ] Integration test: `nostr:npub1...` link in webview content routes through the handler

---

### Phase N2 — Relay and identity metadata

**N2.1 — NIP-11 relay information fetch**

HTTP `GET` to a relay's base URL with `Accept: application/nostr+json` returns relay
info. Implement as a one-shot async fetch in `NostrCoreRegistry`:

```rust
pub(crate) struct NostrRelayInfo {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) pubkey: Option<String>,
    pub(crate) supported_nips: Vec<u16>,
    pub(crate) software: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) limitation: Option<NostrRelayLimitation>,
}

pub(crate) struct NostrRelayLimitation {
    pub(crate) max_message_length: Option<u32>,
    pub(crate) max_subscriptions: Option<u32>,
    pub(crate) auth_required: Option<bool>,
    pub(crate) payment_required: Option<bool>,
}

// In NostrCoreRegistry:
pub(crate) async fn fetch_relay_info(relay_url: &str) -> Result<NostrRelayInfo, NostrCoreError>
```

Cache relay info per URL session-locally (no persistent cache needed initially).
Surface in Settings → Sync relay list view so users can see what each relay supports.

**Done gates:**
- [ ] `fetch_relay_info()` fetches and parses NIP-11 JSON
- [ ] `supported_nips` field is populated and queryable
- [ ] `NostrRelayPolicy` can gate connection to relays that don't support required NIPs
- [ ] Relay list in Settings → Sync displays relay name and NIP-11 status

**N2.2 — NIP-05 identifier resolution**

Fetch `https://<domain>/.well-known/nostr.json?name=<local>`, parse the `names`
object, return the hex pubkey. Used at two points:
1. When a user types `user@domain` in any identity input field
2. When displaying a profile that has a `nip05` metadata tag — verify and badge it

```rust
pub(crate) async fn resolve_nip05(identifier: &str) -> Result<Nip05Resolution, NostrCoreError>

pub(crate) struct Nip05Resolution {
    pub(crate) pubkey_hex: String,
    pub(crate) relays: Vec<String>,   // from the optional `relays` field
    pub(crate) verified: bool,
}
```

**Done gates:**
- [ ] `resolve_nip05()` fetches and parses well-known JSON
- [ ] Resolution result includes relay hints from the `relays` field if present
- [ ] Cache resolution TTL (session-local, 10 min default)
- [ ] Invalid/non-resolving identifiers return `Err` with an explicit reason

**N2.3 — NIP-65 relay list metadata (kind `10002`)**

Publish and fetch a user's canonical read/write relay list.

```rust
pub(crate) struct NostrRelayListEntry {
    pub(crate) url: String,
    pub(crate) marker: Option<RelayMarker>,  // Read | Write | None (both)
}

pub(crate) enum RelayMarker { Read, Write }

// In NostrCoreRegistry:
pub(crate) fn build_relay_list_event(entries: &[NostrRelayListEntry]) -> NostrUnsignedEvent
pub(crate) fn parse_relay_list_event(event: &NostrSignedEvent) -> Vec<NostrRelayListEntry>
```

On startup, if a user identity is configured, subscribe to kind `10002` for that
pubkey to discover their relay list. When the user updates their relay list in
Settings → Sync, publish a new kind `10002` event.

`NostrRelayPolicy::default_relays` should be seeded from the NIP-65 list if one
is found, falling back to the hardcoded default (`wss://relay.damus.io`) only when
no kind `10002` event exists.

**Done gates:**
- [ ] `build_relay_list_event()` produces a valid kind `10002` event with `r` tags
- [ ] `parse_relay_list_event()` extracts read/write markers from `r` tags
- [ ] On identity load, subscribe to kind `10002` for own pubkey
- [ ] Settings → Sync relay list can publish a new kind `10002`
- [ ] `NostrRelayPolicy` prefers NIP-65 relays over hardcoded defaults

---

### Phase N3 — Social graph

**N3.1 — NIP-02 follow list (kind `3`)**

Publish and fetch the user's contact list. Kind `3` events contain `p` tags:
`["p", "<hex-pubkey>", "<relay-hint>", "<petname>"]`.

```rust
pub(crate) struct NostrContact {
    pub(crate) pubkey_hex: String,
    pub(crate) relay_hint: Option<String>,
    pub(crate) petname: Option<String>,
}

// In NostrCoreRegistry:
pub(crate) fn build_contact_list_event(contacts: &[NostrContact]) -> NostrUnsignedEvent
pub(crate) fn parse_contact_list_event(event: &NostrSignedEvent) -> Vec<NostrContact>
```

**Important**: kind `3` is a replaceable event — publishing a new one replaces the
old one. The entire contact list must be included every time (no partial updates).
This is a common footgun; the API must enforce it.

Graph integration: each contact in the follow list maps naturally to a graph node.
A "Fetch follows" intent can populate the graph with the user's social graph as nodes
connected by traversal-derived edges. This is the primary Nostr → graph integration
surface.

**Done gates:**
- [ ] `build_contact_list_event()` produces a valid kind `3` event
- [ ] `parse_contact_list_event()` extracts pubkey, relay hint, and petname
- [ ] Contact list is fetched on identity load (subscribe kind `3` for own pubkey)
- [ ] Graph intent proposal: `NostrFetchFollows` creates graph nodes from contact list
- [ ] Publishing always replaces: API enforces full-list-on-publish invariant

**N3.2 — NIP-51 lists (kind `30000`-range)**

Parameterized replaceable events for user-curated lists. Most immediately useful:

- Kind `10000` — mute list (pubkeys, event IDs, hashtags to suppress)
- Kind `10001` — pin list (pinned note IDs)
- Kind `30000` — categorized follow sets (named groups of pubkeys)
- Kind `30001` — bookmark sets

```rust
pub(crate) enum NostrListKind {
    MuteList,            // kind 10000
    PinList,             // kind 10001
    FollowSet(String),   // kind 30000 with identifier
    BookmarkSet(String), // kind 30001 with identifier
}

pub(crate) struct NostrListEntry {
    pub(crate) tag_type: String,  // "p", "e", "t", "a"
    pub(crate) value: String,
    pub(crate) relay_hint: Option<String>,
}

pub(crate) fn build_list_event(kind: NostrListKind, entries: &[NostrListEntry]) -> NostrUnsignedEvent
pub(crate) fn parse_list_event(event: &NostrSignedEvent) -> (NostrListKind, Vec<NostrListEntry>)
```

Start with mute list (kind `10000`) as the first concrete implementation — it has
immediate usability value and blocks spam/safety concerns. Muted pubkeys/events
should be filtered out in `NostrFilterSet` construction and in event rendering.

**Done gates (mute list minimum):**
- [ ] `build_list_event(NostrListKind::MuteList, ...)` produces valid kind `10000`
- [ ] `parse_list_event()` extracts muted pubkeys, event IDs, and hashtags
- [ ] Muted pubkeys are excluded from display and subscriptions where applicable
- [ ] Mute list is fetched on identity load

---

### Phase N4 — Content interaction

**N4.1 — NIP-25 reactions (kind `7`)**

Reactions are kind `7` events with an `e` tag (event being reacted to), a `p` tag
(author of that event), and content `+`, `-`, or an emoji.

```rust
pub(crate) fn build_reaction_event(
    target_event_id: &str,
    target_author_pubkey: &str,
    reaction: &str,  // "+", "-", or emoji
) -> NostrUnsignedEvent
```

Reaction counts on an event are fetched by subscribing to kind `7` with `#e` filter
pointing at the event ID. Display: aggregate `+` count, `-` count, and up to N
unique emoji reactions.

**Done gates:**
- [ ] `build_reaction_event()` produces a valid kind `7` event
- [ ] Reaction subscription filter uses `#e` tag filter correctly
- [ ] Reaction aggregation helper: `count_reactions(events: &[NostrSignedEvent]) -> ReactionSummary`

**N4.2 — NIP-17 private direct messages**

Gift-wrapped sealed DMs using NIP-44 encryption. The crypto is already landed
(NIP-44). The remaining work is the event kind structure:

- Kind `14` — sealed DM (encrypted, signed by sender)
- Kind `1059` — gift wrap (outer envelope, signed by ephemeral key, sent to recipient's relay)

```rust
pub(crate) fn build_dm_gift_wrap(
    recipient_pubkey_hex: &str,
    plaintext: &str,
    sender_backend: &NostrSignerBackend,
) -> Result<NostrUnsignedEvent, NostrCoreError>

pub(crate) fn unwrap_dm_gift_wrap(
    event: &NostrSignedEvent,
    recipient_backend: &NostrSignerBackend,
) -> Result<String, NostrCoreError>
```

Gift-wrap events should be published to the recipient's NIP-65 write relays (N2.3),
not the sender's default relays. The inbox subscription is kind `1059` events
addressed to the user's pubkey.

**Done gates:**
- [ ] `build_dm_gift_wrap()` produces a valid kind `1059` event with NIP-44 inner encryption
- [ ] `unwrap_dm_gift_wrap()` decrypts the inner kind `14` content
- [ ] DM inbox subscription uses kind `1059` with `#p` filter for own pubkey
- [ ] Gift wrap uses ephemeral signing key for the outer envelope (not user's main key)
- [ ] Send path uses recipient's NIP-65 write relays when available

---

## 4. Dependency Order

```
N1.1 (NIP-19 codec)
  └─ N1.2 (NIP-21 URI handler)  ← needs decode
  └─ N2.2 (NIP-05 resolve)      ← needs npub encode for display
  └─ N2.3 (NIP-65 relay list)   ← needs bech32 for filter authors
  └─ N3.1 (NIP-02 follow list)  ← needs bech32 for filter authors
  └─ N3.2 (NIP-51 lists)        ← needs bech32 for muted entities
  └─ N4.1 (NIP-25 reactions)    ← needs bech32 for event refs

N2.3 (NIP-65 relay list)
  └─ N4.2 (NIP-17 DMs)          ← send to recipient's write relays

N3.1 (NIP-02 follow list)
  └─ Graph social graph integration
```

N2.1 (NIP-11) is independent — no bech32 dependency, pure HTTP.

---

## 5. Crate strategy

The `nostr` crate is already a dependency (imported in `nostr_core.rs` for NIP-44
and key types). It includes NIP-19 codec, NIP-05 resolution helpers, and event
builders for all the kinds above. Use it rather than re-implementing:

- `nostr::nips::nip19` — bech32 encode/decode
- `nostr::nips::nip05` — well-known JSON fetch
- `nostr::nips::nip11` — relay info fetch
- `nostr::nips::nip65` — relay list event helpers
- `nostr::event::builder::EventBuilder` — kind-specific event constructors

Thin wrapper functions in `NostrCoreRegistry` keep the public API stable while
delegating to the crate. This avoids leaking `nostr` crate types across the
registry boundary.

---

## 6. Graph integration model (recap)

All Nostr-originated graph mutations are intent proposals per the mod system contract.
The mapping for the NIPs in this plan:

| NIP result | Graph intent proposal |
|---|---|
| NIP-02 follow list fetch | `NostrFetchFollows` → create profile nodes + follow edges |
| NIP-21 `nostr:npub1...` link | `NostrOpenProfile` → navigate to or create profile node |
| NIP-21 `nostr:nevent1...` link | `NostrOpenEvent` → navigate to or create event node |
| NIP-51 mute list | Filter applied before node creation (no intent needed) |
| NIP-25 reaction publish | `NostrPublishReaction` → publish + annotate event node |
| NIP-17 DM send | `NostrSendDm` → publish + create conversation thread node |

`NostrCoreRegistry` emits the proposal; graph reducers decide whether and how to
apply it. The registry never directly mutates graph state.

---

## 7. Done definition (Sector C — Nostr NIP completeness)

A functional Nostr client layer exists when:

- [ ] NIP-19 codec is implemented and all user-facing pubkey/event display uses bech32
- [ ] NIP-21 `nostr:` URIs resolve in Graphshell without silent failure
- [ ] NIP-11 relay info is fetched before connection and surfaced in Settings → Sync
- [ ] NIP-65 relay list is loaded on identity init and used for relay selection
- [ ] NIP-02 follow list can be fetched and published
- [ ] NIP-05 identifiers resolve to pubkeys
- [ ] NIP-25 reactions can be published (send path)
- [ ] NIP-51 mute list is loaded and applied to filter muted content
- [ ] NIP-17 DMs can be sent and received (gift-wrap / sealed inbox)
- [ ] All new event kinds route through `relay_publish` / `relay_subscribe` capability gates
- [ ] `npub1example` placeholder strings in tests are replaced with valid bech32 test fixtures
