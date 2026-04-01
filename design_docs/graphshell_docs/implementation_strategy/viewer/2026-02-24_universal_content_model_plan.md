# Universal Node Content Model: Implementation Strategy

**Date**: 2026-02-24
**Status**: Implementation-Ready
**Relates to**:

- `../technical_architecture/2026-02-18_universal_node_content_model.md` — Research vision document; this plan is the implementation-ready follow-through
- `../graph/node_badge_and_tagging_spec.md` — Badge/tag system; `viewer_id` field and `mime_hint` interplay with badge display and tag assignment UI
- `2026-02-22_registry_layer_plan.md` — `ViewerRegistry` (Phase 2, complete) is the primary contract surface; `ProtocolRegistry` and `KnowledgeRegistry` are prerequisites for Steps 3 and 6
- `2026-02-22_multi_graph_pane_plan.md` — node viewers are pane-hosted view payloads; graph panes remain separate surface types
- `2026-02-23_wry_integration_strategy.md` — Wry backend is a `Viewer` implementation; Steps 1–3 here are prerequisites for the Wry plan
- `2026-03-08_simple_document_engine_target_spec.md` — canonical spec for `SimpleDocument`, `EngineTarget`, `RenderPolicy` (Steps 11–12)
- `2026-02-23_udc_semantic_tagging_plan.md` — UDC semantic tags drive renderer selection hints and the tag badge system
- `2026-02-24_layout_behaviors_plan.md` — Zone attractor and semantic physics extend naturally to content-type clusters

---

## Context

The research vision (`2026-02-18_universal_node_content_model.md`) establishes the core idea: a
Graphshell node is a **persistent, addressable content container**, not a browser tab. The renderer
(Servo webview, PDF viewer, image viewer, text editor) is one way to look at the node — not the
node itself.

This plan translates that vision into an implementation strategy that:

1. Respects the current `ViewerRegistry` contract (Phase 2, complete), which already abstracts
   the viewer surface.
2. Aligns with the two-phase apply model and intent/reducer boundary.
3. Integrates with the badge/tagging system (`viewer_id` is a badge-relevant node property).
4. Does not break the existing Servo webview path — the `ServoViewer` remains the default and
   default fallback throughout all stages.
5. Provides a clear sandboxing story for non-Servo renderers via the existing `wry` feature
   gate model and Cargo feature flags.
6. Fits the pane-hosted multi-view model: node viewers render inside node viewer panes while graph
   panes remain graph projections over the same node data.

---

## The Critical Architectural Distinction

**Servo — texture mode**: renders to GPU surface; Graphshell owns the pixels and can draw them
anywhere in the scene (graph canvas nodes, workbench tiles, thumbnails). Primary backend.

**Wry — overlay mode**: native OS window handle composited above the app surface. Workbench tiles
only. Fully covered in `2026-02-23_wry_integration_strategy.md`.

**Non-web renderers (PDF, image, text, audio)**: render via egui widgets directly into node viewer
panes (and can also provide graph-node previews/thumbnails where appropriate). These are the new
content types this plan introduces.

All three paths implement the `Viewer` trait from the registry layer plan and share the
`render_embedded` / `sync_overlay` / `is_overlay_mode` interface.

---

## What Changes in the Node Data Model

The existing `Node` struct already has `viewer_id_override: Option<ViewerId>` (from the Wry plan).
This plan extends `Node` with:

```rust
/// Optional declared or sniffed MIME type; drives renderer selection.
pub mime_hint: Option<String>,     // e.g. "application/pdf", "image/png"

/// Address type hint (complement to existing URL field).
pub address_kind: AddressKind,
```

**Canonical `AddressKind` definition**: The six-variant enum is defined in
`universal_content_model_spec.md §2.2` (authoritative). The three-variant sketch above is
superseded by the spec. Use the spec's variants: `Http`, `File`, `Data`, `GraphshellClip`,
`Directory`, `Unknown`.

### What does NOT change

- `Node.url: String` — remains the stable address field. `file://` paths use URL encoding.
- `Node.id: Uuid` — stable UUID identity. Already implemented and aligned.
- `Node.viewer_id_override: Option<ViewerId>` — explicit user backend override (from Wry plan).
- Persistence: `fjall` WAL entries for `mime_hint` and `address_kind` follow the same
  `UpdateNodeMetadata` log entry pattern. These are semantic graph facts; they belong in the WAL.
  Both fields are **core types** (live in `graphshell-core`); see spec §9 for the core/host split.

### Why not the full `Address` enum from the research document?

The `Address` enum in the research doc and the core extraction plan (`Http(Url)`, `File(PathBuf)`,
`Onion`, `Ipfs(Cid)`, `Gemini`, `Custom`) is the long-term target for `graphshell-core`. The
current `Node.url: String` + `AddressKind` hint covers the same address space non-breakingly.
Migration to the typed enum is a separate schema change deferred to the core extraction plan
(`2026-03-08_graphshell_core_extraction_plan.md §2.2`).

---

## The Viewer Trait (Current Contract)

**Canonical definition**: `universal_content_model_spec.md §3`. The trait reproduced here in
earlier drafts is superseded; do not redefine it in this plan.

Key points for implementers: non-web renderers (`PdfViewer`, `ImageViewer`, `PlaintextViewer`,
`AudioViewer`) implement `render_embedded` (no return value; renders into the provided rect) and
`is_overlay_mode` returning false. They use `on_attach`/`on_detach`/`on_navigate` for lifecycle.
The old `-> bool` return on `render_embedded` and the `node: &Node` parameter are removed in the
spec; viewers receive node state at `on_attach` time, not per-frame.

---

## ViewerRegistry Selection Policy

Selection order at node open / lifecycle promotion time:

