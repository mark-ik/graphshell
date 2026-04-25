<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Content-Surface Scoping (2026-04-24)

**Status**: Scoping pass — partially landed (C1 slot 2026-04-24);
plan revised after the
[2026-04-24 renderer-boot + isolation research doc](../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md).
**Parent plan**: [2026-04-14_iced_host_migration_execution_plan.md](2026-04-14_iced_host_migration_execution_plan.md) (§M4.5, §M5, §12.10)
**Author intent**: Identify what it takes to mount content (middlenet
documents, then Servo webviews) inside iced graph node panes, given
that the iced host already owns graph rendering, chrome, events,
camera, and toasts.

**Plan-shape revision (2026-04-24)**: research showed iced 0.14's
`wgpu::Device` is architecturally closed and that iced uses wgpu 27
while Servo's fork is wgpu-29-era. The original "share one wgpu
device" design isn't a viable upstream-iced path. Plan revised to:

1. **First content surface = middlenet** (CPU-side `RenderScene`,
   no wgpu device needed). New §M1 below.
2. **First Servo surface = screenshot loop** (Option A from the
   research doc). Functional today, sidesteps the wgpu version
   split.
3. **Shared-wgpu interop** (Options B/C) is post-prototype.

See the research doc for the full options analysis.

**Long-horizon alternative**: the wgpu version split is permanent
under iced. The
[2026-04-24 Blitz-shaped chrome scoping](2026-04-24_blitz_shaped_chrome_scoping.md)
doc captures what it would take to replace iced with a Stylo +
Taffy + Parley + WebRender stack — same WebRender that hosts the
graph canvas and (eventually) Servo content. ~3.5–5 months of
focused work; not the next slice, but startable when iced's
limitations bite hard enough.

## 0. Platform-Tier Framing (added 2026-04-24)

The content-surface architecture is ultimately platform-tiered.
Native desktop is the priority track; other platforms degrade
gracefully:

| Platform | Web content path | Lighter content path |
|---|---|---|
| **Native desktop** (Windows, macOS, Linux) | **Servo via custom host** — we own the rendering integration; Servo content composes alongside chrome and graph canvas in our pipeline | Middlenet (CPU-side `RenderScene`) |
| **Mobile / iOS / Android** | **wry** (system WebView2 / WKWebView / Android WebView) | Middlenet |
| **Web** (compiled to WASM) | wry-equivalent (browser-in-browser via iframe) or n/a | Middlenet (WASM-portable per [feasibility doc](../../research/2026-04-14_wasm_portable_renderer_feasibility.md)) |

**Implication for the iced content-surface plan**:

- The "Servo + iced" pairing is **native-only**. We don't need to
  generalize it to mobile/web — those tracks use different
  content engines entirely.
- Middlenet is the **universal lower-tier renderer** across all
  platforms. Investing in middlenet integration once (M1) pays off
  on every target.
- "Custom host for Servo on native" is **not impossible** — we
  already do it through `ViewerSurfaceRegistry` and the compositor
  adapter on egui. Iced just needs the same machinery wired
  through its retained-mode model.

This framing changes the C3 question from "how do we mount Servo
*inside iced*?" to "how do we mount Servo content *on the native
desktop platform*, which currently uses iced as the chrome layer?"
The answer can evolve as the chrome layer evolves (iced → Blitz-
shaped) without re-architecting the Servo side.

## 1. Why this is the next big slice

With the 2026-04-24 stateful-adapter work done, the iced host can:

- render the shared graph via `ProjectedScene<NodeKey>`,
- drive the runtime through `runtime.tick(input, &mut IcedHostPorts)`,
- pan/zoom with canvas-local camera state that round-trips into
  `runtime.graph_app.workspace.graph_runtime.canvas_cameras`,
- render chrome (toolbar, toast stack) from `FrameViewModel`,
- accept text input and hotkeys.

What it **cannot** do: paint a webview (Servo/wry) inside a graph node.
That path — mount HTML/web content inside a spatial node — is the
product's defining capability. §12.10 (M4.5) names viewer-surface
host-nativity as the last prerequisite for a "useful" iced host; this
doc scopes the work.

## 2. Current content-surface architecture (as built for egui)

Covered at depth in §12.8/§12.10 of the parent plan and the Explore
survey of 2026-04-24. The short version:

- **`ViewerSurfaceRegistry`** at
  [compositor_adapter.rs:315](../../../../shell/desktop/workbench/compositor_adapter.rs)
  owns `HashMap<NodeKey, ViewerSurface>`. **Already host-neutral.**
