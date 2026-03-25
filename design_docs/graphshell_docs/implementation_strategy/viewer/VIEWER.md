# VIEWER — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `viewer_presentation_and_fallback_spec.md` — canonical viewer selection, render-mode, and fallback contract
- `wry_integration_spec.md` — Wry webview integration contract
- `webview_lifecycle_and_crash_recovery_spec.md` — webview lifecycle, warm/cold/crashed states, recovery
- `node_lifecycle_and_runtime_reconcile_spec.md` — node → pane lifecycle reconciliation
- `node_viewport_preview_spec.md` — thumbnail/preview surface contract
- `visual_tombstones_spec.md` — tombstone display for dead/removed nodes
- `clipping_and_dom_extraction_spec.md` — DOM-level clipping and content extraction
- `universal_content_model_spec.md` — content model underlying viewer rendering
- `2026-02-26_composited_viewer_pass_contract.md` — composited viewer render pass contract
- `../2026-02-28_ux_contract_register.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7)):
- **WCAG 2.2 Level AA** — viewer surfaces must expose accessible content structure; degraded/fallback states must remain operable and perceivable per SC 1.3.1, 4.1.2
- **OSGi R8** — viewer capability declaration and selection follow OSGi capability vocabulary
- **OpenTelemetry Semantic Conventions** — viewer fallback and degraded-state events follow OTel naming/severity

---

## 1. Purpose

This note defines the **Viewer** as the architectural owner of how content is presented once a destination exists.

It exists to keep one boundary explicit:

- Graph and Workbench decide what should be shown and where,
- Viewer decides how that content is rendered and what fallback state is visible.

---

## 2. What The Viewer Domain Feature Area Owns

- viewer selection
- placeholder and fallback presentation
- degraded-state presentation
- loading / blocked viewer surfaces
- overlay and presentation clarity rules

---

## 2A. Servo-First Rich Document Policy

Graphshell is **Servo-first for rich document rendering**, not Servo-only for
all content rendering.

This policy exists to make one product/design choice explicit:

- if content can be faithfully adapted into constrained HTML or otherwise fits
  Servo's strengths as a rich document renderer, Viewer should prefer Servo
  first,
- if content does not fit Servo naturally, Viewer should route to a
  content-native renderer rather than forcing Servo to act like a universal
  do-everything backend.

Practical interpretation:

- Servo is the preferred path for web content and adapted rich-document content
  such as reader-mode output, Gemini text rendered as HTML, markdown/read-only
  document adaptation, and similar block-structured content that benefits from
  document layout, links, styling, and clipping.
- Native or embedded viewers remain the preferred path for content types where
  document adaptation would be artificial, lossy, or operationally heavier than
  a dedicated renderer: plain text editing, image viewing, PDF-specific
  workflows, directory browsing, audio playback, and other specialized content
  surfaces.
- Wry is a compatibility web backend and fallback path, not a replacement for
  Servo's role as the primary rich-document renderer.

This is a **selection-policy principle**, not a claim that Servo should absorb
all renderer responsibilities. Viewer owns the decision about where Servo is the
right tool and where a content-native viewer is the right tool.

---

## 2B. Canonical Viewer Taxonomy

To keep renderer planning coherent, Graphshell distinguishes between three
different layers:

- **Viewer identity**: the user-facing or policy-facing content surface, such as
  `viewer:webview`, `viewer:text-editor`, `viewer:image`, or
  `viewer:directory`
- **Engine target**: the backend render package used to realize a viewer path,
  such as Servo-rendered HTML, Wry-hosted web content, or native-reader block
  rendering
- **Content adaptation**: the transformation layer that turns source material
  into a renderable form, such as `SimpleDocument`

These layers must not be collapsed into one vocabulary.

Canonical viewer families:

- **Rich-document/web viewer**
  - `viewer:webview` is the primary rich-document viewer surface
  - it prefers Servo-first paths for web and adapted rich-document content
- **Document-native viewers**
  - `viewer:plaintext`
  - `viewer:text-editor`
  - `viewer:image`
  - `viewer:pdf`
  - `viewer:directory`
  - `viewer:audio`
- **Internal/tool viewers**
  - settings, diagnostics, history, and other app-owned surfaces
- **Fallback viewer**
  - explicit placeholder or unsupported-content state

Interpretation rules:

- `viewer:markdown` is not required as a separate canonical viewer if markdown
  is already well-served by `viewer:plaintext`, `viewer:text-editor`, or the
  Servo-first rich-document adaptation pipeline.
- `ServoHtml`, `WryWebview`, and `NativeReader` are engine-target/backend terms,
  not the preferred top-level user-facing viewer taxonomy.
- Wry remains a compatibility backend path for web-class content; it should not
  expand the canonical viewer set by itself unless Graphshell intentionally
  exposes a user-visible compatibility mode.

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

Physics/motion presets (`Liquid`, `Gas`, `Solid` — see `../canvas/layout_behaviors_and_physics_spec.md`) may influence presentation feel and motion emphasis, but Viewer still owns visual fallback and visible degradation behavior. These presets do not own graph camera policy or camera-lock semantics.

---

## 4. Bridges

- Graph -> Viewer: node or graph content to render
- Workbench -> Viewer: pane host and destination rect
- Focus -> Viewer: active vs inactive presentation state

---

## 5. Architectural Rule

If a behavior answers "how is this content visibly presented right now?" it belongs to the **Viewer**.
