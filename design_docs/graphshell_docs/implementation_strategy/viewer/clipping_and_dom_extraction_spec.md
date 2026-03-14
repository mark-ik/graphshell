# Clipping and DOM Extraction — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract, pending inspector-mode rewrite completion
**Priority**: Implementation-ready

**Related**:

- `VIEWER.md`
- `universal_content_model_spec.md`
- `2026-02-11_clipping_dom_extraction_plan.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../canvas/node_badge_and_tagging_spec.md`
- `../../TERMINOLOGY.md` — `Clip Node`, `GraphSemanticEvent`, `EmbedderApi`

---

**Status note (2026-03-03):**

- This spec preserves the original `graphshell://clip/<uuid>` wording as the historical clip-address proposal.
- Runtime canonical internal routing has moved to `verso://...`, but clip authority is not yet finalized.
- Until clip authority is explicitly defined, treat all `graphshell://clip/<uuid>` references here as provisional design intent rather than as the current canonical runtime namespace.

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Context-menu / contextual-surface adapter contract** — how backend context actions feed Graphshell-owned inspection surfaces.
2. **Script injection contract** — how DOM extraction is performed for single-element clip and inspector-candidate discovery.
3. **Exploded inspector projection contract** — how the page's element structure is represented as a temporary inspectable Graphshell projection.
4. **Inspector-first interaction contract** — how users filter/select page elements before clip-node creation.
5. **Clip node data model** — the `#clip` tag, address scheme, and edge semantics after explicit materialization.
6. **Graph rendering of clip nodes** — how clip nodes appear in the canvas.
7. **Backend capability contract** — how Servo and Wry expose clipping uniformly.

---

## 2. Context Menu / Contextual Surface Contract

**Compatibility note (2026-03-13):** the older `GraphSemanticEvent::ContextMenu` model below is now historical/speculative rather than current runtime architecture. Current Servo-backed runtime uses `Dialog::ContextMenu` as an adapter seam and opens Graphshell-owned inspector/clip surfaces from there. The long-term invariant remains the same: browser-native context meaning must be surfaced through Graphshell command/inspection authority rather than a separate browser UX.

### 2.1 Trigger Path

DOM extraction is initiated from the viewer's context menu. When the user right-clicks within a web viewer tile (Servo or Wry):

1. The backend emits a context-menu event with hit metadata.
2. The backend integration layer translates it into `GraphSemanticEvent::ContextMenu`.
3. Graphshell's event pipeline receives `GraphSemanticEvent::ContextMenu` and displays the Graphshell context menu (not the browser's native context menu).

**Invariant**: The browser-native context menu must be suppressed for all backends that support clipping. `GraphSemanticEvent::ContextMenu` is the sole trigger for Graphshell clipping actions in web viewer tiles.

### 2.2 GraphSemanticEvent::ContextMenu

```text
GraphSemanticEvent::ContextMenu {
    node_key: NodeKey,         -- the node whose viewer was right-clicked
    position: PhysicalPoint,   -- screen position of the right-click
    hit: ContextMenuHit,       -- what was under the cursor
}

ContextMenuHit =
  | Text { selected: Option<String> }   -- right-click on text; optional selection
  | Image { src_url: String }           -- right-click on an image
  | Link { href: String }               -- right-click on a link
  | Background                          -- right-click on page background
```

**Invariant**: `ContextMenuHit` is determined by Servo at event time. Graphshell must not re-derive hit information from DOM queries after the event arrives — the hit data in the event is authoritative.

### 2.3 Inspector and Clip Actions

The Graphshell contextual surface includes:

- a direct "Clip Element" action for one-step capture
- an "Inspect Page Elements" action that opens a Graphshell-owned inspector surface with filtering/search before node creation

Activating a clip action emits a `ClipContent` intent:

```text
GraphIntent::ClipContent {
    source_node_key: NodeKey,
    clip_kind: ClipKind,
}

ClipKind =
  | SelectedText { text: String }
  | Image { src_url: String }
  | FullPage
```

**Refactor note (2026-03-13):** current runtime implementation has not yet been normalized to a formal `ClipContent` intent. Clip node creation currently occurs through Graphshell app mutation helpers fed by inspector/clip extraction results. The design intent remains that clip creation semantics must converge behind one authoritative action contract.

---

## 3. Script Injection Contract

For single-element clip and for inspector-candidate discovery, Graphshell uses backend JS evaluation against the active page.

### 3.1 Backend JS Evaluation Contract

```text
ViewerBackend::evaluate_javascript(
    webview_id: WebViewId,
    script: &str,
    callback: impl Fn(JsonResult) + Send + 'static,
)
```

- `script` is a JavaScript string evaluated in the page's top-level frame.
- `callback` receives the serialized return value as a JSON string.
- Evaluation is asynchronous; the callback runs on the embedder event thread.

**Invariant**: Scripts injected via `EmbedderApi` must be read-only. They must not mutate page DOM state, submit forms, or initiate navigation. Violations are a security bug.