1. `Node.viewer_id_override` — explicit user choice (highest priority; persisted to WAL).
2. Frame `viewer_id_default` — frame-level default (from `FrameManifest`).
3. `ViewerRegistry::select_for(mime: Option<&str>, address_kind: AddressKind)` — highest-priority
   registered viewer where `can_render()` returns true.
   - `viewer:text-editor` registers for editable `text/*` nodes (edit-intent open). It takes
     priority over `viewer:plaintext` for the same MIME types when the node is opened for editing.
     `viewer:plaintext` remains the read-only display path.
   - See `2026-03-08_servo_text_editor_architecture_plan.md` §9 for the editor selection rule.
4. `viewer:webview` — fallback for all `Http` and `File(html)` addresses.
5. `viewer:plaintext` — last resort; always succeeds; shows raw content (read-only).

The MIME detection pipeline runs on first open:

```rust
fn detect_mime(url: &str, content_bytes: Option<&[u8]>) -> Option<String> {
    // 1. Content-Type header (for HTTP responses — provided by Servo resolver)
  // 2. Extension lookup via `mime_guess` crate (cheap, synchronous)
  // 3. Magic bytes via `infer` crate only when extension is missing/ambiguous
    // 4. None — ViewerRegistry falls back to address-kind heuristics
}
```

`infer` and `mime_guess` are already in scope from the research document. `infer` is already
a transitive dependency via `image`; `mime_guess` is already in `Cargo.toml`.

---

## Implementation Plan

### Step 1: Node Data Model Extension

**Goal**: `Node` carries `mime_hint` and `address_kind`. WAL logs both.

- Add `mime_hint: Option<String>` and `address_kind: AddressKind` to `Node` in `graph/node.rs`.
- Add `LogEntry::UpdateNodeMimeHint { node_id: Uuid, mime_hint: Option<String> }` and
  `LogEntry::UpdateNodeAddressKind { node_id: Uuid, kind: AddressKind }` to the WAL schema.
- Add `GraphIntent::UpdateNodeMimeHint` and `GraphIntent::UpdateNodeAddressKind` variants;
  reducer handles both.
- MIME detection: call `detect_mime(url, None)` at node creation time (extension-only pass);
  call with `content_bytes` when content arrives (magic-byte fallback for missing/ambiguous extension cases). Emit `UpdateNodeMimeHint`
  intent on first detection; do not re-emit if already set unless URL changes.
- Add `infer` crate to `Cargo.toml` (pure Rust, 0.9.x, MIT/Apache).

**Done gate**: `cargo test --lib` passes. A node created with `url = "file:///foo.pdf"` has
`mime_hint = Some("application/pdf")` and `address_kind = AddressKind::File` after detection.
A node created with `url = "file:///foo/Documents/"` resolving to a directory has
`address_kind = AddressKind::Directory` (classified via `Path::is_dir()` at address resolution time).
A node created with `url = "https://example.com"` has `address_kind = AddressKind::Http`.

---

### Step 2: ViewerRegistry Selection Policy

**Goal**: `ViewerRegistry` can select a viewer based on MIME hint and address kind.

- Add `fn select_for(&self, mime: Option<&str>, kind: AddressKind) -> ViewerId` to
  `ViewerRegistry` in `registries/domain/viewer.rs`.
- Implement selection order as described above (override → frame default → registry best →
  servo fallback → plaintext fallback).
- Update `lifecycle_reconcile.rs`: when promoting a node to Active, consult `select_for` if no
  `viewer_id_override` is set. The selected `ViewerId` is stored ephemerally (not persisted) for
  the current session; user explicit overrides are persisted via `UpdateNodeViewerPreference`.
- Add contract test: `select_for(Some("application/pdf"), AddressKind::File)` → `viewer:pdf`
  when that viewer is registered; falls back to `viewer:plaintext` when not registered.

**Done gate**: Selection policy is tested. Lifecycle promote for a PDF node picks `viewer:pdf`
when registered and `viewer:webview` otherwise. No regression in existing Servo webview path.

---

### Step 3: PlaintextViewer (Baseline Non-Web Renderer)

**Goal**: A working non-web renderer that proves the Viewer trait contract for embedded content.

`viewer:plaintext` already exists as a seed in `ViewerRegistry`. Implement it fully:

- Accepts all `text/*` MIME types, `application/json`, `application/toml`, `application/yaml`.
  **Read-only display only.** When the same MIME type is opened for editing, `viewer:text-editor`
  takes priority (see ViewerRegistry Selection Policy above).
- Renders content using egui `ScrollArea` + `TextEdit::multiline` (read-only).
- For Markdown (`text/markdown`, `text/x-markdown`): use `egui_commonmark` (third-party crate
  wrapping `pulldown-cmark` for egui, targets egui 0.29+). Verify it builds against egui 0.33
  before adding it; if not updated, write a minimal Markdown renderer using `pulldown-cmark`
  directly (convert to egui `RichText` spans). Do not route Markdown through Servo.
- For syntax highlighting (`text/x-rust`, `text/x-python`, etc.): use `syntect` (MIT, pure Rust,
  5.3.x) **with the `fancy-regex` feature** (pure Rust regex; required for WASM portability —
  the default oniguruma backend is not WASM-safe). Cache the `SyntaxSet` and `ThemeSet` as
  `std::sync::OnceLock` statics — they are expensive to build. Convert `syntect`'s styled ranges
  to `egui::RichText` spans for rendering. The full syntax set adds ~5MB to the binary; mitigate
  by enabling only a curated language subset.
  **Why not `tree-sitter` here**: `tree-sitter` (the Rust crate) does not compile to
  `wasm32-unknown-unknown` (Cranelift transitive dep blocks it). `syntect` with `fancy-regex` is
  the portable choice for the read-only display path. `tree-sitter` is used in `editor-core`
  (desktop-only, non-WASM) for incremental parse/highlight in `viewer:text-editor`.
- Module: `registries/atomic/viewer/plaintext_viewer.rs`.

Add to `Cargo.toml`:

