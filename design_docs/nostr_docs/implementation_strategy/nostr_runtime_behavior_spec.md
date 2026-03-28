# Nostr Runtime Behavior Spec

**Date:** 2026-03-12  
**Status:** Canonical runtime ownership contract  
**Priority:** Tier 3 / protocol-runtime clarity

**Related docs:**

- [`nostr_core_registry_spec.md`](./nostr_core_registry_spec.md)
- [`identity_registry_spec.md`](./identity_registry_spec.md)
- [`protocol_registry_spec.md`](./protocol_registry_spec.md)
- [`../../system/2026-03-10_nostr_nip_completion_plan.md`](../../system/2026-03-10_nostr_nip_completion_plan.md)
- [`../../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../../technical_architecture/2026-03-12_specification_coverage_register.md)

**External standards anchors:**

- Nostr NIPs

---

## 1. Purpose and Scope

This spec complements `nostr_core_registry_spec.md` by defining the runtime behavior and ownership contract of the `nostr_core` registry implementation.

It governs:

- what runtime state the registry owns,
- relay policy and quota behavior,
- signer-backend behavior,
- subscription and publish semantics,
- worker integration,
- permission and diagnostics behavior,
- the boundary between NIP-defined protocol truth and Graphshell-owned runtime policy.

It does not govern:

- end-user social/timeline UX,
- graph mutation semantics for any future Nostr-derived graph projection,
- non-Nostr identity behavior.

---

## 2. External vs Internal Authority

### 2.1 External authority

Nostr protocol behavior is externally anchored to NIPs.

Examples:

- event structure,
- subscription/event frame semantics,
- signing correctness,
- NIP-07 browser bridge semantics,
- NIP-46 delegated signer interactions.

### 2.2 Graphshell-owned authority

Graphshell is the source of truth for:

- relay policy profiles,
- allowlist/blocklist/default relay resolution,
- caller quotas,
- host-worker topology,
- permission persistence and gating behavior,
- diagnostics channel emission,
- any future graph/workbench projection of Nostr runtime events.

Normative rule:

- the registry must not redefine Nostr protocol semantics,
- but it does own Graphshell runtime policy layered over that protocol.

---

## 3. Canonical Role

`NostrCoreRegistry` is the host-owned runtime capability provider for Nostr signing, relay subscription/publish, and browser bridge services.

It owns:

- capability checks,
- signer backend selection,
- relay policy enforcement,
- subscription and publish quota enforcement,
- worker orchestration,
- diagnostics for accepted/denied/degraded paths.

It does not own:

- raw private-key exposure,
- arbitrary direct relay sockets for callers,
- direct graph mutation.

---

## 4. Current State Ownership

Current runtime state includes:

- in-process relay subscription state,
- optional supervised relay worker channel,
- relay policy,
- caller subscription counts,
- caller publish counts,
- signer backend configuration,
- persisted NIP-07/NIP-46 permission settings.

Normative rule:

- this is registry-owned runtime policy state,
- not graph-domain truth,
- not workbench/session state.

---

## 5. Signer Backend Contract

Current signer backends:

- `LocalHostKey`
- `Nip46Delegated`

### 5.1 LocalHostKey

Behavior:

- uses host identity infrastructure to obtain public key and sign digests,
- produces canonical Nostr signed events,
- never exposes raw secret key material.

### 5.2 Nip46Delegated

Behavior:

- requires relay configuration and signer pubkey,
- may maintain ephemeral session/shared-secret state,
- requires local permission allowance,
- must validate returned signed event integrity.

Normative rule:

- backend selection affects signing path but not the external `sign_event` semantic contract,
- all signer backends must return valid signed events or explicit errors.

---

## 6. Relay Policy Contract

### 6.1 Policy profiles

Current policy profiles:

- `Strict`
- `Community`
- `Open`

Current policy dimensions:

- allowlist
- blocklist
- default relays
- max subscriptions per caller
- max publishes per caller

Normative rule:

- relay selection is not caller-owned arbitrary transport,
- it is subject to host policy.

### 6.2 Resolution behavior

Subscription and publish relay resolution must:

1. normalize relay URLs,
2. deduplicate relay URLs,
3. apply default-relay fallback where appropriate,
4. enforce profile/allowlist/blocklist rules,
5. reject empty or fully-denied relay sets explicitly.

Normative rule:

- denied relay resolution is an explicit runtime-policy failure, not a best-effort silent skip.

---

## 7. Subscription Contract

`relay_subscribe(caller_id, requested_id, filters)` is the canonical subscribe operation.

Required behavior:

- capability gate `nostr:relay-subscribe`,
- reject empty filter sets,
- enforce per-caller subscription quota,
- resolve relays through policy,
- create a caller-owned subscription handle,
- ensure only the owning caller may unsubscribe.

Current backends:

- in-process relay service,
- optional supervised relay worker.

Normative rule:

- backend choice must not change the semantic caller contract.

---

## 8. Publish Contract

`relay_publish(...)` and `relay_publish_to_relays(...)` are the canonical publish operations.

Required behavior:

- capability gate `nostr:relay-publish`,
- validate signed event integrity basics,
- enforce per-caller publish quota,
- resolve relays through policy,
- return explicit publish receipt.

Current receipt semantics:

- `accepted`
- `relay_count`
- `note`

Normative rule:

- publish returns a receipt even when relay backend semantics are degraded or partially scaffolded,
- receipt meaning must remain explicit and diagnosable.

---

## 9. Worker Contract

The registry may attach a supervised relay worker.

Current worker responsibilities:

- websocket relay subscribe/unsubscribe/publish,
- NIP-46 RPC handling,
- inbound event delivery through event sink.

Normative rule:

- worker presence changes transport realization, not registry semantics,
- worker unavailability must degrade to explicit failure or local fallback where implemented,
- callers must not depend on worker implementation details.

---

## 10. Permission and Browser Bridge Contract

### 10.1 NIP-07

The registry owns host-side `window.nostr` bridge permission decisions.

Current bridge methods include:

- `getPublicKey`
- `signEvent`
- `getRelays`

Normative rule:

- per-origin permission is host-owned runtime policy,
- NIP-defined request semantics remain externally anchored,
- denied or malformed requests must emit explicit diagnostics.

### 10.2 NIP-46

The registry owns:

- delegated signer configuration,
- requested permission set,
- local permission grants/decisions,
- runtime connected/disconnected status snapshot.

Normative rule:

- remote signer authority does not erase host policy authority,
- local permission decisions remain Graphshell-owned.

---

## 11. Graph and Intent Boundary

The registry is not a direct graph mutation authority.

Normative rule:

- Nostr-originated data may later be projected into graph/runtime features,
- but such mutation must cross explicit runtime or reducer authorities,
- the registry itself remains a capability and transport/policy boundary.

This preserves the same architectural rule used elsewhere:

- protocol/runtime registry owns protocol behavior,
- graph/workbench layers own graph/workbench mutation.

---

## 12. Diagnostics and Test Contract

Required diagnostics families:

- capability denied,
- sign request denied,
- relay connect started/succeeded/failed/disconnected,
- relay subscription failed,
- relay publish failed,
- security violation,
- intent rejected where policy rejects follow-on behavior.

Required coverage:

1. local signing path produces valid event,
2. NIP-46 signing requires local permission allow,
3. NIP-07 methods are permission-gated,
4. caller-owned unsubscribe enforcement,
5. relay policy profile matrix for publish/subscribe,
6. publish rejects invalid signature,
7. worker subscribe/publish/unsubscribe roundtrip,
8. inbound relay event delivery through worker sink,
9. connect-failure diagnostics.

---

## 13. Acceptance Criteria

- [ ] Nostr protocol semantics remain anchored to NIPs.
- [ ] Graphshell-owned runtime policy is explicit and separate from protocol truth.
- [ ] relay policy, quotas, signer backends, and worker behavior are documented as registry-owned runtime concerns.
- [ ] direct graph mutation remains out of scope for the registry.
- [ ] diagnostics and tests cover denied, degraded, and successful runtime behavior.
