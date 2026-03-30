<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Contacts Spec

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: User contacts (flock): data model, suggestion flow, cross-feature usage
**Out of scope**: Nostr follow lists (NIP-02), Verse membership rosters, Matrix room membership

**Related docs**:

- [`../profile/`](../profile/) — User profile / identity model
- [`../comms/`](../comms/) — Comms applet family that consumes contact entries
- [`../../system/2026-03-05_network_architecture.md`](../../system/2026-03-05_network_architecture.md) — iroh / Nostr identity layers
- [`../../../verso_docs/implementation_strategy/coop_session_spec.md`](../../../verso_docs/implementation_strategy/coop_session_spec.md) — co-op sessions as a contact suggestion source
- [`../../../verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md`](../../../verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md) — Cable cabal persistent chat using contact entries
- [`../../../nostr_docs/technical_architecture/nostr_relay_spec.md`](../../../nostr_docs/technical_architecture/nostr_relay_spec.md) — Flock relay mode uses contacts list for NIP-42 allow-list

---

## 1. Motivation

Graphshell needs a lightweight, deliberate contacts model for three purposes:

1. **Co-op session policy** — the host's approved-guest list can be seeded from contacts, replacing manual invite-per-session flows.
2. **Nostr relay Flock mode** — the embedded relay's NIP-42 allow-list is derived from contacts tagged `Friend` or `Acquaintance`.
3. **Cable persistent cabals** — cabal membership invites can be issued directly from the contacts list.

The model is intentionally minimal: contacts are *opt-in* records the user explicitly saves. Encounters (co-op sessions, Cable chats) generate *suggestions*, not automatic roster additions. This prevents passive accumulation of a permanent social graph the user never consciously curated.

---

## 2. Data Model

```rust
pub struct ContactEntry {
    /// Cross-device identity anchor. Nostr npub if available; falls back to
    /// a stable device-local UUID until the user configures a Nostr key.
    user_id: UserIdentity,
    /// User-editable display name, seeded from the last-seen session display name.
    display_name: String,
    /// User-assigned relationship tags. Multiple tags are allowed.
    tags: Vec<ContactTag>,
    /// iroh NodeIds (device peer IDs) seen for this user across sessions.
    /// Allows re-connecting to a known peer without repeating the invite flow.
    known_device_peers: Vec<NodeId>,
    /// Timestamp of first co-op/cabal encounter that generated this entry.
    first_seen: SystemTime,
    /// Timestamp of most recent encounter.
    last_seen: SystemTime,
}

pub enum ContactTag {
    /// Full trust: approved for Flock relay mode, cabal invites, session auto-approve.
    Friend,
    /// Recognized but not auto-trusted: relay access not granted by default.
    Acquaintance,
    /// Blocked: reject all session join requests and relay connections from this peer.
    Blocked,
    /// User-defined label (e.g. "Work", "Hackathon 2026").
    Custom(String),
}
```

### 2.1 UserIdentity

`UserIdentity` is the cross-device anchor:

```rust
pub enum UserIdentity {
    /// Nostr public key — preferred when available. Stable across devices.
    Nostr(NostrPubkey),
    /// Stable local UUID generated at first encounter, before Nostr keys are in play.
    /// Upgraded to `Nostr` variant when the peer proves ownership of an npub.
    Local(Uuid),
}
```

When a peer presents a `PresenceIdentity` (from co-op) containing both a `NostrPubkey` and a `device_peer_id`, the system:

1. Looks up the contacts store by `NostrPubkey`.
2. If found, appends `device_peer_id` to `known_device_peers` if not already present.
3. If not found, creates a pending suggestion (see §3).

---

## 3. Suggestion Flow

No encounter automatically creates a `ContactEntry`. Instead, encounters emit *contact suggestions* that the user can accept or dismiss.

