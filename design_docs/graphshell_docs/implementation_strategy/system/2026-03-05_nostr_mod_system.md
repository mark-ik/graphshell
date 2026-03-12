<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Nostr Apps as Graph Mods - WASM Mod System

**Date**: 2026-03-05
**Status**: Draft / canonical direction
**Scope**: Capability model and host contracts for running Nostr applications as Graphshell mods (WASM-first, WebView-compatible).

**Related docs**:

- [`2026-03-05_network_architecture.md`](2026-03-05_network_architecture.md) - iroh/libp2p/Nostr layer assignments.
- [`register/mod_registry_spec.md`](register/mod_registry_spec.md) - mod lifecycle and capability declarations.
- [`register/nostr_core_registry_spec.md`](register/nostr_core_registry_spec.md) - `NostrCore` capability IDs, diagnostics descriptors, and native provider profile.
- [`../subsystem_mods/SUBSYSTEM_MODS.md`](../subsystem_mods/SUBSYSTEM_MODS.md) - subsystem policy authority for mod lifecycle integrity.
- [`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) - capability restriction and sandbox policy authority.
- [`viewer/2026-02-23_wry_integration_strategy.md`](../viewer/2026-02-23_wry_integration_strategy.md) - viewer backend and overlay constraints.
- [`../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md`](../../../../verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md) - Verse/Nostr integration boundary.

---

## 1. Design Goal

Graphshell treats Nostr apps as Mods, not as privileged subsystems. A Nostr app may ship as:

- a **WASM Mod** (preferred path for Rust Nostr apps), or
- a **WebView-backed mod surface** using NIP-07 injection (`window.nostr`) for existing browser-targeted clients.

Both paths must obey the same policy: capabilities are deny-by-default, declared in `ModManifest.requires`, and granted by the host only if policy allows.

This is a system-level contract doc, not an implementation receipt.

---

## 2. Architecture Summary

### 2.1 Runtime ownership model

Nostr functionality is split between host-owned infrastructure and mod-owned logic.

| Concern | Owner | Why |
| --- | --- | --- |
| Relay sockets / connection pool | Host (`The Register`) | Prevent arbitrary outbound networking and duplicate socket pools. |
| Event signing operation | Host identity/security boundary | Preserve no-raw-secret invariant (`nsec` never enters mod memory). |
| UI rendering behavior | Mod surface contract | Nostr app controls UI flow within granted surface type. |
| Graph mutation authority | Host reducers/workbench authority | Mods propose intents; host remains single mutation authority. |
| Capability grant and revocation | Mod lifecycle + security subsystem | Enforce least privilege and runtime quarantine. |

### 2.2 Why host-owned relay pool is mandatory

Allowing each mod to open its own WebSocket relays would break three invariants:

1. **Security invariant**: WASM mods cannot gain undeclared network reachability.
2. **Privacy invariant**: relay policy must be centralized (allowlist/blocklist/rate guard).
3. **Performance invariant**: one shared relay pool avoids duplicate subscriptions and socket churn.

Therefore, mods consume a host relay capability rather than direct socket APIs.

---

## 3. Capability Contract for Nostr Mods

Nostr mods should request only the minimum required capabilities via `ModManifest.requires` keys using `namespace:name` format.

### 3.1 Canonical capability keys

| Capability key (`requires`) | Backing authority | Purpose |
| --- | --- | --- |
| `identity:nostr-sign` | Identity/security boundary | Request event signing without exposing raw secret material. |
| `nostr:relay-subscribe` | Host relay pool | Subscribe to filters and receive events from configured relays. |
| `nostr:relay-publish` | Host relay pool | Publish signed events through host-managed relay policy. |
| `graph:intent-emit` | Graph reducer/workbench authority | Emit bounded graph/workbench intents derived from Nostr events. |
| `graph:read-scope` | Read-only graph query API | Inspect graph state for UI context and de-dup heuristics. |
| `surface:overlay` | Workbench surface policy | Render as overlay/panel UI surface. |
| `surface:canvas-widget` | Canvas/workbench policy | Render inline graph-affine widget UI. |
| `action:register` | `ActionRegistry` | Register command palette actions. |
| `event:graph-subscribe` | Host event stream | React to selection/focus/workbench events. |

### 3.2 Minimum viable capability profiles

| Mod profile | Typical use | Required keys |
| --- | --- | --- |
| Read-only feed | Relay reader, no posting | `nostr:relay-subscribe`, `graph:intent-emit`, `surface:overlay` |
| Publisher | Notes/articles publish | Above + `identity:nostr-sign`, `nostr:relay-publish`, `action:register` |
| DM client | NIP-17 inbox/send | Above + `event:graph-subscribe` (optional for deep integration) |
| Canvas inline annotator | Node-local interaction | `nostr:relay-subscribe`, `graph:intent-emit`, `surface:canvas-widget` |

Capability scope is enforced at load and at call time. Undeclared or denied calls are hard failures and diagnostics events.

---

## 4. Surface Model: Overlay vs Canvas

Graphshell must support two valid Nostr UI surface classes.

### 4.1 Overlay surface (`surface:overlay`)

Use for timeline, DM inbox, profile editing, relay settings, and high-density interaction flows.

- Hosted in Workbench tile/pane surfaces.
- Full mod-owned UI state machine in a pane context.
- Best default for existing Nostr client behavior.

### 4.2 Canvas widget surface (`surface:canvas-widget`)

Use for spatially integrated controls attached to graph nodes.

- Inline note preview.
- Node-local actions (reply, repost, open thread, annotate).
- Lightweight interaction envelope to avoid canvas input conflict.

A single mod may provide both surface contracts if declared, but each surface grant is explicit and revocable independently.

---

## 5. Graph Integration Rules

Nostr mods never mutate graph state directly. They submit host-validated intents.

### 5.1 Event-to-intent mapping examples

| Nostr input | Host-side mapping intent |
| --- | --- |
| Kind 1 note containing URL | Create/attach content node proposal via reducer intent path. |
| NIP-23 long-form article | Structured content-node proposal with metadata tags. |
| NIP-84 highlight reference | Annotation node proposal linked to referenced node/address. |
| Profile event update | Identity/profile projection refresh intent. |

### 5.2 Guardrails

1. Intent emission is rate-limited per mod.
2. Intent payload schema is validated before reducer/workbench dispatch.
3. Rejected intents emit diagnostics with mod ID, reason, and severity.

---

## 6. Signing and Identity Boundary

### 6.1 No-raw-secret invariant

A Nostr mod must never receive raw `nsec` or equivalent private-key bytes.

- Allowed: `sign_event` operation with host-held or delegated signer.
- Disallowed: key export, direct key memory mapping, arbitrary signing primitive access.

### 6.2 Supported signing backends

1. Local secure host signer (device key store).
2. NIP-46 delegated remote signer.
3. Hardware wallet signer bridge where available.

Backend selection is host policy, not mod policy.

---

## 7. Mod Packs for Nostr Clients

A Nostr client experience should usually ship as a **mod pack** (composed set of mods) rather than one monolith.

### 7.1 Pack semantics

Mod pack metadata defines:

- module membership,
- shared capability grants,
- startup sequencing,
- optional shared relay/session context.

### 7.2 Example pack topology

| Module | Role | Typical capabilities |
| --- | --- | --- |
| `nostr-feed` | timeline and subscriptions | `nostr:relay-subscribe`, `surface:overlay` |
| `nostr-dm` | DM inbox/send | `nostr:relay-subscribe`, `nostr:relay-publish`, `identity:nostr-sign`, `surface:overlay` |
| `nostr-profile` | profile/follows editor | `nostr:relay-subscribe`, `nostr:relay-publish`, `identity:nostr-sign`, `surface:overlay` |
| `nostr-canvas` | node-embedded affordances | `nostr:relay-subscribe`, `graph:intent-emit`, `surface:canvas-widget` |

Pack resolution is atomic: if required shared capabilities cannot be granted, the pack fails activation with a deterministic diagnostics event.

---

## 8. WebView Compatibility Path (NIP-07)

For existing web-based Nostr clients, Graphshell supports a compatibility mode:

1. Run client in a WebView-backed mod surface.
2. Inject a `window.nostr` provider implementing NIP-07-compatible methods.
3. Route signing and relay operations through host policy boundaries.

This path optimizes adoption speed for existing clients and does not replace the WASM-first path.

Current implementation note:

- The host now injects a built-in `window.nostr` bootstrap through the shared webview
  `UserContentManager`.
- `getPublicKey`, `signEvent`, and `getRelays` route through the host-owned `NostrCoreRegistry`
  rather than page-local secrets or direct sockets.
- Sensitive methods are denied by default until the origin is allowed in Settings -> Sync.
- The current bridge intentionally stops at core NIP-07 methods; optional browser-wallet parity
  methods such as `nip04`/`nip44` remain follow-on depth.

### 8.1 Policy constraints for NIP-07 bridge

- Inject only when mod declares `identity:nostr-sign` and at least one relay capability.
- Expose only approved methods.
- Log denied calls and capability overreach attempts to diagnostics/security channels.

---

## 9. Diagnostics and Failure Modes

Nostr mod runtime must emit explicit diagnostics for policy and health visibility.

Suggested channels (naming aligned to existing `namespace:name` style):

- `mod:nostr:capability_denied` — Warn
- `mod:nostr:sign_request_denied` — Warn
- `mod:nostr:relay_publish_failed` — Warn
- `mod:nostr:relay_subscription_failed` — Warn
- `mod:nostr:intent_rejected` — Warn
- `mod:nostr:security_violation` — Error

Severity values follow canonical diagnostics policy: use `Error` for explicit security/failure channels, `Warn` for denied/fallback/degraded paths.

---

## 10. Implementation Slices (Issue Seeding)

### Slice A - Host relay capability surface

**Goal**: Provide host-owned relay subscribe/publish contract for mods.

- Define ABI-safe host calls for subscribe/unsubscribe/publish.
- Enforce relay policy and per-mod quotas.
- Add diagnostics for relay-level failures and overreach.

### Slice B - Signing bridge contract

**Goal**: Expose operation-level signing without key exposure.

- Define `sign_event` host function contract.
- Support local signer and NIP-46 backend.
- Add denial paths for unauthorized signing requests.

### Slice C - Surface integration

**Goal**: Overlay and canvas widget surface grants for Nostr mods.

- Register Nostr panel entry points in workbench/tooling.
- Define canvas widget lifecycle hooks.
- Ensure focus and accessibility compatibility with existing subsystem contracts.

### Slice D - Mod pack activation

**Goal**: Atomic pack load/validation.

- Add pack manifest schema and dependency semantics.
- Resolve shared capability grants.
- Fail whole pack on unresolved required grants.

### Slice E - NIP-07 bridge for WebView mods

**Goal**: Compatibility lane for existing web clients.

- [x] Inject controlled `window.nostr` bridge.
- [x] Enforce method-level capability checks.
- [ ] Record bridge usage metrics and denied-call diagnostics beyond the existing Nostr denial
  channels.

---

## 11. Acceptance Criteria

The Nostr mod system contract is considered complete when all of the following are true:

1. A WASM Nostr mod can subscribe and publish via host relay capability with no direct socket access.
2. A mod can request signing, but cannot access raw secret key material in any path.
3. Overlay and canvas surface grants are independently enforceable and revocable.
4. Intent emission from Nostr events is host-validated and reducer/workbench-authority compliant.
5. A composed Nostr mod pack can be validated and activated atomically.
6. A WebView Nostr client can run with NIP-07 bridge injection under capability checks.
7. Capability denial, intent rejection, and security violations are observable in diagnostics channels.

---

## 12. Draft Action/Command Catalog (Graph + Workbench Targets)

This section defines practical first-wave integration actions and command palette bindings.

### 12.1 Action IDs and command aliases

| Action ID | Command alias (example) | Primary surface | Caller ID recommendation | Nostr ops |
| --- | --- | --- | --- | --- |
| `action.nostr.feed.subscribe_view` | `/nostr sub view` | Workbench feed pane | `mod:nostr-feed:<graph_view_id>` | `relay_subscribe` |
| `action.nostr.feed.unsubscribe_view` | `/nostr unsub view` | Workbench feed pane | `mod:nostr-feed:<graph_view_id>` | `relay_unsubscribe` |
| `action.nostr.feed.subscribe_filter` | `/nostr sub kind:1 #graphshell` | Workbench feed pane | `mod:nostr-feed:<graph_view_id>` | `relay_subscribe` |
| `action.nostr.publish.selection_note` | `/nostr post selection` | Graph + workbench composer | `mod:nostr-publish` | `sign_event` + `relay_publish` |
| `action.nostr.publish.selection_highlight` | `/nostr highlight selection` | Graph annotation flow | `mod:nostr-publish` | `sign_event` + `relay_publish` |
| `action.nostr.reply.to_node_thread` | `/nostr reply node` | Thread inspector pane | `mod:nostr-thread` | `sign_event` + `relay_publish` |
| `action.nostr.profile.open_author_graph` | `/nostr open author` | Graph view | `mod:nostr-profile` | `relay_subscribe` |
| `action.nostr.dm.open_thread` | `/nostr dm <npub>` | Workbench DM pane | `mod:nostr-dm` | `relay_subscribe` + `relay_publish` |
| `action.nostr.suggest.from_selection` | `/nostr suggest` | Graph canvas | `mod:nostr-suggest:<graph_view_id>` | `relay_publish` (NIP-90 request) + `relay_subscribe` (result stream) |
| `action.nostr.pin_event_to_graph` | `/nostr pin event` | Graph node context menu | `mod:nostr-feed` | read-only (local intent emit only) |

### 12.2 Intent mapping targets

| Action ID | Graph intent target | Workbench intent target |
| --- | --- | --- |
| `action.nostr.feed.subscribe_view` | none | open/refresh feed tile and bind subscription handle |
| `action.nostr.publish.selection_note` | create or update a node with published event metadata | append publish receipt row in composer/log pane |
| `action.nostr.publish.selection_highlight` | create annotation edge from source node to note node | open highlight details pane |
| `action.nostr.reply.to_node_thread` | attach thread edge to selected node context | focus thread pane and jump to reply |
| `action.nostr.profile.open_author_graph` | spawn/merge author-centric node cluster | open profile inspector |
| `action.nostr.suggest.from_selection` | render ghost suggestion nodes and candidate edges | show ranked suggestion list with accept/reject |
| `action.nostr.pin_event_to_graph` | promote feed event into durable node | keep event row pinned in feed pane |

### 12.3 Command UX constraints

1. All commands must be callable from command palette and context menus.
2. Every command must resolve to a caller-scoped Nostr operation (`*_for_caller`) rather than generic runtime caller.
3. Commands that publish should support explicit relay targets (`relay_publish_to_relays`) and fallback to policy defaults only when no target is provided.
4. Subscription commands must return and track handles for deterministic teardown on pane close or graph view exit.

---

## 13. Social Layer Feature Implementations (First-Wave)

### 13.1 Graph-native features

| Feature | Description | Nostr ops | Intent outputs |
| --- | --- | --- | --- |
| Ghost suggestions | DVM-ranked traversal suggestions render as ghost nodes/edges | publish request + subscribe result | candidate node/edge proposal intents |
| Event provenance badges | Nodes show source event kind/author/status | subscribe feed stream | node metadata update intents |
| Thread-linked nodes | Reply chains map to graph edges between event nodes | subscribe + publish | edge create intents (`reply`, `repost`, `quote`) |
| Pin-to-graph | Promote event from transient feed row to durable node | none (local) | node create/link intents |

### 13.2 Workbench-native features

| Feature | Description | Nostr ops | Intent outputs |
| --- | --- | --- | --- |
| Timeline pane | Filterable event feed with per-view subscription scope | subscribe/unsubscribe | pane state + optional pin intents |
| Composer pane | Draft, sign, publish with relay targeting | sign + publish | publish receipt + graph metadata intents |
| Thread inspector | Event thread reading/reply workflow | subscribe + publish | thread context intents |
| Relay policy pane | profile selector (strict/community/open), allow/block/default relay edits | none (registry config) | settings/policy intents |

### 13.3 Co-op aligned social affordances

| Feature | Description | Notes |
| --- | --- | --- |
| Shared session notes | Host/guest publish selected graph notes to session feed | guests follow host grants; private-by-default remains unchanged |
| Cursor-aware references | Optional event tags include active graph view and selection hint | no workbench mirroring required |
| Session snapshot export | Convert visible session-shared nodes + user notes to a new graph view | aligns with coop snapshot carryback policy |

---

## 14. Out of Scope

- Full social-client parity for all Nostr interaction patterns.
- Verse incentive-market features (DVM and economic policy details).
- General-purpose unrestricted outbound networking for WASM mods.

Those remain governed by separate Verse and network architecture lanes.

---

## 15. Native Composition Direction: Verso / Verse / NostrCore

The preferred architecture is compositional at the component level, not monolithic at the system level:

- **Verso** and **Verse** remain composed from multiple internal components.
- `iroh`/`libp2p` transport primitives are first-party native components, not third-party WASM networking mods.
- Nostr capability should be exposed through a dedicated first-party native boundary: **`NostrCore`**.

### 15.1 Why `NostrCore` should be a dedicated native mod

`NostrCore` is the platform capability layer for Nostr identity/event/relay operations. Keeping it separate from Verso/Verse avoids duplicated signing and relay logic while preserving strict authority boundaries.

`NostrCore` responsibilities:

- identity/signing boundary (`identity:nostr-sign`) with no raw key export,
- relay subscribe/publish pool and policy enforcement,
- event normalization and protocol guardrails,
- diagnostics emissions for failure/overreach,
- host bridge for NIP-07 (`window.nostr`) in WebView app-node mode.

Verso/Verse consume `NostrCore` capabilities; they should not each implement their own independent relay/signing stack.

---

## 16. Responsibility Matrix

| Capability area | `NostrCore` (native mod) | Verso components | Verse components | Ecosystem Nostr apps (WebView/WASM) |
| --- | --- | --- | --- | --- |
| Key/signing boundary | Owns | Consumes | Consumes | Never owns raw key |
| Relay pool + policy | Owns | Consumes | Consumes | Consumes via capability only |
| Coop/session transport | No | Owns (`iroh` session primitives) | No | No |
| Swarm discovery/replication | No | No | Owns (`libp2p` topology) | No |
| Graph intent mapping policy | Provides guardrails | Uses for coop workflows | Uses for public workflows | Can propose bounded intents |
| Full social timeline UX | Not required | Not required | Optional later | Primary source |
| NIP-07 web bridge | Owns host bridge | Can trigger usage | Can trigger usage | Consumes |
| Security diagnostics | Owns Nostr channels | Contributes transport channels | Contributes swarm channels | Emits mod-level usage signals |

This split keeps trust-critical transport and identity logic first-party while preserving an ecosystem compatibility surface.

---

## 17. Feature Ownership Policy

### 17.1 First-party baseline (must own)

Graphshell should own these features natively as baseline Nostr capability:

1. Nostr identity/signing abstraction with local signer + NIP-46 support.
2. Shared relay pool and policy enforcement.
3. DM/invite primitives needed by coop and graph workflows.
4. Event-to-graph mapping for graph-native use cases.
5. Minimal profile/follows ingestion for discovery workflows.
6. Host-controlled NIP-07 bridge for app-node compatibility.

### 17.2 Ecosystem accessory lane (should not block baseline)

Use external Nostr clients for optional expansion:

1. Full social timeline parity and social-network UX depth.
2. Zap-heavy or economics-first experiences.
3. Specialized clients that are not graph-native priorities.

The Nostr ecosystem is an accessory and extension lane, not the dependency root for core Graphshell Nostr functionality.

---

## 18. App-Node Policy for Web Nostr Clients

Web Nostr clients can be launched as node-backed app surfaces using `#app` tagging and dedicated UI affordances.

### 18.1 App-node model

- Tag eligible nodes with `#app` and `app:nostr` metadata.
- Offer a workbench affordance to open a frame filtered to an app tag set.
- Launch in WebView/Servo as pane surfaces, subject to capability policy.

### 18.2 App eligibility policy

Adopt a curated allowlist for first-class app-node integrations:

- license compatibility requirement (project-approved SPDX list),
- capability declaration compliance,
- baseline runtime compatibility (WebView/Servo),
- privacy/security behavior requirements.

This gives a fast compatibility story without making third-party apps authoritative over native platform behavior.

---

## 19. Implementation Lanes (Tiered)

### Tier 1 - Native Nostr baseline

1. Define and register `NostrCore` native mod manifest and capability exports.
2. Implement relay pool host service with shared subscribe/publish and policy enforcement.
3. Implement signer service (`local` + `NIP-46`) and no-raw-secret contract tests.
4. Add graph-intent mapping adapters for baseline Nostr event kinds used by graph workflows.
5. Add diagnostics coverage for capability denial, relay errors, signing denial, and security violations.

### Tier 1.5 - Native UX + app-node enablement

1. Build first-party Nostr-integrated native panes (inbox/profile/discovery baseline).
2. Add app-node affordance for `#app` + `app:nostr` tagged nodes.
3. Implement curated app allowlist and compatibility labeling.

### Tier 2 - Ecosystem expansion

1. Expand NIP coverage and optional social features not required by graph workflows.
2. Add richer app-node orchestration and pack-level experiences.
3. Integrate optional Verse economic/NIP-90 lanes without coupling to Tier 1 baseline.
