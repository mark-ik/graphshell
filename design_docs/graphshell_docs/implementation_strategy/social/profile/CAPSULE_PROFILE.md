<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# CapsuleProfile Mapping Spec

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: Defines `CapsuleProfile` as the publication-oriented mapping layer that turns a Graphshell social profile into concrete Nostr kind 0 metadata, legacy Finger text, WebFinger discovery documents, and Gemini/Gopher profile documents.

**Related docs**:

- [`PROFILE.md`](PROFILE.md) — canonical Graphshell social profile surface and ownership boundaries
- [`2026-03-28_social_profile_type_sketch.md`](2026-03-28_social_profile_type_sketch.md) — Rust-facing social profile and `CapsuleProfile` carrier sketch
- [`serve_profile_on_all_protocols_spec.md`](serve_profile_on_all_protocols_spec.md) — execution contract for multi-lane profile publication
- [`../COMMS_AS_APPLETS.md`](../COMMS_AS_APPLETS.md) — social-domain placement for hosted communication surfaces
- [`../../system/2026-03-05_network_architecture.md`](../../system/2026-03-05_network_architecture.md) — Nostr public-profile lane and protocol role boundaries
- [`../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md`](../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) — Gemini/Gopher/Finger publication surfaces and `SimpleDocument` serializers
- [`../../../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Finger publication boundary and small-protocol server behavior

---

## 1. Purpose

The Graphshell social profile is the user's host-side identity card.

`CapsuleProfile` is the **publication mapping layer** that projects that richer host-side identity into concrete transport- or format-specific artifacts.

The intent is not to make every protocol emit the same serialized profile body. The intent is to preserve the same identity **semantics** while rendering them in each protocol's own idiom.

It exists to solve three problems:

- keep one logical profile surface without forcing every publication lane to share the same schema
- allow selective disclosure per lane
- make legacy Finger, WebFinger, and Gemini/Gopher profile publishing use the same upstream data model as Nostr profile publication

`CapsuleProfile` is therefore not the source-of-truth identity model. It is the **canonical publishable projection** derived from the social profile.

---

## 2. Position in the Model

The relationship is:

```text
SocialProfileCard
    -> disclosure filter
    -> CapsuleProfile
    -> lane-specific renderer
        -> Nostr kind 0 event
        -> Finger text
        -> WebFinger JSON document
        -> Gemini text/gemini document
        -> Gophermap profile document
```

Rules:

- `SocialProfileCard` remains the richer host-side editor model.
- `CapsuleProfile` is the normalized publication model.
- lane-specific renderers must not pull fields directly from the editor model.

This keeps publication behavior deterministic and auditable while still allowing each lane to speak in its own native format.

---

## 3. Canonical CapsuleProfile Shape

`CapsuleProfile` should be small, textual, and publication-safe.

Illustrative shape:

```rust
pub struct CapsuleProfile {
    pub card_id: ProfileCardId,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub public_keys: Vec<PublicIdentityRef>,
    pub endpoints: Vec<PublicEndpointRef>,
    pub published_views: Vec<PublishedGraphRef>,
    pub links: Vec<ExternalLinkRef>,
    pub disclosure_scope: DisclosureScope,
}

pub enum PublicIdentityRef {
    Nostr { npub: String },
    DidKey { did: String },
    Peer { node_id: String },
    Matrix { mxid: String },
}

pub enum PublicEndpointRef {
    NostrRelay { url: String },
    Finger { query_name: String },
    WebFinger {
      subject: String,
      aliases: Vec<String>,
    },
    Gemini { url: String },
    Gopher { selector: String },
}
```

Normative rules:

- raw secrets never appear in `CapsuleProfile`
- non-public fields must be removed before the `CapsuleProfile` is built
- unsupported fields are ignored by a renderer rather than causing it to invent new semantics

---

## 4. Disclosure Pipeline

`CapsuleProfile` must be built only after a disclosure filter is applied.

Recommended sequence:

1. Start from one selected social profile card.
2. Resolve the target publication lane.
3. Apply field/section disclosure policy for that lane.
4. Materialize a `CapsuleProfile` containing only allowed fields.
5. Render the lane-specific output.

This prevents accidental leakage such as:

- publishing Matrix account identifiers into a public Finger profile unintentionally
- publishing relay lists to a lane that was meant to expose only name and bio
- exposing internal device/peer references when the user intended a public-facing card only

---

## 5. Mapping to Nostr Kind 0

Nostr kind 0 is the primary structured public-profile lane.

### 5.1 Output model

`CapsuleProfile -> kind 0 content JSON`

Primary mappings:

- `display_name` -> `name` / `display_name`
- `bio` -> `about`
- `avatar_url` -> `picture`
- selected external handles/links -> optional tags or structured extension fields

Graphshell-specific guidance:

- published graph-view links may be included as Graphshell extension fields or linked addressable events
- relay hints belong to operational publishing configuration first; only include them in public metadata if the user explicitly marks them public
- `Peer { node_id }` should not be published into kind 0 by default

### 5.2 Example

```json
{
  "name": "Mark",
  "display_name": "Mark",
  "about": "Research graph, protocols, and browser experiments.",
  "picture": "https://example.com/avatar.png",
  "graphshell_views": [
    "nostr:naddr1...",
    "https://example.net/views/research"
  ]
}
```

### 5.3 Nostr constraints

- kind 0 is the authority for Nostr public profile publication, not for the entire social profile model
- do not overload kind 0 with non-portable Graphshell internals
- use separate Nostr events for follows, shared graph views, or richer publication where appropriate

---

## 6. Mapping to Finger Text (Legacy)

Finger is the simplest human-readable profile publication lane.

### 6.1 Output model

`CapsuleProfile -> plain text`

Recommended layout:

- title line with display name
- short bio paragraph
- optional public identity handles
- optional public endpoints
- optional published graph-view links

### 6.2 Example

```text
Mark

Research graph, protocols, and browser experiments.

Nostr: npub1...
Finger: mark
Views:
- Research graph: https://example.net/views/research
```

### 6.3 Finger constraints

- keep it compact and legible as plain text
- avoid structured internal metadata that only Graphshell understands
- prioritize human-readable identity summary over exhaustive machine metadata
- treat it as a compatibility/legacy export lane, not the preferred modern public discovery path

---

## 7. Mapping to WebFinger

WebFinger is the preferred modern replacement for public contact/discovery.

### 8.1 Output model

`CapsuleProfile -> WebFinger JSON`

Recommended use:

- publish a structured discovery document over HTTPS
- expose aliases and typed links to the user's other public identity lanes
- use it as an indirection layer rather than trying to stuff the entire profile card into one protocol-specific text blob

### 8.2 Example structure

```json
{
  "subject": "acct:mark@example.net",
  "aliases": [
    "https://example.net/profile",
    "nostr:npub1..."
  ],
  "links": [
    { "rel": "self", "href": "https://example.net/profile" },
    { "rel": "alternate", "type": "application/nostr+json", "href": "nostr:npub1..." },
    { "rel": "alternate", "type": "text/gemini", "href": "gemini://example.net/profile" }
  ]
}
```

### 7.3 WebFinger constraints

- WebFinger is primarily a discovery document, not the full rich-profile presentation surface
- prefer links and aliases over dumping large textual profile bodies
- use HTTPS-hosted, domain-bound identity where available

---

## 8. Mapping to Gemini Profile Documents

Gemini publication should render the `CapsuleProfile` through `SimpleDocument` into `text/gemini`.

### 7.1 Output model

`CapsuleProfile -> SimpleDocument -> text/gemini`

Recommended structure:

- heading: display name
- paragraph: bio
- link block(s): public graph views, public endpoints, profile-related external links
- optional small facts list: selected public identities

### 7.2 Example structure

```text
# Mark

Research graph, protocols, and browser experiments.

=> nostr:npub1... Nostr public identity
=> gemini://example.net/profile Personal Gemini profile
=> https://example.net/views/research Research graph
```

### 8.3 Gemini constraints

- prefer navigability and readability over dense metadata dumping
- links should be first-class when a field is naturally a destination
- Gemini output should remain reversible through the existing `SimpleDocument` mapping where practical

---

## 9. Mapping to Gopher Profile Documents

Gopher publication should also derive from `SimpleDocument`, then serialize via `to_gophermap()`.

### 9.1 Output model

`CapsuleProfile -> SimpleDocument -> Gophermap`

Recommended structure:

- info line for display name
- info line(s) for bio
- selector entries for public graph views and external profile endpoints

### 9.2 Gopher constraints

- preserve the same semantic ordering as Gemini where possible
- treat links/endpoints as selector rows
- avoid trying to encode rich nested metadata into Gopher item types

---

## 10. Renderer Responsibilities

Each lane renderer should be a thin adapter from `CapsuleProfile`.

Renderer responsibilities:

- drop unsupported optional fields cleanly
- preserve explicit user-approved field ordering when relevant
- avoid adding lane-local fields that were not present in the `CapsuleProfile` unless they are transport-required wrappers

Non-responsibilities:

- discovering additional secrets or credentials
- fetching private data from password managers
- expanding unpublished fields from the source card

---

## 11. Relationship to GraphshellProfile

If a profile card is associated with a `GraphshellProfile`, that association may influence which card is active or which editor surface is shown, but it does not automatically become part of the `CapsuleProfile` output.

Normative rule:

- `GraphshellProfile` is configuration context
- `CapsuleProfile` is publication content

The mapping layer must not serialize raw workflow/layout/input preferences into public profile publications unless a future explicit publication feature says otherwise.

---

## 12. Future Extensions

- lane-specific templates for "public card", "research card", and "minimal identity card"
- richer mapping for Matrix room profile summaries and Verse community profile references
- receive/import rules for inbound legacy Finger contact info and WebFinger discovery responses
- signed `CapsuleProfile` envelope for non-Nostr publication lanes
- explicit provider adapters for `ServeProfileOnAllProtocols`

---

## 13. Non-Goals

- This doc does not define the editor-side social profile schema in full.
- This doc does not define Nostr event kinds beyond mapping guidance for kind 0.
- This doc does not define Gemini/Gopher/Finger transport lifecycles.
- This doc does not authorize publication of secrets, passwords, or private keys.
