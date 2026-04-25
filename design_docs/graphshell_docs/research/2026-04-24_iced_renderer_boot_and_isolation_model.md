<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Renderer-Boot, Process Isolation, and Middlenet as Test Surface (2026-04-24)

**Status**: Research — synthesis of three parallel surveys. Informs
the [2026-04-24 iced content-surface scoping doc](../implementation_strategy/shell/2026-04-24_iced_content_surface_scoping.md)
and shifts several of its architectural assumptions.

**Question that prompted this**: "Research the iced renderer-boot
path, with an eye to parallelization and memory management like
Firefox does (by origin, domain). Consider the needs of the future
app according to our plans, and how we plan to do middlenet (which
we could use to test)."

**Related**:

- [iced host migration execution plan](../implementation_strategy/shell/2026-04-14_iced_host_migration_execution_plan.md)
- [Content-surface scoping](../implementation_strategy/shell/2026-04-24_iced_content_surface_scoping.md)
- [Middlenet engine spec](../technical_architecture/2026-03-29_middlenet_engine_spec.md)
- [Middlenet lane architecture](../technical_architecture/2026-04-16_middlenet_lane_architecture_spec.md)
- [Middlenet direct lane v1.5 plan](../implementation_strategy/2026-04-20_middlenet_direct_lane_v1_5_plan.md)
- [PROJECT_DESCRIPTION](../../PROJECT_DESCRIPTION.md)
  (calls out "origin-grouped processes" and "active/warm/cold node
  states with memory-pressure demotion")

---

## 1. Executive Summary

Three surveys (iced renderer, Firefox/Chromium/Servo isolation
model, middlenet shape) landed three conclusions that change the
content-surface scoping doc's plan:

1. **Iced 0.14 is architecturally closed around `wgpu::Device`.**
   The device lives inside `iced_wgpu::Engine` with `pub(crate)`
   fields; there's no injection hook, no accessor, and the shader
   widget doesn't expose it either. The C1 slot we just landed
   (`IcedHost.wgpu_context`) has no ergonomic boot path in upstream
   iced 0.14.

2. **iced uses wgpu 27.0; Servo's fork is wgpu-29-era.** Even if
   we forked iced to expose the device, the types wouldn't share
   directly with Servo's wgpu. Two independent device worlds.

3. **Middlenet is a CPU-side `RenderScene`, not a GPU surface.**
   It needs no wgpu device at all. Iced can paint middlenet scenes
   with native primitives today — no renderer-boot work required.
   This is the first-surface test we should reach for next, not
   wgpu-device exposure.

The scoping doc's C1 stays valid as a slot, but the **recommended
next slice is middlenet-in-iced, not iced-device-exposure**. The
Servo/wgpu path changes shape too: rather than "share one wgpu
device between Servo and iced," it becomes "Servo renders
offscreen into a texture we own, upload into iced each frame."

Process-isolation research confirms the product spec direction:
stay on Servo's one-process-many-WebViews model for v1, add
**origin-bucketed multi-Servo instances** later if security or
crash-blast-radius forces the issue. Node lifecycle (active / warm
/ cold) maps cleanly to Firefox's tab-unload model, keyed on
graph-spatial distance × staleness.

---

## 2. Iced 0.14 Renderer Boot — What We Learned

### 2.1 Construction chain

From `iced::application(...).run()` down to wgpu:

- `iced_winit-0.14.0/src/lib.rs:130` `run_instance` starts the async
  runtime inside winit's event loop.
- `iced_wgpu-0.14.0/src/window/compositor.rs:46` `Compositor::request`
  is where wgpu bootstraps:
  - line 55: `wgpu::Instance::new()` with `InstanceDescriptor`
  - line 84: `instance.request_adapter()` (HighPerformance default)
  - line 163: `adapter.request_device()`
  - line 180: `Engine::new(device, queue, ...)` wraps them
- Engine's fields are `pub(crate)` (`iced_wgpu-0.14.0/src/engine.rs:11`).
  No public accessor.

### 2.2 Customization surface (limited)

Exposed to apps via `iced_wgpu::Settings`:

- `present_mode` (VSync/Mailbox/Immediate)
- `backends` (Vulkan/Metal/DX12/GL/WebGL selection)

Also respected: `ICED_PRESENT_MODE` and `WGPU_BACKEND` env vars.

**Not exposed**: adapter preference overrides, device-limits
customization beyond iced's defaults (`max_bind_groups=2`,
`max_non_sampler_bindings=2048`), pre-built device injection, or
any post-boot accessor for the device.

### 2.3 Widget-level device access

