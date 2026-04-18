# Rendering Architecture Vision: Toward a Unified Pipeline

*Research note — captures architectural thinking from the M5 iced-migration
window for future reference. Not a commitment, not a current priority.*

---

## 1. Why this doc exists

The active priority is the iced host migration (M5). That work is progressing
on its own merits: portable Rust UI, mature widget library, single-language
stack. The question this doc captures arose mid-migration and is worth
preserving while the context is fresh:

> Once iced has stabilized graphshell's UI and made it portable, could the
> chrome layer eventually move to an HTML/CSS/JS-based stack — Blitz
> (Stylo + Taffy) for layout, [anyrender][anyrender] for the rendering
> abstraction, WebRender as the backend — unifying the rendering pipeline
> with Servo and gaining CSS expressiveness?

This doc records the architectural shape, what would be gained, what would
be lost, and what would have to be true before it became viable. It is
explicitly not a plan.

[anyrender]: https://github.com/DioxusLabs/anyrender

---

## 2. Current state (iced)

The M5 migration moves graphshell's chrome from egui onto iced.

What iced provides:

- Elm-style architecture (message-driven update + view).
- Mature widget library (text input, scroll, focus, layout primitives).
- wgpu rendering backend with broad platform coverage.
- Single-language Rust stack — UI logic, runtime, and graph code all share
  types and tooling.
- Active development with clear API direction.

Why it's the right choice now:

- The M3.5 runtime/host boundary work needs a *second* host to validate
  portability claims. iced is mature enough today to play that role.
- Stabilizing what the chrome *should be* (commands, focus regions, modal
  semantics) is more valuable than stabilizing how it's rendered.
- It ships. Battle-tested in real apps.

iced is the platform we settle and design against. Anything beyond that is
contingent on iced limitations actually showing up in practice.

---

## 3. Target architecture (future, hypothetical)

If a future migration off iced ever happens, the most coherent target is a
single rendering pipeline shared with Servo content:

```
┌──────────────────────────────────────────────────────────────────┐
│  Chrome UI (HTML/CSS)                                            │
│       Stylo (CSS engine) ─→ Taffy (layout)                       │
│                              │                                   │
├──────────────────────────────┼───────────────────────────────────┤
│  Graph canvas (Rust)         │                                   │
│       direct scene building  │                                   │
│                              ▼                                   │
├──────────────────────────────────────────────────────────────────┤
│                       anyrender::PaintScene                      │
│                              │                                   │
├──────────────────────────────┼───────────────────────────────────┤
│                              ▼                                   │
│                    WebRender (or vello)                          │
└──────────────────────────────────────────────────────────────────┘

Web content (Servo) feeds into the same WebRender instance as a
content source — not as a structural dependency of the UI.
```

Three properties define the shape:

1. **One compositor, one GPU pipeline.** Chrome, graph canvas, and web
   content all paint into the same backend.
2. **Servo as content, not chassis.** The browser engine becomes one of
   several content sources rather than the structural foundation.
3. **anyrender as the abstraction.** Rendering targets stay swappable —
   WebRender for the integrated path, vello for graph-only experiments,
   skia for portability tests.

---

## 4. What anyrender is

[anyrender][anyrender] is a trait-based rendering abstraction from the
DioxusLabs ecosystem. The relevant traits are:

- `PaintScene` — describes a paintable scene (paths, clips, transforms,
  text runs).
- `WindowRenderer` — drives a windowed render target.
- `ImageRenderer` — drives an offscreen render target.

Current backends shipped with anyrender:

- `anyrender_vello` (vello, GPU compute)
- `anyrender_vello_cpu` (vello CPU rasterizer)
- `anyrender_skia` (skia)

What does **not** exist today: an `anyrender_webrender` backend. That
would have to be built. The work is bounded — WebRender's display-list
API is well-documented and anyrender's surface is small — but it's not
free.

Blitz itself is the Dioxus team's HTML/CSS rendering layer that uses
Stylo + Taffy + anyrender. As of writing it is pre-crates.io and
under active development.

---

## 5. What this stack would buy over iced

