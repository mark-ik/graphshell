# Render Backend Contract Spec

**Date**: 2026-03-01
**Status**: Canonical interaction contract
**Priority**: Active — C+F policy in effect

**Related**:
- `ASPECT_RENDER.md`
- `frame_assembly_and_compositor_spec.md`
- `../../TERMINOLOGY.md` — `CompositorAdapter`, `TileRenderMode`, `BackendContentBridgeMode`
- `../2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`
- `../2026-03-01_webrender_readiness_gate_feature_guardrails.md`
- `../research/2026-03-01_webrender_wgpu_renderer_research.md`
- `2026-03-01_webrender_wgpu_renderer_implementation_plan.md`
- `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.6, 3.7)):
- **OpenTelemetry Semantic Conventions** — all diagnostics channels in §7 follow OTel naming and severity conventions
- **OSGi R8** — backend capability probe and mode selection follow OSGi capability registration vocabulary

---

## 1. Purpose and Scope

This spec defines the canonical contract for the render backend abstraction layer in
Graphshell: what the backend boundary owns, what crosses it, what must never cross it, and
what the active and planned backend implementations must satisfy.

It covers:

- the backend bridge mode enum and selection policy,
- the capability probe contract,
- the content callback and texture handoff interfaces,
- GL state isolation requirements (Glow path),
- wgpu texture handoff requirements (wgpu path),
- diagnostics obligations per mode,
- fallback routing rules,
- Glow retirement conditions,
- acceptance criteria.

This spec covers the **backend abstraction contract**. Pass ordering, compositor pass
structure, and `TileRenderMode` dispatch are covered in `frame_assembly_and_compositor_spec.md`.
The phased execution plan for the wgpu renderer is in
`2026-03-01_webrender_wgpu_renderer_implementation_plan.md`.

---

## 2. Backend Ownership Boundary

### 2.1 What the backend boundary owns

- the `render_backend` module is the sole ownership boundary for content-bridge mode
  selection — no other module may select or switch the active backend mode,
- the `BackendContentBridgeMode` enum and its variants,
- the `BackendContentBridgeCapabilities` capability probe,
- the `BackendCallbackFn` / `BackendGraphicsContext` type aliases,
- all GL state save/restore operations on the Glow path,
- all wgpu texture handoff operations on the wgpu path,
- the `UiRenderBackend` type alias (egui rendering backend handle),
- diagnostics channel emission for bridge-mode selection, capability results, and
  per-frame handoff timing.

### 2.2 What must never cross the backend boundary

- direct GL callback shim or `glow::Context` references in workbench or UI modules
  — those are backend implementation details, not embedder APIs,
- direct `wgpu::Device`, `wgpu::Queue`, or `wgpu::Texture` references in workbench or UI
  modules — the backend boundary mediates all GPU resource exposure,
- backend selection logic in `CompositorAdapter`, `tile_compositor`, or any non-backend
  module — bridge mode is always resolved through `render_backend`.

---

## 3. Backend Bridge Mode Enum

```text
BackendContentBridgeMode =
  | GlCallback
    -- current production content bridge; Servo parent-render callback; GL state must be
       saved/restored by CompositorAdapter around every content callback
  | WgpuPreferredFallbackGlCallback
    -- future path; wgpu texture handoff is primary; GlCallback activates
       when wgpu interop capability is absent
