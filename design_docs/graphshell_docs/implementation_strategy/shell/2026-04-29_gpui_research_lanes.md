# GPUI Research Lanes and Task Checklist

**Date:** 2026-04-29
**Status:** Active

## Critical path

1. **Finish research capture**
2. **Build a tiny GPUI lab app**
3. **Compare raw GPUI vs `gpui-tea`**
4. **Prototype graph canvas interaction**
5. **Prototype external texture composition**
6. **Only then start a real `gpui-shell` / `gpui-graphshell-host` crate**

---

## Lane 0 — Branch/document hygiene

**Purpose:** make the current branch clean enough to iterate on.

**Tasks:**
- [x] Commit the GPUI research doc update separately.
- [x] Decide what to do with the pre-existing dirty `2026-04-28_iced_jump_ship_plan.md`. (Marked as paused/deferred).
- [x] Add a short task checklist to the GPUI plan or a companion tracking doc (This document).
- [ ] Keep the GPUI branch research-only until the first code spike lands.

**Deliverable:**
- A clean `gpui` branch with one research commit.

---

## Lane 1 — GPUI ecosystem decision matrix

**Purpose:** turn `awesome-gpui`, `gpui-ce`, Glass-HQ, Zed, and `gpui-tea` into an actionable matrix.

**Options to score:**

- **GPUI line**
  - upstream Zed GPUI
  - Glass-HQ extraction
  - `gpui-ce`
- **App architecture**
  - raw GPUI `Entity<T>` / observer model
  - `gpui-tea`
  - portable-contract shim
- **Chrome**
  - `gpui-component`
  - hand-rolled GPUI elements
- **Graph canvas**
  - Graphshell-owned GPUI canvas
  - `gpui-flow` patterns
  - `ferrum-flow` patterns
  - Vello/shared-`wgpu` path
- **Content surface**
  - external `wgpu::TextureView` patch
  - temporary pixel-copy fallback
  - sprite-atlas/image fallback

**Current likely ratings:**

| Candidate | Rating | Why |
|---|---:|---|
| Glass-HQ / Zed renderer line | Spike | Only realistic path for external texture patch |
| `gpui-ce` | Study | Great learning corpus, weak shared-`wgpu` fit |
| `gpui-component` | Spike/adopt selectively | Strongest chrome accelerator |
| `gpui-tea` | Serious spike | Best bridge from iced-style architecture |
| `gpui-flow` | Spike/study | Fast graph-canvas behavior prototype |
| `ferrum-flow` | Study | Good architecture ideas, likely too alpha |
| `gpui-router` / `gpui-nav` | Defer | Wrong navigation layer for Graphshell |
| `gpui-hooks` | Defer | Ergonomic, but risks hiding lifecycle authority |

**Deliverable:**
- A small table with “adopt / spike / study / defer / reject” (See above).

---

## Lane 2 — Tiny GPUI lab app

**Purpose:** test GPUI structure before pulling Graphshell complexity in.

Create a tiny throwaway crate or example, probably something like:
- `crates/gpui-lab`
- or `experiments/gpui-lab`

**Minimum app:**
- one GPUI window;
- one root shell view;
- one host-owned model;
- one command/action;
- one custom canvas element;
- one fake Navigator/content rectangle;
- one simulated external texture placeholder;
- one command palette or palette-like interaction.

**Two versions:**

1. **Raw GPUI version**
   - `Entity<GraphModel>`
   - `Entity<ShellModel>`
   - explicit `cx.notify()`
   - GPUI actions/key bindings
2. **`gpui-tea` version**
   - `Program<GraphshellLabModel>`
   - `Msg`
   - `Command`
   - `Subscription`
   - keyed async effect for fake content load
   - subscription for fake frame ticks

**Compare:**
- amount of glue;
- clarity of command routing;
- focus handling;
- async/cancellation ergonomics;
- whether `graphshell-runtime`-style host-neutral contracts stay clean.

**Deliverable:**
- a yes/no recommendation: “use `gpui-tea` for Stage A” or “go raw GPUI”.

---

## Lane 3 — `gpui-tea` bridge spike

**Purpose:** determine if `gpui-tea` is an architecture bridge or just an interesting detour.

**Tasks:**
- [ ] Verify `gpui-tea` against the selected GPUI line.
  - Current issue: it depends on crates.io `gpui = 0.2.2`.
  - Our renderer patch may need Glass-HQ/Zed git.
- [ ] Model the five Graphshell domains as child models:
  - `GraphModel`
  - `NavigatorModel`
  - `WorkbenchModel`
  - `ViewerModel`
  - `ShellModel`
- [ ] Test `Composite` child scoping.
- [ ] Test keyed async effects for:
  - search query replacement;
  - Navigator load cancellation;
  - graph layout recomputation;
  - command-palette filtering.
- [ ] Test subscriptions for:
  - frame ticks;
  - runtime events;
  - content-surface events.
- [ ] Check whether `gpui-tea` can wrap Graphshell’s existing `HostIntent` / `ActionRegistry` vocabulary cleanly.

**Deliverable:**
- Decision: `gpui-tea` as Stage A wrapper, or pattern-only reference.

---

## Lane 4 — Graph canvas GPUI prototype

**Purpose:** figure out whether GPUI-native drawing is enough before requiring Vello/shared-`wgpu`.

**Tasks:**
- [ ] Build a GPUI custom `canvas`/`Element` for a tiny graph:
  - nodes;
  - Bezier edges;
  - pan/zoom;
  - selection;
  - hit-testing;
  - drag-to-move;
  - contextual action surface anchor.
- [ ] Map existing `crates/graph-canvas` types into GPUI:
  - camera;
  - scene;
  - hit-test;
  - interaction;
  - projection.
