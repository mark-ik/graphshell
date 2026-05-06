<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Renderer Policy and Host Refactor Plan (2026-04-30)

**Status**: Active — partial-refresh 2026-05-04 (see "2026-05-04 status note" below)

## 2026-05-04 status note

Three changes since the 2026-04-30 first draft, captured in place rather than
forking the doc:

1. **§6.4 action item #1 is DONE** — the iced jump-ship plan §3.2 already
   cites this plan with the 500-LOC threshold (lines 297, 325, 337 of that
   plan). Cross out of the action list.
2. **§3 (middlenet Vello refactor) and §5 (reader modes) are HOLD** —
   three Netrender paths are concurrently in flight (Vello-powered
   WebRender, idiomatic wgpu WebRender, WebRender with additive wgpu
   backend). The plan's §1.3 "long-horizon Netrender" assumption no
   longer holds; the middlenet refactor away from WebRender would be
   throwaway work if a Netrender path lands days later. Revisit when
   one path lands and we know which.
3. **§4.5 audit target files were stale** — `crates/graphshell-runtime/src/content_surface.rs`
   and `crates/graphshell-runtime/src/ports.rs` (the named files) have
   zero `EmbeddedHost`/`EmbeddedEgui` references today. The 10 files
   that actually still contain those identifiers are listed in §4.5
   below (refreshed).

§6.4 (decomposition target), §4.5 (EmbeddedHost audit), and §2.1 (Wry
validation) remain in scope and proceed.

**Status (original)**: Active
**Scope**: Five related directional decisions from the 2026-04-30
architectural review:

1. **Renderer policy**: Vello for canvas surfaces; WebRender for web/document
   tiles; do not rely on Servo/WebRender stabilization for application
   fundamentals.
2. **Application fundamentals path**: test Wry, refine the GUI bridge, use
   upstream Servo; lean on what we have that isn't Servo's WebRender for
   the next milestone.
3. **Middlenet refactor**: middlenet was shaped as Blitz-top + webrender-
   wgpu; refactor toward Vello + Linebender component crates so it can
   render natively under the new policy.
4. **EmbeddedHost audit**: lean against keeping `EmbeddedHost` as a
   render mode by default; audit for bits worth graverobbing into the
   new shape.
5. **Decomposition target**: lower the portable-crate file size threshold
   from 600 LOC to **500 LOC**; decompose every portable-crate file
   exceeding it.

**Related**:

- [`../shell/2026-04-28_iced_jump_ship_plan.md`](../shell/2026-04-28_iced_jump_ship_plan.md) — §3.2 decomposition target list (threshold lowered by this plan)
- [`../shell/2026-04-24_iced_content_surface_scoping.md`](../shell/2026-04-24_iced_content_surface_scoping.md) — middlenet-first content surface; refactor direction in §3 of this plan
- [`../shell/2026-04-24_blitz_shaped_chrome_scoping.md`](../shell/2026-04-24_blitz_shaped_chrome_scoping.md) — long-horizon Blitz-shaped chrome; this plan partially supersedes its middlenet shape
- [`2026-03-23_webrender_wgpu_backend_implementation_plan.md`](2026-03-23_webrender_wgpu_backend_implementation_plan.md) — Netrender fork plan (long-horizon)
- [`2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md`](2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md) — GUI bridge to refine
- [`ASPECT_RENDER.md`](ASPECT_RENDER.md) — Render aspect authority
- [`../system/graphshell_net_spec.md`](../system/graphshell_net_spec.md) — smolnet content fetching flows through this subsystem
- [`../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md`](../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md) — portable-crate guarantees

---

## 1. Renderer Policy (Canonical)

Two primary renderers, two distinct domains:

### 1.1 Vello — graph canvas surfaces

