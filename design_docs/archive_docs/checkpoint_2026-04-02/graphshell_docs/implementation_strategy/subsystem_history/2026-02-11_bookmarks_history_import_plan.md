# Bookmarks And History Import Plan

**Status**: Split note / compatibility redirect  
**Updated**: 2026-04-02

This older combined FT7 import plan is no longer the canonical place to design import work.

It has been split because the current system needs two different treatments:

- bookmark import is imported knowledge capture with provenance/import-record integration,
- browser-history import must stay separate from the live History subsystem's traversal truth.

## Canonical Successors

- [../../../../../../graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_bookmarks_import_plan.md](../../../../../../graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_bookmarks_import_plan.md) - Current bookmark-import plan aligned with import provenance, import records, imported relations, and ActionRegistry-backed invocation.
- [../../../../../../graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_browser_history_import_plan.md](../../../../../../graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_browser_history_import_plan.md) - Current browser-history import plan aligned with the active History subsystem boundary and current imported-data carriers.

## Why The Split Happened

The older combined plan predates the current imported-data and temporal-history boundaries.

Key corrections now enforced by the split:

- imported browser history must not create synthetic traversal truth,
- bookmark folder structure should use imported-data semantics rather than being forced into user tags by default,
- both flows must integrate with current import provenance and import-record surfaces.

Use the two 2026-04-02 docs for active planning.
