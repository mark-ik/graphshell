<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `graphshell-gpu` — Shared GPU Resource Authority

**Date**: 2026-04-20
**Status**: **Skeleton spec / not yet actionable.** Intended to be fleshed out
when trigger conditions fire (see §11). Until then this document captures the
intended shape so code written elsewhere doesn't foreclose design options.
**Scope**: Define the responsibilities, boundaries, core types, key design
decisions, and dependency topology for a future `graphshell-gpu` crate that
serves as the single GPU resource authority for all wgpu consumers in
Graphshell.

**Related docs**:

- [`2026-04-16_middlenet_lane_architecture_spec.md`](2026-04-16_middlenet_lane_architecture_spec.md)
  §4.6 — original sketch of `graphshell-gpu` responsibilities as "shared host
  plumbing"
- [`../implementation_strategy/2026-04-20_middlenet_direct_lane_v1_5_plan.md`](../implementation_strategy/2026-04-20_middlenet_direct_lane_v1_5_plan.md)
  §2.6 — forward-compatible types (`FontHandle`, `ImageRef`,
  `OffscreenTarget`) reserved in `middlenet-render` pending this extraction
- [`../research/2026-04-14_wasm_portable_renderer_feasibility.md`](../research/2026-04-14_wasm_portable_renderer_feasibility.md)
  — WASM envelope GPU constraints (WebGPU vs WebGL2, async init, workers)
- [`../research/2026-04-16_rendering_architecture_vision.md`](../research/2026-04-16_rendering_architecture_vision.md)
  — unifying render pipeline vision
- `design_docs/verso_docs/` — Verso as cross-engine dispatcher (consumer of
  `graphshell-gpu`, not a peer)

---

## 1. Problem Statement

Graphshell has many wgpu consumers. Without a central resource authority, each
independently creates devices, allocates glyph atlases, decodes and caches
images, and makes frame-timing decisions. Consequences:

- **Redundant GPU resources**: duplicate glyph atlases for the same fonts,
  duplicate image decodes, duplicate pipeline state objects.
- **No cross-renderer texture handoff**: when Servo composites a webview into
  a texture and the graph canvas wants that texture as a node-face surface,
  there is no canonical contract — today it requires copying or bespoke
  plumbing.
- **Uncoordinated frame pacing**: each subsystem vsyncs on its own schedule;
  under load, subsystems can starve each other or cause visible tearing at
  handoff boundaries.
- **Duplicated offscreen paint paths**: previews, thumbnails, export surfaces
  each build their own offscreen infrastructure.
- **Fragmented WASM story**: each subsystem has its own "native vs browser"
  branching.

`graphshell-gpu` is the **single GPU resource authority** every wgpu consumer
borrows from. It owns device lifecycle, glyph/image caches, the offscreen
worker pool, frame scheduling, and cross-renderer texture handoff contracts.
It does not own scene-building logic, format decoding, or content semantics.

---

## 2. Core Invariants

### 2.1 One authority, many consumers

There SHALL be exactly one `GpuHost` per Graphshell process. All wgpu
consumers (WebRender fork, vello, iced-wgpu chrome, middlenet-render, Servo
compositor, burn/cubecl) obtain GPU resources from it. No consumer creates
its own `wgpu::Instance` or `Adapter`.

### 2.2 Render vs compute separation

The rendering pipeline and ML compute pipeline MAY use separate `wgpu::Device`
instances so that long-running ML kernels do not block interactive paint
work. They SHOULD share the `wgpu::Instance` and `wgpu::Adapter`.

### 2.3 Resource caches are shared, not duplicated

Glyph atlases, decoded images, and commonly-used pipeline objects SHALL live
in single canonical caches and be referenced by handle. `FontHandle`,
`ImageRef`, and similar handle types are the public currency of resource
sharing; raw `wgpu::Texture` and `Buffer` references SHOULD NOT leak across
consumer boundaries except through explicit handoff contracts (§6.3).

### 2.4 Scene-building is not this crate's concern

