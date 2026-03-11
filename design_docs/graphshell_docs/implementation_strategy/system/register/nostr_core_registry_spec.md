# Nostr Core Registry Spec

**Doc role:** Canonical capability-provider spec for `nostr_core`.
**Status:** Draft / canonical direction
**Kind:** Native mod provider profile (Register-integrated)
**Related docs:**
- [../2026-03-05_nostr_mod_system.md](../2026-03-05_nostr_mod_system.md) (Nostr mod system architecture)
- [mod_registry_spec.md](mod_registry_spec.md) (mod lifecycle and manifest gate)
- [identity_registry_spec.md](identity_registry_spec.md) (signing and trust boundary)
- [protocol_registry_spec.md](protocol_registry_spec.md) (protocol contract integration)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (Register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `nostr_core` capability-provider semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **No-raw-secret policy**: `nostr_core` never exposes raw Nostr private key material to mods or UI callers.
2. **Host-relay policy**: Relay connectivity is host-owned and shared; callers use capability APIs, not direct sockets.
3. **Capability-gate policy**: Every publish/subscribe/sign operation requires declared and granted capability keys.
4. **Intent-boundary policy**: Nostr-originated graph changes are proposals through intent authorities, never direct mutation.
5. **Diagnostics-visibility policy**: Denied, degraded, and violation paths emit explicit diagnostics with severity.

## Purpose and Scope

`nostr_core` defines the first-party Nostr platform capability layer used by Verso, Verse, and eligible mods.

In scope:
- Nostr signing service boundary
- Relay subscribe/publish service boundary
- NIP-07 host bridge boundary
- capability IDs and grant contracts
- diagnostics channel declarations for Nostr capability enforcement

Out of scope:
- full social timeline product UX
- transport substrate ownership (`iroh` and `libp2p` remain Verso/Verse component concerns)
- direct workbench/graph mutation logic

## Canonical Capability Contract

### `ModManifest.provides` (from `nostr_core`)

- `identity:nostr-sign`
- `nostr:relay-subscribe`
- `nostr:relay-publish`
- `nostr:nip07-bridge`
- `nostr:event-normalize`

### `ModManifest.requires` (for `nostr_core`)

- `identity:provider`
- `protocol:websocket-client`
- `diagnostics:channel-write`
- `security:capability-gate`

## Canonical Interfaces

- `sign_event(persona, unsigned_event) -> signed_event`
- `relay_subscribe(caller_id, subscription_id, filter_set) -> stream_handle`
- `relay_unsubscribe(caller_id, stream_handle) -> ack`
- `relay_publish(caller_id, signed_event) -> publish_receipt`
- `relay_publish_to_relays(caller_id, signed_event, relay_urls[]) -> publish_receipt`
- `nip07_request(origin, method, payload) -> response`

`sign_event` is operation-level only. Exporting or reading raw secret material is not a valid interface.

### Caller identity semantics

`caller_id` is a policy and quota dimension, not presentation state. Recommended format:

- `mod:<feature>` for mod-owned flows
- `mod:<feature>:<scope>` for graph-view or pane scoped flows
- `runtime:core` only for compatibility wrappers, not preferred for new integration work

Caller ownership applies to subscription handles (only owner can unsubscribe) and per-caller quota tracking.

### Relay-target publish semantics

`relay_publish_to_relays` is the preferred path for user-initiated publish commands because relay choice is explicit.

Policy behavior:

1. Relay URLs are normalized and de-duplicated.
2. Policy profile (strict/community/open) is enforced per target relay.
3. Denied targets emit publish-failure and security-violation diagnostics.
4. If no explicit targets are provided, provider defaults are used.

## Initial `ModManifest` Shape

```rust
ModManifest {
    mod_id: "graphshell:nostr-core",
    display_name: "NostrCore",
    mod_type: ModType::Native,
    provides: vec![
        "identity:nostr-sign",
        "nostr:relay-subscribe",
        "nostr:relay-publish",
        "nostr:nip07-bridge",
        "nostr:event-normalize",
    ],
    requires: vec![
        "identity:provider",
        "protocol:websocket-client",
        "diagnostics:channel-write",
        "security:capability-gate",
    ],
    capabilities: vec![
        "network:relay-managed",
        "crypto:sign-operation-only",
    ],
}
```

`capabilities` remains deny-by-default and enforced by the mod lifecycle/security subsystems.

## Diagnostics Channel Descriptors

`nostr_core` should declare these channels with explicit severity:

- `mod.nostr.capability_denied` - `Warn`
- `mod.nostr.sign_request_denied` - `Warn`
- `mod.nostr.relay_subscription_failed` - `Warn`
- `mod.nostr.relay_publish_failed` - `Warn`
- `mod.nostr.intent_rejected` - `Warn`
- `mod.nostr.security_violation` - `Error`

Severity rule: denial/degraded/fallback channels use `Warn`; security/failure channels use `Error`.

## Routing and Authority Notes

- Capability invocations originate from native features, WebView bridge callers, or granted mods.
- Register routing may use direct call internally within owned boundaries, but cross-boundary state changes flow through intent authorities.
- Nostr event processing may emit current reducer-carrier or workbench proposals (`GraphIntent` / `WorkbenchIntent` today); ownership remains with existing authorities even if the top-level carrier shape evolves later.

## Planned Extensions

- NIP-46 delegated signer provider abstraction
- relay policy profiles (strict/private/community)
- richer NIP-07/browser-wallet method depth beyond the landed core bridge
- action registry bindings for caller-scoped command palette routes (`action.nostr.*`)

## Implementation Note — 2026-03-10

The host-owned NIP-07 bridge is now live:

- `window.nostr` is injected by the webview host when `nostr:nip07-bridge` capability is active.
- Bridge requests cross the embedder boundary through a reserved prompt RPC namespace and route to
  `NostrCoreRegistry::nip07_request(...)`.
- The landed method surface is `getPublicKey`, `signEvent`, and `getRelays`.
- Sensitive methods are gated by per-origin permission memory persisted through workspace settings.
- Existing denial and security-violation diagnostics are used for blocked or malformed requests.

## Acceptance Criteria

- Capability IDs are stable and documented in one authority spec.
- `nostr_core` manifest declarations satisfy `namespace:name` conventions and pass manifest validation.
- Signing interface exposes operation-level signing only; no raw key path exists.
- Relay subscribe/publish calls are capability-gated and diagnosable.
- Diagnostics descriptors for declared channels include explicit severity values.
- At least one scenario or targeted integration path verifies NIP-07 bridge calls are capability-checked.
