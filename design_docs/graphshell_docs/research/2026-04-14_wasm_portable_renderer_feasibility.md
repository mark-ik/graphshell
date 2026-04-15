<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Feasibility: Fully-WASM Portable Web Renderer

**Date**: 2026-04-14
**Status**: Research / feasibility analysis
**Scope**: Can Graphshell's rendering stack compile to WASM and run portably across
browser, desktop, and mobile runtimes? What does the stack look like, how far can it
go, and what novel architectural properties does the WASM-first design enable?

**Related**:

- [`../implementation_strategy/aspect_render/render_backend_contract_spec.md`](../implementation_strategy/aspect_render/render_backend_contract_spec.md) — backend contract; §12-13 cover RenderingBackendBinding and WgpuHal
- [`../implementation_strategy/aspect_render/2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md`](../implementation_strategy/aspect_render/2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md) — wgpu-gui-bridge interop architecture
- [`2026-03-01_webrender_wgpu_renderer_research.md`](2026-03-01_webrender_wgpu_renderer_research.md) — WebRender wgpu backend research
- [`2026-03-01_servo_script_engine_alternatives.md`](2026-03-01_servo_script_engine_alternatives.md) — JS engine alternatives (Boa, Nova)

---

## 1. Context

The earlier "Lightview" analysis explored a native-only lightweight renderer. This
document extends that analysis to the fully-WASM portable path: instead of a
native-only lightweight renderer, the entire engine compiles to WASM and runs
portably on browsers (via WebGPU), desktop WASM runtimes (wasmtime + wasi-gfx),
and mobile. The JS runtime is acknowledged as slow today — this analysis covers
how far the stack can go and what the JS performance trajectory looks like.

---

## 2. Revised Stack

The proposed stack, updated to reflect what actually compiles to WASM:

```
┌──────────────────────────────────────────────────┐
│  Host (browser / wasmtime / mobile runtime)       │
│  Provides: wasi-gfx (GPU), wasi-http (network)   │
├──────────────────────────────────────────────────┤
│                                                    │
│  ┌────────────┐  WIT: dom-mutations  ┌─────────┐ │
│  │ Boa (JS)   │◄────────────────────►│  DOM     │ │
│  │ [wasm]     │  command buffer      │  Tree    │ │
│  └────────────┘                      │          │ │
│                                      │  html5ever│ │
│  ┌────────────┐                      │  parses  │ │
│  │ wasmtime   │                      │  into it │ │
│  │ (page WASM)│                      └────┬─────┘ │
│  └────────────┘                           │       │
│                                      ┌────▼─────┐ │
│                                      │  Stylo   │ │
│                                      │  (CSS)   │ │
│                                      │  -rayon  │ │
│                                      └────┬─────┘ │
│                                      ┌────▼─────┐ │
│                                      │  Taffy   │ │
│                                      │  (layout)│ │
│                                      │ +parley  │ │
│                                      └────┬─────┘ │
│                                      ┌────▼─────┐ │
│                                      │  wgpu    │ │
│                                      │  (paint) │ │
│                                      └──────────┘ │
└──────────────────────────────────────────────────┘
```

---

## 3. Component WASM-Compilation Status

| Component | Compiles to WASM? | Notes |
|-----------|:-:|-------|
| **reqwest** | ✅ | Auto-detects WASM target; uses browser Fetch API underneath |
| **html5ever** | ✅ | Pure Rust, callback-based TreeSink. Zero issues. |
| **Stylo** | ⚠️ | Compiles with small patch. Must disable rayon (or use wasm-bindgen-rayon + SharedArrayBuffer). Single-threaded is fine for light content. |
| **Taffy** | ✅ | Pure Rust. Flexbox, Grid, Block, Table layout. |
| **Parley** | ✅ | Pure Rust text shaping/layout. |
| **Boa** | ✅ | Already ships `boa_wasm` npm package. Interpreter-only. |
| **wgpu** | ✅ | First-class WASM support → WebGPU backend. Needs `--cfg=web_sys_unstable_apis`. |
| **wasmtime** | ✅ | For running page-level WASM (WebAssembly in web pages). |
| **tiny-skia** | ✅ | Software rasterizer fallback. SIMD128 optimized for WASM. |
| **Nova** | ❌ | WASM support listed as "planned," not implemented. |
| **WebRender** | ✅ (if wgpu branch lands) | All 63 shaders translated to WGSL, `GpuDevice` trait abstracted, end-to-end render+readback proven. The GL hardcoding is removed in the fork. |
| **Servo layout** | ❌ | Too coupled to SpiderMonkey and threading. Use Taffy. |

### Substitutions from the original proposal

