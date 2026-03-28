# Matrix Core Registry Spec

**Doc role:** Canonical capability-provider spec for `matrix_core`.
**Status:** Draft / canonical direction
**Kind:** Native mod provider profile (Register-integrated)
**Related docs:**
- [../2026-03-17_matrix_layer_positioning.md](../2026-03-17_matrix_layer_positioning.md) (Matrix layer placement and non-goals)
- [../2026-03-17_matrix_core_adoption_plan.md](../2026-03-17_matrix_core_adoption_plan.md) (execution phases and done gates)
- [../2026-03-17_matrix_event_schema.md](../2026-03-17_matrix_event_schema.md) (Graphshell-owned Matrix room event families and routing constraints)
- [../2026-03-17_multi_identity_binding_rules.md](../2026-03-17_multi_identity_binding_rules.md) (NodeId / `npub` / Matrix ID binding rules)
- [2026-03-17_matrix_core_type_sketch.md](2026-03-17_matrix_core_type_sketch.md) (Rust-facing registry, worker, and normalized-event type sketch)
- [identity_registry_spec.md](identity_registry_spec.md) (transport/device identity ownership)
- [nostr_core_registry_spec.md](nostr_core_registry_spec.md) (public/social identity and relay ownership)
- [protocol_registry_spec.md](protocol_registry_spec.md) (protocol contract integration)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (Register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `matrix_core`
capability-provider semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Host-session policy**: Homeserver connectivity, sync loops, and crypto/session
   state are host-owned and never delegated as raw networking access to mods.
2. **Room-authority policy**: Matrix room state is an external collaboration substrate,
   not reducer-owned graph truth. Graph changes derived from Matrix must cross intent
   authorities explicitly.
3. **Secret-boundary policy**: Matrix access tokens, device keys, and E2EE material
   never enter mod memory or page-local script memory.
4. **Identity-separation policy**: Matrix IDs remain distinct from `NodeId` and Nostr
   `npub`; any linkage uses explicit verified bindings.
5. **Diagnostics-visibility policy**: Sync degradation, capability denial, membership
   rejection, and bridge violations emit explicit diagnostics.

## Purpose and Scope

`matrix_core` defines the first-party Matrix platform capability layer used for
durable shared graph spaces, room-backed messaging, and membership-governed
collaboration surfaces.

In scope:
- homeserver session lifecycle
- room sync and membership projection
- room send/receive capability boundaries
- E2EE/session-secret ownership boundary
- room-event to graph-intent proposal interfaces
- diagnostics channel declarations for Matrix capability enforcement

Out of scope:
- direct P2P transport (`iroh` remains the transport authority)
- Verse swarm discovery/replication (`libp2p` remains the community transport authority)
- public relay publication (`nostr_core` remains the Nostr authority)
- direct graph mutation authority

## Canonical Capability Contract

### `ModManifest.provides` (from `matrix_core`)

- `identity:matrix-session`
- `matrix:room-subscribe`
- `matrix:room-send`
- `matrix:membership-query`
- `matrix:event-normalize`

### `ModManifest.requires` (for `matrix_core`)

- `identity:provider`
- `diagnostics:channel-write`
- `security:capability-gate`
- `signal:publish`

## Canonical Interfaces

- `session_open(profile_id, homeserver_config) -> session_handle`
- `session_close(session_handle) -> ack`
- `room_subscribe(caller_id, room_id, filter) -> stream_handle`
- `room_unsubscribe(caller_id, stream_handle) -> ack`
- `room_send(caller_id, room_id, event_type, payload) -> send_receipt`
- `room_membership(session_handle, room_id) -> membership_snapshot`
- `resolve_linked_identities(subject) -> linked_identity_snapshot`

`matrix_core` may internally use a Matrix SDK client/session object, but its public
surface stays capability-oriented and caller-scoped.

### Caller identity semantics

`caller_id` is a policy and quota dimension, not presentation state. Recommended format:

- `mod:<feature>` for mod-owned flows
- `mod:<feature>:<scope>` for room- or pane-scoped flows
- `runtime:core` only for compatibility wrappers, not preferred for new integration work

Caller ownership applies to room-subscription handles, send quotas, and bridge-policy
enforcement.

## Initial `ModManifest` Shape

```rust
ModManifest {
    mod_id: "graphshell:matrix-core",
    display_name: "MatrixCore",
    mod_type: ModType::Native,
    provides: vec![
        "identity:matrix-session",
        "matrix:room-subscribe",
        "matrix:room-send",
        "matrix:membership-query",
        "matrix:event-normalize",
    ],
    requires: vec![
        "identity:provider",
        "diagnostics:channel-write",
        "security:capability-gate",
        "signal:publish",
    ],
    capabilities: vec![
        "network:homeserver-managed",
        "crypto:session-secret-owned",
    ],
}
```

`capabilities` remains deny-by-default and enforced by the mod lifecycle/security subsystems.

## Ownership and Routing Notes

- `matrix_core` owns the homeserver client/session lifecycle and room sync workers.
- `IdentityRegistry` remains the owner of `NodeId` / transport identity.
- `NostrCoreRegistry` remains the owner of relay-facing Nostr identity and publication.
- Identity links between Matrix IDs and other identities are resolved through explicit
  binding records, not raw key reuse.
- Room-originated graph proposals cross the current reducer/workbench carrier path
  (`GraphIntent` / `WorkbenchIntent` today; future command/planner entry may wrap it).

Room history is not itself reducer-owned graph truth. Instead:

1. Matrix room events are normalized into Graphshell-owned typed projections.
2. Eligible projections may produce bounded graph/workbench intent proposals.
3. Reducer/workbench authorities accept or reject those proposals explicitly.

## Canonical Use Cases

Good fits for `matrix_core`:

- durable shared graph rooms
- room-backed discussion/history around a workspace or graph collection
- membership-governed collaboration for private teams or organizations
- moderation and role-aware participation surfaces
- bridge/mirror actions that publish selected room events into Nostr when policy allows

Non-goals:

- replacing `iroh` device sync or Coop transport
- replacing Verse community replication
- replacing Nostr public identity or relay publication
- exposing raw homeserver/networking access to mods

## Diagnostics Channel Descriptors

`matrix_core` should declare these channels with explicit severity:

- `mod:matrix:capability_denied` - `Warn`
- `mod:matrix:session_open_failed` - `Warn`
- `mod:matrix:room_subscription_failed` - `Warn`
- `mod:matrix:room_send_failed` - `Warn`
- `mod:matrix:membership_denied` - `Warn`
- `mod:matrix:identity_binding_unverified` - `Warn`
- `mod:matrix:security_violation` - `Error`

Severity rule: denial/degraded/fallback channels use `Warn`; secret-handling or
policy-violation failures use `Error`.

## Planned Extensions

- Matrix room-to-graph templates for graph-space projection
- bridge hooks to selected Nostr publish flows
- device/session trust presentation for multi-device Matrix use
- richer moderation-role mapping into Graphshell surface affordances

## Acceptance Criteria

- Capability IDs are stable and documented in one authority spec.
- `matrix_core` manifest declarations satisfy `namespace:name` conventions and pass
  manifest validation.
- No raw Matrix tokens, device keys, or E2EE secrets are exposed to mods or page-local code.
- Room subscribe/send calls are capability-gated and diagnosable.
- Matrix-originated graph changes flow through explicit intent authorities rather than
  direct mutation.
- Identity bindings between Matrix IDs and `NodeId` / `npub` references are explicit,
  verified, and documented through the shared binding rules.
