# Interactive HTML Export Plan (2026-02-25)

**Status**: Deferred (blocked) — concept adopted into active planning
**Phase**: Phase 3+ (after export/snapshot format and content/viewer model are stable)
**Relates to**:

- `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` §5 — Original archived concept; this doc is the resurrection into current planning
- `design_docs/archive_docs/checkpoint_2026-02-01/technical_architecture/ARCHITECTURE_DECISIONS.md` §8b — Graph JSON schema v0; snapshot field set that the HTML export reads
- `2026-02-24_immediate_priorities.md` — F10: Adopt Interactive HTML Export
- `2026-02-24_universal_content_model_plan.md` — Viewer/content metadata maturity is a blocking prerequisite
- `2026-02-25_backlog_ticket_stubs.md` §F10 — Ticket stub that triggered this document

---

## Context

The archived project philosophy described a Phase 3 goal: _"Export graphs as JSON, PNG, or interactive HTML"_. The interactive HTML path was left unspecified and deferred. This document restates the concept with a concrete artifact scope, privacy/redaction constraints, and viewer fallback rules so it does not disappear between migration work and feature execution.

This is a **planning document, not an implementation plan**. No code changes are implied until the blocking prerequisites (§Blocking Prerequisites) are resolved.

---

## Artifact Scope

### What the export artifact is

A single self-contained `.html` file that:

- Embeds the graph topology and node metadata as an inline JSON blob (the same schema used by the internal JSON export, `schema_version` field included).
- Renders the graph using a JavaScript force-directed layout (no server, no WASM build required for the viewer — plain ES-module JavaScript only).
- Allows the viewer to pan, zoom, and click nodes to see metadata.
- Clicking a node shows: title, URL (as a link), user tags, notes, and edge list.
- Works fully offline after download — no CDN dependencies; all viewer JS/CSS is inlined.

### What the export artifact is NOT

- Not a live browser session — no webview, no page loading, no Servo.
- Not a Graphshell-specific format requiring Graphshell to open — it is a standalone artifact readable in any modern browser.
- Not a two-way sync artifact — the exported HTML is a read-only snapshot.

### Offline capability rules

| Capability | Included | Notes |
| --- | --- | --- |
| Graph topology (nodes + edges) | ✅ Yes | Inline JSON in `<script>` tag |
| Node titles and URLs | ✅ Yes | Subject to redaction rules (§Privacy) |
| User tags and notes | ✅ Yes | Subject to redaction rules |
| Node positions (layout) | ✅ Yes | Saved positions from snapshot; JS layout used as fallback if absent |
| Favicons | ⚠️ Optional | Base64-inlined if present in snapshot; omitted if not |
| Thumbnails / screenshots | ❌ No (MVP) | Excluded from MVP; too large for inline artifact; deferred to Phase 3+ |
| Live page content | ❌ No | Export is a graph structure snapshot, not a web cache |
| Graphshell physics simulation | ❌ No | JS force simulation approximates layout; exact physics parity not required |

---

## Privacy and Redaction Constraints

The interactive HTML export must respect the same privacy rules as any other Graphshell export artifact. The following rules apply:

### Redaction behavior

1. **URL redaction**: If a node's URL matches a user-configured redaction pattern (e.g., `private.*`, `localhost`, intranet hostnames), the URL field is replaced with `[redacted]` in the exported artifact. The node itself is still exported (graph structure is preserved) unless the user opts for full node exclusion.
2. **Notes redaction**: Notes fields are included by default. A per-export option allows the user to strip all notes before export (useful for sharing topology without personal annotations).
3. **Tags**: All user tags are included by default. Tags that match a user-configured private-tag pattern (e.g., tags prefixed `_` or `private/`) are stripped.
4. **Traversal history / edge metadata**: Edge traversal timestamps and traversal-archive metadata are **excluded** from the interactive HTML export artifact. Only structural edge type (e.g., `Hyperlink`, `Manual`) is retained.

### Consent gate

The export flow must present a summary to the user before writing the file:
- Node count and edge count being exported.
- Count of nodes/URLs that will be redacted (if any).
- Whether notes are included.

The user must confirm before the file is written.

### Scope limit

The export artifact scope is the **current workspace snapshot** at the time of export. There is no incremental or differential export in the MVP.

---

## Viewer Fallback Rules

Because the HTML viewer is self-contained and requires no Graphshell runtime, fallback behavior must be defined for environments where the JS renderer cannot execute.

| Scenario | Fallback behavior |
| --- | --- |
| JavaScript disabled | `<noscript>` block renders a plain HTML table of nodes and edges (title, URL, tags). Full topology is still accessible. |
| Very old browser (no ES modules) | Same `<noscript>` table fallback. The inline JSON blob is still present and parseable by external tools. |
| Large graph (>500 nodes) | JS renderer emits a warning banner; user can switch to table-only view via a toggle button. |
| Missing positions in snapshot | JS force simulation places nodes automatically; a notice informs the viewer that positions are approximate. |

The `<noscript>` table fallback is a hard requirement for the MVP. The interactive JS view is the preferred experience but must not be the only path to the data.

---

## Blocking Prerequisites

This plan is **deferred** until both of the following are resolved:

1. **Export/snapshot artifact strategy clarified for viewer/content metadata** — The `Node` content model must stabilize (including `mime_hint`, `viewer_id_override`, and notes field ownership) before the export schema can be finalized. See `2026-02-24_universal_content_model_plan.md`.

2. **Privacy/redaction behavior defined for export path** — Redaction configuration (patterns, private-tag rules, consent gate UX) must be specified in a dedicated privacy/redaction design before the export flow can be implemented. The rules in §Privacy above are the draft scope; they require review against the settings architecture (`2026-02-20_settings_architecture_plan.md`) before implementation.

---

## Adoption Trigger

This concept should be pulled into active implementation planning after:

- `2026-02-24_universal_content_model_plan.md` Steps 1–3 are complete (stable `Node` content model).
- A settings-backed redaction configuration surface exists (from `2026-02-20_settings_architecture_plan.md` or successor).
- The JSON export format (already defined in `ARCHITECTURE_DECISIONS.md` §8b) is confirmed as the snapshot input to the HTML generator.

---

## Implementation Sketch (Post-Unblock)

The following is a high-level sketch for when prerequisites are met. It is not a task list.

1. **`mods/native/html_export/mod.rs`** — Native mod registering `action:export.interactive_html`.
2. **Export intent** — `GraphIntent::ExportInteractiveHtml { redact_urls: Vec<Pattern>, strip_notes: bool }`.
3. **Snapshot extraction** — Read current `GraphWorkspace` snapshot; apply redaction rules; produce a `HtmlExportPayload` (serialized node/edge JSON + layout positions).
4. **HTML generation** — Template-fill the payload into a single `.html` file: inline JSON blob + inline JS renderer + inline CSS + `<noscript>` table fallback.
5. **File write** — Use `rfd` native dialog to let user choose save path; write single file.
6. **JS renderer** — A minimal force-directed graph viewer (D3.js-style, inlined and minified); no external dependencies.

---

## Definition of Done (for this planning doc)

- [x] Export concept is restated in a current (non-archived) strategy doc.
- [x] Artifact scope (offline capabilities, included/excluded fields) is documented.
- [x] Privacy/redaction rules are drafted (redaction patterns, notes stripping, consent gate, traversal exclusion).
- [x] Viewer fallback rules are specified (`<noscript>` table, large-graph warning, missing-position notice).
- [x] Blocking prerequisites are named explicitly.
- [x] Adoption trigger conditions are stated.
