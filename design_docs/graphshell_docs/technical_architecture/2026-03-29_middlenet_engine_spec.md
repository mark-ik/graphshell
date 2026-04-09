<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# MiddleNet and the Portable MiddleNet Engine

**Date**: 2026-03-29
**Status**: Design note — target architecture with phased delivery baseline
**Scope**: Define the MiddleNet protocol space, name the portable WASM rendering
engine that serves it, and describe how it fits into each host envelope.

**Related docs**:

- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
  — host envelope model, portability strategy, and naming context for the
  portable web core
- [`2026-03-29_workspace_restructuring_plan.md`](2026-03-29_workspace_restructuring_plan.md)
  — Cargo workspace layout: crate responsibilities, dependency graph, migration steps
- [`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md)
  — canonical protocol packaging classes, default portable floor, and host-aware degradation model
- [`2026-04-09_identity_convergence_and_person_node_model.md`](2026-04-09_identity_convergence_and_person_node_model.md)
  — current person-node convergence baseline, endpoint binding rules, and resolution provenance model
- [`2026-04-09_graphshell_verse_uri_scheme.md`](2026-04-09_graphshell_verse_uri_scheme.md)
  — canonical `verso://` address space, compatibility aliases, and reserved future categories
- [`2026-02-18_universal_node_content_model.md`](2026-02-18_universal_node_content_model.md)
  — node as persistent content container independent of renderer
- [`../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../verso_docs/technical_architecture/VERSO_AS_PEER.md)
  — current web-peer placement of Servo, Wry, and smallnet servers
- [`../../verso_docs/research/2026-03-28_smolnet_follow_on_audit.md`](../../verso_docs/research/2026-03-28_smolnet_follow_on_audit.md)
  — smallnet protocol audit including §10 Lagrange precedent: document model
  pipeline, Gopher conversion strategy, security posture, and protocol priority order
- [`../research/2026-03-01_webrender_wgpu_renderer_research.md`](../research/2026-03-01_webrender_wgpu_renderer_research.md)
  — WebRender/wgpu rendering research

---

## 1. What MiddleNet Is

**MiddleNet** names the protocol and content space that sits between smallnet and
the full modern web.

| Layer | Protocols / content types | Defining property |
|-------|--------------------------|-------------------|
| **Smallnet** | Gemini, Gopher, Finger, Spartan | Intentionally minimal; no CSS, no JS, text-first |
| **MiddleNet** | RSS/Atom, static HTML, Markdown, simple interactive pages, reader-mode HTTP, smallnet protocols | Structured documents; moderate CSS; light or no JS |
| **Fullnet** | Modern web apps (React, SPAs, authenticated apps) | Full JS runtime, full browser API surface |

MiddleNet is not a new protocol. It is an **observation** about a class of
content that the modern browser treats as a lesser citizen (RSS, Gemini) or
strips down for reader mode, and that intentionally simple protocols like
Gemini deliberately target. A spatial browser that claims smallnet support
implicitly claims MiddleNet support, because the rendering concerns overlap.

MiddleNet content is characterised by:

- Documents and articles over applications
- Structure over interactivity
- Predictable layout over dynamic layout
- Protocols that fit in a single connection without persistent JS runtimes

## 1.1 Current Baseline and Phased Delivery

As of 2026-04-09, Graphshell already ships a meaningful **native desktop
Middlenet lane**, but it does **not** yet ship the extracted portable engine as
its own crate stack.

Current implementation reality:

- `viewer:middlenet` is already a real viewer route on native desktop beside
  `viewer:servo` and `viewer:wry`.
- Protocol-faithful adapters already exist for Gemini/gemtext, Gopher, Finger,
  RSS, Atom, JSON Feed, Markdown, and plain text, with Titan and Misfin helper
  surfaces for mutation and messaging.
- WebFinger, NIP-05, Matrix, and ActivityPub actor resolution already feed a
  person-node model with cached provenance, freshness TTLs, and refresh UI.
- The extracted `graphshell-web-core` / `graphshell-comms` split, browser/PWA
  envelopes, Boa/WIT integration, Wizer snapshotting, and reader-mode HTTP are
  still future phases rather than present repository facts.

This means the doc should be read as a **phased delivery target**:

1. **Phase 0: native baseline**.
   Graphshell continues shipping native desktop Middlenet adapters, viewer
   routing, person-node convergence, and selective Servo/Wry delegation.
2. **Phase 1: extraction boundary**.
   Pull protocol/document adapters and host seams into portable crates without
   yet promising browser-host execution.
3. **Phase 2: portable document engine**.
   Land a first extracted smallnet/document engine for native and controlled
   WASM hosts, starting with protocol-faithful Tier 0/1 content.
4. **Phase 3: browser/mobile envelopes**.
   Add host-aware degradation, browser transport policies, and co-op envelope
   decisions only after the extracted engine boundary is real.

---

## 2. The Portable MiddleNet Engine

The **portable MiddleNet engine** names the target WASM-compilable rendering
crate intended to serve MiddleNet content across all host envelopes. It is the
concrete realisation of the "portable web core" described in the host envelopes
doc, named here for its scope.

### 2.1 Core properties

These are **target properties for phases 2 and 3**, not claims about the
current extracted state of the Graphshell repository.

- **Fully WASM-compilable** — targets `wasm32-wasip2` (native WASM runtime)
  and `wasm32-unknown-unknown` (browser, via WebGPU). Same binary everywhere.
- **Async compositing as architecture** — layout and paint never block on JS.
  CSS renders immediately; JS mutations arrive through a command buffer and are
  applied on the next compositor frame.
- **WASM memory snapshotting** — Wizer pre-initialises the engine (parsed
  default stylesheet, warm Boa context, initialised wgpu pipeline cache) and
  snapshots. Each page instantiates from a CoW copy in ~100μs, not ~50ms cold.
- **WIT component boundary** — JS engine is a separate WASM component. It
  communicates with the DOM via a typed WIT interface. JS engine can be swapped
  (Boa now, Nova later) without touching the renderer.
- **Tiered worlds** — different WIT worlds compile to different binary sizes:
  `middlenet:smallnet` (~3 MB, no JS), `middlenet:document` (~8 MB, basic JS),
  `middlenet:interactive` (~20 MB, fuller DOM surface).

### 2.2 Component stack

```
┌─────────────────────────────────────────────────────┐
│  Host (browser / wasmtime / mobile runtime)          │
│  Provides: wasi-gfx (GPU), wasi-http (network)      │
│                                                       │
│  ┌──────────┐  WIT: dom-mutations  ┌──────────────┐ │
│  │ Boa (JS) │◄────────────────────►│  DOM tree    │ │
│  │ [wasm]   │  command buffer      │              │ │
│  └──────────┘  (batched per frame) │  html5ever   │ │
│                                    │  parses into │ │
│  ┌──────────┐                      └──────┬───────┘ │
│  │wasmtime  │                             │         │
│  │(page wasm│                      ┌──────▼───────┐ │
│  │ support) │                      │    Stylo     │ │
│  └──────────┘                      │  (CSS engine)│ │
│                                    └──────┬───────┘ │
│                                    ┌──────▼───────┐ │
│                                    │    Taffy     │ │
│                                    │   (layout)   │ │
│                                    │  + Parley    │ │
│                                    │   (text)     │ │
│                                    └──────┬───────┘ │
│                                    ┌──────▼───────┐ │
│                                    │  WebRender   │ │
│                                    │  (wgpu back) │ │
│                                    └──────────────┘ │
└─────────────────────────────────────────────────────┘
```

### 2.3 Component crate sources

| Component | Source | WASM status |
|-----------|--------|-------------|
| html5ever | crates.io (servo/html5ever) | ✅ pure Rust |
| Stylo | servo/servo (style crate) | ✅ with rayon disabled |
| Taffy | crates.io | ✅ pure Rust |
| Parley | linebender/parley | ✅ pure Rust |
| WebRender | mark-ik/webrender `wgpu-backend-0.68-minimal` | ✅ wgpu backend (in progress); all 63 shaders translated to WGSL |
| wgpu | crates.io | ✅ WebGPU backend in WASM |
| Boa | crates.io (boa-dev/boa) | ✅ ships `boa_wasm` package (94% Test262) |
| wasmtime | bytecodealliance | ✅ for running page-level WASM content |
| tiny-skia | crates.io | ✅ software rasterizer fallback (SIMD128) |
| reqwest | crates.io | ✅ auto-detects WASM, uses browser Fetch |

The WebRender wgpu backend (mark-ik/webrender) is the critical path item.
When complete, WebRender replaces any lighter-weight paint layer, providing
production-grade compositing: subpixel text, box shadows, blur filters,
gradients, 3D transforms, clip regions — the full Firefox compositor.

---

## 3. Protocol Rendering Strategy

### 3.1 Single intermediate document model

Following Lagrange's architecture, all MiddleNet protocols parse to the same
intermediate document model before rendering. After parsing, the renderer is
format-agnostic — it operates on the DOM tree, not on the source format.

The current repository already implements much of the **protocol-faithful
adapter surface** behind this idea, but it does so inside the native Graphshell
codebase rather than through the fully extracted html5ever/Stylo/Taffy/Boa
engine stack described below.

```
gemini://  → gemtext parser   ─┐
gopher://  → gopher parser    ─┤→ DOM tree + CSS rules → Taffy/Stylo/WebRender
finger://  → plain text       ─┤
spartan:// → Spartan parser   ─┤
nex://     → nex parser       ─┤
misfin:    → misfin parser    ─┤
text/html  → html5ever        ─┘
```

Each protocol parser maps its native semantics to DOM equivalents:

| Protocol | Document model mapping |
|---|---|
| Gemini gemtext | `<h1>`–`<h3>`, `<p>`, `<a>`, `<pre>`, `<ul>` |
| Gopher menu | `<ul>` of `<a>` links + `<pre>` blocks (heuristic); faithful-source toggle renders raw as `<pre>` |
| Finger | `<pre>` plain text body |
| Spartan | Same as Gemini (compatible subset) |
| Nex | `<ul>` of `<a>` links (Gemini-style `=>`) + `<pre>` for files |
| Misfin | `text/gemini` base + additional block types for message metadata |
| RSS/Atom | `<article>` list with `<h2>`, `<p>`, `<time>`, `<a>` |
| JSON Feed | `<article>` list with `<h2>`, `<p>`, `<time>`, `<a>` |
| Markdown | Parsed to equivalent heading/paragraph/link/code blocks |

This means styling (via CSS) and layout (via Taffy/Stylo) work identically
across all protocols. A gemtext `<h1>` and an HTML `<h1>` render through the
same pipeline.

### 3.2 Gopher conversion: heuristic with faithful fallback

Gopher menus are converted to the document model using heuristic detection
(identifying link lines, info lines, preformatted blocks from ASCII art).
Characters that would be misread by the document model are escaped.

A per-node user preference disables conversion and renders the raw gopher
source as monospace preformatted text. This is the same behaviour Lagrange
exposes as "disable Gopher menu styling autodetection."

### 3.3 Faithful render plus optional assistive enrichment

Graphshell's accessibility and protocol policy should be:

- render Gemini, Gopher, gemtext, feeds, Markdown, and other source grammars
  **as themselves**,
- preserve the protocol's authored shape rather than silently replacing it with
  richer proprietary semantics,
- add optional assistive structure **on top** of that faithful render when it
  materially improves orientation, comprehension, or accessibility.

Simple protocols are not inherently less accessible than HTML, but they do
push more responsibility onto the client. For Graphshell that means optional
layers such as:

- heading and outline summaries,
- section/action inventories,
- speech-friendly or low-distraction views,
- alt-text and preformatted-block handling,
- graph-aware provenance and "why is this here?" assistive summaries.

This also preserves the current authored-content boundary:

- **Markdown** remains Graphshell's default inward-facing authored format for
  notes, annotations, and lightweight shared documents.
- **HTML** may become a richer long-term authored/publication surface later,
  but that would be an explicit architectural shift, not an accidental drift
  caused by how browsed content is rendered.

### 3.4 Security preference hierarchy

The engine defaults to the secure protocol where there is meaningful overlap:

- Discovery: WebFinger (HTTPS) preferred over Finger (plaintext)
- Upload/submit: Titan (TLS) preferred over Spartan (plaintext) for equivalent actions
- Modern document: Gemini (TLS) preferred over Spartan for equivalent content

For protocols with no secure analogue (Gopher, Nex, Finger, Guppy): render
faithfully. Show the protocol name and plaintext nature as a neutral
informational indicator — not an alarm. Warn on encryption *failures*
(untrusted or expired certificates), not on encryption *absence*.

### 3.5 Packaging and ownership boundary

The MiddleNet engine owns shared document-model adapters and rendering
semantics. It does **not** own transport realization.

- `graphshell-web-core` owns document/render adapters and the intermediate
  document model.
- `graphshell-comms` or equivalent portable protocol logic owns protocol byte
  parsing/composition.
- Hosts and native mods own raw sockets, TLS sessions, browser APIs, server
  listeners, and other host-specific runtime capabilities.

Default vs optional protocol packaging is governed by
[`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md).

---

## 4. MiddleNet Protocol Coverage

The long-term engine target surface is below. Current Graphshell coverage today
is strongest in Gemini/Gopher/Finger, RSS/Atom/JSON Feed, Markdown/plain text,
WebFinger, and the Titan/Misfin person workflows; Spartan, Nex, Guppy,
reader-mode HTTP, and browser-host execution remain planned.

### Tier 0 — Smallnet (no JS required)

- **Gemini** (`gemini://`) — gemtext; TLS; primary modern secure document lane
- **Gopher** (`gopher://`) — menu/text; heuristic conversion; faithful-source mode
- **Finger** (`finger://`) — plain text profile; WebFinger preferred for identity discovery
- **Titan** — TLS upload extension to Gemini; shared upload dialog with Spartan
- **Misfin** — TLS messaging; `text/gemini` + message line types; social/contact lane
- **Spartan** — plaintext Gemini-adjacent; lightweight request/submit semantics
- **Nex** — plaintext directory/document; Gemini-style link listings
- **Guppy** — UDP plaintext; low priority; compatibility/experiment only

### Tier 1 — Document web (no JS required)

- **RSS / Atom / JSON Feed** — feed parsing and article list rendering
- **Static HTML** — blogs, docs, wikis, forums (HTML + CSS, no JS)
- **Markdown / plain text** — direct rendering
- **`data:` URIs** — inline document rendering

### Tier 2 — Light interactive (Boa JS)

- **Simple HTTP/HTTPS** — static sites with basic JS (navigation menus,
  collapsibles, form validation, `fetch`-based content loading)
- **Reader-mode HTTP** — article extraction from any HTTP page, re-rendered
  with the engine's own layout
- DOM APIs required: `querySelector`, `addEventListener`, `classList`,
  `setAttribute`, `createElement`, `appendChild`, `fetch`, `setTimeout`,
  `requestAnimationFrame`

### Out of scope (Fullnet — host browser handles this)

- Modern SPAs (React, Vue, Svelte)
- Authenticated web apps
- WebGL / Canvas-heavy content
- Streaming media
- Service workers, WebSockets at application level

---

## 4. Host Envelope Roles

The engine's role differs per envelope. **Only the native desktop role exists
today.** The other envelopes are target roles once extraction, host capability
seams, and browser-envelope policy decisions are complete.

### Native desktop

The engine runs as a native library (compiled natively from the same source,
no WASM overhead). It is `viewer:middlenet` in the viewer registry — a third
render mode alongside `viewer:servo` (full web) and `viewer:wry` (OS overlay).

The user can route any node to any viewer. Smallnet URLs are routed to the
MiddleNet engine by default. HTTP URLs can go to Servo (full fidelity), the
MiddleNet engine (reader mode / light interactive), or Wry (OS webview).

### Browser extension

The host browser handles all HTTP/HTTPS rendering natively (it is the webview).
The MiddleNet engine runs as a WASM component inside the extension's service
worker or side panel context. Its role is narrow:

- Render smallnet content (`gemini://`, `gopher://`, `finger://`) in the side
  panel or a controlled tab — the host browser cannot do this
- Render RSS/Atom feeds in the side panel
- Optionally render reader-mode extractions from HTTP pages

Web content opens in native browser tabs. The graph is the navigation memory
and launcher; the host browser is the webview.

### Browser tab / PWA

Same as extension but with weaker host capabilities (no `chrome.tabs`, limited
storage). The engine runs entirely in WASM inside the page. Smallnet and
document rendering work; network requests go through the browser's `fetch`.
Full web rendering is the surrounding browser page itself.

### Mobile (iOS / Android)

The engine runs via a WASM runtime or compiled natively. Platform-provided
webview (WKWebView / Android WebView) handles full web; the engine handles
smallnet and MiddleNet document lanes.

---

## 5. Async Compositing Contract

The engine guarantees that **layout and paint never block on JS**:

```
HTTP response arrives (streaming)
  ↓
html5ever parses incrementally
  ↓
Stylo resolves styles for available nodes
  ↓
Taffy computes layout
  ↓
WebRender paints → wgpu texture              ← user sees content
  ↓  (concurrent, non-blocking)
Boa evaluates <script> tags
  ↓
DOM mutations queued in command buffer
  ↓
Next compositor frame applies mutations → repaint
```

Pages that are CSS-only (Tier 0 and 1) paint at full speed with no JS cost.
JS-enhanced pages (Tier 2) degrade gracefully: content appears, then
interactivity arrives. The user never waits for JS to see the page.

---

## 6. Snapshotting Strategy

Three snapshot levels are useful:

1. **Engine snapshot** — Wizer snapshot of the fully initialised engine
   (parsed UA stylesheet, Boa builtins, wgpu pipeline cache, empty DOM).
   Produced at build time. All page instances CoW-instantiate from this.

2. **Framework snapshot** — Boa context with a specific JS framework
   pre-evaluated (e.g., common polyfills, Preact runtime). Produced on first
   use, cached. Pages using that framework start warm.

3. **Page snapshot** — Full WASM linear memory snapshot of a rendered page
   (DOM, JS heap, layout cache). Used for back/forward navigation. Only the
   dirty pages from the engine snapshot are stored — typically tens of KB for
   document pages.

---

## 7. Relation to the Verso Mod

The existing Verso mod provides `viewer:servo` (Servo + WebRender/GL) and
`viewer:wry` (OS webview overlay). These are native-only, process-coupled, and
cover fullnet.

Today Graphshell operates as a native desktop application with `viewer:middlenet`,
`viewer:servo`, and `viewer:wry` routing inside the same repository. The
portable MiddleNet engine described here is therefore still an extraction and
host-portability project, not a completed subsystem boundary.

The MiddleNet engine is a separate concern:

| Property | Verso (Servo/Wry) | MiddleNet engine |
|----------|-------------------|---------------|
| Target | Fullnet (HTTP/HTTPS, full JS) | MiddleNet (smallnet + document + light interactive) |
| WASM-compilable | No | Yes |
| JS engine | SpiderMonkey (full) | Boa (interpreter, 94% Test262) |
| CSS engine | Stylo (full, parallel) | Stylo (single-threaded) |
| Renderer | WebRender / GL | WebRender / wgpu |
| Host envelopes | Native desktop only | All envelopes |
| Scope | Browser-grade web fidelity | Document-grade MiddleNet fidelity |

They are complementary. On native desktop, both are available; the viewer
registry routes content to the appropriate engine by URL scheme and content
type. In extension/PWA/mobile envelopes, only the MiddleNet engine is available.

---

## 8. Open Questions

1. **Final crate name** — `graphshell-web-core`, `middlenet-engine`, or a name
   under the `graphshell-core` umbrella (per the core extraction plan). The
   architecture is the same regardless.
2. **WebRender wgpu backend readiness** — the render pass wiring (alpha/opaque/
   composite dispatch) and WASM surface integration are the critical path.
   Until complete, Blitz or a simpler wgpu paint layer can fill the gap for
   Tier 0/1 content.
3. **Smallnet server integration** — Gemini/Gopher/Finger servers currently
   live in the Verso native mod. Whether these move into the portable engine
   (client-only, server stays native) or stay in Verso is an open placement
   question.
4. **Reader-mode extraction** — Tier 2 HTTP reader mode requires a content
   extraction step (similar to Firefox Reader View / Readability.js). This
   should be a Rust implementation inside the engine, not a JS library, to
   keep the extraction WASM-portable and JS-free.

---

*The MiddleNet engine is the portable answer to "what does Graphshell render
when it has no OS webview and no full browser engine?" The current codebase
already covers much of the protocol-faithful document lane natively; the
remaining work is to extract, phase, and host that lane without giving up
faithful render or the Markdown-vs-HTML authored-content boundary.*
