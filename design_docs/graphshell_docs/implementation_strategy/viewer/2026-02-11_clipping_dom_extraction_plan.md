# Clipping & DOM Extraction Plan (Refactored 2026-02-24)

**Status**: Partially Implemented
**Phase**: Registry Phase X (Feature Target 9)
**Architecture**: Servo context-menu adapter + Graphshell-owned inspector surface + `WebView::evaluate_javascript(...)` extraction + explicit clip materialization into graph nodes.

**Compatibility note (2026-03-03):**
This plan preserves the original `graphshell://clip/<uuid>` wording as the historical address proposal. Runtime canonical internal routing is now `verso://...`, but clip authority remains intentionally unresolved, so the exact clip address family should be treated as pending rather than final.

## Context
Clipping allows a user to inspect page structure inside a webview, choose meaningful DOM elements, and optionally extract selected elements into independent graph nodes. The clip node preserves the content (HTML/Image) even if the original source changes or goes offline.

The longer-term product is not just "clip this element." It is an **exploded inspector view** of a web node: a temporary Graphshell projection of the page's document-element structure that users can inspect, filter, copy from, and then collapse back into the original page view.

---

## Architecture

### 1. The Trigger: Context Menu as Adapter
The Servo embedder already surfaces native context-menu requests through `Dialog::ContextMenu`.
- **Current seam**: graphshell extends the existing egui-rendered context menu with app-defined "Clip Element" and "Inspect Page Elements" actions.
- **Architectural direction**: the browser/embedder context menu is an adapter, not the final product surface. Graphshell's inspector/palette surface is the authoritative UI for web-element inspection and clip actions.
- **Important refactor**: this no longer needs a new `GraphSemanticEvent::ContextMenu` variant. The authoritative path is dialog action -> host extraction request -> event-loop callback -> Graphshell-owned inspector state.

### 2. The Extraction: Script Injection
Servo does expose `WebView::evaluate_javascript(...)`, so extraction can be performed directly against the active webview.
- **Script**: resolve the target with `document.elementFromPoint(...)`, then return JSON containing `outerHTML`, text excerpt, element tag, page URL/title, and link/image hints.
- **Execution**: triggered when the user selects "Clip Element" from the context menu.
- **Inspector mode**: a sibling extraction path scores salient DOM regions (`article`, `section`, `figure`, headings, images, etc.), deduplicates them, and returns a bounded batch of candidate elements for user inspection/filtering.

### 2.5 Exploded Inspector Projection
The exploded inspector should be understood as a **Graphshell projection of the document element tree**.
- **Base topology**: the page's rendered element tree (parent/child/sibling relationships).
- **Graphshell projection**: a temporary graph over that tree, which may add semantic/grouping edges on top of structural parent/child edges.
- **Why this matters**: the DOM is a tree, but the inspector projection may become a DAG once Graphshell adds repeated-component grouping, semantic-role grouping, or other cross-links.

This means the exploded inspector is not just "show source HTML" and not just "highlight pixels." It is a structured, rendered, inspectable element graph.

### 3. The Data Model: Inspector First, Clip Node Second
The first durable artifact is no longer "a batch of clip nodes." The first artifact is an inspector selection surface fed by extracted DOM candidates. A clip node is created only when the user explicitly materializes one or more inspected elements.

When materialized, a clip node is a regular node with specific metadata:
- **URL**: `data:text/html;base64,...` is the current MVP carrier because it renders immediately in the existing webview viewer and persists with no new storage authority.
- **Tags**: `#clip` is canonical node-owned tag state.
- **Edge**: `UserGrouped` edge from Source Node -> Clip Node, labeled `clip-source`.
- **Deferred route authority**: `verso://clip/<id>` remains a future addressing family for durable clip identities and dedicated clip viewers.

### 4. Clip Fidelity Modes
Clip creation should support multiple fidelity/context modes rather than a single clip format:
- **Clean**: extracted element(s) only, minimal wrapper, no surrounding visual context.
- **Contextual**: extracted element(s) plus a preserved page-context backdrop so the clip can be viewed in situ.
- **Screenshot Note**: raster page/region capture used as a note-like background with optional semantic clip overlays.
- **Offline Slice**: richer local package that can preserve DOM, backdrop texture/screenshot, assets, and metadata depending on user intent.

The important architectural split is:
- **Foreground**: semantic element payloads that remain clip-selectable and inspectable.
- **Backdrop**: optional visual page context (screenshot/texture/page slice) preserved for context.
- **Packaging depth**: how much offline fidelity is stored locally.

Those fidelity modes sit on top of four distinct extraction outputs:
- **Structural extract**: HTML fragment plus relationships/metadata.
- **Semantic extract**: the meaningful content unit Graphshell thinks the element represents.
- **Layout-aware extract**: element plus bounding box / page-context placement.
- **Contextual extract**: element plus some preserved surrounding page state.

The user-facing clip modes are different ways of packaging these underlying extraction outputs.

---

## Implementation Phases

### Phase 1: Context Menu Plumbing
1.  **Landed**: reuse Servo's existing context-menu plumbing and extend the egui dialog with an app-owned "Clip Element" action.
2.  **Landed**: use `ContextMenu::position()` as the extraction anchor instead of introducing a parallel coordinate event path.
3.  **Landed (initial)**: add an app-owned "Inspect Page Elements" action that routes through the same headed event-loop callback seam.
4.  **Deferred**: fully replace this adapter path with a Graphshell-owned contextual palette invocation over web content.

