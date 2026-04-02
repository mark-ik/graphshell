# WebRender wgpu Renderer — Research: Spec, Feasibility, QA, and Upstreaming

**Date**: 2026-03-01
**Status**: Research
**Author**: Arc
**Feeds Into**:
- `implementation_strategy/2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`
- `implementation_strategy/2026-03-01_webrender_readiness_gate_feature_guardrails.md`
- `implementation_strategy/aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`
- `research/2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md`

**External Reference**: Wu Yu-Wei, "WebRender wgpu", https://wusyong.github.io/posts/webrender-wgpu/

---

## 1. Problem Statement

Graphshell embeds Servo as its web rendering engine. Servo's GPU path runs through WebRender,
which uses an OpenGL backend. WebRender's GL dependency creates three concrete problems for
Graphshell's target stack:

1. **Compositor state coupling**: The `CompositorAdapter` must save and restore GL state
   around every Servo render callback. This is the chaos mode the current codebase instruments
   with diagnostics. The root cause is shared GL context ownership, not a Graphshell design
   flaw.

2. **wgpu migration blocker**: Graphshell's own renderer is being migrated from `egui_glow`
   to `egui_wgpu` (tracked in `#183`, gated by `#180`). Issue `#180` is specifically:
   "prove the runtime-viewer GL → wgpu bridge." A WebRender wgpu backend would solve `#180`
   cleanly: if Servo's compositor path emits a `wgpu::Texture` instead of a GL texture,
   the bridge becomes a zero-copy or near-zero-copy wgpu texture handoff, not a cross-API
   copy.

3. **WebGPU buffer throughput**: WebGPU content (canvas elements, GPU worklets) currently
   copies data through the GL intermediary before compositing. A wgpu-native WebRender path
   eliminates this copy.

The blog post by Wu Yu-Wei (a Servo/Tauri contributor) is the primary public design
document proposing to address this at the WebRender level.

---

## 2. Approaches Evaluated in the Blog Post

The post evaluates three paths. This section restates them with Graphshell-specific analysis.

### 2.1 Extend ANGLE Support

**What it does**: Use Mozilla's `mozangle` fork (an ANGLE wrapper) to translate GL calls to
DX12/Metal/Vulkan/wgpu at the driver level. Servo already has an `mozangle` integration.

**Why rejected for Graphshell**:
- Adds ~20 MB binary on Windows (the primary Graphshell development platform).
- Does not remove the GL API surface — Graphshell's compositor adapter still manages GL
  state save/restore; chaos mode still needs to run.
- Does not close `#180`: the bridge is still a GL texture, not a wgpu texture.
- ANGLE is a translation layer, not an architecture improvement.

**Verdict**: Not suitable as a strategic path. Acceptable only as a temporary compatibility
shim on platforms where wgpu lacks a stable backend (e.g., headless CI without a real GPU).

### 2.2 Implement WebRender Compositor Trait

**What it does**: WebRender has an internal `Compositor` trait intended to abstract the
final compositing step. Implementing this trait could allow wgpu to handle the final blit
while WebRender keeps its GL internal rendering.

**Why rejected**:
- WebRender's `Device` and `Renderer` structs still call OpenGL directly for all rendering
  work. Only the final compositor step is abstracted. The GL dependency is not removed;
  it is just patched over at the seam.
- Graphshell's chaos mode problem (shared GL state) exists in the render path, not only in
  the composite step.
- Does not close `#180` cleanly: the texture produced is still GL-origin.

**Verdict**: Potentially useful for a narrow intermediate step, but not the strategic answer.

### 2.3 New wgpu Renderer for WebRender (Recommended)

**What it does**: Implement a parallel `Device` + `Renderer` pair inside WebRender using
wgpu, keeping the existing GL path available behind a feature flag. Shader translation is
required. The rest of WebRender (scene builder, display list, rasterization logic) is
backend-agnostic and requires no changes.

**Why this is the correct long-term path for Graphshell**:
- Closes `#180` definitively: Servo can produce a `wgpu::Texture` at the compositor
  boundary, enabling zero-copy (or bindless) handoff into Graphshell's egui_wgpu frame.
- Removes the need for GL state save/restore in `CompositorAdapter` — chaos mode
  diagnostics can be retired or repurposed.
- Aligns Servo with the wgpu/Rust-native graphics ecosystem that Graphshell is already
  targeting with `egui_wgpu`.
- Enables true zero-copy WebGPU canvas content composition.
- Scoped work: the post notes that GL usage in WebRender is bounded to `Device` and
  `Renderer` only, within ~200k lines. Everything above those structs is backend-agnostic.

---

## 3. Implementation Specification

This section describes the implementation as a spec, not a Graphshell execution plan.
Graphshell's readiness gates (`#180`, `#183`, `#90`) still apply before any of this work
enters mainline.

### 3.1 Scope Boundary

WebRender's GL surface is bounded to two structs:

| Struct | Responsibility |
|--------|---------------|
| `Device` | GPU resource management: textures, buffers, framebuffers, shader programs, VAOs, draw calls |
| `Renderer` | Frame orchestration: pass scheduling, render target management, batching, blitting |

Everything above these (scene builder, display list encoder, glyph rasterizer, image cache,
clip chain compiler) is backend-agnostic. A wgpu `Device` and `Renderer` can be introduced
as parallel implementations without touching the scene build path.

### 3.2 Data Flow Target State

```
WebContent (DOM/CSS/Canvas)
    ↓
Scene Builder (unchanged)
    ↓
Display List (unchanged)
    ↓
[wgpu Device + Renderer]  ← new
    ↓
wgpu::Texture (compositor output)
    ↓
[Graphshell CompositorAdapter — wgpu path]
    ↓
egui_wgpu frame pass (zero-copy texture bind)
    ↓
Screen
```

Compare to current state:
```
...
[GL Device + Renderer]
    ↓
GL texture (compositor output)
    ↓
[Graphshell CompositorAdapter — GL path, save/restore state]
    ↓
egui_glow frame pass (GL texture blit)
    ↓
Screen
```

### 3.3 wgpu Device Layer

The wgpu `Device` struct must implement equivalents for WebRender's current GL operations:

**Texture management**
- `wgpu::Texture` / `wgpu::TextureView` in place of GL textures and renderbuffers.
- Atlas textures (glyph, image) as `wgpu::Texture` with `write_texture` for updates.
- External image import: GL path uses `EXT_image_external`; wgpu path should use
  `wgpu::TextureDescriptor` with `hal::ExternalImageDescriptor` for platform-native
  buffer import (required for zero-copy video and WebGPU canvas integration).

**Buffer management**
- `wgpu::Buffer` in place of GL VBOs. WebRender uses large instance arrays; these map
  directly to wgpu vertex buffers with `COPY_DST | VERTEX` usage.
- Uniform data: wgpu uses `wgpu::BindGroup`; WebRender's GL path uses UBOs. Mapping is
  direct.

**Framebuffer management**
- Replace GL framebuffers with `wgpu::RenderPass` targeting `wgpu::TextureView`.
- Intermediate render targets become `wgpu::Texture` with `RENDER_ATTACHMENT | TEXTURE_BINDING`.

### 3.4 wgpu Renderer Layer

**Shader translation**

WebRender's shaders are written in GLSL. Three translation options:

| Option | Mechanism | Notes |
|--------|-----------|-------|
| **naga** | WebRender shaders compiled through `naga` GLSL → WGSL or SPIR-V at build time | Best option: naga is wgpu's native shader compiler; zero runtime cost; catches errors at compile time |
| **spirv-cross** | GLSL → SPIR-V via `shaderc` at build time, consumed by wgpu | Already used by some WebRender paths; additional dependency |
| **Dynamic** | GLSL → naga at runtime | Acceptable for bring-up but not for production |

Recommendation: use `naga` as the build-time translation target. WebRender's shader set is
described as small ("only a few shaders in Servo context"), making manual review of
translated output tractable.

**Render pass structure**

WebRender's GL renderer uses a multi-pass architecture (shadow pass, opaque pass, alpha
pass, composite pass). Each maps naturally to a `wgpu::RenderPass`. The key adaptation:

- GL path uses framebuffer attachment switching between passes (expensive on tile-based GPUs).
- wgpu path can use `wgpu::RenderPassDescriptor` with explicit load/store ops, enabling
  better optimization on Apple Silicon and mobile GPUs.

**Command encoding**

WebRender batches draw calls per-frame. The wgpu equivalent is a `wgpu::CommandEncoder`
per frame, with `begin_render_pass` / `end_render_pass` blocks. This is a direct mapping.

### 3.5 Compositor Output Interface

The critical interface change for Graphshell is the compositor output contract.

**Current (GL)**: WebRender writes to an FBO; Graphshell reads a GL texture handle.

**Target (wgpu)**: WebRender writes to a `wgpu::Texture`; Graphshell receives a
`wgpu::TextureView` (or a `wgpu::BindGroup` wrapping one) as the compositor output.

This texture handoff must be mediated by shared `wgpu::Device` ownership. There are two
ownership models:

| Model | Graphshell owns `wgpu::Device` | egui_wgpu owns `wgpu::Device` |
|-------|-------------------------------|-------------------------------|
| Servo attachment | Servo receives an externally-created wgpu device handle | Servo must accept an externally-provided device (requires Servo API change) |
| Resource sharing | Graphshell allocates textures, passes them to Servo | Graphshell's graph canvas and the Servo compositor share the same device naturally |
| Recommended | ✅ Preferred for Graphshell | Not recommended — device lifetime owned by egui_wgpu internals |

**Recommendation**: Graphshell owns the `wgpu::Instance`, `Adapter`, `Device`, and `Queue`.
Servo receives a device handle at initialization. This aligns with the research document
`2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md` §3.2 (GPU ownership).

---

## 4. Feasibility Analysis

### 4.1 Scope Risk

