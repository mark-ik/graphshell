<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Servo WPT Mountain and WebRender/wgpu Leverage

**Date**: 2026-04-10  
**Status**: Research note / backlog-shaping input  
**Purpose**: Map the current Servo WPT mountain into actionable buckets, with
special attention to what the current WebRender/wgpu and feature-bump work can
realistically move.

**Related docs**:

- [`2026-03-01_webrender_wgpu_renderer_research.md`](2026-03-01_webrender_wgpu_renderer_research.md)
- [`../technical_architecture/2026-03-29_middlenet_engine_spec.md`](../technical_architecture/2026-03-29_middlenet_engine_spec.md)
- [`../../../../webrender/wr-wgpu-notes/2026-03-01_webrender_wgpu_renderer_implementation_plan.md`](../../../../webrender/wr-wgpu-notes/2026-03-01_webrender_wgpu_renderer_implementation_plan.md)
- [`../../../../webrender/wr-wgpu-notes/servo_wgpu_integration.md`](../../../../webrender/wr-wgpu-notes/servo_wgpu_integration.md)

---

## 1. Executive Summary

Servo's remaining WPT debt is large, but it is not one undifferentiated
mountain.

The current upstream snapshot splits more usefully into three different ridges:

1. A **renderer-adjacent mountain** that the current WebRender/wgpu work can
   plausibly move.
2. A **layout/style mountain** that is large, important, and mostly not solved
   by compositor/backend work.
3. A **runtime/API mountain** that sits in storage, service workers, fetch,
   timing, input, and other DOM/platform layers.

The most important quantitative conclusion is:

- Servo currently has about **17,512** checked-in WPT expectation `.ini` files
  under `tests/wpt/meta`.
- The most strategically relevant renderer-adjacent surface is still large:
  about **3,345 expectation files** across canvas, SVG, and CSS
  paint/composite buckets.
- That renderer-adjacent surface is therefore not a rounding error. It is about
  **19.1%** of all current WPT expectation metadata.

The most important prioritization conclusion is:

1. **Canvas**, especially offscreen canvas, is the best direct attack lane for
   the current branch.
2. **CSS paint/composite** is the best large mixed-yield lane.
3. **SVG paint/styling/geometry** looks promising, but `svg/animations` in
   particular should be treated as only partially renderer-driven.
4. **Feature bumping helps most when it exposes work the renderer branch is
   already positioned to improve**, especially offscreen canvas and WebGPU
   plumbing.
5. **Feature bumping grid/container queries is mostly exposure, not leverage**.

The most important caution is:

- The current branch does **not** meaningfully attack the biggest non-renderer
  mountains such as service workers, IndexedDB, fetch, pointer events, and much
  of grid/flexbox/selectors/layout.

---

## 2. Snapshot and Measurement Notes

### 2.1 Baseline Used

This note is based on the local Servo checkout at:

- local branch: `webrender-wgpu-patch`
- upstream main inspected on 2026-04-10:
  `80c6846dfb7cab8c0b5c2efad0fe0a48cc1c959d`

The local branch currently has **no `tests/wpt/meta` delta** against
`upstream/main`, so the WPT metadata counts below are effectively a current
upstream Servo snapshot as of 2026-04-10.

### 2.2 Sources Used

The main local sources were:

- `servo/tests/wpt/meta`
- `servo/tests/wpt/tests`
- `servo/etc/wpt_result_analyzer.py`
- `servo/Cargo.toml`
- `servo/components/shared/paint/wgpu_rendering_context.rs`
- `servo/components/paint/wgpu_webgl_external_images.rs`
- `servo/components/paint/painter.rs`
- `servo/components/servo/webview.rs`
- `servo/components/config/prefs.rs`
- `servo/ports/servoshell/prefs.rs`
- `servo/resources/wpt-prefs.json`

### 2.3 Caveats

The counts in this note are useful, but they must be interpreted correctly.