- **CSS layout expressiveness.** Flexbox, grid, media queries,
  transitions, animations, custom properties — without re-implementing
  any of it in widget code.
- **Hot-reloadable UI.** Change a stylesheet, see the chrome update,
  no recompile. Significant for the iteration loop on a UI-heavy app.
- **Shared rendering pipeline with Servo.** Chrome and content composite
  through the same WebRender, removing the impedance mismatch that
  currently exists between widget rendering and webview rendering.
- **Web-native chrome identity.** The browser chrome speaks the same
  language as the content it browses. Lower cognitive overhead for
  contributors who know web tech but not Rust UI frameworks.
- **Lower contribution barrier for chrome work.** HTML/CSS for a button
  is a smaller ask than learning iced's widget conventions.
- **WASM portability.** anyrender's abstraction makes the same chrome
  potentially runnable in a browser tab — a graphshell-in-graphshell
  scenario that's interesting for demos and federated identity flows.

---

## 6. What iced provides that this stack doesn't (today)

- **Single-language type safety across UI and logic.** Messages,
  models, and views all share Rust's type system. CSS chrome would
  need an interop layer for state and event handling.
- **Mature widget state machines.** Text input cursors, scroll
  inertia, focus management, IME — iced has these polished. Blitz is
  earlier in its life.
- **Elm architecture's clean update model.** Predictable, testable,
  no implicit reactive graph. CSS-driven UI tends to push state into
  the DOM and out of typed structures.
- **Ships now.** Real apps run on iced today. Blitz is approaching
  but not yet there.
- **Battle-tested.** Years of edge-case fixing across many users.

---

## 7. Prerequisites before this becomes viable

In rough order:

1. **Complete the iced migration.** Port surfaces, settle authority
   ownership on `GraphshellRuntime`, get parity. (M5–M6.)
2. **Stabilize what the chrome should look and behave like.** Use
   iced as the design surface to lock in commands, focus regions,
   palette behavior, modal semantics. The lessons feed any future
   port.
3. **Implement a polished baseline with one eye on web-stack
   modeling.** When designing iced screens, prefer patterns that
   would also map cleanly to CSS — declarative layout over imperative
   measurement, semantic regions over pixel coordinates.
4. **`anyrender_webrender` backend must exist.** Either upstream or in
   our tree. This is the single largest engineering prerequisite.
5. **Blitz must mature.** Crates.io release, stable API, working IME
   and focus management.
6. **Interactivity / event layer must be solved.** How does a CSS
   button dispatch a `WorkbenchIntent`? The shape of this binding —
   data-attribute dispatch, embedded JS bridge, declarative
   command attributes — needs research.

None of these are blockers we should chase early. They're filters: if
they don't all clear, the migration stays as a research note.

---

## 8. Decision criteria

The honest test:

- **If iced works everywhere we need it and causes no real pain →**
  this stays as research. Don't migrate for elegance.
- **If we hit specific iced limitations that bite repeatedly →**
  layout expressiveness, hot-reload cost during chrome iteration,
  the cost of bridging widget rendering and webview rendering — then
  revisit.
- **If the graph canvas alone benefits from anyrender (vello
  backend) →** that migration is independent and could land first.
  The graph canvas is already a custom paint surface; swapping its
  backend doesn't touch chrome.

Two failure modes to avoid:

- **Premature unification.** Locking in WebRender + anyrender +
  Blitz before any of them are stable, on the strength of an
  architectural diagram, would trade a working stack for a moving one.
- **Permanent "not yet."** Treating this as eternally future because
  it's expensive ignores that real costs of the current stack
  (impedance with Servo, layout limits) compound. If those costs
  start hurting, the threshold for revisiting drops.

The right posture: ship iced. Pay attention to where it pinches.
Re-read this doc when those pinch points start dictating workarounds
rather than acceptable trade-offs.

---

## 9. Related context

- M3.5 runtime boundary work — the runtime/host split is what makes
  any future host swap tractable.
- iced migration plan — the active execution plan that this doc
  defers to.
- Servo + WebRender are already in the dependency graph for content
  rendering; the integrated chrome path would extend that surface,
  not introduce it.