**Vello** ([Linebender's GPU vector renderer](https://github.com/linebender/vello))
is the canonical renderer for **graph canvas instances**, including:

- the main graph canvas
- the canvas base layer (when Frame is empty)
- canvas Panes
- Navigator swatches (the multi-canvas case from
  [iced_composition_skeleton_spec.md §4.8](../shell/iced_composition_skeleton_spec.md))
- expanded swatch previews
- any future Atlas/Overview or analysis-canvas surface

Vello renders an arbitrary 2D scene composed of paths, gradients, images,
text, and effects — exactly what a graph canvas is. It composes through
the iced `shader` widget, sharing the iced wgpu device.

### 1.2 WebRender — web/document tiles

**WebRender** is the canonical renderer for **web-document tile content**:

- Servo-rendered tiles for full web content
- (Future) WebRender via the Graphshell Netrender fork for
  Linebender-Servo-component-rendered content
- Document-shaped middlenet content where WebRender is the right
  renderer (currently rare — middlenet primarily lives with native
  iced rendering, see §3)

WebRender is **not** a general 2D scene renderer. Per the
2026-04-30 directive, WebRender's role is bounded: it is the workbench
workhorse for documents/pages/sites, not the canvas renderer.

### 1.3 The dependency-deferral rule

Until the WebRender stabilization path (Netrender fork, Servo wgpu
integration, shared-device interop) is reliable, **Graphshell does
not rely on Servo/WebRender for application fundamentals**.

This means:

- The first iced bring-up does **not** depend on shared-wgpu interop
  with Servo working perfectly.
- Per-pane web content uses **Wry** as the primary path during this
  period (each Wry webview renders into a native overlay or a
  separate texture; not shared with the iced wgpu device).
- Upstream Servo is consumed where it works (offscreen Servo
  rendering; Servo subsystem tool pane access; existing landed
  servo-engine feature-gated paths).
- The Netrender fork plan
  ([2026-03-23_webrender_wgpu_backend_implementation_plan.md](2026-03-23_webrender_wgpu_backend_implementation_plan.md))
  remains active but is treated as a long-horizon goal that, when
  it lands, slots in as a tile renderer alongside Wry — not as a
  prerequisite for first bring-up.

Practically: if Netrender stabilizes, web content moves there.
If it doesn't, Wry remains the path. Either way, Vello handles
canvas; WebRender handles documents-when-available.

---

## 2. Application Fundamentals — What We Have to Use

Per the 2026-04-30 directive: get the application fundamentals down
with what we have that isn't Servo/WebRender. Three concrete paths:

### 2.1 Test Wry

Wry is the immediate path for embedded web content. Already integrated
via `crates/iced-wry-viewer` (per the 2026-04-25 extraction). Next
steps:

- Validate Wry as the primary content-surface during S3/S4 bring-up.
- Confirm the wry → graph-node materialization (link-click → CreateNodeAtUrl)
  round-trip per `iced-wry-viewer` demo.
- Test multi-Pane wry hosting (does cross-Pane focus work? do
  multiple webviews coexist without Z-fighting?).
- ~~Explore Wry's text-extraction story for reader-mode (§5).~~
  **HOLD per 2026-05-04 Netrender update** — §5 reader modes are
  on hold until one of the three Netrender paths lands.

### 2.1b Wry validation gap analysis — 2026-05-04

What `crates/iced-wry-viewer` currently has:

| Surface | Coverage |
|---|---|
| `WryHost` API contract (mount/unmount/sync/hide/navigate/last_url) | 8 unit tests in `src/lib.rs` covering happy + sad paths *without* a real window handle (mount-without-handle, sync-without-webview, unmount-without-webview, etc.) |
| Single-WebView interactive demo | `examples/demo.rs` — fixed 800×500 OVERLAY_RECT, hardcoded NODE_ID = 1, two URLs (Home / Gemini-info), Unmount + Re-mount buttons |
| Window-handle bootstrap | `request_window_handle()` task → `apply_window_handle_outcome()` flow, exercised in demo and one unit test |

What's **missing** vs. the §2.1 validation goals:

| Goal | Gap | Suggested deliverable |
|---|---|---|
| **link-click → CreateNodeAtUrl** | Demo navigates the *same* WebView's URL via toolbar buttons; doesn't observe link clicks inside the WebView and doesn't dispatch a graph intent | Add a `wry::WebView` navigation-event handler in `WryManager` that fires when the WebView attempts to navigate; surface it through `WryHost` as a callback; demonstrate "link click → log a `CreateNodeAtUrl` intent payload" in the demo (as a stepping-stone before the real graph integration) |
| **multi-Pane wry hosting** | Demo mounts exactly one WebView; no test for two or more concurrent WebViews | Extend `examples/demo.rs` (or add `examples/multi_pane_demo.rs`) — mount 2 WebViews at side-by-side rects, navigate each independently, observe whether iced layouts + the OS native overlays Z-fight or stay correctly stacked |
| **cross-Pane focus** | No facility for "which pane has keyboard focus?" — wry overlays are native windows that take focus from the OS, not from iced | Investigate: does clicking into a wry overlay steal focus from the iced window? If so, the OS-overlay model conflicts with iced keyboard shortcuts (Ctrl+F, etc.) — that's a finding worth surfacing before S4 bring-up commits to wry as primary |
| **resize / scroll behavior under iced layout changes** | Demo's overlay rect is `const`; never resized or repositioned mid-frame | Extend demo with a window-resize handler that recomputes OVERLAY_RECT and calls `sync_overlay`; visually confirm the WebView tracks layout changes without flicker |

**What I (Claude) can produce in code without hands-on browser testing**:

- The multi-pane demo extension (~120-180 LOC of `examples/multi_pane_demo.rs`). Mechanical to write; doesn't need a running display to compile-check.
- The navigation-event-handler scaffolding in `WryHost` (callback registration API + demo wiring). Compiles without a real browser; the verso `WryManager` either already exposes a navigation-event seam (worth checking) or needs one added.
- A resize-handler extension to `demo.rs`. Pure layout glue.

**What requires hands-on browser testing** (Mark's territory, not Claude's):

- Z-fighting / stacking observations between OS overlays and iced widgets.
- Focus-stealing observations when clicking into a wry overlay.
- Visual flicker during resize.
- WebKitGTK-vs-WebView2-vs-WKWebView differences (the Linux/Win/Mac matrix per §7.1 open items).

**Recommended sequencing**:

1. Write the multi-pane demo + navigation-event scaffolding as a single
   slice (compile-time deliverable; ~250 LOC across two demo files +
   manager API extension).
2. Mark runs the demos on Win/Linux/Mac (where available) and reports
   findings — Z-fighting, focus, flicker.
3. Findings drive S4 decisions: if Z-fighting or focus-stealing is
   blocking, the wry path needs additional iced widget-layer mediation;
   if not, wry is ready to slot in as the primary content-surface for S4.

### 2.2 Refine the GUI bridge

`crates/iced-graph-canvas-viewer` and the wgpu interop architecture
plan
([2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md](2026-04-10_wgpu_gui_bridge_interop_architecture_plan.md))
are the existing bridge between iced and the canvas renderer (Vello).
Refinement:

- Promote `iced-graph-canvas-viewer` from "starting point" to the
  real `CanvasBackend<NodeKey>` impl per
  [iced jump-ship plan §S4](../shell/2026-04-28_iced_jump_ship_plan.md).
- Validate the multi-canvas hosting requirements
  ([§3.2.1 host-neutral necessities](../shell/2026-04-28_iced_jump_ship_plan.md))
  on the Vello path.
- Tighten the wgpu-device sharing (one device for iced + Vello), so
  Navigator swatches and the main canvas share GPU resources cleanly.

### 2.3 Use upstream Servo

Where Servo's offscreen rendering and devtools access already work,
keep using them. The Servo subsystem tool pane (per
[iced_browser_amenities_spec.md §6](../shell/iced_browser_amenities_spec.md))
remains a viable surface even before shared-wgpu interop.

The egui-host-with-Servo build path (`servo-engine + egui-host`)
remains in code under feature gates until the iced path covers
parity for the use cases that need Servo. Egui retirement (per the
landed `314dc093 retire egui and GL compat build roots`) is staged;
Servo via the iced host is the destination, but the path can route
through the egui host during transition.

### 2.4 smolnet emphasis

"smolnet" — lightweight, non-Servo, non-Wry network content rendering
— is the third leg. middlenet (per
[`2026-04-24_iced_content_surface_scoping.md`](../shell/2026-04-24_iced_content_surface_scoping.md))
covers Gemini, RSS, Markdown, plain text, and similar
document-shaped non-HTML content.

middlenet's network needs flow through `graphshell-net` per
[graphshell-net §8](../system/graphshell_net_spec.md); rendering
goes through Vello (per §3 below).

---

## 3. Middlenet Refactor

### 3.1 Current shape

middlenet was scoped as **Blitz-top + webrender-wgpu** per the
existing
[2026-04-24_iced_content_surface_scoping.md](../shell/2026-04-24_iced_content_surface_scoping.md):
Blitz (Linebender's lightweight HTML/CSS engine) handles document
layout; webrender-wgpu (or Netrender) handles rendering.

### 3.2 New shape

Under the 2026-04-30 renderer policy, middlenet refactors toward:

- **Layout**: keep Blitz for document layout — Blitz uses Stylo
  (Servo's CSS engine) and Taffy, both of which compose well with
  iced and don't drag in WebRender. Layout output is a tree of
  positioned, styled boxes plus inline runs.
- **Rendering**: Vello (instead of webrender-wgpu / Netrender) for
  immediate-term rendering of Blitz-laid-out content. Vello handles
  the path/gradient/text/image primitives Blitz needs.
- **Text shaping**: Parley (Linebender) for text shaping +
  metrics. Parley pairs naturally with Vello.
- **Images / decoded content**: usual `image` crate + GPU upload
  through Vello/wgpu.

The full rendering stack becomes pure Linebender + Servo-component
crates: **Stylo (CSS) + Taffy (layout) + Parley (text) + Vello (paint)**.
No WebRender dependency in the middlenet path.

When/if Netrender stabilizes, middlenet's render path can slot
WebRender back in for content where document-renderer optimization
matters; until then, Vello.

### 3.3 Content kinds

middlenet covers what fits in a document model without full web
runtime:

- **Plain text** (UTF-8) — trivial to render.
- **Markdown** (CommonMark + tables, gfm) — pulldown-cmark → Blitz tree.
- **HTML (subset)** — Blitz natively (it's an HTML engine).
- **RSS / Atom** — feed parser → templated HTML → Blitz.
- **Gemini** — gemtext parser → simple HTML → Blitz.
- **PDF** (future) — separate; not middlenet.

For each content kind, middlenet:

1. Fetches via `graphshell-net` (a `ProviderRequest` or direct
   `DownloadRequest`-shape).
2. Parses to a normalized HTML(-like) tree.
3. Lays out via Blitz/Stylo/Taffy.
4. Shapes text via Parley.
5. Paints to a Vello scene composed into the iced `shader` widget.

### 3.4 Refactor staging

Out of scope for this plan to enumerate refactor commits, but the
direction:

1. Replace middlenet's webrender-wgpu paint with a Vello paint step
   (~Stage A of refactor).
2. Replace any WebRender-specific layout assumptions with Blitz
   directly (most of middlenet's tree probably already passes through
   Blitz; verify and tighten).
3. Wire `graphshell-net` ProviderRequest / DownloadRequest as the
   only network entry (middlenet does not have its own HTTP).
4. Validate against existing middlenet tests + add Vello-paint tests.

The refactor superseded the Blitz-shaped chrome scoping
([2026-04-24_blitz_shaped_chrome_scoping.md](../shell/2026-04-24_blitz_shaped_chrome_scoping.md))
on the **content** side; the chrome side (Blitz for shell chrome) is
a separate question still tracked by that scoping doc.

---

## 4. EmbeddedHost Audit

Per the 2026-04-30 directive: lean against keeping `EmbeddedHost`
as a render mode by default; audit for bits worth graverobbing.

### 4.1 What `EmbeddedHost` currently is

`EmbeddedHost` is a `ViewerRenderMode` / `TileRenderMode` value
(renamed from `EmbeddedEgui` in receipt
[§8.1 of the iced jump-ship plan](../shell/2026-04-28_iced_jump_ship_plan.md))
designating "viewers that draw through the host's UI primitives
rather than to a backend texture or via a native overlay." It was
the canonical home for non-web tool/diagnostic viewers in the egui
era.

### 4.2 Why audit-and-retire

Under the new renderer policy:

- **Canvas content** has a clear renderer (Vello).
- **Web/document content** has a clear renderer (WebRender via Servo
  or Wry; middlenet via Vello).
- **Tool/diagnostic panes** historically used `EmbeddedHost` because
  egui rendered them inline; in iced these are just **plain iced
  widgets** (`column!`, `text`, `scrollable`, etc.) with no special
  render mode.

The `EmbeddedHost` mode therefore loses its distinct purpose — every
"embedded host" surface in iced is just a normal widget tree. Keeping
the mode as a separate render category adds bookkeeping without
buying anything.

### 4.3 Audit graverob list

What may be worth keeping when the mode is retired:

- **`TileRenderMode` enum's invariant statements** (e.g., "exactly
  one of CompositedTexture / NativeOverlay / EmbeddedHost / Placeholder
  for a tile"). The remaining variants (`CompositedTexture`,
  `NativeOverlay`, `Placeholder`) cover the iced cases; `Placeholder`
  takes over for "uninitialized / loading" states.
- **`CompositorAdapter` callback ordering rules** (Surface Composition
  Pass in TERMINOLOGY.md). These are renderer-independent and apply
  to canvas Panes / tile Panes regardless of mode.
- **The diagnostics channel events** (`embedded_host` /
  `CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_HOST`). Drop the
  embedded-host-specific events; keep the generic mode-transition
  events.
- **Serde aliases** for the legacy `EmbeddedEgui` payload — keep
  during migration, retire when egui code retires.

### 4.4 Replacement

For tool/diagnostic panes, the iced realization is:

- A normal iced widget tree as the Pane body.
- `TileRenderMode::Placeholder` while loading.
- No need for an `EmbeddedHost` mode — the pane simply renders
  through iced's standard widget pipeline.

For non-web non-canvas viewers (e.g., a future PDF viewer, an audio
viewer, a video viewer), each gets its own `TileRenderMode` if it
needs special compositor handling; otherwise they fit under
`CompositedTexture` (their content is GPU-textured) or just a
custom iced widget.

### 4.5 Action items

1. ~~Audit `crates/graphshell-runtime/src/content_surface.rs` and
   `crates/graphshell-runtime/src/ports.rs` for `EmbeddedHost`
   references.~~ **2026-05-04 refresh**: those two files have zero
   `EmbeddedHost`/`EmbeddedEgui` references today. The actual current
   reference locations (10 files, audited 2026-05-04) are:

   | File | Likely role |
   |---|---|
   | `crates/registrar/register-viewer/src/lib.rs` | viewer-registry enum / descriptor |
   | `crates/graphshell-core/src/pane.rs` | portable pane types referencing render mode |
   | `shell/desktop/runtime/diagnostics.rs` | diagnostics channel events |
   | `shell/desktop/workbench/compositor_adapter.rs` | compositor-side mode dispatch |
   | `shell/desktop/workbench/tile_compositor.rs` | tile compositor body |
   | `shell/desktop/workbench/tile_render_pass.rs` | tile render pass selection |
   | `shell/desktop/workbench/tile_runtime.rs` | tile runtime state |
   | `shell/desktop/workbench/ux_tree.rs` | workbench tree presentation |
   | `shell/desktop/ui/overview_plane.rs` | overview-plane surface |
   | `registries/domain/layout/viewer_surface.rs` | viewer-surface profile registry |

   Categorize each as keep / graverob / retire (results below in §4.5b).
2. Update viewer specs
   ([viewer_presentation_and_fallback_spec.md](../viewer/viewer_presentation_and_fallback_spec.md),
   [universal_content_model_spec.md](../viewer/universal_content_model_spec.md),
   [node_lifecycle_and_runtime_reconcile_spec.md](../viewer/node_lifecycle_and_runtime_reconcile_spec.md))
   to remove `EmbeddedHost` from the canonical mode set; describe
   tool/diagnostic panes as plain iced widget trees.
3. Retire the `EmbeddedHost` enum variant in code once the iced
   bring-up reaches a point where no callers use it (likely S6 of
   the iced jump-ship plan).
4. Preserve serde aliases for one release cycle for any persisted
   `EmbeddedEgui` / `EmbeddedHost` payloads.

This is an audit + retirement, not a hard cut; egui-host code keeps
the mode while the egui path is alive.

### 4.5b Categorized audit results — 2026-05-04

Each of the 10 reference locations classified per §4.3
(keep / graverob / retire) plus a sequencing call. The §1 Netrender
update affects only the viewer-descriptor entries (categories marked
**HOLD**); everything else is sequenceable now.

#### Tier KEEP — canonical type/policy, dies with the variant

| Location | What's there | Why keep |
|---|---|---|
| `crates/graphshell-core/src/pane.rs` | `TileRenderMode` enum + `#[serde(alias = "EmbeddedEgui")]` + doc comment | The canonical render-mode enum. §4.3 explicitly calls out "TileRenderMode enum's invariant statements" as keep. Serde alias preserved per §4.5 action #4. |
| `crates/registrar/register-viewer/src/lib.rs` (lines 37-42, 626-631) | `ViewerRenderMode` enum + serde alias + legacy-payload deserialization test | Mirror of the above for viewer-side. Keep through migration. |
| `shell/desktop/workbench/tile_compositor.rs` (lines 351-352, 1419-1420, 2488-2500) | sort order (`EmbeddedHost => 2`), hover-overlay policy (`EmbeddedHost \| Placeholder => AreaStroke`), tests | Generic mode-policy dispatch. The `EmbeddedHost` arm disappears when the variant retires; the policy structure stays. |
| `shell/desktop/workbench/tile_render_pass.rs` (line 1179) | `EmbeddedHost \| Placeholder => { ... }` fallback arm in render pass selection | The "host UI handles it, no compositor pass needed" fallback. Branch arm dies with the variant; surrounding logic is policy. |
| `shell/desktop/workbench/tile_runtime.rs` (line 35, 164-165, plus 5 test fixtures) | `ViewerRenderMode::EmbeddedHost → TileRenderMode::EmbeddedHost` translation, "embedded-host" string formatter for diagnostics, plaintext-viewer default mode | The translation bridge between the two enums. Translation arm + tests die with the variants. |
| `shell/desktop/ui/overview_plane.rs` (line 895, plus 2 test fixtures) | `EmbeddedHost => "embedded host"` debug-info label, fixture data | Trivial label formatter; dies with the variant. |

#### Tier GRAVEROB — keep generic policy, drop mode-specific event/mapping

| Location | What's there | Action |
|---|---|---|
| `shell/desktop/runtime/diagnostics.rs` (lines 32, 815-817, 1195-1198, 1598-1601, 2476-2478) | `CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_HOST` import + counting + event emission + name mapping | Per §4.3: "Drop the embedded-host-specific events; keep the generic mode-transition events." When the variant retires, drop the channel constant from `register-diagnostics::channels` and remove these references. The generic `CHANNEL_COMPOSITOR_OVERLAY_*` family stays. |
| `shell/desktop/workbench/compositor_adapter.rs` (lines 48, 633, 1455-1460) | channel-import + dispatch arm `EmbeddedHost => CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_HOST` + invariant test | Same fate as the channel constant. The dispatch table is generic policy; the embedded-host arm dies with the channel and the variant. |

#### Tier RETIRE — biggest leverage; replacement is plain iced widgets

| Location | What's there | Action |
|---|---|---|
| `shell/desktop/workbench/ux_tree.rs` (**17 occurrences** across lines 747, 786, 827, 865, 999, 1079, 1133, 1190, 1228, 1266, 1306, 1353, 1445, 1492, 1554, 1754, plus formatter at 1839) | 17 z-pass spec entries declaring `render_mode: Some(TileRenderMode::EmbeddedHost)` for `command.bar`, `command.omnibar`, `command.palette`, `command.context_palette`, `command.radial` (+ `tier1`/`tier2`/`summary`/`sector`), `workbench.graph.lens_scope`, `workbench.navigator.projection`, `workbench.route_open.boundary`, `graph.layer.status`, `workbench.content`, `graph.layer.node`, `workbench.tool`, plus a formatter mapping | **Single biggest concentration of EmbeddedHost in the codebase.** Per §4.4: "in iced these are just plain iced widgets (column!, text, scrollable, etc.) with no special render mode." Each of the 17 surfaces becomes a custom iced widget tree; the `render_mode: Some(TileRenderMode::EmbeddedHost)` field becomes `None` or is removed entirely. **Sequencing**: lands alongside iced jump-ship S6 (egui retirement); the surfaces re-materialize as iced widgets one slice at a time. |

#### Tier RETIRE-VIEWER — HOLD pending the Netrender outcome

| Location | What's there | Action |
|---|---|---|
| `crates/registrar/register-viewer/src/lib.rs` (lines 429-432) | 7 viewer-id mappings → `EmbeddedHost`: `viewer:middlenet`, `viewer:plaintext`, `viewer:markdown`, `viewer:pdf`, `viewer:csv`, `viewer:settings`, `viewer:metadata`, `viewer:audio` | Each of these viewers eventually targets a non-host renderer (middlenet → Stylo/Vello/WebRender; pdf → separate PDF renderer; csv/audio → custom iced widget; settings/metadata → plain iced). **HOLD**: per the 2026-05-04 Netrender update, the middlenet target (and therefore the `viewer:middlenet`/`viewer:markdown` mappings) is unsettled. Revisit when one Netrender path lands. |
| `registries/domain/layout/viewer_surface.rs` (lines 117, 176, 193) | mapping `EmbeddedHost → VIEWER_SURFACE_EMBEDDED`, plus markdown and gemini viewer descriptors with `render_mode: ViewerRenderMode::EmbeddedHost` | Same HOLD rationale: markdown and gemini are middlenet-bound; the descriptor shape changes when middlenet lands its renderer. The mapping function stays regardless. |

#### 4.5b sequencing summary

- **Now**: §4.5 action items #1 ✅ done (audit complete; this section), #2 (update viewer specs to remove EmbeddedHost from canonical mode set) is doable as docs-only and doesn't depend on Netrender.
- **Alongside iced jump-ship S6**: retire the 17 ux_tree.rs surfaces one by one as iced widgets land. Drop the diagnostics channel + compositor_adapter dispatch arm in the same sweep.
- **HOLD until Netrender outcome**: the 9 viewer-descriptor mappings (7 in register-viewer + 2 in viewer_surface). These are §3 middlenet territory.
- **Final**: drop the `EmbeddedHost` enum variant + serde alias from both `TileRenderMode` and `ViewerRenderMode` once all callers migrate. One release cycle of serde-alias preservation per §4.5 #4.

---

## 5. Reader Modes / View Modes — Tailored for Middlenet

Per the 2026-04-30 directive (#8): reader modes tailored for
middlenet. Per the
[settings + permissions spine](../aspect_control/settings_and_permissions_spine_spec.md)
view/tile scope.

### 5.1 What "reader mode" means here

Two related but distinct view modes:

- **Native rendering**: the content as the viewer renders it natively
  — Servo's full rendering for HTML, middlenet's Blitz/Vello stack
  for non-HTML, Wry's webview for the wry path.
- **Reader rendering**: a content-extracted, simplified view where
  middlenet's renderer takes over. The Servo-rendered HTML page's
  main content is extracted (heuristic), reflowed through Blitz, and
  painted by Vello. Same stack as middlenet's native content
  rendering.

Reader mode for non-HTML middlenet content (Markdown, RSS, Gemini,
plain text) is **the default**: those content kinds are already
"reader" by nature. The mode toggle is meaningful primarily for
Servo-rendered web pages.

### 5.2 Surface

A per-tile toggle in the tile chrome (icon button, also in the
context menu):

- `View: Native` — viewer's native rendering
- `View: Reader` — middlenet-rendered content extraction

State is **tile-scope** per the settings spine (§3.1 canonical scope:
"Reader mode toggle"); a user can default to reader for one graph
or one persona.

### 5.3 Content extraction for Servo pages

When a user toggles a Servo-rendered page to reader mode:

1. Servo continues to load/run the page in the background (so
   navigation state is preserved).
2. Graphshell asks Servo for the rendered DOM via the
   devtools/observer protocol (or scrapes the rendered HTML).
3. The DOM is fed through a Readability-style extractor (the
   open-source Readability port for Rust, or `readable-readability`).
4. The extracted main content is converted to clean HTML.
5. Blitz/Stylo/Taffy lay out the cleaned HTML.
6. Parley shapes the text.
7. Vello paints. (Same middlenet path as §3.)

Toggle back to native: Vello scene is destroyed; Servo's native
output reappears (the underlying view never stopped). No re-fetch
required.

### 5.4 Reader-mode-specific styling

Reader mode applies a dedicated `font_family_content` (per
[theme tokens spec §2.2](theme_and_tokens_spec.md)) and a wider
density profile. Per-persona reader-mode style overrides
("Pinkish background, serif font, 18pt") live in persona settings.

### 5.5 Interaction

In reader mode:

- Links remain clickable; activation routes through the same
  `OpenAddress` intent as the omnibar.
- Selection works (text selection for clip / quote / agent
  reference).
- Find-in-page (Ctrl+F) operates on the extracted content (a
  `ViewerIntent::FindInPage` variant for reader-mode targets).
- The reader-mode renderer does not run JavaScript; this is part
  of what makes it "reader."

### 5.6 Out of scope for this plan

- The full extraction-quality story (Readability heuristics,
  fallback when extraction fails, ML-augmented extraction). Tracked
  as middlenet content-extraction follow-up.
- PDF reader (separate viewer, not middlenet).
- Print/export from reader mode (separate spec — see
  [graphshell-net §9](../system/graphshell_net_spec.md) print
  pipeline open item).

---

## 6. Decomposition Target Update

Per the 2026-04-30 directive (#12): lower the threshold from 600 LOC
to **500 LOC** and decompose every portable-crate file exceeding it.

### 6.1 New target

> **No portable-crate Rust file exceeds 500 LOC** (was 600 in the
> prior iced jump-ship plan §3.2). When a file approaches the
> threshold, decompose by responsibility before adding more code.

### 6.2 Files to decompose

#### 6.2a Refreshed inventory — 2026-05-04 (canonical)

Fresh sweep of every Rust file under `crates/` (excluding `target/` and
`vendor/`). 43 files exceed the 500-LOC threshold; 4 exceed 2,000 LOC.
**The "constraint on new code" rule is not being enforced** — every file
on the original 2026-04-28 list has GROWN by 40-520 LOC, and three files
over threshold landed in the past four days (signal_bus.rs from the
2026-05-04 Slice B, edge_style.rs and theme.rs from Slice C). The
back-fill stream (§6.5) and the new-file constraint both need active
enforcement.

**Severe (>2000 LOC):**

| File | Lines | Δ vs. 2026-04-28 |
|---|---:|---:|
| `crates/graphshell-core/src/graph/mod.rs` | 5,459 | +522 |
| `crates/graph-cartography/src/lib.rs` | 3,958 | +335 |
| `crates/registrar/register-diagnostics/src/descriptor.rs` | 2,334 | (new — Slice 53) |
| `crates/graph-tree/src/tree.rs` | 2,010 | +244 |

**Heavy (1000-2000 LOC):**

| File | Lines | Δ vs. 2026-04-28 |
|---|---:|---:|
| `crates/registrar/register-mod-loader/src/loader.rs` | 1,600 | (new — Slice 68c) |
| `crates/graph-memory/src/lib.rs` | 1,527 | +155 |
| `crates/graphshell-comms/src/identity.rs` | 1,501 | (not on prior list) |
| `crates/graph-canvas/src/derive.rs` | 1,494 | +103 |
| `crates/registrar/register-input/src/lib.rs` | 1,477 | (new — Slice 54) |
| `crates/graphshell-comms/src/misfin.rs` | 1,272 | (not on prior list) |
| `crates/graph-canvas/src/engine.rs` | 1,255 | +46 |
| `crates/graphshell-comms/src/transport.rs` | 1,181 | (not on prior list) |

**Moderate (700-1000 LOC):**

| File | Lines | Δ vs. 2026-04-28 |
|---|---:|---:|
| `crates/middlenet-core/src/document.rs` | 935 | +48 |
| `crates/graph-canvas/src/layout/rapier_adapter.rs` | 930 | +68 |
| `crates/graphshell-core/src/ux_probes.rs` | 902 | (not on prior list) |
| `crates/graph-tree/src/topology.rs` | 892 | +107 |
| `crates/graph-canvas/src/layout/extras.rs` | 888 | +84 |
| `crates/graph-canvas/src/layout/registry.rs` | 869 | +76 |
| `crates/graph-canvas/src/layout/static_layouts.rs` | 867 | +66 |
| `crates/graphshell-runtime/src/system/signal_bus.rs` | 859 | (new — Slice 51, this session) |
| `crates/registrar/register-viewer/src/lib.rs` | 852 | (new — Slice 56) |
| `crates/graphshell-runtime/src/frame_projection.rs` | 783 | +75 |
| `crates/graphshell-core/src/graph/filter.rs` | 783 | +74 |
| `crates/graphshell-core/src/shell_state/frame_model.rs` | 757 | +84 |
| `crates/verso/src/lib.rs` | 750 | (not on prior list) |
| `crates/graphshell-core/src/actions.rs` | 741 | +30 |
| `crates/graph-canvas/src/scene_physics.rs` | 717 | +65 |

**Newly in scope under the lowered threshold (500-700 LOC):**

| File | Lines | Note |
|---|---:|---|
| `crates/registrar/register-theme/src/edge_style.rs` | 666 | just landed Slice C, this session |
| `crates/middlenet-engine/src/adapters.rs` | 665 | |
| `crates/registrar/register-lens/src/physics.rs` | 641 | |
| `crates/iced-graph-canvas-viewer/src/lib.rs` | 621 | |
| `crates/registrar/register-theme/src/theme.rs` | 618 | just landed Slice C, this session |
| `crates/graphshell-core/src/persistence.rs` | 611 | |
| `crates/graph-canvas/src/layout/penrose.rs` | 595 | |
| `crates/middlenet-render/src/lib.rs` | 593 | |
| `crates/graph-canvas/src/simulate.rs` | 588 | |
| `crates/graph-canvas/src/layout/semantic_embedding.rs` | 588 | |
| `crates/graphshell-core/src/verso_address.rs` | 564 | |
| `crates/graph-canvas/src/layout/barnes_hut.rs` | 563 | |
| `crates/graphshell-core/src/shell_state/authorities.rs` | 524 | |
| `crates/graph-tree/src/reconciliation.rs` | 516 | |
| `crates/graph-canvas/src/layout/force_directed.rs` | 510 | |
| `crates/middlenet-engine/src/document.rs` | 504 | |

#### 6.2b Original inventory (2026-04-28) — preserved for historical comparison

Per the 2026-04-28 inventory in
[iced jump-ship plan §3.2](../shell/2026-04-28_iced_jump_ship_plan.md),
files over 600 LOC at that snapshot:

| File | Lines | Decomposition target |
|---|---:|---|
| `crates/graphshell-core/src/graph/mod.rs` | 4,937 | identity, node, edge, graphlet, lifecycle, mutation, query, selection |
| `crates/graph-cartography/src/lib.rs` | 3,623 | projection, mapping, layout_export, view_model, registry, error |
| `crates/graph-tree/src/tree.rs` | 1,766 | tree storage, mutation commands, traversal, focus/activation, layout I/O |
| `crates/graph-canvas/src/derive.rs` | 1,391 | node projection, edge projection, selection enrichment, style, diagnostics |
| `crates/graph-memory/src/lib.rs` | 1,372 | memory model, indexing, recall, persistence, scoring |
| `crates/graph-canvas/src/engine.rs` | 1,209 | engine state, tick loop, input commands, camera, backend handoff |
| `crates/middlenet-core/src/document.rs` | 887 | document model, block tree, text ranges, annotations, serialization |
| `crates/graph-canvas/src/layout/rapier_adapter.rs` | 862 | bodies/colliders, force application, constraints, result extraction |
| `crates/graph-canvas/src/layout/extras.rs` | 804 | clustering, pinning, viewport constraints, debug aids |
| `crates/graph-canvas/src/layout/static_layouts.rs` | 801 | per-static-layout-family modules + facade |
| `crates/graph-canvas/src/layout/registry.rs` | 793 | layout descriptors, profile registry, factory, validation |
| `crates/graph-tree/src/topology.rs` | 785 | model, adjacency, paths, invariants |
| `crates/graphshell-core/src/actions.rs` | 711 | identifiers, descriptors, dispatch metadata, serialization |
| `crates/graphshell-core/src/graph/filter.rs` | 709 | AST/types, parser, evaluator, text matching, diagnostics |
| `crates/graphshell-runtime/src/frame_projection.rs` | 708 | input collection, frame/view projection, overlay projection, command-surface projection |
| `crates/graphshell-core/src/shell_state/frame_model.rs` | 673 | frame identity, tree/model, lifecycle commands, persistence shape |
| `crates/graph-canvas/src/scene_physics.rs` | 652 | force model, integration, constraints, scene-runtime adapters |
| `crates/middlenet-adapters/src/lib.rs` | 649 | adapter traits, iced adapter, wry/host adapters, test fixtures |

Plus any portable-crate files between **500 and 600 LOC** at the
same inventory point, which were not previously called out — those
now also need decomposition under the new threshold. A fresh
inventory pass is the first sub-step.

### 6.3 Decomposition rules (carried forward)

Per
[iced jump-ship plan §3.2](../shell/2026-04-28_iced_jump_ship_plan.md):

- Do not introduce new files over 500 LOC in portable crates.
- When a slice touches an oversized file, either decompose first or
  extract the touched responsibility in the same change.
- Preserve public crate APIs during the first split (re-export from
  the old module path); rename external APIs only in a focused
  follow-up.
- Keep extraction mechanical before changing behavior: move code,
  re-export, run narrow tests/checks, then make semantic changes.
- Prefer domain names (graphlet, lifecycle, projection) over
  implementation names (helpers, utils).

### 6.4 Action items

1. Update [iced jump-ship plan §3.2](../shell/2026-04-28_iced_jump_ship_plan.md)
   "no new files over 600 LOC" rule to read 500 LOC.
2. Take a fresh inventory across all `crates/` portable crates and
   list 500-600 LOC files alongside the existing >600 LOC list.
3. Decomposition lands as a separate work stream from the iced
   bring-up (§6.5).

### 6.5 Sequencing

The decomposition target is **decoupled from the iced bring-up**:
S3/S4 work proceeds against the current code shape; decomposition
happens as its own slice work stream so it doesn't block surface
implementation. The **constraint on new code** (no new files over
500 LOC in portable crates) applies immediately; the **back-fill
decomposition** of existing oversized files can land progressively.

---

## 7. Open Items and Sequencing

### 7.1 Open items

- **Netrender fork status**: the long-horizon Netrender fork
  ([2026-03-23_webrender_wgpu_backend_implementation_plan.md](2026-03-23_webrender_wgpu_backend_implementation_plan.md))
  remains active; this plan does not redirect that work, only
  decouples the iced bring-up from its completion.
- **Vello-via-shader-widget device sharing**: how cleanly does the
  iced wgpu device share with Vello scene buffers across N canvas
  instances (per the multi-canvas hosting requirements)? Validation
  needed in the Stage C (canvas Program) iced bring-up.
- **Reader-mode extraction quality**: Readability ports vary; needs
  evaluation in middlenet-side tests.
- **Wry on Linux**: WebKitGTK is the Wry backing on Linux; behavior
  differs from WebView2/WKWebView. Multi-Wry-pane testing is needed.
- **Servo via egui-host transition path**: how long does the egui
  host stay alive specifically for Servo features that haven't
  ported to iced? Tracked under the iced jump-ship plan's S6 timing.
- **PDF / multimedia viewers**: not in this plan; their renderer
  choice (likely separate from Vello/WebRender split) is a future
  spec.

### 7.2 Sequencing summary

1. **Immediate**: §6.4 update iced jump-ship plan §3.2 threshold to
   500 LOC; the constraint on new files applies immediately.
2. **iced bring-up (S3/S4 in parallel)**: §2.1 Wry validation, §2.2
   GUI bridge refinement (Vello as canvas backend), §2.3 upstream
   Servo where it works.
3. **Stage A done condition uses Vello**: per the iced jump-ship
   plan §12.3, Stage A is the iced Application + Subscription
   closure. Vello as the canvas renderer is part of that closure.
4. **Middlenet refactor (§3.4)**: parallel to iced bring-up; can
   start as soon as the iced canvas backend is solid.
5. **EmbeddedHost retirement (§4.5)**: incremental audit + retire
   alongside egui retirement (S6 timing).
6. **Reader modes (§5)**: depends on middlenet refactor reaching a
   stable point; not a first-bring-up requirement.
7. **Decomposition back-fill (§6.5)**: continuous; lands as its own
   slice stream alongside everything else.

---

## 8. Bottom Line

Vello renders graph canvases (one renderer for all canvas-shaped
surfaces); WebRender renders web/document tiles when available;
neither path depends on the other. Until Servo/WebRender stabilizes
through the Netrender fork, application fundamentals use **Wry +
upstream Servo + smolnet (middlenet)** for content. Middlenet
refactors from Blitz-top + webrender-wgpu to Stylo + Taffy + Parley +
Vello — all Linebender + Servo-component crates, no WebRender
dependency. EmbeddedHost as a render mode is audited for retirement
in favor of plain iced widget trees for tool/diagnostic panes. Reader
modes use the middlenet stack to render extracted Servo-page content.
Decomposition target lowers to 500 LOC for portable-crate files;
constraint on new files is immediate, back-fill is continuous.

This plan is a **direction** — the surface specs (composition
skeleton, canvas instances, middlenet) reference this for the
renderer choices; the iced jump-ship plan §3.2 picks up the
500-LOC threshold; the EmbeddedHost retirement folds into the
existing egui-retirement schedule.