`graphshell-gpu` does not know about HTML, SemanticDocument, graph node
layout, ML tensors, or any content domain. It offers primitives; consumers
build scenes.

### 2.5 API shape is identical across native and WASM

Native-only and browser-only differences SHALL be confined to initialization
and worker-pool internals. The public surface consumers see is the same.

---

## 3. Responsibility Boundaries

### 3.1 `graphshell-gpu` owns

- `wgpu::Instance`, `wgpu::Adapter`, `wgpu::Device`(s), `wgpu::Queue`(s)
  lifecycle
- Font registry (shaping engine + glyph atlas) — `FontHandle` is opaque
- Image decode and cache — `ImageRef` is opaque
- Offscreen paint worker pool — scenes in, raster images out
- Frame scheduling / vsync coordination across render subsystems
- Cross-renderer texture handoff contracts — `TextureBridge`
- Surface lifecycle (window and headless) — `SurfaceTarget`
- Capability reporting (features, limits, backend, WASM vs native)

### 3.2 `graphshell-gpu` does NOT own

- Scene-building logic — per-renderer (middlenet-render, WebRender, vello)
- Format decoding — `middlenet-formats`
- Content semantics — `middlenet-core`
- HTTP / transport — `middlenet-transport`, `graphshell-comms`
- Cross-engine dispatch — `verso`
- Shell/app state, workbench, navigator
- ML workload graphs — burn / cubecl own their own compute graphs;
  `graphshell-gpu` only gives them a device

---

## 4. Core Types (sketched)

All types below are **sketches**, not API commitments. Final shapes emerge
during extraction.

### 4.1 `GpuHost`

```rust
pub struct GpuHost {
    pub instance: wgpu::Instance,
    pub adapter:  wgpu::Adapter,

    pub render: GpuRenderContext {
        pub device: Arc<wgpu::Device>,
        pub queue:  Arc<wgpu::Queue>,
    },
    pub compute: Option<GpuComputeContext {
        pub device: Arc<wgpu::Device>,
        pub queue:  Arc<wgpu::Queue>,
    }>,

    pub fonts:     FontRegistry,
    pub images:    ImageCache,
    pub offscreen: OffscreenPool,
    pub scheduler: FrameScheduler,
    pub bridge:    TextureBridge,
    pub capabilities: HostCapabilities,
}
```

`GpuHost` is constructed once at app start (async on WASM, sync on native)
and passed by `Arc<GpuHost>` everywhere.

### 4.2 `FontRegistry`

Owns font loading (via parley / swash), shaping, and glyph atlas management.

```rust
pub struct FontHandle(u64);

impl FontRegistry {
    pub fn register(&self, spec: FontSpec) -> FontHandle;
    pub fn shape(&self, handle: FontHandle, text: &str, size: f32)
        -> ShapedRun;
    pub fn atlas(&self) -> &GlyphAtlas;  // read-only for renderers
}
```

WebRender, vello, iced-wgpu, and middlenet-render all read from the same
glyph atlas. Font collection is shared; re-shaping the same text at the same
size in two renderers is a cache hit.

### 4.3 `ImageCache`

```rust
pub struct ImageRef(u64);

impl ImageCache {
    pub fn decode_async(&self, source: ImageSource)
        -> impl Future<Output = Result<ImageRef, ImageError>>;
    pub fn texture(&self, handle: ImageRef) -> Option<Arc<wgpu::Texture>>;
    pub fn dimensions(&self, handle: ImageRef) -> Option<(u32, u32)>;
}
```

LRU eviction with pinning support. Images decoded once, reused across
renderers and across resolutions (mipmaps on demand).

### 4.4 `OffscreenPool`

```rust
pub trait OffscreenTarget {
    fn paint(&mut self, scene: &RenderScene) -> Result<(), OffscreenError>;
    fn into_image(self) -> Result<RasterImage, OffscreenError>;
}

impl OffscreenPool {
    pub fn submit(&self, job: OffscreenJob)
        -> impl Future<Output = Result<RasterImage, OffscreenError>>;
    pub fn cancel(&self, job_id: OffscreenJobId);
}
```