- `syntect = { version = "5", default-features = false, features = ["default-fancy"] }` (pure Rust,
  MIT) — `fancy-regex` backend; WASM-portable. Enable a curated language subset to limit binary size.
- `pulldown-cmark = "0.13"` (pure Rust, MIT) — for Markdown.

**Done gate**: `viewer:plaintext` renders a `.rs` file with syntax highlighting in a workbench
tile. Renders a `.md` file with basic Markdown formatting. No panic on binary files (falls through
to hex display). Contract test for `render_embedded` returning true.

---

### Step 4: ImageViewer

**Goal**: PNG, JPEG, GIF, WebP, BMP, SVG rendered directly in egui tiles without a webview.

`image` crate is already in `Cargo.toml`. Use it for raster formats.

- Module: `registries/atomic/viewer/image_viewer.rs`.
- Decode the image from a `File` address (local filesystem) or from an HTTP response byte stream
  (for `Http` addresses with `image/*` MIME).
- Convert to `egui::ColorImage` and load as a texture via `ctx.load_texture()`.
- Render with `egui::Image` widget inside a `ScrollArea` + zoom controls.
- EXIF orientation: `image` crate handles this automatically with the `jpeg` feature.
- SVG: use `resvg` (pure Rust, MIT, 0.47.x, very active). Rasterize to a `tiny_skia::Pixmap`
  (RGBA byte buffer) via `resvg::render(&tree, transform, &mut pixmap)`, then convert to
  `egui::ColorImage::from_rgba_unmultiplied()` and upload to a `TextureHandle`. No integration
  crate is needed — this is a clean 10-line integration. `resvg` does not support SVG animations
  or `<script>` elements; animated SVGs fall through to the Servo renderer.
  Rasterize at display size, cache the texture. Implement a size-budget LRU cache (target 64MB).
- Thumbnail fallback: when an image node is in graph view (Cold), emit `MarkNodePreviewDirty` on
  first open and let the preview system handle thumbnail generation via the event-driven refresh
  path (`2026-03-05_node_viewport_preview_minimal_slice_plan.md` Slice C). Do not write
  `Node.thumbnail_data` directly — the preview system is the canonical thumbnail authority for
  all viewer types.

Add to `Cargo.toml`:
- `resvg = "0.47"` (pure Rust, MIT) with `tiny-skia` rasterizer.

**Done gate**: A PNG file opened in a workbench tile renders correctly. An SVG file renders via
`resvg`. Zoom works. Thumbnail generated and stored on first open. Contract test for `can_render`
returning true for `image/png`, false for `application/pdf`.

---

### Step 5: PdfViewer

**Goal**: PDF files render in workbench tiles without a webview.

PDF is the one non-pure-Rust renderer in Tier 1 (requires PDFium or MuPDF via C FFI).

