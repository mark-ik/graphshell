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
lineage and the Glass-HQ renderer architecture distinct in the docs.
**Correction (2026-04-29)**: the "March 2026 Blade-to-wgpu migration" claim
that previously appeared in this section was misattributed. The actual
migration is upstream Zed — [zed-industries/zed#46758](https://github.com/zed-industries/zed/pull/46758),
merged Feb 2026, removes Blade and reimplements the Linux renderer on wgpu.
That PR is in the upstream tree, not a Glass-HQ-specific change. Glass-HQ
remains the standalone-fork-of-gpui candidate; treat its rendering posture
as inheriting upstream's wgpu work, not as an independent migration.

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
  fintech app (Longbridge Pro). Dock/panel layout, virtualized Table/List,
  Tree, command palette, code editor with LSP/Tree-Sitter, menus, theming.
  11k+ stars.
- Proven complex-app architecture: Zed is more architecturally demanding than
  a typical widget showcase. FocusHandle / FocusableView is more explicit
  than iced's per-widget focus and is a closer architectural fit for the
  six-track focus model in §3.7.
- Cross-platform: macOS, Linux (Wayland + X11, Vulkan), Windows (DX11 +
  DirectWrite) — all shipping in Zed (Linux/Windows ports still stabilizing
  per upstream as of 2026-Q1).

**Calibration note (2026-04-29)**: an earlier framing of this section said
gpui-component is "substantially richer than iced's current widget set for
shell chrome." That comparison was against bare iced. The realistic iced
stack — iced core + `iced_aw` (Tabs, Menu, Context Menu, Sidebar) +
`libcosmic` (System76 COSMIC DE — list/grid views, drag-drop, theming, IME
work) + `iced_webview` (Servo / Blitz / litehtml / CEF behind feature flags)
— is **broader and more multi-source** than gpui-component, which is a
single-vendor catalogue. gpui-component is genuinely stronger on three
specific surfaces — command palette polish, virtualized Table/List
performance, and the built-in code editor — and gpui's focus model is
cleaner for multi-pane apps. iced's wins are: documented Servo embedding
(`iced_webview`), shipping multi-platform reach (libcosmic on
Win/macOS/Linux X11+Wayland), AccessKit integration in progress, and a
documented custom-wgpu pipeline path since [iced-rs/iced#183](https://github.com/iced-rs/iced/pull/183).
gpui's custom-renderer story is still upstream-gap-blocked
([zed-industries/zed#45996](https://github.com/zed-industries/zed/discussions/45996));
the patch shape below is exactly what would close it for Graphshell.

**Ecosystem update (2026-04-29): `gpui.rs` + `gpui-ce`.** The public
[`gpui.rs`](https://www.gpui.rs/) site is currently best read as upstream GPUI
onboarding: `Application::new().run`, `App::open_window`, `Entity<T>` views,
`Render`, `div()`/tailwind-style element builders, context documentation,
key dispatch, and examples for canvas, images, input, uniform lists, window
positioning, etc. It points learners back to Zed crates for larger-app
patterns.

[`gpui-ce/gpui-ce`](https://github.com/gpui-ce/gpui-ce) is a community edition
published as `gpui-ce` 0.3.x while preserving `use gpui::...` imports via
`package = "gpui-ce"`. It is useful as a distilled GPUI learning corpus: a
single primary crate, explicit `docs/contexts.md` and `docs/key_dispatch.md`,
`examples/learn/custom_drawing.rs` for `canvas` + `PathBuilder` +
`window.paint_*`, and `examples/bench/data_table.rs` for `uniform_list`,
virtualized row rendering, scroll handles, and custom scrollbar hit-testing.
Those examples strongly support a Graphshell structure based on small
`Entity`-owned models and custom elements/canvases at the graph and Navigator
boundaries.

But `gpui-ce` is not the best dependency candidate for Graphshell's GPU goal
as of this survey: its Linux path is on `wgpu = "24"`, macOS remains Metal,
Windows remains DirectX, and repository search found no `WgpuContext` /
`PaintSurface` / external-`wgpu::TextureView` hook. Treat it as a pattern and
API-learning source, not the Phase 0 patch base. Glass-HQ / upstream Zed remain
the relevant code lines for shared-device external texture research.

**Ecosystem update (2026-04-29): `awesome-gpui`.** The
[`zed-industries/awesome-gpui`](https://github.com/zed-industries/awesome-gpui)
index widens the research space beyond core GPUI/forks. The relevant options
for Graphshell split into five buckets:

- **Shell chrome dependency candidate:** `gpui-component` remains the strongest
  candidate for Dock/Tabs/panels, virtualized Table/List, Tree, command
  palette, notifications, themes, markdown/simple HTML, charts, and editor
  widgets. Use it behind Graphshell-owned shell abstractions; do not let it own
  runtime, focus, or content-surface authority.
- **Architecture bridge candidate:** `gpui-tea` is a serious option for an
  experimental GPUI host because it preserves TEA-style `init/update/view`,
  `Command`, `Subscription`, nested models, keyed async effects, cancellation,
  backpressure policies, and runtime telemetry while mounting as a GPUI
  `Entity`. It can bridge Graphshell's existing portable-contract / intent
  model to GPUI before a full observer-model rewrite.
- **Graph canvas candidates:** `gpui-flow` is the quickest React Flow-style
  prototype path: custom GPUI node renderers, Bezier/Straight/SmoothStep edges,
  handles, pan/zoom, selection, undo/redo, minimap, controls, and viewport
  culling. `ferrum-flow` is more useful as an architectural reference for
  plugin/command/model separation, collaboration, and extensibility, but looks
  too alpha to own Graphshell's canvas dependency graph today. In both cases,
  keep Graphshell's graph domain model owned by Graphshell; borrow interaction
  and rendering patterns, not authority.
- **Pattern-only app references:** Zed remains the production GPUI architecture
  reference. Arbor is useful for daemon/runtime split and long-running local
  workflows. Hunk appears close to the browser/offscreen-frame problem, but its
  GPL licensing means study patterns only unless licensing changes. DBFlux,
  Zedis, Fulgur, Okena/terminal apps, and similar developer tools are useful
  for keyboard-first workspace ergonomics, panes, virtualized data views,
  command palettes, and background task UX.
- **Defer/reject:** `gpui-nav` and `gpui-router` solve app screen routing, not
  Graphshell's browser/navigation/focus model. `gpui-hooks` may be useful for
  small component ergonomics, but Graphshell's runtime/focus/surface lifecycle
  should stay explicit. `gpui-form` and `gpui-storybook` are later settings and
  component-harness tools. Plotting libraries (`gpui-d3rs`, `gpui-px`,
  `plotters-gpui`) are later diagnostics/analytics candidates, not the main
  graph canvas. Simple demo apps are learning references only.

The ecosystem still does **not** contain a ready-made shared-`wgpu` /
external-texture solution for Navigator/Servo surfaces. The closest references
are GPUI/Zed renderer internals, `gpui-video-player`'s frame buffering and
`CVPixelBuffer`/sprite-atlas fallback, Hunk's browser/offscreen-frame patterns,
and React Native GPUI's foreign-runtime/view-tree/event boundary discipline.
External texture integration remains Graphshell-owned.

**GPUI blocking gap:** No public API for custom GPU renderer embedding. No
known community workaround exists as of 2026-04-29 after checking Glass-HQ,
upstream Zed, `gpui.rs`, and `gpui-ce`: the available public patterns are
GPUI-native vector/image/canvas painting, not cross-renderer texture
composition. The only surface injection in all of the Zed/Glass-HQ lineage is
a macOS-only `CVPixelBuffer` path for camera frames. See patch shape below.

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

## Idiomatic GPUI Adaptation

Phases 0–3 above are about renderer plumbing — getting Graphshell's three GPU
producers (chrome, graph canvas, content surfaces) onto a single shared
`wgpu::Device` through GPUI. They do not describe what Graphshell looks like
as a gpui *application*. This section does.

The renderer-integration work makes the host run. The adaptation work makes
the host idiomatic. Both can be partial; "idiomatic" is a slider, not a
binary. Stage A below is the minimum to ship; Stages B–F are layered
investments that each trade shim for idiom.

### Programming model translation

| Concept | iced (TEA pull) | gpui (reactive entity-view-model) |
|---|---|---|
| State authority | Application struct, mutated in `update` | `Model<T>`; mutations call `cx.notify()` |
| Re-render trigger | Every frame: `view(&self) -> Element` | Observation: dependents re-render only on `cx.notify()` |
| Long-lived view state | Application-held or widget-local | `View<T>` — persistent state per view instance |
| Action dispatch | `Message` enum + `update` | `cx.dispatch(action)` + `actions!` macro + key bindings |
| Focus | Per-widget; iced tracks one focused widget | `FocusHandle` per element; explicit register/observe; focus return on modal close is a first-class primitive |
| Async | `Subscription` + `Command` | `cx.spawn(async move { ... })` + `BackgroundExecutor` |
| Cross-element coordination | Through `Message` round-trips | Direct: views observe shared models; models notify |

The portable contract (`runtime.tick()`, `FrameViewModel`, `HostPorts`) was
shaped for the iced/egui pull loop. It still works on gpui — a root `View`
whose `render` calls `runtime.tick()` and feeds the resulting view-model to
children — but that shim leaves gpui's reactivity unused. Each subsequent
stage trades shim for idiom.

### Subsystem implementation — iced vs. gpui

Cross-referenced against the
[browser subsystem taxonomy](../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md).
"Edge" is the calibrated read after the 2026-04-29 ecosystem survey
(see [shell/2026-04-28_iced_jump_ship_plan.md](2026-04-28_iced_jump_ship_plan.md)
for the iced-side plan and the realistic iced stack: iced + `iced_aw` +
`libcosmic` + `iced_webview`).

| Taxonomy subsystem | iced approach | gpui approach | Edge |
|---|---|---|---|
| §3.6 FrameTree (Window → H/V splits → Panes) | `pane_grid::PaneGrid<Pane>` — resizable, adjustable, nestable splits as a native primitive | gpui-component `Dock` — splits + tabs + tiles built in, Zed-style drag-rearrange | gpui (polish), iced (semantic purity) |
| §3.6 Tab bar over active tiles | `iced_aw::Tabs` or libcosmic | gpui-component `Tabs` (production polish) | gpui |
| §3.6 Omnibar (Shell input + Navigator breadcrumb) | `text_input` + `row!` + custom result list; per-pane drafts via existing `OmnibarSearchSession` | gpui-component `Palette` is a near-direct fit; Zed's command palette is the reference impl | gpui |
| §3.6 Command palette | Custom; libcosmic launcher patterns adaptable | gpui-component `CommandPalette` | gpui (decisive) |
| §3.6 Context palette (right-click) | `iced_aw::ContextMenu` | gpui-component menus | tie |
| §3.6 Radial palette | Custom widget | Custom `Element` | tie |
| §3.6 Toasts | Custom or libcosmic patterns | gpui-component `Notification` | gpui (small) |
| §3.1 `WebViewSurface<NodeKey>` (Servo content) | `iced_webview` already has a Servo feature flag; or custom `shader` widget over Servo's wgpu external image API per [iced-rs/iced#183](https://github.com/iced-rs/iced/pull/183) | Glass-HQ's `PaintSurface::WgpuTexture` patch (still local; not upstream). Without it: no clean integration | **iced (decisive)** |
| §3.1 Wry viewer (non-web content) | `iced-wry-viewer` already exists in the Cargo tree | Build from scratch | iced |
| §3.1 Graph canvas (Vello) | `shader` widget submitting Vello scene's wgpu commands; canvas `Program` for hit-testing | Native Vello canvas `Element` via shared device (with Glass-HQ patch) | gpui (small) |
| §3.7 Six-track focus (G1 in iced plan) | iced's per-widget focus is shallow; SemanticRegion / PaneActivation / GraphView / EmbeddedContent / ReturnCapture all live in `graphshell-runtime`; iced widgets consult runtime state | gpui's `FocusHandle` is explicit and Zed-tested for focus-return, multi-pane, multi-cursor; GraphView and EmbeddedContent map to `FocusHandle`s directly | gpui (modest) |
| §3.7 IME (G8 in iced plan) | iced 0.14 IME + libcosmic Linux IME WIP; CJK testing uneven | Zed-tested macOS IME (solid); Linux Wayland fcitx/ibus behind | macOS → gpui; Linux → mixed |
| §3.7 AccessKit (G9 in iced plan) | `iced_accessibility` exists, integration in progress | No public AccessKit story | iced |
| §3.5 Navigator sidebar (graphlet members) | Custom tree view; libcosmic has list patterns | gpui-component `Tree` | gpui |
| §3.5 History view | Custom virtualized list; libcosmic | gpui-component virtualized `List` | gpui (perf) |
| §3.8 Diagnostics Inspector | Custom virtualization | gpui-component virtualized `Table` | gpui |
| §3.6 Settings panes (`verso://settings/<section>`) | Forms via core widgets | gpui-component forms | tie |
| §4.5 Canvas base layer (FrameTree empty) | Root `Container` swaps between `PaneGrid` and graph canvas widget | Root `View` swaps between `Dock` and canvas `Element` | tie |
| §3.12 Cross-platform reach | Win/macOS/Linux(X11+Wayland) shipping today via libcosmic + iced core | macOS-first; Linux/Windows still stabilizing in Zed (2026-Q1) | iced |

The two **load-bearing axes** for a browser shell are §3.1 (Servo embedding)
and §3.7 (a11y). iced wins both today: `iced_webview` has a working Servo
path, gpui needs the Glass-HQ patch landed before it has any path; AccessKit
is in progress on iced and absent on gpui. The **shell-quality wins** for
gpui (palette, virtualized list/table, Tree, focus model, polished Tabs/Dock)
stack on top of those load-bearing wins, not under them.

### Stages of adoption

Each stage is independently shippable; landing one does not commit to the
next.

### GPUI-shaped Graphshell structure

The ecosystem pass suggests a concrete crate and ownership shape for a future
`gpui` experiment:

- Add a host adapter crate, tentatively `crates/gpui-graphshell-host` or
  `crates/gpui-shell`. It should depend on `graphshell-runtime`,
  `graph-canvas`, and the selected GPUI line, but `graphshell-runtime` must
  not depend on GPUI.
- Mirror the current iced adapter boundary rather than rewriting domain
  logic. `crates/iced-graph-canvas-viewer` becomes the comparison point for a
  future `crates/gpui-graph-canvas-viewer`: translate GPUI input/layout/paint
  into existing `graph-canvas` camera, hit-test, scene, and interaction types.
- Wrap existing `app/*` domain state in GPUI-owned runtime objects at the
  host boundary. There are two viable shapes: raw GPUI `Entity<T>` models for
  a fully idiomatic experiment, or `gpui-tea` `Model`/`Program` values for a
  lower-risk TEA bridge. Natural first models are `GraphModel`,
  `NavigatorModel`, `WorkbenchModel`, `ViewerModel`, and `ShellModel`,
  matching the runtime five-domain split instead of one monolithic iced
  `Message` loop.
- Keep `ActionSurfaceState`, focus-selection state, routing, and workspace
  commands host-neutral. The GPUI layer should bind keyboard shortcuts and
  menus to GPUI `actions!`, then dispatch into the existing command/intention
  vocabulary.
- Treat graph canvas as a GPUI-native custom `canvas`/`Element` first. The
  `gpui-ce` custom drawing and data-table examples show that native
  `canvas`, `PathBuilder`, `window.paint_path`, `uniform_list`, scroll
  handles, and direct mouse-event hit-testing are enough for a serious first
  prototype. Only require Vello/shared-wgpu if native paths are insufficient
  for edge quality or scale.
- Treat Navigator/web content differently from graph canvas: it remains a
  cross-renderer texture-composition problem. Do not model it as a GPUI image
  or canvas unless the goal is a temporary pixel-copy fallback. The durable
  route is still `PaintSurface::WgpuTexture` against Glass-HQ/upstream Zed.
- Make Phase 0 a tiny GPUI sample app before touching Graphshell: one window,
  one host-owned model, one graph-canvas-style custom canvas, one simulated
  external texture, and a command palette action. That proves the architecture
  seams independently from Servo/NetRender complexity.

### `gpui-tea` bridge option

`gpui-tea` deserves its own branch-spike if the GPUI experiment continues. It
is not a renderer solution and does not change the external-texture gap, but it
may substantially reduce app-architecture migration risk.

Pros:

- It maps closely to Graphshell's existing TEA-ish iced shape: explicit
  messages, `update`, `view`, commands/effects, and subscriptions.
- It mounts as a GPUI `Entity<Program<M>>`, so it still lives inside GPUI's
  app/window model and can coexist with GPUI elements and gpui-component.
- It supports keyed latest-wins foreground/background effects and cancellation,
  which maps well to Navigator loads, content-surface lifecycle changes,
  graph-layout recomputation, search queries, command-palette queries, and
  MCP/agent tasks.
- Declarative subscriptions keyed by stable identity are a strong fit for
  frame ticks, runtime event streams, file/watch events, network/runtime
  lifecycle notifications, and per-surface producer streams.
- The `Composite` derive and explicit child paths are a clean bridge from a
  monolithic shell model to the five-domain split without leaking GPUI types
  into `graphshell-runtime`.
- Queue policies and telemetry hooks are directly useful for backpressure and
  diagnostics; Graphshell already treats runtime observability as a first-class
  concern.

Risks:

- `gpui-tea` depends on crates.io `gpui = 0.2.2`. The GPUI renderer patch work
  may target Glass-HQ or upstream Zed git, so compatibility must be proven. A
  fork or patch may be required if the selected GPUI line diverges.
- It can preserve too much of the iced-era pull/message architecture. If used
  permanently for every subsystem, Graphshell may miss GPUI's finer-grained
  observer re-render model.
- It introduces another runtime layer exactly where Graphshell already has a
  runtime crate. The right use is host-boundary orchestration, not replacing
  `graphshell-runtime`'s host-neutral contracts.
- The library is young (`0.1.x`, first released March 2026). Treat it as a
  spike candidate, not a foundational commitment until compatibility and
  maintenance pace are clear.

Recommended spike: build the Phase 0 GPUI sample twice — once with raw GPUI
`Entity` models and once as a `gpui-tea::Program`. Compare command dispatch,
subscription/backpressure behavior, focus integration, and how much glue is
required to keep `graphshell-runtime` host-neutral. If `gpui-tea` wins, make it
Stage A's app shell wrapper; if not, keep the architectural lessons and proceed
with raw GPUI entities.

#### Stage A — Portable-contract shim (post-Phase 3)

A single root `View` calls `runtime.tick()` inside `render`. The whole UI
rebuilds each tick, like in iced. gpui-component provides the chrome (Dock,
Tabs, Palette). Custom Elements for graph canvas and `WebViewSurface` use the
Glass-HQ external-texture surface. Functional gpui app; reactivity unused.

Done condition: the same UX target the iced host hits, on gpui.

#### Stage B — Action dispatch through gpui actions

`ActionRegistry`'s `namespace:name` actions become gpui `actions!`
declarations. Key bindings register through `cx.bind_keys`. `HostIntent`
variants become payload types on those actions. The omnibar and command
palette dispatch through gpui rather than collecting a Message in iced
fashion.

Buys: native gpui-component `CommandPalette` integration; idiomatic key
binding; no bespoke routing layer.

#### Stage C — Runtime decomposition into observable models

Split the monolithic Runtime into per-domain `Model<T>` instances aligned
with [SHELL.md](SHELL.md)'s five domains: Graph, Navigator, Workbench,
Viewer, Shell. Each Model holds its slice of state and `cx.notify()` on
mutation. Views observe only the slices they render.

Buys: per-View re-render scoping. The Navigator sidebar re-renders when
graphlet membership changes, not when a frame border drags. The Diagnostics
Inspector re-renders when a new event arrives, not when the graph canvas
pans.

Cost: the portable contract loses its single-tick-per-frame shape. Stage C
moves pull-loop semantics out of the host and into a `Timer` subscription on
the Graph and Workbench Models (driving physics + animation), with
everything else event-driven. The portable types must stay free of gpui
types — `graphshell-runtime` continues to expose plain `Model`-of-T-friendly
state, and the gpui host wraps each domain in `Model` at the boundary.

#### Stage D — Custom Elements for graph canvas and `WebViewSurface`

Stage A renders both as opaque painted regions. Stage D makes them
first-class gpui `Element` impls:

- `GraphCanvasElement` — Vello scene submitted directly to gpui's wgpu
  pass; hit-testing in `layout()`; pan/zoom state in the Element's View.
- `WebViewSurfaceElement` — texture lifecycle, content generation signals,
  pointer/keyboard/IME forwarding to Servo through `web_runtime`. Built on
  `PaintSurface::WgpuTexture` (the patch in §Findings).

Buys: native focus integration (Tab cycles into the canvas), gpui-style
input event flow, no shader-widget wrapper.

#### Stage E — `FocusHandle` integration with six-track focus

The six-track `RuntimeFocusAuthorityState` (G1 in the iced jump-ship plan)
carries authority for SemanticRegion, PaneActivation, GraphView,
LocalWidget, EmbeddedContent, ReturnCapture. In gpui:

- LocalWidget = gpui's native `FocusHandle` on the widget.
- GraphView and EmbeddedContent = `FocusHandle` on the canvas /
  `WebViewSurface` Element.
- PaneActivation = `FocusHandle` on each Pane View.
- SemanticRegion and ReturnCapture stay runtime-side (host-neutral); they
  coordinate with gpui's focus by observing it.

This is the cleanest fit gpui offers. Zed's focus-return-on-modal-close is
exactly the ReturnCapture pattern.

#### Stage F — AccessKit

The largest gap. gpui has no public AccessKit story; Zed itself does not
yet ship AccessKit. Three options:

1. Build AccessKit support inside the Graphshell `gpui-shell` crate
   (host-side, not upstream).
2. Contribute AccessKit support to gpui upstream (large; uncertain
   reception).
3. Wait for upstream to add it (no signal as of 2026-04-29).

Option 1 is the only one Graphshell can drive on its own timeline. The
Graph Reader (planned virtual a11y tree) is host-neutral and lands either
way; the gap is the rendered-tree side.

### Gaps that block "fully idiomatic"

These are not decisions to make today; they are the surface that has to be
navigated if the experiment is revived past Stage A.

- **AccessKit on gpui** — see Stage F. WCAG 2.2 AA target makes this
  load-bearing.
- **IME on Linux** — gpui's Wayland fcitx/ibus story is behind iced's.
  Affects CJK / Arabic users on the largest Graphshell-target platform.
- **`PaintSurface::WgpuTexture` patch** — Stage A through D depend on the
  Glass-HQ patch surviving upstream churn or being merged.
- **Runtime model decomposition (Stage C)** — touches the boundary of the
  portable contract. Done carelessly, it leaks gpui types into
  `graphshell-runtime`. Done carefully, it cleanly aligns the runtime with
  the five-domain split that is already canonical in
  [SHELL.md](SHELL.md).
- **gpui-component pace** — single-vendor (Longbridge); if upstream
  investment slows, the chrome-polish argument weakens.
- **Cross-platform parity of Linux/Windows ports** — still stabilizing in
  upstream Zed as of 2026-Q1.

### What this sharpens about the revisit trigger

The 2026-04-29 progress entry below records the revisit trigger as four
named conditions. Stages A–F give the revisit a concrete shape: not "switch
hosts" but "switch hosts and absorb adaptation cost X." Stage A alone is a
real migration project; Stages B–F are layered. Picking the right stopping
point depends on which trigger actually fired:

- Trigger 1 (omnibar / palette quality) → Stage A is sufficient; gpui-component
  delivers immediately.
- Trigger 2 (Navigator sidebar / Diagnostics perf at scale) → Stages A + C
  are needed; the perf win comes from per-Model observation, not from
  gpui-component alone.
- Trigger 3 (six-track focus reconciliation kludge) → Stages A + E.
- Trigger 4 (AccessKit stall on iced) → Stages A + F. Stage F is where the
  cost shape is least certain, and that uncertainty is itself a reason to
  let the iced AccessKit work mature first.

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

**2026-04-29** — ecosystem calibration. Two findings in §Findings were
corrected after a focused iced-vs-gpui ecosystem survey:

1. The "substantially richer than iced's current widget set" framing
   compared against bare iced rather than the realistic iced stack
   (iced + `iced_aw` + `libcosmic` + `iced_webview`). Replaced with a
   calibration note enumerating where gpui-component genuinely wins
   (command palette, virtualized Table/List, code editor) and where
   iced wins (Servo embedding via `iced_webview`, multi-platform reach
   via libcosmic, AccessKit in progress, documented custom-wgpu path
   since [iced-rs/iced#183](https://github.com/iced-rs/iced/pull/183)).
2. The "March 2026 Blade-to-wgpu migration" was misattributed to
   Glass-HQ. The actual migration is upstream Zed
   ([zed-industries/zed#46758](https://github.com/zed-industries/zed/pull/46758),
   merged Feb 2026). Glass-HQ inherits upstream's wgpu work.

Plan posture unchanged: GPUI remains a branch experiment after iced
stabilizes. The calibration sharpens the case for *when* to revisit
(when the iced chrome work surfaces a chrome-quality bar gpui-component
clearly clears that the iced stack does not) rather than leaving the
revisit trigger ambiguous.

**2026-04-29** — GPUI ecosystem expansion pass started on branch `gpui`.
Added `gpui-ce/gpui-ce`, `gpui.rs`, and `awesome-gpui` to the survey. Initial
conclusion: `gpui-ce` is valuable as a compact learning corpus for GPUI app
shape (`Entity` models, `Render` views, context use, actions/key contexts,
`canvas`, `PathBuilder`, `uniform_list`, custom scroll/hit-test logic), but
it is not a strong shared-`wgpu` patch base because its Linux renderer remains
on `wgpu = "24"`, while macOS/Windows remain backend-native. `awesome-gpui`
adds three serious Graphshell research threads: `gpui-component` for shell
chrome, `gpui-flow`/`ferrum-flow` for graph canvas patterns, and `gpui-tea` as
an app-architecture bridge. The practical Graphshell structure to research next
is: GPUI-native chrome behind Graphshell abstractions; graph canvas as a GPUI
custom `canvas`/element first, with `gpui-flow`/`ferrum-flow` mined for
interaction patterns and Vello/shared-wgpu required only if native paths are
insufficient; Navigator surfaces behind a narrow external-texture patch against
Glass-HQ/upstream Zed rather than `gpui-ce`.

**2026-04-29** — `gpui-tea` examined as a serious bridge option. It is a young
Apache-2.0 crate (`gpui_tea` 0.1.1) that depends on crates.io `gpui = 0.2.2`
and provides TEA-style `Model`, `Program`, `Command`, `Subscription`, keyed
latest-wins async effects, cancellation, queue policies, nested `Composite`
models, and runtime telemetry. It does not solve renderer integration, but it
may reduce GPUI host migration risk by preserving Graphshell's existing
message/update/subscription architecture while mounting inside GPUI. Next spike:
implement the tiny Phase 0 GPUI sample once with raw GPUI `Entity` models and
once with `gpui-tea::Program`, then compare compatibility with the selected
GPUI line, command/focus integration, and host-neutral runtime cleanliness.