**Low risk** (backend-agnostic WebRender layers are large and stable):
- Scene builder, display list, glyph rasterizer, image cache: no changes required.
- Existing GL path remains fully functional in parallel.

**Medium risk** (translation surface, not correctness hazard):
- Shader translation via naga: GLSL → WGSL is well-supported but requires manual validation
  of translated shaders, especially for custom WebRender blend modes and the alpha pass.
- External image import for WebGPU canvas: platform-specific (`wgpu::hal` + `VkImage` or
  `MTLTexture`); requires per-platform implementation. This is a known wgpu limitation.

**High risk** (novel integration surface):
- Shared `wgpu::Device` lifetime with Servo: requires a Servo API addition (or patch) to
  accept an externally-provided wgpu device handle. This is the deepest Servo coupling
  change and requires upstream coordination (see §6).
- `wgpu::hal` external image import: not all wgpu backends support this at the same API
  level. Metal and Vulkan have good support; DX12 is more limited. The zero-copy WebGPU
  canvas path requires per-platform validation.

### 4.2 Dependency Risk

| Dependency | Current state | Risk |
|-----------|--------------|------|
| `wgpu` version in Servo | Servo uses wgpu; version pinned in Servo `Cargo.lock` | Version skew between Servo's wgpu and Graphshell's egui_wgpu must be resolved. This is `#183` G1 (dependency control). |
| `naga` version | naga is bundled with wgpu; same version concern | Resolved if wgpu versions are unified. |
| WebRender in Servo | WebRender is a Servo internal crate; it is not independently published | Patching requires either a Servo fork/patch or upstreaming. |
| `egui_wgpu` | Graphshell's target renderer backend | Must share `wgpu::Device` with the Servo-side path. |

**Critical**: the wgpu version alignment between Servo's WebRender dependency and
Graphshell's `egui_wgpu` dependency is a hard prerequisite. This must be validated in a
spike before any integration work begins (aligns with readiness gate G1).

### 4.3 Platform Feasibility

| Platform | wgpu backend | External image import | Zero-copy path |
|----------|-------------|----------------------|----------------|
| Windows (DX12) | ✅ Stable | Partial (`DXGI_SHARED_HANDLE`) | Medium confidence |
| macOS (Metal) | ✅ Stable | ✅ (`MTLTexture` IOSurface) | High confidence |
| Linux (Vulkan) | ✅ Stable | ✅ (`VkImage` DMABuf) | High confidence |
| Linux (OpenGL fallback) | Via naga/GLSL | N/A | Requires GL→wgpu copy |

Graphshell's current milestone targets Windows as the primary platform. Windows DX12 has
partial external image support; a copy path may be required until DX12 support matures in
wgpu. This should be treated as a fallback, not a blocker.

### 4.4 Binary Size Impact

Replacing ANGLE (which is currently a concern in the blog post) is not relevant for
Graphshell's direct path, since Graphshell does not use ANGLE today. However, adding wgpu
as a direct Graphshell dependency alongside Servo's wgpu would not increase binary size if
they share a version (Cargo deduplication). Version skew would add ~5–10 MB per distinct
wgpu version. This is another argument for dependency unification.

---

## 5. Quality Assurance Strategy

This section defines what "correct" means for a wgpu WebRender renderer from Graphshell's
perspective. It is input to the readiness gates defined in
`2026-03-01_webrender_readiness_gate_feature_guardrails.md`.

### 5.1 Compositor Parity (maps to gates G2, G3, G5)

**Definition of correct**: the wgpu compositor path must produce pixel-equivalent output
to the GL path for a defined reference set.

| Test category | Method | Acceptance criterion |
|--------------|--------|---------------------|
| Static page render | Capture GL output and wgpu output for a fixed URL at a fixed viewport. Compare pixel diff. | ≤0.5% pixel difference (allowing for float precision variance) |
| Glyph rendering | Render a text-heavy page; compare glyph atlas and final pixel output | Fonts must be visually equivalent; hinting differences are acceptable |
| CSS compositing | Render pages with opacity, filter, mix-blend-mode; compare output | No missing layers; blend modes must be visually equivalent |
| WebGPU canvas | Render a canvas element using WebGPU; verify the canvas pixels appear in the compositor output | Zero-copy path must produce correct pixels |
| Focus ring pass-order | Verify overlay affordances (Pass 3) appear over web content (Pass 2) on wgpu path | Pixel inspection: ring must occlude content pixels at expected positions |

### 5.2 CompositorAdapter Invariants (maps to gate G3)

The chaos mode diagnostics in `shell/desktop/workbench/compositor_adapter.rs` currently
test GL state isolation invariants. On the wgpu path:

- GL save/restore is replaced by wgpu command encoder scoping (inherently isolated).
- The chaos mode should be adapted to verify that the `wgpu::RenderPass` covering the
  Servo compositor output does not leak state into Graphshell's own egui_wgpu render pass.
- Specifically: texture view bindings, bind groups, and pipeline state set in Servo's
  WebRender command buffer must not affect Graphshell's egui_wgpu command buffer.
