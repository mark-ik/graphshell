# Clipping & DOM Extraction Plan (Refactored 2026-02-24)

**Status**: Implementation-Ready
**Phase**: Registry Phase X (Feature Target 9)
**Architecture**: Extension of `Verso` Native Mod + `GraphSemanticEvent`.

**Compatibility note (2026-03-03):**
This plan preserves the original `graphshell://clip/<uuid>` wording as the historical address proposal. Runtime canonical internal routing is now `verso://...`, but clip authority remains intentionally unresolved, so the exact clip address family should be treated as pending rather than final.

## Context
Clipping allows a user to right-click a DOM element in a webview and extract it into a new, independent graph node. This node preserves the content (HTML/Image) even if the original source changes or goes offline.

---

## Architecture

### 1. The Trigger: Context Menu
The `Verso` mod (Servo embedder) must intercept context menu events.
- **Event**: `GraphSemanticEvent::ContextMenu { webview_id, coords, ... }`.
- **Payload**: Needs to carry or allow retrieval of element metadata (tag name, ID, text content).

### 2. The Extraction: Script Injection
Since Servo doesn't expose a direct "get DOM element at point" API to the embedder, we use script injection via the `EmbedderApi`.
- **Script**: `document.elementFromPoint(x, y).outerHTML` (simplified).
- **Execution**: Triggered when user selects "Clip" from the context menu.

### 3. The Data Model: Clip Node
A clip node is a regular node with specific metadata:
- **URL**: `data:text/html;base64,...` (Self-contained) OR `graphshell://clip/<uuid>` (Pointer to storage).
- **Tags**: `#clip`, `#starred`.
- **Edge**: `UserGrouped` edge from Source Node -> Clip Node.

---

## Implementation Phases

### Phase 1: Context Menu Plumbing
1.  **Extend `GraphSemanticEvent`**: Add `ContextMenu` variant.
2.  **Update `EmbedderWindow`**: Implement `handle_context_menu` delegate callback.
    -   Convert Servo coordinates to window coordinates.
    -   Emit `GraphSemanticEvent::ContextMenu`.
3.  **UI**: In `gui.rs` (or `context_menu.rs`), handle the event by showing an egui popup at the coordinates.
    -   Menu Item: "Clip Element".

### Phase 2: Content Extraction
1.  **Implement `extract_element_at(webview_id, x, y)`**:
    -   Use `webview.evaluate_script(...)`.
    -   JS Logic: Identify target element (heuristic: smallest container with text/image), get `outerHTML`, get computed styles (optional), get bounding rect.
    -   Return JSON payload to embedder.
2.  **Screenshot (Optional/Future)**: Use `webview.capture_rect(...)` if available, or full page capture + crop.

### Phase 3: Clip Node Creation
1.  **Action Handler**: When "Clip Element" is clicked:
    -   Run extraction.
    -   Generate `data:` URL from extracted HTML.
    -   Emit `GraphIntent::AddNode`.
    -   Emit `GraphIntent::TagNode { tag: "#clip" }`.
    -   Emit `GraphIntent::CreateUserGroupedEdge { from: source_node, to: new_clip_node }`.

### Phase 4: Clip Rendering
1.  **ViewerRegistry**: Ensure `viewer:webview` handles `data:` URLs correctly (Verso mod already does).
2.  **Graph View**: Update `GraphNodeShape` to render `#clip` nodes with a distinct visual (e.g., dashed border or scissor icon badge).

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
3.  **Content Fidelity**: Opening the clip node shows the extracted HTML element (isolated from original page).
4.  **Persistence**: Clip node survives restart (data URL is persisted).
5.  **Linkage**: Edge exists between Source and Clip.
6.  **Nostr publication (Phase 5)**: "Publish to Nostr" action produces a valid kind 9802 event with canonical `r` tag; `nostr_event_id` is stored on the clip node; action is absent when no keypair is configured.
