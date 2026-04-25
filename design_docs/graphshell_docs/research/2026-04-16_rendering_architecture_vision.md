<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Rendering Architecture Vision: Blitz + anyrender + WebRender

**Date**: 2026-04-16
**Status**: Research / future vision (not actionable until iced migration completes)

**2026-04-24 follow-on**: The
[Blitz-shaped chrome scoping doc](../implementation_strategy/shell/2026-04-24_blitz_shaped_chrome_scoping.md)
fleshes out the "we own the assembly" variant of this vision in
concrete terms: component inventory, sliced execution plan with
estimates (~3.5–5 months), risks, decision criteria for when to
start. Updated with the
[2026-04-24 renderer-boot research](2026-04-24_iced_renderer_boot_and_isolation_model.md)'s
finding that iced's wgpu version split with Servo is **permanent**,
not temporary, which strengthens this vision's long-term case.
**Scope**: Could graphshell's chrome layer eventually move from iced to an
HTML/CSS/JS-based stack using Blitz (Stylo + Taffy) + anyrender + WebRender,
unifying the rendering pipeline with Servo and gaining CSS expressiveness?

**Related**:

- [`2026-04-14_wasm_portable_renderer_feasibility.md`](2026-04-14_wasm_portable_renderer_feasibility.md) — WASM-portable rendering stack analysis
- [`2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`](2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md) — framework comparison that led to iced selection
- [`2026-03-01_webrender_wgpu_renderer_research.md`](2026-03-01_webrender_wgpu_renderer_research.md) — WebRender wgpu backend research
- [`2026-04-03_servo_slint_wgpu_hal_interop_research.md`](2026-04-03_servo_slint_wgpu_hal_interop_research.md) — Servo interop research

---

## 1. The Question

Graphshell currently uses egui (migrating to iced) for all UI: chrome (toolbar,
menus, settings) and graph canvas. Iced was chosen for good reasons — maturity,
Elm architecture, shortest integration code, portability. But iced is still a
Rust-native widget toolkit. The question is whether there's a more natural
long-term architecture for a project that already embeds a browser engine.

The core observation: graphshell embeds Servo, which uses WebRender. Why should
the chrome use a completely separate rendering pipeline? And if the chrome were
HTML/CSS, the browser's shell would speak the same language as its content.

---

## 2. The Target Architecture

```
Chrome UI (HTML/CSS) ──→ Blitz (Stylo + Taffy) ──→ anyrender ──→ WebRender
Graph canvas (Rust) ───────────────────────────────→ anyrender ──→ WebRender
Web content (optional) ──→ Servo ──────────────────────────────→ WebRender
```

Three content sources, one compositor, one GPU pipeline.

### Components

**Blitz** is a native HTML/CSS renderer built on Stylo (Firefox/Servo's CSS
engine) and Taffy (flexbox/grid layout). It renders HTML/CSS without a full
browser engine — no DOM scripting, no network stack, no JS engine (unless you
add one). It uses anyrender for painting.

**anyrender** (<https://github.com/DioxusLabs/anyrender>) is a trait-based
rendering abstraction from DioxusLabs. Core traits: `PaintScene` (drawing
commands), `WindowRenderer` (window rendering), `ImageRenderer` (buffer
rendering). Current backends: vello, vello-cpu, skia. Applications push
commands into a `PaintScene`; backends execute them.

**WebRender** is Servo's GPU-accelerated display list compositor. It already
runs when Servo is present. An anyrender-webrender backend would let Blitz
and the graph canvas paint through the same pipeline Servo uses.

### Why anyrender, not raw WebRender

Using WebRender directly for the chrome would mean hand-computing every widget
position, doing your own text shaping, managing focus — accidentally writing a
UI toolkit. Blitz + anyrender avoids this: Stylo handles styling, Taffy handles
layout, anyrender abstracts the paint target.

The anyrender abstraction also enables backend flexibility: vello for
WASM/no-Servo targets, WebRender when Servo is present, skia as a fallback.

---

## 3. What This Buys Over Iced

**CSS expressiveness.** Flexbox, grid, media queries, transitions, variables,
`calc()`. Toolbar, radial menu, settings panel — trivial in CSS, fiddly in iced.

**Hot-reloadable UI.** Change a stylesheet, see it immediately. No recompile.
Transformative for UI iteration speed.

**Shared rendering pipeline.** One WebRender instance for chrome, graph, and
(optionally) Servo web content. No second GPU pipeline.

**Web-native identity.** The browser's chrome speaks the same language as its
content. There's a philosophical coherence — especially for a spatial browser.

**Lower contribution barrier.** HTML/CSS is a much larger skill pool than iced
widget authoring in Rust.

---

## 4. What Iced Provides That This Stack Doesn't (Today)

**Single-language type safety.** UI state, graph model, event handling — all
Rust, all compiled, all type-checked. No serialization boundary.

**Elm architecture.** Clean message-based state management that fits
graphshell's two-phase apply model (apply_intents + reconcile).

**Mature widget implementations.** Text input with selection, scrollable
containers, pick lists — done, tested, edge-cases handled. Blitz would need
interactivity wired up for all of these.

**Ships now.** Iced is stable (0.14+). Blitz is pre-crates.io. The
anyrender-webrender backend doesn't exist yet.

---

## 5. Character Difference

These produce different kinds of applications:

**Iced graphshell** feels like a Rust desktop app. Consistent, typed,
predictable. UI changes go through the compiler. The chrome is obviously
"not web" — it has its own aesthetic. It's an application that embeds a browser.

**Blitz graphshell** feels web-native. The chrome is HTML/CSS — fluid,
expressive, themeable with a stylesheet swap. The boundary between "app" and
"content" blurs. It's a browser that grew its own shell from the same material
it renders.

---

## 6. Prerequisites

This vision becomes actionable only after:

1. **Iced migration complete.** Graphshell is portable and owns its authority.
2. **UI stabilized.** We know what the UI should look and behave like —
   the "statue in the marble" has been found.
3. **Polished baseline implemented.** A sensible, working UI exists in iced,
   designed with an eye toward eventual web-stack modeling.
4. **anyrender-webrender backend exists.** Either we build it or someone does.
5. **Blitz matures.** Published on crates.io, stable API, proven in production.
6. **Interactivity layer solved.** Events flowing between CSS UI and Rust state
   management — the Blitz equivalent of iced's message/subscription model.

---

## 7. Decision Criteria

- If iced works everywhere we need and causes no friction → this stays as
  research and a cool future possibility.
- If we hit iced limitations (layout expressiveness, hot-reload needs,
  renderer unification with Servo) → revisit this document.
- The graph canvas could migrate to vello-via-anyrender independently of the
  chrome layer — these are separable decisions.
- If anyrender-webrender emerges from the ecosystem naturally (DioxusLabs or
  Servo community), the cost/benefit shifts significantly.

---

## 8. Sequencing Summary

```
NOW         Complete iced migration
            ↓
NEXT        Stabilize UI design, find the right UX
            ↓
THEN        Implement polished iced baseline
            ↓
EVALUATE    Does iced cause pain? Do we need renderer unification?
            ↓
IF YES      Build/adopt anyrender-webrender, prototype Blitz chrome
IF NO       Keep iced, file this as validated research
```