```

### 3.1 Mode selection policy

- `active_backend_content_bridge_policy()` is the sole function that determines the active
  mode at runtime.
- The function currently returns `GlCallback` unconditionally (the GL callback bridge remains the
  production content path for the current milestone).
- The function will return `WgpuPreferredFallbackGlCallback` only after readiness gates
  G1–G5 in the readiness gate document are all closed with linked tracker evidence.
- No code outside `render_backend` may call `active_backend_content_bridge_policy()` and
  then act on the result directly — all callers must go through the bridge selection helpers
  exported by `render_backend`.

### 3.2 Mode invariants

**Invariant**: The active mode is determined once per app startup (or once per device
reinit). It is not recalculated per-frame or per-tile. Mode changes require a documented
reinit or restart path.

**Invariant**: `WgpuPreferredFallbackGlCallback` is not a permanent dual-path mode. It
is a migration bridge. The GL path within `WgpuPreferredFallbackGlCallback` must be
retired once wgpu + fallback make it redundant for all supported targets.

---

## 4. Capability Probe Contract

`BackendContentBridgeCapabilities` is a struct that holds the result of probing whether
the runtime environment supports the preferred backend mode.

### 4.1 Probe interface

```text
BackendContentBridgeCapabilities {
    wgpu_interop_available: bool,
    -- true iff: wgpu device initialized, Servo WebRender wgpu backend active,
    --           shared device handoff succeeded, zero-copy texture path available
    gl_fallback_available: bool,
    -- true iff: the GL parent-render callback path is initialized and usable
    --           on any platform Graphshell supports)
    probe_diagnostics_channel: DiagnosticChannelId,
    -- the channel on which probe results are emitted
}
```

### 4.2 Probe timing

- The capability probe runs once at backend initialization, before the first frame.
- The probe result is cached for the session lifetime.
- If the environment changes (e.g. GPU device loss + reinit), the probe re-runs and the
  result is re-evaluated.

### 4.3 Probe invariants

**Invariant**: The probe must never panic. Capability unavailability is a valid result, not
an error.

**Invariant**: The probe result for `wgpu_interop_available` must accurately reflect
whether the zero-copy wgpu texture handoff path works for the current device and platform.
Returning `true` when the path would fail at frame time is a correctness bug.

---

## 5. GL Callback Path Contract (Current Production)

### 5.1 Content callback interface

On the GL callback path, the `CompositorAdapter` invokes Servo's `render_to_parent` callback
through the backend callback shim owned by `render_backend`.

No caller outside `render_backend` or `compositor_adapter` may construct or invoke a raw
backend callback shim.

### 5.2 GL state isolation contract

Before invoking the content callback:

- The current scissor box, viewport rect, blend enable, active texture unit, and bound
  framebuffer are saved by `CompositorAdapter`.

After the content callback returns:

- All saved GL state is restored to exactly the pre-callback values.
- If restoration fails for any reason, `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` is emitted
  at `Error` severity and the frame is completed with a fallback render path.

**Invariant**: The content callback (Servo `render_to_parent`) must not permanently change
any GL state that was not owned by the callback's render scope. Transient state changes
during rendering are acceptable; leaked state after return is a correctness bug.

**Invariant**: GL state save/restore must cover at minimum: scissor box, viewport,
blend enable, active texture unit, bound framebuffer. The chaos mode diagnostics gate
verifies these six invariants in diagnostics-enabled builds.

### 5.3 Chaos mode

In diagnostics-gated builds, `CompositorAdapter` operates in chaos mode:

- GL state is sampled before and after every content callback.
- Any deviation between pre- and post-callback state that was not explicitly allowed by
  the compositor contract is emitted as `CHANNEL_COMPOSITOR_CHAOS_FAIL` at `Error` severity.
- Clean passes emit `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS` at `Info` severity.

Chaos mode must not be active in production builds (it adds per-callback GPU readback cost).

---

## 6. wgpu Path Contract (Planned — Behind Readiness Gates)

This section defines the contract that the wgpu path must satisfy when it becomes active.
It is not yet production policy. The wgpu path is gated by readiness gates G1–G5.

### 6.1 Compositor output texture interface

On the wgpu path, WebRender's compositor output is a `wgpu::Texture` owned by the shared
device. The `render_backend` module exposes:

```text
CompositorOutputTexture {
    view: wgpu::TextureView,
    dimensions: (u32, u32),
    format: wgpu::TextureFormat,
    generation: u64,  -- increments every frame; used to detect stale references
}
```

The `CompositorAdapter`, on the wgpu path, binds `view` directly into the `egui_wgpu`
paint callback without a copy. The GL save/restore machinery is not required (wgpu command
encoder scoping provides structural isolation).

### 6.2 Shared device ownership

- Graphshell owns the `wgpu::Instance`, `Adapter`, `Device`, and `Queue`.
- `egui_wgpu::Renderer` receives the `wgpu::Device` reference at initialization.
- Servo/WebRender receives the same `wgpu::Device` reference at initialization.
- All GPU resources allocated by WebRender's wgpu renderer are allocated on the shared
  device.

**Invariant**: There must be exactly one `wgpu::Device` active in any Graphshell session.
Two devices on the same process are a resource management hazard. If shared device
initialization fails, the wgpu path falls back to Glow via `glow_fallback_available`.

### 6.3 Texture pool contract

The backend maintains a pool of pre-allocated compositor output textures to avoid
per-frame allocation:

- Pool size: at least 2 textures per active `CompositedTexture` tile (double-buffering).
- On tile resize: the pool entry is re-created at the new dimensions; frames during
  re-creation use a placeholder tile.
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_HIT` emitted at `Info` when pool rotation succeeds.
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_MISS` emitted at `Warn` when a new allocation is
  required mid-session (allocation succeeded but was outside the pool budget).

### 6.4 wgpu isolation invariants

The wgpu path provides structural isolation by construction:

- Each WebRender frame is encoded in its own `wgpu::CommandEncoder`.
- Each Graphshell egui_wgpu frame is encoded in its own `wgpu::CommandEncoder`.
- The two encoders do not share mutable GPU state.
- Chaos mode on the wgpu path verifies that no bind group or pipeline state set inside
  WebRender's encoder is visible inside Graphshell's encoder.

---

## 7. Diagnostics Obligations

All channels follow the Graphshell diagnostics channel schema.

### 7.1 Backend selection channels

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_BACKEND_MODE_SELECTED` | `Info` | Active bridge mode determined at startup |
| `CHANNEL_BACKEND_CAPABILITY_PROBE` | `Info` | Capability probe result recorded |
| `CHANNEL_BACKEND_WGPU_INTEROP_UNAVAILABLE` | `Warn` | wgpu interop probed as unavailable; Glow fallback active |