- **`ViewerSurfaceBacking`** has two variants: `CompatGlOffscreen`
  (legacy, GL callback path) and `NativeRenderingContext` (shared-wgpu
  path). **Portable enum; no egui types.**
- **Content composition has two paths**, selected by backing type:
  1. **Shared-wgpu** — webview renders into a wgpu texture the host
     imports via `upsert_native_content_texture`. The host's wgpu
     device/queue is shared with Servo.
  2. **Callback fallback** — webview renders into a GL context; the
     host registers a `ParentRenderCallback` that runs inside the
     host's paint pass.
- **`ContentPassPainter` trait** is the host-neutral painter seam.
  Egui impl lives at [compositor_adapter.rs:1046 `EguiContentPassPainter`](../../../../shell/desktop/workbench/compositor_adapter.rs).
  **Iced already has a stub** ([iced_host_ports.rs `IcedContentPassPainter`](../../../../shell/desktop/ui/iced_host_ports.rs)).

**The single genuine leak**: `BackendTextureToken` at
[render_backend/mod.rs:335](../../../../shell/desktop/render_backend/mod.rs)
wraps `egui::TextureId`. The whole shared-wgpu path assumes the host
has an egui texture atlas to import into. Iced has its own
`iced::image::Handle` / wgpu texture pipeline.

## 3. The iced-native mental model

Iced's content composition idioms differ from egui's fundamentally.
Rather than trying to fit iced into `CompositorAdapter`'s
static-painter pattern, the iced host should use iced-native
primitives:

- **For shared-wgpu content** — iced supports custom wgpu passes via
  `iced::widget::shader` (the shader widget) or via the advanced
  renderer pipeline. A Servo-produced wgpu texture can be consumed
  by iced's shader widget and composited inside an iced canvas pane.
- **For overlay painting** — `GraphCanvasProgram::draw` already
  paints the scene inline. Overlay rects from `FrameViewModel.overlays`
  flow through the same `iced_canvas_painter` module we already built.
- **For texture lifecycle** — iced widgets own their own cache. No
  static `COMPOSITOR_NATIVE_TEXTURES` map needed; iced's shader widget
  holds onto the `wgpu::Texture` reference for the duration of the
  widget tree.

So the iced content-surface path is **not** "implement
`ContentPassPainter` for iced." It's "define how iced mounts Servo
content as an iced widget" — a distinct architecture.

## 4. What needs to be assembled

### 4.1 Portable content-surface types (core crate work)

Today's `BackendTextureToken(egui::TextureId)` is the only non-portable
type on the path. Options:

- **A**: Make `BackendTextureToken` an opaque `u64` id that each host
  maps to its native texture type internally.
- **B**: Add an iced-flavored variant (`BackendTextureToken::Iced(...)`).
  Breaks the "single token" simplicity.
- **C**: Retire `BackendTextureToken` entirely for iced; iced holds
  `wgpu::Texture` references directly inside widget state.

**Recommendation**: **C**. Token-based indirection was useful when
there was only one host and one atlas; with iced using a different
texture lifecycle entirely, direct `wgpu::Texture` references are
cleaner. `BackendTextureToken` stays in egui-land; iced uses its own
handle type. The portable seam is the `wgpu::Texture` produced by
Servo, not a host-specific token wrapped around it.

### 4.2 Shared wgpu device/queue acquisition for iced

The egui host owns a `UiRenderBackendHandle` that provides
`shared_wgpu_device_queue()` ([compositor_adapter.rs near line 1488](../../../../shell/desktop/workbench/compositor_adapter.rs)).
Iced needs equivalent access to iced's wgpu device/queue so Servo can
render directly into a texture iced's shader widget will sample.

Iced 0.14 exposes the wgpu device via the advanced renderer. The iced
host will need a hook (probably a custom shader-widget subclass or
`iced::advanced` integration) to expose `wgpu::Device` + `wgpu::Queue`
for Servo's use. **This is the largest open architectural question.**

### 4.3 Iced "webview widget" design

A new iced widget — provisionally `WebViewSurface<NodeKey>` — that:

- Owns a reference to the `ViewerSurfaceRegistry` (via the iced host).
- Given a `NodeKey`, resolves its `ViewerSurface` and paints the
  backing texture inside the widget's bounds.
- Forwards pointer/keyboard events to Servo (IME, scroll, click) —
  this overlaps with Servo's own input handling and needs careful
  integration with iced's event model.
- Signals content generation changes (via the existing
  `bump_content_generation` path) so iced redraws when the webview
  renders a new frame.

