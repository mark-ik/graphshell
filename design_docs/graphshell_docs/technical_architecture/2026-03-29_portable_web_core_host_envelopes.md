<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Portable Web Core and Host Envelopes

**Date**: 2026-03-29
**Status**: Design note
**Scope**: Capture the architectural conclusion that Graphshell should have one reusable portable
web/document engine shared across native, extension, mobile, and browser-tab hosts, with host
capabilities layered around it.

**Related docs**:

- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
  — MiddleNet protocol space definition and portable engine component spec
- [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)
  — existing identity/authority kernel extraction plan
- [`2026-03-29_workspace_restructuring_plan.md`](2026-03-29_workspace_restructuring_plan.md)
  — Cargo workspace layout: crate responsibilities, dependency graph, migration steps
- [`GRAPHSHELL_AS_BROWSER.md`](GRAPHSHELL_AS_BROWSER.md) — user-visible browser model summary
- [`../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../verso_docs/technical_architecture/VERSO_AS_PEER.md)
  — current web-peer and browser-capability placement
- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
  — current content adaptation and shared document-model direction
- [`../research/2026-03-01_webrender_wgpu_renderer_research.md`](../research/2026-03-01_webrender_wgpu_renderer_research.md)
  — WebRender/WGPU research context

---

## 1. Purpose

This note records the conclusion of the 2026-03-29 architecture discussion:

- Graphshell should expose **one portable reusable web/document engine** across all deployment
  contexts.
- Native desktop, Firefox/Chrome extensions, browser-tab builds, iOS/Android, and future hosted
  shells are different **host envelopes**, not different engines.
- The portable engine should be capable enough to serve both HTTP content and smallnet
  document-style protocols, while degrading cleanly when a host cannot provide richer
  capabilities.

This is a product-architecture position with one naming clarification:
the canonical portable engine crate name should be `middlenet-engine`.

Important clarification:

- `wasm32-unknown-unknown` is the portable **browser-host** target.
- `wasm32-wasip2` is the portable **runtime/service-host** target.
- Windows/macOS/Linux desktop apps remain native hosts by default.

That means WASI is not the main desktop packaging story. It is the portable
runtime story for capabilities that need sockets, listeners, storage backends,
or headless embedding outside the browser.

---

## 2. Naming Clarification

There is already an active plan defining `graphshell-core` as the **identity, authority, and
mutation kernel** of the system, with no knowledge of rendering, web runtimes, or platform I/O.
That plan remains valid.

This note describes a second architectural concern: the **portable web/document engine** reused by
all hosts.

To avoid accidental conflict with the existing `graphshell-core` plan, this document uses the term
**portable web core** as architecture prose.

The naming split should now be treated as settled:

1. `graphshell-core` remains the identity, authority, and mutation kernel.
2. `middlenet-engine` is the sibling portable document/render engine.
3. "portable web core" remains acceptable descriptive language for the shared
  engine boundary, but not the Cargo package name.

The architecture decision in this note is still about **singularity and
reuse**, but the crate name is no longer deferred.

---

## 3. Decision

Graphshell should have **one singular portable web/document core** reused by all platform shells.

Recommended top-level product shape:

- `middlenet-engine`
- `graphshell-core`
- `graphshell-firefox`
- `graphshell-chrome`
- `graphshell-ios`
- `graphshell-android`
- `graphshell-windows`
- `graphshell-macos`
- `graphshell-linux`

Interpretation:

- the platform packages are thin host shells and distribution targets,
- the portable engine is where the durable browser/document semantics live,
- platform packages differ by capability envelope, not by document/rendering identity.

The discussion rejected the idea that the repo should present many peer "core-ish" crates as the
primary mental model. The user-facing and team-facing model should be: **one core, many hosts**.

### External pattern note (2026-04-01): Grafeo / SparrowDB

Grafeo is a useful reminder that broad host reach is valuable only if the reusable engine remains singular and host capability envelopes stay explicit. SparrowDB is a useful reminder that the embedded story should remain blunt about startup model, workload, and durability promises.

Together they support the one-core-many-host framing in this note and argue against per-host semantic drift, hidden transport or storage ownership inside the portable engine, or vague platform-parity claims without explicit capability ladders.

---

## 4. What the Portable Web Core Owns

The portable web core should own the logic that ought to behave the same everywhere Graphshell
runs.

### 4.1 Document and Navigation Kernel

- content classification
- normalized resource/document model
- DOM or document-tree model
- navigation state
- history semantics local to the content engine
- link routing and internal navigation intents

### 4.2 Parsing and Adaptation

- HTML parsing path
- `SimpleDocument`-style block adaptation path
- smallnet document parsing where the semantics are universal
- feed/article/document normalization

### 4.3 Style, Layout, and Rendering

- style system
- layout engine
- render-tree or display-list construction
- rendering/compositor abstraction
- the renderer path intended to be portable across hosts

### 4.4 Protocol-Neutral Semantics

The portable core may encapsulate document-side semantics for:

- HTTP/HTTPS content
- Gemini/gemtext
- Gopher menus and text documents
- Finger/profile-style text resources
- Markdown/plain text/readable article transforms

This means the core can be the shared rendering substrate for both the web and the smallnet,
provided protocol transport details remain outside the core.

---

## 5. What Host Envelopes Own

Hosts provide capabilities that vary by platform power and permission model.

Examples:

- network transport
- storage and persistence adapters
- clipboard
- notifications
- file access
- page integration
- protocol launching
- native bridge access
- raw socket or helper-process access

The core should consume these through capability interfaces and degrade when they are absent.

Examples by host:

| Host | Core engine | Host-specific envelope |
| --- | --- | --- |
| Native desktop | Full portable web core | native transport, filesystem, richer storage, full UI integration |
| Firefox/Chrome extension | Same portable web core | extension permissions, background worker, content-script/page integration, optional native messaging |
| Browser tab / hosted site | Same portable web core | browser-only APIs, no native bridge unless companion service exists |
| iOS / Android | Same portable web core | mobile storage, mobile permissions, mobile shell integration |
| Native WASM runtime / service host | Same portable web core | `wasi:sockets`, `wasi:filesystem`, component embedding, headless/service lifecycle |

---

## 6. HTTP Capability Ladder

The portable core should treat HTTP as the richest protocol lane.

Reason:

- HTTP can carry simple documents, rich hypertext, and eventually interactive applications.
- The same rendering/document kernel that serves reader-mode and article content can grow upward
  toward broader browser compatibility.

The realistic capability ladder is:

### Level 1 — Reader-grade HTTP

- documents, blogs, docs, wikis, forums, articles
- strong static HTML/CSS
- image and subresource loading
- no or minimal JS

### Level 2 — Light-interactive HTTP

- form handling
- light DOM mutation
- timers, fetch, basic eventing
- moderate JS support

### Level 3 — App-leaning HTTP

- richer DOM/WebIDL surface
- more complete runtime behaviors
- compatibility with more modern sites

The transition from Level 1 to Level 3 is not primarily "better rendering." It is "more browser
runtime."

This note therefore preserves the distinction:

- rendering portability is one problem,
- browser-application compatibility is a larger runtime problem layered above it.

---

## 7. Smallnet Position

Smallnet protocols are intentionally smaller in scope than the modern web. That is a benefit, not
a mismatch.

The portable core should treat them as first-class document lanes where:

- parsing is simpler,
- rendering semantics are more stable,
- protocol-native text/document presentation matters,
- the same core engine can render them in native, extension, and browser-tab contexts.

However, the core should not absorb all protocol transport/runtime ownership.

Keep outside the portable core:

- socket handling
- TLS/session policy
- capsule/server behavior
- protocol publishing/serving
- trust and certificate storage

Keep inside the portable core:

- document parsing
- normalized content model
- navigation semantics
- rendering
- cross-protocol adaptation to a common engine

In short:

- the core should be able to **render** smallnet content everywhere,
- hosts decide how much of the smallnet **transport power** they can expose.

---

## 8. Extension and Browser-Tab Value

The discussion explicitly concluded that the portable web core is strategically more valuable if it
can be hosted in:

- a browser extension,
- a hosted website/PWA,
- the native Graphshell app.

This makes the engine:

- distributable,
- embeddable,
- useful outside the native app,
- capable of shipping a smallnet browser experience inside existing browsers.

### 8.1 Extension Host

An extension host is the strongest non-native envelope because it can usually provide:

- background execution context
- host permissions
- stronger storage than a normal webpage
- page integration via content scripts
- optional native helper bridge

This makes an extension a plausible "near-parity" envelope for core Graphshell browsing behavior.

### 8.2 Browser-Tab / Hosted Site Host

A plain website or PWA is weaker but still strategically important:

- it can host the same portable engine,
- it can render the same document model,
- it can expose a useful subset of browsing behavior,
- it can serve as a hosted smallnet/web reader package.

The hosted-site envelope should be designed for graceful degradation rather than parity with the
native desktop app.

---

## 9. Path B Interpretation

The discussion contrasted two broad paths:

- growing upward from a lighter reader-oriented stack,
- trimming downward from a browser-engine-shaped stack.

The conclusion here is:

- a portable Graphshell web core is possible from the **browser-engine-shaped path**,
- but the right target is a **Servo-shaped kernel**, not "full Servo unchanged."

That means:

- reuse the architectural split of document/script/layout/rendering concerns,
- keep the portable core focused on the document/rendering kernel,
- avoid requiring the entire native browser runtime to live inside the core unchanged.

This is compatible with pursuing a portable rendering path centered on `WebRender`/`wgpu`, while
still acknowledging that full browser-app compatibility is a larger runtime effort.

---

## 10. Packaging and Internal Shape

The preferred repo and product presentation is:

- one obvious portable engine,
- many platform hosts,
- internal modules beneath the core rather than many peer "core" products.

Conceptually:

```text
middlenet-engine/
  src/
    document/
    parsing/
    style/
    layout/
    render/
    navigation/
    protocols/
    capabilities/

graphshell-firefox/
graphshell-chrome/
graphshell-ios/
graphshell-android/
graphshell-windows/
graphshell-macos/
graphshell-linux/
```

Important: the architectural point is not that every internal concern must become a top-level
crate. The internal concerns may remain modules within the portable engine as long as the host
boundary stays clear.

---

## 11. Working Rule

When deciding whether something belongs in the portable web core, ask:

> If Graphshell runs in native desktop, Firefox, Chrome, iOS, Android, and a browser tab, should
> this behavior feel like the same engine?

If yes, it belongs in the portable core.

If no, it is a host-envelope concern.

---

## 12. Capability Portability Survey

Not every capability currently in a native mod is native-only. The determining
factor is not which mod owns the code today, but which of three axes apply:

1. **Logic / parsing / crypto** — pure Rust, no platform dependency → fully
   WASM-portable to any target.
2. **Networking** — portable with the right WASI interface: `wasi:sockets/tcp`
   for raw TCP (wasmtime/native WASM runtimes), browser `fetch`/WebSocket for
   `wasm32-unknown-unknown` (browser context).
3. **Platform services** — OS keychain, filesystem, native windowing, hardware
   — host-provided only, never in the portable core.

The split is **logic vs. platform service**, not **Verso vs. everything else**.

### 12.1 Capabilities portable to all WASM targets

These can live in the portable core unconditionally:

| Capability | Current location | Notes |
|---|---|---|
| HTTP/HTTPS client (`reqwest`) | Verso | Auto-detects WASM; uses browser `fetch` in browser context |
| Gemini/Gopher/Finger **client** (fetch + parse + render) | Verso | Pure Rust parsing; transport via `reqwest` |
| TLS cert generation (`rcgen`) | Verso (Gemini server) | Pure Rust |
| `rustls` | Verso | Pure Rust TLS |
| Crypto: `ed25519-dalek`, `secp256k1`, `sha2`, `aes-gcm` | Identity / Verso | Pure Rust; key ops and signing portable everywhere |
| Nostr **protocol** (parsing, event signing, serialization) | Nostrcore | Pure Rust; network transport is separate |
| Content hashing: `cid`, `multihash`, `base64`, `zstd` | Various | Pure Rust |
| `serde` / `serde_json` / `rkyv` | Everywhere | Pure Rust |
| `client_storage` **interface** (the trait) | Verso | Trait only; backends are host-provided |
| Feed parsing (RSS/Atom) | (not yet extracted) | Pure Rust; belongs in portable core |

### 12.2 Capabilities portable to `wasm32-wasip2` (native WASM runtime) only

These require `wasi:sockets` or `wasi:filesystem`, which are available via
wasmtime/native WASM runtimes but not in a browser context:

| Capability | Gating interface | Notes |
|---|---|---|
| Gemini server (TLS TCP) | `wasi:sockets/tcp` | tokio → WASI-native async on this target |
| Gopher server (plain TCP) | `wasi:sockets/tcp` | RFC 1436; simple enough to port off tokio |
| Finger server (plain TCP) | `wasi:sockets/tcp` | Trivial protocol |
| Nostr relay **connection** (WebSocket/TCP to relays) | `wasi:sockets/tcp` | Browser context uses WebSocket instead |
| `client_storage` **fjall/redb backend** | `wasi:filesystem` | Browser context uses IndexedDB instead |
| `iroh` P2P (QUIC transport) | `wasi:sockets` + QUIC | Browser context: WebTransport (experimental) |

For the server capabilities specifically: the server *logic* (request parsing,
response serialisation, content routing from `SimpleDocument`) is portable
today. The only change needed for `wasm32-wasip2` is replacing tokio's
platform-native async runtime with a WASI-compatible executor (e.g. `wstd`, or
tokio with the WASI target when that stabilises).

This is the important product interpretation:

- `wasm32-unknown-unknown` proves the core works in browser-hosted contexts.
- `wasm32-wasip2` gives Graphshell a portable service/runtime target for
  protocol servers, relay/storage workers, background graph updaters, tests,
  and embedded non-browser hosts.
- native desktop remains the straightforward UI app deployment target.

### 12.3 Native-only (never in the portable core)

| Capability | Reason |
|---|---|
| `viewer:wry` (OS webview overlay) | WebView2/WebKit are platform-native browser engines |
| `viewer:webview` (Servo) | SpiderMonkey, native threads, GL |
| `keyring` (OS keychain) | Platform keychain API |
| `mdns-sd` (local discovery) | Raw UDP multicast; no WASI equivalent |
| `gilrs` (gamepad) | Platform HID |
| `winit` / `egui` / `egui-wgpu` | Platform windowing + GPU-backed UI composition |
| Native filesystem paths (direct `std::fs`) | Use `wasi:filesystem` abstraction instead |

### 12.4 The working rule for capability placement

> If the capability's behaviour is identical regardless of whether Graphshell runs natively, inside wasmtime, or inside a browser extension — it belongs in the portable core, gated by the appropriate WASI capability interface. If the capability requires a platform service that varies per host (OS keychain, native windowing, raw hardware access) — it is a host envelope concern and stays outside the portable core.

The portable core **declares** capability requirements via WIT imports. The
host envelope **grants** them. The core degrades cleanly when a capability is
absent; it does not hard-fail.

---

## 13. Open Follow-Up

This note leaves the following decisions open:

1. final crate naming relative to the existing `graphshell-core` extraction plan,
2. whether the portable web core is a sibling crate or a major subsystem under the existing core
   umbrella,
3. how aggressively the HTTP runtime should grow beyond reader-grade behavior,
4. which parts of smallnet transport, if any, deserve optional helper-backed adapters in extension
   and hosted-site environments.

---

*This note records the architecture conclusion of the 2026-03-29 discussion: one portable web
core, many host envelopes, with HTTP and smallnet content sharing the same engine where their
document semantics overlap.*