- wgpu's encoder model makes this isolation structural rather than heuristic — this is
  a net improvement over the GL path.

Chaos mode should evolve, not be retired, to cover the new invariants.

### 5.3 Frame Budget (maps to viewer backend research agenda §Performance Boundary)

The existing frame budget target is `≤16 ms total`, `≤4 ms per active tile`.

For the wgpu path, the key measurement points are:

| Measurement | Method | Target |
|-------------|--------|--------|
| Servo wgpu render time | wgpu timestamp queries on WebRender command encoder | ≤4 ms per active tile at 1080p |
| Texture handoff latency | Time from WebRender `queue.submit()` to Graphshell texture bind | ≤0.5 ms (same-device shared texture should be near-zero) |
| Resize handling | Trigger tile rect changes every frame; measure texture recreation cost | No frame drops on tile resize; texture pool should pre-allocate |
| Device contention | Run graph canvas wgpu pass + Servo wgpu pass simultaneously | No deadlock; no measurable throughput regression |

These targets feed directly into issue `#180`'s "measured evidence" requirement.

### 5.4 Fallback Testing (maps to gate G4 and C+F policy)

The C+F policy requires the GL fallback path to remain valid during wgpu bring-up.

| Test | Condition | Expected behavior |
|------|-----------|-----------------|
| GL fallback on wgpu capability miss | wgpu adapter lacks required features | Servo falls back to GL renderer; Graphshell uses `GlowCallback` bridge mode |
| Mixed-mode stability | GL bridge active for Servo; wgpu active for egui | Both paths share no state; no rendering corruption |
| Capability probe accuracy | `BackendContentBridgeCapabilities` probe runs | Returns `false` for wgpu interop when wgpu Servo renderer is not available |

### 5.5 Replay Diagnostics Parity (maps to gate G5)

Graphshell's compositor replay diagnostics (channel `CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE`)
currently sample the GL bridge callback duration. The wgpu path must emit equivalent
diagnostics. Channels to be added or adapted:

- `CHANNEL_COMPOSITOR_WGPU_HANDOFF_US_SAMPLE` — texture handoff time from Servo to egui_wgpu
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_HIT` — whether handoff used a pooled texture
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_MISS` — whether a new texture had to be allocated

These feed the diagnostics parity gate in `2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`.

---

## 6. Upstreaming Strategy

A wgpu renderer for WebRender is not a Graphshell-internal patch. It is a capability that
Servo needs and that benefits all Servo embedders. This section describes the upstreaming
model.

### 6.1 Relationship to Servo Upstream

WebRender is developed as a component of Servo. The blog post (by a Servo/Tauri contributor)
represents existing upstream intent for this direction. Graphshell's interests align exactly
with upstream goals.

**Recommended upstreaming model:**

1. **Fork-and-patch for spike work**: For Graphshell's initial spike proving `#180`, use a
   local Cargo patch against a Servo fork branch. This is already the pattern documented in
   `2026-03-01_webrender_readiness_gate_feature_guardrails.md` (G1: local patch path
   demonstrated and documented).

2. **Coordinate with active upstream contributors**: The blog post demonstrates that Wu
   Yu-Wei is actively working on this direction. Graphshell should monitor and contribute
   to the upstream Servo/WebRender tracking issue for the wgpu renderer rather than
   implementing in isolation.

3. **Upstream-first for WebRender internals**: Changes to `Device` and `Renderer` in
   WebRender should be submitted upstream. Graphshell-specific integration (shared device
   ownership model, texture handoff API) should be proposed as a Servo embedding API
   addition.

4. **Graphshell-specific layer stays in Graphshell**: The `CompositorAdapter` wgpu path,
   the `BackendContentBridgeMode::WgpuPreferredFallbackGlowCallback` selection logic, and
   the diagnostics channels are Graphshell-owned. These do not need upstream acceptance.

### 6.2 Upstream API Changes Required

To support Graphshell's shared-device ownership model, Servo needs a new embedding API:

```rust
// Proposed Servo embedding API addition (not yet proposed upstream)
// Allows an embedder to provide an externally-created wgpu device
// so that compositor output textures can be shared zero-copy.
pub fn initialize_with_wgpu_device(
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    adapter_info: wgpu::AdapterInfo,
) -> Result<(), EmbedderError>;
```

This API would be:
- Optional: Servo falls back to creating its own device if not provided.
- Validated upstream as a general embedding concern, not Graphshell-specific.
- Proposed after Graphshell's spike proves the model works (evidence-first upstreaming).

### 6.3 Upstreaming Risk and Fallback

**Risk**: Upstream Servo or WebRender moves in a different direction (e.g., Vello-based
rasterizer instead of wgpu-native WebRender). This is a real possibility given the active
state of Rust graphics ecosystem exploration.

**Mitigation**: Graphshell's C+F contract means the GL path remains valid indefinitely.
The wgpu path is additive, not destructive. If upstream chooses a different path (e.g.,
Servo adopts Vello for rasterization), Graphshell's bridge contract adapts to whatever
compositor API Servo exposes — the `BackendContentBridgeMode` abstraction is designed for
exactly this flexibility.