Worker pool sized to available cores on native; Web Workers + OffscreenCanvas
on WASM. Prioritizable (§5.2): interactive previews beat background
thumbnails.

### 4.5 `FrameScheduler`

```rust
pub enum PaintPriority {
    UserInput,    // triggered by current user gesture
    Animation,    // continuous anim (scroll, physics)
    Background,   // preview/thumbnail work
}

pub trait FramePainter {
    fn paint(&mut self, ctx: FrameContext);
    fn priority(&self) -> PaintPriority;
}

impl FrameScheduler {
    pub fn register(&self, painter: Arc<dyn FramePainter>);
    pub fn request_frame(&self, priority: PaintPriority);
    pub fn present(&self);
}
```

Priority-queue model (§5.2).

### 4.6 `TextureBridge`

Cross-renderer texture handoff. Required so Servo's composited webview can
become a node-face texture on the graph canvas without CPU round-trip.

```rust
pub trait TextureSource {
    fn texture(&self) -> Arc<wgpu::Texture>;
    fn dimensions(&self) -> (u32, u32);
    fn frame_counter(&self) -> u64;  // for invalidation
}

impl TextureBridge {
    pub fn publish(&self, producer_id: ProducerId,
                   source: Arc<dyn TextureSource>);
    pub fn subscribe(&self, producer_id: ProducerId)
        -> BridgeSubscription;
}
```

Zero-copy where the render device is shared; explicit cross-device copy when
it isn't (§5.3).

### 4.7 `HostCapabilities`

```rust
pub struct HostCapabilities {
    pub backend:      wgpu::Backend,
    pub envelope:     Envelope,  // Native | Wasm
    pub features:     wgpu::Features,
    pub limits:       wgpu::Limits,
    pub has_compute_device: bool,
    pub max_texture_size:   u32,
    pub supports_timestamp_queries: bool,
}
```