This widget is the iced-native counterpart to the egui host's
`tile_render_pass` + `compose_webview_content_pass` machinery. It is
**not** a thin wrapper around `ContentPassPainter`; it's a full iced
widget.

### 4.4 Surface-per-node lifecycle from iced-app state

`IcedHost.pending_present_requests` already exists as the deferred
queue for `bump_content_generation`. For the full lifecycle:

- **Allocate on first render** — when `IcedApp::view` builds a
  `WebViewSurface<NodeKey>` for a node that isn't in the registry,
  iced requests the runtime allocate a `ViewerSurface`. This goes
  through the runtime's existing `viewer_surface_host` path; the
  iced host supplies the wgpu device.
- **Retire on node destruction / pane close** — today the
  `retire_surface` port method exists but iced leaves it as a no-op.
  When `WebViewSurface` widgets unmount, iced should push retire
  requests into the `IcedHost.pending_retire_requests` queue (which
  we already have scaffolded).

### 4.5 Servo input routing into iced

Iced already translates iced events → `HostEvent`s. Servo consumes
events via its own input API (pointer, keyboard, IME). The iced
WebViewSurface widget needs to **also** forward iced events to Servo
directly (bypassing the runtime for events that are scoped to the
webview's bounds). This mirrors what
`shell/desktop/workbench/tile_behavior/node_pane_ui.rs` does on the
egui side — iced will need its own equivalent, probably as a method on
`WebViewSurface`.

## 5. Architectural decisions to make

These shape the slicing but shouldn't block scoping:

1. **Shader widget or custom widget?** iced's `shader` widget can
   consume a wgpu texture but requires a concrete pipeline; a fully
   custom widget gives more control over event handling but is more
   boilerplate. (Recommendation: start with `shader`, move to custom
   if input forwarding needs it.)
2. **Shared device across egui and iced?** During overlap, both hosts
   run in separate binaries (the binary is chosen at CLI time). No
   device sharing needed; each host owns its own `wgpu::Device`.
3. **Servo wgpuification status**
   ([servo-wgpu/docs/2026-04-18_servo_wgpuification_plan.md](../../../../../../servo-wgpu/docs/2026-04-18_servo_wgpuification_plan.md))
   — iced depends on Servo producing wgpu textures cleanly. The
   callback-fallback path (GL) is explicitly out of scope for iced;
   iced ships shared-wgpu-only. Blocks on Servo's readiness.
4. **AccessKit bridge** — iced's accesskit integration (§5.2 of
   2026-04-17_chrome_port_cleanup_plan) is a parallel track. The
   WebViewSurface widget needs AT semantics for the hosted webview;
   deferred until the accesskit bridge lands.

## 6. Estimated slices

**Revised 2026-04-24** after the
[renderer-boot research](../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md).
The Servo-dependent slices are now sequenced after a non-Servo first
content surface (middlenet). Iced gains real content rendering
without blocking on Servo wgpuification, the wgpu version split, or
the iced renderer-boot question.

### M1 — Middlenet-in-iced (first content surface, no wgpu work)

Middlenet produces a CPU-side `RenderScene` (blocks + hit regions +
outline) per [middlenet-render](../../../crates/middlenet-render/src/lib.rs).
Iced paints these with native widgets the same way egui does today.
No device sharing, no version mismatch, no Servo dependency.

The egui side already does the work: [registries/viewers/middlenet.rs:902-956](../../../registries/viewers/middlenet.rs)
walks `scene.blocks` and dispatches by `RenderBlockKind` to egui
widgets. Each block kind maps cleanly to an iced primitive:

| `RenderBlockKind` | egui rendering | iced equivalent |
|---|---|---|
| `Rule` | `ui.separator()` | `iced::widget::horizontal_rule(1)` |
| `CodeFence` | `TextEdit::multiline` (read-only, monospace) | `text(...).font(Font::MONOSPACE)` inside a `container` |
| `List { ordered }` | `horizontal!{label("•"), label(line)}` per line | `column!` of `row![text("•"), text(line)]` |
| `FeedHeader` / `FeedEntry` | text-run loop + spacing | text-run loop + `Space::with_height(6)` |
| `Heading` / `Paragraph` / `Link` / `Quote` / `MetadataRow` / `Badge` / `RawSourceNotice` | text-run loop | text-run loop |

