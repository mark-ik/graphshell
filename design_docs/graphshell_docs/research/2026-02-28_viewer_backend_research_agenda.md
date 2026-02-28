# Viewer/Backend Behavior and Presentation Contracts — Research Agenda

**Date**: 2026-02-28
**Status**: Research / Active
**Author**: Arc
**Feeds Into**:
- `implementation_strategy/viewer/2026-02-26_composited_viewer_pass_contract.md`
- `implementation_strategy/viewer/2026-02-24_universal_content_model_plan.md`
- `implementation_strategy/viewer/2026-02-23_wry_integration_strategy.md`
- `implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md`

---

## Background

Graphshell already has three render modes (CompositedTexture, NativeOverlay, EmbeddedEgui), a
three-pass compositor model, and a defined viewer registry with fallback chain. The specs are
architecturally coherent. The risk is exactly what the user framed: **spec elegance without
runtime coherence** — users encounter a different feel per content type, per render path, per
fallback level, and nothing in the spec validates whether those transitions are perceptible or
acceptable.

This agenda targets four empirical gaps the current docs cannot resolve from design alone.

---

## Thread 1 — Embedded vs. Externalized Viewers: User Expectation Mapping

### What the Docs Say

The composited viewer pass contract defines three render modes:
- **CompositedTexture**: Servo renders to a GPU texture; Graphshell compositor owns the pixel
  rect. Feels native to the app; enables overlays, focus rings, spatial effects.
- **NativeOverlay**: Wry places a platform webview above or behind the egui layer via OS window
  hierarchy. Content is visually "in" the tile but architecturally external; overlays cannot
  be composited on top without platform-specific hacks.
- **EmbeddedEgui**: Non-web viewers (plaintext, markdown, image, PDF) render into egui's own
  draw list. Fully embedded; no pixel ownership issues.

The Wry integration strategy notes that NativeOverlay creates a Z-order inversion problem:
the Wry window sits above the egui canvas in the OS compositor, so egui affordances (resize
handles, focus rings, drag targets) cannot be drawn over the content. The spec handles this
by routing affordances to the border/chrome layer, never the content rect.

The viewer state matrix confirms that only `viewer:webview` (Servo path) is operationally
stable with full composited rendering. Everything else is either placeholder or EmbeddedEgui.

### What Is Not Known

1. **Embeddedness expectation by content type.** Users have context-dependent expectations:
   a PDF opened from the graph may be expected to behave like Acrobat (externalized, feature-
   rich), or like a browser PDF viewer (embedded, limited controls). Which expectation
   dominates, and does it vary by task (reading vs. extracting vs. annotating)?

2. **When NativeOverlay mode is a problem users notice.** The spec acknowledges Z-order
   inversion and the overlay affordance workaround. But does the resulting chrome-only focus
   ring register as focus to users, or do they reach for the content area expecting hover
   affordances that aren't there?

3. **Exit behavior expectations.** If a viewer opens a non-embeddable format (e.g., a
   download, a native app deep-link), users expect one of: (a) the system default app opens,
   the tile stays as a placeholder; (b) the tile shows a "launched externally" state; (c)
   nothing visible happens but the action is done. Which of these is lowest friction?

4. **Overlay affordance visibility threshold.** The three-pass compositor places overlays in
   Pass 3 above composited content. At what overlay density or what tile zoom level do users
   stop perceiving the overlay as "part of the tile" vs. a separate floating chrome element?

### Research Methods

**Study 1.1 — Content-Type Embeddedness Expectation Survey (unmoderated, n=40)**
Present screenshots or short recordings of: (a) PDF in embedded egui scroll view, (b) PDF in
system viewer launched externally, (c) same for markdown, image, CSV. Ask: "Which feels right
for this content when you're browsing a graph of research?" Rate embeddedness preference on
5-point scale and collect open-ended justification.

**Study 1.2 — NativeOverlay Affordance Behavioral Probe (moderated, n=10)**
Give participants a graph with a NativeOverlay tile (Wry-rendered) and tasks requiring tile
interaction: "Resize this tile," "Drag this tile to another pane," "Mark this page as
important." Observe where they first click/hover. Note failures when they reach for the
content rect and encounter no affordance response.

