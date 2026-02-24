# Clipping & DOM Extraction Plan (Refactored 2026-02-24)

**Status**: Implementation-Ready
**Phase**: Registry Phase X (Feature Target 9)
**Architecture**: Extension of `Verso` Native Mod + `GraphSemanticEvent`.

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

## Validation

1.  **Right-Click**: Context menu appears at correct coordinates over webview.
2.  **Extraction**: "Clip" creates a new node.
3.  **Content Fidelity**: Opening the clip node shows the extracted HTML element (isolated from original page).
4.  **Persistence**: Clip node survives restart (data URL is persisted).
5.  **Linkage**: Edge exists between Source and Clip.