The **text-run loop** is the only non-trivial piece — it walks
`block.text_runs: Vec<RenderTextRun>` honoring per-run `TextStyle`
(weight, italic, monospace, etc.) and `link_target`. Iced's
`text` widget handles the styling parts; link clicks need to
emit `Message::LinkActivated(LinkTarget)` which routes back into
`HostIntent` (probably `CreateNodeAtUrl` for click-to-open
behavior, mirroring the egui `intents` parameter at
`registries/viewers/middlenet.rs::render_text_run`).

**Sliced execution**:

- **M1.1** (~1 session): New module `shell/desktop/ui/iced_middlenet_viewer.rs`.
  Public function `render_scene(scene: &RenderScene) -> Element<'_, Message>`
  that walks blocks and returns an iced `Element`. Block-by-block
  dispatch mirroring the egui table above. Link-target handling
  emits `Message::LinkActivated(LinkTarget)` (new variant); not
  yet wired to runtime — placeholder log-and-toast.
- **M1.2** (~1 session): Wire `viewer:middlenet` route into
  `IcedApp::view`. The view function decides per-node whether
  it's a graph view (existing `GraphCanvasProgram`) or a
  middlenet view (new widget). Decision lives on the node's
  `ViewerKind` — needs to be readable from the `FrameViewModel`
  or via a fallback into `runtime.graph_app`.
- **M1.3** (~1 session, validation): End-to-end test —
  `LocationSubmitted("gemini://example.gemini/")` →
  `HostIntent::CreateNodeAtUrl` → runtime creates node →
  protocol probe identifies it as middlenet content →
  `viewer:middlenet` route → iced renders blocks. Smoke test the
  golden path. Real Gemini fetch may need stubbing in the test
  harness; live fetch validation runs as a manual test.
- **M1.4** (~1 session, link routing): Wire
  `Message::LinkActivated(LinkTarget)` to push another
  `HostIntent::CreateNodeAtUrl` so clicking a link inside a
  middlenet document creates a new graph node and opens the
  target. This closes the loop on "spatial browsing of
  middlenet content."

After M1, iced is a usable spatial browser for middlenet content
(Gemini, RSS, Markdown, plain text). **Shipping-quality even if
Servo integration stalls** — and the same crate stack
(middlenet-engine + middlenet-render) is reusable on
mobile/wasm targets without modification.

**Out of scope for M1** (can land later as polish):

- Find-in-page over rendered blocks (per
  [middlenet direct lane v1.5 plan](../2026-04-20_middlenet_direct_lane_v1_5_plan.md)).
- Outline navigation panel rendering scene.outline.
- AccessKit projection for screen readers (waits on iced
  accesskit bridge).
- Streaming `DocumentDelta` updates for live feeds.

### C1. Iced wgpu device/queue slot ✓ (slot landed; boot wiring deferred)

- **C1. Iced wgpu device/queue exposure** (~1 session).
  ~~Add a method to `IcedHost` that surfaces iced's `wgpu::Device` +
  `wgpu::Queue` references.~~ **Landed 2026-04-24** (slice 21 of the
  iced-host migration execution plan). `IcedHost.wgpu_context:
  Option<IcedWgpuContext>` slot + `install_wgpu_context` / `wgpu_context`
  accessors in place. Uses `servo::wgpu` types pending iced-version
  resolution. Boot-path wiring (where iced's renderer calls
  `install_wgpu_context`) is deferred until the advanced-renderer
  exposure is pinned down — this is a known unknown, but the slot is
  ready.
- **C2. Token-less texture flow** (~1 session).
  `HostSurfacePort::register_content_callback` and
  `paint_native_content_texture` paths split: egui keeps
  `BackendTextureToken`; iced bypasses it and holds `wgpu::Texture`
  directly. The `ViewerSurfaceBacking::NativeRenderingContext`
  variant stays portable; the token is a host-side wrapping concern.
- **C3. `WebViewSurface<NodeKey>` iced widget** (~2–3 sessions).
  **Revised 2026-04-24** (post-renderer-boot research):
  first implementation uses **Option A (screenshot loop)** —
  Servo renders into a texture on its own wgpu-29 device, the
  host reads back to RGBA, uploads into iced via
  `iced::image::Handle::from_rgba` per frame. Sidesteps the
  wgpu version split and the iced renderer-boot closure problem.
  Acceptable latency for prototype; **acceptable as the long-term
  path while iced is the chrome layer**. If we later move to
  Blitz-shaped chrome (per
  [2026-04-24_blitz_shaped_chrome_scoping.md](2026-04-24_blitz_shaped_chrome_scoping.md))
  the Servo content lands in WebRender directly and the
  screenshot loop retires with iced.