### 7.2 Glow path channels

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` | `Error` | GL state leaked across content callback |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS` | `Info` | Chaos mode: no state violation detected |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL` | `Error` | Chaos mode: state violation detected |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE` | `Info` | Sampled Glow content callback latency in microseconds |

### 7.3 wgpu path channels (active when wgpu path selected)

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_COMPOSITOR_WGPU_HANDOFF_US_SAMPLE` | `Info` | Sampled wgpu texture handoff latency in microseconds |
| `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_HIT` | `Info` | Pool texture rotation succeeded |
| `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_MISS` | `Warn` | Pool allocation was required outside budget |

**Invariant**: Equivalent diagnostic coverage must exist on both paths before the wgpu path
is promoted to production. Parity is a readiness gate (G5).

---

## 8. Fallback Routing Rules

| Scenario | Active mode | Fallback result |
|----------|-------------|----------------|
| wgpu interop probe returns false | `WgpuPreferredFallbackGlCallback` | GL path activates; `CHANNEL_BACKEND_WGPU_INTEROP_UNAVAILABLE` emitted |
| GL context lost | `GlCallback` | Frame skipped; `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` emitted; recovery per GPU surface lifecycle spec |
| wgpu device lost | `WgpuPreferredFallbackGlCallback` | GL path activates if available; `Error` diagnostic emitted; reinit attempted |
| Both paths unavailable | any | Compositor output is `Placeholder`; `Error` diagnostic emitted |

**Invariant**: A fallback activation must never be silent. At minimum one `Warn` or `Error`
diagnostic must be emitted each time the primary path degrades to a fallback.

---

## 9. GL Callback Retirement Conditions

The GL callback path may be retired (removed from the production codebase) only when all of the
following are true, each with linked tracker evidence:

1. Compositor replay diagnostics parity: wgpu path emits equivalent diagnostic coverage to
  the GL baseline on the same compositor scenarios.
2. No open stabilization regressions tied to pass-order, callback-state isolation, or
  overlay affordance visibility that are GL-path-specific.
3. Fallback path behavior validated in at least one non-interop environment (e.g. a device
   where `wgpu_interop_available` returns false).
4. All required pass-contract scenarios covered by wgpu-primary + fallback-safe paths with
   tracker-linked evidence.
5. One full release cycle on `WgpuPreferredFallbackGlCallback` with no GL-path
   activations reported from the production distribution.

Until all five conditions are met, the Glow path remains a first-class code path, not a
deprecated compatibility shim.

---

## 10. Feature Guardrails (Effective for All New Feature Work)

Any new feature slice touching rendering or composition must comply with:

1. **No new renderer-specific coupling in UI/workflow code** — feature code must consume
   backend contracts, not backend internals (no direct `glow::Context` or `wgpu::Device`
   outside `render_backend`).
2. **Bridge metadata preservation** — any slice touching content-pass wiring must preserve
   bridge-path and bridge-mode diagnostic observability.
3. **Fallback-safe behavior** — new behavior must define what happens when the preferred
   rendering capability is unavailable.
4. **Receipt-linked evidence** — migration-adjacent feature slices must post tracker
   evidence proving guardrail compliance.

---

## 11. Acceptance Criteria

1. `render_backend` is the sole module that selects or switches backend bridge mode.
2. No backend-specific callback or GPU types escape the backend boundary into workbench or UI modules.
3. GL state save/restore covers scissor, viewport, blend, active texture unit, and bound
  framebuffer on the GL callback path.
4. Capability probe accurately reflects wgpu interop availability; false positives are
   correctness bugs.
5. All diagnostics channels defined in §7 are registered and emitting.
6. Fallback activations are never silent.
7. Glow path retirement requires all five conditions in §9 with linked evidence.
8. Feature guardrails in §10 are enforced for all new render-adjacent slices.

---

## 12. Servo Rendering Backend: RenderingBackendBinding Cleanup Plan

*Upstream work that feeds into the graphshell rendering backend contract.*

The wgpu rendering pipeline already works end-to-end (Painter branches on
`SERVO_WGPU_BACKEND` env var, `RenderingContext` trait has `wgpu_device()` /
`wgpu_queue()` / `wgpu_hal_device_factory()`, GL ops gated behind `!use_wgpu`).
The following work adds architectural cleanup and a zero-copy render path.

### 12.1 RenderingBackendBinding Enum

Replace env-var detection and separate `wgpu_device()`/`wgpu_queue()` methods
with an explicit sum type in `components/shared/paint/rendering_context.rs`:

```rust
pub struct GlBinding {
    pub gleam_gl: Rc<dyn gleam::gl::Gl>,
    pub glow_gl: Arc<glow::Context>,
}

