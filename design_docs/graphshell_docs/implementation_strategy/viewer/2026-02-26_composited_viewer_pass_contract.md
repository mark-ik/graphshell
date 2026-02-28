# Composited Viewer Pass Contract (Servo Callback Path)

**Date**: 2026-02-26
**Status**: Architectural gap-closure note (implementation-guiding)
**Scope**: Workbench/graph viewer composition semantics for composited web content (`viewer:webview`) and contrast with native-overlay backends (`viewer:wry`)
**Purpose**: Define explicit pass ownership and backend render-mode behavior so UI affordances are not dependent on implicit egui layer ordering or servoshell-era callback assumptions.

## Why This Exists

A current stabilization bug exposed an architectural gap:

- focus/hover affordances can render over tile chrome,
- but fail to render over webpage pixels,
- even when the page is in the composited Servo path (not a native OS overlay).

This indicates a compositor callback ordering and/or GL state isolation problem, not just an egui z-order problem.

Historically, Graphshell inherited a servoshell-style GL callback composition path (`render_to_parent`) without a documented pass contract. This note makes that contract explicit.

## Core Principle

Do not treat composited web content as "just another egui layer."

Treat web content composition as a **backend render pass** with explicit ownership, ordering, and state-isolation rules.

> **Opportunity — Shared Compositor Protocol to Verso.** The principle above — explicit pass ownership, not implicit layer inheritance — can be extracted as a *formal protocol contract* between Graphshell and Verso (and eventually any upstream Servo embedder). If the pass contract, state isolation adapter API, and overlay affordance hooks are defined as a trait boundary rather than implementation-internal code, Graphshell's fork-migration work becomes a *contribution* to Verso's embedder story. This turns servoshell debt repayment into an upstream value proposition. See §Appendix A.4.

## Render Modes (Runtime Policy)

Graphshell viewer backends must map to a render mode with explicit affordance policy.

| Render mode | Example backend | Pixel ownership | Over-content Graphshell overlays? | Primary usage |
| --- | --- | --- | --- | --- |
| `composited` | `viewer:webview` | Graphshell-owned callback/texture composition | Yes (if pass contract is honored) | Graph canvas, workbench tiles |
| `native-overlay` | `viewer:wry` | OS/native view hierarchy | No | Stable pane/tile regions, detached windows |
| `placeholder` / `thumbnail` | degraded fallback | Graphshell-owned image/UI | Yes | Graph-view fallback, unsupported/degraded states |

This note defines the contract for `composited` mode.

> **Opportunity — Multi-Backend Hot-Swap Per-Tile.** Because `TileRenderMode` is resolved at viewer attachment time (not baked into the tile tree), nothing prevents *runtime mode transitions*: swap a tile from `viewer:webview` to `viewer:wry` (or back) while preserving navigation state, session cookies, and scroll position. This turns the multi-backend migration from a compatibility burden into a user-facing power feature ("try this page in Wry"). See §Appendix A.2.

## Composited Viewer Pass Contract

### Passes (conceptual order)

For a frame that includes composited web content, the architecture must distinguish:

1. **UI Layout Pass**
- Computes tile rects, active tabs, visibility, occlusion, and focus ownership.
- Produces geometry and visibility data only.
- No backend rendering side effects should be required to complete layout.

2. **Composited Content Pass**
- Invokes backend callbacks (e.g. Servo `render_to_parent`) for visible composited viewer regions.
- Responsible for rendering web pixels into the host app render pipeline.
- Must be wrapped by a state-isolation adapter (see below).

> **Opportunity — Differential Frame Composition.** Because each tile's content pass is independently invocable and state-isolated, the compositor can *skip re-compositing unchanged tiles*. If a tile's backing webview reports no dirty rect since the last frame, reuse the previous composited texture. This turns GL state isolation overhead into a net performance win for static/document-heavy workloads. See §Appendix A.8.

3. **Composited Overlay Affordance Pass**
- Draws focus/hover/selection/rearrange affordances that must appear over composited web pixels.
- Uses the same tile rects/focus data as the content pass.
- Must execute after composited content pass for the same frame.