**Study 1.3 — External Launch Expectation Interview (semi-structured, n=8)**
Show a graph tile for a `.exe` or `.app` deep-link. Ask: "What do you expect to happen when
you open this?" Follow up: "If the system default app opened, what would you expect to see in
this tile?" Probe for: placeholder, loading state, launch confirmation, nothing.

### Deliverable

A **Viewer Embeddedness Expectation Map**: a 3×3 grid of content types vs. task types
(reading/extracting/launching) with a recommended render mode for each cell, annotated with
the empirical confidence level. Feeds directly into ViewerRegistry selection policy.

---

## Thread 2 — Fallback Acceptability: What Degradation Users Will Tolerate

### What the Docs Say

The viewer presentation and fallback spec defines a five-stage degradation chain:
**Select → Present → Degrade → Fallback → Explain**. Each viewer must be able to hand off to
the next tier. The final fallback is always plaintext. The spec defines what the system
should do but not what users find acceptable at each tier.

The universal content model plan (`2026-02-24`) names specific fallback paths:
- PDF: pdfium-render (feature-gated) → plaintext extraction → placeholder with "Open
  Externally" button.
- CSV: EmbeddedEgui table → plaintext raw → placeholder.
- Image: resvg/image crate → placeholder with dimensions.
- Audio: symphonia + rodio → waveform placeholder → "Audio not playable in this build."

The viewer state matrix is explicit: PDF, CSV, image, audio viewers are declared but not
embedded — they are research targets, not shipped code. This means the current fallback for
all non-web, non-text content is the plaintext or placeholder path.

The composited viewer pass contract Appendix A identifies "GPU budget degradation path" as an
opportunity: under memory pressure, switch from CompositedTexture to EmbeddedEgui
transparently. No fallback acceptability criteria are defined for this transition.

### What Is Not Known

1. **Minimum viable fallback for each content type.** For PDF: is a plaintext dump
   acceptable? For images: is a filename + dimensions placeholder acceptable, or do users
   need a thumbnail? For audio: is "Audio not playable in this build" acceptable or confusing
   (sounds like a bug, not a feature toggle)?

2. **Fallback messaging register.** "Could not display this content" is generic. Does
   specificity help ("This is a .psd file; Graphshell can open it as plaintext only") or
   overwhelm? Should fallback messages mention what viewer would handle this and how to get
   it?

3. **GPU degradation transparency threshold.** If a tile silently degrades from
   CompositedTexture to EmbeddedEgui under memory pressure, the visual change is: overlays
   may disappear, composited effects (e.g., DOI opacity, focus rings) may change rendering
   path. At what degradation delta do users notice and attribute it to a bug vs. just accept
   it as normal rendering variance?

4. **"Open Externally" as a fallback destination.** The universal content model plan includes
   an "Open Externally" button as a fallback affordance. Is this button perceived as a
   solution or an apology? Does its presence reassure users that the content is accessible,
   or signal that the app failed?

### Research Methods

**Study 2.1 — Fallback Acceptability Rating (unmoderated, n=50)**
Show 8 fallback screens (PDF→plaintext, CSV→raw text, image→placeholder, audio→message,
archive→list, binary→hex, unknown→generic, build-feature-missing) and ask:
- "Is this acceptable for casual browsing?" (yes/no)
- "Is this acceptable when this file is central to your task?" (yes/no)
- "What would make this better?" (open text)

Threshold: accept a fallback if ≥70% of participants call it acceptable for casual use and
≥40% for task-central use.

**Study 2.2 — GPU Degradation Detection Probe (moderated, n=8)**
Run Graphshell with a memory pressure script that degrades tiles from CompositedTexture to
EmbeddedEgui mid-session. Observe whether participants notice the transition and what they
attribute it to. Target: ≤20% of participants attribute silent degradation to a bug.

**Study 2.3 — "Open Externally" Button Perception (moderated, n=10)**
Present a tile with a fallback state + "Open Externally" button. Task: "You need to read this
document. What do you do?" Note whether they click the button immediately, hesitate, look for
alternatives, or ask what it does. Follow-up: "Does this make you feel like the app handled
the file well or not?"

### Deliverable

A **Fallback Acceptability Threshold Table**: for each fallback tier in the
Select→Present→Degrade→Fallback→Explain chain, a concrete pass/fail criterion expressed as
a percentage of users who find it acceptable, plus a recommended fallback message template for
each content category. Feeds into `viewer_presentation_and_fallback_spec.md` §§ Degrade and
Fallback.