1. A metadata `.ini` file is **not** the same thing as one failing test case.
   One file can carry many subtest expectations.
2. Some analyzer percentages can exceed 100% because a single test file can
   have multiple expected failures or reftest/reference expectations.
3. `tests/wpt/meta` is best read as the set of **known expected-problem areas**,
   not as a perfect accounting of every remaining failure.
4. Several platform/API features are default-off in the general config, so the
   relationship between product defaults and WPT execution surface is not always
   one-to-one.

---

## 3. Overall Size of the Mountain

### 3.1 Raw Counts

The current snapshot looks like this:

| Metric | Count | Note |
|---|---:|---|
| Total WPT expectation `.ini` files in `tests/wpt/meta` | 17,512 | Top-level raw metadata count |
| Total WPT test files in `tests/wpt/tests` | 82,702 | Broad file-count surface |
| Paired WPT test files in directories that also have metadata | 57,283 | Aligned to `etc/wpt_result_analyzer.py` logic |
| Paired expectation files | 17,272 | Same paired-sibling logic |
| Paired expectation ratio | 30.15% | Expectation files / paired test files |

If the `expected:` lines inside metadata files are counted directly, the local
picture becomes even clearer:

| Expected status token | Count |
|---|---:|
| `FAIL` | 157,059 |
| `NOTRUN` | 3,291 |
| `ERROR` | 3,288 |
| `TIMEOUT` | 2,637 |
| `PRECONDITION_FAILED` | 132 |
| `CRASH` | 34 |

This reinforces the central point: the `.ini` count understates how many
subtests or reftest expectations are still being carried.

### 3.2 Top-Level Areas by Metadata Volume

The biggest top-level areas in `tests/wpt/meta` are:

| Area | Metadata files | Share of all metadata |
|---|---:|---:|
| `css` | 7,962 | 45.5% |
| `html` | 3,367 | 19.2% |
| `svg` | 860 | 4.9% |
| `content-security-policy` | 340 | 1.9% |
| `fetch` | 314 | 1.8% |
| `service-workers` | 289 | 1.7% |
| `referrer-policy` | 222 | 1.3% |
| `IndexedDB` | 201 | 1.1% |
| `dom` | 200 | 1.1% |
| `webaudio` | 193 | 1.1% |
| `editing` | 156 | 0.9% |
| `bluetooth` | 153 | 0.9% |
| `pointerevents` | 150 | 0.9% |
| `wasm` | 140 | 0.8% |
| `workers` | 131 | 0.7% |
| `shadow-dom` | 120 | 0.7% |

Two immediate observations matter:

- CSS is still the dominant bucket by far.
- HTML + SVG + CSS paint/composite subtrees together are large enough that
  renderer work can produce meaningful visible progress, even though it will
  not erase the whole mountain.

### 3.3 Useful Bucketization

The following buckets are the most useful planning split for current work.
These are selected planning buckets, not an exhaustive partition of all WPT
metadata.

| Bucket | Metadata files | Share of all metadata | Likely relevance to current WR/wgpu work |
|---|---:|---:|---|
| Canvas (`html/canvas/offscreen` + `html/canvas/element`) | 1,116 | 6.4% | High |
| SVG (`svg`) | 860 | 4.9% | Medium to high |
| CSS paint/composite (`filter-effects`, `css-images`, `css-backgrounds`, `css-transforms`, `css-shadow`, `css-masking`, `css-paint-api`) | 1,369 | 7.8% | Medium to high |
| Renderer-adjacent total | 3,345 | 19.1% | Strategic core |
| CSS layout core (`css-grid`, `css-flexbox`, `css-sizing`, `css-overflow`, `css-gaps`, `css-align`) | 2,667 | 15.2% | Mostly low |
| Runtime/storage/network (`service-workers`, `IndexedDB`, `fetch`, `resource-timing`, `event-timing`, `largest-contentful-paint`, `websockets`, `xhr`) | 1,188 | 6.8% | Low |
| Input/interaction (`pointerevents`, `input-events`, `selection`, `editing`, `fullscreen`, `clipboard-apis`) | 477 | 2.7% | Low |
| Selected feature/API buckets (`fedcm`, `cookiestore`, `webauthn`, `bluetooth`, `streams`, `close-watcher`) | 445 | 2.5% | Low except for exposure |

