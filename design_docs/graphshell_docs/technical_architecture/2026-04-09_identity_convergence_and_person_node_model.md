<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Identity Convergence and the Person Node

**Date**: 2026-04-09
**Status**: Architectural baseline aligned to current implementation
**Purpose**: Define the canonical Graphshell person-node model, the current
cross-protocol identity convergence baseline, and the remaining trust/binding
rules needed to take the model beyond local resolution/import.

**Related docs**:

- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
- [`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md)
- [`2026-04-09_graphshell_verse_uri_scheme.md`](2026-04-09_graphshell_verse_uri_scheme.md)
- [`../research/2026-03-30_middlenet_vision_synthesis.md`](../research/2026-03-30_middlenet_vision_synthesis.md)
- [`../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md`](../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md)

---

## 1. Why This Doc Exists

Graphshell already has a substantial amount of identity convergence logic in
code, but until now it has lacked a canonical architectural note describing:

- what a person node is,
- which protocol identities are converged into it,
- how resolution/import/merge/provenance work,
- where endpoint binding stops being a resolver problem and starts being a
  trust-policy problem.

This document closes that gap.

---

## 2. Current Baseline (2026-04-09)

The following capabilities are already implemented in the repository:

- a `PersonIdentityProfile` that can hold human handles, WebFinger resources,
  NIP-05 identifiers, Matrix MXIDs, Nostr identities, Misfin mailboxes,
  Gemini capsule endpoints, Gopher resources, ActivityPub actor URLs, profile
  pages, aliases, and other imported endpoints,
- protocol-specific identity normalization and resolution for WebFinger,
  NIP-05, Matrix, and ActivityPub actors,
- person-node merge/reuse based on converged identity claims,
- protocol capability descriptors for identity resolution, publish, and deliver
  lanes,
- cached resolution provenance with freshness TTLs,
- refresh actions and UI surfacing for resolution freshness/cache state,
- Titan and Misfin person helper flows for publication/message delivery.

The following are **not** yet closed:

- Gemini client-certificate identity as a first-class converged identity lane,
- stronger sender binding semantics for Misfin and related message surfaces,
- explicit verification/conflict UI for identity claims,
- community-synced identity policy and trust exchange,
- richer ActivityPub object ingestion beyond actor resolution.

---

## 3. The Canonical Person Node

Graphshell's person node is the graph-native identity anchor for a human or
human-operated entity across multiple protocol surfaces.

Current baseline:

- canonical person nodes use `verso://person/<id>` style addresses,
- the node is a durable graph object rather than a transient resolver result,
- protocol-specific identities attach to the person node as classifications,
  imported endpoints, and semantic relations,
- mutation/publication helpers may derive person-owned artifact nodes beneath
  the person address space.

The person node is therefore not just a profile card. It is the stable join
point through which Graphshell connects:

- identity inputs,
- trusted or untrusted endpoint claims,
- refresh/provenance history,
- publish/deliver affordances,
- future social/discovery projections.

---

## 4. Identity Classes and Endpoint Classes

The convergence model needs a clean distinction between **identity claims** and
**reachable endpoints**.

Identity claims:

- human handle,
- WebFinger resource,
- NIP-05 identifier,
- Matrix MXID,
- Nostr identifier,
- ActivityPub actor URL when used as identity.

Endpoint classes:

- Gemini capsule endpoint,
- Misfin mailbox,
- publication endpoint,
- profile/document URL,
- imported protocol-specific endpoint links that may support later actions.

Rule:

- not every endpoint is an identity claim,
- not every identity claim is directly actionable as a transport endpoint,
- Graphshell should preserve both categories separately even when they were
  discovered through the same resolver.

---

## 5. Resolution and Merge Model

The current resolution pipeline is:

1. Normalize the user-supplied identity query for the selected protocol.
2. Resolve/import through the protocol-specific adapter.
3. Build a `PersonIdentityProfile`.
4. Reuse or merge into an existing canonical person node when identity claims
   already match.
5. Record provenance:
   - protocol,
   - normalized query,
   - source endpoints,
   - resolved-at timestamp,
   - cache hit/miss state,
   - freshness state.
6. Surface the resulting state in audit history and inspector metadata.

This is already sufficient for local graph truth and repeatable refresh.

What it is **not** yet sufficient for:

- global proof semantics,
- multi-party trust decisions,
- authoritative conflict resolution between inconsistent claims,
- sync/federation policy.

---

## 6. Binding Strength and Conflict Rules

Graphshell should treat bindings with ordered strength rather than as one flat
bucket of imported facts.

Suggested order of strength:

1. **User-authored explicit binding**.
   A user directly attaches or accepts an identity/endoint relationship.
2. **Cryptographic or domain-bound proof**.
   Examples: NIP-05 proof, domain-controlled WebFinger data, future certificate
   or signature-backed proofs.
3. **Protocol-declared imported endpoint**.
   Resolver output declares a reachable endpoint, but the claim is only as
   strong as that protocol's trust model.
4. **Heuristic or inferred relation**.
   Useful for discovery, not strong enough for silent collapse.

Conflict policy:

- conflicting claims should be preserved rather than discarded,
- imported conflicts should not be silently auto-merged into one "truth",
- stronger bindings may supersede weaker ones for default action choice, but
  weaker claims remain legible as graph history unless explicitly rejected,
- refresh should update provenance and freshness without destroying the user's
  ability to inspect prior disagreement.

---

## 7. Freshness, Refresh, and Provenance

Identity convergence is not a one-time import. It is an ongoing graph concern.

Current baseline:

- protocol descriptors carry freshness TTLs where appropriate,
- resolution provenance records freshness/cache state,
- selected person nodes can be refreshed,
- refresh outcomes are surfaced in the UI and audit history.

Policy implications:

- freshness is protocol-specific, not one global timeout,
- refresh is additive audit/provenance work, not a silent replacement,
- audit payloads should stay structured so UI and tests can reason over them
  without brittle string parsing.

---

## 8. Remaining Gaps

The highest-priority remaining identity work is:

1. Gemini client certificate identity modeling.
2. Sender binding and trust UX for Misfin and future message surfaces.
3. Verification/conflict UI on person nodes.
4. Community or synced identity policy beyond local graph truth.
5. Stronger ActivityPub read lanes beyond actor resolution.

This means the near-term job is **not** to invent a new identity substrate.
It is to formalize and extend the one Graphshell already has.