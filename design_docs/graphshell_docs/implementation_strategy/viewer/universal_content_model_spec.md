# Universal Content Model — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active (implementation in progress)

**Related**:

- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `2026-02-24_universal_content_model_plan.md` — implementation plan with done gates
- `2026-03-08_servo_text_editor_architecture_plan.md` — `viewer:text-editor` selection rule and edit-intent policy
- `../../technical_architecture/2026-03-29_middlenet_engine_spec.md` — current architectural authority for the shared document-model / adaptation direction behind UCM Track B; extract narrower `SimpleDocument` / target contracts from here if needed
- `../system/register/canvas_registry_spec.md`
- `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` — core/host split for node fields (§ below)
- `../../TERMINOLOGY.md` — `Viewer`, `ViewerRegistry`, `TileRenderMode`, `AddressKind`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Node content fields** — `mime_hint`, `AddressKind`, and how nodes encode their content type.
2. **Viewer trait** — the shared interface all viewer backends must satisfy.
3. **ViewerRegistry selection policy** — how the correct viewer is resolved for a node.
4. **MIME detection pipeline** — the ordered detection strategy for unknown content types.
5. **Non-web viewer types** — PlaintextViewer, ImageViewer, PdfViewer, DirectoryViewer, AudioViewer, TextEditorViewer.
6. **Feature flags** — optional viewer capabilities and their activation model.
7. **Security and sandboxing** — file permissions and network isolation for non-Servo viewers.
8. **Core/host split** — which types belong in `graphshell-core` vs. the desktop host.

### 1.1 Selection Policy Context

This spec inherits the Viewer domain's **Servo-first rich document policy** from
`VIEWER.md`.

Interpretation for UCM:

- if content can be rendered faithfully through Servo directly, or adapted into
  a rich-document form that Servo can render well, Viewer selection should
  prefer Servo first,
- if content is better served by a content-native renderer, UCM should select
  that renderer directly rather than stretching Servo beyond its natural role.

This means UCM is not trying to make Servo a universal renderer for every file
type. It is trying to use Servo aggressively where a rich document engine is
the best fit, while preserving dedicated viewers for content types that want
specialized treatment.

---

## 2. Node Content Fields Contract

These node content fields are durable node-data inputs. They are not the same
thing as the PMEST facet projection used by faceted filtering. Facet values such
as `address_kind`, `mime_hint`, `domain`, or `viewer_binding` may be projected
from these fields plus runtime/workbench state, but the node content fields
remain the source of truth.

### 2.1 mime_hint

Every graph node carries an optional `mime_hint: Option<MimeType>` field.

```text
MimeType = String  -- e.g. "text/plain", "image/png", "application/pdf"
```

- `mime_hint` is a **hint**, not a guarantee. The ViewerRegistry may override it based on MIME detection results (see §5).
- `mime_hint` is set: at node creation time (from `Content-Type` header, user input, or inference); and updated when detection produces a higher-confidence result.
- `mime_hint = None` triggers the full MIME detection pipeline (see §5).

**Invariant**: `mime_hint` is a node data field. It must not be stored on the `NodePaneState` or `ViewerRegistry` state — it lives in the graph data model.

### 2.2 AddressKind

Every graph node carries an `address_kind: AddressKind` field.

```text
AddressKind =
  | Http           -- http:// or https:// URL
  | File           -- file:// URL or local path
  | Data           -- data: URL
  | GraphshellClip -- legacy clip-address family (historically `graphshell://clip/<uuid>`; final canonical clip namespace pending clip-authority resolution)
  | Directory      -- local filesystem directory path
  | Unknown        -- address type not determined