- ~~WebRender GL-backend~~ → **WebRender + wgpu backend** (removes GL hardcoding)
- ~~Servo's layout~~ → **Taffy** (handles flexbox, grid, block, table, absolute, fixed)
- ~~Nova~~ → **Boa** (for now; Nova when ready)
- Stylo → **Stylo, single-threaded** (disable rayon feature)

---

## 4. The GPU Question

The GPU path is solved. The wgpu-backend experimental branch has:

- `GpuDevice` trait fully extracted from WebRender's GL device
- All 63 shader variants translated to WGSL via a 2,139-line translation pipeline
  (`webrender_build/src/wgsl.rs`) using naga's GLSL frontend with 16 preprocessing
  transforms
- All WGSL shaders compiled into `wgpu::RenderPipeline` at device init time
- Bind group layouts matching WebRender's full resource model (12 texture slots +
  3 uniform buffers + global sampler)
- Vertex/index buffer management, render pass encoding
- Pixel readback via staging buffer
- End-to-end rendering proven: solid color quad + textured quad, pixel-level verified tests

What wgpu provides across targets:

- **Browser**: wgpu → WebGPU (direct, zero overhead, first-class)
- **Desktop WASM** (wasmtime): wgpu → wasi-gfx → Vulkan/Metal/DX12
- **Mobile**: wgpu → wasi-gfx → platform GPU
- **No GPU**: tiny-skia software rasterizer → canvas putImageData

The rendering pipeline does not need to escape WASM for GPU access.

### Remaining wgpu backend work

- Wire WebRender's actual render passes (alpha, opaque, composite) through `GpuDevice`
  — current draw paths are debug helpers proving the pipeline works; the
  batching/dispatch loop is the remaining work
- Surface integration for non-headless rendering (headless is proven)
- WASM-specific: `--cfg=web_sys_unstable_apis`, swap `pollster` for a WASM-compatible
  async executor on the init path

---

## 5. The Stack With WebRender

With the wgpu fork, the paint layer becomes **WebRender itself**:

```
html5ever  →  DOM tree
Stylo      →  computed styles (Firefox's CSS engine)
Taffy      →  box layout (flexbox, grid, block, table, abs/fixed)
WebRender  →  display list → GPU (wgpu fork → WebGPU in WASM)
Boa        →  JS mutations via command buffer (async, non-blocking)
```

This is the Firefox rendering pipeline minus SpiderMonkey, portable to WASM. WebRender's display list handles:

- Subpixel text AA with glyph cache, harfbuzz shaping
- Box shadows, CSS `filter: blur()`, opacity layers
- Rounded corners, borders (including border-image)
- Image compositing with correct blend modes
- Linear/radial/conic gradients (GPU-side)
- Clip regions, masks, 3D transforms, `perspective()`
- Scrolling frame compositing with GPU-side culling
- Display list batching and Z-ordering

Blitz's paint layer cannot do any of that. The stack therefore uses WebRender, not Blitz,
once the GL hardcoding is removed.

### Path to completion

1. Complete the wgpu backend's render pass wiring (alpha/opaque/composite passes)
2. Swap `pollster` for a WASM-compatible async executor on init path
3. Add surface presentation for non-headless (canvas integration)
4. Compile Stylo + Taffy + WebRender-wgpu to wasm32 (rayon off for Stylo)
5. Wire Boa via WIT command buffer for JS mutations
6. Expose as a WASM component

---

## 6. WIT DOM Boundary

No standard `web:dom` WIT interface exists. The required interface:

```wit
// graphshell:dom — the mutation interface
interface dom-mutations {
    // Batch operations for performance (WIT calls have μs overhead)
    record dom-op {
        kind: op-kind,
        target: node-id,
        // ... payload variants
    }

    enum op-kind {
        create-element,
        create-text,
        set-attribute,
        remove-attribute,
        append-child,
        remove-child,
        set-text-content,
        set-style-property,
    }

    // JS engine flushes a batch of mutations per microtask checkpoint
    apply-mutations: func(ops: list<dom-op>) -> result<list<node-id>, error>

    // Query interface (JS reading DOM state)
    query-selector: func(selector: string) -> option<node-id>
    get-attribute: func(node: node-id, name: string) -> option<string>
    get-computed-style: func(node: node-id, prop: string) -> string
}
```

**Why a command buffer, not direct calls**: WIT cross-component calls cost microseconds
each. A React render can produce hundreds of DOM mutations per frame. Batching into
`apply-mutations(ops)` amortizes the boundary crossing and aligns naturally with the
async compositing model.

---

## 7. Feasibility by Level

### Level 1: Static HTML/CSS — Fully feasible now

- html5ever + Stylo + Taffy + wgpu
- No JS at all. Renders articles, docs, RSS, structured content.
- Stylo gives Firefox-grade CSS: selectors, cascade, specificity, custom properties,
  `calc()`, media queries, pseudo-elements.
