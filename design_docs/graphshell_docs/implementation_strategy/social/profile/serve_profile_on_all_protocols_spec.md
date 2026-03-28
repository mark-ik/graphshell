<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# ServeProfileOnAllProtocols Spec

**Doc role:** Execution contract for publishing one social profile card across Nostr, legacy Finger, WebFinger, Gemini, and Gopher lanes
**Status:** Draft / canonical direction
**Kind:** Intent and execution contract

**Related docs:**

- [PROFILE.md](PROFILE.md) (social profile surface authority)
- [CAPSULE_PROFILE.md](CAPSULE_PROFILE.md) (publication mapping authority)
- [2026-03-28_social_profile_type_sketch.md](2026-03-28_social_profile_type_sketch.md) (Rust-facing social profile carriers)
- [../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md](../../../../verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) (small-protocol server surfaces)
- [../../system/2026-03-05_network_architecture.md](../../system/2026-03-05_network_architecture.md) (Nostr publication and identity boundaries)

---

## 1. Purpose

`ServeProfileOnAllProtocols` is the high-level user action that publishes one selected social profile card to every enabled lane the user explicitly requested, **using each protocol's native idiom rather than forcing one identical payload shape everywhere**.

Legacy note: Finger may remain available as an opt-in compatibility lane, but it should not be treated as the preferred modern public-profile transport.

It is a coordination contract, not a claim that every lane must always be used.

It answers:

- what the high-level action means
- how the action is decomposed into per-lane publication work
- which parts are reducer-owned, workbench-owned, or runtime-owned
- how partial failure is reported safely

---

## 2. User-Level Meaning

When the user invokes `ServeProfileOnAllProtocols`, Graphshell should:

1. identify the active or selected social profile card
2. resolve which publication lanes are enabled for this request
3. build a lane-specific `CapsuleProfile` projection per enabled lane
4. publish or serve that profile through each lane's owning system
5. report per-lane success, skip, or failure status back to the user

This is an orchestration command over multiple backends. It is not a single monolithic transport operation, and it should produce **protocol-native renderings** rather than a uniform cross-protocol blob.

---

## 3. High-Level Intent Sketch

### 3.1 Current bridge carrier (`GraphIntent` naming)

```rust
ServeProfileOnAllProtocols {
    card_id: SocialProfileCardId,
    targets: BTreeSet<PublicationTargetKind>,
    max_scope: DisclosureScope,
}
```

Companion explicit variants:

- `PublishProfileToNostr { card_id, max_scope }`
- `PublishProfileToFinger { card_id, query_name, max_scope }`
- `PublishProfileToWebFinger { card_id, subject, max_scope }`
- `PublishProfileToGemini { card_id, route, max_scope }`
- `PublishProfileToGopher { card_id, route, max_scope }`

Design rule:

- `ServeProfileOnAllProtocols` is the user-facing orchestration action
- the lane-specific variants are the execution fanout surface

---

## 4. Boundary Split

### 4.1 Reducer-owned

Reducer-owned durable state changes:

- selecting which card is being published
- recording requested targets and requested disclosure scope
- updating durable publish-status records
- storing last-known publication receipts, timestamps, or failure summaries

### 4.2 Workbench-owned

Workbench-owned/UI-only actions:

- open publish dialog
- preview per-lane output
- show field disclosure matrix
- show per-lane progress and retry affordances

These must not become durable reducer history unless the user confirms a publish action.

### 4.3 Runtime-owned

Runtime-owned execution:

- Nostr signing and relay publication
- Finger registry mutation / server publication
- WebFinger document publication / HTTPS endpoint refresh
- Gemini registry update / served profile route generation
- Gopher registry update / served profile route generation

Runtime workers consume the lane-specific publish commands after the reducer has accepted the durable request.

---

## 5. Execution Flow

```text
ServeProfileOnAllProtocols
    -> resolve card + target set
    -> for each target:
        -> build CapsuleProfile with disclosure filter
        -> render lane-specific output
        -> dispatch lane-specific runtime publish action
    -> collect receipts
    -> persist status summary
```

Normative rule:

- a failure in one lane must not silently suppress status for the others

---

## 6. Per-Lane Execution Contracts

Normative interpretation:

- Nostr should receive Nostr-shaped profile publication
- Finger should receive a compact human-readable text profile only when the legacy lane is explicitly enabled
- WebFinger should receive a structured HTTPS discovery document
- Gemini should receive a navigable `text/gemini` profile document
- Gopher should receive a Gophermap-appropriate profile document

The goal is semantic consistency across lanes, not byte-for-byte sameness.

### 6.1 Nostr

Execution steps:

1. build `CapsuleProfile` for `NostrKind0`
2. render to kind 0 JSON content
3. sign through local signer or NIP-46 backend
4. publish to selected relays
5. store a lane receipt/status summary

### 6.2 Finger

Execution steps:

1. build `CapsuleProfile` for `Finger`
2. render to plain text
3. issue `PublishFingerProfile`-style runtime registration with the target query name
4. store route/query-name receipt

### 6.3 WebFinger

Execution steps:

1. build `CapsuleProfile` for `WebFinger`
2. render to WebFinger JSON document
3. publish or refresh the HTTPS-hosted discovery document
4. store resulting subject/url receipt

### 6.4 Gemini

Execution steps:

1. build `CapsuleProfile` for `Gemini`
2. render via `SimpleDocument` to `text/gemini`
3. register or refresh the Gemini-served profile route
4. store resulting route/url receipt

### 6.5 Gopher

Execution steps:

1. build `CapsuleProfile` for `Gopher`
2. render via `SimpleDocument` to Gophermap
3. register or refresh the Gopher-served profile route
4. store resulting selector receipt

---

## 7. Failure Model

Per-lane outcomes:

- `Published`
- `SkippedUnavailable`
- `RejectedByDisclosurePolicy`
- `FailedRuntime`
- `FailedSigning`
- `FailedRouting`
- `FailedDiscoveryPublication`

Suggested summary carrier:

```rust
pub struct ProfilePublicationSummary {
    pub card_id: SocialProfileCardId,
    pub lane_results: BTreeMap<PublicationTargetKind, PublicationResult>,
}
```

Design rule:

- partial failure is normal and must be visible; it is not grounds to roll back every successful lane automatically

---

## 8. Security Rules

- `ServeProfileOnAllProtocols` must never bypass disclosure filtering
- lane renderers must not fetch private fields directly from `SocialProfileCard`
- secret providers may assist signing or auth, but secret contents must not be copied into `CapsuleProfile`
- `Peer { node_id }` and relay hints should remain opt-in for public Nostr publication

---

## 9. Guard Tests (Minimum)

- `serve_profile_all_protocols_applies_disclosure_filter_before_render`
- `serve_profile_all_protocols_partial_lane_failure_preserves_other_successes`
- `serve_profile_to_nostr_never_embeds_private_peer_id_by_default`
- `serve_profile_to_finger_renders_plain_text_from_capsule_profile_only`
- `serve_profile_to_webfinger_emits_discovery_document_not_full_profile_dump`
- `serve_profile_to_gemini_and_gopher_share_simple_document_projection_rules`
- `serve_profile_all_protocols_never_reads_raw_secret_from_profile_card`

---

## 10. Non-Goals

- this spec does not define the full social profile editor UX
- this spec does not require that every card publish to every lane
- this spec does not define the final runtime API spelling for each publication worker