- [ ] Study `gpui-flow` for:
  - pan/zoom behavior;
  - node renderer registration;
  - handles;
  - minimap;
  - viewport culling;
  - undo/redo.
- [ ] Study `ferrum-flow` for:
  - plugin architecture;
  - graph command model;
  - collaboration/undo/redo ideas;
  - separation between graph model, interaction, and renderer.
- [ ] Decide if Graphshell should:
  - borrow patterns only;
  - wrap `gpui-flow`;
  - or keep everything in `graph-canvas`.

**My bias:**
- **Do not let `gpui-flow` own Graphshell’s graph model.**
- Use it to benchmark/borrow interaction ideas.
- Keep `graph-canvas` authoritative.

**Deliverable:**
- `gpui-graph-canvas-viewer` design sketch or prototype.

---

## Lane 5 — External texture / shared `wgpu` spike

**Purpose:** this is still the hardest technical gate.

**Tasks:**
- [ ] Inspect selected GPUI renderer line:
  - Glass-HQ;
  - upstream Zed;
  - current `PaintSurface`;
  - current `WgpuContext`;
  - renderer resource lifecycle.
- [ ] Patch shape:
  - expose GPUI `wgpu::Device` / `Queue`;
  - add `ExternalWgpuSurface`;
  - add `PaintSurface::WgpuTexture`;
  - composite texture in the GPUI renderer.
- [ ] First proof:
  - generated checkerboard texture;
  - same GPUI device;
  - repaint/resize stable;
  - no cross-device copy.
- [ ] Second proof:
  - fake browser/content surface;
  - then real Serval/NetRender surface.
- [ ] Track:
  - device loss;
  - resize;
  - texture generation;
  - alpha mode;
  - clipping/z-order/masks.

**Deliverable:**
- screenshot + narrow patch + notes suitable for Glass-HQ/Zed outreach.

---

## Lane 6 — Shell chrome prototype

**Purpose:** see whether GPUI materially improves shell polish over iced.

**Tasks:**
- [ ] Try `gpui-component` for:
  - Dock;
  - Tabs;
  - Tree;
  - virtualized List/Table;
  - CommandPalette;
  - Notification;
  - Theme.
- [ ] Wrap all uses behind Graphshell-owned interfaces.
- [ ] Avoid letting `gpui-component` dictate:
  - Graphshell runtime authority;
  - focus model;
  - content-surface lifecycle;
  - navigation semantics.
- [ ] Compare against iced stack:
  - iced core;
  - `iced_aw`;
  - `libcosmic`;
  - `iced_webview`.

**Deliverable:**
- chrome comparison: what GPUI actually gives us that iced does not.

---

## Lane 7 — Focus, input, and action dispatch

**Purpose:** prove GPUI’s focus model helps Graphshell instead of just moving complexity around.

**Tasks:**
- [ ] Map six-track focus to GPUI:
  - SemanticRegion;
  - PaneActivation;
  - GraphView;
  - LocalWidget;
  - EmbeddedContent;
  - ReturnCapture.
- [ ] Test:
  - command palette open/close;
  - focus return;
  - canvas focus;
  - embedded content focus;
  - right-click/context palette;
  - keyboard navigation;
  - IME text input path.
- [ ] Convert sample actions:
  - `ActionRegistry` → GPUI `actions!`;
  - key bindings → GPUI key contexts;
  - command palette selection → `HostIntent`.

**Deliverable:**
- proof that GPUI’s `FocusHandle` improves or at least cleanly represents Graphshell focus.

---

## Lane 8 — Accessibility / AccessKit gap

**Purpose:** avoid getting trapped by a beautiful GPUI shell that cannot meet the accessibility target.

**Tasks:**
- [ ] Confirm current GPUI AccessKit status.
- [ ] Identify whether Zed has internal accessibility work.
- [ ] Compare iced AccessKit progress.
- [ ] Define minimum Graphshell accessibility tree:
  - graph reader;
  - panes/tabs;
  - command palette;
  - Navigator surfaces;
  - embedded content boundary.
- [ ] Decide if GPUI experiment must include a host-side AccessKit bridge.

**Deliverable:**
- hard go/no-go condition for GPUI migration.

---

## Lane 9 — Real Graphshell host skeleton

Only after the lab and external texture proof.

**Tasks:**
- [ ] Add `crates/gpui-shell` or `crates/gpui-graphshell-host`.
- [ ] Add `crates/gpui-graph-canvas-viewer` if graph prototype succeeds.
- [ ] Wire:
  - `graphshell-runtime`;
  - `graph-canvas`;
  - GPUI model layer;
  - shell chrome;
  - fake content surface;
  - then real content surface.
- [ ] Keep all GPUI types out of:
  - `graphshell-runtime`;
  - `graph-canvas` core;
  - existing app/domain logic wherever possible.

**Deliverable:**
- one GPUI window running a minimal Graphshell shell with graph canvas and placeholder Navigator surface.

---

## Suggested immediate task order

If I were turning this into actual tickets, I’d start with these:

1. **Commit current GPUI research docs**
   - isolate from the existing dirty iced doc.
2. **Create GPUI lab crate**
   - tiny window, root view, one model, one action.
3. **Implement lab twice**
   - raw GPUI;
   - `gpui-tea`.
4. **Prototype graph canvas in lab**
   - pan/zoom, nodes, Bezier edge, hit-test.
5. **Study `gpui-flow` against our graph-canvas needs**
   - interaction borrow list;
   - dependency recommendation.
6. **Select GPUI line for renderer patch**
   - Glass-HQ vs upstream Zed vs `gpui-ce`.
7. **Implement checkerboard external texture proof**
   - shared device or fail fast.
8. **Write migration decision memo**
   - “GPUI remains research only” vs “begin real host crate”.