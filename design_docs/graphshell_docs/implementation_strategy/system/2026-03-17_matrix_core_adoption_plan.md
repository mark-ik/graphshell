<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# MatrixCore Adoption Plan

**Date**: 2026-03-17
**Status**: Active / planning
**Scope**: Execution plan for introducing `MatrixCore` as Graphshell's durable
shared-space substrate without collapsing existing `iroh`, Verse, or Nostr boundaries.
**Parent**: [2026-03-17_matrix_layer_positioning.md](2026-03-17_matrix_layer_positioning.md)

**Related docs**:
- [register/matrix_core_registry_spec.md](register/matrix_core_registry_spec.md) - `MatrixCore` capability-provider boundary
- [register/2026-03-17_matrix_core_type_sketch.md](register/2026-03-17_matrix_core_type_sketch.md) - Rust-facing registry, worker, and normalized-event type sketch
- [2026-03-17_multi_identity_binding_rules.md](2026-03-17_multi_identity_binding_rules.md) - three-identity model and binding rules
- [2026-03-17_matrix_event_schema.md](2026-03-17_matrix_event_schema.md) - Graphshell-owned Matrix room event families and routing rules
- [2026-03-05_network_architecture.md](2026-03-05_network_architecture.md) - current network layer assignments
- [register/2026-03-08_sector_c_identity_verse_plan.md](register/2026-03-08_sector_c_identity_verse_plan.md) - current identity/user-signing lane and binding seam
- [register/SYSTEM_REGISTER.md](register/SYSTEM_REGISTER.md) - Register routing, control-panel, and capability boundary rules

---

## 1. Goal

Add a Matrix-backed collaboration lane for:

- durable shared graph rooms
- room-backed message history
- membership and moderation semantics
- private/team collaboration spaces
- optional bridge affordances to Nostr publication surfaces

while preserving the existing role split:

- `iroh` = trusted peer/session transport
- Verse / `libp2p` = subject/community transport and replication
- `Nostr` = public/social identity and relay-native publication
- `Matrix` = durable room/state substrate

---

## 2. Non-Goals

This plan does **not** authorize:

- replacing `iroh` for Device Sync or Coop transport
- replacing Verse as the canonical subject/community substrate
- replacing Nostr as the portable public/social identity layer
- exposing raw homeserver connections, tokens, or crypto material to mods
- treating Matrix room history as reducer-owned graph truth

---

## 3. Adoption Shape

The lane should land in four layers:

1. **Register layer**: `MatrixCore` native provider boundary, capability IDs, diagnostics,
   and worker ownership
2. **Identity layer**: explicit Matrix ID binding into the existing `NodeId` / `UserIdentity`
   model without key reuse
3. **Projection layer**: normalize room events into Graphshell-owned typed projections
4. **Surface layer**: room/workspace UI, membership views, and graph-space affordances

The reducer remains the sole owner of graph truth. Matrix is an external collaboration
substrate that can propose state changes through bounded carriers.

---

## 4. Event Mapping Rule

Matrix room events are divided into three buckets:

### 4.1 Observe-only

These update Graphshell-owned room/session state but never propose graph mutation:

- room timeline messages
- membership changes
- typing/read-receipt/presence-like metadata
- moderation and role metadata

### 4.2 Projectable

These may create or refresh Graphshell projections, but only through explicit host-owned
projection code:

- room/topic metadata
- room avatar/name metadata
- pinned/reference events that map to graph-space descriptors
- explicit Graphshell room-state events under a Graphshell namespace

### 4.3 Intent-capable

Only explicitly allowlisted Graphshell-owned event types may propose graph/workbench intents.

Initial allowlist:

- `graphshell.room.link_node`
- `graphshell.room.open_workspace_ref`
- `graphshell.room.attach_reference`
- `graphshell.room.publish_selection`

Everything else is observe-only until separately specified.

Guardrail:

- No generic "arbitrary Matrix event -> GraphIntent" adapter is allowed.

---

## 5. Implementation Phases

### Phase M1 - `MatrixCore` skeleton and runtime ownership

Create the provider boundary and supervised worker shape without yet committing to
full room UX.

Deliver:

- `MatrixCoreRegistry` struct/API scaffold
- `RegistryRuntime` wiring
- `ControlPanel` worker supervision entry point for Matrix sync/session workers
- diagnostics channel registration
- settings-visible session status placeholder

Done gates:

- [ ] `MatrixCoreRegistry` exists as a real runtime-owned type
- [ ] capability IDs from [matrix_core_registry_spec.md](register/matrix_core_registry_spec.md) are wired into manifest/spec surfaces
- [ ] `ControlPanel` can supervise a no-op or stub Matrix session worker
- [ ] diagnostics channels emit for startup, shutdown, and capability denial

### Phase M2 - session lifecycle and secret boundary

Land authenticated homeserver session handling while preserving the host-owned secret boundary.

Deliver:

- session open/close flow
- device/session persistence policy
- token/key storage strategy
- sign-in/sign-out state transitions
- failure and degraded-mode diagnostics

Done gates:

- [ ] no raw Matrix session tokens or crypto material are exposed to mods or page-local code
- [ ] session lifecycle is host-owned and survives restart according to documented policy
- [ ] login failure, session expiry, and logout paths emit explicit diagnostics
- [ ] settings surface can show current homeserver/account/session state

### Phase M3 - room sync and membership projection

Add durable room synchronization and a Graphshell-owned projection model for room state.

Deliver:

- room subscription/sync worker
- room membership snapshot model
- room metadata projection
- timeline projection storage boundary