This is the key planning split for the rest of the note.

---

## 4. Why the Current Branch Has Real Leverage

The current branch is not just a generic "graphics modernization" effort. It
already contains branch-local mechanics that create plausible WPT leverage in
renderer-adjacent areas.

### 4.1 Cargo and Dependency Topology

`servo/Cargo.toml` shows the important shape directly:

- WebRender is built with `wgpu_backend` at `Cargo.toml:214`.
- Servo is using `wgpu = "29"` at `Cargo.toml:216`.
- Servo patches WebRender to the local checkout at `Cargo.toml:370-371`.

That means this is not an abstract future plan. The local Servo tree is already
wired to consume a locally modified WebRender checkout.

### 4.2 Pure-wgpu Rendering Context

`components/shared/paint/wgpu_rendering_context.rs` establishes a real
pure-wgpu rendering path:

- the file is explicitly described as a `RenderingContext` "backed entirely by
  wgpu"
- it creates its own surface via `create_surface` at line 52
- it requests features including `DUAL_SOURCE_BLENDING` at line 65
- it exposes `backend_binding` at line 190
- it exposes `acquire_wgpu_frame_target` at line 197

This matters because a meaningful slice of renderer-related WPT debt lives in
areas where correct render-target behavior, compositing, filter execution, and
surface/view handling are central.

### 4.3 Zero-Copy or Near-Zero-Copy Composite Output

`components/servo/webview.rs` now exposes composite output in a way that is
strategically important:

- `WebView::composite_texture()` returns a `wgpu::Texture` at line 410
- the comments at lines 400-407 explicitly describe the shared-device,
  no-extra-copy path

The `examples/wgpu-embedder/src/main.rs` example is similarly explicit:

- line 1 says it is "A pure-wgpu Servo embedder using the zero-copy
  render_to_view() path"
- lines 4-8 describe Servo/WebRender rendering directly into the surface
  texture with no intermediate blit

This gives the current branch unusually direct leverage over:

- compositing correctness
- canvas presentation paths
- external-image interop
- final-pass renderer behavior that used to be trapped behind GL bridging

### 4.4 WebGL-to-wgpu External Image Bridge

`components/paint/wgpu_webgl_external_images.rs` is a strong signal that the
branch is not only about top-level composition:

- the file declares a "WebGL external image handler for the wgpu backend"
- `lock_wgpu()` at line 72 imports WebGL surfaces into wgpu
- `unlock_wgpu()` at line 173 returns them cleanly

This is directly relevant to canvas- and image-heavy WPT areas where texture
ownership, external image handling, and cross-subsystem compositing matter.

### 4.5 Actual Render-to-View Use in the Painter

This is not only an API surface. `components/paint/painter.rs` shows real usage:

- the file installs a wgpu WebGL external image handler around lines 355-375
- the renderer uses `render_to_view()` at line 579

Taken together, the branch is already positioned to affect WPT families that
depend on:

- render target correctness
- intermediate surface use
- filter and blend behavior
- canvas image presentation
- texture import/export and composition

---

## 5. Most Relevant Areas to Current WR/wgpu Work

This section answers the narrow question: **where is the current branch most
relevant, not merely theoretically related?**

### 5.1 Direct-Relevance Buckets

These are the strongest current candidates because they line up closely with the
branch's render-target, compositor, surface, and external-image work.

