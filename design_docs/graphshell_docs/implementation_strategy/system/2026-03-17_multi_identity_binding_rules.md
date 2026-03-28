<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Multi-Identity Binding Rules - `NodeId` / `npub` / Matrix ID

**Date**: 2026-03-17
**Status**: Draft / architectural identity note
**Scope**: Defines how Graphshell binds transport identity, public/social identity,
and durable-room identity without collapsing them into one keypair or one global handle.

**Related docs**:

- [`2026-03-17_matrix_layer_positioning.md`](2026-03-17_matrix_layer_positioning.md) - Matrix placement in the network stack
- [`register/matrix_core_registry_spec.md`](register/matrix_core_registry_spec.md) - `MatrixCore` ownership boundary
- [`register/identity_registry_spec.md`](register/identity_registry_spec.md) - transport/device identity authority
- [`register/nostr_core_registry_spec.md`](register/nostr_core_registry_spec.md) - Nostr/user identity authority
- [`register/2026-03-08_sector_c_identity_verse_plan.md`](register/2026-03-08_sector_c_identity_verse_plan.md) - current `UserIdentity` <-> `NodeId` binding seam
- [`../../../verso_docs/implementation_strategy/coop_session_spec.md`](../../../verso_docs/implementation_strategy/coop_session_spec.md) - current `npub` / `NodeId` split for co-op

---

## 1. Decision Summary

Graphshell uses a **three-identity model**:

- `NodeId` = transport/device identity for `iroh` and Verse transport continuity
- `npub` = public/social user identity for Nostr-native publishing, discovery, and messaging
- Matrix ID (`@user:homeserver`) = durable room membership identity for Matrix-backed spaces

These identities must remain distinct. Graphshell may bind them together for UX,
trust, or profile presentation, but must never pretend they are one protocol-native
identifier.

---

## 2. Identity Roles

| Identity | Authority | Primary use |
| --- | --- | --- |
| `NodeId` | `IdentityRegistry` | device trust, transport authentication, live peer presence |
| `npub` | `NostrCoreRegistry` / `UserIdentity` lane | public/social presence, relay publishing, follows, DM identity |
| Matrix ID | `MatrixCore` session lane | room membership, moderation, durable shared-space participation |

Design rule:

- `NodeId` answers "which device/peer is this?"
- `npub` answers "which public/social persona is this?"
- Matrix ID answers "which room participant/account is this?"

---

## 3. Binding Principles

1. **No key reuse rule**: Binding does not imply shared secret material.
2. **Explicit consent rule**: Cross-identity linking requires explicit user action.
3. **Verifiable-link rule**: A claimed link must be backed by signed proof, verified
   session state, or an equivalent trusted authority receipt.
4. **Scope rule**: Some bindings are durable profile links; others are session- or
   room-scoped only.
5. **Revocation rule**: A user can remove or expire a binding without destroying the
   underlying identities.
6. **No-silent-upgrade rule**: Presence in one ecosystem must not silently grant trust
   or moderation rights in another.

---

## 4. Canonical Binding Shapes

### 4.1 `NodeId` <-> `npub`

This is the existing Graphshell seam used for Coop/Verse-style presence binding.

Meaning:

- "This device transport identity is currently speaking on behalf of this public/user identity."

Recommended shape:

- short-lived signed assertion
- explicit audience
- issue/expiry timestamps
- verifier can reject stale or unverifiable assertions

This is the right model for live presence and device-scoped trust.

### 4.2 `npub` <-> Matrix ID

Meaning:

- "This Matrix account and this Nostr identity are claimed to belong to the same user/persona."

Recommended shape:

- user-approved account link record
- signed proof published or stored in at least one verifiable surface
- optionally mirrored on both sides when the user wants portability

This is a **profile/account link**, not a transport assertion.

### 4.3 `NodeId` <-> Matrix ID

Meaning:

- "This local device/transport identity is one of the devices or clients participating
  in Graphshell under this Matrix account/session."

Recommended shape:

- local host session binding record
- optional room/session-scoped proof when exposed to other participants
- not assumed to be globally portable outside Graphshell

This binding is mainly for local trust and presentation, not for public export.

---

## 5. Binding Record Model

A shared conceptual shape is preferred even if backing storage differs by lane:

```rust
pub struct IdentityBindingRecord {
    pub binding_id: String,
    pub left_identity: BoundIdentityRef,
    pub right_identity: BoundIdentityRef,
    pub scope: IdentityBindingScope,
    pub verification: IdentityBindingVerification,
    pub created_at_secs: u64,
    pub expires_at_secs: Option<u64>,
    pub revoked_at_secs: Option<u64>,
}
```

Where:

- `BoundIdentityRef` may reference `NodeId`, `UserIdentity`/`npub`, or Matrix ID
- `scope` is one of `Session`, `Room`, `Workspace`, or `Profile`
- `verification` records how the link was proven

This is a planning contract, not a claim that the exact struct exists today.

---

## 6. Verification Modes

Bindings should distinguish at least these modes:

- `SignedAssertion` - explicit cryptographic proof carried by one identity lane
- `SessionVerified` - verified by a currently authenticated local session
- `UserConfirmed` - user manually accepted the link but no cryptographic proof exists
- `Imported` - migrated from prior state or external metadata and awaiting confirmation

UX rule:

- Graphshell may display all four states differently, but only `SignedAssertion` and
  `SessionVerified` count as strong bindings for trust-sensitive surfaces.

---

## 7. Trust and Permission Rules

Bindings do **not** automatically copy permissions across ecosystems.

Examples:

- Linking an `npub` to a Matrix ID does not automatically make that user a moderator in
  a Matrix room.
- Linking a Matrix ID to a `NodeId` does not automatically mark that peer as trusted for
  device sync.
- Linking an `npub` to a `NodeId` does not automatically subscribe the user to a Nostr
  feed or join a Matrix room.

Bindings are for:

- profile coherence
- participant display
- user-recognition and bridge affordances
- scoped trust hints

They are not a universal ACL transport.

---

## 8. UX/Surface Rules

Graphshell should permit partial identity configuration:

- `NodeId` only: local-first / transport-only usage
- `NodeId` + `npub`: public/social participation without Matrix rooms
- `NodeId` + Matrix ID: durable room participation without Nostr publishing
- all three: full multi-surface participation

Preferred presentation:

- one person/profile card may show multiple linked identities
- each identity line shows verification state
- actions are lane-specific: "pair device", "publish to relays", "join room", and
  "link account" remain separate

---

## 9. Planning Implications

If the Matrix lane proceeds, the next concrete steps should be:

1. extend `UserIdentity`/binding vocabulary to reference Matrix IDs explicitly
2. add a registry-owned binding store or equivalent authority surface
3. define which bindings are persisted workspace-wide vs device-local vs room-local
4. define which surfaces may act on `UserConfirmed` links and which require stronger proof

This preserves the existing Graphshell identity split while making room for Matrix-backed spaces.