---

## Thread 3 — Tile Render Mode Consistency: Focus Rings, Overlays, and Composited Behavior

### What the Docs Say

The three-pass compositor contract defines strict pass sequencing:
1. **UI Layout Pass**: egui layout, tile chrome, physics canvas background.
2. **Composited Content Pass**: per-tile backend renders; GL state fully isolated.
3. **Composited Overlay Affordance Pass**: DOI rings, traversal sparklines, clipping handles,
   drag affordances — drawn over composited content.

The GL state isolation protocol specifies that each CompositorAdapter call captures/restores
scissor, viewport, blend, depth/stencil, and culling state. This ensures backends cannot
corrupt the egui render state. However, each render mode achieves different levels of overlay
fidelity:
- CompositedTexture: overlays can be drawn at pixel-perfect positions over content (Pass 3
  has full access to the content rect).
- NativeOverlay: overlays are restricted to the tile chrome/border because the content rect
  is owned by the OS window hierarchy.
- EmbeddedEgui: overlays are native egui widgets; the "overlay pass" is conceptually merged
  with the content pass since everything is egui draw calls.
- Placeholder: no content, overlays have nothing to indicate.

The composited viewer pass contract Appendix A lists "content-aware overlay affordances" as
an opportunity: DOI rings that change based on content type, focus rings that respect content
scroll position, clipping handles that only appear when clippable content is visible.

The workbench frame/tile interaction spec defines keyboard focus management separately from
visual focus rings, and notes that `focused_webview_hint` creates a single-webview assumption
debt that must be resolved for multi-pane correctness.

### What Is Not Known

1. **Cross-render-mode focus ring perceptual equivalence.** The spec achieves different focus
   ring rendering per mode (over-content for CompositedTexture, border-only for
   NativeOverlay, native widget for EmbeddedEgui). Do users perceive these as the same
   affordance or as three different visual languages?

2. **Overlay density at which users stop reading them.** The three-pass design enables rich
   overlay affordances (DOI rings, traversal sparklines, clipping handles, hover tooltips,
   drag handles). At what overlay count per tile does the canvas become illegible? Is there
   a density threshold beyond which overlays are counterproductive?

3. **Overlay attribution.** When a user sees a ring or badge on a tile, do they correctly
   attribute it to: (a) the content type, (b) the traversal history, (c) their interaction
   state, (d) a system-generated recommendation? Misattribution leads to confusion and
   distrust of the overlay layer.

4. **Focus ring behavior on tile-without-content.** The workbench spec defines placeholder
   tiles (loading, error, empty). What does a focus ring on a placeholder communicate, and
   does it need to look different from a focus ring on a loaded tile?

5. **Pass 3 composition timing.** The spec does not define whether Pass 3 overlays are drawn
   synchronously with Pass 2 or can be deferred. If overlay data (e.g., DOI score) is stale
   by 1–2 frames, do users notice flicker or perceive the overlay as reactive?

### Research Methods

**Study 3.1 — Focus Ring Cross-Mode Recognition (unmoderated, n=30)**
Show screenshots of focused tiles in all three render modes. Ask: "Which of these tiles has
keyboard focus?" and "Are these three states showing the same type of focus?" Record
attribution accuracy target: ≥85% correct cross-mode recognition.

**Study 3.2 — Overlay Density Threshold Study (within-subjects, n=20)**
Present tiles with incrementally richer overlay sets: (a) no overlay, (b) focus ring only,
(c) focus ring + DOI ring, (d) + traversal sparkline, (e) + clipping handle, (f) all + hover
tooltip. Ask after each increment: "Does this feel like useful information or clutter?" Locate
the density level where ≥50% of users first report "clutter." That becomes the maximum
simultaneous overlay budget.

**Study 3.3 — Overlay Attribution Behavioral Probe (moderated, n=10)**
Show a live graph with overlays active. Give no instructions. After 5 minutes of free
exploration, ask: "What do you think the rings on the tiles mean?" "What do you think makes
a tile glow differently from others?" Record correct/incorrect attributions and the
explanations users construct unprompted.