- This alone is a useful product for a spatial browser.

### Level 2: Basic JS interactivity — Feasible, 3-6 months

- Add Boa via WIT command buffer
- Implement ~50 core DOM APIs
- Add `fetch()`, `setTimeout`, `requestAnimationFrame`
- Handles: static sites with dropdowns/menus, basic SPAs, progressive enhancement,
  form validation

### Level 3: Modern interactive web — Feasible, 12-18 months

- Expand DOM API surface to ~200 APIs
- Add: `IntersectionObserver`, `MutationObserver`, `ResizeObserver`, `CustomEvent`,
  `history.pushState`, `localStorage`, `URL`, `FormData`, `TextEncoder/Decoder`,
  `AbortController`
- CSS transitions + animations (Stylo computes them; need to wire timing)
- Canvas 2D API (render to wgpu texture)
- Handles: most content sites, blogs, documentation, dashboards

### Level 4: Web app compatibility — Hard, 2-3 years

- React/Vue/Svelte apps (they hammer the DOM API surface)
- Full `Event` model, `Range`/`Selection`, `contenteditable`, `<input>` elements
- WebSocket, Server-Sent Events, Service Workers
- This is where competition with Servo/Chromium begins and the long tail hurts

### Level 5: Full web compat — Not the goal

- The value proposition is Levels 1-3 in a portable WASM binary with instant boot.

---

## 8. JS Performance Trajectory

JS in Boa in WASM is ~50-100× slower than V8 today. But:

1. WASM itself is getting faster: relaxed SIMD, tail calls, exception handling, GC
   proposal, stack switching — each closes the native gap
2. Boa is getting a JIT (roadmap item) — even a baseline JIT gives ~5-10× speedup
3. Nova + Cranelift would bring near-native JIT quality to a WASM target
4. The async compositing architecture means JS speed does not block rendering — the
   page paints immediately from CSS; JS only affects interactivity latency
5. For Levels 1-2, JS speed barely matters

The critical performance path is the rendering/CSS/layout pipeline — all Rust compiled
to WASM (~0.7-0.9× native speed). JS is the slow part, but it is async and the
ecosystem will improve it.

---

## 9. WASM Snapshotting

With the fully-WASM architecture, snapshotting enables several optimizations:

**Snapshot the entire renderer initialization:**

1. Compile the engine to WASM
2. Run initialization: parse default stylesheet, create empty DOM, warm Boa builtins,
   initialize wgpu pipeline caches
3. Wizer snapshots → pre-initialized `.wasm` binary
4. Each page load: CoW-instantiate from snapshot (~100μs), not cold-start (~50ms)

**Snapshot common frameworks**: Pre-initialize a Boa context with React's runtime
already parsed and snapshot it. Pages using React start from a warm context.

**Back/forward navigation**: Snapshot the WASM linear memory (DOM state, JS heap,
layout cache) on navigation-away; restore from snapshot on navigation-back. Faster
than any browser's back/forward cache because there is no serialization/deserialization
— just remap virtual memory pages.

---

## 10. Architectural Innovations

The fully-WASM component architecture enables properties structurally impossible in
traditional browser engines:

### 10.1 Massively Parallel Page Rendering from CoW Snapshots

In a spatial browser, dozens of nodes are visible simultaneously. With WASM + Wizer
snapshots + CoW instantiation:

- Instantiate 20 renderer instances from the same base snapshot
- Each gets its own copy-on-write linear memory (shared pages until mutated)
- Each renders a different node's content on its own Web Worker / OS thread
- Memory overhead: proportional to what each page *changes*, not total size
- Boot cost per instance: ~100μs, not ~50ms

### 10.2 Speculative Pre-Rendering from Graph Proximity

The force-directed graph provides a spatial model of likely next navigations. Nodes
near the cursor, nodes connected to the focused node — these are high-probability next
navigations.

- Speculatively instantiate renderer instances (from snapshot: cheap) for nearby nodes
- Fetch their HTML in the background
- Parse + layout + first paint into an offscreen wgpu texture
- When the user navigates → texture is already ready, swap it in

Browsers have `<link rel="prerender">` but it is a hint with no spatial signal. Here,
the graph topology is the prediction model. And because instances are cheap (CoW
snapshots), pre-rendering 5-10 nearby nodes has minimal memory cost.

### 10.3 Tiered WIT Worlds (Compile-Time API Tree Shaking)

Not every page needs the full engine. WIT worlds define minimal API surfaces:

```
world rss-viewer {          // ~3MB WASM, no JS at all
    import wasi:http/fetch
    export render: func(html: string) -> texture
}

world article-reader {      // ~8MB WASM, basic JS
    import wasi:http/fetch
    import graphshell:dom/query
    export render: func(url: string) -> texture
}

world full-web {            // ~20MB WASM, everything
    import wasi:http/fetch
    import wasi:gfx/webgpu
    import graphshell:dom/mutations
    import graphshell:dom/events
    export render: func(url: string) -> texture
}
```

Benefits: smaller binaries for simple content, reduced attack surface, graceful
degradation. This is impossible in monolithic engines.

### 10.4 Capability-Attenuated Security via WIT

The WIT boundary is a **capability boundary**. A renderer component literally cannot
access the network, filesystem, or GPU unless the host grants those capabilities.
This is enforced by the WASM runtime at the instruction level, not by
application-level policy checks. For a spatial browser showing untrusted web content
this is stronger isolation than any browser ships today, with less engineering effort.

### 10.5 Deterministic Replay and Bug Reproduction

WASM execution is deterministic given the same inputs:

- **Record**: log all host→guest inputs (network responses, timer firings, user events,
  GPU frame acks)
- **Replay**: feed the same inputs → get identical rendering output
- **Bug reports**: ship the input recording, not steps to reproduce
- **Regression testing**: replay recordings against new engine versions, diff output
  textures pixel-by-pixel

Traditional browsers cannot do this because of non-deterministic threading, memory
allocation, and JIT compilation.

### 10.6 Structural Sharing for Navigation Snapshots

On navigation-away, snapshot the instance's linear memory. On navigation-back, CoW-
restore from the snapshot. Because wasmtime uses virtual memory pages, the snapshot
is just the set of dirty pages — an article that rendered 2MB of DOM but only had
50KB of JS mutations stores a ~50KB delta from the base snapshot.

### 10.7 Shared Immutable Resources Across Instances

When 20 renderer instances are running, they often reference the same resources:
common fonts, CSS resets, framework code. The host can deduplicate at the
content-address level — first instance fetches a font, host stores by hash, second
instance requesting the same font gets the same read-only memory page. Traditional
browsers do this at the HTTP cache level but still parse/decode per-process. Here,
the *parsed* representation can be shared read-only.

### 10.8 Progressive Rendering with Layout Commitments

Combine streaming HTML parsing with a layout stability contract:

- html5ever parses the first `<section>` → Taffy lays it out → wgpu paints it
- More HTML arrives → incremental layout for new content
- The engine *commits* to not reflowing already-painted regions unless JS explicitly
  mutates them

This eliminates Cumulative Layout Shift by construction for static content. JS
mutations can still cause reflow, but they go through the async command buffer — the
compositor can choose to animate the transition rather than jump.

---

## 11. Graphshell Integration

For Graphshell specifically, this becomes a `viewer:lightrender` backend that renders
web content without requiring Servo or a platform WebView. Nodes in the graph that
link to articles, docs, RSS — they render natively in the graph compositor via wgpu
texture handoff. No IPC, no child process, no GL state save/restore.

The target binary is a ~15-30MB WASM binary that:

- Runs in any browser (via WebGPU)
- Runs on desktop (via wasmtime + wasi-gfx)
- Runs on mobile (via wasm runtime)
- Renders HTML/CSS at Firefox-grade fidelity (Stylo)
- Handles basic-to-moderate JS interactivity
- Boots in <1ms from snapshot
- Paints before JS evaluates (async compositing)
- Can be embedded in any application as a WASM component

---

## 12. Honest Assessment

**How far can this go?** Farther than the earlier Lightview analysis suggested. Key
upgrades from that analysis:

1. **GPU is solved** — wgpu→WebGPU from WASM is production-ready
2. **Stylo in WASM works** — Firefox-grade CSS, not Blitz's subset
3. **Blitz already assembles most of the stack** — less greenfield than it seemed
4. **wasi-gfx provides the native GPU bridge** — desktop/mobile WASM gets real GPU
5. **The WIT component model provides clean module boundaries** — wasmtime supports it
   today; the web path uses wasm-bindgen

**Realistic ceiling**: Level 3 (modern interactive web) is achievable in 12-18 months
for a focused effort. That is articles, blogs, docs, dashboards, simple SPAs — a
large fraction of all pages on the web.

**The novel contribution** is not "yet another browser engine." It is:

- Portable WASM binary — same engine everywhere
- Instant boot via memory snapshotting — no cold start
- Async compositing as architecture — CSS always paints, JS catches up
- Componentized via WIT — swap JS engines, add renderers, without touching core
- Embeddable — a rendering component for applications like Graphshell, not a
  standalone browser

This is architecturally distinct from Servo/Chromium/Ladybird in ways that matter for
the spatial browser use case.