**Do not upstream prematurely**: The spike (proving `#180`) should produce local evidence
before any upstream proposal. An upstream PR without measured correctness evidence is
counterproductive.

### 6.4 Community Coordination Points

| Party | Coordination action |
|-------|-------------------|
| Servo project | Monitor WebRender wgpu tracking issue; align with Wu Yu-Wei's work; don't duplicate effort |
| egui_wgpu (emilk/egui) | Shared device ownership is a general concern for embedders; `egui_wgpu::winit::Painter` currently creates its own device. Propose extension or use `egui_wgpu::Renderer` directly. |
| wgpu project | External image import capabilities; report any platform gaps encountered in Graphshell's spike work |

---

## 7. Graphshell Execution Sequencing

This research feeds three active planning documents. The sequencing relative to those is:

### 7.1 What this research does NOT authorize

- Starting the wgpu WebRender renderer implementation now.
- Switching the Servo compositor path off GL today.
- Changing `BackendContentBridgeMode` behavior before `#180` has measured evidence.

Readiness gates G1–G5 in `2026-03-01_webrender_readiness_gate_feature_guardrails.md` remain
the authoritative switch conditions.

### 7.2 What this research authorizes

- **Bounded spike for `#180`**: Build exactly one composited Servo viewer surface presented
  in a wgpu-backed frame, using a Cargo patch on a Servo fork. Measure the metrics defined
  in §5.3. Post results to `#180`.

- **Dependency version audit**: Determine the current wgpu version used by Servo's
  WebRender and by the target `egui_wgpu` release. Document whether they are
  compatible. This is G1 evidence.

- **Upstream coordination**: Check the Servo/WebRender repository for any existing wgpu
  renderer PR or tracking issue. Do not start work that duplicates in-flight upstream
  effort.

### 7.3 Relationship to C+F Policy

The C+F policy in `2026-03-01_backend_bridge_contract_c_plus_f_receipt.md` says:

> Glow remains temporary baseline infrastructure only while wgpu bridge parity is proven.

This research document provides the technical spec for what "wgpu bridge parity" means at
the WebRender level. The QA strategy in §5 is the definition of "proven."

---

## 8. Upstream Community State (2026-03-01 Reconnaissance)

This section records the state of upstream efforts as of March 2026, resolving research
Q2 and informing the implementation strategy. It also updates the implementation plan
with findings about the wgpu vs wgpu-hal choice and shader translation direction.

### 8.1 jdm's byo-renderer Branch

**Location**: `jdm/servo:byo-renderer`
**Last activity**: 2025-07-22 (comment update); last commits 2025-06-25. Not abandoned in
intent; not active in practice.

jdm extracted a `Renderer` trait in `components/compositing/render.rs` covering the
full compositor-to-WebRender boundary (18 methods). `ServoBuilder::renderer()` allows
embedder injection. **This is a compositor-level seam, not a GPU backend seam.**

The key limitation: `send_transaction(transaction: Transaction)` — any alternative backend
must still speak the full `webrender_api::Transaction` vocabulary. The trait does not
abstract the GL `Device`/`Renderer` layer at all. GL-specific methods (`assert_no_gl_error`,
`gl_info`, `assert_gl_framebuffer_complete`) would need stub implementations.