**Study 3.4 — Placeholder Focus Ring Interpretation (unmoderated, n=25)**
Show a placeholder tile (loading spinner) with a focus ring. Ask: "What does the ring on this
tile indicate?" Acceptable answers: keyboard focus, selected, active. Concerning answers:
loaded, ready, highlighted by system. Use results to define whether placeholder tiles need a
visually distinct focus state.

### Deliverable

A **Tile Overlay Visual Language Spec**: defines the maximum simultaneous overlay count per
tile, the cross-mode focus ring equivalence contract (what visual properties must be
preserved across CompositedTexture/NativeOverlay/EmbeddedEgui), the attribution labels that
must accompany each overlay type on first encounter, and the placeholder focus ring behavior.
Feeds into `2026-02-26_composited_viewer_pass_contract.md` Pass 3 and the workbench
frame/tile interaction spec.

---

## Thread 4 — Non-Web Viewer Priority: Highest-Leverage Content Types

### What the Docs Say

The universal content model plan defines the full viewer class hierarchy and crate choices:
- **viewer:plaintext** (syntect + pulldown-cmark) — already stable.
- **viewer:image** (image + resvg) — documented target; moderate effort; highest visual
  payoff for graph nodes derived from image URLs.
- **viewer:pdf** (pdfium-render, feature-gated) — highest effort; system dependency (pdfium
  .dll/.so); significant research/productivity use case.
- **viewer:csv** (egui_extras TableBuilder) — low effort; high leverage for data-oriented
  workflows.
- **viewer:directory** (std::fs) — very low effort; enables `file://` path nodes as
  browseable filesystem tiles.
- **viewer:audio** (symphonia + rodio, feature-gated) — niche; high effort relative to use
  frequency.

The viewer state matrix flags PDF and CSV as "declared, not embedded" — the architecture is
designed for them but the implementations are research/backlog items.

The GRAPHSHELL_AS_BROWSER doc emphasizes that `file://` URL support is a first-class target.
Directory tiles are the primary entry point for local filesystem navigation, making
`viewer:directory` a structural prerequisite for the file-browsing workflow.

The research accessibility spec notes that non-web content types have unique accessibility
requirements: PDFs may not have tagged structure; images may lack alt text; audio has no
visual representation. Viewer implementations must handle these gaps without failing silently.

The composited viewer pass contract Appendix A notes "mod-hosted overlay passes" as a future
capability: third-party viewers can register Pass 3 overlay hooks. This creates a path for
specialized viewers (e.g., a CAD file viewer, a scientific data viewer) without core changes.

### What Is Not Known

1. **Actual content-type distribution in target workflows.** The plan assumes research/
   productivity workflows, but it has not been validated which non-HTML content types appear
   most frequently in those workflows. Is PDF the dominant non-web type in a research context,
   or is Markdown + local files (directory + plaintext) more common day-to-day?

2. **Viewer switching cost in task flow.** When a user opens a graph node that resolves to
   a PDF and the viewer degrades to plaintext, does the task context break, or do users
   adapt? The switching cost determines how urgent full PDF rendering is relative to other
   priorities.

3. **Local file integration usage patterns.** `file://` node support exists architecturally.
   Do users actually want to put local directories and files in their graphs? Or do they
   primarily browse web content and only occasionally reference local artifacts? This
   determines whether `viewer:directory` is a high-priority primitive or a niche affordance.

4. **PDF-as-research-artifact expectations.** PDF viewers in browsers have converged on a
   feature set: zoom, search, page jump, annotation highlight. Does the graph-embedded PDF
   viewer need to match this feature set to be acceptable, or is read-only rendering without
   annotation sufficient for the primary use case (reading and potentially clipping content)?

5. **Audio/media in graph context.** Audio nodes could represent podcasts, recordings, or
   embedded media. Is audio playback in a spatial graph coherent — would users want multiple
   audio-playing tiles simultaneously, or does the graph context make audio confusing
   (no clear "primary" audio tile)?

### Research Methods

**Study 4.1 — Content-Type Frequency Diary Study (longitudinal, n=15, 2 weeks)**
Ask research/productivity workers to log every non-HTML file type they encounter in their
daily workflow that they wish they could open directly in a browser/spatial tool. Categorize
and rank by frequency. Secondary question: "For each type, is viewing in-context important,
or is opening in the native app always fine?" Use results to build a priority-weighted viewer
roadmap.