#[cfg(feature = "wgpu_backend")]
pub struct WgpuBinding {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

pub enum RenderingBackendBinding {
    Gl(GlBinding),
    #[cfg(feature = "wgpu_backend")]
    Wgpu(WgpuBinding),
}
```

Add `fn backend_binding(&self) -> RenderingBackendBinding` to the trait. Painter
switches from `use_wgpu` bool to matching on this enum. Remove `SERVO_WGPU_BACKEND`
env var detection. Existing `wgpu_device()` / `wgpu_queue()` can be deprecated once
all consumers use the enum.

### 12.2 Promote WgpuRenderingContext to Shared Crate

Move `WgpuRenderingContext` from `examples/wgpu-embedder/` into
`components/shared/paint/wgpu_rendering_context.rs` (gated behind `wgpu_backend`).
Extend it to own the surface and support frame acquisition:

```rust
pub struct WgpuRenderingContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: RefCell<wgpu::SurfaceConfiguration>,
    size: Cell<PhysicalSize<u32>>,
    current_frame: RefCell<Option<wgpu::SurfaceTexture>>,
}
```

Implements `RenderingContext`:

- `backend_binding()` → `Wgpu(WgpuBinding { device.clone(), queue.clone() })`
- `acquire_wgpu_frame_target()` → gets surface texture, stores it, returns TextureView
- `present()` → presents the stored SurfaceTexture
- `resize()` → reconfigures surface
- `read_to_image()` → GPU→CPU readback via staging buffer

### 12.3 Zero-Copy Render via render_to_view()

The current pipeline uses `composite_output()` + host blit (extra GPU copy). Switch to
`render_to_view()` for zero-copy in `components/paint/painter.rs`:

```rust
// wgpu path: acquire frame target from context, render directly into it
if let Some(frame_view) = self.rendering_context.acquire_wgpu_frame_target() {
    if let Some(renderer) = self.webrender_renderer.as_mut() {
        let size = self.rendering_context.size2d().to_i32();
        renderer.render_to_view(frame_view, size, self.frame_id);
    }
    self.rendering_context.present();
}
```

Eliminates the blit pipeline, blit shader, and intermediate texture sample.

### 12.4 WebGL External Image Stubs

On wgpu path, `webrender_external_images.rs` returns stub/no-op for WebGL external
images. Temporary — wgpu-gui-bridge provides real GL→wgpu interop (see §13).

### 12.5 Clean Up Painter wgpu Gating

Replace `if !self.use_wgpu { ... }` throughout with match on backend binding. Remove
the `use_wgpu: bool` field.

### 12.6 Key Files

| File | Change |
|------|--------|
| `components/shared/paint/rendering_context.rs` | `RenderingBackendBinding` enum, `acquire_wgpu_frame_target()` |
| `components/shared/paint/wgpu_rendering_context.rs` | NEW — promoted from example, surface-owning |
| `components/paint/painter.rs` | Match on enum, `render_to_view()`, remove `use_wgpu` bool |
| `components/paint/webrender_external_images.rs` | Stub for wgpu path |
| `examples/wgpu-embedder/src/main.rs` | Simplify to use shared `WgpuRenderingContext` |

---

## 13. WebRender wgpu-hal Backend Extension: WgpuHal Variant

### 13.1 Architecture Decision: Extension, Not a Separate Backend

`WgpuHal` should be an extension of `WgpuShared`, not a separate rendering backend.

The WebRender wgpu rendering pipeline (shaders, pipelines, GPU cache, texture cache) is
**100% identical** for all wgpu paths. From `renderer/init.rs:407–430`, both `Wgpu` and
`WgpuShared` route to the same `create_webrender_instance_wgpu()` function via `WgpuInit`
variants. Rendering diverges only at device/queue creation, surface management, and output
access. A separate backend would duplicate ~3000 lines for zero gain.

The escalatory wrapper model:

```
RendererBackend::Gl          → GL path (unchanged)
RendererBackend::Wgpu        → wgpu path, WebRender owns device
RendererBackend::WgpuShared  → wgpu path, host owns device (wgpu::Device)
RendererBackend::WgpuHal     → wgpu path, host owns raw hal device
                                ↑ wraps hal→wgpu internally, routes to WgpuDevice