```

`AddressKind` is the primary dispatch axis for viewer selection (§4, Step 1). It is resolved at node creation time from the address string and does not change unless the node's address changes.

**Invariant**: `AddressKind` must be set for every node that has an address. A node with `address = None` has `address_kind = Unknown`.

**Long-term migration note**: `AddressKind` is the current runtime hint used for viewer dispatch. The `graphshell-core` extraction plan (`2026-03-08_graphshell_core_extraction_plan.md §2.2`) introduces a typed `Address` enum (`Http(Url)`, `File(PathBuf)`, `Onion`, `Ipfs(Cid)`, `Gemini`, `Custom`) as the long-term cross-platform address type. Migration to the typed enum is a separate schema change; `AddressKind` is authoritative until that plan reaches implementation.

---

## 3. Viewer Trait Contract

All viewer backends implement the `Viewer` trait. The trait defines the minimal shared interface for rendering and lifecycle participation.

```rust
trait Viewer {
    fn viewer_id(&self) -> ViewerId;
    fn tile_render_mode(&self) -> TileRenderMode;
    fn render_embedded(&mut self, ui: &mut Ui, tile_rect: Rect);
    fn sync_overlay(&self, overlay_ctx: &mut OverlayContext);
    fn is_overlay_mode(&self) -> bool;
    fn on_attach(&mut self, node_key: NodeKey, prefs: &AppPreferences);
    fn on_detach(&mut self);
    fn on_navigate(&mut self, address: &str);
}
```

### 3.1 render_embedded

Called every frame when the tile is in the viewport and the `TileRenderMode` is `EmbeddedEgui`.

- Must not perform blocking I/O.
- Must complete within the per-frame render budget (see `performance_contract_spec.md`).
- Must not mutate graph state directly. Viewer emits `GraphSemanticEvent` for any graph-affecting side effects.

### 3.2 sync_overlay

Called after the Content Pass to synchronize any native overlay position/size with the tile rect. Only meaningful for `NativeOverlay` viewers.

- For `EmbeddedEgui` and `CompositedTexture` viewers, this is a no-op.
- Must not emit `GraphSemanticEvent` — overlay sync is a pure positional operation.

### 3.3 is_overlay_mode

Returns `true` if the viewer uses a native overlay window (i.e., `TileRenderMode::NativeOverlay`). Used by the compositor to select the correct pass behavior.

### 3.4 on_attach / on_detach

`on_attach` is called when a viewer is assigned to a node pane. `on_detach` is called when the viewer is unassigned. These bracket the viewer's active lifecycle.

**Invariant**: `on_attach` is always followed by `on_detach` before the same viewer instance is attached elsewhere.

### 3.5 on_navigate

Called when the node's address changes while the viewer is active. The viewer must handle address changes without requiring a full detach/re-attach cycle where possible.

---

## 4. ViewerRegistry Selection Policy

The `ViewerRegistry` resolves which `Viewer` backend handles a given node. Selection follows a five-step ordered policy. The first step that produces a definitive result wins.

| Step | Condition | Result |
|------|-----------|--------|
| 1 | `address_kind == Http` or `address_kind == Data` | Select `ServoViewer` |
| 2 | `address_kind == GraphshellClip` | Select `ClipViewer` (renders the legacy clip-address family; exact canonical namespace remains pending clip-authority resolution) |
| 3 | `mime_hint` is in the `text/*` family **and** node is opened with edit intent | Select `TextEditorViewer` (see §4.2) |
| 4 | `mime_hint` is set and a registered viewer claims it | Select that viewer |
| 5 | MIME detection pipeline (§5) produces a MIME type with a registered viewer | Select that viewer |
| 6 | No viewer matched | Select `FallbackViewer` (placeholder surface) |

**Invariant**: The selection result is stored on `NodePaneState.tile_render_mode` at attachment time. The registry does not re-run selection per frame.

**Invariant**: `ServoViewer` is never selected for `File` or `Directory` address kinds, even if the address could theoretically be loaded in a browser.

### 4.1 Viewer Priority Override

A node may carry a `viewer_override: Option<ViewerId>` field to force a specific viewer regardless of address or MIME type. This is user-set and takes precedence over all six steps.

**Invariant**: `viewer_override` is stored in graph data, not in registry state. The registry reads it before executing the six-step policy.

### 4.2 Edit-Intent Open Policy

**Edit intent** is defined as one of:

- The node was created as a new local text file (address `File`, `mime_hint` in `text/*`) with no prior content.
- The user explicitly invoked an edit action (`action:node.edit`, available in the command palette and node context menu) on a node whose active viewer is `PlaintextViewer` or `FallbackViewer`.
- `viewer_override` is explicitly set to `viewer:text-editor` by the user.

Read-intent open (the default for all other cases, including clicking a link to a local text file from the `DirectoryViewer`) selects `PlaintextViewer`. Edit intent is never inferred automatically from MIME type alone; it requires an explicit user gesture or node-creation-as-new-file context.

**Invariant**: `TextEditorViewer` is only selected at node-open time via edit intent. The registry does not switch from `PlaintextViewer` to `TextEditorViewer` mid-session without a new explicit edit-intent open. See `2026-03-08_servo_text_editor_architecture_plan.md §9` for the full selection and fallback chain.

---

## 5. MIME Detection Pipeline

When `mime_hint` is `None` and no registered viewer claims the address kind directly, the detection pipeline runs in the following order:

| Stage | Method | When used |
|-------|--------|-----------|
| 1 | HTTP `Content-Type` header | `address_kind == Http`; available after first response |
| 2 | File extension lookup (`mime_guess`) | `address_kind == File`; cheap, synchronous; runs first |
| 3 | Magic byte inspection (`infer`, first 512 bytes) | `address_kind == File` or `Data`; only when extension is absent or ambiguous |
| 4 | None | Detection failed; selection falls through to Step 5 of §4 |

Rationale for extension-before-magic order: extension lookup is synchronous and covers the vast majority of correctly-named files without I/O. Magic byte inspection is an async fallback for extension-missing or extension-ambiguous cases only. This minimizes I/O task pool pressure for normal use.

**Invariant**: Detection is performed once and the result is written back to the node's `mime_hint` field via a `SetMimeHint` graph intent. The detection pipeline must not re-run on every frame.

**Invariant**: Magic byte inspection must not read the full file. It reads only the first 512 bytes. No blocking I/O on the frame thread; detection runs on the I/O task pool.

---

## 6. Non-Web Viewer Types

The following viewer backends are defined for non-HTTP content. Each is an `EmbeddedEgui` viewer unless noted.

| Viewer | MIME types handled | Feature flag | Notes |
|--------|--------------------|--------------|-------|
| `PlaintextViewer` | `text/plain`, `text/markdown`, `text/csv`, `text/*` | none (always on) | Read-only display. Syntax highlighting via `syntect` with `fancy-regex` feature (WASM-portable). See §6.1. |
| `TextEditorViewer` | `text/*`, selected code/doc formats | none (always on) | Edit-intent open only (see §4.2). Servo surface + `editor-core` Rust crate. See `2026-03-08_servo_text_editor_architecture_plan.md`. |
| `ImageViewer` | `image/*` (PNG, JPEG, GIF, SVG, WebP) | none (always on) | SVG via `resvg`; animated GIF via frame sequence |
| `PdfViewer` | `application/pdf` | `pdf` | Uses `pdfium-render`; disabled if feature flag off → falls back to FallbackViewer |
| `DirectoryViewer` | `AddressKind::Directory` | none (always on) | Browse-in-place file listing; emits `NavigateTo` on file click, `CreateNode` on drag-to-graph |
| `AudioViewer` | `audio/*` (MP3, OGG, FLAC, WAV) | `audio` | Uses `symphonia` + `rodio`; minimal transport controls; disabled if feature flag off |
| `ClipViewer` | `AddressKind::GraphshellClip` | none (always on) | Renders clipped content stored in the clip-address family defined by the clipping spec; canonical clip namespace pending resolution |
| `FallbackViewer` | anything unmatched | n/a | Placeholder surface; shows address, detected MIME, and "No viewer available" message |

**Invariant**: All non-web viewers use `TileRenderMode::EmbeddedEgui`. No non-web viewer may use `NativeOverlay` or `CompositedTexture`.

### 6.1 PlaintextViewer

- Read-only display. `TextEditorViewer` takes priority for edit-intent opens of the same MIME types (§4.2).
- Markdown is rendered with `pulldown-cmark`; links in markdown emit `NavigateTo` on click.
- Syntax highlighting uses `syntect` with `default-features = false, features = ["default-fancy"]` — the `fancy-regex` pure-Rust backend is required for WASM portability (the default oniguruma backend is not WASM-safe). `tree-sitter` is used in `editor-core` for incremental highlight in edit mode; `syntect` is the read-only display path only.
- Syntax highlighting language is inferred from file extension or explicit `mime_hint` subtype.
- Large files (> 1 MB) are rendered in virtual scroll mode; only visible lines are laid out.

### 6.2 ImageViewer

- Images are loaded on the I/O task pool; the viewer shows a loading indicator until available.
- SVG rendering uses `resvg`; SVG `<a>` links emit `NavigateTo` on click.
- Animated GIFs respect the `prefers-reduced-motion` user preference: static first frame if motion reduced.

### 6.3 DirectoryViewer

- Directory listing is fetched from the local filesystem; not from a remote server.
- Selecting a file emits `NavigateTo { address: file://<path> }` intent to the Command subsystem.
- Selecting a subdirectory navigates the current node to that directory (updates `address`).

---

## 7. Feature Flag Contract

Optional viewer capabilities are gated by Cargo feature flags.

| Flag | Enables | Default |
|------|---------|---------|
| `pdf` | `PdfViewer` (pdfium-render) | off |
| `audio` | `AudioViewer` (symphonia + rodio) | off |

**Invariant**: When a feature flag is off, the corresponding viewer type must not appear in the binary. The `ViewerRegistry` must not register it. MIME types that would have been claimed by the disabled viewer fall through to `FallbackViewer`.

**Invariant**: Feature flags are compile-time only. There is no runtime toggle for viewer feature flags.

---

## 8. Security and Sandboxing Contract

### 8.1 FilePermissionGuard

All non-web viewer access to the local filesystem goes through `FilePermissionGuard`. This section is the canonical specification; UCM Step 9 (`2026-02-24_universal_content_model_plan.md`) and the filesystem ingest plan (`2026-03-02_filesystem_ingest_graph_mapping_plan.md`) both defer to this section.

#### What constitutes the "home directory"

The home directory boundary is defined as:

- **Linux/macOS**: the value of the `HOME` environment variable, resolved to an absolute path. If `HOME` is unset, fall back to the `passwd` entry for the current UID. If both are unavailable, no path is auto-allowed.
- **Windows**: the value of `USERPROFILE` environment variable resolved to an absolute path. If unset, fall back to `FOLDERID_Profile` via the Windows Shell API. If unavailable, no path is auto-allowed.
- The home directory boundary is evaluated at `FilePermissionGuard` construction time and cached. It is not re-evaluated per request.
- Symlinks in the home path are resolved to their canonical target before comparison. A file whose resolved path is inside the resolved home directory is considered home-relative regardless of how it was addressed.

#### Allow-list structure in `AppPreferences`

```rust
// In AppPreferences (host crate)
pub struct FileAccessPolicy {
    /// Paths explicitly allowed by the user (persisted per workspace).
    /// Each entry is a canonicalized absolute directory path.
    pub allowed_directories: Vec<PathBuf>,
    /// If true, the home directory is auto-allowed without a prompt.
    /// Default: true.
    pub home_directory_auto_allow: bool,
    /// If Some(Deny), all file access outside the allow-list is silently denied.
    /// If None (default), out-of-scope access triggers a prompt.
    pub out_of_scope_policy: Option<OutOfScopePolicy>,
}

pub enum OutOfScopePolicy {
    Deny,
    // (future: Allow, for trusted workspaces)
}
```

`allowed_directories` contains **directory** paths, not file paths. A file is permitted if any allowed directory is a prefix of the file's canonicalized path. Prefix matching is done on path components, not string prefixes (to avoid `/home/user` matching `/home/username`).

`FileAccessPolicy` is stored in `AppPreferences` and persisted in the WAL as a `UpdatePreferences` log entry. It survives app restarts.

#### Prompt UX

When a `file://` address is outside the home directory and the allow-list, and `out_of_scope_policy` is `None` (default), `FilePermissionGuard` triggers a one-time permission prompt:

- The prompt is modal and blocks the viewer from loading until resolved.
- Prompt text: **"Allow access to \<directory\>?"** — showing the parent directory of the requested file, not the full path.
- Options: **Allow this directory** (adds to `allowed_directories`), **Deny** (denies this request; does not persist), **Always deny** (sets `out_of_scope_policy = Deny`).
- The prompt is shown once per unique directory per workspace session. If the user selects "Allow this directory", subsequent accesses to the same directory within the session (and across restarts) are auto-allowed.
- The prompt is emitted as a `GraphSemanticEvent::RequestFilePermission` from `FilePermissionGuard`; the host UI layer renders it. `FilePermissionGuard` does not render UI directly.

#### Denial propagation

When access is denied (either by `out_of_scope_policy = Deny` or user selecting "Deny"):

- The requesting viewer receives `Err(FilePermissionDenied)` from `FilePermissionGuard::check()`.
- The viewer falls back to `FallbackViewer` with message: **"Access denied — \<address\>"**. It does not show partial content or a loading state.
- The denial is emitted as a diagnostic event on the `viewer:permission_denied` channel (severity: `Warn`).
- Denied addresses are not cached or persisted (each new viewer attachment re-runs the check).

**Invariant**: No viewer backend may call filesystem APIs directly. All file access goes through `FilePermissionGuard::check()` before any read is attempted.

**Invariant**: `FilePermissionGuard` is a host-only type. It must not appear in `graphshell-core`. Its construction requires access to `AppPreferences` and the host filesystem for path canonicalization.

**Hard prerequisite**: `FilePermissionGuard` must reach its done gate (UCM Step 9) before the filesystem ingest feature (Phase 1) can close. See `2026-03-02_filesystem_ingest_graph_mapping_plan.md §Feature Gate`.

### 8.2 No-Network Invariant for Non-Servo Viewers

Non-web viewers (`PlaintextViewer`, `ImageViewer`, `PdfViewer`, `DirectoryViewer`, `AudioViewer`) must not initiate any network requests.

- These viewers operate on local content only.
- If a local file contains a remote reference (e.g., an `<img src="https://...">` in a markdown file), the reference is not fetched. It renders as a broken-image placeholder.
- This invariant is enforced at the viewer implementation level; the I/O task pool used by non-web viewers does not have network access.

### 8.3 WASM-Sandboxed Viewer Extension

Third-party viewers loaded via the Mods subsystem (WASM tier) are sandboxed by the extism runtime. They implement the `Viewer` trait via a host-side wrapper. The wrapper enforces: no direct filesystem access, no network access, no access to graph state beyond the provided `tile_rect` and node metadata.

---

## 9. Core/Host Split

The `graphshell-core` extraction plan (`2026-03-08_graphshell_core_extraction_plan.md`) requires that types shared across all deployment targets (desktop, mobile, WASM, browser extension) live in a WASM-clean core crate. This has direct implications for the UCM:

| Type | Layer | Rationale |
| --- | --- | --- |
| `mime_hint: Option<String>` | **Core** — lives on `Node` in graph domain state | Must be identical across platforms for sync correctness |
| `AddressKind` enum | **Core** — graph data field | Dispatch hint must be cross-platform |
| `address_kind: AddressKind` field | **Core** | Same as above |
| `viewer_override: Option<ViewerId>` | **Core** | User preference stored in WAL; must survive sync |
| `Viewer` trait | **Host only** — references `egui::Ui`, `AppPreferences` | egui is a desktop dep; WASM builds have no viewer runtime |
| `ViewerRegistry` | **Host only** | Registry manages live viewer instances; desktop-only |
| `FilePermissionGuard` | **Host only** | Filesystem access is a host capability |
| `PlaintextViewer`, `ImageViewer`, etc. | **Host only** | egui widget code; desktop-only |
| `TextEditorViewer` / `editor-core` | `editor-core` = **WASM-clean**; surface module = **Host only** | See text editor plan §3 |

The `Viewer` trait and all viewer implementations stay in the host crate. Only the node data fields (`mime_hint`, `address_kind`, `viewer_override`) migrate to core.

## 10. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| `ServoViewer` selected for `Http` address | Test: node with `address_kind = Http` → `ViewerRegistry::select` returns `ServoViewer` |
| `FallbackViewer` selected when no viewer matches | Test: unknown MIME with no registered viewer → `FallbackViewer` |
| `mime_hint` written back after detection | Test: node with `mime_hint = None`, `File` address → after attach, `mime_hint` is set |
| Detection does not re-run per frame | Test: attach viewer → detection pipeline called exactly once; subsequent frames skip detection |
| `PdfViewer` absent when `pdf` feature off | Test: build without `pdf` flag → `PdfViewer` type not in binary; PDF MIME falls to `FallbackViewer` |
| Non-web viewer does not initiate network request | Test: `PlaintextViewer` with remote image ref in markdown → no outbound connection |
| `FilePermissionGuard` blocks out-of-scope path | Test: node address outside permitted set → viewer shows "Access denied", not content |
| `viewer_override` takes precedence over selection policy | Test: node with `viewer_override = PlaintextViewer`, `Http` address → `PlaintextViewer` selected |
| `on_detach` always called before re-attach | Architecture invariant: no `on_attach` without prior `on_detach` on same instance |
| `render_embedded` does not mutate graph state | Architecture invariant: no `GraphIntent` dispatch from `render_embedded` |
