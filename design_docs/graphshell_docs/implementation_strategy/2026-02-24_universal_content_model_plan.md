# Universal Node Content Model: Implementation Strategy

**Date**: 2026-02-24
**Status**: Implementation-Ready
**Relates to**:

- `../technical_architecture/2026-02-18_universal_node_content_model.md` — Research vision document; this plan is the implementation-ready follow-through
- `2026-02-20_node_badge_and_tagging_plan.md` — Badge/tag system; `viewer_id` field and `mime_hint` interplay with badge display and tag assignment UI
- `2026-02-22_registry_layer_plan.md` — `ViewerRegistry` (Phase 2, complete) is the primary contract surface; `ProtocolRegistry` and `KnowledgeRegistry` are prerequisites for Steps 3 and 6
- `2026-02-23_wry_integration_strategy.md` — Wry backend is a `Viewer` implementation; Steps 1–3 here are prerequisites for the Wry plan
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

---

## The Critical Architectural Distinction

**Servo — texture mode**: renders to GPU surface; Graphshell owns the pixels and can draw them
anywhere in the scene (graph canvas nodes, workbench tiles, thumbnails). Primary backend.

**Wry — overlay mode**: native OS window handle composited above the app surface. Workbench tiles
only. Fully covered in `2026-02-23_wry_integration_strategy.md`.

**Non-web renderers (PDF, image, text, audio)**: render via egui widgets directly into any tile
pane or graph node view. These are the new content types this plan introduces.

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

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AddressKind {
    Http,      // served over HTTP/HTTPS — default, Servo renders
    File,      // local filesystem path (file:// or PathBuf)
    Custom,    // any other scheme; renderer selected by ViewerRegistry
}
```

**What does NOT change:**

- `Node.url: String` — remains the stable address field. `file://` paths use URL encoding.
- `Node.id: Uuid` — stable UUID identity. Already implemented and aligned.
- `Node.viewer_id_override: Option<ViewerId>` — explicit user backend override (from Wry plan).
- Persistence: `fjall` WAL entries for `mime_hint` and `address_kind` follow the same
  `UpdateNodeMetadata` log entry pattern. These are semantic graph facts; they belong in the WAL.

**Why not the full `Address` enum from the research document?**

The `Address` enum in the research doc (`Http`, `File`, `Onion`, `Ipfs`, `Gemini`, `Dat`,
`Custom`) is the long-term vision. The current `Node.url: String` already covers the same address
space with URL encoding. Adding `AddressKind` as a hint is additive and non-breaking; migrating
to a typed `Address` enum is a schema migration that warrants its own plan when IPFS/Gemini/Tor
resolvers become implementation priorities. This plan does not attempt that migration.

---

## The Viewer Trait (Current Contract)

The `Viewer` trait was extended in `2026-02-23_wry_integration_strategy.md`. All new renderer
implementations use this interface:

```rust
pub trait Viewer {
    /// Render content into an egui Ui region (texture mode).
    /// Returns true if the viewer handled rendering, false if it requires overlay mode.
    fn render_embedded(&mut self, ui: &mut egui::Ui, node: &Node) -> bool;

    /// Synchronize overlay position and visibility (overlay mode).
    /// Called by TileCompositor after layout is computed for overlay-backed tiles.
    fn sync_overlay(&mut self, rect: egui::Rect, visible: bool);

    /// Returns true if this viewer requires overlay mode (cannot render embedded).
    fn is_overlay_mode(&self) -> bool { false }
}
```

Non-web renderers (`PdfViewer`, `ImageViewer`, `TextViewer`, `AudioViewer`) all implement
`render_embedded` returning true and `is_overlay_mode` returning false. They are purely
egui-widget-based and never require overlay mode.

---

## ViewerRegistry Selection Policy

Selection order at node open / lifecycle promotion time:

1. `Node.viewer_id_override` — explicit user choice (highest priority; persisted to WAL).
2. Workspace `viewer_id_default` — workspace-level default (from `WorkspaceManifest`).
3. `ViewerRegistry::select_for(mime: Option<&str>, address_kind: AddressKind)` — highest-priority
   registered viewer where `can_render()` returns true.
4. `viewer:servo` — fallback for all `Http` and `File(html)` addresses.
5. `viewer:plaintext` — last resort; always succeeds; shows raw content.

The MIME detection pipeline runs on first open:

```rust
fn detect_mime(url: &str, content_bytes: Option<&[u8]>) -> Option<String> {
    // 1. Content-Type header (for HTTP responses — provided by Servo resolver)
    // 2. Magic bytes via `infer` crate (most reliable for local files)
    // 3. Extension fallback via `mime_guess` crate
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
  call with `content_bytes` when content arrives (magic-byte pass). Emit `UpdateNodeMimeHint`
  intent on first detection; do not re-emit if already set unless URL changes.
- Add `infer` crate to `Cargo.toml` (pure Rust, 0.9.x, MIT/Apache).

**Done gate**: `cargo test --lib` passes. A node created with `url = "file:///foo.pdf"` has
`mime_hint = Some("application/pdf")` and `address_kind = AddressKind::File` after detection.
A node created with `url = "https://example.com"` has `address_kind = AddressKind::Http`.

---

### Step 2: ViewerRegistry Selection Policy

**Goal**: `ViewerRegistry` can select a viewer based on MIME hint and address kind.

- Add `fn select_for(&self, mime: Option<&str>, kind: AddressKind) -> ViewerId` to
  `ViewerRegistry` in `registries/domain/viewer.rs`.
- Implement selection order as described above (override → workspace default → registry best →
  servo fallback → plaintext fallback).
- Update `lifecycle_reconcile.rs`: when promoting a node to Active, consult `select_for` if no
  `viewer_id_override` is set. The selected `ViewerId` is stored ephemerally (not persisted) for
  the current session; user explicit overrides are persisted via `UpdateNodeViewerPreference`.
- Add contract test: `select_for(Some("application/pdf"), AddressKind::File)` → `viewer:pdf`
  when that viewer is registered; falls back to `viewer:plaintext` when not registered.

**Done gate**: Selection policy is tested. Lifecycle promote for a PDF node picks `viewer:pdf`
when registered and `viewer:servo` otherwise. No regression in existing Servo webview path.

---

### Step 3: PlaintextViewer (Baseline Non-Web Renderer)

**Goal**: A working non-web renderer that proves the Viewer trait contract for embedded content.

`viewer:plaintext` already exists as a seed in `ViewerRegistry`. Implement it fully:

- Accepts all `text/*` MIME types, `application/json`, `application/toml`, `application/yaml`.
- Renders content using egui `ScrollArea` + `TextEdit::multiline` (read-only).
- For Markdown (`text/markdown`, `text/x-markdown`): use `egui_commonmark` (third-party crate
  wrapping `pulldown-cmark` for egui, targets egui 0.29+). Verify it builds against egui 0.33
  before adding it; if not updated, write a minimal Markdown renderer using `pulldown-cmark`
  directly (convert to egui `RichText` spans). Do not route Markdown through Servo.
- For syntax highlighting (`text/x-rust`, `text/x-python`, etc.): use `syntect` (MIT, pure Rust,
  5.3.x). Cache the `SyntaxSet` and `ThemeSet` as `std::sync::OnceLock` statics — they are
  expensive to build. Convert `syntect`'s styled ranges to `egui::RichText` spans for rendering.
  The full syntax set adds ~5MB to the binary; mitigate by enabling only a curated language subset.
- Module: `registries/atomic/viewer/plaintext_viewer.rs`.

Add to `Cargo.toml`:
- `syntect = "5"` (pure Rust, MIT) — default features include many languages; disable
  `fancy-regex` feature if binary size matters.
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
- Thumbnail fallback: when an image node is in graph view (Cold), use `Node.thumbnail_data` as
  before — the `ImageViewer` generates and stores the thumbnail on first open.

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

### Step 6: DirectoryViewer (File Manager Mode)

**Goal**: `file://` addresses that point to directories render as a navigable file listing.

This is pure Rust using `std::fs` — no new crates required.

- Module: `registries/atomic/viewer/directory_viewer.rs`.
- `can_render(address_kind: AddressKind::File, mime: None)` — returns true when the `file://`
  URL resolves to a directory path (check `Path::is_dir()`).
- Render a two-column table (name + size/type) inside a `ScrollArea`.
- Click on a file: emit `GraphIntent::NavigateNode { node_key, url: file_url }` — navigates the
  current node to the file (viewer swaps to the appropriate viewer for that file type).
- Click on a directory: emit `GraphIntent::NavigateNode` with the directory URL — navigates in.
- Drag a file to graph: emit `GraphIntent::CreateNode { url: file_url }` — creates a new node.
- Breadcrumb navigation: maintain a `Vec<PathBuf>` history in `DirectoryViewerState` per node.

`address_kind = AddressKind::File` is set by the URL detection in Step 1.

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

This step integrates with `2026-02-20_node_badge_and_tagging_plan.md`:

- **Address-kind badge**: Add a `Badge::ContentType(ViewerId)` variant. When a node's
  viewer is not `viewer:servo` (the default), show a small icon badge indicating the content type:
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

---

### Step 9: Security and Sandboxing Model

**Goal**: Define the permission model for non-HTTP content types.

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

## Crate Summary

| Crate | Version | Feature | Purpose | License | Notes |
| ----- | ------- | ------- | ------- | ------- | ----- |
| `infer` | 0.19 | default | MIME magic-byte detection | MIT | Pure Rust; transitive dep via `image` |
| `mime_guess` | 2.0.5 | default | MIME extension detection | MIT | Already in `Cargo.toml` |
| `syntect` | 5.3 | default | Syntax highlighting | MIT | Pure Rust; cache via `OnceLock` |
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
| `2026-02-20_node_badge_and_tagging_plan.md` | Step 8 extends badge system with `ContentType` badge |
| `2026-02-22_registry_layer_plan.md` | `ViewerRegistry` (Phase 2) is the contract surface; `ProtocolRegistry` is the resolver surface |
| `2026-02-23_udc_semantic_tagging_plan.md` | MIME hints drive UDC tag suggestions; semantic physics clusters by content type |
| `2026-02-24_layout_behaviors_plan.md` | Zone attractors can group PDF nodes together, image nodes together, etc. |
| `2026-02-21_lifecycle_intent_model.md` | Lifecycle promotion/demotion applies to all viewer types; `lifecycle_reconcile.rs` must dispatch to the correct viewer |

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
```

Recommended implementation sequence: 1 → 2 → 3 → 4 → 6 → 8 → 9 → 5 → 7 → 10.

Steps 5, 7, and 10 are feature-gated and can be deferred or skipped without blocking the rest.

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

- Plan created. Research vision (`2026-02-18_universal_node_content_model.md`) and badge/tagging
  plan (`2026-02-20_node_badge_and_tagging_plan.md`) synthesized into this implementation plan.
- `Viewer` trait from `2026-02-23_wry_integration_strategy.md` adopted as the shared contract.
- Ten implementation steps defined with done gates.
- Feature flag strategy aligned with existing Cargo.toml conventions.
- Crate selection finalized for Tier 1 (unconditional) and Tier 2 (feature-gated).
- Security/sandboxing model documented (in-process, file permission guard, no network from
  non-Servo viewers).
- Execution order dependency graph documented.
