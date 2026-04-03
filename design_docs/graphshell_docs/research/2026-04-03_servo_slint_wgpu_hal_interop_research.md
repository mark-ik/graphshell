# Servo / Slint WGPU-HAL Interop Research

**Date**: 2026-04-03
**Status**: Research
**Author**: Codex
**Feeds Into**:
- `research/2026-03-01_webrender_wgpu_renderer_research.md`
- `implementation_strategy/2026-03-01_webrender_wgpu_renderer_implementation_plan.md`

---

## 1. Research Question

The Slint `examples/servo` sample demonstrates a path for embedding Servo inside another Rust GUI
stack while the host UI owns a `wgpu` device and Servo still renders through its own OpenGL-based
rendering context. This note answers four questions:

1. How does the example get OpenGL to cooperate with a host `wgpu` compositor?
2. How does it work even though Servo `v0.0.5` is on the `wgpu 26` line while the Slint example
   uses `wgpu 28`?
3. Which parts of the `wgpu-hal` / native-handle side look abstractable?
4. What utility-crate shape would preserve the reusable value without baking in Slint- or
   Servo-specific assumptions?

---

## 2. Executive Summary

The main finding is that the Slint example does **not** directly interoperate between two Rust
`wgpu` versions. Instead, it avoids version coupling by meeting at the **native GPU resource**
boundary:

- On Linux/Android, it allocates a new Vulkan image using the **host application's** `wgpu 28`
  HAL device, exports its memory as an FD, imports that memory into OpenGL with
  `GL_EXT_memory_object_fd`, blits Servo's GL framebuffer into that texture, then wraps the same
  Vulkan image back into a host `wgpu::Texture`.
- On Apple platforms, it extracts an `IOSurface`-backed native surface from Surfman, creates a
  Metal texture from it, converts that raw Metal texture into a `wgpu-hal` Metal texture, then
  wraps that in the host `wgpu::Texture`.

So the reusable boundary is not "Servo-to-wgpu" in the abstract. It is more specifically:

**native surface / native texture interchange between an offscreen GL producer and a host-owned
`wgpu` device**

This is promising for WebRender research because it aligns with the long-term upstream direction
that prefers `wgpu-hal` at the renderer boundary rather than `wgpu` or `wgpu-core`.

---

## 3. How OpenGL Cooperates

### 3.1 Servo still renders into a Surfman-managed OpenGL context

The Slint example creates a Surfman rendering context that owns:

- a Surfman device,
- a Surfman GL context,
- a Surfman surface / swap-chain,
- loaded GL entry points through both `gleam` and `glow`.

This is a normal Servo-compatible rendering context, not a `wgpu` renderer. Servo paints into the
Surfman-attached OpenGL framebuffer through the `RenderingContext` trait.

### 3.2 The host UI never tries to share GL state directly

The example avoids "single-context chaos mode" by not asking Slint's renderer to render into the
same GL context Servo uses. Instead:

- Servo renders offscreen into its Surfman/OpenGL surface.
- On each new frame, the example converts the rendered result into an image the host UI can sample.
- Slint then redraws using its own renderer path.

This is the crucial architectural move. The cooperation is **resource handoff**, not **shared
render-state coexistence**.

### 3.3 Linux/Android: GL cooperates by blitting into externally backed memory

On Linux/Android, the sample:

1. unbinds the Surfman surface from the current GL context,
2. obtains the host `wgpu` device's Vulkan HAL handle with `device.as_hal::<Vulkan>()`,
3. creates a new Vulkan image with external-memory support,
4. exports that image's memory as an opaque FD,
5. imports the FD into OpenGL with `GL_EXT_memory_object_fd`,
6. binds the imported memory to a GL texture,
7. blits Servo's GL framebuffer into that texture,
8. flushes GL,
9. wraps the Vulkan image into the host `wgpu::Texture` with
   `create_texture_from_hal::<Vulkan>()`.

So OpenGL "cooperates" because it is writing into memory that was allocated to be visible to the
host Vulkan / `wgpu-hal` side.

### 3.4 Apple: GL cooperates by exposing an IOSurface-backed native surface

On Apple platforms, Surfman exposes a native surface carrying an `IOSurface`. The sample:

1. unbinds the Surfman surface,
2. extracts the `IOSurface`,
3. creates a Metal texture backed by that `IOSurface`,
4. converts that raw Metal texture into a `wgpu-hal` Metal texture,
5. wraps it into the host `wgpu::Texture`,
6. runs a small flip pass because of coordinate / format differences.

Again, the cooperation is via native objects, not shared Rust `wgpu` types.

### 3.5 Important limitation: synchronization is minimal

The Linux path includes GL extension generation for semaphores in `build.rs`, but the current code
path does not actually use cross-API semaphore signaling. The visible synchronization is basically
`gl.flush()` plus ownership sequencing and surface bind/unbind discipline.

