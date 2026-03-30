<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Protocol Modularity and Host-Capability Model

**Date**: 2026-03-30
**Status**: Active / canonical
**Scope**: Define how Graphshell classifies protocols across the portable core,
the portable middlenet engine, portable comms logic, host envelopes, and mod
tracks. This is the canonical architecture rule for protocol packaging and
host-aware degradation.

**Related docs**:

- [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)
  — `graphshell-core` authority boundary
- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
  — portable middlenet engine
- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
  — host-envelope capability model
- [`2026-03-29_workspace_restructuring_plan.md`](2026-03-29_workspace_restructuring_plan.md)
  — workspace crate boundaries
- [`../implementation_strategy/system/register/protocol_registry_spec.md`](../implementation_strategy/system/register/protocol_registry_spec.md)
  — protocol registry contract
- [`../implementation_strategy/system/register/mod_registry_spec.md`](../implementation_strategy/system/register/mod_registry_spec.md)
  — mod lifecycle contract
- [`../implementation_strategy/subsystem_mods/SUBSYSTEM_MODS.md`](../implementation_strategy/subsystem_mods/SUBSYSTEM_MODS.md)
  — mods subsystem policy authority
- [`../research/2026-03-30_middlenet_vision_synthesis.md`](../research/2026-03-30_middlenet_vision_synthesis.md)
  — research synthesis that motivated this canonicalization

---

## 1. Purpose

Graphshell needs a protocol model that stays coherent across:

- native desktop,
- browser extension hosts,
- browser-tab / PWA hosts,
- mobile hosts,
- future WASM-clean extraction work,
- native and WASM mod tracks.

The project already has:

- a portable authority core,
- a portable middlenet engine,
- a portable comms/protocol-logic direction,
- a mod-first registry architecture,
- a host-envelope capability model.

What was still missing was one canonical rule describing how protocols fit into
that structure without collapsing into either:

- "everything is in the engine", or
- "everything is a plugin", or
- "host differences force a different product model on every platform."

This document fills that gap.

---

## 2. Core Position

Graphshell has:

- a **portable core** for shared truth and authority,
- a **portable middlenet engine** for renderable document lanes,
- a **portable comms/protocol-logic layer** for protocol composition/parsing,
- **host envelopes** that grant or deny runtime capabilities,
- and **mods** that extend registry surfaces within those capability bounds.

Protocols are not a flat list. They are classified by:

1. the **user job** they serve,
2. the **technical layer** that owns them,
3. the **packaging class** they belong to,
4. the **host envelopes** that can realistically support them.

---

## 3. Product Rule: Organize By User Job First

Graphshell should reason about protocols first by the user job they serve, then
by the implementation mechanism.

Useful protocol job families:

- **Document lane** — what the user opens and reads
- **Discovery lane** — how a person/publication/endpoint is found
- **Mutation lane** — how a user publishes, uploads, or sends
- **Collaboration lane** — how users stay present together
- **Storage / replication lane** — how data persists, replicates, or is fetched from peers

This rule prevents the architecture from treating every protocol as a
"middlenet protocol" merely because it touches the web.

---

## 4. Engine Rule: What Belongs Where

### 4.1 `graphshell-core`

`graphshell-core` owns:

- shared truth semantics,
- identity and authority models,
- reducer-owned data contracts,
- durable portable schemas.

It does **not** own protocol transport realization.

### 4.2 `graphshell-web-core`

`graphshell-web-core` owns:

- shared document-model adapters,
- parsing-to-document-model integration,
- rendering semantics,
- style/layout/compositor behavior for middlenet content.

It owns **document/render adapters**, not raw socket transport, TLS sessions,
server listeners, or host API bindings.

### 4.3 `graphshell-comms`

`graphshell-comms` owns:

- portable protocol byte parsing,
- request/response construction,
- portable client-side protocol logic,
- protocol-specific normalization inputs for the middlenet engine.

It is protocol logic, not a network stack.

### 4.4 Hosts and native mods

Hosts and native feature mods own:

- raw sockets,
- TLS sessions and trust stores,
- keychain access,
- server listeners,
- browser-extension APIs,
- OS webviews,
- native viewers and runtime services.

