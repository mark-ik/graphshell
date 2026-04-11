# Browser Import Normalized Carrier Sketch

**Date**: 2026-04-11  
**Status**: Design sketch / follow-on to bookmark and browser-history import plans  
**Scope**: Define normalized import carriers for browser bookmarks, browser history, and browser live-session snapshots so host-specific readers can share one reducer-facing boundary without collapsing distinct truth models.

**Related**:

- [2026-04-02_bookmarks_import_plan.md](2026-04-02_bookmarks_import_plan.md)
- [2026-04-02_browser_history_import_plan.md](2026-04-02_browser_history_import_plan.md)
- [../graph/2026-03-14_graph_relation_families.md](../graph/2026-03-14_graph_relation_families.md)
- [../system/register/action_registry_spec.md](../system/register/action_registry_spec.md)
- [../../technical_architecture/2026-03-29_middlenet_engine_spec.md](../../technical_architecture/2026-03-29_middlenet_engine_spec.md)

---

## Goal

Graphshell should accept browser-derived data from multiple source strategies:

- bookmark export files,
- copied profile databases,
- native profile readers,
- extension/native-host session bridges.

Those source readers should not mutate graph state directly. They should emit a normalized carrier layer that:

- preserves source identity and import-run metadata,
- distinguishes bookmark, history, and live-session semantics,
- gives reducers enough structure to create imported/provenance-backed graph state,
- and avoids smuggling browser-local timeline/session truth into Graphshell's live traversal or workbench authority.

## Design Rules

1. One normalized import boundary, multiple source readers.
2. Bookmarks, history, and live sessions share an envelope but not a semantic payload.
3. Imported browser data is external knowledge capture by default.
4. Live browser session state is not the same thing as Graphshell's current workbench session.
5. Imported history must not become live traversal truth.
6. Browser/session carriers may capture richer structure than MVP reducers choose to project.

## Proposed Layering

### Source reader layer

Host-specific code reads:

- Netscape bookmark HTML,
- Chrome-style bookmark JSON,
- Places/History SQLite copies,
- Firefox recovery/session files,
- Chromium session files,
- extension/native-messaging tab snapshots.

This layer is allowed to know browser-specific schemas and file formats.

### Normalized carrier layer

All readers emit typed normalized carriers.

This layer is browser-agnostic and reducer-facing.

### Reducer/projection layer

Graphshell reducers decide how much of each carrier becomes:

- URL-backed nodes,
- import records,
- provenance attachments,
- imported-family relations,
- optional workbench/session affordances.

## Shared Envelope Types

Rust-like sketch:

```rust
pub enum BrowserImportPayload {
    Bookmark(ImportedBookmarkItem),
    HistoryVisit(ImportedHistoryVisitItem),
    SessionSnapshot(ImportedBrowserSessionItem),
}

pub struct BrowserImportBatch {
    pub run: BrowserImportRun,
    pub items: Vec<BrowserImportPayload>,
}

pub struct BrowserImportRun {
    pub import_id: String,
    pub source: BrowserImportSource,
    pub mode: BrowserImportMode,
    pub observed_at_unix_secs: i64,
    pub user_visible_label: String,
}

pub enum BrowserImportMode {
    OneShotFile,
    OneShotProfileRead,
    SnapshotBridge,
    IncrementalBridge,
}

pub struct BrowserImportSource {
    pub browser_family: BrowserFamily,
    pub profile_hint: Option<String>,
    pub source_kind: BrowserImportSourceKind,
    pub stable_source_id: Option<String>,
}

pub enum BrowserFamily {
    Chrome,
    Chromium,
    Edge,
    Brave,
    Arc,
    Firefox,
    Safari,
    Other(String),
}

pub enum BrowserImportSourceKind {
    BookmarkFile,
    HistoryDatabase,
    SessionFile,
    NativeProfileReader,
    ExtensionBridge,
    NativeMessagingBridge,
}

pub struct ImportedPageSeed {
    pub canonical_url: String,
    pub normalized_title: Option<String>,
    pub raw_url: Option<String>,
    pub raw_title: Option<String>,
    pub favicon_url: Option<String>,
}
```

### Why this shared envelope exists

- `BrowserImportRun` gives import-record creation a stable source of truth.
- `BrowserImportSource` lets Navigator/imported-data UI explain where the data came from.
- `ImportedPageSeed` centralizes URL/title normalization and dedupe keys.
- `BrowserImportMode` keeps one-shot file import distinct from live bridge import.

## Bookmark Carrier

Rust-like sketch:

```rust
pub struct ImportedBookmarkItem {
    pub page: ImportedPageSeed,
    pub bookmark_id: Option<String>,
    pub folder_path: Vec<ImportedFolderSegment>,
    pub location: BookmarkLocation,
    pub created_at_unix_secs: Option<i64>,
    pub modified_at_unix_secs: Option<i64>,
    pub tags: Vec<String>,
}

pub struct ImportedFolderSegment {
    pub stable_id: Option<String>,
    pub label: String,
    pub position: usize,
}

pub enum BookmarkLocation {
    Toolbar,
    Menu,
    Other,
    Unknown,
}
```

### Reducer expectations

- Merge or create the URL-backed node.
- Create/update import provenance and import-record membership.
- Apply bookmark-specific MVP semantics such as `#starred`.
- Project folder hierarchy through imported-family structure, not traversal truth.