| Directory | Metadata files | Relevance | Why |
|---|---:|---|---|
| `html/canvas/offscreen/text` | 183 | High | Offscreen surface lifecycle plus text draw/present path |
| `html/canvas/offscreen/layers` | 140 | High | Intermediate surfaces and layered composition |
| `html/canvas/offscreen/path-objects` | 120 | High | Canvas draw pipeline on offscreen target |
| `html/canvas/offscreen/shadows` | 72 | High | Blur/shadow/filter-like offscreen behavior |
| `html/canvas/offscreen/filters` | 52 | High | Direct filter pipeline relevance |
| `html/canvas/offscreen/compositing` | 40 | High | Blend/composite correctness |
| `html/canvas/element/path-objects` | 63 | High | Direct 2D canvas render correctness |
| `html/canvas/element/text` | 55 | High | Canvas text rendering and output correctness |
| `html/canvas/element/layers` | 54 | High | Layer and intermediate-surface behavior |
| `html/canvas/element/shadows` | 35 | High | Shadow pipeline relevance |
| `html/canvas/element/filters` | 26 | High | Direct filter pipeline relevance |
| `html/canvas/element/compositing` | 20 | High | Blend/composite correctness |

Area totals:

- `html/canvas/offscreen`: **724**
- `html/canvas/element`: **392**
- Canvas total: **1,116**

This is the single strongest immediate lane.

### 5.2 Strong but Mixed-Relevance Buckets

These are large and important, but not every failure inside them is guaranteed
to be fixed by renderer work alone.

| Directory | Metadata files | Relevance | Why |
|---|---:|---|---|
| `css/css-images` | 182 | Medium to high | Image rendering and gradient/image treatment often hit paint paths |
| `css/filter-effects` | 158 | High | Directly tied to filters and intermediate surfaces |
| `css/css-backgrounds/background-size/vector` | 142 | Medium to high | Strong render/path/image angle |
| `css/css-paint-api` | 108 | Medium | Paint-side output matters, but worklet/runtime semantics also matter |
| `css/css-masking/clip-path` | 78 | Medium to high | Paint/composite/clip pipeline overlap |
| `css/css-backgrounds` | 66 | Medium | Mixed parsing/layout/paint bucket |
| `css/css-transforms/animation` | 60 | Medium | Paint/composite plus animation semantics |
| `css/css-shadow` | 57 | Medium to high | Shadow pipeline overlap |
| `css/css-backgrounds/animations` | 53 | Medium | Paint plus animation semantics |
| `css/css-transforms/transform-origin` | 50 | Medium | Transform math and paint |
| `css/css-transforms` | 48 | Medium | Mixed |
| `css/css-transforms/matrix` | 40 | Medium | Transform correctness |
| `css/css-images/gradient` | 35 | Medium | Rendering-heavy gradient surface |
| `css/css-transforms/transform-box` | 34 | Medium | Transform/render integration |
| `css/css-backgrounds/background-clip` | 29 | Medium | Clip/composite surface overlap |

Area totals:

- `css/filter-effects`: **195**
- `css/css-images`: **247**
- `css/css-backgrounds`: **336**
- `css/css-transforms`: **307**
- `css/css-shadow`: **66**
- `css/css-masking`: **106**
- `css/css-paint-api`: **112**
- CSS paint/composite total: **1,369**

This is the biggest mixed-yield lane by raw metadata volume.

### 5.3 Probable-Relevance SVG Buckets

SVG is a real renderer-adjacent mountain, but it is not uniformly the same kind
of work. Some SVG failures are painter/compositor problems; others are DOM,
styling, invalidation, or animation semantics.