This makes the example valuable, but still somewhat prototype-ish. A production-grade extraction
should treat synchronization as a first-class API concern.

---

## 4. Why Different WGPU Versions Can Still Work

### 4.1 The versions do not meet at the Rust type level

Servo `v0.0.5` uses the `wgpu 26` line in its workspace dependency graph. The Slint example uses
host-side `wgpu 28`.

That would be a serious problem if the design attempted to pass:

- `wgpu::Texture`,
- `wgpu::Device`,
- `wgpu-hal::Texture`,
- or any other Rust-side backend type

from one version line directly into the other.

It does not.

### 4.2 The real compatibility boundary is native API ABI / object identity

The bridge works because the handoff object is one of:

- a Vulkan image + exported memory FD,
- a Metal texture / `IOSurface`,
- or, more generally, a native backend-owned resource plus enough metadata to describe it.

That boundary is not "versionless" in the absolute sense, but it is far more stable than attempting
to share Rust crate-level `wgpu` internals across versions.

### 4.3 Practical consequence

The example should be understood as:

`OpenGL producer -> native GPU resource -> host wgpu-hal import -> host wgpu::Texture`

not as:

`Servo wgpu 26 -> Slint wgpu 28`

This is good news for extraction: a crate can be designed around **native resource interchange**
without encoding hard assumptions about one specific `wgpu` version pairing.

---

## 5. HAL-Side Analysis

### 5.1 The sample uses exactly the `wgpu` seams Graphshell cares about

The important APIs used in the example are:

- `wgpu::Device::as_hal::<A>()`
- `wgpu_hal::<backend>::Device::texture_from_raw(...)`
- `wgpu::Device::create_texture_from_hal::<A>(...)`

That is nearly the canonical "unsafe handoff" seam between a native backend resource and a host
`wgpu::Texture`.

### 5.2 There are two abstractable strategies, not one

The sample actually demonstrates two different import strategies:

#### Strategy A: Direct native texture import

Used on Apple:

- start from a native shareable surface,
- derive a backend-native texture,
- wrap it directly through `wgpu-hal`,
- optionally run a post-process normalization pass (flip / format fixup).

This is the cleaner pattern.

#### Strategy B: Allocate host-owned external texture, then blit producer output into it

Used on Linux/Android:

- allocate host-owned backend-native image,
- expose it to the producer-side API,
- copy / blit producer output into it,
- wrap the same image back into host `wgpu`.

This is more general and likely more relevant to WebRender if the renderer cannot directly emit a
host-consumable native texture at first.

### 5.3 What is abstractable

These pieces look meaningfully reusable:

- backend detection and `as_hal::<...>()` acquisition,
- raw texture wrapping (`texture_from_raw` + `create_texture_from_hal`),
- native-handle metadata descriptors,
- explicit lifetime / drop-callback management,
- format normalization and Y-flip post-processing,
- a producer/consumer abstraction for "rendered frame to imported texture."

### 5.4 What is not yet abstracted cleanly

These pieces are still too backend- and producer-specific:

- the exact Vulkan memory export/import sequence,
- the GL extension calls for external-memory textures,
- Surfman-specific surface extraction,
- Servo-specific frame timing and callback flow,
- synchronization policy.

The value is real, but the correct reusable unit is narrower than "generic Servo webview crate."

---

## 6. Value for WebRender Research

### 6.1 Immediate value for the plain `wgpu` spike

For a Graphshell-first proof using plain `wgpu`, the Slint example is useful because it shows a
pragmatic bridge shape:

- let the renderer keep its current producer API,
- convert its output into a host-visible texture through native handles,
- consume the result as a host `wgpu::Texture`.

This is especially relevant if the first milestone is "get a `wgpu::Texture` out" before the
renderer internals are fully refactored.

### 6.2 Longer-term value for the `wgpu-hal` path

For the upstream-aligned WebRender direction, the strongest lesson is not the Slint-specific UI
integration. It is that the final contract should be formulated in terms of:

- native texture ownership,
- explicit backend type,
- synchronization,
- import / export descriptors,
- and lifetime callbacks.

That is exactly the kind of boundary a `wgpu-hal`-based WebRender backend would want anyway.

### 6.3 Important caution

The Slint example is still a consumer-side bridge around an OpenGL producer. It should not be read
as proof that WebRender itself should keep an OpenGL renderer forever. Its value is mainly:

- proving texture handoff patterns,
- reducing risk for an intermediate migration phase,
- and clarifying what the eventual `wgpu-hal` seam should look like.

---

## 7. Utility-Crate Opportunity

### 7.1 What not to package first

A first crate should **not** try to be:

- a full Servo embedder framework,
- a generic Rust-GUI webview crate,
- or a one-size-fits-all "cross-version wgpu interop" crate.

Those scopes are too broad and hide the actual reusable primitive.

### 7.2 Recommended first crate

The best first utility crate is a low-level interop crate focused on:

**offscreen producer output -> host-owned `wgpu` texture import**

Working name candidates:

- `wgpu-native-texture-interop`
- `wgpu-hal-texture-interop`
- `surfman-wgpu-interop` if the initial scope stays Surfman-centric

### 7.3 Proposed crate responsibilities

The crate should own:

- native texture descriptor types,
- backend-specific unsafe import helpers,
- explicit lifetime / drop-callback handling,
- optional normalization passes (flip / format convert),
- synchronization strategy traits,
- platform feature gating.

It should not own:

- Servo lifecycle,
- GUI event translation,
- app framework widgets,
- URL/navigation/browser state.

### 7.4 Suggested module boundaries

```text
interop-core/
  backend.rs
  error.rs
  frame_desc.rs
  sync.rs

interop-vulkan-gl/
  external_image.rs
  gl_ext.rs
  blit.rs
  import.rs

interop-metal/
  iosurface.rs
  import.rs
  normalize.rs

interop-surfman/
  surface_extract.rs
  surfman_bridge.rs
```

### 7.5 Suggested public traits

```rust
trait HostTextureImporter {
    type ImportedTexture;
    type Error;

    fn import_native_frame(&self, frame: NativeFrame) -> Result<Self::ImportedTexture, Self::Error>;
}

trait ProducerFrameSource {
    type Error;

    fn acquire_native_frame(&self) -> Result<NativeFrame, Self::Error>;
}

trait InteropSynchronizer {
    fn before_import(&self) -> Result<(), InteropError>;
    fn after_blit(&self) -> Result<(), InteropError>;
}
```

`NativeFrame` would be an enum carrying backend-specific variants such as:

- `VulkanImageFd { ... }`
- `MetalIosurface { ... }`
- `GlFramebufferBlitSource { ... }`

This keeps the unsafe backend work explicit and keeps higher-level embedders out of the import
details.

---

## 8. Suggested Extraction Path

### Phase 1: Producer-agnostic host import crate

Extract only:

- Metal raw texture import,
- Vulkan raw image wrap,
- `create_texture_from_hal` glue,
- flip / format normalization helpers.

Goal: prove the host-side import seam independent of Servo.

### Phase 2: Surfman bridge crate

Add:

- Surfman surface extraction,
- GL framebuffer / texture blit helpers,
- extension loading and capability checks.

Goal: support any Surfman/OpenGL producer, not just Servo.

### Phase 3: Servo adapter crate

Add:

- `RenderingContext` / frame callback integration,
- event-loop wake integration,
- optional helper for producing imported host textures per frame.

Goal: keep Servo-specific lifecycle and embedder glue out of the lower-level interop crates.

This sequencing keeps the reusable value visible and prevents the first crate from turning into a
grab-bag of embedder logic.

---

## 9. Recommendation for Graphshell

For Graphshell specifically:

1. Treat the Slint example as evidence for a **native-handle handoff strategy**, not as a
   drop-in architectural template.
2. For the first plain-`wgpu` milestone, use the example to shape a temporary bridge contract
   that yields a host `wgpu::Texture`.
3. Keep the long-term WebRender design aligned with `wgpu-hal` and native-texture ownership.
4. If packaging work begins, start with the low-level interop layer, not a Servo widget crate.

The biggest productizable value here is a crate that turns "unsafe native texture import and
normalization" into a small, audited boundary that higher-level renderers and embedders can reuse.

---

## 10. Sources

- Slint Servo example README:
  https://github.com/slint-ui/slint/tree/master/examples/servo
- Slint Servo example manifest:
  https://raw.githubusercontent.com/slint-ui/slint/master/examples/servo/Cargo.toml
- Slint Surfman rendering context:
  https://raw.githubusercontent.com/slint-ui/slint/master/examples/servo/src/webview/rendering_context/surfman_context.rs
- Slint GPU rendering bridge:
  https://raw.githubusercontent.com/slint-ui/slint/master/examples/servo/src/webview/rendering_context/gpu_rendering_context.rs
- Slint Metal import path:
  https://raw.githubusercontent.com/slint-ui/slint/master/examples/servo/src/webview/rendering_context/metal/metal.rs
- Slint build-time GL extension generation:
  https://raw.githubusercontent.com/slint-ui/slint/master/examples/servo/build.rs
- Servo crate docs:
  https://doc.servo.org/servo/index.html
- Servo `v0.0.5` release:
  https://github.com/servo/servo/releases/tag/v0.0.5
- Servo `v0.0.5` workspace dependency graph:
  https://raw.githubusercontent.com/servo/servo/v0.0.5/Cargo.toml
- Graphshell WebRender research note:
  [2026-03-01_webrender_wgpu_renderer_research.md](/c:/Users/mark_/Code/source/repos/graphshell/design_docs/graphshell_docs/research/2026-03-01_webrender_wgpu_renderer_research.md)