- `iced::widget::shader::Program<Message>` — the closest iced has
  to a "custom wgpu widget." Returns a `Primitive` (display-list
  entry) which iced interprets in its renderer later. **The `draw`
  method never receives a `wgpu::Device`.** Custom primitives are
  deferred, not immediate-mode, and the device stays private.
- `iced::advanced::Widget` — doesn't expose device access either.
- `iced::widget::canvas` — builds geometry (paths, fills, strokes),
  no device access.

### 2.4 Threading / multi-window

- **Single-threaded**: event loop and render run on the same thread
  via winit's `EventLoop::run()`. No built-in render-thread split.
- **Multi-window is supported** (`iced_winit-0.14.0/src/window.rs:28`)
  but all windows share one `Engine` — one device, many surfaces.

### 2.5 wgpu version mismatch

- `iced_wgpu-0.14.0/Cargo.toml:110` — `wgpu = "27.0"` upstream.
- Graphshell's shared-wgpu path uses `servo::wgpu` from the Servo
  fork — wgpu-29-era.
- **These are different crates at the type level.** Even if iced
  exposed its device, `servo::wgpu::Texture` ≠ `wgpu::Texture`.
  Any cross-boundary handoff needs raw-handle interop
  (HAL level) or a render-to-buffer round trip.

### 2.6 Implication for C1

The scoping doc's C1 is "expose iced's `wgpu::Device` via
`IcedHost`." Now we know:

- **Producing the slot is trivial** (done as slice 21).
- **Filling the slot needs either a fork of iced or an indirect
  path.** There's no clean upstream hook.
- **The filled slot still doesn't solve Servo interop** because
  of the version split.

This doesn't invalidate the C3 widget design — iced can still
render webview content — but the **mechanism** is different from
what the scoping doc assumed.

---

## 3. Middlenet as the First Content Surface

### 3.1 What middlenet produces

Per [middlenet-render/src/lib.rs](../../../crates/middlenet-render/src/lib.rs):

- `RenderScene { blocks, hit_regions, outline, scroll_extent, diagnostics }`
- `RenderBlock` — block kind, rect, text runs, link targets
- Pure Rust, **no GPU dependency, no wgpu device required**
- Designed WASM-portable (Phase 2)

### 3.2 Current flow (egui)

[registries/viewers/middlenet.rs:902](../../../registries/viewers/middlenet.rs)
iterates `RenderScene.blocks` and emits egui widgets per block
(labels for headings, buttons for links, text_edit for code blocks,
etc.). No shared rendering context — the egui `Ui` is the paint
target.

Protocols live today: Gemini, Gopher, Finger, Markdown, RSS, Atom,
JSON Feed, plain text. Lane architecture supports `Html` (pending,
Phase 2) and `FaithfulSource` fallback.

### 3.3 Why this is the iced path's first-render win

- **Zero wgpu device sharing needed.** Iced paints middlenet
  scenes with native widgets the same way egui does.
- **End-to-end validation**: URL → fetch → semantic doc → render
  scene → iced paint. Proves the full content lifecycle inside
  iced without pulling in Servo.
- **~2–3 week effort** (per the middlenet survey agent) — port the
  egui block-iteration loop to iced widgets. No middlenet crate
  changes required.
- **Exercises the HostIntent pipeline we just landed.** Submit a
  `gemini://` URL in iced's toolbar → `HostIntent::CreateNodeAtUrl`
  → runtime creates node → node's viewer resolves to
  `viewer:middlenet` → iced's middlenet widget renders blocks.
- **Regression-safe**: middlenet is its own crate stack; iced's
  integration doesn't touch Servo or compositor paths.

### 3.4 What it doesn't prove

- Iced + **Servo** wgpu handoff. That's C3/C5 material and needs
  the version-split resolution.
- Iced's wgpu-texture sampling path. Middlenet is CPU-layout;
  iced's text/container widgets handle rasterization. No custom
  wgpu pass exercised.
- Per-origin process isolation. Middlenet runs in-process today.

### 3.5 Sliced plan (revised)

New **M1: Middlenet-in-iced** slice, inserted between C0 (chrome)
and C1 (wgpu device):

- **M1.1**: Iced middlenet viewer widget (block iterator).
  ~1 session. Takes a `RenderScene`, renders blocks using
  `iced::widget::{text, button, container, column}`.
- **M1.2**: Wire `viewer:middlenet` route through the iced
  `WebViewSurface` scaffolding. ~1 session. `IcedApp::view`
  chooses between middlenet widget and Servo-texture widget
  based on the node's viewer kind.
- **M1.3**: End-to-end test: submit a `gemini://` URL in the
  toolbar, verify a node appears with rendered middlenet content
  inside an iced pane. ~1 session, plus a small integration test.

