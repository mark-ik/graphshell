<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Social Profile Spec

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: Defines the Graphshell-hosted social profile surface and its composition rules across Nostr, legacy Verso/Finger, Matrix, Verse-facing community identity, and future HTTPS/WebFinger discovery.

**Related docs**:

- [`../COMMS_AS_APPLETS.md`](../COMMS_AS_APPLETS.md) — social-domain positioning for hosted communication surfaces
- [`CAPSULE_PROFILE.md`](CAPSULE_PROFILE.md) — canonical mapping from social profile card to Nostr, legacy Finger, WebFinger, Gemini, and Gopher publication formats
- [`2026-03-28_social_profile_type_sketch.md`](2026-03-28_social_profile_type_sketch.md) — Rust-facing type sketch for social profile cards, disclosure carriers, and provider references
- [`serve_profile_on_all_protocols_spec.md`](serve_profile_on_all_protocols_spec.md) — execution contract for publishing one card across enabled lanes
- [`../../aspect_control/2026-03-02_graphshell_profile_registry_spec.md`](../../aspect_control/2026-03-02_graphshell_profile_registry_spec.md) — persisted `GraphshellProfile` app/workflow configuration; distinct from social identity profile
- [`../../system/2026-03-05_network_architecture.md`](../../system/2026-03-05_network_architecture.md) — protocol layer assignment for Nostr public profile, follows, DMs, and Verse community identity
- [`../../../../nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md`](../../../../nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md) — Nostr profile/follows editor and runtime boundary
- [`../../../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Finger profile publication and Verso bilateral identity boundary
- [`../../../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md`](../../../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md) — Matrix identity cards as room-context identity rather than public profile authority

---

## 1. Purpose

Graphshell needs a canonical place for the user's **social/public identity surface**.

This is not the same thing as:

- `GraphshellProfile`, which stores app/workflow preferences and control-surface configuration
- a Matrix member profile, which is room-context identity
- a Verse community record, which is community-state/governance identity
- a raw Nostr kind 0 event, which is one publication format rather than the whole host-side profile concept

This document defines the host-side **social profile surface**: the profile Graphshell presents, edits, previews, and publishes across supported identity/publication lanes.

---

## 2. Canonical Position

The social profile belongs under `graphshell_docs/implementation_strategy/social/` because it is a **hosted social surface**, not a transport protocol and not a general app-settings profile.

Practical rule:

- The social domain owns profile composition, editing surface, preview, and publication orchestration.
- Nostr owns relay-facing public identity publication.
- Verso owns legacy Finger publication and bilateral identity transport details.
- Matrix owns room-scoped member identity.
- Verse may reference public identities and community roles, but does not become the authority for the user's base profile card.
- WebFinger is the preferred modern replacement for public contact/discovery when Graphshell exposes an HTTPS-hosted identity endpoint.

---

## 3. Core Model

The Graphshell social profile is a **composed public identity card**.

Users may own **multiple profile cards**. Each card is a separately managed social/public identity surface with its own associations, publication choices, and disclosure policy.

Minimum conceptual fields:

- display name
- short bio / description
- avatar or icon reference
- canonical public keys / identifiers (`npub`, optional `did:key`, optional transport references)
- publication endpoints or hints (preferred relays, legacy Finger query name, WebFinger account/domain mapping, published graph views)
- optional public links to shared graph views, collections, or published snapshots

Depending on configured mods and credentials, a card may also associate:

- Nostr identity and relay preferences
- Verso peer identity references and legacy Finger publication names
- Matrix account references
- Gemini/Gopher/Finger publication endpoints
- WebFinger account/domain discovery handles
- Verse-facing public community references
- one or more linked `GraphshellProfile` records used as workflow/configuration companions
- a password-manager or secret-provider reference for related services

The host surface may render these fields differently per publication lane, but the user's profile should remain recognizably the same identity artifact across those lanes.

Important boundary: the card carries **references, associations, and publication choices**, not raw secret material.

### 3.1 Mod-aware fill-in rule

Cards are intentionally **partially populated**. If a user has not configured a given mod or identity lane, that section remains absent rather than forcing placeholder data.

---

## 4. Ownership Boundaries

### 4.1 Graphshell social-domain ownership

Graphshell owns:

- the profile editor surface
- local draft state and preview behavior
- field-level validation before publish
- mapping one logical profile into multiple publication targets
- user-visible publication status and diagnostics

### 4.2 Nostr ownership

Nostr owns:

- kind 0 public profile publication
- kind 3 follows publication
- relay-targeted dissemination and replacement semantics

Graphshell must not redefine Nostr event semantics in this profile spec.

### 4.3 Verso ownership

Verso owns:

- bilateral identity substrate details
- Finger profile serving and query routing
- any future capsule-profile publication over Gemini/Gopher/Finger lanes

Graphshell social profile may feed Verso publication lanes, but it does not replace the Verso runtime boundary.

### 4.4 Matrix ownership

Matrix owns room member identity cards and room-bound presentation. Matrix IDs and room display state may be linked from the social profile, but are not defined by it.

### 4.5 Verse ownership

Verse owns community manifests, roles, and community-scoped identity references. The social profile can be referenced by Verse communities, but Verse does not own the base user profile contract.

---

## 5. Required Distinction From GraphshellProfile

`GraphshellProfile` is the persisted app/workflow configuration object.

The social profile is a **public-identity document/surface**.

They must remain separate because they answer different questions:

- `GraphshellProfile`: "How should this app behave for me?"
- social profile: "Who am I, and what public identity card do I want to publish or present?"

A social profile card may be **associated with** one or more `GraphshellProfile` records, but only by reference. That lets a card carry a preferred workflow/configuration companion without collapsing public identity data into app-settings state.

If a future implementation stores both in one backing store, the schema boundary must still stay explicit. Social identity fields must not be treated as generic UI/workflow preferences.

---

## 6. Publication Lanes

The same logical social profile may publish to multiple lanes.

### 6.1 Nostr lane

Primary public-network lane.

- canonical public profile event: kind 0
- related follows surface: kind 3
- optional profile-address links to published graph views, snapshots, and collections

### 6.2 Finger lane (legacy)

Human-readable plain-text profile lane.

- published through Verso's Finger server
- acceptable as a legacy compatibility/import lane
- not recommended as the primary public publication path because it is plaintext, unauthenticated, and tied to an old protocol with weak deployment/security posture

### 6.3 WebFinger lane (preferred replacement)

Modern HTTPS-based account and identity discovery lane.

- suitable for exposing contact/discovery metadata over `/.well-known/webfinger`
- better fit than Finger for modern public profile discovery because it runs over HTTPS and returns structured metadata
- especially useful as a discovery indirection layer that points to Nostr, Matrix, Gemini, Gopher, or published graph-view endpoints

### 6.4 Capsule lanes (future)

Gemini/Gopher/Finger publication may converge on a shared `CapsuleProfile` representation, but that representation is a publication format, not the canonical host-side profile concept.

### 6.5 Matrix and Verse references

Matrix rooms and Verse communities may display or reference the social profile, but they should consume it as a linked or projected artifact rather than redefine it locally.

### 6.5 Selective disclosure

Cards should support **partial sharing**. The user may publish only subsets of a card per lane or audience, such as name + avatar only, public graph-view links only, or `npub` without other associated identities.

---

## 7. Editor and UX Rules

The profile editor should:

- present one logical profile with per-lane publication toggles or status
- support multiple cards and make the active card explicit
- distinguish draft changes from published state
- show which fields are publishable publicly versus locally retained only
- show which sections are backed by currently configured mods or credentials
- surface capability failures clearly when a lane is unavailable
- avoid implying that publishing to one lane automatically publishes everywhere unless the user explicitly chooses that action

Non-goal: a full social client. The profile surface is an identity/editor/publisher surface, not a timeline, inbox, or community browser.

---

## 8. Diagnostics and Safety

The social profile surface must expose explicit status for:

- unsigned or unpublished draft state
- relay publish failure
- Finger publication failure
- WebFinger publication or discovery failure
- capability denial or missing provider
- identity mismatch between configured public identity and target publication lane
- disclosure-policy conflict between a field and a requested publication lane
- secret-provider or password-manager resolution failure

Unsafe default to avoid: silently publishing profile updates to every available lane.

---

## 9. Future Follow-Ons

- field-level privacy classes (public, followers-only, room-only, local-only)
- richer identity binding rules across `npub`, `did:key`, Matrix ID, and Verse community roles
- profile-backed discovery affordances for public graph views and community membership
- explicit provider adapter contracts for password managers, remote signers, and wallet-backed secret providers

---

## 10. Non-Goals

- This doc does not define Nostr kind semantics.
- This doc does not define Matrix member-state schema.
- This doc does not define Verse community manifests or governance roles.
- This doc does not redefine `GraphshellProfile`.