### 4.5 Non-engine network layers

Some systems are important to Graphshell but are **not** middlenet engine
protocol modules by default:

- Nostr
- Matrix
- WebRTC
- IPFS / IPNS
- BitTorrent / WebTorrent
- Hypercore

These belong to identity, collaboration, storage, or community-network layers
unless a narrowly-scoped receivable integration is explicitly adopted.

---

## 5. Packaging Classes

Every protocol or protocol family should be classifiable into one of these five
packaging classes.

### 5.1 `CoreBuiltins`

System-owned, always-on capability floor required for Graphshell to remain
useful offline and boot coherently.

Examples:

- `file://`
- `about:`
- local plaintext/metadata viewers
- other non-network built-in capability seeds

Rule:

- `CoreBuiltins` are not optional feature mods.
- During transition they may be represented with manifest-like structures, but
  architecturally they are system-owned composition.

### 5.2 `DefaultPortableProtocolSet`

The practical baseline Graphshell should ship in nearly every host envelope.

This is the portable protocol floor that defines the product across desktop,
extension, browser-tab/PWA, and mobile.

Recommended default portable document/discovery floor:

- Gemini
- gemtext
- Gopher
- Finger
- static HTML
- Markdown / plain text
- RSS / Atom / JSON Feed
- gempub
- WebFinger

Rule:

- These protocols are central to Graphshell's middlenet/browser identity.
- They should be spec-able in a way that makes sense across practically every
  supported host.

### 5.3 `OptionalPortableProtocolAdapters`

Protocols that still fit multiple hosts, but are not part of the minimum
portable product floor.

Recommended examples:

- Titan
- Misfin
- Spartan
- Nex
- read-only ActivityPub / ActivityStreams ingestion

Rule:

- These remain valid extension units and may be shipped selectively, but their
  absence must not make the product incoherent.

### 5.4 `NativeFeatureMods`

Host-specific capability bundles that rely on platform services or runtime power
not uniformly available across hosts.

Examples:

- raw protocol servers/listeners,
- Guppy transport support,
- mDNS-driven local discovery,
- Wry-specific integrations,
- Servo-specific native integrations,
- keychain-backed native trust or identity helpers.

Rule:

- "Modular" does not mean the same binary plugin loads everywhere.
- A native feature mod is still a valid extension unit even if it is only
  available in one host class.

### 5.5 `NonEngineNetworkLayers`

Important Graphshell systems that should not be documented as middlenet engine
protocol modules by default.

Examples:

- Nostr
- Matrix
- WebRTC
- IPFS / IPNS
- BitTorrent / WebTorrent
- Hypercore

Rule:

- These may have receivable or bridge surfaces inside Graphshell, but they are
  primarily identity, collaboration, storage, or community-network layers.

---

## 6. Host Capability Profiles

Each protocol adapter must be spec-able against a named host capability profile.

### 6.1 Canonical envelopes

| Host profile | Typical powers | Typical gaps |
|---|---|---|
| **Desktop** | Raw sockets, native TLS, local filesystem, keychain, native viewers, richer runtime services | None relative to current target floor |
| **Extension** | Browser APIs, WebView/tab integration, storage APIs, WebRTC, fetch/WebSocket | No general raw TCP/UDP; constrained background/runtime APIs |
| **BrowserTab/PWA** | Fetch, WebSocket, WebRTC, browser storage, WebGPU | No extension APIs, no raw TCP/UDP, weaker local integration |
| **Mobile** | Native app runtime, platform webview, local storage, some keychain access | Different UI/runtime constraints; raw network and background policies vary by platform |

### 6.2 `HostCapabilityProfile`

Conceptually, every protocol capability description should be able to answer:

- Which host profiles can support this protocol meaningfully?
- Does support require transport delegation, native bridging, or host proxying?
- What is the degradation mode when the required capability is absent?

This profile is descriptive architecture truth even before it becomes a Rust
type or manifest schema.

---

## 7. Graceful Degradation Rule

Graceful degradation means:

- capability absence is explicit,
- registry registration is absent or reduced,
- diagnostics make the degraded state visible,
- the product remains coherent.

Graceful degradation does **not** mean:

- flattening every protocol into `graphshell-core`,
- pretending every host can support the same transport realization,
- silently falling back to unrelated systems,
- loading a protocol mod whose required capabilities do not exist.

The portable product model stays the same; hosts differ in which capabilities
they grant and which adapters can register.

---

## 8. Recommended Protocol Placement

### 8.1 Default portable floor

These are the strongest default portable document/discovery candidates:

- Gemini / gemtext
- Gopher
- Finger
- static HTML
- Markdown / plain text
- RSS / Atom / JSON Feed
- gempub
- WebFinger

These define the practical baseline Graphshell can plausibly support in nearly
every host envelope.

### 8.2 Optional portable adapters

These are valid cross-host extensions, but not part of the required floor:

- Titan
- Misfin
- Spartan
- Nex
- read-only ActivityPub

### 8.3 Native / host-bounded candidates

These should be documented as host-bounded unless and until a portable
realization is explicitly designed:

- Guppy
- raw smallnet protocol servers/listeners
- mDNS-driven local discovery
- Wry-specific integrations
- Servo-specific native integrations

### 8.4 Non-engine layers

These are not first-class middlenet engine protocols by default:

- Nostr
- Matrix
- WebRTC
- IPFS / IPNS
- BitTorrent / WebTorrent
- Hypercore

They remain strategically important, but they belong to adjacent system layers
unless a narrower receivable integration is explicitly adopted.

---

## 9. `ProtocolCapability` As Future Source Of Truth

The protocol registry should treat `ProtocolCapability` as the future source of
truth for packaging and host availability.

Required conceptual fields:

- `scheme_or_family`
- `lane_kind`
- `packaging_class`
- `host_support_profile`
- `requires_capabilities`
- `degradation_mode`

Where:

- `lane_kind` is one of:
  - `document`
  - `discovery`
  - `mutation`
  - `collaboration`
  - `storage_replication`
- `packaging_class` is one of:
  - `core_builtin`
  - `default_portable`
  - `optional_portable`
  - `native_only`
  - `non_engine_layer`

This document does not require the exact Rust type shape today, but it fixes the
fields future implementation work must preserve.

---

## 10. Decision Rule For Future Proposals

When evaluating a new protocol or protocol family:

- if it changes shared truth semantics, it belongs in core planning
- if it adds a renderable document lane, it belongs in protocol adapter and middlenet-engine planning
- if it is protocol parsing/composition without transport ownership, it belongs in comms planning
- if it requires raw sockets, keychain, native viewers, or host APIs, it belongs in host or native-mod planning
- if it is primarily identity, collaboration, or storage/replication infrastructure, it must not be documented as a middlenet engine protocol by default

This rule is intended to be decision-complete enough that later implementation
work does not have to reinterpret the architecture.

---

## 11. Relationship To Mods

This model does not replace the Mods subsystem taxonomy. It adds a protocol
packaging taxonomy that fits inside it.

Key alignment rules:

- `CoreBuiltins` remain system-owned composition, even when represented with
  manifest-like structures during transition.
- `DefaultPortableProtocolSet` and `OptionalPortableProtocolAdapters` may be
  built-ins, portable crates, or future WASM-capable extension units.
- `NativeFeatureMods` remain first-class mods even when they are unavailable in
  most hosts.
- "Modular" means capability-scoped and host-aware, not "the same binary
  plugin loads everywhere."

---

## 12. Acceptance Standard For This Model

This model is acceptable when:

- every protocol already discussed in Graphshell middlenet/smolnet research can
  be classified without ambiguity,
- the default portable floor makes sense across desktop, extension,
  browser-tab/PWA, and mobile,
- the docs clearly separate engine protocols from non-engine network/social/
  storage systems,
- a later engineer can place a candidate protocol into `graphshell-core`,
  `graphshell-web-core`, `graphshell-comms`, a host crate, or a non-engine
  subsystem without making new architecture decisions.

---

*Graphshell is modular by capability and host envelope, not by flattening all
protocols into core and not by demanding one universal plugin binary. The
portable floor defines the product; optional portable adapters extend it;
native feature mods add host-specific power; non-engine network layers remain
adjacent systems unless explicitly adopted as receivable lanes.*