After M1, iced is a **usable spatial browser for middlenet
content** (Gemini, RSS, etc.) even without Servo wired in. That's
a shipping-quality product capability.

---

## 4. Process / Memory Isolation Model

### 4.1 What major browsers do

- **Firefox (Fission)**: isolates at eTLD+1 ("site"). Shared GPU
  process. OS-memory-pressure-driven tab unloader (LRU + cost
  model above ~11 tabs).
- **Chromium**: site-keyed by default, origin-keyed opt-in. Shared
  GPU process via command buffer / Dawn wire. Soft process limit
  based on system RAM; below 2 GB Android disables site isolation
  entirely.
- **Servo (today)**: single-process default. `--multiprocess`
  exists on Linux/macOS but isn't the shipping path. One
  constellation + one WebRender + many WebViews, all sharing GL
  context.

### 4.2 What nobody does

- **Per-origin GPU device.** GPU is always a shared service (GPU
  process in Chromium/Firefox; single WebRender in Servo). This is
  the pattern Graphshell should follow.
- **One OS process per tab** at the scale Graphshell targets.
  Chromium itself degrades to process reuse beyond its soft cap
  and disables isolation under memory pressure.

### 4.3 Recommendation for Graphshell

**v1 model** (ship this):

- **One Servo instance, many WebViews** (what Servo already does).
  Origin isolation is thread-level, not process-level. Weaker
  than Fission but acceptable for a prototype that trusts content.
- **Shared GPU device** across graph canvas + all content. One
  wgpu device owned by the host; render all content to textures
  composed in the force-directed scene. **This aligns with every
  production browser's GPU architecture.**
- **Active / warm / cold node lifecycle** modeled on Firefox's
  tab unloader:
  - *Active*: script running, in viewport/attention radius
  - *Warm*: DOM retained, script paused, no compositor frames
  - *Cold*: unloaded, metadata only (URL, scroll, form state,
    last screenshot), restore on focus
  - Trigger on OS memory pressure (same APIs Firefox uses)
  - **Novel for Graphshell**: demote by
    `(graph_distance_from_focus × staleness)` rather than LRU —
    reflects the spatial-browsing reality that distant nodes in
    graph space are less likely to be revisited.

**v2 model** (future, if needed):

- **Origin-bucketed Servo instances**: K ≪ N Servo processes,
  nodes hashed to instances by eTLD+1. Gives crash isolation and
  GPU sharing within each bucket without exploding process count.
  The right granularity for 100–500 nodes. Revisit when a concrete
  security or reliability requirement forces it.
- **WASM sandbox per origin** (RLBox-style) — keep in design
  space but not in v1.

### 4.4 What to add to PROJECT_DESCRIPTION

The product spec already mentions "origin-grouped processes" and
"active/warm/cold node states." Suggested refinements once the
isolation model is formally scoped:

- Clarify "origin-grouped" = thread bucket in v1, possibly process
  bucket in v2.
- Explicitly name the lifecycle trigger signals:
  (graph_distance × staleness) + OS memory pressure.
- Note pinned exceptions: media-playing nodes, WebRTC sessions,
  user-pinned "never cold" nodes.

---

## 5. Options for the Iced ↔ Servo wgpu Handoff

Four options, ordered from least-invasive to most-invasive:

### Option A — "Screenshot loop" (iced image handle per frame)

- Servo renders offscreen into a `wgpu::Texture` on its own
  wgpu-29 device.
- Each frame, copy the texture → CPU → `iced::image::Handle::from_rgba`.
- Iced paints the handle in an image widget.
- **Pros**: Works today. Zero device sharing. Survives the wgpu
  version split.
- **Cons**: Full readback every frame (latency + bandwidth).
  Unsuitable for video / fast-updating content.
- **Fit**: Good for prototype; unacceptable for production.

### Option B — "Hal-level raw handle interop"

- Both wgpu 27 and wgpu 29 support raw Vulkan/Metal/DX12 handle
  extraction via `wgpu::hal`.
- Servo renders to a native texture; we extract its raw Vulkan
  handle; import into iced's wgpu via `Device::create_texture_from_hal`.
- **Pros**: Zero-copy. Shares GPU memory.
- **Cons**: Platform-specific glue per backend. Fragile across
  wgpu version bumps. Requires both wgpu instances to pick the
  same adapter + backend.
- **Fit**: Production-quality answer if we're committed to
  independent wgpu instances.

### Option C — "Shared wgpu via forked iced"

- Fork iced to expose `Engine.device` / `Engine.queue` as public
  accessors, or add a `Compositor::with_pre_built_device` hook.
- Align iced's wgpu version with Servo's (upstream wgpu 29
  landing in iced is future; or pin iced to a compatible
  revision).
- **Pros**: Actually shared device, matches every production
  browser's GPU architecture.