```

### 13.2 What wgpu-hal Enables (beyond WgpuShared)

| Capability | wgpu-hal method | Use case |
|---|---|---|
| Wrap raw hal device | `Adapter::create_device_from_hal(hal_device)` | Share device with host without two separate stacks |
| Get raw texture handle | `Texture::as_hal::<A>()` → VkImage / MTLTexture | Zero-copy embed in native render pass |
| Inject Vulkan semaphores | `Queue::as_hal::<Vulkan>()` → `add_signal_semaphore()` | Sync WebRender completion with native Vulkan queue |
| Wrap raw texture in wgpu | `Device::create_texture_from_hal()` | Host pre-allocates render target |

### 13.3 WgpuHal as Factory-Based Variant (Preferred)

```rust
#[cfg(feature = "wgpu_backend")]
WgpuHal {
    device_factory: Box<dyn FnOnce() -> (wgpu::Device, wgpu::Queue) + Send>,
}
```

In `create_webrender_instance_with_backend()`:

```rust
if let RendererBackend::WgpuHal { device_factory } = backend {
    let (device, queue) = device_factory();
    return create_webrender_instance_wgpu(
        notifier, options,
        WgpuInit::SharedDevice { device, queue }
    );
}
```

The host provides a closure calling `adapter.create_device_from_hal(hal_device, &desc)`
internally. WebRender never needs to be generic over `A: HalApi`.

### 13.4 Raw Output Texture Access

Add to `Renderer` in `webrender/src/renderer/mod.rs`:

```rust
pub unsafe fn composite_output_hal<A: wgpu::wgc::hal_api::HalApi>(
    &self
) -> Option<impl std::ops::Deref<Target = A::Texture>> {
    self.composite_output()?.texture.as_hal::<A>()
}
```

### 13.5 Files to Modify

| File | Change |
|---|---|
| `webrender/src/renderer/init.rs` | Add `WgpuHal` variant; route through `WgpuInit::SharedDevice` |
| `webrender/src/renderer/mod.rs` | Add `composite_output_hal<A>()` generic accessor |
| `webrender/examples/wgpu_hal_device.rs` | New demo: hal device → WgpuHal → render → verify |

**Files NOT changed**: All wgpu rendering code (`wgpu_device.rs`, pipelines, shaders) —
unchanged. The entire rendering path is reused.

### 13.6 Scope Boundary

**In scope**: `WgpuHal` variant with factory closure, `composite_output_hal<A>()`, demo.

**Deferred**: Semaphore injection (Vulkan-only), `Device::create_texture_from_hal()`
integration, Servo-side `RenderingContext` extension to expose hal device factory.
