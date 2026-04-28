# GPUI Host Integration Plan

Supersedes the host-framework target in
[`2026-04-14_iced_host_migration_execution_plan.md`](2026-04-14_iced_host_migration_execution_plan.md)
for the long-run host choice. iced remains the active implementation target
until the milestone gate below is met.

Related research:
[`../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`](../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md),
[`../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md`](../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md),
[`../../technical_architecture/2026-04-20_graphshell_gpu_spec.md`](../../technical_architecture/2026-04-20_graphshell_gpu_spec.md).

---

## Naming context

- **Servo** is the upstream browser engine and embedding API lineage.
- **Serval** is Graphshell's proposed minimal Servo mirror/fork: Servo-shaped
  at the embedder boundary, but moving rendering to wgpu and deleting GL-era
  assumptions and constraints as they stop being useful.
- **NetRender** is the proposed name for `webrender-wgpu`: the
  Graphshell/Serval-maintained, wgpu-native continuation of WebRender focused
  on page and document rendering without GL-era backend assumptions. Until the
  rename is executed, use "NetRender (proposed name for `webrender-wgpu`)" in
  docs that need both clarity and path accuracy.

Renames are terminology only for this plan. Do not churn crate paths,
repository names, or Cargo references until the technical migration gates below
are proven.

## Phase 0 — Local external-texture proof

Before outreach, prove the API shape locally against the Glass-HQ fork. The
first proof should be a static generated wgpu texture owned by the GPUI device,
then a Serval/NetRender page texture. Outreach is stronger with code, a narrow
diff, and a screenshot than with an imagined API.

Acceptance:

1. GPUI window composites a generated checkerboard `wgpu::TextureView` through
   the proposed external texture surface API.
2. The texture is produced from the same `wgpu::Device`/`Queue` GPUI uses.
3. Resize and repaint do not leak resources or require cross-device copies.

## Phase 1 — Glass-HQ outreach (after Phase 0)

The Glass-HQ GPUI fork is the wgpu-aligned candidate, but keep the Zed GPUI
lineage and the Glass-HQ renderer architecture distinct in the docs. If the
March 2026 Blade-to-wgpu migration claim remains here, pin it to a source or
commit before treating it as a durable fact.

- Open a discussion in `Glass-HQ/gpui` describing the use case and the patch
  shape (see *Findings* below), backed by the Phase 0 proof.
- If positive: submit the PR against Glass-HQ and depend on Glass-HQ as a git
  dep.
- If no response within 30 days: maintain the patch as a local diff against
  Glass-HQ main. Keep the diff narrow, but do not assume it is permanently
  small: renderer resource recovery and device-loss paths are part of the
  surface contract.
- Separately, post a pointer issue in `zed-industries/zed` referencing the
  Glass-HQ implementation. No expectation of acceptance; low-cost signal.

Acceptance: Glass-HQ outreach sent, response recorded here.

## Phase 2 — Patch implementation

Implement the three-part patch against Glass-HQ (detail in *Findings*).
Validate the GPUI host build with `cargo tree -d`: single wgpu 29 in that
feature/profile's dep graph.

Acceptance: `cargo check -p graphshell` clean for the GPUI host feature/profile,
no duplicate wgpu in that graph.

## Phase 3 — Migration gate

Do not begin the iced→GPUI migration in Graphshell until all gates are green:

1. Glass-HQ patch merges (or proves clean across two upstream pulls solo).
2. Single GPUI window hosts one Serval/NetRender Navigator surface via the
   external wgpu surface API, rendering a real page.
3. Graph canvas renders bezier edges on the same shared device, either through
   a proven Vello integration or through GPUI-native path primitives if those
   are sufficient.
4. Pointer, keyboard focus, resize, clipping/z-order, and repaint routing work
   through Graphshell's existing host/runtime seams.
5. Device loss/recreate has an explicit recovery path for GPUI and
   Serval/NetRender surfaces.
6. `cargo tree -d` shows no duplicate wgpu for the GPUI host feature/profile.

## Monitor conditions

- **Xilem/Masonry**: two concrete production blockers remain as of 2026-04-27
  — HiDPI scale factor pervasively hacked across all widget measure passes
  (`// TODO: Remove HACK: just pretend it's always 1.0`), and pointer capture
  not wired to the platform (`capture_pointer()` exists but `// TODO: plumb
  through to winit`). Drag-to-move-node breaks. If both close, revisit host
  framework choice — Xilem + Masonry is the architecturally cleanest answer
  because Vello integration is native rather than patched.