> **Opportunity — Content-Aware Overlay Affordances.** Because the overlay pass executes *after* web content is composited into a Graphshell-owned texture, the host can sample composited pixel data (edge luminance, dominant color) to adapt affordance colors and opacity. A focus ring over a white page renders differently than over a dark-mode page — automatically, without user configuration. See §Appendix A.5.

> **Opportunity — Mod-Hosted Overlay Passes.** The pass contract creates a natural extension point: allow mods to register *additional* overlay passes between Content and UI Chrome. Examples: AI annotation layers, accessibility contrast overlays, content-analysis heatmaps, privacy redaction screens. The pass model becomes a plug-in rendering pipeline. See §Appendix A.10.

4. **Non-content UI Overlays / Popups**
- Tooltips, menus, dialogs, debug overlays, etc.
- May be implemented through egui ordering, but should not be the mechanism relied upon for composited-web affordance correctness.

### Architectural requirement

The correctness of focus/hover affordances over composited web content must depend on the **Composited Overlay Affordance Pass**, not incidental egui layer ordering.

## Callback State Isolation Contract (Composited Backends)

Backend callbacks that render web content into the app surface (e.g. Servo `render_to_parent`) are treated as untrusted with respect to host renderer state.

### Required adapter behavior

A compositor adapter/wrapper around the callback must:

- establish callback clip/viewport inputs explicitly,
- invoke backend callback,
- restore or scrub host renderer state before subsequent host overlays render.

At minimum, the host must assume backend callbacks may affect:

- scissor state
- viewport
- blend state
- depth/stencil state
- culling state
- bound framebuffer/attachments (where applicable)

Implementation note:
- Exact save/restore mechanics are backend-specific (`egui_glow`, future WGPU path, etc.).
- The contract is architectural; concrete implementation may use "full restore" or "known-good scrub."

> **Opportunity — Composited Content Replay / Time-Travel Debugging.** Because the adapter already captures and restores GL state snapshots around every backend callback, extending it to *record the snapshot sequence* per frame is a small delta. This creates a time-travel composition debugger: replay the exact GL state transitions that produced a given frame, identify which callback corrupted state, and compare frames across sessions. Turn the "untrusted callback" defense into a forensic superpower. See §Appendix A.1.

> **Opportunity — Compositor Pass Chaos Engineering.** The state-isolation contract can be *actively verified* by a diagnostics mode that deliberately injects GL state corruption between passes (randomized viewport, scissor, blend mutations). If the compositor self-heals correctly, the isolation contract is proven under adversarial conditions — not just trusted-callback conditions. This turns testing overhead into a continuous confidence signal. See §Appendix A.3.

## Affordance Policy by Render Mode

### `composited`

- Focus/hover rings may render over web pixels.
- Prefer compositor overlay pass for tile-level affordances.
- Diagnostics should confirm callback registration and overlay pass execution.

> **Opportunity — Cross-Tile Compositor Transitions.** Because composited content is texture-owned, Graphshell can animate tile split/merge/rearrange transitions with *live web content visible* during the animation — not frozen thumbnails. Tiles slide, scale, and cross-fade showing real rendered pages. This turns texture ownership (a technical implementation detail) into theatrical, spatial-browser UX that no Chromium-based browser can replicate. See §Appendix A.6.

### `native-overlay`

- Graphshell cannot render over live web pixels.
- Use tile chrome/gutter/frame affordances instead.
- When dialogs/menus must appear visually above the native webview, hide/suspend the overlay-backed viewer (`sync_overlay(..., false)`), or move the affordance to native chrome.

### `placeholder` / `thumbnail`

- Standard Graphshell overlays are allowed.
- Treat as ordinary UI/image rendering.

## What This Replaces (Servoshell-Era Assumptions)

This contract replaces the following implicit assumptions inherited from the servoshell-style composition path:

1. **Assumption**: "If it is in egui, `Order::Foreground` is enough."
- False for composited callback paths when ordering/state isolation is unspecified.