The branch targets WebRender 0.67 git. Current Servo main uses 0.68 crates.io. The API
delta between 0.67 and 0.68 is small (PR#4878 crates.io prep, minor API adjustments).
The trait design is sound and the 0.67→0.68 forward-port is estimated at one day of work.

**Actionable**: jdm's 4–5 commits can be cherry-picked or reproduced against current
Servo main. This gives Graphshell the compositor-level injection point (`ServoBuilder::renderer()`)
needed for the P8 integration spike, without waiting for jdm's PR to upstream.

### 8.2 Mozilla's Authoritative Direction: wgpu-hal, Not wgpu

@nical (Mozilla, WebRender lead) posted the following on servo/servo#37149 (2025-06-02):

> *"We plan to add a **wgpu-hal backend** to WebRender besides the existing GL one. Over
> time the GL backend will hopefully be phased out but it will take a long time. We do
> not plan to use `wgpu` or even `wgpu-core` in WebRender, but instead use **`wgpu-hal`
> directly**. WebRender has a lot of shader code written in GLSL. The source of truth
> that is checked into the repository must be GLSL in the foreseeable future. Adding
> SPIRV/DXIL/etc. as a build step from the GLSL is fine, but rewriting WebRender's
> shaders in another language would come with too many complications.
> This work is gated on WebGPU shipping in Firefox and stabilizing."*

This has two direct implications for the implementation plan written above:

**Implication 1: wgpu-hal vs wgpu at the WebRender Device boundary.**
Mozilla explicitly rules out `wgpu-core` in WebRender. The reason is resource ownership:
`wgpu-hal` exposes raw platform handles (`vk::Image`, `ID3D12Resource`, `MTLTexture`)
that can be shared across subsystem boundaries without wgpu-core's reference-counted
ownership model creating friction. At the compositor output boundary — where WebRender
must hand a texture to the embedder — `wgpu-hal` textures can be consumed by wgpu-core
(Graphshell's side) via the unsafe `hal` API (`device.as_hal`, `texture.as_hal`). This
is the intended architecture for zero-copy handoff.

The implementation plan (§3.3, P5) currently targets `wgpu` for `WgpuDevice`. This
should be reconsidered. Two valid positions:

| Position | wgpu-hal in WebRender | wgpu in Graphshell compositor | Trade-off |
| --- | --- | --- | --- |
| **A (match Mozilla)** | `wgpu-hal` directly for Device/Renderer | `wgpu` (`device.as_hal` for texture import) | Closer to upstream; unsafe boundary at handoff; no `wgpu-core` in WebRender |
| **B (pragmatic)** | `wgpu` for Device/Renderer (uses `wgpu-core`) | `wgpu` (shared device, zero-copy native) | Simpler; may diverge from upstream; `wgpu-core` in WebRender |

For Graphshell's initial spike (P8), Position B is acceptable — it is faster to prove
and produces measurable evidence. Position A is the correct long-term architecture if
Graphshell's changes are intended to upstream. The plan should state this explicitly.

**Implication 2: GLSL→SPIRV, not GLSL→WGSL.**
Mozilla's position is that GLSL stays as the shader source of truth. The accepted
build-time path is GLSL → SPIR-V (via `glslang` or `shaderc`), not GLSL → WGSL.

The implementation plan (§3.4, P4) currently recommends naga for GLSL→WGSL translation.
@wusyong's June 2025 experiment validated that all optimized Servo WebRender shaders
translate cleanly to SPIR-V via `glslang-validator` (only `gpu_cache_update`, an
unoptimized shader, required manual fixup). This is faster than the naga WGSL path and
aligns with upstream.

**Updated shader translation recommendation**: use `glslang` / `shaderc` for GLSL→SPIR-V
at build time. Consume SPIR-V through `wgpu::ShaderSource::SpirV` (for Position B) or
directly through `wgpu-hal` pipeline creation (for Position A). naga remains useful for
validation (naga can parse SPIR-V), but is no longer the recommended translation target.

### 8.3 Community Experiments

| Contributor | Experiment | Date | Status |
| --- | --- | --- | --- |
| @wusyong | GLSL→SPIR-V translation of all Servo WR shaders via `glslang-validator` | 2025-06 | Successful; not followed up |
| @jdm | Compositor-level `Renderer` trait seam (byo-renderer branch) | 2025-06 | Prototype; not PRed |
| @sagudev | Vello canvas 2D backend for `<canvas>` element | 2025-07 | **Merged** to Servo main |
| Mozilla | wgpu-hal WebRender backend | Planned | Blocked on WebGPU-in-Firefox stabilization; no timeline |

**Vello note**: The merged Vello canvas backend is behind `servo/vello` feature and
`dom_canvas_vello_enabled` pref. It uses `wgpu` for `<canvas>` 2D rendering. It does
not touch the WebRender compositor path. Graphshell's `vello = ["servo/vello"]` feature
already exposes this and can be activated independently of the WebRender wgpu work.
Vello's subcrates (`vello_encoding`, `vello_shaders`) are not usable as a WebRender
Device/Renderer substrate — their rendering model (compute-shader vector rasterization)
is fundamentally incompatible with WebRender's CSS primitive batch rendering model.

### 8.4 Updated Open Questions

The following questions from §9 (below) are resolved or updated by this reconnaissance:

| Q# | Resolution |
| --- | --- |
| Q2 | Upstream active state: byo-renderer prototype exists but is stalled. Mozilla has a plan (wgpu-hal) but no active resources or timeline. No competing implementation to coordinate with at this time. |
| Shader Q | GLSL→SPIR-V preferred over GLSL→WGSL per upstream direction. @wusyong's experiment validates feasibility. |
| API level Q | wgpu-hal preferred over wgpu for WebRender Device internals per Mozilla. Graphshell spike can start with wgpu (Position B) and migrate to wgpu-hal if upstreaming proceeds. |

### 8.5 Revised Decision Log Entries

The following supersede or refine the implementation plan's §11 decisions:

| Decision | Prior (2026-03-01) | Updated |
| --- | --- | --- |
| Shader translation | naga GLSL→WGSL | GLSL→SPIR-V via `shaderc`/`glslang` (matches upstream; validated by @wusyong) |
| GPU API level in WebRender Device | `wgpu` | `wgpu-hal` for upstream alignment; `wgpu` acceptable for initial spike (Position B → Position A migration path) |
| Compositor seam | Implement from scratch | Cherry-pick jdm's byo-renderer commits onto current Servo main (saves ~1 day of work; known-good design) |

---

## 9. Open Questions

These are research questions that remain unresolved and must be answered before a full
implementation plan is written. They are the inputs to the spike work in §7.2.

| # | Question | Blocking for |
|---|---------|-------------|
| Q1 | What wgpu version does the current Servo main branch use? Does it match the egui_wgpu version Graphshell will target? | G1, dependency spike |
| Q2 | ~~Active upstream Servo or WebRender wgpu renderer work?~~ **Resolved**: byo-renderer stalled; Mozilla wgpu-hal plan exists but ungated. | — |
| Q3 | Does wgpu on Windows DX12 support external image import (shared DXGI handle)? What is the minimum wgpu version? | Platform feasibility, Windows spike |
| Q4 | Can `egui_wgpu::Renderer` accept a Graphshell-owned `wgpu::Device`, or does it always create its own? | Device ownership model |
| Q5 | How many WebRender shaders are there? What GLSL features require manual fixup for SPIR-V translation? (`gpu_cache_update` known; others TBD) | Shader translation risk |
| Q6 | Is the Servo `OffscreenRenderingContext` API extensible to return a `wgpu::Texture` instead of a GL texture ID, without breaking the GL path? | Embedding API design |
| Q7 | For Position A (wgpu-hal in WebRender): which `wgpu-hal` backends are stable enough on Windows DX12, macOS Metal, and Linux Vulkan for Graphshell's milestone platform matrix? | Platform confidence for upstream-aligned path |

---

## 9. Summary

| Dimension | Finding |
|-----------|---------|
| **Recommended approach** | Parallel wgpu `Device` + `Renderer` inside WebRender, with GL path retained behind flag |
| **Key benefit for Graphshell** | Closes `#180` (runtime-viewer GL→wgpu bridge), enabling the `egui_glow` → `egui_wgpu` migration tracked in `#183` |
| **Compositor improvement** | Replaces GL state save/restore (chaos mode) with structurally isolated wgpu command encoder scoping |
| **Feasibility** | Medium: bounded scope in WebRender; medium risk in shader translation and shared device ownership |
| **QA definition** | Pixel parity with GL baseline; frame budget ≤4 ms/tile; fallback path valid; diagnostics channels emitting |
| **Upstreaming model** | Spike locally first; upstream WebRender changes to Servo; keep Graphshell bridge layer local |
| **Current gate** | `#180` spike authorized; full implementation blocked by G1–G5 readiness gates |

---

## 10. Current Branch Reality Update (2026-04-02)

The research above remains directionally useful, but the local `wgpu-backend-0.68-minimal`
branch has changed the practical starting point.

The March framing assumed that the next meaningful move would be a relatively clean parallel
`Device` + `Renderer` backend split. The branch now proves something narrower and more concrete:

- WebRender can construct a `RendererBackend::Wgpu` path and execute real wgpu rendering work.
- The current implementation is a **hybrid proof path**, not yet the long-term dual-backend
  architecture described in §3.
- GL is still the correctness oracle, compatibility backend, and fallback path for both
  WebRender and Graphshell.

In code, the most important current-state facts are:

- `Renderer` still has GL-shaped ownership, now with `device: Option<Device>` plus
  `wgpu_device: Option<WgpuDevice>` in
  `webrender/webrender/src/renderer/mod.rs`.
- The wgpu path is routed through `render_wgpu()` rather than through a settled shared
  executor seam.
- Several renderer subsystems have `Wgpu(...)` enum variants, but those variants are still
  placeholders or no-op carriers rather than true backend-neutral abstractions.
- Shader translation succeeds today, but it still depends on build-time compatibility passes
  and string-keyed metadata rather than typed shader or pipeline descriptions.

### 10.1 Proposal Consequence

This changes the implementation advice.

The right next step is **not** an immediate renderer-wide trait or generic rewrite.
The right next step is a staged convergence plan:

1. keep GL as the parity oracle and production-safe fallback,
2. improve the wgpu backend subsystem-by-subsystem,
3. replace stringly backend metadata with typed metadata,
4. move toward backend-specific executors at explicit seams,
5. retain Graphshell's bridge/fallback contract while parity evidence is gathered.

### 10.2 Practical Reading Rule

Read §§3-7 in this document as the architectural direction and problem framing.
Do **not** read them as a claim that the current branch already has that architecture.

For active implementation planning, the branch reality is:

- current WebRender work is still GL-shaped internally,
- current wgpu work is already useful and worth improving,
- GL must remain available for parity testing, fallback behavior, and downstream safety.

---

## 11. Architectural Overview: Current WebRender GL Backend

This section is an orientation map for the current GL backend as it exists in the tree now.
It is intentionally code-grounded, because the GL backend is still the reference path used
to judge wgpu parity.

### 11.1 High-Level Shape

The GL backend is concentrated in two major runtime owners:

- `Device` in `webrender/webrender/src/device/gl.rs`
- `Renderer` in `webrender/webrender/src/renderer/mod.rs`

Everything above those two layers is closer to backend-neutral scene construction:

- scene building
- display list processing
- render task graph construction
- batching policy
- primitive and clip data generation

The GL backend is therefore best understood as the **execution layer** for a largely shared
render policy stack.

### 11.2 `Device`: GL Resource and State Owner

`Device` in `webrender/webrender/src/device/gl.rs` owns the concrete OpenGL execution model.
Its responsibilities include:

- bound GL state tracking: textures, programs, VAOs, read/draw FBOs,
- capability detection and policy decisions,
- texture creation and upload behavior,
- depth target sharing,
- shader/program lifetime,
- draw target binding,
- frame begin/end lifecycle.

Representative state carried directly on `Device` includes:

- bound texture slots,
- currently bound program,
- currently bound VAO,
- read/draw framebuffer bindings,
- upload method and batching policy,
- hardware/API capability cache,
- shared depth target pool,
- program cache and frame ID.

This is a classic stateful GL executor. It is efficient for the existing backend, but it also
shows why a second backend cannot simply "slot in" without either mirroring these concerns or
moving the execution seam upward.

### 11.3 `Renderer`: Frame Orchestrator and Backend Aggregator

`Renderer` in `webrender/webrender/src/renderer/mod.rs` owns per-frame orchestration. In the
GL path it coordinates:

- result-message processing from backend threads,
- texture cache updates,
- GPU cache uploads,
- vertex-data texture binding,
- shader selection,
- render pass execution,
- compositing,
- profiler/debug state,
- screenshots/capture integration.

Important GL-owned or GL-shaped renderer members include:

- `device: Option<Device>`
- `shaders`
- `vaos`
- `gpu_cache_texture`
- `vertex_data_textures`
- `texture_resolver`
- `upload_state`
- `aux_textures`
- `async_frame_recorder` and `async_screenshots`

The current branch adds wgpu state next to those fields rather than replacing the ownership
shape, which is why the current backend story is still hybrid.

### 11.4 Shader Management

The GL backend uses a substantial runtime shader catalog in
`webrender/webrender/src/renderer/shade.rs`.

That layer is responsible for:

- lazily compiling many specialized shader programs,
- selecting shader feature variants,
- keeping the GL shader catalog coherent with batch kinds and pass needs,
- surfacing compile and link errors,
- supporting shader caching and precache flows.

The important architectural fact is that shader identity today is strongly tied to backend
execution details. This is one reason the current wgpu path still relies on string-based
pipeline naming and compatibility metadata: the GL backend's shader organization is a mature,
runtime-oriented program catalog, not yet a shared typed pipeline description system.

### 11.5 GPU Cache and Data Upload Model

The GL backend uses several data movement strategies that are deeply GL-shaped:

- GPU cache textures in `webrender/webrender/src/renderer/gpu_cache.rs`
- texture upload staging and PBO pools in `webrender/webrender/src/renderer/upload.rs`
- vertex-data textures in `webrender/webrender/src/renderer/vertex.rs`

These layers collectively handle:

- persistent GPU cache storage,
- row- or scatter-style cache updates,
- staging and batched texture upload,
- per-frame primitive header, transform, and render-task data upload,
- VAO-oriented instance submission.

This matters for wgpu planning because the current wgpu backend is still largely emulating the
same data model, rather than replacing it with a more native buffer-first model.

### 11.6 Render Pass Execution

At frame time, `Renderer` executes the render-task graph by drawing into:

- texture cache targets,
- alpha/color targets,
- picture cache targets,
- final composite outputs.

The GL backend owns the concrete mechanics for:

- draw target binding,
- clears,
- scissor and depth state,
- shader binding,
- texture binding,
- instanced draw submission,
- final composite presentation.

This is the key execution surface any backend proposal must reckon with. The scene-building
side may be largely backend-neutral, but draw submission is still organized around explicit
GL execution primitives.

### 11.7 Compositing

The GL backend also still owns the mature compositor integration paths:

- draw compositing,
- layer compositing,
- native compositor support.

In Graphshell terms, this is especially important because the current product contract still
depends on GL compositor behavior at the Servo boundary, and the Graphshell bridge policy keeps
`GlowCallback` as the current production path.

### 11.8 Profiler, Queries, and Capture

`webrender/webrender/src/device/query_gl.rs` and `webrender/webrender/src/screen_capture.rs`
show two further reasons GL remains first-class:

- profiler/query plumbing is GL-backed,
- asynchronous screen capture and related tooling are GL-backed,
- debug marker behavior is GL-backed.

The wgpu path currently uses no-op or parallel logic for some of this, but not full feature
parity. That makes GL valuable not just as a fallback renderer, but as the richer diagnostic
and observability backend during transition.

### 11.9 Pressure Points for a Dual-Backend Future

The current GL architecture suggests five practical pressure points for future dual-backend work:

1. execution metadata is still too stringly,
2. renderer ownership is still too GL-shaped,
3. upload/cache/data-texture flows are still compatibility-driven,
4. compositor/profiler/capture facilities remain uneven across backends,
5. parity testing still depends on GL being available and trustworthy.

These are not reasons to avoid the wgpu backend. They are reasons to preserve GL while the
wgpu backend is improved.