- **Cons**: Fork maintenance burden. Blocked on iced catching
  up to wgpu 29 (or Servo catching down to 27).
- **Fit**: Long-term correct answer; probably v2.

### Option D — "Skip Servo wgpu-texture path entirely; use GL compat"

- Retain the existing GL callback fallback (`ViewerSurfaceBacking::CompatGlOffscreen`).
- Iced renders chrome via its own wgpu; Servo renders content via
  GL into an offscreen buffer; composite via CPU copy into iced.
- **Pros**: No wgpu version sharing needed.
- **Cons**: Loses the shared-wgpu performance path egui already
  has; perpetuates the GL compat lane Servo is trying to retire.
- **Fit**: Escape hatch only.

### Recommendation

- **M1 (middlenet-in-iced)**: neither option matters — middlenet
  is CPU. Do this first.
- **C3 prototype (Servo first render)**: **Option A** (screenshot
  loop). Ugly but works today, lets us prove the widget API and
  event routing without blocking on wgpu interop.
- **Post-prototype**: evaluate **Option B vs C** based on actual
  perf needs. If we see readback latency in real content, go B.
  If iced lands wgpu 29 upstream within a reasonable window, go C.
- **Option D is not recommended** — it perpetuates a path the rest
  of the stack wants to retire.

---

## 6. What Changes in the Plans

### 6.1 Content-surface scoping doc ([2026-04-24_iced_content_surface_scoping.md](../implementation_strategy/shell/2026-04-24_iced_content_surface_scoping.md))

**Update §C1**: slot landed, but boot-path wiring is **not simple**
— it requires either (a) iced fork, (b) render-to-buffer workaround,
or (c) wait for iced wgpu 29. Keep the slot; don't block content
surfaces on filling it.

**Insert new §M1 before §C1**: "Middlenet-in-iced" as the first
content-rendering surface. No wgpu device needed. ~3 sessions.

**Revise §C3 "`WebViewSurface<NodeKey>` widget"**: the widget's
**first implementation** should be the screenshot-loop (Option A
above) against Servo — not a shared-device integration. That's
the actually-achievable path.

**Add §C3.5 (new)**: "Shared-wgpu interop" — the Option B or
Option C work. Post-prototype. Tracks the iced-wgpu-version
landscape.

### 6.2 Iced migration execution plan

Add to the Related section:

- This research doc.

Add a sequence-rule corollary: **"Iced chrome isn't blocked on
Servo readiness."** Middlenet gives iced a real content surface
ahead of Servo wgpu being ready, so chrome polish (command
palette, settings, overlays) can proceed against a real
content-rendering substrate.

### 6.3 PROJECT_DESCRIPTION (suggested refinements)

Clarify the isolation model paragraph:

> **Origin-grouped processes** — in v1, origin grouping is
> thread-level (shared Servo process, isolated script threads per
> WebView); v2 introduces origin-bucketed Servo instances (K ≪ N
> processes, nodes hashed by eTLD+1) if security/reliability
> requirements force it. GPU device is shared across all content
> and the graph canvas, matching every production browser's
> architecture.

Clarify the lifecycle trigger:

> **Active / warm / cold node states** — triggered by OS memory
> pressure + a Graphshell-specific signal
> `(graph_distance_from_focus × staleness)`. Pinned exceptions for
> media-playing, WebRTC, and user-pinned "never cold" nodes.
> Modeled on Firefox's tab unloader, adapted to spatial browsing.

---

## 7. Bottom Line

1. **Iced's renderer is architecturally closed.** Filling the C1
   slot requires a fork or a workaround — not a simple upstream
   integration. But this doesn't block progress, because…

2. **Middlenet gives iced a real content surface today.** It's
   the correct first target: no wgpu device negotiation, it
   exercises the full URL→fetch→render→paint lifecycle, and it
   ships a useful product capability (Graphshell-for-Gemini/RSS)
   even if Servo integration stalls.

3. **For Servo integration: start with a screenshot loop.** It's
   ugly but it works today, across the wgpu version split, and
   lets us develop the widget API before committing to
   shared-device interop.

4. **Process isolation model is clear: shared GPU, thread-level
   origin grouping in v1, Firefox-style unload with graph-distance
   signals.** Matches product spec and every production browser.
   Origin-bucketed Servo instances are a v2 lane.

5. **Middlenet is the ideal test surface** for everything iced
   chrome-related that needs "a live content renderer to verify
   against." Earmark its block-iterator as a reusable test target
   for future iced work.

Recommended **next slice**: **M1.1** — port the middlenet
block-iterator to iced widgets. ~1 session, no new dependencies,
unlocks the full middlenet-in-iced capability.
