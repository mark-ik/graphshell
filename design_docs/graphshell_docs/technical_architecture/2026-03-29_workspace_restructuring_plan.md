<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workspace Restructuring Plan

**Date**: 2026-03-29
**Status**: Design note — structural target, not a build schedule
**Scope**: Define the Cargo workspace layout needed to support the portable
core extraction plans already in progress, and establish where each
protocol/capability lives in that layout.

**Related docs**:

- [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)
  — identity/authority/mutation kernel extraction plan; defines `graphshell-core`
- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
  — portable MiddleNet engine spec; defines `graphshell-web-core`
- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
  — host envelope model; capability portability survey (§12)
- [`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md)
  — canonical protocol packaging classes and host-aware protocol placement

---

## 1. Current State

The repository is a single Cargo crate (`graphshell`) with an empty
`[workspace]` table. All platform-native deps (egui, winit, Servo, wry,
fjall, keyring, iroh, gilrs) live alongside pure-Rust portable logic in
the same Cargo.toml. `[workspace]` must be populated with `members` before
any sub-crate can be extracted.

---

## 2. Target Workspace Shape

```
graphshell/                         ← workspace root (no lib/bin of its own)
  Cargo.toml                        ← [workspace] members = [...]
  crates/
    graphshell-core/                ← identity, authority, mutation kernel
    graphshell-core-wasm/           ← wasm-bindgen wrapper re-exporting core
    graphshell-core-uniffi/         ← UniFFI wrapper for iOS/Android
    graphshell-web-core/            ← portable MiddleNet rendering engine
    graphshell-comms/               ← portable comms protocol logic
  hosts/
    graphshell-desktop/             ← current `graphshell` binary (egui, Servo, wry)
    graphshell-firefox/             ← Firefox extension host (stub)
    graphshell-chrome/              ← Chrome extension host (stub)
    graphshell-ios/                 ← iOS host (stub)
    graphshell-android/             ← Android host (stub)
```

The current `graphshell` crate becomes `hosts/graphshell-desktop/` once the
portable crates are extracted. During the extraction period it can remain at
the workspace root with its existing `lib.rs`/`main.rs` while sub-crates are
carved out incrementally.

---

## 3. Crate Responsibilities

### 3.1 `graphshell-core`

The identity, authority, and mutation kernel. No rendering, no network I/O,
no platform I/O. Compiles to `wasm32-unknown-unknown` with zero errors.

Owns: `Graph`, `NodeKey`/`EdgeKey`, `Node`/`EdgePayload`, `GraphWorkspace`,
`GraphIntent` + `apply_intents()`, `GraphSemanticEvent`, `Address` enum,
`HistoryEntry`, `CoopSession` authority types, WAL log entry types, snapshot
serialization, NIP-84 clip schema, `CompactCode` (UDC), Nostr event/protocol
types (parsing, signing, serialization — no transport).

Does **not** own: storage backends, network transport, UI, rendering, TLS,
keypair storage.

Full spec: [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)

### 3.2 `graphshell-core-wasm` / `graphshell-core-uniffi`

Thin binding wrappers around `graphshell-core`. No logic of their own.
- `graphshell-core-wasm`: `wasm-bindgen` annotations; target for extension
  and browser-tab builds.
- `graphshell-core-uniffi`: UniFFI `cdylib` annotations; target for iOS and
  Android hosts.

### 3.3 `graphshell-web-core`

The portable MiddleNet rendering engine. Compiles to `wasm32-unknown-unknown`
(browser, via WebGPU) and `wasm32-wasip2` (native WASM runtimes, via
wasi-gfx). Also compiles natively with zero overhead.

Owns: html5ever DOM parsing, Stylo (single-threaded), Taffy layout, Parley
text, WebRender-wgpu rendering, Boa JS engine (via WIT command buffer),
protocol-to-DOM parsers (Gemini gemtext, Gopher, Finger, Spartan, Nex, RSS/
Atom, Markdown), the single intermediate document model, WASM snapshotting
hooks (Wizer pre-init), tiered WIT worlds (`middlenet:smallnet`,
`middlenet:document`, `middlenet:interactive`), reader-mode content extraction.

Does **not** own: network transport (hosts provide `wasi:http` or
`reqwest`), TLS/session policy, keypair storage, protocol server listeners,
graph state.

Full spec: [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)

### 3.4 `graphshell-comms`

Portable comms protocol client logic. Compiles to `wasm32-unknown-unknown`
(browser WebSocket/fetch paths) and `wasm32-wasip2` (wasi:sockets paths).

Owns:

| Protocol | What comms owns | What stays in host |
|---|---|---|
| **Titan** | Client request construction (URI, body, MIME, token), response parsing | TLS transport (rustls), TCP connection |
| **Misfin** | Message composition, wire format serialisation, received-message parsing, `text/gemini` body model | TLS transport, TCP listener (inbox server stays in host/wasip2) |
| **Gemini client** | Request/response parsing, gemtext → DOM adapter | TLS transport, TCP connection |
| **Gopher client** | Menu/text fetch, heuristic→DOM conversion, faithful-source mode | TCP connection |
| **Finger client** | Plain-text fetch, plain→DOM conversion | TCP connection |
| **Spartan client** | Request construction (GET/POST semantics), response parsing | TCP connection |
| **Nex client** | Directory/document fetch, gemini-style link parsing | TCP connection |
| **Guppy client** | UDP datagram construction/parsing | UDP socket (host-only) |
| **WebFinger** | `/.well-known/webfinger` query construction, JRD parsing | HTTP transport (reqwest) |
| **RSS/Atom** | Feed parsing, item normalization | HTTP transport (reqwest) |

`graphshell-comms` is **protocol logic**, not a network stack. It constructs
and parses bytes; hosts move the bytes.

Does **not** own: TLS sessions, TCP/UDP sockets, TLS certificates,
keypair storage, server listeners (those are wasip2-tier, in the desktop
host or a future `graphshell-server` crate).

**Relationship to `graphshell-web-core`**: `graphshell-comms` provides
parsers that `graphshell-web-core` can call to convert protocol responses
into the intermediate document model. `graphshell-web-core` depends on
`graphshell-comms`; not the other way around.

**Relationship to `graphshell-core`**: Nostr event types (parsing, signing,
serialization) live in `graphshell-core` because they are part of the
identity/publication schema. `graphshell-comms` may re-export or depend on
those types for the relay connection logic (WebSocket framing, filter
subscription), but the event model itself is in `graphshell-core`.

### 3.5 `hosts/graphshell-desktop`

The current `graphshell` crate, rehoused. Owns everything that requires a
native platform: egui, winit, Servo, wry, fjall, redb, keyring, iroh,
gilrs, mdns-sd, surfman. Depends on `graphshell-core`, `graphshell-web-core`,
and `graphshell-comms`. Adds: TCP/TLS transport, server listeners (Gemini,
Gopher, Finger inbox), OS keychain integration, filesystem paths, native
GPU surface, window management.

The desktop host is also the home for `wasm32-wasip2`-tier capabilities
that run natively without overhead: server listeners, iroh QUIC transport,
fjall/redb storage backends.

---

## 3A. Protocol Packaging Expectations

The workspace shape should be read together with the canonical protocol
modularity model.

Packaging rule:

- `graphshell-core` never owns protocol transport realization.
- `graphshell-web-core` owns portable rendering/document semantics.
- `graphshell-comms` owns portable protocol parsing/composition and client-side
  protocol logic.
- hosts and native feature mods own raw sockets, TLS sessions, browser APIs,
  keychains, server listeners, and native viewers.

Recommended packaging classes:

- `CoreBuiltins` — system-owned offline floor
- `DefaultPortableProtocolSet` — practical cross-host baseline
- `OptionalPortableProtocolAdapters` — extra cross-host lanes
- `NativeFeatureMods` — host-bounded capability bundles
- `NonEngineNetworkLayers` — adjacent identity/collaboration/storage systems

This gives later extraction work a canonical target for deciding whether a
protocol belongs in a portable crate, a host crate, or a non-engine subsystem.

---

## 4. Protocol Server Placement

Protocol servers (Gemini capsule, Gopher server, Finger responder, Misfin
inbox) are **not** in any portable crate. They require TCP listeners and
belong in:

- `hosts/graphshell-desktop` for immediate use (current `mods/native/verso/`)
- A future `graphshell-server` crate if a standalone headless server target
  is needed (wasm32-wasip2 + wasi:sockets)

The *server logic* (request routing, `SimpleDocument` → response, content
serialization) is pure Rust and could be extracted into `graphshell-comms`
as a server-side counterpart to the client parsers. The TCP listener itself
stays outside the portable crates.

---

## 5. Dependency Graph

```
graphshell-core          (no deps on other gs crates)
       ↑
graphshell-comms         (depends on graphshell-core for Nostr types, Address)
       ↑
graphshell-web-core      (depends on graphshell-comms for protocol parsers)
       ↑
hosts/*                  (depend on all three portable crates + platform deps)

graphshell-core-wasm     (re-exports graphshell-core with wasm-bindgen)
graphshell-core-uniffi   (re-exports graphshell-core with UniFFI)
```

No circular dependencies. The portable layer knows nothing about hosts. Hosts
compose the portable crates and add platform I/O.

---

## 6. WASM Compilation Targets by Crate

| Crate | `wasm32-unknown-unknown` | `wasm32-wasip2` | Native |
|---|---|---|---|
| `graphshell-core` | ✅ target | ✅ | ✅ |
| `graphshell-core-wasm` | ✅ (binding layer only) | — | — |
| `graphshell-core-uniffi` | — | — | ✅ (`cdylib`) |
| `graphshell-comms` | ✅ (fetch/WS paths) | ✅ (sockets paths) | ✅ |
| `graphshell-web-core` | ✅ (WebGPU) | ✅ (wasi-gfx) | ✅ |
| `hosts/graphshell-desktop` | — | — | ✅ |
| `hosts/graphshell-{firefox,chrome}` | ✅ | — | — |
| `hosts/graphshell-{ios,android}` | — | — | ✅ (`cdylib`) |

---

## 7. Migration Path

The workspace conversion is incremental. Each step is independently
mergeable and leaves the desktop build working throughout.

### Step 1 — Activate the workspace

Populate `[workspace]` in the root `Cargo.toml` with `members = ["."]`
(just the current crate). All existing tests continue to pass. This
unblocks adding sub-crates.

### Step 2 — Extract `graphshell-core`

Follow the existing extraction plan
([`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)).
Move identity/authority/mutation types into `crates/graphshell-core/`.
Desktop crate depends on it as a workspace member. WASM CI target added.

### Step 3 — Extract `graphshell-comms`

Move Gemini/Gopher/Finger/Spartan/Titan/Misfin/RSS client parsers and
protocol logic from `mods/native/verso/` into `crates/graphshell-comms/`.
TCP transport stays in the desktop crate (behind `tokio`). The server
listeners stay in the desktop crate. `graphshell-comms` depends on
`graphshell-core` for `Address` and Nostr types.

### Step 4 — Extract `graphshell-web-core`

Move or create the MiddleNet rendering engine in `crates/graphshell-web-core/`.
This is greenfield for the most part (Stylo, Taffy, WebRender-wgpu assembly).
`graphshell-web-core` depends on `graphshell-comms` for the protocol parsers.
Registered as `viewer:middlenet` in the desktop viewer registry.

### Step 5 — Rename desktop crate

Rename the root crate to `hosts/graphshell-desktop/` once the portable
crates carry the bulk of the logic. The root `Cargo.toml` becomes a pure
workspace manifest.

### Step 6 — Add extension/mobile host stubs

Add `hosts/graphshell-firefox/` etc. as WASM targets that depend on the
portable crates and the appropriate binding wrappers.

---

## 8. Open Questions

1. **`graphshell-comms` granularity** — should smallnet client parsers be a
   separate crate from RSS/WebFinger/Nostr relay logic, or is one comms crate
   sufficient? The single crate is simpler; split only if binary size or
   compile-time becomes a concern.

2. **Server logic placement** — server request/response logic (content routing,
   `SimpleDocument` → response) is portable. Should it live in `graphshell-comms`
   as a `server` feature, or stay in the desktop host until a server target
   is needed?

3. **`graphshell-core` + `graphshell-comms` Nostr boundary** — Nostr event
   types belong in `graphshell-core` (identity/publication schema). The relay
   connection (WebSocket framing, subscription management) is a transport
   concern and should live in `graphshell-comms` or the host. The split point
   is: *event model in core, relay I/O in comms or host*.

4. **Workspace root binary** — during the transition period, should the desktop
   binary remain at the workspace root (current layout) or move immediately to
   `hosts/graphshell-desktop/`? The former is lower friction; the latter is
   cleaner. Either works.

---

*This workspace shape is the structural consequence of having two parallel
portable extraction plans (identity kernel and MiddleNet engine) plus comms
protocol logic that serves both. The three portable crates form a clean
dependency chain; hosts are thin shells around them.*