### Graph mapping

- Existing imported relation support already fits bookmark folder membership via `ImportedSubKind::BookmarkFolder`.
- No new truth family is needed for bookmark import MVP.

## History Carrier

Rust-like sketch:

```rust
pub struct ImportedHistoryVisitItem {
    pub page: ImportedPageSeed,
    pub visit_id: Option<String>,
    pub visited_at_unix_secs: i64,
    pub visit_count_hint: Option<u32>,
    pub transition: Option<HistoryTransitionKind>,
    pub referring_url: Option<String>,
    pub session_context: Option<ExternalSessionContext>,
}

pub enum HistoryTransitionKind {
    Link,
    Typed,
    AutoBookmark,
    AutoSubframe,
    Reload,
    Redirect,
    Generated,
    Other(String),
}

pub struct ExternalSessionContext {
    pub external_window_id: Option<String>,
    pub external_tab_id: Option<String>,
}
```

### Reducer expectations

- Merge or create the URL-backed node.
- Attach imported-history provenance and import-record membership.
- Optionally aggregate last-visited / frequency hints in imported metadata.
- Do not create Graphshell traversal append records.
- Do not create live `History` family edges.

### Graph mapping

- Existing imported-family support already fits imported-history grouping via `ImportedSubKind::HistoryImport`.
- `referring_url` is useful capture data but should not become a traversal edge in MVP.

## Live Session Carrier

The browser-session case is different. It is neither a bookmark nor a generic visit row.

It is a snapshot of another browser's currently open work: windows, tabs, active tabs, and possibly per-tab navigation stacks.

Rust-like sketch:

```rust
pub struct ImportedBrowserSessionItem {
    pub snapshot_id: String,
    pub observed_at_unix_secs: i64,
    pub windows: Vec<ImportedBrowserWindow>,
}

pub struct ImportedBrowserWindow {
    pub external_window_id: Option<String>,
    pub ordinal: usize,
    pub tabs: Vec<ImportedBrowserTab>,
    pub focused: bool,
}

pub struct ImportedBrowserTab {
    pub page: ImportedPageSeed,
    pub external_tab_id: Option<String>,
    pub ordinal: usize,
    pub active: bool,
    pub pinned: bool,
    pub audible: bool,
    pub opener_url: Option<String>,
    pub navigation: Vec<ImportedNavigationEntry>,
    pub active_navigation_index: Option<usize>,
}

pub struct ImportedNavigationEntry {
    pub url: String,
    pub title: Option<String>,
    pub ordinal: usize,
    pub visited_at_unix_secs: Option<i64>,
}
```

### Why session gets its own carrier

- The top-level unit is a window/tab snapshot, not an individual bookmark or visit.
- Per-tab navigation stacks may exist even when there is no durable history export.
- Extension/native-messaging bridges naturally produce snapshots or deltas, not bookmark trees.

### Reducer expectations

MVP should treat session import as imported graph material first, not as authoritative workbench state. The safe minimum is:

- merge or create nodes for active tab URLs,
- attach import provenance and membership,
- preserve window/tab grouping in the import record,
- optionally create imported-family grouping edges for window and tab membership,
- keep the entire session snapshot available for later projection.

### Recommended graph mapping

Current imported sub-kinds cover bookmark folders and history import, but not browser-session snapshot structure.

Follow-on addition recommended:

```rust
ImportedSubKind::SessionImport
```

This avoids overloading `HistoryImport` for window/tab grouping and avoids pretending a browser window is a bookmark folder.

## Delta vs Snapshot

For live browser integration there are two viable shapes:

### Snapshot-first

The host emits complete session snapshots at explicit moments:

- on demand,
- on browser focus change,
- on timer,
- or after meaningful tab changes.

This is simpler, deterministic, and easiest to map to import records.

### Delta follow-on

Later, a bridge may emit incremental events such as:

```rust
pub enum BrowserSessionDelta {
    WindowOpened { external_window_id: String },
    WindowClosed { external_window_id: String },
    TabOpened { external_window_id: String, tab: ImportedBrowserTab },
    TabClosed { external_tab_id: String },
    TabActivated { external_tab_id: String },
    TabNavigated {
        external_tab_id: String,
        entry: ImportedNavigationEntry,
    },
}
```

This should be treated as a follow-on only after the snapshot carrier and import-record projection are stable.

## What Existing Architecture Makes Cheap

### Already cheap

- ActionRegistry-backed explicit import entry points.
- Import records and provenance surfaces.
- Imported-family relations for bookmark/history projections.
- JSON-friendly intent boundaries for extension/PWA hosts.
- Viewer/Protocol separation, which keeps import logic out of viewer runtime.

### Not yet free

- A dedicated imported-session relation sub-kind.
- A canonical imported-history metadata carrier for counts/recency/ranking.
- A stable external-browser bridge for live snapshots/deltas.

## Recommended First Implementation Order

1. Land the normalized carrier layer for bookmarks and history.
2. Keep live browser session import snapshot-only at first.
3. Add `ImportedSubKind::SessionImport` before projecting browser window/tab structure into the graph.
4. Defer live session deltas until import records and imported-session UI are stable.

## Done Gate For This Sketch

This sketch is successful when implementation can follow one clear rule:

- source-specific readers produce normalized carriers,
- reducers consume normalized carriers,
- and Graphshell's existing graph/history/workbench authority boundaries remain intact.