2. **Assumption**: backend callback execution is "just paint" and preserves host render state
- Not safe to assume.

3. **Assumption**: overlay behavior can be specified without backend render mode
- False once multiple viewer backends (`servo`, `wry`, fallback viewers) are present.

## Integration Points (Current Code)

Primary hotspots:

- `shell/desktop/workbench/tile_compositor.rs`
- `shell/desktop/workbench/tile_render_pass.rs`

Related architectural/roadmap docs:

- `2026-02-23_wry_integration_strategy.md`
- `../aspect_render/2026-02-20_embedder_decomposition_plan.md`
- `PLANNING_REGISTER.md` (`lane:stabilization`, `lane:embedder-debt`, `lane:viewer-platform`, `lane:spec-code-parity`)

## Diagnostics / Validation Expectations

The diagnostics subsystem should be able to prove, for affected tiles:

- render mode (`composited` / `native-overlay` / fallback)
- callback registration/execution (for composited)
- overlay affordance pass execution for the same frame
- degraded/fallback path activation reason when applicable

Current `render_path_hint` diagnostics are a useful transitional step, but the long-term goal is runtime-authoritative render mode metadata rather than inferred hints.

> **Opportunity — Per-Tile GPU Memory Budget with Graceful Degradation.** The pass model and `TileRenderMode` enable per-tile GPU memory accounting. When total GPU memory pressure exceeds a threshold, automatically degrade tiles from `CompositedTexture` to `Placeholder`/`Thumbnail` — rendering a visual tombstone with a "tap to reactivate" affordance. The transition is reversible, observable via diagnostics channels, and aligned with the lifecycle model (`Active → Cold`). This turns resource exhaustion from a crash risk into a graceful, visible, user-controllable degradation. See §Appendix A.7.

## Non-Goals

- Full Wry integration design (covered by `2026-02-23_wry_integration_strategy.md`)
- Rewriting all workbench rendering
- Mandating one graphics backend implementation strategy

## Next Implementation Slices (Issue-Seeding)

1. Compositor callback adapter wrapper with GL state isolation/scrub
2. Explicit composited overlay affordance pass in `tile_compositor.rs`
3. Diagnostics fields for pass execution/order confirmation
4. Regression coverage: focus/hover affordance visible over composited web pixels during tile rearrange

> **Opportunity — Viewer Backend Telemetry Races.** For sites with both `viewer:webview` and `viewer:wry` capable, maintain *both* backends simultaneously (one visible, one shadow-rendering). Compare load times, fidelity, crash rates, and memory usage per-site. Publish the telemetry (anonymized) to Verse communities as a distributed web-compatibility dataset. This turns the dual-backend situation from a migration cost into a crowd-sourced browser-engine benchmarking tool — and gives Servo upstream actionable compatibility data. See §Appendix A.9.

---
---

# Appendix A — Servoshell Debt Analysis & Radical Opportunity Inventory

**Date**: 2026-02-26
**Method**: Codebase audit (grep + structural analysis) + architectural doc cross-reference + this session's iterative analysis
**Scope**: Remaining servoshell-era debt, single-webview assumptions, compositor callback contracts, and 10 features that convert migration work into competitive advantages

---

## A.0 — Debt Inventory Summary

### A.0.1 Naming / Identity Debt (47 references) (approved)

Forty-seven `servoshell` / `ServoShell` references remain across the codebase:

| File | Count | Nature |
| --- | --- | --- |
| `tracing.rs` | 11 | **Functional**: all `RUST_LOG` filter strings use `servoshell::` prefix. Renaming breaks existing log filtering. Requires coordinated rename + user-facing migration note. |
| `egl/ohos/mod.rs` | 11 | Platform-specific embedder; lower priority but contributes to identity confusion. |
| `egl/android/mod.rs` | 4 | Same as above. |
| `running_app_state.rs` | 4 | Includes `ServoShellServoDelegate` struct name and documented preference comment. Highest-impact rename target (developer-facing). |
| `headed_window.rs` | 3 | "servoshell key bindings" comments. Easy doc-only fix. |
| `prefs.rs` | 3 | Preference loading comments. |
| `gui.rs` | 2 | Doc comments referencing servoshell composition model. |
| Others | 9 | Scattered across `event_loop.rs`, `window.rs`, `webxr.rs`, `desktop/mod.rs`. |