| Directory | Metadata files | Relevance | Why |
|---|---:|---|---|
| `svg/animations` | 259 | Medium | Large, but not pure renderer work |
| `svg/types/scripted` | 76 | Medium | Renderer + DOM/script split |
| `svg/painting/parsing` | 61 | Medium to high | Paint-side behavior matters |
| `svg/styling` | 44 | Medium to high | Rendered styling output still central |
| `svg/painting/reftests` | 28 | Medium to high | Reftest renderer surface |
| `svg/geometry/parsing` | 24 | Medium | Geometry and render integration |
| `svg/interact/scripted` | 20 | Medium | Interaction plus render |
| `svg/geometry/reftests` | 17 | Medium | Geometry/render surface |
| `svg/linking/scripted` | 17 | Medium | Not purely renderer |
| `svg/extensibility/foreignObject` | 16 | Medium | Render integration surface |
| `svg/shapes` | 16 | Medium | Render/math surface |
| `svg/path/parsing` | 15 | Medium | Shape/path interpretation |

Useful SVG subtotals:

- `svg/animations`: **271** if scripted sub-bucket is included
- `svg/painting`: **109**
- `svg/styling`: **56**
- `svg/geometry`: **53**

The key practical reading is:

- **SVG paint/styling/geometry** is a reasonable renderer lane.
- **SVG animations** should be treated as a mixed lane with more semantic risk.

---

## 6. Highest-Impact Attack Order

This section is not asking "what is most relevant?" It asks the stricter
question: **what should be attacked first if the goal is meaningful WPT
movement per unit effort, while staying aligned with the current WR/wgpu
branch?**

### 6.1 Lane A: Canvas First

**Recommendation**: First priority.

**Why**:

- It is the cleanest overlap with current branch mechanics.
- It is big enough to matter: **1,116 metadata files**.
- It directly exercises offscreen targets, surface acquisition, final composited
  output, filters, shadows, image presentation, and texture lifecycle.
- It aligns well with the branch's `render_to_view()`, pure-wgpu rendering
  context, and external image handling.

**Best immediate targets**:

1. `html/canvas/offscreen/text`
2. `html/canvas/offscreen/layers`
3. `html/canvas/offscreen/path-objects`
4. `html/canvas/offscreen/filters`
5. `html/canvas/offscreen/shadows`
6. `html/canvas/element/path-objects`
7. `html/canvas/element/text`
8. `html/canvas/element/layers`

**Why this beats some larger CSS buckets**:

- The fix surface is more tightly aligned with current code.
- The pass signal should be easier to interpret.
- Failures are more likely to be immediately visible and parity-checkable.

### 6.2 Lane B: CSS Paint/Composite Cluster

**Recommendation**: Second priority.

**Why**:

- It is the largest renderer-mixed bucket at **1,369 metadata files**.
- It likely benefits from the same render-target, filter, texture, and
  compositing improvements that help canvas.
- It is likely to produce visible product-quality wins in addition to pass
  count wins.

**Best targets inside this lane**:

1. `css/filter-effects`
2. `css/css-images`
3. `css/css-backgrounds/background-size/vector`
4. `css/css-masking/clip-path`
5. `css/css-shadow`
6. selected `css/css-transforms` buckets

**Important caution**:

- This lane is mixed. Some failures will turn out to be parsing, layout,
  interpolation, invalidation, or animation semantics rather than pure renderer
  bugs.

### 6.3 Lane C: SVG Paint/Styling/Geometry, Then Selective Animations

**Recommendation**: Third priority.

**Why**:

- SVG as a whole is large: **860 metadata files**.
- The renderer-relevant slice is real, especially painting, styling, and
  geometry.
- The branch's render/composite work is likely to help here, especially where
  reftests are involved.

**Priority inside SVG**:

1. `svg/painting`
2. `svg/styling`
3. `svg/geometry`
4. only then selective `svg/animations`

**Why not put `svg/animations` first despite its raw size?**

- Animation failures often mix timing, invalidation, and DOM semantics with
  renderer output.
- This makes them harder to use as a clean backend-progress bar.

### 6.4 Lane D: Exposure Bumps Only Where They Help the Current Work

**Recommendation**: Use feature bumps surgically, not as a generic progress
strategy.

**Good exposure candidates**:

- `dom_offscreen_canvas_enabled`
- `dom_webgpu_enabled`

**Lower-leverage exposure candidates**:

- `layout_grid_enabled`
- `layout_container_queries_enabled`

**Reason**:

- Offscreen canvas and WebGPU exposure line up with the current branch's
  actual work.
- Grid and container queries mostly expose a large style/layout mountain that
  this branch does not directly solve.

---

## 7. Feature-Bumping Analysis

The current codebase shows several important things about feature exposure.

### 7.1 What Is Default-Off in the General Config

`components/config/prefs.rs` shows the following defaults:

- `dom_offscreen_canvas_enabled: false` at line 375
- `dom_webgpu_enabled: false` at line 399
- `layout_container_queries_enabled: false` at line 466
- `layout_grid_enabled: false` at line 468
- `dom_bluetooth_enabled: false` at line 350
- `dom_cookiestore_enabled: false` at line 357
- `dom_indexeddb_enabled: false` at line 367
- `dom_serviceworker_enabled: false` at line 380
- `dom_webrtc_enabled: false` at line 401

At the same time:

- `dom_canvas_text_enabled: true` at line 353

That last point matters. It implies that many **canvas text** failures are not
merely hidden-behind-a-pref debt. They are already part of the active
implementation surface.

### 7.2 What ServoShell Treats as Experimental

`ports/servoshell/prefs.rs` marks several relevant prefs as experimental:

- `dom_offscreen_canvas_enabled`
- `dom_webgpu_enabled`
- `layout_container_queries_enabled`
- `layout_grid_enabled`

This is a useful signal for product readiness, but it should not be confused
with WPT pass leverage.

### 7.3 What the Checked-In Shared WPT Prefs File Actually Enables

`resources/wpt-prefs.json` is minimal:

- line 2: `dom_webxr_test`
- line 3: `editing_caret_blink_time`
- line 4: `gfx_text_antialiasing_enabled`
- line 5: `dom_testutils_enabled`

Notably, that file does **not** globally enable:

- offscreen canvas
- WebGPU
- grid
- container queries
- service workers
- IndexedDB

This does not prove the WPT runner never enables them elsewhere, but it does
mean the checked-in shared prefs evidence is conservative rather than
aggressively feature-on.

### 7.4 Practical Conclusions

Feature bumping is useful in two very different ways:

1. **Exposure bumping**:
   turn on a feature so WPT can exercise it more broadly.
2. **Leverage bumping**:
   turn on a feature that the current implementation branch is already well
   positioned to improve.

For this branch:

- `dom_offscreen_canvas_enabled` is a **good leverage bump**
- `dom_webgpu_enabled` is a **good leverage bump**
- `layout_grid_enabled` is mostly an **exposure bump**
- `layout_container_queries_enabled` is mostly an **exposure bump**

The risk with indiscriminate bumping is that it makes the mountain look larger
without creating a correspondingly useful attack surface.

---

## 8. What WR/wgpu Work Is Unlikely to Move Much

This is the main anti-confusion section. Several of the highest-count backlog
areas are important, but they are not where current renderer/backend work has
its strongest leverage.

### 8.1 High-Count Areas with Low Renderer Leverage

| Directory | Metadata files | Likely leverage from current branch | Why |
|---|---:|---|---|
| `css/css-grid/alignment` | 250 | Low | Layout algorithm and alignment semantics |
| `service-workers/service-worker` | 245 | Very low | Lifecycle, fetch interception, worker/runtime model |
| `css/css-overflow/line-clamp` | 212 | Low | Layout/line breaking/fragmentation |
| `css/css-fonts` | 209 | Low to medium | Font selection/parsing/layout more than compositor |
| `IndexedDB` | 200 | Very low | Storage/runtime implementation |
| `css/css-flexbox` | 198 | Low | Layout algorithm |
| `css/css-text-decor` | 189 | Low to medium | Mostly style/layout/text semantics |
| `css/css-grid/abspos` | 173 | Low | Grid + abspos layout semantics |
| `css/selectors` | 167 | Very low | Selector matching/invalidation |
| `css/css-conditional/container-queries` | 157 | Low | Style/layout evaluation and invalidation |
| `css/css-pseudo` | 151 | Low | Selector/style semantics |
| `css/css-sizing/aspect-ratio` | 148 | Low | Layout sizing rules |
| `css/css-overflow/scroll-markers` | 147 | Low | Layout and scroll behavior |
| `pointerevents` | 122 | Very low | Input/event dispatch |
| `html/dom/elements/global-attributes` | 111 | Very low | DOM/HTML semantics |