**Invariant**: The script content is constructed by Graphshell, not by user input or page content. No user-controlled string is interpolated into the script without sanitization.

### 3.1.1 Backend note (Servo and Wry)

The injection contract is backend-neutral at the Graphshell boundary:

- Servo path: existing embedder injection route.
- Wry path: backend executes equivalent read-only JS evaluation and returns serialized JSON via the same callback/result shape.

Backend-specific API names may differ; behavior at the Graphshell contract boundary must match §3.1.

### 3.2 DOM Extraction Scripts

Current runtime uses two script classes:
1. **Single-element clip** — resolves the target with `document.elementFromPoint(...)` and returns one serialized element payload.
2. **Inspector candidate discovery** — scores salient page regions (`article`, `section`, `figure`, headings, images, etc.), deduplicates them, and returns a bounded array of candidate element payloads.

Each payload currently includes:
- `source_url`
- `page_title`
- `clip_title`
- `outer_html`
- `text_excerpt`
- `tag_name`
- `href`
- `image_url`
- `dom_path`

**Invariant**: The extraction script does not exfiltrate cookies, localStorage, or any credential data. It reads only publicly visible page content.

### 3.3 Backend Capability Contract

Clipping must be exposed via a backend-neutral capability surface rather than backend-specific callsites.

```rust
pub trait ViewerClipProvider {
    fn supports_context_menu_clip(&self) -> bool;
    fn supports_dom_extract(&self) -> bool;
    fn clip_from_hit(&mut self, hit: ContextMenuHit) -> Result<ClipKind, ClipError>;
    fn extract_full_page(&mut self, node_key: NodeKey, callback: ClipExtractCallback) -> Result<(), ClipError>;
}
```

- `ServoViewer` and `WryViewer` both implement this capability for pane-hosted web viewers.
- Graph/render code dispatches clip requests through the capability surface, not through backend type checks.
- Backends that do not support DOM extraction must return capability false and surface a deterministic fallback message.

---

## 4. Exploded Inspector Projection Contract

The exploded inspector is not just a list of clip candidates. The canonical direction is a temporary **Graphshell projection of the document element tree** for the active page.

### 4.1 Base Topology

The underlying browser structure is the rendered document element tree:
- parent/child relationships
- sibling ordering
- attributes and text content
- rendered footprint (bounds, overlap, visibility)

### 4.2 Graphshell Projection

Graphshell may project that tree into a temporary graph for inspection. The base projection is structural, but Graphshell may add semantic/grouping edges that turn the temporary projection into a DAG:
- structural parent/child edges
- repeated-component grouping edges
- semantic-role grouping (`content`, `media`, `nav`, `comments`, etc.)
- provenance edges to durable clip nodes created from inspected elements

**Invariant**: the exploded inspector projection is temporary inspection state, not durable graph mutation by default. Entering inspector mode must not permanently materialize the entire page element tree into the user's graph.

### 4.3 Extraction Layers

Inspector outputs should be understood through four extraction layers:
- **Structural extract**: HTML fragment plus relationships/metadata
- **Semantic extract**: the meaningful content unit Graphshell thinks the element represents
- **Layout-aware extract**: element plus bounding box / page-context placement
- **Contextual extract**: element plus preserved surrounding page state

Clip fidelity modes (`Clean`, `Contextual`, `Screenshot Note`, `Offline Slice`) are packaging choices built on top of these extraction layers.

---

## 5. Inspector-First Interaction Contract

Before clip-node creation, Graphshell may open an inspector surface over the extracted candidate set.

Current runtime initial slice:
- search query
- category filter (`All`, `Text`, `Link`, `Image`, `Structure`, `Media`)
- explicit "Clip Selected" and "Clip Filtered" actions
- in-situ highlight overlay
- stacked-element traversal under pointer

Deferred canonical inspector behavior:
- ancestor/descendant stepping
- temporary exploded element-tree projection view
- contextual-palette parity with the rest of Graphshell command surfaces

**Invariant**: multi-element page discovery must not force immediate node creation. Users need an inspection/filtering step before Graphshell materializes a batch of clip nodes.

---

## 6. Clip Node Data Model Contract

### 5.0 Clip Fidelity Modes

Clip materialization is not limited to a single content shape. The canonical direction is a family of locally stored clip modes:

- **Clean**: extracted DOM element(s) only
- **Contextual**: extracted DOM element(s) plus preserved page-context backdrop
- **Screenshot Note**: screenshot/raster capture used as a note background
- **Offline Slice**: richer local package combining semantic element payloads, backdrop, and supporting metadata/assets

**Invariant**: the semantic foreground element(s) and the contextual backdrop are distinct capture layers. Graphshell must be able to preserve element semantics without requiring a screenshot, and preserve visual context without collapsing the semantic payload into raster-only form.

### 4.1 NodeState and Tag

A clip node is an ordinary graph node with:

- `node_state: NodeState::Active` (clip nodes are active nodes, not a special lifecycle state)
- Tag: `#clip` (system-managed; see `../canvas/node_badge_and_tagging_spec.md §2.1`)
- `address_kind: AddressKind::GraphshellClip`
- `address: "graphshell://clip/<uuid>"`

**Invariant**: The `#clip` tag is in the `#` reserved namespace. It must not be user-removable while the node retains clip content. Removing clip content (via "Delete clip" action) removes the tag and transitions the node to a standard empty node or triggers node deletion.

### 4.2 Content Storage

Clip content is stored at the `graphshell://clip/<uuid>` address in the local graphshell storage layer. The storage format depends on the clip kind:

| ClipKind | Storage format |
|----------|---------------|
| `SelectedText` | `text/plain` or `text/markdown` |
| `Image` | Original image bytes; MIME type from source |
| `FullPage` | JSON (title, description, text, url from extraction script) |

**Invariant**: Clip content is stored locally. No clip content is transmitted to external services.

**Invariant**: The `<uuid>` in the address is generated fresh at clip creation time. It is stable and does not change for the lifetime of the clip node.

### 4.3 Edge to Source Node

When a clip node is created from a source node, a `UserGrouped` edge is created between the clip node and the source node:

```text
Edge {
    kind: EdgeKind::UserGrouped,
    source: clip_node_key,
    target: source_node_key,
    label: "clipped from",
}
```

**Invariant**: The edge is created atomically with the clip node (same `LogEntry` batch). A clip node without an edge to its source is an invalid state.

**Invariant**: Deleting the source node does not automatically delete clip nodes derived from it. The edge becomes a `UserGrouped` edge to a tombstoned node (or, if the source is hard-deleted, a dangling edge that is repaired at load time by removing it).

---

## 7. Graph Rendering of Clip Nodes

### 5.1 Visual Style

Clip nodes render on the canvas with a distinct visual identity:

- Node body background: distinct tint (e.g., amber/yellow in the default theme) to differentiate from web nodes.
- Node badge: `#clip` badge rendered in the primary badge slot (highest-priority system badge).
- Node label: clip title if available; otherwise the truncated UUID.

**Invariant**: Clip node visual style is defined in `CanvasStylePolicy`. It must not be hardcoded in the render path.

### 5.2 Viewer for Clip Nodes

Clip nodes use `ClipViewer` (see `universal_content_model_spec.md §6`). `ClipViewer`:

- Reads content from `graphshell://clip/<uuid>` storage.
- Renders text clips as scrollable text (via `PlaintextViewer` logic).
- Renders image clips as `ImageViewer` content.
- Renders full-page clips as a structured card (title, description, excerpt).

`TileRenderMode` for clip nodes is always `EmbeddedEgui`.

### 5.3 Interaction on Clip Nodes

| Interaction | Behavior |
|-------------|----------|
| Click | Select clip node; focused pane shows `ClipViewer` content |
| Right-click | Context menu: "Open source", "Delete clip", "Copy text", "Export" |
| "Open source" | Emit `NavigateTo` for the source node's address |
| Double-click | Same as clicking the source edge's target (navigate to source) |

**Invariant**: "Delete clip" removes the clip content from storage, removes the `#clip` tag, and deletes the clip node. This is irreversible (no tombstone; clip content is not preserved post-deletion).

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Browser native context meaning bridged into Graphshell-owned actions | Test: right-click in Servo/Wry tile → Graphshell action surface appears rather than a standalone browser UX path |
| Inspector action opens Graphshell-owned selection surface | Test: invoke "Inspect Page Elements" → inspector window shows extracted candidates |
| Pointer-stack inspection works in situ | Test: move pointer over nested/stacked page content → inspector highlight can step through stacked elements |
| Entering exploded inspector does not mutate the user graph by default | Test: open inspector/exploded view → no durable graph nodes created until explicit clip action |
| Clip node address is `graphshell://clip/<uuid>` | Test: create clip → node address matches scheme |
| `#clip` tag is system-managed and non-removable by user | Test: attempt to remove `#clip` tag via tag panel → tag remains |
| Clip content stored locally, no external transmission | Architecture invariant: no outbound network calls during `ClipContent` intent processing |
| `UserGrouped` edge created with clip node | Test: create clip → edge exists from clip node to source node |
| Injected script does not read cookies or localStorage | Architecture invariant: extraction script source contains no `document.cookie` or `localStorage` access |
| Clipping contract is backend-neutral | Test: clip actions route through capability surface with both Servo and Wry implementations |
| `ClipViewer` selected for `GraphshellClip` address | Test: clip node → `ViewerRegistry::select` returns `ClipViewer` |
| "Delete clip" removes content and node | Test: delete clip → `graphshell://clip/<uuid>` address no longer resolves; node gone |
| Inspector batch discovery does not force immediate node creation | Test: invoke inspector → no clip nodes appear until user chooses "Clip Selected" or "Clip Filtered" |
