# Bookmarks And History Import Plan (Refactored 2026-02-24)

**Status**: Implementation-Ready
**Phase**: Registry Phase X (Feature Target 7)
**Architecture**: Native Mod (`ImportWizardMod`) registering actions to `ActionRegistry`.

## Context

This plan implements the "Import" capability as a self-contained **Native Mod**. It leverages the `ActionRegistry` to expose commands and the `GraphIntent` system to mutate the graph. It aligns with the unified tag system (`#starred`, user tags) and the edge traversal model.

---

## Architecture: The Import Wizard Mod

The `ImportWizardMod` is a native mod (compiled-in) that provides:
- **Actions**: `import.bookmarks_from_file`, `import.history_from_file`.
- **UI**: Uses `rfd` (Rust File Dialog) for file selection; no complex in-app wizard UI required for MVP.
- **Logic**: Parsers for standard browser formats.

### Dependencies
- `rfd`: Native file dialogs.
- `scraper` or `html5ever`: Parsing Netscape Bookmark HTML (Firefox/Safari/standard export).
- `serde_json`: Parsing Chrome/Edge JSON exports.
- `rusqlite`: Reading browser history databases (SQLite).
- `url`: URL validation and normalization.

---

## Implementation Phases

### Phase 1: Mod Scaffold & Action Registration

1.  Create `mods/native/import_wizard/mod.rs`.
2.  Implement `ModManifest` via `inventory::submit!`.
    -   `provides`: `["action:import.bookmarks", "action:import.history"]`.
3.  Register actions in `ActionRegistry`.
    -   `import.bookmarks`: Triggers file picker -> parses -> emits intents.
    -   `import.history`: Triggers file picker -> parses -> emits intents.

### Phase 2: Bookmarks Import (HTML & JSON)

**Netscape HTML (Standard)**:
-   Parse `<DT><A HREF="...">Title</A>` structure.
-   Map folder hierarchy (`<DL><p><DT><H3>Folder</H3>...`) to **User Tags**.
    -   Example: `Bookmarks Bar / Tech / Rust` -> Node tags: `Tech`, `Rust`.
-   **Intents**:
    -   `AddNode { url, title }`
    -   `TagNode { tag: "#starred" }` (All imports are treated as starred/bookmarked).
    -   `TagNode { tag: "Folder" }` (For each folder in path).

**Chrome JSON**:
-   Parse recursive JSON structure (`roots` -> `bookmark_bar` -> `children`).
-   Same tag mapping logic.

**Dedup Strategy**:
-   Rely on `GraphBrowserApp`'s `AddNode` idempotency (or check existence via `url_to_nodes` before emitting).
-   If node exists, only emit `TagNode` intents.

### Phase 3: History Import (SQLite)

**Security Constraint**:
-   Open SQLite DB in **Read-Only Mode** (`OpenFlags::SQLITE_OPEN_READ_ONLY`).
-   Do not copy the DB to app data; read in-place or copy to temp if locked.

**Query Logic**:
-   Select `url`, `title`, `last_visit_time` from `urls` table.
-   Limit to last N entries (e.g., 1000) or time range to prevent graph explosion.
-   **Intents**:
    -   `AddNode { url, title }`
    -   **No** `#starred` tag for history items.
    -   Optional: `AddTraversal` if referrer data is reliably available (often complex to reconstruct from raw history DBs; MVP can skip edges).

---

## Validation & Testing

### Manual Validation
1.  **Firefox Export**: Export bookmarks to HTML. Run `import.bookmarks`. Verify nodes appear with `#starred` and folder tags.
2.  **Chrome Export**: Export to JSON. Run `import.bookmarks`. Verify structure.
3.  **Dedup**: Import the same file twice. Verify no duplicate nodes created (node count stable), tags preserved.
4.  **History**: Point `import.history` to a copy of a browser profile `History` file. Verify recent nodes appear.

### Automated Tests (Unit)
-   `test_parse_netscape_html`: Feed sample HTML string, assert `Vec<ImportedItem>` output.
    -   Check folder stack -> tag conversion.
-   `test_parse_chrome_json`: Feed sample JSON, assert output.
-   `test_import_emits_correct_intents`: Mock `ActionContext`, run import logic, verify `GraphIntent` vector.

---

## Integration Points

-   **Settings**: Add "Import" buttons to `graphshell://settings/persistence` or a new `graphshell://settings/import` page that trigger these actions.
-   **Command Palette**: Actions are automatically available via `ActionRegistry`.