These are all real backlog, but they should not be used as the primary scorecard
for current WR/wgpu success.

### 8.2 Mostly-Unimplemented Feature Families

Several families look close to "mostly incomplete" rather than "mostly there
with a few renderer bugs."

Examples from the paired test/metadata analysis:

| Directory | Test files | Metadata files | Ratio |
|---|---:|---:|---:|
| `service-workers/service-worker` | 250 | 245 | 98.00% |
| `IndexedDB` | 225 | 200 | 88.89% |
| `resource-timing` | 100 | 85 | 85.00% |
| `event-timing` | 71 | 61 | 85.92% |
| `largest-contentful-paint` | 61 | 55 | 90.16% |
| `fedcm` | 39 | 39 | 100.00% |
| `close-watcher/user-activation` | 36 | 36 | 100.00% |
| `webaudio/the-audio-api/the-audioworklet-interface` | 37 | 37 | 100.00% |
| `clipboard-apis` | 28 | 25 | 89.29% |

These areas matter, but they are separate programs of work.

---

## 9. Broader Backlog Map

The following long list is a useful overall hotspot view, annotated with rough
relevance to the current branch.

### 9.1 Top Hotspots Overall

| Directory | Metadata files | Planning tag |
|---|---:|---|
| `svg/animations` | 259 | WR-probable, mixed |
| `css/css-grid/alignment` | 250 | Layout/style |
| `service-workers/service-worker` | 245 | Runtime |
| `css/css-overflow/line-clamp` | 212 | Layout/style |
| `css/css-fonts` | 209 | Layout/style |
| `IndexedDB` | 200 | Runtime |
| `css/css-flexbox` | 198 | Layout/style |
| `css/css-text-decor` | 189 | Layout/style |
| `html/canvas/offscreen/text` | 183 | WR-direct |
| `css/css-images` | 182 | Paint-mixed |
| `css/css-grid/abspos` | 173 | Layout/style |
| `css/selectors` | 167 | Layout/style |
| `css/filter-effects` | 158 | Paint-mixed |
| `css/css-conditional/container-queries` | 157 | Layout/style |
| `css/css-pseudo` | 151 | Layout/style |
| `css/css-sizing/aspect-ratio` | 148 | Layout/style |
| `css/css-overflow/scroll-markers` | 147 | Layout/style |
| `css/css-backgrounds/background-size/vector` | 142 | Paint-mixed |
| `html/canvas/offscreen/layers` | 140 | WR-direct |
| `css/css-text/white-space` | 140 | Layout/style |
| `pointerevents` | 122 | Input/runtime |
| `html/canvas/offscreen/path-objects` | 120 | WR-direct |
| `css/css-ui` | 117 | Mixed, mostly not renderer |
| `html/dom/elements/global-attributes` | 111 | Runtime/DOM |
| `css/css-paint-api` | 108 | Paint-mixed |

### 9.2 Highest-Value WR/wgpu-Adjacent Long List

This is the long list most worth using as a branch-local scoreboard.