**Risk**: The `tracing.rs` references are the only *functional* debt — everything else is cosmetic. The tracing rename needs a migration path (accept both `servoshell::` and `graphshell::` filter prefix temporarily).

### A.0.2 Single-Webview Assumption Debt (30+ references) (approved)

The `focused_webview_hint` / `focused_webview_id` pattern in `tile_render_pass.rs` (20+ occurrences) and `tile_compositor.rs` (6+ occurrences) assumes a single active webview per workbench. This is a servoshell-era assumption where there was exactly one browser window with one focused tab.

**Current impact**: Focus ring rendering, Servo `make_current` / `paint` / `present` sequencing, and keyboard input routing all flow through a single `Option<WebViewId>`. This works because Graphshell currently has one visible composited webview at a time, but it blocks:
- Split-view with two simultaneously visible composited tiles
- Picture-in-picture or floating viewer overlays
- Graph-canvas inline content previews (live, not thumbnail)

**Migration path**: Replace `focused_webview_hint: Option<WebViewId>` with a `CompositorFocusSet` or per-tile focus state, then update all `tile_render_pass.rs` consumers. This is a prerequisite for multi-webview composition and should be tracked as a `lane:embedder-debt` item.

### A.0.3 Compositor Callback Contract Debt (22 references) (approved)