**Study 4.2 — Task Continuity with Fallback PDF (moderated, n=12)**
Give participants a research task (e.g., "Find and note three claims from these papers") where
all PDFs render as plaintext fallback. Measure: (a) task completion rate, (b) time on task
vs. control group with rendered PDFs, (c) number of times participants attempt to open the
PDF externally. The gap between conditions quantifies the urgency of full PDF rendering.

**Study 4.3 — Local File Graph Integration Motivation Interview (semi-structured, n=10)**
Ask heavy browser users: "Do you ever want to combine web research with local files in the
same workspace?" Probe for concrete scenarios. Follow up: "Would it help to have a local
folder as a node in your graph?" Record whether the use case is spontaneous (high leverage)
or only emerges with explicit prompting (low natural demand).

**Study 4.4 — Multi-Pane Audio Coherence Study (concept validation, n=8)**
Show a mockup of a graph with 3 audio-playing tiles simultaneously. Ask: "What do you expect
happens with the audio?" and "How would you control which one you're listening to?" Use
results to determine whether audio viewer needs play/pause coordination (single-active model)
or full multi-track support.

### Deliverable

A **Non-Web Viewer Priority Matrix**: ranks viewer implementations (image, PDF, CSV,
directory, audio) by: (a) content-type frequency in target workflows, (b) fallback
acceptability score from Thread 2, (c) implementation effort from the universal content
model plan. Produces a ranked implementation order and a minimum-viable-feature spec for each
viewer type (e.g., "PDF viewer needs: render + search + page jump; annotation is Phase 2").
Feeds directly into `2026-02-24_universal_content_model_plan.md` implementation sequencing.

---

## Cross-Thread Concerns

### The Coherence Gap

The core risk across all four threads is the same: each subsystem (compositor, viewer
registry, fallback chain, non-web viewers) is well-specified in isolation but the *seams*
between them — the moments when a user traverses from one render mode to another, or from
a loaded viewer to its fallback, or from content to overlay — are not specified from the
user's perspective. Research should explicitly probe seam moments, not just steady-state
behavior.

### Accessibility

All viewer implementations must be validated for accessibility at the fallback level:
- Plaintext fallback must produce screen-reader-readable output, not raw binary or
  render-artifact text.
- Focus rings must work with keyboard navigation in all render modes.
- Overlay affordances must have accessible labels, not just visual markers.

These are not post-hoc concerns; they must be built into the acceptance criteria for each
viewer type at specification time.

### Performance Boundary

The composited viewer pass contract defines a `≤16 ms total frame budget` with a suggested
`≤4 ms per active tile`. Non-web viewer rendering must be profiled against this budget.
EmbeddedEgui viewers (plaintext, CSV, image) render into the egui draw list and share the
frame budget. PDF rendering via pdfium-render must be benchmarked separately: pdfium renders
to a CPU bitmap and then uploads to GPU — this upload cost must be measured against the 4 ms
per-tile ceiling.

---

## Summary: Four Open Questions → Four Deliverables

| Thread | Core Question | Deliverable |
|--------|--------------|-------------|
| 1. Embedded vs. Externalized | What render mode matches user expectations per content type and task? | Viewer Embeddedness Expectation Map (3×3 grid) |
| 2. Fallback Acceptability | What degradation tiers are tolerable, and with what messaging? | Fallback Acceptability Threshold Table |
| 3. Render Mode Consistency | What overlay/focus-ring visual language works across all render modes? | Tile Overlay Visual Language Spec |
| 4. Non-Web Viewer Priority | Which viewers to build first, and to what feature depth? | Non-Web Viewer Priority Matrix |

Each deliverable maps directly to a named implementation plan or spec. This agenda is
complete when all four deliverables are available and have been incorporated as explicit
acceptance criteria into those plans.

---

## References

- `implementation_strategy/viewer/2026-02-26_composited_viewer_pass_contract.md`
- `implementation_strategy/viewer/2026-02-24_universal_content_model_plan.md`
- `implementation_strategy/viewer/2026-02-23_wry_integration_strategy.md`
- `implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md`
- `research/2026-02-27_viewer_state_matrix.md`
- `implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md`
- `research/2026-02-18_graph_ux_research_report.md`
- `research/2026-02-24_spatial_accessibility_research.md`
