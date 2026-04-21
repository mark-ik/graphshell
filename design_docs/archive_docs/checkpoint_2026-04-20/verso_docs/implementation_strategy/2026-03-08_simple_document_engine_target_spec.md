# SimpleDocument and EngineTarget Spec

**Date**: 2026-03-08
**Status**: Canonical contract
**Priority**: Required before Gemini resolver, Reader Mode, or markdown pipeline implementation

**Related docs**:

- `2026-02-24_universal_content_model_plan.md` — UCM Steps 11–12 (where these types originate)
- `universal_content_model_spec.md` — Viewer selection policy; O4 tracked as open concern
- `2026-03-08_servo_text_editor_architecture_plan.md` — text-editor short-circuit (exempted from this pipeline)
- `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` — core/host split; these types are host-only

---

## 1. Scope and Purpose

`SimpleDocument` and `EngineTarget` are the intermediate types for the **Servo-first content adaptation pipeline** (UCM Step 12). They allow Graphshell to transform non-HTML content (Gemini text, reader-mode extracted HTML, Markdown docs, safe previews) into a form that Servo can render, without modifying Servo itself.

These types are **host-only**. They are not WASM-clean and do not belong in `graphshell-core`. The pipeline that uses them runs in the desktop host.

This spec defines backend/render-target contracts, not a replacement taxonomy
for viewer identity. `EngineTarget` answers "how should this adapted content be
realized by the runtime?" It does not answer "what canonical viewer class is
this?" The Viewer domain remains the authority for viewer identity and
selection semantics.

This spec closes open concern O4 from `../../../archive_docs/checkpoint_2026-03-27/graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md`.

---

## 2. `SimpleDocument`

`SimpleDocument` is a format-agnostic block-structured intermediate model. It is the canonical output of any content source that targets the adaptation pipeline.

```rust
// crates/graphshell-desktop/src/viewer/adaptation/simple_document.rs

pub enum SimpleDocument {
    Blocks(Vec<SimpleBlock>),
}

pub enum SimpleBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    Link { text: String, href: String },
    Quote(String),
    CodeFence { lang: Option<String>, text: String },
    List { ordered: bool, items: Vec<String> },
    Rule,
}
```

### 2.1 Producers

The following content sources produce a `SimpleDocument`:

| Source | How |
| --- | --- |
| `text/gemini` content | `GeminiRenderer::parse()` — line-by-line format; exact mapping in §5 |
| HTTP Reader Mode extraction | DOM-to-blocks pass on Servo's extracted readable content |
| `text/markdown` via local `File` node (read-only) | `pulldown-cmark` → block mapping |
| Safe preview of untrusted content | Sanitized subset; all blocks allowed except `Link` with non-`https` href |

**Not a producer**: `viewer:text-editor`. Editable `text/*` + `File` nodes short-circuit directly to `editor-core` and never enter the adaptation pipeline. See `2026-03-08_servo_text_editor_architecture_plan.md §9`.

### 2.2 Consumers

| Consumer | How |
| --- | --- |
| `EngineTarget::ServoHtml` compiler | Renders blocks as constrained HTML fed to Servo |
| `EngineTarget::NativeReader` | Renders blocks directly via egui layout (fallback, low-surface) |

### 2.3 Invariants

- `SimpleDocument` never contains raw HTML. Block types are enumerated and exhaustive.
- `Link.href` values are validated before entering the document. Non-`https`/`gemini`/`file` schemes are either rejected or stored as plain text.
- `CodeFence.lang` is an advisory syntax hint; renderers must not fail on unknown languages.
- `SimpleDocument` is not persisted. It is an ephemeral pipeline artifact computed from source content on each load.

---

## 3. `EngineTarget`

`EngineTarget` is the output of the adaptation pipeline — the final render package bound to a specific rendering path.

