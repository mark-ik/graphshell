<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# MiddleNet and the Portable MiddleNet Engine

**Date**: 2026-03-29
**Status**: Design note — architectural position, not a build plan
**Scope**: Define the MiddleNet protocol space, name the portable WASM rendering
engine that serves it, and describe how it fits into each host envelope.

**Related docs**:

- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
  — host envelope model, portability strategy, and naming context for the
  portable web core
- [`2026-03-29_workspace_restructuring_plan.md`](2026-03-29_workspace_restructuring_plan.md)
  — Cargo workspace layout: crate responsibilities, dependency graph, migration steps
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

---

## 2. The Portable MiddleNet Engine

The **portable MiddleNet engine** is the WASM-compilable rendering crate that
serves MiddleNet content across all host envelopes. It is the concrete realisation
of the "portable web core" described in the host envelopes doc, named here for
its scope.

### 2.1 Core properties

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

### 3.3 Security preference hierarchy

The engine defaults to the secure protocol where there is meaningful overlap:

- Discovery: WebFinger (HTTPS) preferred over Finger (plaintext)
- Upload/submit: Titan (TLS) preferred over Spartan (plaintext) for equivalent actions
- Modern document: Gemini (TLS) preferred over Spartan for equivalent content

For protocols with no secure analogue (Gopher, Nex, Finger, Guppy): render
faithfully. Show the protocol name and plaintext nature as a neutral
informational indicator — not an alarm. Warn on encryption *failures*
(untrusted or expired certificates), not on encryption *absence*.

---

## 4. MiddleNet Protocol Coverage

The engine targets this protocol surface, in priority order:

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

- **RSS / Atom** — feed parsing and article list rendering
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

The engine's role differs per envelope. In all cases, the same WASM binary is
used; the host envelope determines which capabilities it is granted.

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

*The MiddleNet engine is the portable answer to "what does Graphshell render when
it has no OS webview and no full browser engine?" — smallnet protocols,
documents, feeds, and lightly interactive pages, rendered at document fidelity,
everywhere Graphshell runs.*