- **C3.5. Shared-wgpu interop on iced** (~3+ sessions; viability
  reassessed 2026-04-24). The original plan was to upgrade to
  Option B (HAL raw-handle interop) or Option C (forked iced)
  if screenshot-loop perf disappointed. Updated assessment:
  - **Option B (HAL interop)** is still viable as a perf
    optimization on the iced track. Estimated ~3 sessions.
  - **Option C (forked iced)** has shifted out of consideration.
    Iced 0.14's wgpu 27 dependency is fundamental to its
    upstream architecture; aligning iced with our wgpu 29
    ecosystem would mean tracking a fork indefinitely. **The
    energy that would go into "fork iced for wgpu alignment"
    goes into "Blitz-shaped chrome" instead** — same engineering
    spend, much larger payoff (chrome unification + wgpu
    unification + HTML/CSS authoring).
  - Net: if iced screenshot-loop perf is acceptable for v1, ship
    that. If not, consider Option B as a stopgap **or** accelerate
    Blitz-shaped as the proper long-term answer.

**The "custom Servo host" framing**: scope C3 ultimately delivers
a **custom Servo host** for native desktop — we own the
integration, Servo content composes alongside chrome and graph
canvas through whatever pipeline our chrome layer exposes
(screenshot-into-iced today; one WebRender tomorrow under
Blitz-shaped). This is in contrast to mobile/web targets which
delegate to system WebViews via wry. The architectural
separation lets each target use its native strength.

- **C4. Surface allocation lifecycle** (~1 session).
  Wire allocation on first render (via `viewer_surface_host` bridge)
  and retirement on widget unmount. Uses the existing
  `pending_retire_requests` queue on `IcedHost`.
- **C5. First end-to-end webview render** (~1 session, validation).
  Load a test URL through the toolbar submit path (requires the
  intent-routing follow-on), render inside an iced node pane, verify
  the wgpu texture samples correctly and pointer events reach Servo.
- **C6. AccessKit bridge for the webview surface** (~TBD).
  Blocked on the iced accesskit bridge (M6 §5.2).

Rough total: **5–6 sessions of focused work**, plus Servo/accesskit
dependencies.

## 7. What we can start today without Servo readiness

- **C1** (wgpu device exposure) is independent — we can expose the
  device handle now and sit on it.
- **Design doc iteration** — this doc is the scoping pass; a
  follow-on design doc would cover C3's widget API in detail.

What we **cannot** do today: full C5 (end-to-end) without Servo's
wgpu content surface.

## 8. Bottom line

The iced content-surface path is **mostly a new iced widget**, not a
refactor of the compositor adapter. The portable core
(`ViewerSurfaceRegistry`, `ViewerSurfaceBacking`, `NodeKey` identity,
`bump_content_generation` lifecycle) is already host-neutral — the
audit found zero egui types there. The only real leak,
`BackendTextureToken(egui::TextureId)`, doesn't need to be plumbed for
iced; iced holds `wgpu::Texture` references directly.

**Blocker ladder** (top → bottom = soonest, **revised 2026-04-24**
post-research-doc):

1. ~~Iced `wgpu::Device` exposure slot (C1)~~ **Landed 2026-04-24**.
2. ~~Runtime-intent routing (`LocationSubmitted` follow-on)~~
   **Landed 2026-04-24** via `HostIntent::CreateNodeAtUrl`.
3. ~~Iced renderer-boot wiring (research)~~ **Researched
   2026-04-24** — closed in upstream iced 0.14; deferred until
   either fork or wgpu version alignment (research doc §5
   Options B/C).
4. **M1 — Middlenet-in-iced** (new top blocker). No upstream
   blockers. Three sessions. Unlocks shipping-quality content
   rendering for the smallnet/middlenet protocol class.
5. C3 prototype via screenshot-loop — independent of Servo
   wgpuification timing (Option A works against any wgpu
   producer). Depends on Servo exposing a wgpu render target
   handle.
6. Servo wgpuification readiness — blocks shared-device interop
   (C3.5 Options B/C) but not C3 prototype.
7. Iced accesskit bridge — blocks C6 only.

**Recommendation** (updated 2026-04-24, post-research): **next up
is M1.1 — port the middlenet block-iterator to iced widgets.**
~1 session, no new dependencies, unlocks the full middlenet-in-iced
capability. After M1, iced is a real spatial browser for the
smallnet/middlenet content class. The Servo wgpu-handoff story
(C3 onward) is independent and proceeds at its own pace.