```rust
// crates/graphshell-desktop/src/viewer/adaptation/engine_target.rs

pub enum EngineTarget {
    ServoHtml {
        html: String,
        base_url: Option<String>,
        content_security_policy: String,
        policy: RenderPolicy,
        /// User-supplied CSS injected into the compiled document's <style> block.
        /// Appended after the Graphshell default stylesheet; user rules win on specificity.
        /// None = use Graphshell default only.
        user_stylesheet: Option<String>,
    },
    WryWebview {
        source_url: String,
    },
    NativeReader {
        doc: SimpleDocument,
        policy: RenderPolicy,
        /// Which visual theme profile to apply. Defaults to the source-appropriate preset.
        /// Users may select a different profile via the node's Render With menu.
        theme: NativeReaderTheme,
    },
}

/// Visual theme for NativeReader rendering.
/// The default for Gemini content is `Gemini` (minimal, respects capsule author conventions).
/// The default for Reader Mode and Markdown is `Readable`.
pub enum NativeReaderTheme {
    /// Minimal presentation aligned with Gemini community conventions:
    /// monospace or serif body, generous line height, no decorative chrome.
    Gemini,
    /// Clean readable prose layout — Reader Mode and Markdown default.
    Readable,
    /// User-defined profile stored in GraphshellProfile. Loaded by name at render time.
    Custom(String),
}

pub struct RenderPolicy {
    pub scripts_allowed: bool,
    pub remote_subresources_allowed: bool,
    pub storage_allowed: bool,
    pub cookies_allowed: bool,
    pub intercept_links: bool,
}
```

### 3.1 Target selection policy

The pipeline selects an `EngineTarget` based on the following ordered rules:

| Priority | Condition | Target |
| --- | --- | --- |
| 1 | Edit-intent text node (`address_kind = File`, `mime_hint` in `text/*`, edit intent set) | **Short-circuit** — skip pipeline, route to `viewer:text-editor` |
| 2 | Source is `text/gemini` | `NativeReader` with `theme: NativeReaderTheme::Gemini` — **default for Gemini** |
| 3 | Source can be compiled to valid constrained HTML (Reader Mode, Markdown, safe preview) | `ServoHtml` with restrictive `RenderPolicy` |
| 4 | Source is a raw web URL and Servo fails or user selects Wry | `WryWebview` (compatibility fallback) |
| 5 | Source is `SimpleDocument` and Servo compilation fails or is unavailable | `NativeReader` with `theme: NativeReaderTheme::Readable` (fallback) |

**Gemini default is `NativeReader`.** Gemini content is presentation-agnostic by protocol design; the Gemini community expects minimal, text-first rendering without web-engine overhead. `NativeReader` with the `Gemini` theme is the correct default. `ServoHtml` remains available as an explicit user choice via `Render With`.

`ServoHtml` is the preferred target for HTTP Reader Mode, Markdown, and safe previews. `WryWebview` is not a general target for `SimpleDocument` sources.

Interpretation:

- `EngineTarget` is a backend choice underneath viewer policy.
- `ServoHtml` and `NativeReader` are peer realization paths for adapted rich-document content, each preferred for different source types.
- `WryWebview` is the compatibility backend for eligible raw web content, not a canonical answer for arbitrary adapted document sources.
- A user-facing `Render With` command exposes backend choice (NativeReader / ServoHtml) and, for ServoHtml, a CSS editor. These choices still sit under the canonical viewer taxonomy defined in `VIEWER.md`.

### 3.2 `RenderPolicy` defaults by source

| Source | Default target | `scripts_allowed` | `remote_subresources_allowed` | `storage_allowed` | `cookies_allowed` | `intercept_links` |
| --- | --- | --- | --- | --- | --- | --- |
| Gemini capsule | `NativeReader` (Gemini theme) | n/a | n/a | n/a | n/a | true |
| Gemini capsule (user-selected ServoHtml) | `ServoHtml` | false | false | false | false | true |
| HTTP Reader Mode | `ServoHtml` | false | false | false | false | true |
| Markdown doc | `ServoHtml` | false | false | false | false | true |
| Safe preview / untrusted content | `ServoHtml` | false | false | false | false | true |
| Raw web URL | `WryWebview` | true | true | true | true | false |

`NativeReader` does not use `RenderPolicy` fields — it has no script or subresource execution model. Link interception is always active: clicks emit `GraphIntent::NavigateNode`.

`intercept_links = true` means all link navigations are captured by Graphshell and emitted as `GraphIntent::NavigateNode` rather than being followed by the renderer. This is the default for all pipeline-compiled targets.

### 3.3 CSP string generation

