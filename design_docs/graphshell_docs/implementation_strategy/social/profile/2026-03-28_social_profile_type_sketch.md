<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Social Profile Type Sketch

**Doc role:** Rust-facing type sketch for social profile cards, `CapsuleProfile`, disclosure policy, and provider references
**Status:** Draft / implementation-oriented planning note
**Kind:** Social-domain type sketch

**Related docs:**

- [PROFILE.md](PROFILE.md) (canonical social profile surface)
- [CAPSULE_PROFILE.md](CAPSULE_PROFILE.md) (publication mapping layer)
- [../../aspect_control/2026-03-02_graphshell_profile_registry_spec.md](../../aspect_control/2026-03-02_graphshell_profile_registry_spec.md) (`GraphshellProfile` configuration boundary)
- [../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md](../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) (small-protocol publication surfaces)

**Interpretation note**:

- this document is a type sketch, not a claim that these exact names must ship unchanged
- opaque IDs, carrier boundaries, and ownership rules matter more than exact field spelling
- raw secrets should stay behind keychain, wallet, signer, or password-manager providers rather than entering these types directly

---

## 1. Purpose

This sketch exists to make the social profile lane implementation-shaped.

It answers:

- what a profile card roughly contains
- how `GraphshellProfile` associations stay separate from public identity content
- what `CapsuleProfile` looks like as a publication-safe projection
- how provider references are represented without leaking secrets

---

## 2. Core Opaque Types

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SocialProfileCardId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapsuleProfileId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GraphshellProfileId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SecretProviderRefId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PublicationTargetId(pub String);
```

Rule:

- wrap public-facing identifiers and association handles early so the rest of Graphshell depends on Graphshell-owned types rather than raw backend strings

---

## 3. Social Profile Card Shape

```rust
pub struct SocialProfileCard {
    pub card_id: SocialProfileCardId,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar: Option<ProfileAvatarRef>,
    pub public_identities: Vec<LinkedIdentityRef>,
    pub publication_targets: Vec<PublicationTargetRef>,
    pub published_views: Vec<PublishedGraphRef>,
    pub external_links: Vec<ExternalLinkRef>,
    pub linked_profiles: Vec<GraphshellProfileAssociation>,
    pub secret_providers: Vec<SecretProviderRef>,
    pub disclosure_rules: Vec<DisclosureRule>,
    pub status: SocialProfileCardStatus,
}

pub enum SocialProfileCardStatus {
    Draft,
    Ready,
    Degraded { reason: String },
}
```

Design rule:

- this is the host-side editor model, not the publication model

---

## 4. Identity and Publication Reference Types

```rust
pub enum LinkedIdentityRef {
    Nostr {
        npub: String,
        relay_hints: Vec<String>,
    },
    Peer {
        node_id: String,
        finger_query_name: Option<String>,
    },
    Matrix {
        mxid: String,
        homeserver: Option<String>,
    },
    DidKey {
        did: String,
    },
}

pub enum PublicationTargetRef {
    NostrKind0 {
        target_id: PublicationTargetId,
        relay_urls: Vec<String>,
    },
    Finger {
        target_id: PublicationTargetId,
        query_name: String,
    },
    Gemini {
        target_id: PublicationTargetId,
        url: String,
    },
    Gopher {
        target_id: PublicationTargetId,
        selector: String,
    },
}
```

Rules:

- publication targets contain routing and endpoint data only
- transport/runtime-specific handles should be resolved later at execution time

---

## 5. GraphshellProfile Association

```rust
pub struct GraphshellProfileAssociation {
    pub profile_id: GraphshellProfileId,
    pub role: GraphshellProfileAssociationRole,
}

pub enum GraphshellProfileAssociationRole {
    PreferredWhenActive,
    SuggestedForPublish,
    SafetyFallback,
}
```

Normative rule:

- `GraphshellProfile` associations are configuration companions only; they do not become publishable fields inside `CapsuleProfile` unless a future explicit feature says otherwise

---

## 6. Secret Provider References

```rust
pub struct SecretProviderRef {
    pub provider_id: SecretProviderRefId,
    pub kind: SecretProviderKind,
    pub account_ref: Option<String>,
    pub item_ref: Option<String>,
    pub policy: SecretProviderPolicy,
}

pub enum SecretProviderKind {
    OsKeychain,
    Bitwarden,
    OnePassword,
    Keepass,
    Nip46Signer,
    WalletSigner,
    Other { label: String },
}

pub struct SecretProviderPolicy {
    pub requires_user_presence: bool,
    pub hardware_backed: bool,
    pub remote_sign_only: bool,
}
```

Hard rule:

- provider refs may identify where a secret lives, but they must not contain raw passwords, exportable private keys, or long-lived tokens

---

## 7. Disclosure Types

```rust
pub enum DisclosureScope {
    LocalOnly,
    TrustedPeers,
    RoomScoped,
    CommunityScoped,
    Public,
}

pub struct DisclosureRule {
    pub field: SocialProfileFieldKey,
    pub max_scope: DisclosureScope,
    pub allowed_targets: Vec<PublicationTargetKind>,
}

pub enum PublicationTargetKind {
    NostrKind0,
    Finger,
    Gemini,
    Gopher,
}

pub enum SocialProfileFieldKey {
    DisplayName,
    Bio,
    Avatar,
    PublicIdentity(String),
    PublicationTarget(String),
    PublishedView(String),
    ExternalLink(String),
}
```

Rule:

- disclosure policy is evaluated before building `CapsuleProfile`

---

## 8. CapsuleProfile Shape

```rust
pub struct CapsuleProfile {
    pub capsule_id: CapsuleProfileId,
    pub source_card_id: SocialProfileCardId,
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
    Gemini { url: String },
    Gopher { selector: String },
}
```

Rule:

- `CapsuleProfile` is publication content only; it should be safe to hand to a lane renderer without access to other local card state

---

## 9. Builder Boundary

```rust
pub struct CapsuleProfileBuilder;

impl CapsuleProfileBuilder {
    pub fn build_for_target(
        card: &SocialProfileCard,
        target: &PublicationTargetRef,
        scope: DisclosureScope,
    ) -> Result<CapsuleProfile, SocialProfileError>;
}
```

Builder responsibilities:

- enforce disclosure policy
- drop unsupported or non-public fields
- normalize output ordering and publication-safe shapes

Non-responsibilities:

- direct network publication
- secret retrieval from providers
- runtime server lifecycle control

---

## 10. Renderer Sketches

```rust
pub trait CapsuleProfileRenderer {
    type Output;

    fn render(&self, profile: &CapsuleProfile) -> Result<Self::Output, SocialProfileError>;
}

pub struct NostrKind0Renderer;
pub struct FingerTextRenderer;
pub struct GeminiProfileRenderer;
pub struct GopherProfileRenderer;
```

Renderer rule:

- renderers are thin adapters from `CapsuleProfile`; they must not reach back into secret providers or editor-only card state

---

## 11. Non-Goals

- this sketch does not define the UI widget tree for the profile editor
- this sketch does not define transport/runtime worker commands in full
- this sketch does not authorize storing secrets directly in social profile carriers
