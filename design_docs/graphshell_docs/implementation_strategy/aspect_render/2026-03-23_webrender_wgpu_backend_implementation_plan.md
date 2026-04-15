<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# wgpu Backend for WebRender — Implementation Plan

**Date**: 2026-03-23
**Status**: Design / planning
**Scope**: Adding a `wgpu` compile-time-selectable alternative to WebRender's existing
OpenGL (`gleam`) backend. Covers feature flags, device abstraction, shader translation,
bind group layout, render pass restructuring, swapchain integration, phased rollout,
and proof-of-correctness strategy.

**Related**:

- [render_backend_contract_spec.md](render_backend_contract_spec.md) — backend contract; §13 covers WgpuHal variant (escalatory wrapper approach)
- [gl_to_wgpu_plan.md](gl_to_wgpu_plan.md) — compositor GL→wgpu redesign (different scope: compositor seams, not WebRender internals)
- [`../../research/2026-03-01_webrender_wgpu_renderer_research.md`](../../research/2026-03-01_webrender_wgpu_renderer_research.md) — WebRender wgpu renderer research and feasibility
- Servo issue #37149

---

## 1. Feature Flag Design

`webrender/Cargo.toml`:

```toml
[features]
default = ["gl_backend"]
gl_backend = ["dep:gleam", "dep:surfman"]
wgpu_backend = ["dep:wgpu", "dep:wgpu-types"]
```

Enforce mutual exclusivity in `build.rs`:

```rust
fn main() {
    let gl = cfg!(feature = "gl_backend");
    let wgpu = cfg!(feature = "wgpu_backend");
    assert!(gl ^ wgpu, "exactly one backend must be enabled");
}
```

Propagate in Servo's `Cargo.toml`:

```toml
[features]
default = ["webrender/gl_backend"]
wgpu = ["webrender/wgpu_backend"]
```

Build: `cargo build --no-default-features --features wgpu`

---

## 2. Device Abstraction Layer

`webrender/src/device/` — extract a `GpuDevice` trait:

```rust
pub trait GpuDevice {
    fn create_texture(&mut self, desc: TextureDescriptor) -> TextureId;
    fn upload_texture(&mut self, id: TextureId, rect: DeviceIntRect, data: &[u8]);
    fn delete_texture(&mut self, id: TextureId);
    fn create_vbo(&mut self) -> VboId;
    fn upload_vbo<T: bytemuck::Pod>(&mut self, id: VboId, data: &[T]);
    fn create_program(&mut self, name: &str, features: &str) -> ProgramId;
    fn begin_frame(&mut self);
    fn bind_draw_target(&mut self, target: DrawTarget);
    fn draw_triangles_u32(&mut self, first: i32, count: i32);
    fn end_frame(&mut self);
    fn read_pixels_rgba8(&mut self, rect: DeviceIntRect) -> Vec<u8>; // for tests
}
```

Monomorphization (zero-cost, no vtable):

```rust
#[cfg(feature = "gl_backend")]
use device::gl::GlDevice as ActiveDevice;
#[cfg(feature = "wgpu_backend")]
use device::wgpu::WgpuDevice as ActiveDevice;
```

---

## 3. Shader Translation

WR shaders are GLSL. wgpu expects WGSL.

**Recommended: naga in `build.rs`** (naga is already in the wgpu ecosystem):

```rust
fn translate_shader(glsl_path: &Path, stage: naga::ShaderStage) -> String {
    let src = std::fs::read_to_string(glsl_path).unwrap();
    let module = naga::front::glsl::Frontend::default()
        .parse(&naga::front::glsl::Options::from(stage), &src)
        .expect("GLSL parse failed");
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator.validate(&module).unwrap();
    naga::back::wgsl::write_string(
        &module, &info, naga::back::wgsl::WriterFlags::empty()
    ).unwrap()
}
```

Embed translated WGSL via `include_str!` on generated files. Commit generated
WGSL to repo for human reviewability.

**WR-specific GLSL complications:**