Done gates:

- [ ] `room_subscribe` / `room_unsubscribe` / `room_membership` paths exist behind capability gates
- [ ] room sync is supervised by `ControlPanel`, not ad hoc background tasks
- [ ] membership state is queryable without leaking Matrix SDK internals across the app
- [ ] room metadata and membership projections survive restart as designed

### Phase M4 - graph-safe event projection and allowlisted intents

Define the event normalization contract and the first bounded set of room events that may
propose graph/workbench changes.

Deliver:

- normalized Matrix event enum or equivalent projection layer
- Graphshell namespaced event types for graph-space interaction
- reducer/workbench bridge mapping for the initial allowlist
- rejection diagnostics for unsupported or malformed event types

Done gates:

- [ ] observe-only vs projectable vs intent-capable event classes are encoded in one authority doc/code seam
- [ ] only allowlisted Graphshell-owned event types may produce intent proposals
- [ ] rejected room events emit diagnostics rather than silently no-op
- [ ] no direct graph mutation occurs from Matrix callbacks/workers

### Phase M5 - multi-identity binding

Extend the existing identity lane so Matrix IDs can be linked to `NodeId` and `npub`
without collapsing the model.

Deliver:

- binding record authority/store
- Matrix ID link flow
- verification-state presentation
- revocation/unlink path

Done gates:

- [ ] Matrix ID bindings follow [2026-03-17_multi_identity_binding_rules.md](2026-03-17_multi_identity_binding_rules.md)
- [ ] strong and weak verification states are distinguished in UI/runtime policy
- [ ] unlinking a Matrix account does not damage the underlying `NodeId` or `npub`
- [ ] permission/membership state is not silently transferred across ecosystems

### Phase M6 - durable shared-space surfaces

Expose Matrix-backed rooms as first-class Graphshell surfaces.

Deliver:

- room list / join / leave flow
- room detail surface
- membership/moderation panels
- room-to-graph-space affordances

Done gates:

- [ ] users can enter a room-backed surface without needing direct reducer knowledge
- [ ] membership and role semantics are visible in the surface
- [ ] room-scoped graph affordances use bounded actions, not hidden side effects
- [ ] diagnostics/empty/degraded states are represented in UI

### Phase M7 - optional Nostr bridge affordances

Add explicit, opt-in bridge actions between Matrix rooms and Nostr publication lanes.

Deliver:

- publish-to-Nostr actions for selected room content
- linked-identity display in room/member surfaces
- bridge-policy prompts and diagnostics

Done gates:

- [ ] bridge actions are explicit and user-initiated
- [ ] Nostr publication still routes through `NostrCoreRegistry`
- [ ] Matrix moderation state is not treated as Nostr relay policy
- [ ] unverified identity links cannot silently authorize bridge actions

---

## 6. Worker and Routing Model

All async Matrix activity remains outside the reducer:

- session sync workers run under `ControlPanel`
- workers emit normalized events, diagnostics, and bounded intent proposals
- reducer/workbench authorities remain synchronous and deterministic

Preferred routing pattern:

```text
Matrix homeserver/client
  -> MatrixCore worker
  -> normalized room event
  -> (a) projection state update
  -> (b) optional allowlisted intent proposal
  -> reducer/workbench authority
```

Anti-patterns to forbid:

- Matrix callbacks mutating graph state directly
- raw SDK client handles escaping into arbitrary UI code
- generic "room event executes command" paths without allowlisting

---

## 7. Storage Boundaries

The adoption lane must keep these storage scopes distinct:

- **device-local**: Matrix session/device state, local `NodeId`, local secret material
- **workspace-local**: room selections, presentation state, projection caches where appropriate
- **room-derived**: synchronized room metadata and timeline-derived projections
- **public/social**: Nostr publications and linked `npub` metadata

No room-derived data should be mistaken for canonical graph truth unless explicitly imported
through reducer-owned paths.

---

## 8. Risks and Guardrails

### Risk: room history becomes a second graph database

Mitigation:

- keep Matrix room state external
- import/project selectively
- require explicit graph-owned event types for mutation proposals

### Risk: identity confusion between Matrix IDs, `npub`, and `NodeId`

Mitigation:

- keep the three-identity model explicit
- show verification state everywhere identities are linked
- require explicit user linking and unlinking flows

### Risk: Matrix starts to absorb Coop/Verse semantics

Mitigation:

- keep non-goals active in this plan and in `matrix_core_registry_spec.md`
- require architecture review before any Matrix lane claims transport or community authority

---

## 9. Suggested First Merge Sequence

1. `M1` runtime skeleton with diagnostics only
2. `M2` session lifecycle and settings surface
3. `M3` room sync + membership projection
4. `M5` identity binding authority
5. `M4` allowlisted graph-event projection
6. `M6` room surfaces
7. `M7` optional Nostr bridge actions

Reason:

- the session and projection seams need to exist before graph-affecting room events are safe
- identity binding should land before rich room/member UI claims cross-ecosystem coherence
- Nostr bridging should stay last so it builds on stable room and identity semantics

---

## 10. Acceptance Snapshot

This lane is credible when all of the following are true:

- Graphshell can maintain a host-owned Matrix session without leaking secrets
- room membership and metadata are projected into stable Graphshell surfaces
- only allowlisted Graphshell-owned room events can propose graph/workbench actions
- Matrix IDs, `npub`, and `NodeId` remain distinct but linkable through explicit verified bindings
- Matrix augments the stack as the durable room layer without displacing `iroh`, Verse, or Nostr
