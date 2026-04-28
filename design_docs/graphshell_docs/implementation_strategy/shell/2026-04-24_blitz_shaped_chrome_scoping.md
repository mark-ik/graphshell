<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Blitz-Shaped Chrome Scoping (2026-04-24)

**Status**: Scoping pass — no implementation committed
**Lane**: Long-horizon alternative to the iced chrome track; informs
when/if to abandon iced for an HTML/CSS-rendered chrome stack.

**2026-04-28 update — premise shift**: Iced reached wgpu 29 parity by
vendoring (see [GPUI plan progress entry](2026-04-27_gpui_host_integration_plan.md#progress)
and [iced migration plan progress entry](2026-04-14_iced_host_migration_execution_plan.md#10-progress-log)).
The wgpu version split that this doc treats as a permanent
architectural cost (§1, §3.5, §5 comparison table) **no longer
exists**. "Removing iced *also removes the version split*" is no
longer a unique benefit of Blitz-shaped — that win is already
captured in iced.

This doc's case for Blitz-shaped therefore reduces to the
*non-GPU* arguments: HTML/CSS as the source of truth for chrome,
Firefox-grade selectors via Stylo, and architectural alignment
with Servo's own renderer model. Those are still substantive but
they're a chrome-semantics case, not a GPU-stack-alignment case.
Any future activation of this plan should re-anchor §1 on those
arguments rather than the wgpu split. Decision criteria in §8
remain valid — the "wgpu version split causes a real, blocking
problem" trigger is now defensively unlikely to fire.

**Context**: The 2026-04-24 [renderer-boot research](../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md)
turned up two architectural surprises that shift the cost/benefit
of replacing iced:

- iced 0.14 is closed around `wgpu::Device` (no public accessor, no
  injection hook).
- iced uses wgpu 27 while Servo's fork is wgpu-29-era; the two
  cannot share `wgpu::Texture` / `wgpu::Device` types directly.

These together mean iced will **always** be architecturally awkward
for a Servo-hosting application — not just temporarily until iced
catches up. That makes a chrome-renderer alternative more attractive
than it appeared at the
[2026-04-16 vision doc](../../research/2026-04-16_rendering_architecture_vision.md).

This doc fleshes out **scope #2** from the 2026-04-24 question
"how difficult would Blitz-shaped chrome be?" — specifically the
"we own the assembly" variant that wires Stylo + Taffy + Parley +
our WebRender fork directly, without depending on the upstream
Blitz crate or DioxusLabs's `anyrender` abstraction.

**Related**:

- [Rendering Architecture Vision: Blitz + anyrender + WebRender](../../research/2026-04-16_rendering_architecture_vision.md)
  — original framing; this doc is its concrete follow-on.
- [WASM portable renderer feasibility](../../research/2026-04-14_wasm_portable_renderer_feasibility.md)
  — proves Stylo + Taffy + WebRender-wgpu compose; same components,
  different target (chrome vs portable web).
- [Middlenet lane architecture spec](../../technical_architecture/2026-04-16_middlenet_lane_architecture_spec.md)
  — already names blitz-dom + blitz-html + Stylo + Taffy + Parley
  + WebRender for the HTML middlenet lane. Scope #2 overlaps this
  stack with chrome instead of content.
- [Iced host migration execution plan](2026-04-14_iced_host_migration_execution_plan.md)
- [Iced content-surface scoping](2026-04-24_iced_content_surface_scoping.md)
- [WebRender wgpu renderer research](../../research/2026-03-01_webrender_wgpu_renderer_research.md)

---

## 1. Scope of "Option 2"

| Option | Description | Time | Decision risk |
|---|---|---|---|
| 1 — Adopt Blitz proper | Use `blitz-dom` + `blitz-html` + `anyrender-webrender` (which doesn't exist) for chrome | 3–6 months | Blitz API unstable, we don't control timing |
| **2 — Blitz-shaped (we own the assembly)** | **Wire Stylo + Taffy + Parley + WebRender ourselves; use `blitz-dom` if convenient, but no `anyrender` indirection** | **2–3 months** | **Integration risk we control** |
| 3 — Blitz only for HTML content | Add Blitz inside the middlenet HTML lane, keep iced chrome | ~1 month | Smallest commitment; doesn't unify GPU |

**This doc covers option 2.** The product framing: build a custom
HTML/CSS-rendered chrome that paints into the same WebRender our
graph canvas and (eventually) Servo content paint into. Replace
iced's role as the chrome host. Keep iced files in the tree only as
long as needed for graceful overlap.

**Platform-tier scope** (per the
[2026-04-24 iced content-surface scoping §0](2026-04-24_iced_content_surface_scoping.md#0-platform-tier-framing-added-2026-04-24)):
Blitz-shaped chrome is **native-desktop-only** (Windows, macOS,
Linux). Mobile / web targets continue to use wry (system WebView)
for fullnet content and middlenet for lighter content. The
chrome rendering decision (iced vs Blitz-shaped) is therefore a
**desktop architecture decision**; no other platform is affected.

**Key reframing from 2026-04-24 research**: the wgpu version split
isn't a permanent architectural cost — it's specifically caused by
**iced 0.14 being on wgpu 27** while everything else in the
graphshell ecosystem is already on **wgpu 29**:

| Crate | wgpu version |
|---|---|
| `webrender-wgpu` (our fork) | **29** |
| `servo-wgpu` (our Servo fork) | **29** |
| `egui-wgpu` 0.34.1 (graphshell main) | **29** |
| `iced` 0.14 | **27** ← outlier |

Removing iced *also removes the version split*. The Blitz-shaped
plan delivers wgpu unification by retiring the one component
that's out of alignment, not by uplifting everything else to
match it.

The sub-question that determines difficulty: **how much of
upstream Blitz do we adopt vs. how much do we hand-assemble?**
The answer here is "use Blitz's DOM tree where it's load-bearing
(`blitz-dom`); skip Blitz's paint and event layers; assemble our
own from Stylo + Taffy + Parley + WebRender."

---

## 2. Component Inventory

### 2.1 Components we'd consume

| Component | Source | License | Notes |
|---|---|---|---|
| **html5ever** | `servo/html5ever` (crates.io) | MPL/Apache | HTML parser, already a transitive dep via Servo. |
| **Stylo** | `servo-style` from our Servo fork | MPL | CSS engine. **Already in workspace** via `servo` dep. Disable rayon for simplicity (single-threaded chrome layout is fine; document layout volume is small). |
| **Taffy** | `taffy` (crates.io) | MIT | Layout (flexbox + grid + block + table + abs/fixed). Pure Rust, no patches needed. |
| **Parley** | `linebender/parley` (crates.io) | Apache/MIT | Text shaping + line breaking. Same dep we already use indirectly through Vello-related work in `graph-canvas`. |
| **WebRender** | Our `webrender-wgpu` fork | MPL | GPU display-list compositor. Already pinned in `Cargo.toml` patches. |
| **`blitz-dom`** | `DioxusLabs/blitz` (probably git, not crates.io) | Apache/MIT | DOM tree representation Blitz uses internally. Optional — we could roll our own if it's not stable enough, but it's a natural anchor for "we use what Blitz already assembled here." |

### 2.2 Components we explicitly skip

| Component | Why skip |
|---|---|
| `blitz-html` | Convenience facade for blitz-dom + html5ever + Stylo. We'd inline its assembly to keep dependencies under control. |
| `anyrender` (DioxusLabs trait crate) | Two render targets only matters if we want backend pluggability. We commit to WebRender for chrome paint; skipping `anyrender` removes one trait hop. Vello-via-WebRender is already an option through WebRender's primitives. |
| Blitz's event layer | Blitz's event handling is geared toward rendering web pages, not driving native chrome. We'd implement event routing tailored to graphshell's `Message`-style state model directly. |
| Iced (post-cutover) | Once Blitz-shaped chrome reaches feature parity, iced retires. Until then, iced + Blitz-shaped overlap (chosen at startup like `--iced` vs `--blitz`). |

### 2.3 Components we already own (kept as-is)

- `graphshell-core` — portable shell state (FrameViewModel, FrameHostInput, HostIntent). Survives unchanged.
- `graphshell-runtime` — runtime kernel. Survives unchanged.
- `graph-canvas` — graph rendering. Survives unchanged; paints into the same WebRender that the chrome paints into.
- `graph-tree` — workbench/navigator. Survives unchanged.
- `HostPorts` traits + sanctioned-writes contracts. Survive; the new Blitz-shaped host implements `HostPorts` the same way iced does.

The portable crate boundary we built for the iced migration **is the reason this is feasible**. None of the cross-cutting work (intent routing, view-model projection, host-neutral runtime) is wasted. Only the host adapter changes.

---

## 3. Architectural Design

### 3.1 Stack diagram

```
┌──────────────────────────────────────────────────────────────────┐
│ graphshell-app (new shell host crate)                            │
│   - main loop (winit-driven; replaces iced::application)         │
│   - state container (FrameViewModel + Blitz DOM root)            │
│   - event router (winit::WindowEvent → HostEvent / DOM event)    │
│   - HostPorts impl (BlitzHostPorts)                              │
└────────────┬─────────────────────────────────┬───────────────────┘
             │                                 │
             ▼                                 ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ Chrome via Blitz-shaped      │  │ Graph canvas via graph-canvas │
│   - HTML + CSS source         │  │   - ProjectedScene<NodeKey>   │
│   - blitz-dom tree            │  │   - canvas_bridge derive      │
│   - Stylo computed styles     │  │                               │
│   - Taffy layout              │  │                               │
│   - Parley text shaping       │  │                               │
│   - WebRender display list    │  │   - WebRender display list    │
└──────────────┬───────────────┘  └─────────────┬─────────────────┘
               │                                │
               └────────────┬───────────────────┘
                            ▼
              ┌──────────────────────────────┐
              │ WebRender (one instance)     │
              │   - one wgpu::Device         │
              │   - one render thread        │
              └────────────┬─────────────────┘
                           ▼
                    one wgpu::Surface
```

One WebRender, one wgpu device, two display-list producers (chrome + graph canvas). Future: Servo content composes in too — same pipeline, no version split.

### 3.2 Chrome model — HTML/CSS as the source of truth

Today's iced `view()` is a Rust function that returns an `Element`. Tomorrow's Blitz-shaped equivalent would be:

- **A static HTML document** loaded at startup that defines the chrome shape (toolbar, sidebar, status bar, toast container, modal slots). Looks like:
  ```html
  <body class="graphshell-chrome">
    <header class="toolbar">
      <input id="location" type="text" placeholder="Enter URL…">
      <span class="nav-hint" data-back="true" data-fwd="false"></span>
    </header>
    <main class="canvas-host"><!-- graph canvas mounts here --></main>
    <aside class="toast-stack" id="toasts"></aside>
  </body>
  ```
- **A stylesheet** that takes the lion's share of design decisions — themed via CSS custom properties.
- **Runtime DOM updates** driven by `FrameViewModel`: a small "view-model → DOM diff" routine each frame updates text content, attribute states, and class lists. Conceptually similar to a tiny React reconciler, but Rust-side and not user-extensible.

The "tiny reconciler" is the load-bearing piece. It's what makes the chrome reactive without a full virtual-DOM library.

### 3.3 Event flow

Event routing is the area where this scope rolls its own work hardest. Current design:

1. **winit emits a window event** (key, mouse, resize…).
2. **Hit-test against Taffy-laid DOM**: which DOM element does this pointer event land on? (Taffy returns rectangles; we walk the tree to find the deepest match.)
3. **Two paths**:
   - If the hit element is the `canvas-host`, dispatch into `graph-canvas`'s existing event path (same code we landed for iced's `canvas::Program::update`).
   - Otherwise, the hit element gets a synthetic event sent to a registered Rust handler. Handlers map (element_id, event_kind) → `Message` (or `HostIntent`). Looks like:
     ```rust
     event_router.on("#location", "submit", |value| {
         Message::LocationSubmitted(value.into_string())
     });
     ```
4. **`Message` → `update`** runs the same way iced's update does.
5. **Update populates `FrameHostInput.host_intents`** if needed; runtime tick runs.
6. **View-model → DOM-diff** updates the chrome shape next frame.

Crucially, this preserves the `HostIntent` contract we just landed. Blitz-shaped is a different chrome **shape**, not a different mutation **path**.

### 3.4 Painting

Each frame:

1. **Layout pass**: Taffy walks the DOM, asks Stylo for computed styles for each node, computes a layout tree.
2. **Display-list build**: walk the laid-out tree; for each node emit WebRender display-list items (rect, text run via Parley, image, etc.).
3. **Submit to WebRender**: the same WebRender instance accepts the chrome's display list and the graph canvas's display list as separate pipelines, composed by WebRender at GPU time.
4. **Present**: one swap, one frame, one wgpu device.

This is the same architecture Servo uses internally — we're just doing it for our chrome instead of for HTML content (it's the same problem, smaller scope).

### 3.5 What survives from the iced work

The portable contract layer:

- `graphshell-core` types (`FrameViewModel`, `FrameHostInput`, `HostIntent`, `HostEvent`, `OverlayStrokePass`, …) — unchanged.
- `graphshell-runtime` kernel — unchanged.
- `graph-canvas` and `graph-tree` crates — unchanged (graph canvas now paints into WebRender via a new adapter instead of via `iced::widget::canvas`, but the portable derive pipeline is identical).
- `HostPorts` trait suite — unchanged. New impls land for the Blitz-shaped host.
- Sanctioned-writes contract tests — unchanged. Adds `BlitzHost` files to the host-adapter allowlist.

What gets retired (after parity):

- `shell/desktop/ui/iced_app.rs`, `iced_host.rs`, `iced_host_ports.rs`, `iced_canvas_painter.rs`, `iced_graph_canvas.rs`, `iced_events.rs` (5 files; ~1500 lines).
- `iced` and `iced_*` crate deps.
- The wgpu version split — gone, since both chrome and Servo eventually share WebRender.

What gets retired (immediately, even before Blitz-shaped lands):

- Nothing forced. Iced and Blitz-shaped can coexist behind feature flags during overlap.

---

## 4. Sliced Execution Plan

Eight slices, ordered to keep the project shippable at every checkpoint. Estimates assume one developer with AI assistance, comparable cadence to the iced work landed through 2026-04-24.

### Phase 0 — Prerequisites (research, **mostly done 2026-04-24**)

- **B0.1** ~~Audit Stylo's standalone usability outside Servo's full
  pipeline.~~ **Researched 2026-04-24** (parallel-agent survey).
  Findings:
  - Stylo lives at <https://github.com/servo/stylo>, pulled into our
    `servo-wgpu` fork via git dep at commit
    `a556f4cbd15fc289039261661b049a5dc845cd80`.
  - Exports 8 crates: `stylo`, `stylo_atoms`, `stylo_dom`,
    `stylo_traits`, `stylo_static_prefs`, `stylo_malloc_size_of`,
    `selectors` (v0.37), `servo_arc` (v0.4.3).
  - **Stylo does NOT pull in Servo's layout, constellation, or
    SpiderMonkey.** Clean library boundary; designed for embedders.
  - Works against trait-based DOM abstractions (`TNode`, `TElement`)
    — not tied to html5ever; Blitz implements them against
    `blitz-dom`. We can do the same.
  - **Correction to the 2026-04-14 feasibility doc**: rayon is
    **required, not optional**. There is no feature flag to disable
    it. For **native desktop chrome (our use case), this is fine —
    rayon helps**. For WASM (a future concern, not v1), we'd need
    either a fork-and-patch or `wasm-bindgen-rayon +
    SharedArrayBuffer`. Neither blocks the chrome project.
  - CSS feature support is Firefox-grade: modern selectors, custom
    properties, flexbox/grid, container queries, `calc()`,
    transitions. No JS-driven anything (we wouldn't want it
    anyway).
  - Servo bumps Stylo roughly monthly; the public trait API is
    stable across bumps. Upgrade story is mature.
  - **Implementation effort**: 1–2 weeks for a `TElement` adapter
    over our chosen DOM type, 1–2 weeks to wire computed styles
    into Taffy.

- **B0.2** ~~Audit `blitz-dom`'s API stability and licensing.~~
  Optional adoption — Blitz pins the same Stylo version
  (`v0.16.0`) and `selectors` (`v0.37.0`) we'd consume from Servo,
  so dependency-tree alignment is fine. Adopt-vs-roll-our-own is
  a Phase B1 decision, not a B0 blocker.

- **B0.3** ~~Confirm WebRender display-list API is reachable as a
  library.~~ **Researched 2026-04-24**. Findings:
  - Working example exists in the fork:
    [webrender-wgpu/examples/wgpu_shared_device.rs](../../../../webrender-wgpu/examples/wgpu_shared_device.rs).
    Creates a wgpu device → hands to WebRender via
    `RendererBackend::WgpuShared` → builds display list → renders
    to texture → reads back pixels. **This is the proof-of-concept
    for standalone embedding.**
  - Public API (in `webrender/src/lib.rs`):
    `create_webrender_instance_with_backend()` constructor,
    `RenderApiSender` channel, `DisplayListBuilder` for scene
    construction, `Transaction::set_display_list` /
    `RenderApi::send_transaction` for submission, `Notifier` for
    frame-ready callbacks.
  - WebRender spawns 3 internal threads (render, render-backend,
    scene-builder via Rayon); embedder doesn't manage them.
  - Display list capabilities are sufficient for chrome:
    rectangles, gradients (linear/radial/conic), images, borders,
    box shadows, **text runs (Parley-style pre-shaped glyphs)**,
    clips, 3D transforms, scroll frames, CSS filters.
  - **wgpu version: 29.0.** Matches `servo-wgpu` (Servo's fork),
    matches `egui-wgpu` 0.34.1 (graphshell's main path).
    **Iced 0.14 is the odd one out at wgpu 27.** Removing iced
    *also* removes the wgpu version split — the graphshell-side
    ecosystem is otherwise already aligned on wgpu 29.

- **B0.4** ~~Build a 1-day spike to confirm the WebRender entry
  point works.~~ **Already done by upstream** —
  `wgpu_shared_device.rs` is the spike. We can run it and verify
  it works in our environment as a 30-minute check, not a full
  spike.

**Receipt**: B0 essentially landed via 2026-04-24 research. The
remaining concrete deliverable is running `wgpu_shared_device.rs`
in our local environment to confirm the example builds + executes
on our machines and validate wgpu adapter selection. ~1 day of
work, not 1 week.

### Phase 0.5 — Shader pipeline migration awareness (NEW; ~ongoing)

The 2026-04-24 WebRender survey turned up a real, in-progress
shader pipeline migration that affects timing:

- The
  [2026-04-18 SPIR-V shader pipeline plan](../../../../webrender-wgpu/wr-wgpu-notes/2026-04-18_spirv_shader_pipeline_plan.md)
  is **mid-Phase 2 of 5**. SPIR-V is becoming the canonical shader
  source; naga's GLSL frontend is being retired.
- **Phases 3–5 (queued)**: switch wgpu consumer to artifact-backed
  shader identity, switch GL fallback, retire GLSL preprocessing.
- **Risk to chrome project**: during Phase 3 (rough timing
  ~weeks 4–8 of chrome work), runtime shader identity changes from
  `(name, config)` tuple → SPIR-V digest. Our display-list
  submission code is **not affected** (that contract is stable);
  only our WebRender device/pipeline initialization touches the
  shader-identity boundary. Concretely: 2–3 weeks of "shader
  adapter" work somewhere in the project's middle.
- **Mitigation**: do not author custom WebRender shaders during
  the chrome project — author SPIR-V if any are needed. Track the
  shader pipeline plan weekly. If Phase 3 lands cleanly mid-project,
  no rebase work; if it lands roughly, budget ~2 weeks to absorb.

### Phase 1 — Skeleton (~2 weeks)

- **B1.1** New crate `crates/graphshell-blitz-host` (or a `blitz-host` feature flag in graphshell, mirroring `iced-host`). Empty scaffolding: `BlitzApp`, `BlitzHost`, `BlitzHostPorts`. All ports stubbed identically to early `IcedHostPorts`.
- **B1.2** WebRender bootstrap — winit window, wgpu device, WebRender renderer, swap-chain. The shared singleton.
- **B1.3** First chrome render: a hardcoded HTML string ("Graphshell — Blitz-shaped chrome") through Taffy → display list → WebRender. No interactivity, no Stylo yet.
- **B1.4** CLI flag `--blitz` (mirroring `--iced`). Bin chooses between three hosts: egui (default, retiring), iced, blitz.

**Receipt**: `graphshell --blitz` opens a window showing static text rendered via the Blitz-shaped pipeline.

### Phase 2 — Style + reactive chrome (~3 weeks)

- **B2.1** Stylo integration. Load a stylesheet, compute styles for the DOM tree, feed them into Taffy.
- **B2.2** Tiny reconciler: minimal "view-model → DOM" diff routine. Updates text content, class lists, and a small set of attributes. No vDOM, no patches; direct mutation against `blitz-dom` (or our own DOM if we rolled it).
- **B2.3** First reactive surface: render the toolbar from `FrameViewModel.toolbar.location`. Editable text input requires interaction wiring (next phase) — for now it's read-only, mirroring iced's M5.4b state.
- **B2.4** Toast stack (HTML/CSS instead of iced's `column![]`). Renders from `FrameViewModel` toasts.

**Receipt**: Static CSS-styled chrome with toolbar text and toasts that update from runtime ticks. Visually equivalent to iced's chrome before editable toolbar landed.

### Phase 3 — Interactivity (~3 weeks)

- **B3.1** Hit-testing. Walk the laid-out Taffy tree to map screen coords to DOM elements.
- **B3.2** Event router. Map (element_selector, event_kind) → application message. Wire keyboard, pointer, scroll, focus.
- **B3.3** Editable text input — chrome equivalent of iced's `text_input`. Driven by Parley for cursor + selection + IME.
- **B3.4** Toolbar submit: type URL → `LocationSubmitted` → `HostIntent::CreateNodeAtUrl` (existing path). End-to-end navigation works in Blitz-shaped chrome.
- **B3.5** Hotkeys: Ctrl+L focuses the location bar (mirrors iced's hotkey).

**Receipt**: Toolbar editing + URL submit + Ctrl+L works in `--blitz` mode the same way it does in `--iced`. End-to-end test mirroring iced's `location_submitted_clears_draft_and_creates_node`.

### Phase 4 — Graph canvas integration (~2 weeks)

- **B4.1** Graph canvas adapter. The `<main class="canvas-host">` element gets its layout rect, then `graph-canvas`'s `derive_scene` runs against that viewport, producing a `ProjectedScene<NodeKey>`. New adapter `render/canvas_webrender_painter.rs` (mirror of `canvas_egui_painter.rs` and `iced_canvas_painter.rs`) translates `SceneDrawItem` → WebRender display-list items.
- **B4.2** Camera/pan/zoom owned by host (no iced `canvas::Program::State`). The `BlitzApp` keeps a `CanvasCamera` and round-trips it into `runtime.canvas_cameras` the same way iced does — same `Message::CameraChanged` shape, just a different host loop.
- **B4.3** Pointer/wheel events on the canvas-host element route into the existing canvas update path (event router dispatches to `graph_canvas_update_camera` instead of a chrome handler when the hit element is the canvas-host).

**Receipt**: Graph nodes render and are pan/zoomable inside the Blitz-shaped chrome. `cargo run --features blitz-host -- --blitz` shows a live spatial canvas.

### Phase 5 — HostPorts parity (~2 weeks)

- **B5.1** All `HostPorts` impls fleshed out for `BlitzHostPorts`: clipboard (arboard, same as iced), texture cache (now using WebRender's image API instead of an iced-specific handle), toast queue, present requests, accessibility stub.
- **B5.2** Sanctioned-writes test allowlist updated to include Blitz host files.
- **B5.3** Cross-host parity tests extended: replay-trace parity across **three** hosts (egui, iced, blitz), all three driving the same `runtime.tick`.

**Receipt**: Blitz-shaped is a peer host to iced; all `HostPorts` are real; parity tests prove the runtime is genuinely host-neutral.

### Phase 6 — Servo content surface (~3 weeks)

This is where Blitz-shaped earns its keep over iced — Servo content can now share the same WebRender + wgpu device.

- **B6.1** Servo's `WebView` API exposes per-view rendering output. Wire Servo to render into a WebRender pipeline rooted inside our chrome's display list (via `<iframe>`-style nested pipelines, which WebRender supports natively — Servo uses this internally).
- **B6.2** ViewerSurfaceRegistry stays — it already keys on `NodeKey`, not on a paint backend. The `NativeRenderingContext` backing now points at a WebRender pipeline ID rather than a wgpu texture handle.
- **B6.3** First end-to-end Servo content render in Blitz-shaped chrome. No screenshot loop needed; no wgpu version split.
- **B6.4** Pointer/keyboard events for content panes route through the existing input path Servo expects.

**Receipt**: Loading `https://example.com/` in a graph node renders Servo content inside the Blitz-shaped chrome through one WebRender, one wgpu device. The architectural unification the 2026-04-16 vision doc promised.

### Phase 7 — Cutover and cleanup (~1 week)

- **B7.1** Soak-test Blitz-shaped against the iced acceptance checklist. Identify any iced-only conveniences worth porting.
- **B7.2** Default `graphshell` (no flag) launches Blitz-shaped. Iced retained behind `--iced` for parity comparison and bisecting; egui retired.
- **B7.3** Delete `iced_*` files from `shell/desktop/ui/` once the parity period elapses (1–2 months of overlap).

**Receipt**: Default Graphshell experience is Blitz-shaped chrome. Iced files can be retired in a clean follow-on PR.

---

## 5. Total estimated effort

**~15 weeks (3.5 months)** end-to-end if everything goes smoothly. Realistic estimate with normal friction: **4–5 months**.

**Revised confidence (post-2026-04-24 research)**: ~75–80% probability
of hitting parity in 6 months. Risk drivers:

1. WebRender shader pipeline migration (Phases 3–5, queued) may
   force ~2 weeks of mid-project adaptation work.
2. The "tiny reconciler" (view-model → DOM diff) is the most
   invent-it-ourselves piece; could expand if we underestimate.
3. Text input + IME is the single largest widget cost — Parley
   handles shaping but cursor/selection/IME composition is ours.

The Stylo + WebRender risks the original draft worried about have
both shrunk: Stylo is mature and production-proven via Blitz;
WebRender has a working standalone embedding example and our fork
is on a stable wgpu 29 with a clear migration plan we can track.

Compared to other paths:

| Path | Estimate to feature parity with current iced | Architectural endpoint |
|---|---|---|
| Continue iced + screenshot-loop Servo (current plan) | ~1–2 months for chrome polish + C3 prototype | Permanent wgpu version split; awkward forever |
| Iced + middlenet (M1) only | ~3 weeks; ships middlenet capability | Same wgpu split; no Servo content in iced |
| **Blitz-shaped (this plan)** | **~3.5–5 months** | **One WebRender, one wgpu device, no version split, Firefox-grade CSS for chrome** |
| Full Blitz adoption (Option 1) | 6+ months, blocked on Blitz crates.io maturity | Same as Blitz-shaped, but we depend on upstream Blitz cadence |

Blitz-shaped is **2–3× the time of staying-on-iced** but **half the time of full Blitz adoption** because we control the integration cadence.

---

## 6. Key Risks

### 6.1 Stylo standalone usability

**Status (2026-04-24 research)**: ~~Risk~~ **Largely de-risked.**
Stylo is production-proven as a standalone library — Blitz
consumes it that way today, and the API boundary is clean (no
SpiderMonkey, no Servo layout). Native desktop chrome is the
natural fit (rayon helps; required, not optional, but we have
threads to spare).

**Residual risk**: WASM target requires fork-and-patch or
`wasm-bindgen-rayon + SharedArrayBuffer`. Not a v1 concern.

### 6.2 WebRender as a library (not as Servo's internal renderer)

**Status (2026-04-24 research)**: ~~Risk~~ **Working example
exists.** Our fork ships
[wgpu_shared_device.rs](../../../../webrender-wgpu/examples/wgpu_shared_device.rs) —
a standalone embedding that creates a wgpu device, hands it to
WebRender via `RendererBackend::WgpuShared`, builds a display
list, renders to a texture, and reads back pixels. This is the
contract we'd be building against.

**Residual risk**: the WebRender shader pipeline is mid-Phase 2
of a 5-phase SPIR-V migration. Phase 3 (queued) changes how wgpu
consumes shader artifacts; this is the "moving target" cost.
**Display-list submission API is stable through the migration.**
Only device/pipeline initialization touches the shader-identity
boundary. Budget ~2 weeks mid-project for shader adapter work; do
not author custom shaders during the chrome project.

### 6.3 Tiny reconciler is non-trivial

**Risk**: "View-model → DOM diff" sounds simple but text shaping reuse, layout invalidation, animation hooks, and CSS transitions all become our problem. This is the part where "we own the assembly" extracts the most cost.

**Mitigation**: keep it dumb — full DOM rebuild + Stylo recompute + Taffy relayout each frame. Optimize only when profiling forces it. Chrome DOM is small (dozens of elements), so brute force is viable.

### 6.4 Text input + IME

**Risk**: Editable text with selection, IME composition, accessibility — this is the single most expensive widget to write. Iced gives us this for free; Blitz-shaped means we own it.

**Mitigation**: Parley does shaping/breaking; cursor + selection logic is a few hundred lines but it's a known shape. IME requires winit IME hooks. Accessibility is a long-tail problem we'd defer behind a shim until parity.

### 6.5 Ecosystem velocity

**Risk**: We're choosing the path Blitz/anyrender/DioxusLabs is on, but doing it ourselves. If Blitz proper matures faster than expected, we'd want to converge on it; that means redoing bits of B1–B3.

**Mitigation**: B0.2 audits Blitz's stability. If the API looks like it'll stabilize in 6 months, consider waiting. If not, our hand-assembly is independent of their cadence.

### 6.6 Loss of iced velocity during overlap

**Risk**: Both hosts in tree means double maintenance. New `Message` variants and view-model fields need handling in both places. Slows iced + chrome work.

**Mitigation**: keep iced frozen at "shippable" state during Blitz-shaped development. The portable runtime contract makes this clean — nothing in iced's host adapter changes once the runtime API does.

---

## 7. Open Questions

1. **Do we adopt `blitz-dom`?** API stability check in B0.2 decides. Adopting it gives us a battle-tested DOM tree representation; rolling our own is ~2 weeks of work but pure control.
2. **What's the chrome stylesheet authoring story?** A single static stylesheet at startup? Multiple themes via CSS variable swaps? A loose "user.css" in the config directory? Decide before B2.
3. **Do we want JS/WASM-based extension hooks for chrome?** The original 2026-04-16 vision doc mentions Boa as a possible extension point. For v1 this is a hard no — we keep extensions in graphshell mods, not in chrome JS. Reconsider post-v1.
4. **Hot-reload story?** CSS hot-reload is the killer demo for HTML/CSS chrome. Probably trivial — re-read stylesheet on change, force relayout. Add in Phase 2.
5. **Tooling: web devtools for graphshell chrome?** Could expose `chrome://` devtools for the chrome itself, mirroring browser self-hosted devtools. Parking-lot for v2.
6. **Cross-platform font/text quality.** Parley + WebRender give us native font rendering. Need to compare against iced's text quality on Windows/macOS/Linux during Phase 2.

---

## 8. Decision criteria — when to start this

**Start if any of these become true**:

- iced + screenshot-loop Servo proves unworkable in practice (latency, IME, accessibility issues we can't fix upstream).
- Wgpu version split causes a real, blocking problem (bug we can't fix without sharing devices).
- A second developer joins and the chrome rewrite is parallelizable with iced maintenance.
- Blitz crate matures to crates.io with a usable API — we'd reconsider scope #1 vs #2 at that point.

**Don't start yet if**:

- Iced + middlenet (M1 from the content-surface scoping doc) gives Graphshell a shipping-quality content path.
- The team is one person (you) and the iced work is finally yielding an actually-running product. Don't blow up momentum chasing architectural cleanliness.

---

## 9. Bottom Line

**Blitz-shaped chrome is feasible and architecturally compelling.** The portable contract layer we built for the iced migration makes it ~3.5–5 months of focused work, which is realistic but not free. The big win is the wgpu unification: one WebRender, one wgpu device, no version split between chrome and Servo content forever.

**Right now, it's not the right next slice.** The iced + middlenet path produces a shippable spatial-browser product faster, and it's compatible with a future Blitz-shaped move (the runtime kernel doesn't change). This doc documents the option so it's startable when conditions warrant — most likely after iced + middlenet ships and we have a polished UX target the Blitz-shaped chrome can imitate.

**Recommended adjacent action now**: revive the middlenet HTML lane work (per
[2026-04-16_middlenet_lane_architecture_spec.md](../../technical_architecture/2026-04-16_middlenet_lane_architecture_spec.md)).
That uses the same Stylo + Taffy + WebRender stack at smaller scope (HTML
content rendering) and proves the integration end-to-end before any chrome
bet. **It's actually scope #3 from the original three-option breakdown, and
it de-risks scope #2 for free.**