| Directory | Metadata files |
|---|---:|
| `svg/animations` | 259 |
| `html/canvas/offscreen/text` | 183 |
| `css/css-images` | 182 |
| `css/filter-effects` | 158 |
| `css/css-backgrounds/background-size/vector` | 142 |
| `html/canvas/offscreen/layers` | 140 |
| `html/canvas/offscreen/path-objects` | 120 |
| `css/css-paint-api` | 108 |
| `css/css-masking/clip-path` | 78 |
| `svg/types/scripted` | 76 |
| `html/canvas/offscreen/shadows` | 72 |
| `css/css-backgrounds` | 66 |
| `html/canvas/element/path-objects` | 63 |
| `svg/painting/parsing` | 61 |
| `css/css-transforms/animation` | 60 |
| `css/css-shadow` | 57 |
| `html/canvas/element/text` | 55 |
| `html/canvas/element/layers` | 54 |
| `css/css-backgrounds/animations` | 53 |
| `html/canvas/offscreen/filters` | 52 |
| `css/css-transforms/transform-origin` | 50 |
| `css/css-transforms` | 48 |
| `svg/styling` | 44 |
| `css/css-transforms/matrix` | 40 |
| `html/canvas/offscreen/compositing` | 40 |
| `css/css-images/gradient` | 35 |
| `html/canvas/element/shadows` | 35 |
| `css/css-transforms/transform-box` | 34 |
| `css/css-backgrounds/background-clip` | 29 |
| `svg/painting/reftests` | 28 |
| `html/canvas/element/filters` | 26 |
| `svg/struct/reftests` | 26 |
| `svg/geometry/parsing` | 24 |
| `html/canvas/element/fill-and-stroke-styles` | 22 |
| `html/canvas/element/compositing` | 20 |
| `html/canvas/offscreen/reset` | 20 |
| `svg/interact/scripted` | 20 |

The most important reading of this list is that the current branch has a
non-trivial, high-signal measurement surface that is much more informative than
watching grid or service-worker counts.

---

## 10. Recommendations

### 10.1 Primary Recommendation

Treat the WPT mountain as **three mountains**, not one:

1. **Renderer-adjacent**:
   canvas, CSS paint/composite, SVG paint/styling/geometry.
2. **Layout/style**:
   grid, flexbox, sizing, overflow, selectors, pseudo, container queries.
3. **Runtime/API**:
   service workers, IndexedDB, fetch, timing APIs, input/event families.

This prevents success in one lane from being judged by the wrong scoreboard.

### 10.2 Immediate Branch Scoreboard

Use the following as the main WR/wgpu scoreboard:

1. `html/canvas/offscreen/*`
2. `html/canvas/element/*`
3. `css/filter-effects`
4. `css/css-images`
5. `css/css-backgrounds/*` with paint-heavy emphasis
6. `css/css-masking/clip-path`
7. `svg/painting`
8. `svg/styling`
9. `svg/geometry`
10. only then selective `svg/animations`

### 10.3 Feature-Bump Posture

Use feature bumps deliberately:

- bump **offscreen canvas** and **WebGPU** when the goal is to expose and
  validate current WR/wgpu work
- do **not** use grid/container-query bumps as evidence that renderer work is
  progressing well

### 10.4 Backlog-Shaping Posture

When broader Servo progress is discussed, keep these separate work programs:

- renderer/compositor progress
- layout/style progress
- platform/runtime/API implementation progress

Without that separation, the WPT mountain will remain psychologically
misleading and strategically hard to navigate.

---

## 11. Bottom Line

The current WebRender/wgpu work does give Servo real WPT leverage, but the
leverage is concentrated rather than universal.

The strongest claim that the numbers support is:

- there is a **substantial renderer-adjacent WPT surface**
- the current branch is **already wired into the right seams**
- the best immediate payoff is **canvas first, CSS paint/composite second, SVG
  paint/styling/geometry third**

The strongest claim the numbers do **not** support is:

- that current WR/wgpu work is the main path to shrinking the whole Servo WPT
  mountain

It is a crucial path, but it is one path through a larger range.