- `#define`-based feature flags → expand before feeding to naga
- `gl_FragCoord` → naga maps to `@builtin(position)`
- `textureLod` / `textureGrad` → verify naga emits correct WGSL intrinsics
- Flat interpolation qualifiers → `@interpolate(flat)`
- `#extension GL_ARB_gpu_shader5` → strip/replace before naga ingests

---

## 4. Bind Group Layout

| Binding | Content | Update frequency |
|---------|---------|-----------------|
| Group 0, binding 0 | Frame-level uniforms (transform, device pixel ratio) | Once per frame |
| Group 0, binding 1 | Render task data (atlas coords) | Per render task |
| Group 1, binding 0..N | Texture atlas samplers | Per batch |
| Group 2, binding 0 | Per-primitive data (storage buffer) | Per batch |

The primitive data buffer replaces WR's `TEX_SAMPLER_PrimitiveHeadersI/F` texture
trick — use `wgpu::BufferBindingType::Storage` instead.

---

## 5. Render Pass Restructuring

WR's render pass concept maps to `wgpu::RenderPass`. Each WR render pass → one
`wgpu::RenderPassDescriptor`.

Refactor render loop from:

```
begin_frame() → [bind_draw_target; draw*; bind_draw_target; draw*] → end_frame()
```

to:

```
begin_frame() → [begin_pass → draw* → end_pass] → submit()
```

wgpu render passes are RAII-scoped by `'encoder` lifetime — mechanical but
touches many call sites in `renderer.rs`.

---

## 6. Swapchain / Surface Integration

WR acquires the framebuffer implicitly via GL's default framebuffer. With wgpu,
surface management is explicit.

Servo uses `surfman` for GL surfaces. For wgpu, use `wgpu::Surface` directly:

```rust
let surface = instance.create_surface(&window)?;
let config = wgpu::SurfaceConfiguration {
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    format: surface.get_capabilities(&adapter).formats[0],
    width, height,
    present_mode: wgpu::PresentMode::Fifo,
    ..Default::default()
};
surface.configure(&device, &config);
```

Wrap in `WgpuSurface` that mirrors the `surfman::Surface` interface so the
windowing layer stays unchanged.

---

## 7. Implementation Phases

**Phase 1 — Infrastructure**

- Feature flags, `build.rs` enforcement
- Extract `GpuDevice` trait; `GlDevice` implements it; all tests still pass
- `WgpuDevice` stub (creates instance/adapter/device/queue; draw calls are no-ops)

**Phase 2 — Shader pipeline**

- Build script: translate all WR shaders via naga; fail build on any failure
- Create `wgpu::RenderPipeline` for each shader; verify pipeline creation succeeds
- Fix naga translation gaps shader by shader

**Phase 3 — Core rendering primitives**

- Solid color rectangles
- Image rendering (texture sampling)
- Render target management (ping-pong textures for effects)
- Goal: static webpage renders correctly

**Phase 4 — Full batch type coverage**

- Gradients (linear, radial, conic)
- Clip masks (stencil → depth/stencil in wgpu)
- Blur passes
- Mix-blend-mode compositing
- Border rendering
- Text (glyph atlas upload, subpixel AA)

**Phase 5 — Integration & CI**

- Wire into Servo's `--features wgpu` build path
- Add backend selection to `mach build`
- Run existing reftests against both backends in CI
- Performance profiling

---

## 8. Proving Correct Operation to Reviewers

### 8.1 Pixel-exact reference comparison

WR has `Renderer::save_capture`. Use it to dump frame outputs from both backends
on the same display list:

```
cargo run --example frame_capture --features gl_backend -- capture/gl/
cargo run --example frame_capture --features wgpu_backend -- capture/wgpu/
python tools/compare_captures.py capture/gl/ capture/wgpu/
```

Threshold: 1 LSB per channel (allows rounding differences). Post results in PR body.

### 8.2 WPT pass rate

Run Servo's WPT suite against both backends; report pass rate delta. No regression
allowed. Document known divergences (e.g., subpixel AA differences due to sRGB
framebuffer handling).

### 8.3 WR reftest harness

`webrender/tests/` snapshot reftests — parameterize runner to run once per enabled
backend, or gate with `#[cfg(feature = "wgpu_backend")]`.