```
[co-op session ends]  ──►  suggest_contact(peer_identity)  ──►  ContactSuggestion queue
[Cable cabal message received from new peer]  ──►  same queue

ContactSuggestion {
    user_id: UserIdentity,
    display_name: String,
    source: SuggestionSource,
    suggested_at: SystemTime,
}

enum SuggestionSource {
    CoopSession(CoopSessionId),
    CableCalab(CabalId),
}
```

Suggestions appear in a dedicated "People you've met" section in the Contacts UI (§5). The user can:

- **Save as Friend** — creates a `ContactEntry` with `ContactTag::Friend`.
- **Save as Acquaintance** — creates a `ContactEntry` with `ContactTag::Acquaintance`.
- **Dismiss** — removes the suggestion permanently.
- **Block** — creates a `ContactEntry` with `ContactTag::Blocked`.

Suggestions expire after 30 days if not acted on.

---

## 4. Storage

Contacts are stored in the local fjall store under the `contacts` partition, separate from the graph store.

| Table | Key | Value | Purpose |
|-------|-----|-------|---------|
| `contacts` | `UserIdentity` (encoded) | `ContactEntry` (rkyv) | Primary store |
| `contacts_by_tag` | `(ContactTag, UserIdentity)` | `()` | Tag-based enumeration |
| `contact_suggestions` | `(suggested_at, UserIdentity)` | `ContactSuggestion` (rkyv) | Pending suggestions |

Storage is local-only. Contacts are not replicated to Device Sync by default (the user may choose to sync them as a portable archive class in a future release).

---

## 5. UI Surface

### 5.1 Contacts panel

A dedicated panel (accessible from the workbench chrome or profile menu) showing:

- **Contacts** tab — lists all saved `ContactEntry` records, filterable by tag.
- **Suggestions** tab — lists pending `ContactSuggestion` records with accept/dismiss actions.

### 5.2 In-session contact badge

During a co-op session, participants already in the contacts list display a small badge (e.g. "★ Friend"). Unknown participants display a "Save contact?" prompt after session end (which populates the suggestion queue rather than auto-saving).

### 5.3 Relay allow-list derivation

When the embedded Nostr relay is in Flock mode, it queries `contacts_by_tag` for all entries tagged `Friend` or `Acquaintance` and constructs the NIP-42 allow-list from their `UserIdentity::Nostr` pubkeys. Entries without a Nostr pubkey are excluded from relay access (iroh peer ID alone is not sufficient for NIP-42).

---

## 6. Cross-Feature Usage Summary

| Feature | How contacts are used |
|---------|----------------------|
| **Co-op session** | Approved-guest list seeded from `Friend` contacts; auto-approve toggle per session |
| **Nostr relay (Flock mode)** | NIP-42 allow-list derived from `Friend`+`Acquaintance` Nostr pubkeys |
| **Cable cabal** | Cabal invite list populated from contacts; blocked contacts filtered from cabal joins |
| **Comms applet** | Contact picker for direct messages and group formation |

---

## 7. Relationship to Nostr Social Graph

Graphshell contacts are **not** a replacement for or mirror of the Nostr social graph (NIP-02 follow lists). They are a local, device-scoped, application-level roster. The relationship:

- A user's Nostr follows are fetched from public relays and used for content discovery (not stored as contacts).
- A user's contacts are saved locally and used for access control and session management.
- If a contact has a Nostr pubkey, their public profile (NIP-01 kind-0 metadata) may be fetched to populate `display_name` automatically.

This separation keeps the contacts store lightweight and avoids creating a duplicate social graph.

---

## 8. Rollout

**C1 — Foundation**: `ContactEntry`, `ContactTag`, `UserIdentity` types. fjall schema. Suggestion queue. Contacts panel (read-only, no suggestion UI yet).

**C2 — Suggestion flow**: co-op session end generates suggestions. Suggestions panel with accept/dismiss. Block action.

**C3 — Relay integration**: Flock relay mode reads contacts for NIP-42 allow-list (depends on nostr_relay_spec R2).

**C4 — Cable integration**: cabal invite list pulls from contacts. Blocked contacts filtered from cabal joins (depends on cable_coop_minichat_spec Phase 5).