- **Glass-HQ dormancy**: if no activity for 60 days after outreach, evaluate
  patching directly against `zed-industries/zed` main instead.
- **GPUI render loop churn**: the patch touches `wgpu_renderer.rs`. If
  upstream restructures that file in a way that inflates the diff, reassess.

---

## Findings

### Framework survey (2026-04-27)

Survey researched: iced, Xilem/Masonry, GPUI, rust-gpu. Key version map:

| Component | wgpu |
|---|---|
| webrender-wgpu fork | 29 |
| Vello | 29 (bumped 2026-04-23) |
| iced dev (0.15-dev) | 28 |
| GPUI via Glass-HQ fork | 29 |

iced dev requires either a wgpu bump each Vello cycle or pixel-copy
compositing for the WebRender Navigator path. GPUI via Glass-HQ is on wgpu 29,
aligning all three rendering layers on one shared device.

**GPUI strengths:**
- gpui-component (longbridge): 60+ production widgets backed by a shipping
  fintech app (Longbridge Pro). Dock/panel layout, virtualized lists, menus,
  theming. 11k+ stars. Substantially richer than iced's current widget set for
  shell chrome.
- Proven complex-app architecture: Zed is more architecturally demanding than
  a typical widget showcase.
- Cross-platform: macOS, Linux (Wayland + X11, Vulkan), Windows (DX11 +
  DirectWrite) — all shipping in Zed.

**GPUI blocking gap:** No public API for custom GPU renderer embedding. No
community workaround exists as of 2026-04-27 (confirmed by exhaustive search:
zero repos, zero PRs, one unanswered discussion thread). The only surface
injection in all of GPUI is a macOS-only `CVPixelBuffer` path for camera
frames. See patch shape below.

**iced status:** Not frozen — master (0.15-dev) is on wgpu 28 after bumping
in January 2026 (~1 month lag from wgpu release). The published 0.14.0 release
is on wgpu 27. No wgpu 29 branch yet. Track record: ~1–2 month lag per wgpu
release. iced_blocks / iced_frame provide a working pixel-copy compositor
integration (same pattern as iced_servo). Viable path, but recurring version
management as Vello bumps.

**Xilem/Masonry:** wgpu 28 / Vello 0.8. Canvas widget hands you a Vello
`Scene` — right for graph canvas. Two production blockers:
- HiDPI scale factor: `// TODO: Remove HACK: just pretend it's always 1.0`
  pervasive across all widget measure passes.
- Pointer capture not wired to platform: drag-to-move-node breaks.
Self-declared alpha. Not ready for production shell in 2026.

**rust-gpu:** 0.10.0-alpha.1, requires pinned nightly. naga `spv-in`
compatibility explicitly broken and not CI-gated by the project. Not relevant
to the current webrender-wgpu SPIR-V pipeline.

### Target architecture

```
GPUI (Glass-HQ fork)
├── gpui-component — shell chrome
│   ├── Dock / panel layout
│   ├── Toolbar, menus, tab bar
│   └── General widgets
├── Vello Canvas element — graph canvas
│   ├── Shared wgpu 29 device from GPUI
│   ├── Bezier edges, node geometry, gradients, LOD levels
│   └── Pan/zoom via kurbo::Affine per frame
└── External wgpu PaintSurface — Navigator surfaces
    ├── Shared wgpu 29 device from GPUI
    ├── Serval/NetRender renders into wgpu::Texture per frame
    └── GPUI composites as textured quad at Navigator bounds
```

All three layers share one `Arc<wgpu::Device>`. No pixel copies for the graph
canvas. Serval/NetRender Navigator surfaces use the same device — no cross-device
transfers.

### Patch shape (three additive changes)

**1. Expose a wgpu context through the `gpui` public API**

Prefer a narrow capability over permanently exposing raw renderer internals,
but Graphshell must be able to construct Serval/NetRender on GPUI's device. A
minimal shape could be:

```rust
pub fn with_wgpu_context<R>(&self, f: impl FnOnce(&WgpuContext) -> R) -> R
```

or explicit accessors if upstream prefers:

```rust
pub fn gpu_device(&self) -> Arc<wgpu::Device>;
pub fn gpu_queue(&self) -> Arc<wgpu::Queue>;
```

Source: `WgpuContext.device` / `.queue` are already available within
`gpui_wgpu` in the Glass-HQ fork. This should be a capability re-export, not a
new ownership model.
Files: `crates/gpui/src/window.rs`, `crates/gpui/src/gpui.rs`.

