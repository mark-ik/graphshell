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

## GPUI Host Integration Plan

### Phase 1 — Glass-HQ outreach (June 2026)

The Blade→wgpu migration in GPUI landed March 2026. Let the backend settle for
~2 months before patching the render loop.

- Open a discussion in `Glass-HQ/gpui` describing the use case and the patch
  shape (see *Findings* below).
- If positive: submit the PR against Glass-HQ and depend on Glass-HQ as a git
  dep.
- If no response within 30 days: maintain the patch as a local diff against
  Glass-HQ main. The diff is small; upstream pulls merge cleanly as long as
  `wgpu_renderer.rs` is not restructured.
- Separately, post a pointer issue in `zed-industries/zed` referencing the
  Glass-HQ implementation. No expectation of acceptance; low-cost signal.

Acceptance: Glass-HQ outreach sent, response recorded here.

### Phase 2 — Patch implementation

Implement the three-part patch against Glass-HQ (detail in *Findings*).
Validate with `cargo tree -d`: single wgpu 29 in the full dep graph.

Acceptance: `cargo check -p graphshell` clean, no duplicate wgpu.

### Phase 3 — Migration gate

Do not begin the iced→GPUI migration in Graphshell until all four are green:

1. Glass-HQ patch merges (or proves clean across two upstream pulls solo).
2. Single GPUI window hosts one Serval/WebRender Navigator surface via the
   `WgpuTexture PaintSurface` API, rendering a real page.
3. Vello graph canvas renders bezier edges on the same shared device.
4. `cargo tree -d` shows no duplicate wgpu.

### Monitor conditions

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
└── WgpuTexture PaintSurface — Navigator surfaces
    ├── Shared wgpu 29 device from GPUI
    ├── WebRender fork renders into wgpu::Texture per frame
    └── GPUI composites as textured quad at Navigator bounds
```

All three layers share one `Arc<wgpu::Device>`. No pixel copies for the graph
canvas. WebRender Navigator surfaces use the same device — no cross-device
transfers.

### Patch shape (three additive changes)

**1. Expose device and queue through the `gpui` public API**

Add to `WindowContext` (or `AppContext`):
```rust
pub fn gpu_device(&self) -> Arc<wgpu::Device>
pub fn gpu_queue(&self) -> Arc<wgpu::Queue>
```
Source: `WgpuContext.device` / `.queue` are already `pub` within `gpui_wgpu`
in the Glass-HQ fork. This is a re-export change, not new logic.
Files: `crates/gpui/src/window.rs`, `crates/gpui/src/gpui.rs`.

**2. Cross-platform wgpu texture variant in `PaintSurface`**

```rust
pub enum PaintSurface {
    #[cfg(target_os = "macos")]
    CvPixelBuffer(CVPixelBuffer, Bounds<Pixels>),
    // new:
    WgpuTexture(Arc<wgpu::TextureView>, Bounds<Pixels>),
}
```
File: `crates/gpui/src/scene.rs`.

**3. Blit in the wgpu compositor**

Handle `WgpuTexture` in GPUI's wgpu render pass: sample the `TextureView` and
blit as a textured quad at the specified bounds. Structurally equivalent to
GPUI's existing image rendering.
File: `crates/gpui_wgpu/src/wgpu_renderer.rs`.

Scope: ~300–600 lines across three files. Additive, no existing behaviour
changes. Merge conflict risk is low once the post-Blade migration settles.

### Cargo dependency sketch

```toml
[dependencies]
gpui           = { git = "https://github.com/Glass-HQ/gpui" }
gpui_component = { git = "https://github.com/longbridge/gpui-component" }
vello          = { version = "0.8" }   # wgpu 29
# webrender-wgpu via serval dep chain — wgpu 29
```

Confirm with `cargo tree -d` after wiring up: no duplicate wgpu entries.

---

## Progress

**2026-04-27** — Decision made: GPUI via Glass-HQ is the long-run host target.
iced remains the active implementation target until Phase 3 gate is met.
Glass-HQ outreach targeted for June 2026 (post-Blade wgpu backend settling
period). Plan written; no code changed.

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