Surfaced so consumers can gate behavior (e.g., "this node wants compute; is a
compute device available?").

---

## 5. Key Design Decisions

### 5.1 Device topology: one adapter, multiple devices

**Decision (leaning)**: share `wgpu::Instance` and `wgpu::Adapter`; provision
**separate `wgpu::Device`s** for rendering and compute so ML kernels don't
serialize behind interactive paints.

**Why not one device?** wgpu has one queue per device. All rendering and all
ML compute serialize on a single queue → long burn kernels cause visible
jank.

**Why not many devices?** Devices are not free. Cross-device texture sharing
requires explicit export/import and extra synchronization. Keep the device
count small: render (shared by WebRender/vello/iced/middlenet-render) +
compute (shared by burn/cubecl) is the baseline. Additional isolated devices
(e.g., per-Servo-webview) only if a specific reason emerges.

**Open**: Should per-webview Servo compositing get its own device for
isolation? Tentatively no — keep it on the render device, use the texture
bridge for handoff.

### 5.2 Frame scheduling: priority-queue

**Decision (leaning)**: subsystems register as `FramePainter`s with a
`PaintPriority`. Each vsync, the scheduler:

1. drains any `UserInput`-priority paints first (must land this frame),
2. runs `Animation` paints as time allows,
3. yields remaining time to `Background` offscreen work.

**Why not free-running?** Subsystems starving each other under load.

**Why not central beat?** Adds latency to user-input paths when they should
be first in line.

**Open**: Should background offscreen work live in `OffscreenPool` on workers
(off the vsync) entirely, with `FrameScheduler` only coordinating
on-screen paint? Probably yes — cleaner separation.

### 5.3 Texture handoff: zero-copy when possible

**Decision (leaning)**:

- When producer and consumer share the same `wgpu::Device`, hand off
  `Arc<wgpu::Texture>` directly. Zero-copy.
- When they don't (e.g., compute device produces, render device consumes),
  use explicit copy at the bridge; potentially via shared memory where the
  platform supports it.
- Never expose raw textures outside the bridge; always via `TextureSource`
  with a `frame_counter` so consumers can invalidate.

**Why this matters**: determines whether Graphshell can run 40 live webview
nodes smoothly or chokes at 10. The first real use case will force
specifics; design the contract now so it's not retrofitted.

### 5.4 WASM envelope parity

**Decision**: public API identical across native and WASM. Divergence
confined to:

- Initialization: `async fn GpuHost::new_wasm(canvas)` vs
  `fn GpuHost::new_native()`.
- `OffscreenPool` internals: Web Workers + OffscreenCanvas on WASM; OS
  threads on native.
- Image decode: browser-native `createImageBitmap` on WASM; `image` crate
  decoders on native.

**Rationale**: lets the same consumer code (middlenet-render, vello, etc.)
run in both envelopes without cfg-heavy branching.

### 5.5 Font shaping: parley + swash; atlas ownership scoped per path

**Decision (recorded 2026-04-28)**: **parley** for shaping; **swash**
for glyph rasterization on graphshell-gpu-owned paths. Atlas ownership
is **scoped per render path, not global**:

- Direct Lane / vello and any graphshell-gpu-rendered content:
  graphshell-gpu owns the atlas, keyed by
  `(font_id, glyph_id, size_bucket, subpixel_pos)`.
- WebRender path (HTML Lane and any webrender-wgpu-rendered content):
  webrender-wgpu owns the atlas internally; graphshell-gpu does not
  duplicate it. See
  [`../../../../webrender-wgpu/wr-wgpu-notes/2026-04-28_idiomatic_wgsl_pipeline_plan.md`](../../../../webrender-wgpu/wr-wgpu-notes/2026-04-28_idiomatic_wgsl_pipeline_plan.md)
  §10 Q14.

WebRender does not shape — embedders submit pre-shaped glyph runs via
its display-list API — so parley sits *above* webrender-wgpu in the
HTML Lane stack (Stylo → Taffy → Parley → webrender-wgpu) without API
conflict.

**Why not cosmic-text?** Parley is where the Servo/vello ecosystem is
converging. Cosmic-text is nice but diverges from Servo's direction.

### 5.6 Image cache eviction

**Decision (leaning)**: size-bounded LRU with pinning.

- `ImageRef` is refcounted internally; hard references pin.
- Soft references (via a weaker handle type) are evictable.
- Pinning is how live webview thumbnails stay hot; LRU handles historical
  previews.

**Open**: Is a second tier of disk-backed cache warranted, or is reload from
source fast enough? Probably yes for decoded images from network sources.

---

## 6. Dependency Topology

### 6.1 What `graphshell-gpu` depends on

- `wgpu`
- `parley` (text shaping)
- `swash` (glyph rasterization)
- `image` (native image decoding) — feature-gated off in WASM
- `lru` or `moka` (caches)
- `futures` (async traits)

### 6.2 What depends on `graphshell-gpu`

| Crate / component | Uses |
|---|---|
| WebRender fork | render `Device`/`Queue`, glyph atlas, image cache |
| vello integration | render `Device`/`Queue`, glyph atlas, image cache |
| iced-wgpu chrome | render `Device`/`Queue`, fonts |
| `middlenet-render` | `FontHandle`, `ImageRef`, `OffscreenTarget`, glyph atlas for direct scene painting |
| Servo compositor (in Verso) | render `Device`, `TextureBridge` (publishes webview textures) |
| `verso` | reads `HostCapabilities` for engine selection |
| Graph canvas backend | render `Device`, glyph atlas, image cache, `TextureBridge` (consumes webview textures for node faces) |
| `burn` / `cubecl` | compute `Device` only; not the full host |

### 6.3 Dependency boundary rules

- `graphshell-gpu` MUST NOT depend on any Graphshell content or app crate
  (no `middlenet-*`, no `verso-*`, no shell, no workbench).
- `graphshell-gpu` MUST NOT depend on format decoders or parsers.
- Consumers MUST NOT bypass `graphshell-gpu` to create their own devices.
- `TextureBridge` is the only sanctioned path for cross-renderer texture
  sharing.

---

## 7. Interactions with Other Subsystems

### 7.1 WebRender fork

WebRender currently assumes device ownership. Extracting
`graphshell-gpu` requires a fork-side refactor: WebRender takes an external
`Arc<Device>`, shares the glyph atlas with `graphshell-gpu`, and publishes
its composited output through `TextureBridge` where applicable.

This is the largest single piece of work in the extraction; it dominates the
timeline.

### 7.2 vello

Vello already accepts external devices. Integration is small: construct
vello's `Renderer` from `host.render.device.clone()` and wire its text path
through `FontRegistry`.

### 7.3 iced-wgpu

iced-wgpu has its own setup path but accepts external devices in some
configurations. Integration is moderate — mostly about sharing the font
registry so iced's chrome and document content don't maintain parallel
glyph atlases.

### 7.4 middlenet-render

In v1.5 this crate owns `FontHandle`, `ImageRef`, `OffscreenTarget` as
opaque placeholders. On `graphshell-gpu` extraction these types move to
`graphshell-gpu` and `middlenet-render` re-exports or depends on them.
This is the cheapest migration of any consumer — the whole point of
reserving those types in v1.5.

### 7.5 Servo compositor (within Verso)

Servo publishes composited webview output via `TextureBridge::publish`.
Graph canvas subscribes to render those textures as node faces. This is the
primary use case that forces `TextureBridge`'s design.

### 7.6 Verso

`verso` is a content-dispatch layer, not a GPU layer. It reads
`HostCapabilities` to decide engine routing (e.g., "Servo lane requires
compute device for WebGPU content" → fall back to Wry if absent). It does
not own any GPU resources.

### 7.7 burn / cubecl

ML crates get the compute `Device` only. They do not participate in frame
scheduling. The compute device and render device may share the adapter but
not the queue — ML work happens in parallel with paint.

---

## 8. Cross-Cutting Concerns

### 8.1 Error handling

GPU errors (device loss, out-of-memory, unsupported feature) surface through
typed errors: `DeviceLost`, `AllocationFailed`, `FeatureUnsupported`. Device
loss triggers a recreation protocol: `GpuHost` rebuilds, all consumers
reinitialize via a `HostRebuildEvent`.

### 8.2 Diagnostics

`graphshell-gpu` MUST expose a diagnostics channel emitting: device memory
pressure, cache hit rates, offscreen queue depth, frame scheduler decisions,
texture bridge subscription counts. Plumbs into Graphshell's existing
`DiagnosticChannelDescriptor` system (see shell guidelines).

### 8.3 Testing

- Unit tests for `FontRegistry` shaping cache, `ImageCache` LRU, frame
  scheduler priority math.
- Integration tests with a headless wgpu backend verifying device sharing
  and texture bridge contracts.
- WASM envelope tests deferred until the WASM envelope is building.

### 8.4 Performance budgets

The crate SHOULD hold GPU work submission under:

- Frame submission latency: p99 < 2ms from `request_frame` to queue submit
  (native).
- Glyph shaping cache hit: p99 < 100μs.
- Image decode (cached): p99 < 50μs.
- Offscreen paint start latency: p99 < 10ms.

Concrete numbers land when real measurements exist; these are placeholders.

---

## 9. WASM Envelope Notes

In browser WASM:

- `wgpu::Instance` maps to WebGPU (preferred) or WebGL2 (fallback).
- `GpuHost::new_wasm(canvas)` is async; awaits adapter and device.
- No threads on wasm32-unknown-unknown default profile. `OffscreenPool` uses
  Web Workers with OffscreenCanvas; image decode uses browser APIs via
  wasm-bindgen.
- `FrameScheduler` ties to `requestAnimationFrame` instead of OS vsync.
- `TextureBridge` zero-copy works on WebGPU (shared `GPUTexture`); on WebGL2
  fallback it degrades to explicit copy.
- All of this is confined to `graphshell-gpu`; consumers see the same API.

---

## 10. Out of Scope

- **Specific scene-building logic** for any renderer — stays in that
  renderer's crate.
- **Format decoding** — `middlenet-formats`.
- **Content semantics** — `middlenet-core`.
- **Cross-engine content dispatch** — `verso`.
- **ML workload graphs** — burn / cubecl own these.
- **HTTP / transport** — `graphshell-comms`, `middlenet-transport`.
- **Shell / workbench / navigator state** — stays in shell crates.
- **Shader authoring IR** — the rust-gpu → SPIR-V → naga pipeline is its own
  concern; `graphshell-gpu` consumes compiled shaders, doesn't own the
  pipeline that produces them.

---

## 11. Trigger Conditions (when to make this real)

This skeleton becomes an actionable plan when any of the following fire:

1. **Cross-surface drift observed.** In v1.5 Step 6, observation cards,
   hover previews, search snippets, and feed tiles each maintain independent
   font/image state AND measurable duplication emerges (VRAM, CPU decode
   cost).
2. **WebRender fork accepts external device.** The fork stabilizes enough
   that taking an `Arc<wgpu::Device>` from outside is a small patch rather
   than a fight.
3. **First cross-renderer texture handoff use case lands.** Most likely
   Servo webview → graph node face. This use case forces the `TextureBridge`
   design decisions to concrete specifics.
4. **ML workload contention.** Burn kernels start visibly blocking paint,
   making the render/compute device split non-optional.
5. **WASM envelope work starts.** Needing a single "native vs browser" seam
   for GPU resources rather than per-consumer branching.

Until then, v1.5's forward-compatible types (`FontHandle`, `ImageRef`,
`OffscreenTarget` in `middlenet-render`) carry enough design intent to keep
the option open without premature extraction.

---

## 12. Open Questions

- **Per-webview Servo device?** Tentatively no (use render device + texture
  bridge). Revisit if Servo's compositor contention with other renderers is
  observable.
- **Disk-backed image cache tier?** Probably yes for network-sourced images;
  deferred decision.
- **Diagnostic surface depth.** How much GPU introspection does the shell
  expose to users vs reserve for development-only diagnostics?
- **Feature negotiation failure.** What does `graphshell-gpu` do when an
  adapter lacks features a consumer requires? Degrade the consumer
  (middlenet-render drops backdrop-filter), refuse to init, or warn and
  continue?
- **Threading model for offscreen pool on native.** Rayon, tokio
  blocking pool, custom? Probably tokio for consistency with transport.
- **Pipeline cache.** Does `graphshell-gpu` own a cross-renderer pipeline
  cache, or does each renderer manage its own? Leaning toward per-renderer
  since pipelines are renderer-specific.

---

## 13. Relationship to v1.5

v1.5 explicitly **does not** extract `graphshell-gpu`. What v1.5 does:

- Reserves `FontHandle`, `ImageRef`, `OffscreenTarget` as opaque types in
  `middlenet-render`.
- Routes all middlenet-render surface reuse through these types.
- Keeps the integration surface small enough that extraction later is a
  move-and-re-export, not a rewrite.

This spec exists so that decisions made during v1.5 don't accidentally close
off options this extraction will need — especially the device-topology
question (§5.1) and the texture-handoff contract (§5.3).

---

## 14. Follow-on Work (after this skeleton becomes a plan)

- `2026-XX-XX_graphshell_gpu_extraction_plan.md` — ordered extraction
  milestone (concrete crate split, WebRender fork refactor, consumer
  migration sequence).
- `2026-XX-XX_texture_bridge_contract_spec.md` — detailed texture handoff
  semantics once the first real use case lands.
- `2026-XX-XX_gpu_diagnostics_spec.md` — diagnostics surface design.
- WASM envelope enablement plan (depends on WASM-target work in other
  subsystems).
