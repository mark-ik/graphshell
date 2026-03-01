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

## 8. Open Questions

These are research questions that remain unresolved and must be answered before a full
implementation plan is written. They are the inputs to the spike work in §7.2.

| # | Question | Blocking for |
|---|---------|-------------|
| Q1 | What wgpu version does the current Servo main branch use? Does it match the egui_wgpu version Graphshell will target? | G1, dependency spike |
| Q2 | Is there an active upstream Servo or WebRender tracking issue for a wgpu renderer? Who is the primary contributor? | Upstream coordination |
| Q3 | Does wgpu on Windows DX12 support external image import (shared DXGI handle)? What is the minimum wgpu version? | Platform feasibility, Windows spike |
| Q4 | Can `egui_wgpu::Renderer` accept a Graphshell-owned `wgpu::Device`, or does it always create its own? | Device ownership model |
| Q5 | How many WebRender shaders are there? What GLSL features do they use that naga may not translate correctly? | Shader translation risk |
| Q6 | Is the Servo `OffscreenRenderingContext` API extensible to return a `wgpu::Texture` instead of a GL texture ID, without breaking the GL path? | Embedding API design |

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