**2. Cross-platform wgpu texture variant in `PaintSurface`**

```rust
pub struct ExternalWgpuSurface {
    pub view: Arc<wgpu::TextureView>,
    pub sampler: Option<Arc<wgpu::Sampler>>,
    pub bounds: Bounds<Pixels>,
    pub size: Size<Pixels>,
    pub format: wgpu::TextureFormat,
    pub alpha_mode: ExternalSurfaceAlphaMode,
    pub generation: u64,
}

pub enum PaintSurface {
    #[cfg(target_os = "macos")]
    CvPixelBuffer(CVPixelBuffer, Bounds<Pixels>),
    // new:
    WgpuTexture(ExternalWgpuSurface),
}
```
File: `crates/gpui/src/scene.rs`.

The `TextureView` alone is not enough API: the compositor needs size,
format/color assumptions, alpha semantics, same-device guarantees, and enough
lifecycle information to recover when GPUI recreates the device.

**3. Blit in the wgpu compositor**

Handle `WgpuTexture` in GPUI's wgpu render pass: sample the `TextureView` and
blit as a textured quad at the specified bounds while respecting masks,
z-order, alpha, and renderer resource recovery. Structurally similar to GPUI's
existing image rendering, but not just an image-path alias because the producer
is another renderer on the same device.
File: `crates/gpui_wgpu/src/wgpu_renderer.rs`.

Scope estimate after Phase 0. Treat `~300–600 lines across three files` as a
hypothesis, not a promise, until device-loss and resource-lifetime handling are
proven.

### Cargo dependency sketch

```toml
[dependencies]
gpui           = { git = "https://github.com/Glass-HQ/gpui" }
gpui_component = { git = "https://github.com/longbridge/gpui-component" }
vello          = { version = "0.8" }   # wgpu 29
# NetRender (proposed name for webrender-wgpu) via Serval dep chain — wgpu 29
```

Confirm with `cargo tree -d` after wiring up: no duplicate wgpu entries in the
GPUI host feature/profile.

---

## Progress

**2026-04-27** — Candidate promoted: GPUI via Glass-HQ is the primary long-run
host candidate pending the proof gates above. iced remains the active
implementation target until Phase 3 gate is met. Plan written; no code changed.

**2026-04-27 terminology update** — Serval named as the proposed minimal Servo
mirror/fork. NetRender recorded as the proposed name for `webrender-wgpu`, but
kept as terminology only until the renderer boundary is proven and a rename is
explicitly scheduled.

**2026-04-27** — iced bumped from 0.14 (wgpu 27) to 0.15.0-dev git master
(wgpu 28) on the `iced-wgpu-bump` branch. One code change required:
`Event::Clipboard(_) => None` added to `iced_events.rs` for the new
clipboard-read response variant. pdfium-render bumped 0.8→0.9 to resolve
a web-sys exact-version conflict (`iced_winit` pins `=0.3.85`; pdfium 0.8.37
was locked to 0.3.95 — pdfium 0.9.0 resolves to 0.3.85).

Dep graph before/after:

- Before: wgpu 27 (iced), 28 (vello), 29 (servo/egui/webrender) — 3 versions
- After:  wgpu 28 (iced+vello), 29 (servo/egui/webrender) — 2 versions

wgpu 27 is fully eliminated. iced_wgpu, cryoglyph, and vello now share wgpu 28.
Two winit versions (iced-rs fork 0.30.8 + crates.io 0.30.13) — acceptable.
The 28/29 gap remains; single-wgpu requires the GPUI path (Phase 3 gate).

**2026-04-28** — wgpu 29 parity reached via vendored iced. Iced (and one
supporting crate) vendored in-tree and bumped to wgpu 29; the change was
simple in practice. Dep graph: single wgpu 29 across iced, vello, servo,
webrender, and egui.

This eliminates the load-bearing motivation for prioritizing GPUI: the
patch shape proposed in §Findings was justified by *single-wgpu through a
shared device*, but iced now satisfies that on its own. The plan is not
withdrawn — gpui-component's widget richness, the Glass-HQ/Zed lineage,
and the architectural cleanness of native wgpu external-texture support
remain genuine advantages — but it moves from "long-run candidate" to
"branch experiment after iced stabilizes." Re-evaluate when the iced
chrome work has matured enough to define what a better host would
actually need to deliver.

Phase 0 (local external-texture proof) and Phase 1 (Glass-HQ outreach)
are not urgent. Phase 3 migration gate stays as written for whenever
the experiment is revisited.