### 8.4 Per-operation unit tests

Test the device layer in isolation using headless wgpu:

```rust
#[test]
fn wgpu_texture_roundtrip() {
    let mut dev = WgpuDevice::new_headless();
    let id = dev.create_texture(TextureDescriptor { width: 4, height: 4, format: RGBA8 });
    let data: Vec<u8> = (0..64).collect();
    dev.upload_texture(id, DeviceIntRect::new(0,0,4,4), &data);
    let readback = dev.read_pixels_rgba8(DeviceIntRect::new(0,0,4,4));
    assert_eq!(readback, data);
}
```

wgpu readback is async — use `device.poll(wgpu::Maintain::Wait)` to block for
test sync.

### 8.5 Criterion perf benchmark

Render a fixed display list N times; report frame time vs GL. Expectation: within
10% on same hardware; strictly better on Metal (macOS) and DX12 (Windows).

### 8.6 Validation layers

Always enabled in debug builds:

- wgpu built-in validation (default in debug)
- `WGPU_BACKEND=vulkan` + Vulkan validation layers in CI

### 8.7 CI matrix

```yaml
strategy:
  matrix:
    include:
      - os: ubuntu-latest
        features: gl_backend
        backend: gl
      - os: ubuntu-latest
        features: wgpu_backend
        backend: vulkan   # lavapipe software Vulkan
      - os: macos-latest
        features: wgpu_backend
        backend: metal
      - os: windows-latest
        features: wgpu_backend
        backend: dx12
```

Use `lavapipe` (Mesa's software Vulkan) on Linux CI to avoid needing a real GPU.

---

## 9. Key Files to Touch

| File | Change |
|------|--------|
| `webrender/Cargo.toml` | Feature flags, conditional deps |
| `webrender/build.rs` | Shader translation, feature enforcement |
| `webrender/src/device/mod.rs` | Extract `GpuDevice` trait |
| `webrender/src/device/gl.rs` | `GlDevice` impl (rename/refactor existing) |
| `webrender/src/device/wgpu.rs` | New `WgpuDevice` impl |
| `webrender/src/renderer/mod.rs` | Parameterize over `ActiveDevice` |
| `webrender/src/renderer/upload.rs` | Texture upload path per backend |
| `webrender/shaders/*.glsl` | No changes; serve as translation source |
| `webrender/shaders/*.wgsl` | Generated; committed for reviewability |
| `servo/components/servo/Cargo.toml` | Propagate `wgpu` feature |
| `.github/workflows/` | Extend CI matrix |

---

## 10. Non-obvious Risks

1. **GLSL extensions** — naga's GLSL frontend may not handle
   `#extension GL_ARB_gpu_shader5`; preprocess to strip/replace before naga.
2. **Stencil clipping** — WR uses stencil buffer for clip regions; wgpu stencil
   API is more explicit. Translate stencil op sequences carefully.
3. **sRGB framebuffer differences** — `wgpu::TextureFormat::Bgra8UnormSrgb` applies
   gamma on write; GL's `GL_FRAMEBUFFER_SRGB` behaves differently. Pin format
   explicitly and document.
4. **Async readback** — wgpu buffer mapping is async; use
   `device.poll(Maintain::Wait)` in tests.
5. **`gleam` transitive deps** — some WR consumers may link `gleam`
   unconditionally; audit full dep tree when `gl_backend` is disabled.

---

## 11. Landing Checklist

- [ ] Feature flags compile-enforced; CI passes for both backends
- [ ] All existing WR reftests pass with `gl_backend` (no regression)
- [ ] Pixel comparison tool exists and output posted in PR body
- [ ] WPT pass rate documented for wgpu backend
- [ ] WGSL shaders committed to repo (human-reviewable)
- [ ] Headless wgpu unit tests for texture roundtrip, basic draw
- [ ] CI matrix covers Linux/Vulkan (lavapipe), macOS/Metal, Windows/DX12
- [ ] Validation layers clean (no wgpu errors in debug CI)
- [ ] Criterion benchmark posted showing perf comparison
