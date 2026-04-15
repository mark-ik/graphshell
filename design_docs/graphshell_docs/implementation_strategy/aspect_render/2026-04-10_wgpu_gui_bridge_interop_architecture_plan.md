# wgpu-gui-bridge — Broader Interop Architecture Plan

**Date**: 2026-04-10
**Status**: Planning
**Scope**: Decoupling the GL import in `wgpu-gui-bridge` from surfman and enabling the
Windows platform path. This crate bridges GL-rendered content (currently Servo via surfman)
into host-owned wgpu textures and is a dependency of the graphshell Servo integration.

**Related**:

- [render_backend_contract_spec.md](render_backend_contract_spec.md) — backend contract; §13 covers WgpuHal
- [gl_to_wgpu_plan.md](gl_to_wgpu_plan.md) — compositor migration strategy
- [2026-04-12_rendering_pipeline_status_quo_plan.md](2026-04-12_rendering_pipeline_status_quo_plan.md) — current rendering pipeline reality

---

## Context

`wgpu-gui-bridge` embeds GL-rendered content into host-owned wgpu textures. Currently
handles two platform paths:

- **Linux**: GL FBO → Vulkan external memory FD → wgpu::Texture (RGBA8Unorm)
- **Apple**: Surfman surface → IOSurface → Metal texture (BGRA8Unorm) → normalize to RGBA8Unorm

**Problem**: The GL import logic is coupled to surfman. A non-Servo GL app (game engine,
video decoder, CAD viewport) can't use this without bringing surfman along. The Windows
path is unimplemented.

**Connection to WebRender wgpu work**: When Servo eventually renders via wgpu natively, the
interop crate becomes unnecessary for the Servo case — the adapter would just embed Servo
directly (same device, same queue, no cross-API boundary). Until then, this bridge serves
both Servo and any other GL producer.

**Goal**: Decouple the GL import from surfman so any GL producer can use it, and enable the
Windows platform path.

---

## Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Host wgpu Application                     │
│  (demo-servo-winit, egui app, iced app, game engine, etc.)  │
└──────────────────────┬──────────────────────────────────────┘
                       │ ImportedTexture / wgpu::Texture
                       │
┌──────────────────────┴──────────────────────────────────────┐
│              wgpu-native-texture-interop                      │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Layer 1: Raw Import Primitives (no surfman dependency) │  │
│  │                                                        │  │
│  │  import_gl_fbo_vulkan()   — Linux/Android, raw GL FBO  │  │
│  │  import_gl_fbo_win32()    — Windows, NT handle path    │  │
│  │  import_iosurface_metal() — Apple, raw IOSurfaceRef    │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Layer 2: Producer Adapters (feature-gated)             │  │
│  │                                                        │  │
│  │  SurfmanFrameProducer  — surfman surface lifecycle     │  │
│  │  RawGlFrameProducer    — any GL app with an FBO        │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                       │
┌──────────────────────┴──────────────────────────────────────┐
│  servo-wgpu-interop-adapter (unchanged API, uses Layer 2)    │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Decouple GL Import from Surfman

### What's Coupled Today

In `surfman_gl/linux.rs`, `import_current_frame()` interleaves two concerns:

**Surfman lifecycle** (lines 22-36, 214-219):
`device.unbind_surface_from_context()`, `device.make_context_current()`,
`device.surface_info(&surface).framebuffer_object`, `device.bind_surface_to_context()`

**Generic GL→Vulkan import** (lines 37-212): Create Vulkan image with `OPAQUE_FD` external
memory, export FD, import into GL via `GL_EXT_memory_object_fd`, create GL texture from
external memory, blit from source FBO, wrap Vulkan image as wgpu texture.

Same pattern on Apple: `metal.rs` mixes surfman surface ops with generic IOSurface→Metal
wrapping.

### Refactoring: Extract Layer 1

```rust
// New: wgpu-native-texture-interop/src/raw_gl/linux.rs
pub fn import_gl_framebuffer_vulkan(
    gl: &glow::Context,
    gl_proc_loader: &dyn Fn(&str) -> *const std::ffi::c_void,
    source_fbo: u32,
    size: PhysicalSize<u32>,
    host: &HostWgpuContext,
    options: &ImportOptions,
) -> Result<ImportedTexture, InteropError>

// New: wgpu-native-texture-interop/src/raw_gl/metal.rs
pub fn import_iosurface_metal(
    iosurface: &IOSurfaceRef,
    size: PhysicalSize<u32>,
    host: &HostWgpuContext,
    options: &ImportOptions,
) -> Result<ImportedTexture, InteropError>
```

The surfman adapter becomes a thin wrapper calling into `raw_gl`.

### New RawGlFrameProducer

