# Universal Content Model — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active (implementation in progress)

**Related**:

- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `viewer/2026-02-24_universal_content_model_plan.md`
- `../system/register/canvas_registry_spec.md`
- `../../TERMINOLOGY.md` — `Viewer`, `ViewerRegistry`, `TileRenderMode`, `AddressKind`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Node content fields** — `mime_hint`, `AddressKind`, and how nodes encode their content type.
2. **Viewer trait** — the shared interface all viewer backends must satisfy.
3. **ViewerRegistry selection policy** — how the correct viewer is resolved for a node.
4. **MIME detection pipeline** — the ordered detection strategy for unknown content types.
5. **Non-web viewer types** — PlaintextViewer, ImageViewer, PdfViewer, DirectoryViewer, AudioViewer.
6. **Feature flags** — optional viewer capabilities and their activation model.
7. **Security and sandboxing** — file permissions and network isolation for non-Servo viewers.

---

## 2. Node Content Fields Contract

### 2.1 mime_hint

Every graph node carries an optional `mime_hint: Option<MimeType>` field.

```
MimeType = String  -- e.g. "text/plain", "image/png", "application/pdf"
```

- `mime_hint` is a **hint**, not a guarantee. The ViewerRegistry may override it based on MIME detection results (see §5).
- `mime_hint` is set: at node creation time (from `Content-Type` header, user input, or inference); and updated when detection produces a higher-confidence result.
- `mime_hint = None` triggers the full MIME detection pipeline (see §5).

**Invariant**: `mime_hint` is a node data field. It must not be stored on the `NodePaneState` or `ViewerRegistry` state — it lives in the graph data model.

### 2.2 AddressKind

Every graph node carries an `address_kind: AddressKind` field.

```
AddressKind =
  | Http           -- http:// or https:// URL
  | File           -- file:// URL or local path
  | Data           -- data: URL
  | GraphshellClip -- graphshell://clip/<uuid>  (see clipping spec)
  | Directory      -- local filesystem directory path
  | Unknown        -- address type not determined
```

`AddressKind` is the primary dispatch axis for viewer selection (§4, Step 1). It is resolved at node creation time from the address string and does not change unless the node's address changes.

**Invariant**: `AddressKind` must be set for every node that has an address. A node with `address = None` has `address_kind = Unknown`.

---

## 3. Viewer Trait Contract

All viewer backends implement the `Viewer` trait. The trait defines the minimal shared interface for rendering and lifecycle participation.

```
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
| 2 | `address_kind == GraphshellClip` | Select `ClipViewer` (renders `graphshell://clip/` content) |
| 3 | `mime_hint` is set and a registered viewer claims it | Select that viewer |
| 4 | MIME detection pipeline (§5) produces a MIME type with a registered viewer | Select that viewer |
| 5 | No viewer matched | Select `FallbackViewer` (placeholder surface) |

**Invariant**: The selection result is stored on `NodePaneState.tile_render_mode` at attachment time. The registry does not re-run selection per frame.

**Invariant**: `ServoViewer` is never selected for `File` or `Directory` address kinds, even if the address could theoretically be loaded in a browser.

### 4.1 Viewer Priority Override

A node may carry a `viewer_override: Option<ViewerId>` field to force a specific viewer regardless of address or MIME type. This is user-set and takes precedence over all five steps.

**Invariant**: `viewer_override` is stored in graph data, not in registry state. The registry reads it before executing the five-step policy.

---

## 5. MIME Detection Pipeline

When `mime_hint` is `None` and no registered viewer claims the address kind directly, the detection pipeline runs in the following order:

| Stage | Method | When used |
|-------|--------|-----------|
| 1 | HTTP `Content-Type` header | `address_kind == Http`; available after first response |
| 2 | Magic byte inspection | `address_kind == File` or `Data`; read first 512 bytes |
| 3 | File extension | `address_kind == File`; path extension lookup |
| 4 | None | Detection failed; selection falls through to Step 5 of §4 |

**Invariant**: Detection is performed once and the result is written back to the node's `mime_hint` field via a `SetMimeHint` graph intent. The detection pipeline must not re-run on every frame.

**Invariant**: Magic byte inspection must not read the full file. It reads only the first 512 bytes. No blocking I/O on the frame thread; detection runs on the I/O task pool.

---

## 6. Non-Web Viewer Types

The following viewer backends are defined for non-HTTP content. Each is an `EmbeddedEgui` viewer unless noted.

| Viewer | MIME types handled | Feature flag | Notes |
|--------|--------------------|--------------|-------|
| `PlaintextViewer` | `text/plain`, `text/markdown`, `text/csv` | none (always on) | Syntax highlighting via `syntect`; renders as scrollable egui widget |
| `ImageViewer` | `image/*` (PNG, JPEG, GIF, SVG, WebP) | none (always on) | SVG via `resvg`; animated GIF via frame sequence |
| `PdfViewer` | `application/pdf` | `pdf` | Uses `pdfium-render`; disabled if feature flag off → falls back to FallbackViewer |
| `DirectoryViewer` | `AddressKind::Directory` | none (always on) | File browser widget; emits `NavigateTo` intent on file selection |
| `AudioViewer` | `audio/*` (MP3, OGG, FLAC, WAV) | `audio` | Uses `symphonia` + `rodio`; minimal transport controls; disabled if feature flag off |
| `ClipViewer` | `AddressKind::GraphshellClip` | none (always on) | Renders clipped content stored in `graphshell://clip/` address space |
| `FallbackViewer` | anything unmatched | n/a | Placeholder surface; shows address, detected MIME, and "No viewer available" message |

**Invariant**: All non-web viewers use `TileRenderMode::EmbeddedEgui`. No non-web viewer may use `NativeOverlay` or `CompositedTexture`.

### 6.1 PlaintextViewer

- Markdown is rendered with `pulldown-cmark`; links in markdown emit `NavigateTo` on click.
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

All non-web viewer access to the local filesystem goes through `FilePermissionGuard`.

- `FilePermissionGuard` checks the node's address against the workspace's permitted path set.
- Access outside the permitted path set is denied; the viewer falls back to `FallbackViewer` with an explicit "Access denied" message.
- The permitted path set is configured per-workspace in `AppPreferences`.

**Invariant**: No viewer backend may call filesystem APIs directly. All file access goes through `FilePermissionGuard`.

### 8.2 No-Network Invariant for Non-Servo Viewers

Non-web viewers (`PlaintextViewer`, `ImageViewer`, `PdfViewer`, `DirectoryViewer`, `AudioViewer`) must not initiate any network requests.

- These viewers operate on local content only.
- If a local file contains a remote reference (e.g., an `<img src="https://...">` in a markdown file), the reference is not fetched. It renders as a broken-image placeholder.
- This invariant is enforced at the viewer implementation level; the I/O task pool used by non-web viewers does not have network access.

### 8.3 WASM-Sandboxed Viewer Extension

Third-party viewers loaded via the Mods subsystem (WASM tier) are sandboxed by the extism runtime. They implement the `Viewer` trait via a host-side wrapper. The wrapper enforces: no direct filesystem access, no network access, no access to graph state beyond the provided `tile_rect` and node metadata.

---

## 9. Acceptance Criteria

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