The `render_to_parent` → `PaintCallback` → `CallbackFn` pipeline in `tile_compositor.rs` (10 references) uses raw GL callback closures without the full pass contract defined by this document. The `CompositorAdapter` in `compositor_adapter.rs` (merged via PR #165) adds GL state isolation, but the overlay affordance pass is not yet explicitly sequenced — it still depends on egui layer ordering rather than the contract-mandated compositor pass ordering.

**Current state**: CompositorAdapter provides the state isolation guardrail (capture → callback → restore), but the three-pass architecture (UI Layout → Content → Overlay Affordance) is not yet structurally enforced. The overlay pass is "accidentally correct" due to egui `Order::Foreground`, which is exactly the fragile assumption this document was written to replace.

### A.0.4 God-Object / Module Size Debt (11 files > 600 lines) (approved)

| File | Lines | Issue |
| --- | --- | --- |
| `gui.rs` | 1845 | Still the primary decomposition target (Stage 4b). Mixes frame orchestration, state ownership, and render delegation. |
| `registries/mod.rs` | 1802 | Registry composition root. Size is partially justified by breadth of concerns. |
| `diagnostics.rs` | 1649 | Channel schema + inspector pane rendering. Should split schema from pane. |
| `headed_window.rs` | 1481 | Servo window host. Contains servoshell key binding comments. |
| `toolbar_omnibar.rs` | 1308 | Post-decomposition; still large but focused. |
| `gui_frame.rs` | 1199 | Per-frame rendering. Stage 4b target: State+Renderer split. |
| `tile_behavior.rs` | 921 | Tile interaction/behavior. Moderately sized but coupling-dense. |
| `running_app_state.rs` | 845 | Contains `ServoShellServoDelegate`. Stage 3 target (partially done). |
| `persistence_ops.rs` | 792 | Persistence operations. |
| `window.rs` | 719 | Embedder window. |
| `dialog.rs` | 695 | Dialog system. |

The embedder decomposition plan (Stage 4b) targets `gui.rs` and `gui_frame.rs` specifically. The 600-line guideline from that plan is aspirational — current trajectory suggests ~800–1000 is more realistic for coordinator modules.

### A.0.5 Frame Loop Debt (paint/present/make_current patterns) (approved)

The `make_current` → `prepare_for_rendering` → `paint` → `present` sequence appears in:
- `tile_compositor.rs` (compositor callback path)
- `gui.rs` (4× `make_current` calls, paint/present orchestration)
- `headed_window.rs` (window-level present)
- `window.rs` (GL context management)

This sequence is Servo-specific. When Wry is added, the frame loop must accommodate both "invoke GL callback + present" (Servo) and "sync overlay position" (Wry) paths without interleaving. The `TileRenderMode` dispatch in `tile_compositor.rs` is the correct branch point, but the `gui.rs` orchestration still assumes a single compositor pipeline.

---

## A.1 — Composited Content Replay / Time-Travel Debugging (approved)

**Problem it resolves**: Compositor state corruption bugs are notoriously hard to reproduce. The current GL state snapshot (5 fields: viewport, scissor, blend, active_texture, framebuffer_binding) captures enough to detect violations but not to *replay* them.

**Proposal**: Extend `GlStateSnapshot` capture to record a timestamped sequence per frame. Store N frames in a ring buffer (configurable via diagnostics). When a violation is detected, dump the snapshot sequence to the diagnostics channel with before/after deltas per callback.

**Radical angle**: Go further — intercept the GL command stream itself (via `glow` tracing hooks or a recording GL wrapper) during diagnostics mode. This creates a full GL command replay that can be exported, shared, and replayed on a different machine. A Servo developer could reproduce a Graphshell-reported compositor bug without running Graphshell.

**Beneficiaries**: Graphshell (debugging), Verso (bug reports with replay artifacts), Servo upstream (reproducible GL compositor issues).

**Prerequisites**: `compositor_adapter.rs` GL state capture (done), diagnostics channel infrastructure (done), ring buffer per-frame storage (new).

---

## A.2 — Multi-Backend Hot-Swap Per-Tile (approved)

**Problem it resolves**: When a page doesn't render correctly in Servo, the user must close the tab, change the default backend, and re-open. The `ViewerRegistry` already supports multiple viewer IDs per protocol, and `TileRenderMode` is resolved at viewer attachment time.

**Proposal**: Add a `SwapViewerBackend` workbench-authority intent that:
1. Snapshots the current viewer state (URL, scroll position, form data if feasible).
2. Detaches the current viewer.
3. Attaches the alternate viewer with the same URL.
4. Restores state where possible.
5. Re-resolves `TileRenderMode` from the new viewer.

Expose this via the Command Palette and a tile-chrome context action ("Try in Servo" / "Try in Wry").

**Radical angle**: This isn't just a fallback mechanism — it's *A/B testing for web rendering*. The user can split a tile and compare the same page in both backends side-by-side. No other browser offers per-tab engine selection at runtime.

**Beneficiaries**: Graphshell (UX), Servo upstream (compatibility gap discovery by real users).

**Prerequisites**: `ViewerRegistry` multi-backend resolution (done), Wry integration (Step 3+ of wry_integration_strategy), `SwapViewerBackend` intent design.

---

## A.3 — Compositor Pass Chaos Engineering (approved)

**Problem it resolves**: The GL state isolation contract is currently verified by unit tests with mock GL state. Production backend callbacks may corrupt state in ways that mocks don't simulate (driver-specific, platform-specific, version-specific).

**Proposal**: Add a `diagnostics.compositor_chaos` channel (gated behind a diagnostics feature flag) that, when enabled:
1. After each compositor content pass, randomly mutates 1–3 GL state fields (viewport to garbage values, scissor to inverted rect, blend mode toggled).
2. Verifies that the host renderer state is correctly restored before the overlay pass.
3. Records pass/fail per frame in the diagnostics channel.
4. Reports cumulative chaos-test results in the Diagnostic Inspector pane.

**Radical angle**: This is Netflix's Chaos Monkey applied to GPU compositor state. Instead of hoping isolation works, *prove it continuously under fire*. The diagnostics pane shows a live "compositor resilience score."

**Beneficiaries**: Graphshell (confidence), any Servo embedder that adopts the pattern.

**Prerequisites**: `CompositorAdapter` (done), diagnostics channel registration (done), randomized state mutation injector (new, ~100 lines).

---

## A.4 — Shared Compositor Protocol to Verso (approved)

**Problem it resolves**: Graphshell's compositor pass contract, state isolation adapter, and overlay affordance hooks are currently Graphshell-internal. Verso (as the native mod packaging Servo) doesn't benefit from this work. Other Servo embedders (if they emerge) would reinvent the same patterns.

**Proposal**: Extract the following as a formal trait boundary (initially within the Graphshell crate, eventually as a Verso-side contract):

```rust
pub trait CompositorPassContract {
    /// Called before content pass. Returns opaque state token for restore.
    fn pre_content_pass(&mut self) -> CompositorStateToken;
    /// Called after content pass. Restores host state using the token.
    fn post_content_pass(&mut self, token: CompositorStateToken);
    /// Called to render overlay affordances after content pass.
    fn overlay_affordance_pass(&mut self, tile_rect: Rect, focus_state: &TileFocusState);
    /// Returns the render mode for this viewer/tile pair.
    fn render_mode(&self) -> TileRenderMode;
}
```

**Radical angle**: Graphshell's fork debt becomes an *API contribution*. The trait contract could be proposed to Verso as the canonical embedder compositor interface, replacing ad-hoc `render_to_parent` callback conventions across all Servo embedders.

**Beneficiaries**: Verso (standard compositor API), Servo ecosystem (reproducible embedder patterns), Graphshell (upstream contract alignment reduces future drift).

**Prerequisites**: Pass contract document (this file), CompositorAdapter implementation (done), Verso mod boundary definition (partially done via `EmbedderApi` trait).

---

## A.5 — Content-Aware Overlay Affordances (approved)

**Problem it resolves**: Focus/hover rings use hard-coded colors that may be invisible or jarring against web content with matching colors (e.g., blue focus ring on a blue-dominant page, light ring on a white page).

**Proposal**: Along with allowing user-configured color, intensity, and opacity values, after the composited content pass writes web pixels to the host texture, sample a strip of pixels along the tile border (e.g., 4px inside each edge). Compute dominant luminance. Choose affordance color and opacity from a lookup table:
- Dark content → light affordance stroke (white/yellow, higher opacity).
- Light content → dark affordance stroke (blue/gray, standard opacity).
- Mixed → adaptive: per-edge luminance sampling, variable opacity.

Cache the luminance result per tile per N frames (content doesn't change every frame for most pages).

**Radical angle**: This is *content-responsive UI chrome* — the browser's interface adapts to the content it's showing, not the other way around. Combined with the PresentationDomain's ThemeRegistry, this creates a system where affordances are always visually optimal regardless of web content.

**Beneficiaries**: Graphshell (UX polish), accessibility subsystem (guaranteed contrast ratios for focus indicators).

**Prerequisites**: Composited overlay pass (content already in texture), pixel sampling API (new, ~50 lines), theme integration point.

---

## A.6 — Cross-Tile Compositor Transitions (approved)

**Problem it resolves**: When tiles are split, merged, or rearranged, the transition is currently instantaneous — a jarring jump. Conventional browsers handle this with tab-strip animations but not with live content during the transition.

**Proposal**: Because composited tiles render to Graphshell-owned textures, the compositor can retain the pre-transition texture and cross-fade/slide/scale it to the post-transition position over N frames (e.g., 150–300ms). During the transition:
1. The old tile texture is rendered at the old rect, animating toward the new rect.
2. The new tile texture (if different) fades in at the new rect.
3. Content callbacks continue rendering into the new texture at the new rect immediately.

**Radical angle**: This is *spatial continuity for web content* — the page doesn't disappear and reappear, it *moves* through space. This is core to the spatial browser identity. No tab-based browser can do this because they don't own the content pixels.

**Beneficiaries**: Graphshell (core spatial-browser identity), UX research (spatial memory retention through animated transitions).

**Prerequisites**: Texture caching per tile (new), transition animation system (can build on egui's `animate_value_with_time`), compositor multi-texture composition (extension of current single-callback model).

---

## A.7 — Per-Tile GPU Memory Budget with Graceful Degradation (approved)

**Problem it resolves**: Opening many composited webview tiles can exhaust GPU memory, causing driver-level failures or undefined behavior. Current mitigation is the lifecycle model (Active/Warm/Cold), but GPU memory is not directly tracked.

**Proposal**: 
1. Track estimated GPU memory per composited tile (texture dimensions × bit depth × buffer count).
2. Set a configurable GPU memory budget (default: 50% of reported GPU memory, or a fixed ceiling on integrated GPUs).
3. When the budget is exceeded, automatically degrade the least-recently-focused composited tiles to `Placeholder` render mode, displaying a visual tombstone.
4. The visual tombstone shows a frozen thumbnail + "Click to reactivate" affordance + a diagnostics badge showing why it was deactivated.
5. Reactivation re-promotes the tile to `CompositedTexture` and triggers viewer attachment.

**Radical angle**: This is OS-level memory management applied at the browser-tile level, with full transparency. The user sees exactly why a tile degraded and has one-click recovery. Combined with the visual tombstone plan, this creates a *self-managing workbench* that never crashes from resource exhaustion.

**Beneficiaries**: Graphshell (stability), users with constrained GPUs (graceful behavior instead of crashes).

**Prerequisites**: GPU memory estimation per tile (new), visual tombstone rendering (2026-02-26 plan), TileRenderMode runtime transition (partially supported), diagnostics channel for degradation events.

---

## A.8 — Differential Frame Composition (approved)

**Problem it resolves**: Every frame currently re-composites all visible tiles, even if their content hasn't changed. For document-heavy workflows (reading, reference browsing), most tiles are static most of the time.

**Proposal**:
1. After each content pass, compute a dirty flag from the backend (Servo reports recomposite-needed; Wry reports native-redraw-needed).
2. If a tile's content is clean, skip the `render_to_parent` callback entirely and reuse the previous frame's composited output.
3. The overlay affordance pass still runs every frame (affordances may change due to focus/hover even when content is static).
4. Track skip rate in a diagnostics channel (`compositor.frame_skip_rate`) for performance monitoring.

**Radical angle**: Most browsers re-render everything because they don't own the composition pipeline. Graphshell does. The GL state isolation adapter creates a natural boundary where the compositor can make per-tile decisions about whether to invoke the backend at all. This turns the isolation overhead into a *net performance gain* for the common case.

**Beneficiaries**: Graphshell (performance, power efficiency), laptop/mobile users (battery life).

**Prerequisites**: Backend dirty-flag API (Servo side: check `needs_recomposite`; Wry side: check native dirty rect), per-tile frame cache (texture or FBO retention), diagnostics instrumentation.

---

## A.9 — Viewer Backend Telemetry Races (approved)

**Problem it resolves**: When both Servo and Wry are available, there's no systematic way to determine which backend is *better* for a given site. Users report anecdotal preferences; engineers guess.

**Proposal**:
1. For a configurable set of URLs (user opt-in), maintain both `viewer:webview` and `viewer:wry` backends simultaneously. One is visible; the other is shadow-rendering off-screen.
2. Collect per-page telemetry: time-to-first-paint, time-to-interactive, memory usage, crash count, rendering fidelity score (via perceptual image diff if thumbnails are available).
3. Store telemetry locally in the graph model (node-level metadata, tagged via KnowledgeRegistry).
4. Optionally publish anonymized telemetry to Verse communities as `VerseBlob` reports — crowd-sourced web compatibility data.

**Radical angle**: This turns every Graphshell user into a *distributed browser engine benchmark node*. The resulting dataset — real-world page compatibility across Servo and system webviews — would be unprecedented. Servo upstream gets actionable compatibility data; the Graphshell community gets automatic backend recommendations per-site.

**Beneficiaries**: Servo upstream (compatibility data), Graphshell (automatic backend selection), Verse communities (shared web-compat knowledge).

**Prerequisites**: Wry integration (Step 5+), off-screen rendering path for shadow backend, telemetry schema (node metadata fields), Verse publish path (Tier 1+).

---

## A.10 — Mod-Hosted Overlay Passes (Compositor Extension Points) (approved)

**Problem it resolves**: The three-pass model (UI Chrome → Content → Overlay Affordance) is Graphshell-internal. Third-party or first-party mods have no way to inject rendering between content and chrome — they can only add egui widgets, which are subject to the same ordering fragility this document was written to solve. Depending on performance, it could be a good model to extend.

**Proposal**: Extend the compositor pass contract to accept registered overlay passes from mods:

```rust
pub struct CompositorPassRegistration {
    pub pass_id: &'static str,
    pub order: OverlayPassOrder, // BeforeAffordance, AfterAffordance, BeforeChrome
    pub render: Box<dyn Fn(&glow::Context, Rect, &TileState) + Send>,
    pub owner: ModId,
}
```

Mods register overlay passes through a new `CompositorRegistry` (atomic). The compositor dispatches registered passes in declared order between the Content pass and the UI Chrome pass.

**Examples**:
- **AI Annotation Layer**: An agent mod highlights content regions with semantic annotations (key facts, entities, sentiment) rendered as translucent overlays on web content.
- **Accessibility Contrast Overlay**: A subsystem overlay that draws WCAG contrast violation markers directly over content regions that fail contrast thresholds.
- **Privacy Redaction Screen**: A security mod that draws opaque overlays over detected PII regions before the content reaches the screen.
- **Reading Progress Heatmap**: A knowledge mod that visualizes which portions of a long page the user has actually read (scroll tracking → heat overlay).

**Radical angle**: The compositor becomes a *plug-in rendering pipeline*. This is architecturally impossible in Chromium-based browsers (extension APIs don't have access to the compositor). It's possible in Graphshell precisely because the pass contract + GL state isolation make it safe to interleave third-party rendering code in the compositor sequence.

**Beneficiaries**: Graphshell mod ecosystem (new capability class), AgentRegistry mods (visual output surface), accessibility subsystem (direct content annotation).

**Prerequisites**: Pass contract (this file), CompositorAdapter (done), mod lifecycle hooks (ModRegistry Phase 3+), `CompositorRegistry` atomic registry definition.

---

## A.11 — Cross-Feature Dependency Map

Features are not independent. The following dependency chains exist:

```
A.4 (Shared Protocol) ──→ enables A.3 (Chaos Engineering) upstream adoption
                       ──→ enables A.10 (Mod Passes) via formal extension points

A.8 (Differential Composition) ──→ enables A.7 (GPU Budget) efficiently
                                ──→ enables A.6 (Cross-Tile Transitions) without perf regression

A.1 (Replay Debugging) ──→ enables A.3 (Chaos Engineering) replay of failures
                        ──→ enables A.9 (Telemetry Races) debugging path

A.2 (Hot-Swap) ──→ enables A.9 (Telemetry Races) user-facing surface
               ──→ requires A.7 (GPU Budget) for safety when both backends active

A.5 (Content-Aware Overlays) ──→ enables A.10 (Mod Passes) examples (accessibility)
                              ──→ same texture-sampling mechanism serves A.6 (Transitions)
```

**Recommended implementation order** (based on prerequisites and value):

1. **A.8** (Differential Composition) — lowest risk, immediate performance value, unlocks A.6/A.7
2. **A.1** (Replay Debugging) — extends existing CompositorAdapter, high debugging value
3. **A.3** (Chaos Engineering) — builds on A.1, proves isolation contract
4. **A.4** (Shared Protocol) — formalizes trait boundary, enables upstream contribution
5. **A.5** (Content-Aware Overlays) — self-contained, high UX polish value
6. **A.7** (GPU Budget) — requires A.8 for efficiency, unlocks graceful degradation
7. **A.6** (Cross-Tile Transitions) — requires texture caching from A.8, high UX impact
8. **A.2** (Hot-Swap) — requires Wry integration, medium complexity
9. **A.10** (Mod Passes) — requires A.4 protocol + ModRegistry maturity, highest long-term value
10. **A.9** (Telemetry Races) — requires Wry + Verse integration, highest ecosystem value