**Crate decision**: Use `pdfium-render` (PDFium bindings, MIT). Key properties:
- PDFium ships as a pre-built dynamic library (`pdfium.dll` / `libpdfium.dylib` / `libpdfium.so`).
- `pdfium-render` (0.8.37, MIT) provides a Rust safe API over it.
- On Windows, pre-built PDFium binaries are published weekly by
  [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (x64/x86/arm64).
  A `build.rs` script can download the appropriate binary during CI. The DLL must be co-located
  with the executable or in PATH at runtime.
- Thread safety: PDFium serializes access internally; parallel page rendering is not supported.
  Use a single `Mutex<PdfiumLibrary>` handle shared across all `PdfViewer` instances.
- Licensing: PDFium itself is BSD-3-Clause; `pdfium-render` is MIT.
- **Do not use `mupdf-sys`**: its AGPL-3.0 license requires open-sourcing the entire application
  or purchasing a commercial Artifex license. Avoid.

Feature gate this behind `--features pdf`. Default: off. The Verso mod `ModManifest` declares
`feature:pdf` in `requires` — if the feature is off, `viewer:pdf` simply is not registered.

Implementation:
- Module: `registries/atomic/viewer/pdf_viewer.rs` under `#[cfg(feature = "pdf")]`.
- Hold a `PdfiumLibrary` handle (thread-safe wrapper) and a `HashMap<NodeKey, PdfDocument>` of
  open documents.
- `render_embedded`: render the current page to an egui texture; provide prev/next page controls.
- Page caching: cache the current page texture; regenerate on page change or zoom change.
- Search: `PdfDocument::search()` returns text matches; highlight with egui `Shape::Rect`.
- Security: PDFium runs in-process. For now, this is acceptable for a desktop app with
  user-sourced files. Sandboxed out-of-process PDF rendering is a Tier 3 concern.

**Done gate**: `cargo build --features pdf` compiles. A `.pdf` file opened in a workbench tile
shows page 1. Prev/Next page controls work. `cargo build` (without `--features pdf`) compiles
clean. Contract test for `can_render` gated by feature flag.

---

### Step 6: DirectoryViewer (Browse-in-Place)

**Goal**: `file://` addresses that point to directories render as a navigable file listing inside
the current tile (browse-in-place), without creating new graph nodes.

**Scope note**: This viewer handles *in-tile local navigation only*. Bulk import of a directory
into graph nodes (files → nodes, folders → frames) is a separate feature covered by
`2026-03-02_filesystem_ingest_graph_mapping_plan.md`, which gates on this viewer being active.

This is pure Rust using `std::fs` — no new crates required.

- Module: `registries/atomic/viewer/directory_viewer.rs`.
- `can_render(address_kind: AddressKind::Directory, mime: None)` — returns true unconditionally
  for `Directory` address kind. No `Path::is_dir()` check needed in the viewer; address
  classification (Step 1) is where the `is_dir()` check lives and sets `address_kind = Directory`.
- Render a two-column table (name + size/type) inside a `ScrollArea`.
- Click on a file: emit `GraphIntent::NavigateNode { node_key, url: file_url }` — navigates the
  *current tile* to that file (viewer swaps to the appropriate viewer for that file type). This
  is browse-in-place: no new graph node is created. Use this for exploration within a single node.
- Click on a directory: emit `GraphIntent::NavigateNode` with the directory URL — navigates in.
- Drag a file to graph canvas: emit `GraphIntent::CreateNode { url: file_url }` — creates a new
  graph node for that file (the explicit import gesture).
- Breadcrumb navigation: maintain a `Vec<PathBuf>` history in `DirectoryViewerState` per node.

`address_kind = AddressKind::Directory` is set by the URL detection in Step 1 (via `Path::is_dir()`
on the resolved path). `AddressKind::File` is reserved for file addresses only.

**Done gate**: Opening `file:///C:/Users/foo/Documents/` in a workbench tile shows a file listing.
Clicking a `.pdf` file navigates the tile to show the PDF via `PdfViewer`. Clicking up (..)
navigates to the parent directory. No panic on symlinks or permission-denied paths.

---

### Step 7: AudioViewer (Media Playback)

**Goal**: Audio files play directly in a workbench tile with waveform + controls.

Feature gate: `--features audio`. Default: off.

- Module: `registries/atomic/viewer/audio_viewer.rs` under `#[cfg(feature = "audio")]`.
- Decode with `symphonia` (pure Rust, MPL-2.0, covers MP3, FLAC, OGG, WAV, AAC, M4A).
- Play via `rodio` (pure Rust, Apache-2.0, uses `cpal` for OS audio).
- UI: egui-based waveform (simple amplitude bar chart from decoded samples), playback position
  scrubber, play/pause/stop buttons, volume slider.
- Waveform: decode all samples on open (async thread), store as `Vec<f32>`, draw via
  `egui::Shape::line()` or a simple bar chart.

Add to `Cargo.toml` under `cfg(feature = "audio")`:
- `symphonia = { version = "0.5", features = ["mp3", "flac", "ogg", "wav"] }` (MPL-2.0, pure Rust)
- `rodio = "0.19"` (Apache-2.0, pure Rust)

**Done gate**: `cargo build --features audio` compiles. An `.mp3` file plays in a workbench tile
with visible playback controls. `cargo build` (without `--features audio`) compiles clean.

---

### Step 8: Badge and Tag Integration

**Goal**: `mime_hint` and `address_kind` inform badge rendering and tag suggestions.

This step integrates with `../graph/node_badge_and_tagging_spec.md`:

- **Address-kind badge**: Add a `Badge::ContentType(ViewerId)` variant. When a node's
  viewer is not `viewer:webview` (the default), show a small icon badge indicating the content type:
  - `viewer:pdf` → 📄 PDF badge
  - `viewer:image` → 🖼 Image badge
  - `viewer:plaintext` → 📝 Text badge
  - `viewer:audio` → 🎵 Audio badge
  - `viewer:directory` → 📁 Directory badge
  This makes non-web nodes visually distinct in the graph view.

- **Tag suggestions from MIME**: When the tag assignment panel opens for a node with a known
  `mime_hint`, pre-populate the suggestions with relevant UDC codes from `KnowledgeRegistry`.
  Example: `mime: application/pdf` → suggest `udc:002` (Bibliography, reference; documents).

- **Clipboard-kind node shape**: The `#clip` reserved tag already marks DOM-extracted nodes
  (from the clipping plan). The `address_kind` field complements this — file and audio nodes
  get distinct node shapes in the graph canvas alongside the clip shape.

**Done gate**: A PDF node in graph view shows a 📄 badge. An image node shows a 🖼 badge.
Badge ordering follows the priority table from the badge plan (ContentType lower priority than
Pinned/Starred, but visible in expanded orbit).

**Render mode note**: `ContentType` badges for native viewers (`viewer:pdf`, `viewer:image`,
`viewer:plaintext`, `viewer:audio`, `viewer:directory`) render in the standard overlay pass —
these viewers use `placeholder`/`thumbnail` render mode in graph view, so standard Graphshell
overlays are always permitted over them. See `2026-02-26_composited_viewer_pass_contract.md`
§Affordance Policy for the full render-mode/overlay permission table.

---

### Step 9: Security and Sandboxing Model

**Goal**: Define the permission model for non-HTTP content types.

**Prerequisite note**: `FilePermissionGuard` (defined below) is a hard prerequisite for the
filesystem ingest feature (`2026-03-02_filesystem_ingest_graph_mapping_plan.md`). Step 9 must
reach its done gate before filesystem ingest Phase 1 can close.

All new viewers access content through the existing `file://` URL routing path or via Servo's
net layer. The permission model follows the research document's table:

| Address / Viewer | Permission | Default |
| ---------------- | ---------- | ------- |
| `Http`/`Https` (Servo) | Network access | Granted |
| `File` (any viewer) | Filesystem read | Prompt on first `file://` access outside home dir |
| `viewer:pdf` | Filesystem read (via File permission) | Same as File |
| `viewer:audio` | Filesystem read (via File permission) | Same as File |
| `viewer:wry` | Network access (native webview) | Granted; feature-gated |

Sandboxing for renderers:

- **In-process renderers** (`PdfViewer`, `ImageViewer`, `TextViewer`, `AudioViewer`): run in the
  main process. PDFium parses user-selected files; the threat model is user-sourced content, not
  adversarial network content. Acceptable for desktop app context.
- **WASM mods** (future): The `extism` (1.13.0, BSD-3-Clause) plugin system provides WASM
  sandboxing via Wasmtime (production-hardened, WASI capability model). Plugins have no filesystem
  or network access unless the host explicitly grants it. Version 1.x stability guarantee and
  multi-language PDK support make it production-ready for the mod sandbox. This is a Tier 2
  concern (after the `ModRegistry` WASM tier is implemented in the registry layer plan).
  Native mods (compiled-in via `inventory::submit!`) are trusted by definition.
- **File access guard**: Add a `FilePermissionGuard` struct to the registry infrastructure.
  Before any viewer opens a `file://` address, check if the path is within:
  - The user's home directory: auto-allow.
  - A user-configured allow-list: auto-allow.
  - Outside both: prompt once per directory, store decision in `AppPreferences`.
- **No network access from non-Servo viewers**: `PdfViewer`, `ImageViewer`, `TextViewer` must
  not initiate network requests. They receive content as bytes from the resolver layer or read
  from local paths. The resolver is the network boundary; viewers are pure renderers.

**Done gate**: Opening a file outside the home directory triggers a permission prompt (or
auto-denies if `AppPreferences.file_access_policy == Deny`). Contract test for
`FilePermissionGuard::check()`.

---

### Step 10: Gemini Protocol Resolver (Optional Extension)

**Goal**: `gemini://` URLs resolve and render natively in a `TextViewer`-based renderer.

This step is explicitly optional and feature-gated. The research document notes that a Gemini
resolver is ~500 LOC in pure Rust.

Feature gate: `--features gemini`. Default: off.

- Module: `mods/native/verso/gemini_resolver.rs` under `#[cfg(feature = "gemini")]`.
- The Gemini protocol uses a simple TLS-based text protocol (no HTTP). Connect via
  `rustls` (already in `Cargo.toml`) to the Gemini server, send `URL\r\n`, receive response.
- Response header: `<status> <meta>\r\n`. Success = status 2x; content type in meta.
- Render: Gemini markup (`text/gemini`) is a simple line-based format. Implement a
  `GeminiRenderer` that extends `PlaintextViewer` with Gemini-specific rendering (headers, links,
  preformatted blocks).
- Register `gemini` scheme in `ProtocolRegistry`; register `viewer:gemini` in `ViewerRegistry`.
- Privacy: Gemini servers do not support cookies or tracking pixels. The resolver must not send
  HTTP headers or user-agent strings beyond what Gemini requires (none).

**Done gate**: `cargo build --features gemini` compiles. `gemini://gemini.circumlunar.space/`
renders the Gemini capsule's index page as styled text. Links are clickable and navigate the node.

---

### Step 11: Explicit Render Selection + Shared Viewer Profiles

**Goal**: Users can choose rendering path directly (`Servo`, `Wry`, `Viewer`) without one path
being gated behind another, and Reader behavior is treated as a Viewer profile that can target
Servo.

#### 11.1 User-facing render selection policy

- Add a context/action surface entry:
  - `Render With -> Servo | Wry | Viewer`
- The currently active renderer is shown as selected/disabled in the menu.
- All available renderers for the current node are directly selectable; fallback suggestions are
  advisory only.
- If a renderer is unavailable (feature gate off or capability missing), show it disabled with a
  reason label.

Suggested command IDs (ActionRegistry-facing):

- `viewer.render_with_servo`
- `viewer.render_with_wry`
- `viewer.render_with_viewer`
- `viewer.profile.reader_toggle` (omnibar/button action on Servo tiles)

#### 11.2 Shared Simple Document Model (`Viewer` profiles)

Define a format-agnostic intermediate model used by Viewer profiles:

```rust
enum SimpleDocument {
    Blocks(Vec<SimpleBlock>),
}

enum SimpleBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    Link { text: String, href: String },
    Quote(String),
    CodeFence { lang: Option<String>, text: String },
    List { ordered: bool, items: Vec<String> },
    Rule,
}
```

Producers into this model:

- `GeminiRenderer` path (`text/gemini` -> `SimpleDocument`)
- HTTP reader transform path (`http/https` readable extraction -> `SimpleDocument`)

Consumer:

- profile compiler to `EngineTarget::ServoHtml` (primary)
- optional `NativeReader` target for deterministic low-surface fallback

This is not a new browser engine. It is a shared text-first presentation layer.

#### 11.3 Relationship to Servo/Wry

- `Servo` is the primary engine path for Viewer profiles.
- `Wry` remains compatibility-only native webview and does not ingest Viewer profile targets.
- `Viewer` owns content/policy profiles (Reader, Media, future profiles) and compiles targets for
  Servo rendering where possible.

#### 11.4 Implementation notes

- Do not require Wry for Viewer profiles; Reader profile must function with Servo-only builds.
- Prefer reusing existing `PlaintextViewer`-adjacent rendering code for block layout and link
  interaction instead of introducing a separate graphics renderer.
- Preserve node identity while switching renderer (same node address, different viewer binding).

**Done gate**:

- Context/action menu exposes `Render With -> Servo | Wry | Viewer` with current mode indicator.
- A node can switch between renderers without reopening or recreating the node.
- Gemini and HTTP Reader Mode both render through Viewer profile compilation using one shared
  document model.
- Viewer profile mode compiles and runs when `wry` is disabled.

---

### Step 12: Servo-First Content Adaptation Pipeline (Generic)

**Goal**: Maximize Servo coverage by routing non-HTML and simplified content through a generic
adaptation pipeline that can produce Servo-renderable targets without requiring deep Servo-core
modifications.

This step formalizes a host-owned adaptation layer in Graphshell. Servo remains the primary full
renderer; Graphshell compiles content into render targets that Servo can consume.

#### 12.1 Pipeline structure

1. Resolve source content via `ProtocolResolver` (`http`, `https`, `gemini`, `file`, future schemes).
2. Classify source (`AddressKind`, MIME/content-type, confidence).
3. **Short-circuit check**: if `address_kind = File` and `mime_hint` is in the `text/*` family and
   the node is opened for editing, route directly to `viewer:text-editor` — do **not** run the
   adaptation pipeline. Editing semantics are owned by `editor-core`, not `SimpleDocument`.
   See `2026-03-08_servo_text_editor_architecture_plan.md` §9.
4. Adapt source into a normalized intermediate model (`SimpleDocument` or equivalent).
5. Compile intermediate model into an engine target package.
6. Bind selected path (`Servo`, `Wry`, `Viewer`) from explicit user command.
7. Enforce per-target policy (CSP, script/network/storage constraints, link interception).

#### 12.2 Engine target contract

```rust
enum EngineTarget {
  ServoHtml {
    html: String,
    base_url: Option<String>,
    content_security_policy: String,
    policy: RenderPolicy,
  },
  WryWebview {
    source_url: String,
  },
  NativeReader {
    doc: SimpleDocument,
    policy: RenderPolicy,
  },
}

struct RenderPolicy {
  scripts_allowed: bool,
  remote_subresources_allowed: bool,
  storage_allowed: bool,
  cookies_allowed: bool,
  intercept_links: bool,
}
```

`ServoHtml` is the key cross-applicability path: Graphshell can transform Gemini or readerized
HTTP content into constrained HTML and render it through Servo's existing pipeline.

#### 12.3 Servo-first policy

- Default renderer remains Servo when available.
- `Viewer` and `Wry` are explicit selectable alternatives, not hidden fallback-only paths.
- For simplified/profiled content, Graphshell should first attempt `EngineTarget::ServoHtml`.
- `Wry` escalation is compatibility fallback for eligible web-source nodes, not a universal
  target for Viewer profile outputs.
- Keep a native-reader target available for deterministic low-surface rendering when needed.

#### 12.4 Five high-value uses for this pipeline

1. Gemini capsule rendering through Servo via constrained HTML compilation.
2. HTTP reader mode for complex/broken pages without immediate renderer switch.
3. Markdown/docs and note content rendered through one unified simple presentation contract.
4. Safe preview mode for untrusted content (sanitized output + restrictive policy envelope).
5. Snapshot/archive replay from normalized content independent of live site behavior.

#### 12.5 Servo evaluation gates ("get the most out of Servo")

Before recommending `Render With -> Wry`, record whether Servo target path succeeded under policy,
and ensure the node is eligible for webview compatibility fallback:

- did `ServoHtml` render successfully,
- did navigation/link interception behave correctly,
- did policy constraints hold (no script/network escape),
- was output readable/usable by user criteria.

This turns renderer switching into evidence-based selection rather than premature engine bypass.

**Done gate**:

- `EngineTarget` contract is documented and used by at least one non-HTML adapter path.
- Gemini or readerized HTTP content can render through Servo without Servo-core protocol changes.
- Diagnostics can report target type and policy mode for rendered nodes.
- Renderer switch recommendations are backed by recorded Servo-path outcome data and explicit
  webview-fallback eligibility.

---

### Step 13: Servo Capability Packs (Built-In Modes + Mod-Provided Support)

**Goal**: Keep Servo as the primary rendering path while allowing user-selectable content
encapsulations and mod-provided capability packs (codecs, backend options, render profiles,
format adapters) to expand what Servo can handle.

#### 13.1 Built-in mode model

Two mode families are built in and always user-visible:

- **Reader Mode**: text/document-first rendering contracts for `gemini`, `simplehtml`, `txt`,
  `markdown`, `pdf`-derived document projections, and other HTML-formattable document inputs.
- **Media Mode**: structured rendering contracts for image/video/audio-centric content.

Both modes compile content into Servo-compatible targets where possible (`EngineTarget::ServoHtml`)
and remain selectable directly through `Render With` commands.

#### 13.2 Capability packs as mods

Capability packs are mod contributions that do not replace built-in mode semantics; they extend
support depth:

- codec packs (decode/transcode capabilities),
- parser/transform packs (format-specific adapters into `SimpleDocument` or media descriptors),
- render profile packs (policy presets and style/layout templates),
- backend support packs (feature-gated integration toggles where host policy allows).

Pack contribution model:

- Built-in mode contracts remain canonical (`Reader`, `Media`).
- Packs register declared capabilities through registry contracts.
- Selection UI exposes packs as options/profiles under mode families, not as opaque renderer forks.

#### 13.3 User control contract

- User can always choose rendering mode directly: `Render With -> Servo | Wry | Viewer`.
- Within `Reader` and `Media`, user can choose available capability profiles/packs.
- Current mode/profile is visible; unavailable options are disabled with reason.

Wry policy note:

- Wry does not consume Viewer profile targets; it remains a compatibility webview path.

This keeps user agency explicit and avoids hidden fallback-only behavior.

#### 13.4 Registry integration

Viewer and protocol registries remain authorities:

- `ProtocolRegistry` resolves scheme/source content.
- adaptation layer compiles source into engine targets.
- `ViewerRegistry` selects/dispatches renderer + mode/profile binding.

Packs declare compatibility and conformance before they become selectable.

#### 13.5 Crate-first implementation guidance

Prefer crate composition over custom engine construction:

- parse/transform using ecosystem crates,
- compile to constrained Servo HTML targets,
- keep host policy envelope explicit (CSP, script/storage/network rules),
- introduce bespoke rendering only when crate-based paths cannot satisfy required behavior.

This approach maximizes Servo utilization while minimizing duplicate rendering stacks.

**Done gate**:

- Built-in `Reader` and `Media` mode families are defined and selectable.
- At least one capability pack extends Reader and one extends Media via registry declarations.
- Pack profile selection appears in UI with deterministic availability/diagnostics.
- Servo remains the default path for supported pack outputs.

---

### Step 14: Servo Adoption Policy (Evidence-Gated, Non-Dogmatic)

**Goal**: Use Servo aggressively where it is technically superior, and avoid forcing Servo into
formats/workflows where it is lower fidelity, less efficient, or operationally higher risk.

This step codifies a hard decision rule: Servo-first by default, but only kept as the chosen path
when evidence supports it.

#### 14.1 Four-gate decision scorecard

Every candidate content path should be scored across four gates:

1. **Fidelity**: semantic and visual correctness versus user expectation for the content type.
2. **Efficiency**: runtime cost (startup, frame time, memory footprint, decode/render overhead).
3. **Complexity**: implementation and maintenance burden compared to alternatives.
4. **Reliability**: failure modes, crash/fallback frequency, and cross-platform consistency.

Decision rule:

- If Servo passes all four gates (or ties best option), keep Servo as primary.
- If Servo fails two or more gates, do not force Servo as primary for that content type.
- If Servo fails one gate narrowly, keep Servo path behind a profile/flag until metrics improve.

#### 14.2 Two Servo usage classes

Use Servo in one of these two explicit classes:

1. **Servo as renderer**:
  - content compiled/adapted to Servo-native web output (`EngineTarget::ServoHtml`),
  - Servo performs primary rendering work.
2. **Servo as presentation shell**:
  - external parser/codec/backend performs heavy lifting,
  - Graphshell wraps results into Servo-presented HTML UI surfaces.

Both are valid. Selection depends on scorecard outcomes, not ideology.

#### 14.3 Viewer policy by outcome

- **Promote Servo path** when scorecard is green and diagnostics confirm stable runtime behavior.
- **Keep dual path** (Servo + native path) when results are close or platform variability is high.
- **Prefer non-Servo primary** when Servo evidence is materially worse and no short-term mitigation
  exists.

Wry policy remains unchanged:

- Wry is compatibility webview fallback for eligible web-source nodes.
- Wry is not a universal Viewer-profile ingest target.

#### 14.4 Required diagnostics evidence

Before declaring Servo primary for a content/profile path, collect at minimum:

- render success rate,
- fallback frequency,
- median and p95 frame/render latency,
- memory delta versus alternate path,
- user-visible fidelity regressions count.

Recommendation logic must cite these metrics in lane receipts/issues.

#### 14.5 Initial target guidance

Likely strong Servo candidates:

- readerized HTTP documents,
- Gemini/Text/Markdown-derived document profiles,
- HTML-wrapped presentation of structured outputs.

Likely conditional (measure first):

- heavy media codec workflows,
- PDF primary rendering paths where external engines may remain superior.

**Done gate**:

- Scorecard table exists for initial target formats/profiles.
- At least one profile is promoted to Servo-primary with recorded diagnostics evidence.
- At least one profile remains dual-path or non-Servo-primary with documented justification.
- Viewer policy decisions reference measurable gate outcomes, not preference-only rationale.

---

## Crate Summary

| Crate | Version | Feature | Purpose | License | Notes |
| ----- | ------- | ------- | ------- | ------- | ----- |
| `infer` | 0.19 | default | MIME magic-byte detection | MIT | Pure Rust; transitive dep via `image` |
| `mime_guess` | 2.0.5 | default | MIME extension detection | MIT | Already in `Cargo.toml` |
| `syntect` | 5.3 | default | Syntax highlighting (`viewer:plaintext` read-only) | MIT | Use `fancy-regex` feature; WASM-portable. `tree-sitter` used in `editor-core` (desktop-only) for `viewer:text-editor`. |
| `pulldown-cmark` | 0.13 | default | Markdown parsing | MIT | Pure Rust; use with `egui_commonmark` |
| `resvg` | 0.47 | default | SVG rasterization | MIT | Pure Rust; `tiny-skia`; render to `Pixmap` |
| `pdfium-render` | 0.8.37 | `pdf` | PDF rendering | MIT | PDFium C FFI; bundle DLL from bblanchon |
| `symphonia` | 0.5.5 | `audio` | Audio decoding | MPL-2.0 | Pure Rust; MP3/FLAC/OGG/WAV/AAC |
| `rodio` | 0.22 | `audio` | Audio playback | Apache-2.0 | Pure Rust; WASAPI on Windows |

**Crates to avoid**:

- `mupdf-sys` — AGPL-3.0; requires licensing the entire application or commercial Artifex license.
- `wry` — winit 0.30 version matches, but wry cannot share the egui/eframe event loop. On Linux,
  WebKitGTK requires a GTK event loop that is incompatible with eframe's winit loop. On Windows,
  child-HWND embedding into eframe's window is feasible but requires unsafe Win32 code. The Wry
  integration plan (`2026-02-23_wry_integration_strategy.md`) is Windows-first for exactly this
  reason; the universal content model does not depend on wry.

---

## Feature Flag Summary

```toml
[features]
default = ["gamepad", "servo/clipboard", "js_jit", "max_log_level", "webgpu", "webxr", "diagnostics"]
pdf = ["dep:pdfium-render"]
audio = ["dep:symphonia", "dep:rodio"]
gemini = []
wry = ["dep:wry"]                 # from wry integration plan
```

Tier 1 renderers (`PlaintextViewer`, `ImageViewer`, `DirectoryViewer`) are always compiled in —
they require only already-in-`Cargo.toml` crates (`image`, `mime_guess`). `syntect` and
`pulldown-cmark` and `resvg` are small enough to include unconditionally.

---

## Relationship to Other Plans

| Plan | How It Connects |
| ---- | --------------- |
| `2026-02-23_wry_integration_strategy.md` | Steps 1–2 here are prerequisites; `Viewer` trait is shared |
| `../graph/node_badge_and_tagging_spec.md` | Step 8 extends badge system with `ContentType` badge |
| `2026-02-22_registry_layer_plan.md` | `ViewerRegistry` (Phase 2) is the contract surface; `ProtocolRegistry` is the resolver surface |
| `2026-02-23_udc_semantic_tagging_plan.md` | MIME hints drive UDC tag suggestions; semantic physics clusters by content type |
| `2026-02-24_layout_behaviors_plan.md` | Zone attractors can group PDF nodes together, image nodes together, etc. |
| `2026-02-21_lifecycle_intent_model.md` | Lifecycle promotion/demotion applies to all viewer types; `lifecycle_reconcile.rs` must dispatch to the correct viewer |
| `2026-03-08_servo_text_editor_architecture_plan.md` | `viewer:text-editor` takes priority over `viewer:plaintext` for editable `text/*` nodes; Step 12 pipeline short-circuits to it; `tree-sitter` (desktop-only) vs `syntect`/`fancy-regex` (WASM-portable) split originates here |
| `2026-03-02_filesystem_ingest_graph_mapping_plan.md` | Step 6 (`DirectoryViewer`) is a browse-in-place prerequisite for ingest; Step 9 (`FilePermissionGuard`) is a hard ingest gate |
| `2026-03-05_node_viewport_preview_minimal_slice_plan.md` | Step 4 (`ImageViewer`) defers thumbnail generation to the preview system via `MarkNodePreviewDirty` |
| `2026-02-26_composited_viewer_pass_contract.md` | Step 8 badges render in overlay pass; native viewers use `placeholder` render mode — standard overlays always permitted |

---

## Execution Order

Steps have the following dependencies:

```
Step 1 (Node Data Model) → Step 2 (ViewerRegistry Selection)
Step 2 → Step 3 (PlaintextViewer)   [proves trait; baseline]
Step 2 → Step 4 (ImageViewer)       [parallel with Step 3]
Step 2 → Step 6 (DirectoryViewer)   [parallel; no new deps]
Step 3 → Step 5 (PdfViewer)         [Viewer trait must work first]
Step 4 → Step 5                     [texture pipeline already proven]
Steps 3–6 → Step 8 (Badge Integration)
Step 1 → Step 9 (Security Model)    [file permission guard at Node creation]
Step 3 → Step 7 (AudioViewer)       [optional; parallel]
Step 3 → Step 10 (Gemini)           [optional; parallel]
Step 2 + Step 3 + Step 10 → Step 11 (Render selection + Reader shared model)
Step 11 + Step 10 → Step 12 (Servo-first adaptation pipeline)
Step 12 + registry capability wiring → Step 13 (Servo capability packs)
Step 12 + Step 13 + diagnostics instrumentation → Step 14 (Servo adoption policy)
```

Recommended implementation sequence: 1 → 2 → 3 → 4 → 6 → 9 → 8 → 5 → 7 → 10 → 11 → 12 → 13 → 14.

Step 9 is moved before Step 8 because `FilePermissionGuard` is a hard prerequisite for the
filesystem ingest feature gate (`2026-03-02_filesystem_ingest_graph_mapping_plan.md`).

Steps 5, 7, and 10 are feature-gated and can be deferred or skipped without blocking the rest.
Step 11 can ship incrementally: first command/menu wiring, then shared `SimpleDocument` adapters.
Step 12 can ship incrementally: first `EngineTarget` contract + diagnostics labels, then adapter
coverage expansion (Gemini, HTTP reader, markdown/docs).
Step 13 can ship incrementally: first profile declaration/selection plumbing, then codec/parser
pack coverage expansion.
Step 14 can ship incrementally: scorecard templates first, then evidence-backed profile decisions
as diagnostics data accumulates.

---

## Risks and Mitigations

**PDFium binary distribution**: PDFium requires a native shared library (`pdfium.dll` on Windows,
`libpdfium.dylib` on macOS, `libpdfium.so` on Linux). Source these from `bblanchon/pdfium-binaries`
in CI (automated weekly builds, all platforms). The DLL must be included in the release package.
Do not use `mupdf-sys` as a substitute — its AGPL-3.0 license is a hard blocker.
If distribution complexity exceeds acceptable bounds, defer `PdfViewer` to a later phase and
route `.pdf` files through the Servo renderer (which handles PDFs via its built-in PDF.js path).

**Audio on Windows**: `rodio` uses `cpal` which uses WASAPI on Windows. This should work without
system dependencies; verify on the CI target before enabling the `audio` feature by default.

**`resvg` and egui texture cache**: SVG files rasterized at large sizes create large textures.
Implement a texture cache with a size budget (e.g., 64MB) and eviction policy (LRU).
The existing `Node.thumbnail_data` pipeline is a reference for this pattern.

**Viewer state per node**: `DirectoryViewer` needs per-node state (current path, scroll offset).
This state is ephemeral (not persisted). Use `HashMap<NodeKey, DirectoryViewerState>` inside the
`DirectoryViewer` struct, cleaned up when nodes are demoted to Cold.

**`syntect` compile time**: `syntect` compiles the full syntax set (400+ languages) at build time.
This adds ~5 seconds to clean build time. Mitigate by enabling only a curated subset of
languages, or use `syntect`'s `PACKS` feature to load syntax definitions at runtime.

---

## Findings

The Universal Node Content Model is architecturally natural within the existing registry and
Viewer trait infrastructure. The key insight from the research document holds: the `ViewerRegistry`
contract (`render_embedded` / `sync_overlay` / `is_overlay_mode`) already provides the right
abstraction surface. Adding new renderer types is additive — no existing code changes.

The most significant implementation friction is the PDFium binary distribution story and audio
subsystem integration. Both are contained behind feature flags. All Tier 1 renderers
(`PlaintextViewer`, `ImageViewer`, `DirectoryViewer`) are pure Rust and unconditionally compiled,
making the common case (text, images, local files) available without configuration.

The `AddressKind` field on `Node` is intentionally minimal — it is a hint for renderer selection,
not a full-blown address type system. The migration to a typed `Address` enum remains a long-term
option when IPFS, Gemini, and Tor resolvers are prioritized.

---

## Progress

### 2026-02-24

- Plan created. Research vision (`2026-02-18_universal_node_content_model.md`) and the badge/tagging
  interaction contract (`../graph/node_badge_and_tagging_spec.md`) synthesized into this implementation plan.
- `Viewer` trait from `2026-02-23_wry_integration_strategy.md` adopted as the shared contract.
- Ten implementation steps defined with done gates.
- Feature flag strategy aligned with existing Cargo.toml conventions.
- Crate selection finalized for Tier 1 (unconditional) and Tier 2 (feature-gated).
- Security/sandboxing model documented (in-process, file permission guard, no network from
  non-Servo viewers).
- Execution order dependency graph documented.