```rust
pub struct RawGlFrameSource {
    pub gl: Arc<glow::Context>,
    pub gl_proc_loader: Box<dyn Fn(&str) -> *const std::ffi::c_void>,
    pub source_fbo: u32,
    pub size: PhysicalSize<u32>,
}

impl FrameProducer for RawGlFrameProducer {
    fn acquire_frame(&mut self) -> Result<NativeFrame, InteropError> { ... }
}
```

### File Changes

| File | Change |
|------|--------|
| `src/raw_gl/mod.rs` | New module, re-exports platform functions |
| `src/raw_gl/linux.rs` | Extracted Vulkan FD import from `surfman_gl/linux.rs` |
| `src/raw_gl/metal.rs` | Extracted IOSurface import + normalizer |
| `src/raw_gl/texture_normalizer.rs` | Moved from `surfman_gl/metal/texture_importer.rs` |
| `src/surfman_gl/linux.rs` | Thin wrapper calling `raw_gl::linux` |
| `src/surfman_gl/metal/metal.rs` | Thin wrapper calling `raw_gl::metal` |
| `src/lib.rs` | New `pub mod raw_gl` (no feature gate) |
| `Cargo.toml` | `surfman` feature gates only `surfman_gl`; `raw_gl` depends on `glow` + platform deps |

---

## Phase 2: Windows Import Path

### Two Sub-Paths

**2a: GL → NT Handle → Vulkan → wgpu** (when wgpu uses Vulkan on Windows)
Nearly identical to Linux FD path — replace `OPAQUE_FD` with `OPAQUE_WIN32`, `ImportMemoryFdEXT` with `ImportMemoryWin32HandleEXT`. ash already supports `khr::external_memory_win32`.

**2b: GL → NT Handle → DX12 → wgpu** (default wgpu on Windows)
Create DX12 committed resource with `D3D12_HEAP_FLAG_SHARED`, export NT handle, import into GL via `GL_EXT_memory_object_win32`, wrap DX12 resource as wgpu texture. Requires `windows` crate.

**Recommendation**: Start with **2a** (Vulkan on Windows) — reuses 95% of Linux code with
handle type swaps. Defer **2b** (DX12 native).

### Key Differences from Linux FD Path

```rust
// Linux:
vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD
ash::khr::external_memory_fd::Device
gl::ImportMemoryFdEXT(memory_object, size, handle_type, fd)

// Windows:
vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32
ash::khr::external_memory_win32::Device
gl::ImportMemoryWin32HandleEXT(memory_object, size, handle_type, handle)
```

### Phase 2 File Changes

| File | Change |
|------|--------|
| `src/raw_gl/windows.rs` | New — Vulkan NT handle import path |
| `Cargo.toml` | Windows target dep: `ash` (already cross-platform) |
| `src/raw_gl/mod.rs` | `#[cfg(target_os = "windows")] mod windows;` |
| `src/lib.rs` | Update `CapabilityMatrix::for_backend` for Vulkan-on-Windows |
| `src/surfman_gl/mod.rs` | Add Windows dispatch to `GlFramebufferSourceImpl` |

---

## Phase 3: Demo and Testing

New `demo-raw-gl`: minimal demo showing a non-Servo GL app using the raw import path
(create GL context via glutin, render triangle to GL FBO, import via `RawGlFrameProducer`,
present in winit window). Validates Phase 1 and demonstrates broader applicability.

Add to `docs/testing.md`: Windows Vulkan path validation, raw GL producer demo.

---

## Implementation Order

1. **Phase 1** — structural refactoring; extract `raw_gl` module, keep surfman adapter working.
2. **Phase 2a** — Windows Vulkan; extends `raw_gl` with new platform file.
3. **Phase 3** — demo; validates raw GL path end-to-end.
4. **Phase 2b** — Windows DX12; deferred until DX12 is the only available backend.

---

## Risk Assessment

- **Low (Phase 1)**: Pure refactoring — no behavior change. Existing tests validate.
- **Medium (Phase 2a)**: New platform code; Vulkan NT handle path well-documented but
  untested until Windows+Vulkan runtime test. GL extension bindings already generated.
- **Medium (Phase 3)**: Requires non-surfman GL context (glutin or raw EGL). Dependency
  decision needed.

---

## Long-Term Relationship to WebRender wgpu Backend

When the WebRender wgpu backend reaches production quality, this bridge becomes unnecessary
for the Servo case — Servo renders via wgpu on the host device, and the adapter exposes the
output texture directly with no cross-API boundary. At that point,
`servo-wgpu-interop-adapter` simplifies to a thin embedding API. The bridge crate still
serves non-Servo GL producers (video decoders, legacy GL apps) where the cross-API boundary
is real and unavoidable.