For `ServoHtml` targets, the `content_security_policy` field is generated from `RenderPolicy`:

- `scripts_allowed = false` → `script-src 'none'`
- `remote_subresources_allowed = false` → `default-src 'none'; style-src 'unsafe-inline'` (inline styles allowed for block rendering)
- The CSP string is injected as a `<meta http-equiv="Content-Security-Policy">` tag in the compiled HTML. It is not a response header (Servo's in-process HTML load does not use HTTP response headers for injected documents).

---

## 4. Pipeline Structure

The adaptation pipeline runs in the host crate on the I/O task pool (not the frame thread).

```text
ProtocolResolver
    ↓ bytes + MIME
ContentClassifier  (AddressKind + MIME → source type)
    ↓ source type
  [Short-circuit check: edit-intent text/File → viewer:text-editor]
    ↓ if not short-circuited
SimpleDocumentProducer  (source bytes → SimpleDocument)
    ↓ SimpleDocument
EngineTargetCompiler  (SimpleDocument → EngineTarget)
    ↓ EngineTarget
ViewerRegistry  (bind EngineTarget to viewer lifecycle)
```

Each stage is a pure function or async task. No stage mutates graph state directly. Side effects (e.g., setting `mime_hint` after detection) are emitted as `GraphIntent` values from the pipeline coordinator.

---

## 5. Gemini Format Mapping

`text/gemini` line-type to `SimpleBlock` mapping:

| Gemini line prefix | `SimpleBlock` |
| --- | --- |
| `# ` | `Heading { level: 1, text }` |
| `## ` | `Heading { level: 2, text }` |
| `### ` | `Heading { level: 3, text }` |
| `=> URL [label]` | `Link { text: label or URL, href: URL }` |
| `> ` | `Quote(text)` |
| `* ` (list item) | `List { ordered: false, items }` (consecutive `*` lines grouped) |
| ` ``` ` / ` ``` lang` (toggle) | `CodeFence { lang, text }` (content between toggles) |
| (blank line or `---`) | `Rule` |
| (any other line) | `Paragraph(text)` |

Grouping rules: consecutive same-type lines that can be grouped (list items, paragraph text) are merged before producing the block sequence. This is a producer-side responsibility.

---

## 6. HTML Compilation

`SimpleDocument` → `ServoHtml.html` compilation rules:

- Output is a minimal `<!DOCTYPE html><html><head>...</head><body>...</body></html>` document.
- The `<head>` contains the CSP `<meta>` tag and a minimal stylesheet for block layout (inline `<style>`; no external stylesheet).
- Block type to HTML element mapping:
  - `Heading { level, text }` → `<h1>`–`<h3>`
  - `Paragraph(text)` → `<p>`
  - `Link { text, href }` → `<a href="...">` — href is validated against allowed schemes before emission
  - `Quote(text)` → `<blockquote><p>`
  - `CodeFence { lang, text }` → `<pre><code class="language-{lang}">`
  - `List { ordered: false, items }` → `<ul><li>` per item
  - `List { ordered: true, items }` → `<ol><li>` per item
  - `Rule` → `<hr>`
- All text content is HTML-escaped (`<`, `>`, `&`, `"`) before insertion.
- `href` values: only `https://`, `gemini://`, `file://`, and relative paths are passed through. Other schemes are replaced with `about:blank`.

---

## 7. `NativeReader` Rendering

`NativeReader` renders `SimpleDocument` blocks directly via egui without Servo. It is the **default path for Gemini** and the fallback for other adapted sources.

- Implemented in the same module as `PlaintextViewer`, sharing block layout utilities.
- `Heading` blocks use egui `RichText` with scaled font size.
- `Link` blocks render as underlined text; click emits `GraphIntent::NavigateNode`.
- `CodeFence` blocks render inside a `ScrollArea` with monospace font and optional syntax hint label.
- `NativeReader` does not support images, tables, or inline HTML. `SimpleDocument` cannot produce these by construction.

### 7.1 `NativeReaderTheme` contracts

**`Gemini` theme** — minimal, in line with Gemini community conventions:
- Serif or monospace body font (user-configurable in `GraphshellProfile`)
- No decorative chrome, no borders around block elements
- Links rendered inline in body flow, not as a separate link list
- Generous line height (≥ 1.6)
- No images rendered (Gemini has no inline image syntax; external image links remain as clickable links)
- Color scheme respects the app's active egui theme (light/dark)

**`Readable` theme** — clean prose layout for Reader Mode and Markdown:
- Sans-serif body font
- Slightly narrower max-width column (readable line length)
- Subtle heading rules

**`Custom(name)` theme** — user-defined profile stored in `GraphshellProfile` under `native_reader_themes`. Loaded by name at render time; falls back to `Readable` if not found.

Users can create, edit, and name custom themes via the node's **Render With** panel. Theme definitions are stored in the user profile, not per-node — they are reusable across all `NativeReader` nodes.

---

## 7.2 `ServoHtml` CSS Customization

When `EngineTarget::ServoHtml` is selected (either as default for non-Gemini sources, or as a user-selected override for Gemini), the compiled document's `<style>` block includes:

1. **Graphshell base stylesheet** — minimal block layout, font defaults, link color. Kept deliberately sparse.
2. **Source-appropriate preset** — a small per-source-type stylesheet layer (e.g., Gemini preset applies the same minimal conventions as the `Gemini` NativeReader theme, translated to CSS).
3. **User stylesheet** (`user_stylesheet: Option<String>`) — appended last; user rules win via specificity. Empty by default.

The source-appropriate preset for Gemini (`ServoHtml` override case):
- `font-family: serif` body
- `max-width: 70ch; margin: 0 auto` for line length
- `a` color matches the active egui link color
- No `background-image`, no `box-shadow`, no decorative borders

**CSS editor surface**: accessible via the node's **Render With** panel → **Customize stylesheet**. Edits are stored in `GraphshellProfile` keyed by `(source_type, profile_name)`. The user can save named CSS profiles and switch between them, following the same pattern as `NativeReaderTheme::Custom`.

---

## 8. Downstream Feature Dependencies

The following features depend on this spec being implemented before they can proceed:

| Feature | Dependency |
| --- | --- |
| Gemini resolver (`viewer:gemini`, UCM Step 10) | `SimpleDocument` Gemini producer (§5) |
| HTTP Reader Mode (`viewer.profile.reader_toggle`) | `SimpleDocument` reader-mode producer + `ServoHtml` compiler |
| Markdown docs pipeline | `SimpleDocument` Markdown producer + `ServoHtml` or `NativeReader` compiler |
| Safe content preview (untrusted nodes) | `SimpleDocument` sanitized producer + restrictive `RenderPolicy` |

---

## 9. Acceptance Criteria

1. A `gemini://` URL resolves to a `SimpleDocument` and renders via `EngineTarget::NativeReader` with `theme: Gemini` by default. No Servo process is started for a default Gemini load.
2. The `Render With` panel on a Gemini node offers **NativeReader (Gemini)** and **ServoHtml** as backend choices. Selecting ServoHtml renders the capsule via Servo with the Gemini CSS preset and scripts/remote resources blocked.
3. HTTP Reader Mode extracts a readable `SimpleDocument` from an `http://` or `https://` page and renders via `EngineTarget::ServoHtml` without scripts or tracking.
4. A local `text/markdown` file opened read-only renders via `ServoHtml` (preferred) or `NativeReader` (fallback if Servo unavailable).
5. `viewer:text-editor` is never selected by the pipeline for read-only opens. The short-circuit check (§3.1 priority 1) only fires on edit-intent opens.
6. All `Link.href` values in compiled HTML are validated; non-`https`/`gemini`/`file` schemes do not appear in the rendered output.
7. The pipeline does not block the frame thread; it runs on the I/O task pool with a `GraphIntent::UpdateNodeMimeHint` emitted on classification completion.
8. Switching from `NativeReader` to `ServoHtml` (or vice versa) via `Render With` does not recreate the source `SimpleDocument`; the already-computed document is re-compiled to the new target.
9. A user can open **Customize stylesheet** on a `ServoHtml` node, enter CSS, save it as a named profile, and have it persist across sessions and apply to subsequent loads of the same source type.
10. `NativeReaderTheme::Custom` profiles are stored in `GraphshellProfile` and survive app restart.
