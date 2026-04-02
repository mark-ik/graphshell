# Bookmarks Import Plan

**Date**: 2026-04-02  
**Status**: Planned / currentized split from `2026-02-11_bookmarks_history_import_plan.md`  
**Scope**: Import browser bookmark exports as imported graph knowledge without conflating imported structure with live traversal truth.

**Related**:

- [SUBSYSTEM_HISTORY.md](SUBSYSTEM_HISTORY.md)
- [../graph/2026-03-11_graph_enrichment_plan.md](../graph/2026-03-11_graph_enrichment_plan.md)
- [../graph/node_badge_and_tagging_spec.md](../graph/node_badge_and_tagging_spec.md)
- [../system/register/action_registry_spec.md](../system/register/action_registry_spec.md)
- [../../technical_architecture/GRAPHSHELL_AS_BROWSER.md](../../technical_architecture/GRAPHSHELL_AS_BROWSER.md)

---

## Goal

Import browser bookmark exports into Graphshell as durable imported graph content that:

- creates or merges URL-backed nodes,
- preserves source provenance and import-record membership,
- optionally preserves bookmark-folder structure,
- makes imported data visible through existing imported-data surfaces,
- and avoids inventing fake traversal/history truth.

This plan is the bookmark half of the older combined FT7 import plan.

## Non-Goals

- Do not write synthetic traversal records or history timeline entries.
- Do not overload bookmark-folder structure into live History subsystem semantics.
- Do not assume imported folders must always become user-authored tags.

## Current-System Fit

The old combined import plan predated the current imported-data model. The bookmark import path now needs to align with:

- node import provenance,
- normalized import-record membership,
- imported relation families/sub-kinds,
- ActionRegistry-backed command surfaces,
- Navigator/omnibar imported-data projections.

The import seam can still be a native mod plus ActionRegistry entry, but the persisted graph shape must use the current import carriers rather than only `AddNode` + tags.

## Canonical MVP Model

### 1. Invocation Surface

- Provide `import.bookmarks_from_file` through a native import mod or equivalent registry-owned host action.
- Surface it in ActionRegistry-backed command surfaces first.
- Additional UI entry points may be added later, but command-surface invocation is the baseline requirement.

### 2. Accepted Inputs

- Netscape bookmark HTML (Firefox/Safari/common export format).
- Chrome/Edge bookmark JSON exports.

### 3. Node Merge Rules

- Normalize and validate URLs before graph mutation.
- Merge against existing nodes by canonical URL/address-kind policy.
- When a node already exists, enrich it rather than duplicating it.

### 4. Imported Data Carriers

For each imported bookmark membership:

- attach node import provenance with a stable source identity and user-facing source label,
- update or create an import record for the import run/source,
- mark membership in that import record,
- apply `#starred` to imported bookmark nodes for MVP bookmark semantics.

### 5. Folder Structure Representation

Bookmark folders are imported structure, not automatically user-authored taxonomy.

Default MVP policy:

- preserve folder hierarchy through imported relations where useful,
- use `ImportedSubKind::BookmarkFolder` for imported folder membership semantics,
- do **not** automatically convert every folder path segment into a user tag.

Optional later enhancement:

- offer an explicit opt-in to materialize folder segments as tags after import.

This keeps imported organization separate from intentional user classification.

## Implementation Slices

### Slice A: Parser and Import Carrier

- Add bookmark parser support for Netscape HTML and Chrome/Edge JSON.
- Produce a normalized intermediate carrier such as `ImportedBookmarkItem` with:
  - canonical URL,
  - title,
  - source metadata,
  - folder-path segments,
  - stable import-run grouping metadata.

### Slice B: Graph Merge and Provenance

- Route execution through an explicit action -> intent -> reducer boundary.
- For each imported item:
  - create or merge the target node,
  - set or refine title when the imported title is better than current blank/default state,
  - attach import provenance,
  - update import-record membership,
  - apply `#starred`.

### Slice C: Imported Folder Semantics

- Preserve folder-path information without polluting traversal truth.
- If folder carriers are represented as nodes/edges in MVP, ensure they land in the imported relation family rather than semantic/history families.
- If folder projection is deferred, retain the folder path in the normalized import pipeline so relation projection can be added later without reparsing source files.

### Slice D: Imported-Data Surfaces

- Ensure imported bookmark data is visible through the existing imported-data projections:
  - Navigator imported grouping,
  - omnibar/import snippets,
  - graph/node provenance UI.
- Do not require a bespoke wizard UI for MVP beyond file selection and explicit success/failure reporting.

## Diagnostics and Safety

- Report parse failure, file format mismatch, and URL normalization failure explicitly.
- Do not silently drop malformed items without surfaced counts/diagnostics.
- Treat imported files as untrusted input.
- Never execute, navigate, or dereference imported URLs during parsing.

## Validation

### Manual

1. Import a Firefox HTML export and verify nodes appear as imported bookmarks with stable source labeling.
2. Import a Chrome/Edge JSON export and verify the same merge/provenance behavior.
3. Re-import the same file and verify nodes do not duplicate while import membership/provenance stays coherent.
4. Verify imported bookmarks surface in imported-data UI projections instead of masquerading as history timeline rows.

### Automated

- Parser tests for Netscape HTML and Chrome/Edge JSON.
- Merge tests for repeated import of the same canonical URL.
- Provenance/import-record tests ensuring imported memberships persist through snapshot round-trip.
- Regression test ensuring bookmark import does not create `History` edges or traversal append records.

## Done Gate

This plan is complete when:

- bookmark export files can be imported from an ActionRegistry-backed command,
- imported bookmark nodes merge into the current graph model without duplication,
- source provenance and import-record membership are preserved,
- imported bookmark structure is represented through imported-data semantics rather than history semantics,
- and imported bookmarks are visible through current imported-data surfaces.