### Phase 2: Content Extraction
1.  **Landed MVP**: `request_clip_element(...)` runs `WebView::evaluate_javascript(...)` and serializes extraction data back through the headed event loop.
2.  **Current payload**: `outerHTML`, text excerpt, page URL/title, tag name, and link/image hints.
3.  **Landed (initial inspector feed)**: `request_page_inspector_candidates(...)` returns a bounded list of salient DOM components for inspection/filtering.
4.  **Deferred fidelity work**: computed-style capture, screenshot crop, iframe/shadow-root handling, stronger salience heuristics, and stacked-element traversal under pointer.

### Phase 3: Inspector Surface
1.  **Landed (initial)**: Graphshell now opens a web inspector panel fed by extracted page candidates instead of immediately materializing batch clips.
    -   Search and category filters (`All`, `Text`, `Link`, `Image`, `Structure`, `Media`)
    -   Explicit actions for "Clip Selected" and "Clip Filtered"
    -   Node creation moved behind deliberate user action
2.  **Landed (early interaction loop)**: in-situ overlay highlighting and stacked-element traversal under pointer now exist as the first step toward live page inspection.
3.  **Deferred**: ancestor/descendant stepping, semantic grouping, temporary exploded element-tree projection view, and palette-mode parity with the rest of Graphshell's contextual command surfaces.

### Phase 4: Clip Node Creation
1.  **Landed MVP**: `GraphBrowserApp::create_clip_node_from_capture(...)` creates a self-contained clip node from the extraction result.
    -   Generate `data:` URL from extracted HTML wrapped in a minimal standalone document.
    -   Set `#clip`, `text/html`, and `AddressKind::Custom`.
    -   Create a labeled `UserGrouped` edge from source node to clip node.
    -   Open the new clip in a split node pane.
2.  **Landed (internal helper)**: `GraphBrowserApp::create_clip_nodes_from_captures(...)` remains available as the explicit "Clip Filtered" materialization path from inspector selections.
3.  **Deferred**: clip fidelity-mode choice (`Clean`, `Contextual`, `Screenshot Note`, `Offline Slice`), durable clip IDs, dedicated clip metadata fields, and `verso://clip/<id>` route resolution.

### Phase 5: Clip Rendering
1.  **Landed**: existing webview viewer already renders `data:` URLs.
2.  **Landed**: graph-node `#clip` visual treatment is already in place.
3.  **Landed (intermediate)**: inspector-selected multi-clip materialization can still produce a graph fan-out of ordinary `#clip` nodes.
4.  **Deferred**: dedicated clip viewer, richer provenance chrome, and a true temporary exploded element-tree view that can be entered/collapsed from a page node.

---

---

## Phase 5: Nostr Publication (Optional, Identity-Gated)

**Status**: Design-ready. Implementation deferred until Nostr identity (keypair) is available in graphshell. Not required for Phases 1–4.

**Reference implementation**: Lantern by fiatjaf (`nostrapps.com/lantern`) — a Hypothesis fork that publishes NIP-84 highlights to Nostr. Lantern is the standard reference for the NIP-84 wire format; graphshell does not depend on or embed it.

### Data model

A clip or text selection may optionally be published as a **NIP-84 Highlight** event:

```json
{
  "kind": 9802,
  "content": "<selected text or empty for full-element clips>",
  "tags": [
    ["r", "<canonical source URL>"],
    ["context", "<surrounding text for text selections>"],
    ["alt", "Highlight"]
  ]
}
```

For full-element clips (HTML extraction, not text selection), `content` is the visible text content of the element; the full `outerHTML` is stored locally only and is never published to relays.

**URL normalization**: The `r` tag must use a canonical URL (scheme + host + path, no tracking parameters). Strip UTM/tracking query params, normalize trailing slashes, prefer `https`. This same normalization feeds graphshell node deduplication (`cached_host`).

### Publication flow

1. User completes a clip action (Phase 3).
2. If a Nostr keypair is configured in graphshell settings, a "Publish to Nostr" option appears in the clip node context menu (not automatic — always explicit).
3. User selects "Publish to Nostr" → graphshell signs and publishes a kind 9802 event to the user's configured relay set (NIP-65 relay list).
4. The clip node gains a `nostr_event_id` metadata field; the published event ID is stored locally for deduplication and link-back.
5. NIP-22 replies to the published highlight (from other Nostr users) may be imported as annotation edges on the clip node — this is a Verse Tier 2 feature (kind 5401 DVM), deferred.

### Scope boundaries

- **Local-first is the default.** Clips are useful without Nostr. Publication is always a user-initiated action, never automatic.
- **No Lantern dependency.** NIP-84 is a published standard; graphshell implements it directly via the `nostr` crate (already in the codebase as `mods/native/nostr`).
- **Verso scope only for extraction.** Clip DOM extraction (script injection via `EmbedderApi`) requires the Verso mod. The Nostr publication step uses the `nostr` mod and is independent of Verso — it can be triggered from any clip node regardless of how it was created.
- **Verse Tier 2 extension point.** When Verse communities annotate pages (NIP-84 highlights from community members), those events may surface as ghost nodes or annotation edges in the graph. This is tracked in `verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md` §4 (kind 5401) and is out of scope for the clipping plan itself.

---

## Validation

1.  **Right-Click**: Context menu appears at correct coordinates over webview.
2.  **Extraction**: "Clip" creates a new node.
3.  **Inspector**: "Inspect Page Elements" opens a Graphshell-owned selection surface with candidate filtering and explicit clip actions.
4.  **Materialization**: "Clip Selected" and "Clip Filtered" create the expected linked `#clip` nodes.
5.  **Content Fidelity**: Opening a clip node shows the extracted HTML element (isolated from original page).
6.  **Persistence**: Clip node survives restart (data URL is persisted).
7.  **Linkage**: Edge exists between Source and each Clip.
8.  **Nostr publication (Phase 6)**: "Publish to Nostr" action produces a valid kind 9802 event with canonical `r` tag; `nostr_event_id` is stored on the clip node; action is absent when no keypair is configured.
