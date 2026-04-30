<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Browser Amenities Spec

**Date**: 2026-04-29
**Status**: Canonical / Active — fourth concrete S2 deliverable for the iced jump-ship plan
**Scope**: Per-amenity specifications for the eight browser amenities reshaped
to the graph paradigm in [`2026-04-28_iced_jump_ship_plan.md` §4.6](2026-04-28_iced_jump_ship_plan.md):
History, Bookmarks, Find-in-page / find-in-graph, Downloads, Devtools,
Sessions / restore, Profiles, Multi-window. Plus the implicit ninth amenity
(Frametree) added in the §4.6 update. Each section names: surface (where
the amenity lives), data source (which authority owns the truth), intent
flow (uphill routing), and `verso://` address (where applicable).

**Code-sample mode**: **Illustrative signatures**. Concrete implementation
lives in S3/S4 code, not this spec.

**Related**:

- [`2026-04-28_iced_jump_ship_plan.md` §4.6](2026-04-28_iced_jump_ship_plan.md) — canonical reshape table with Presentation Bucket assignments
- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — slot model, three Navigator buckets, canvas instances
- [`iced_omnibar_spec.md`](iced_omnibar_spec.md) — URL-entry surface (revised 2026-04-29; no longer the find-in-graph entry point)
- [`iced_node_finder_spec.md`](iced_node_finder_spec.md) — Ctrl+P fuzzy graph-node search (added 2026-04-29; this is now the find-in-graph surface)
- [`iced_command_palette_spec.md`](iced_command_palette_spec.md) — find-in-page / find-in-graph as palette-routable actions
- [`../navigator/NAVIGATOR.md` §8](../navigator/NAVIGATOR.md) — Presentation Bucket Model
- [`../subsystem_history/SUBSYSTEM_HISTORY.md`](../subsystem_history/SUBSYSTEM_HISTORY.md) — traversal / history authority
- [`../subsystem_storage/SUBSYSTEM_STORAGE.md`](../subsystem_storage/SUBSYSTEM_STORAGE.md) — settings / profile persistence
- [`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) — security context for downloads, permissions
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — Verso internal address scheme, projection vocabulary

---

## 1. Intent

Browser amenities (history, bookmarks, find, downloads, devtools, etc.)
exist in every browser. The §4.6 reshape established that Graphshell does
*not* copy chrome-shaped UI for them — each amenity reshapes to the graph
paradigm. This spec answers, per amenity, the four questions S2 said had to
be answered before S4 implementation: **surface** (which iced widget /
slot), **data source** (which authority holds truth), **intent flow** (the
uphill route for any mutation), and **`verso://` address** (where the
amenity has an addressable surface).

Each section is short — one canonical answer per question. Anything
deeper (provider catalogs, ranking algorithms, retention policies) lives
in the subsystem-specific specs referenced inline.

The default expectation: an amenity that did not require surface-level
specification beyond §4.6's table row gets a brief restatement here, plus
the four answers.

---

## 2. History

§4.6 reshape: *Two graph-native systems — (1) edge history (traversal /
origin edges); (2) graph memory (per-node memory tree via `graph-memory`
crate). History view is a Navigator projection.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | Activity Log (recency lane + traversal events) |
| **Surface** | The Navigator Activity Log bucket renders the time-ordered traversal stream. A dedicated History Manager tool pane (`verso://tool/history`) provides the full Timeline / Dissolved tabs view (per TERMINOLOGY.md §History Manager) for richer queries. |
| **Data source** | SUBSYSTEM_HISTORY owns the traversal event stream and the recency aggregate (per [SUBSYSTEM_HISTORY.md](../subsystem_history/SUBSYSTEM_HISTORY.md)). `graph-memory` provides the per-node memory tree. Edge history is graph-backed via `EdgePayload.kinds` carrying `EdgeKind::TraversalDerived` plus `Traversal` event records. |
| **Intent flow** | Read-only at the Navigator surface. Clicking a recency entry emits a Navigation/Reveal intent that routes to the relevant Pane. Deletion of a traversal event (rare; e.g., for privacy) emits a `HistoryIntent::ForgetTraversal { traversal_id }` to SUBSYSTEM_HISTORY; this is the only way Activity Log entries are ever removed. |
| **`verso://` address** | `verso://tool/history` for the History Manager tool pane. Individual traversal events do not carry their own addresses — they reference node addresses. |

### 2.1 What constitutes a "visit" event for the edge-history writer

A `Traversal` is recorded whenever a node enters Active presentation state
in any Pane, with `NavigationTrigger` indicating the cause. Specifically:

- `LinkClick`, `BackButton`, `ForwardButton`, `AddressBarEntry`,
  `Programmatic`, `Hyperlink` — recorded as a Traversal with the
  trigger.
- `PanePromotion` (egui-era) — superseded by direct Active
  presentation state transition; the new equivalent is "node entered
  Active in a Pane" with `NavigationTrigger::Activated`.
- Hover, scaffold selection, and viewport pan/zoom on the canvas
  do **not** emit Traversal events.
- Deduplication policy: consecutive Active transitions for the same
  node within a 5-second window collapse to one Traversal entry. The
  exact dedupe window is a SUBSYSTEM_HISTORY policy, configurable via
  settings.

### 2.2 Activity Log row shape

```rust
// Illustrative.
pub struct ActivityLogEntry {
    pub timestamp: Instant,
    pub kind: ActivityEventKind,
    pub primary_target: Option<NodeKey>,        // for Navigate / Activate
    pub secondary_target: Option<NodeKey>,      // for edges (source-target)
    pub label: String,
    pub source: ActivityEventSource,            // SUBSYSTEM_HISTORY | runtime | graph | shell
}

pub enum ActivityEventKind {
    Traversal { trigger: NavigationTrigger },
    Lifecycle { from: NodeLifecycle, to: NodeLifecycle },
    GraphMutation { mutation: GraphMutationDescriptor },
    ImportEvent { source: ImportSource },
    FrameSnapshotSaved { frame_id: FrameId },
    DownloadEvent { event: DownloadEventKind },
}
```

The Activity Log is read-only; click-to-reveal is the only interaction.

---

## 3. Bookmarks

§4.6 reshape: *Tagged graphlets. A "bookmark" is a graphlet tagged as such,
with node references that can recreate nodes across graphs.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | Tree Spine (graphlet sections) + Swatches (saved graphlet previews) |
| **Surface** | Tree Spine renders bookmark-tagged graphlets as a category in the spine; clicking expands to the graphlet's tile list. Swatches bucket renders saved graphlet previews. The CommandBar's "Bookmark this…" command emits a tagging intent. |
| **Data source** | Graph truth. A graphlet has a `tag` field carrying `#bookmark` (and optionally other tags); the bookmark catalog is a query over graph nodes for graphlets carrying `#bookmark`. No separate bookmark store. |
| **Intent flow** | "Bookmark current graphlet" → `GraphIntent::TagGraphlet { graphlet_id, tag: "#bookmark" }`. "Unbookmark" → `GraphIntent::UntagGraphlet`. "Open bookmark" → `WorkbenchIntent::OpenGraphletInPane { graphlet_id }`. All intents route through the canonical command authority and surface in the Activity Log. |
| **`verso://` address** | None for the bookmark concept itself (bookmarks are graphlets, which already carry node addresses). Folders map to nested graphlets — same address scheme. |

### 3.1 Graphlet-tag schema

Per the iced jump-ship plan §10 Q6 (confirmed):

- A bookmark graphlet carries the canonical tag `#bookmark`.
- Folder hierarchy maps to nested graphlets — a parent graphlet
  contains other graphlets as members; the parent carries
  `#bookmark-folder` to identify it as a folder rather than a leaf
  bookmark.
- User-defined tags (e.g., `#research`, `#archived`) coexist with
  the canonical bookmark tag set.
- Re-tagging a graphlet from `#bookmark` to nothing makes it cease to
  appear in the bookmark catalog without deleting the graphlet.

### 3.2 Cross-graph node references

Per the iced jump-ship plan §10 Q6:

- A bookmark graphlet may reference nodes that exist in a different
  `GraphId` than the user's current Workbench.
- Activating a cross-graph bookmark either (a) opens the target
  Workbench/`GraphId` in a new Frame slot, or (b) materializes the
  referenced nodes in the current `GraphId` if the user grants this.
  The default is (a).
- Cross-graph identity uses the canonical address (e.g.,
  `https://example.com/page`) plus the source graph's `GraphId`. If
  the same URL exists in both source and current graph, they are
  treated as the same node by address-as-identity.

### 3.3 Browser-bookmark import

Importing browser bookmarks is a one-shot graph population command:

- Surface: a Settings pane (`verso://settings/import`) with an
  "Import bookmarks" form.
- Data: the user's browser-export file (HTML, JSON, etc.) parsed
  by an importer.
- Intent flow: each parsed bookmark emits one or more
  `GraphIntent::CreateNode` and `GraphIntent::AddToGraphlet`
  intents; the resulting graph state surfaces in the Activity Log
  as a single import event with link expansions.
- No separate bookmark-import service runs in `graphshell-runtime`;
  the import is a one-shot graph write.

Browser-history import is a related path; it requires
parsing-level semantic autotagging (per §10 Q6) and is deferred
beyond the first iced bring-up.

---

## 4. Find — In-Page and In-Graph

§4.6 reshape: *Graph-scoped find — search-in-pane (current viewer's
content) and search-in-graph (across nodes). Both Shell-owned commands.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | n/a — modal command surface, not a Navigator bucket |
| **Surface** | Find-in-pane: `Ctrl+F` opens an inline find toolbar in the focused tile pane (or canvas pane). Find-in-graph: `Ctrl+P` opens the Node Finder Modal (per the 2026-04-29 omnibar-split simplification — was previously an omnibar prefix mode, now its own surface). |
| **Data source** | Find-in-pane: the focused pane's viewer (Servo, middlenet, wry, tool pane) owns the find API; the Shell command dispatches to the viewer. Find-in-graph: the canonical graph index (in `graphshell-runtime`) via `NodeFinderViewModel::rank_for_query`; searches over node title / tag / address / content snapshot. |
| **Intent flow** | Find-in-pane: `ViewerIntent::FindInPage { pane_id, query }` routes to the focused pane's viewer. Find-in-graph (Node Finder): activation on a selected result emits `WorkbenchIntent::OpenNode { node_key, destination }`. Search results return via the Node Finder's Subscription with request-id supersession (per `iced_node_finder_spec.md`). |
| **`verso://` address** | None — find is a transient command surface, not a tool pane. The find toolbar is a per-Pane affordance with no addressable identity. |

### 4.1 Find-in-page toolbar shape

```rust
fn find_in_page_toolbar(pane: &Pane, find_state: &FindInPageState) -> Element<Message> {
    row![
        text_input(&find_state.query, Message::FindInPageQuery)
            .on_submit(Message::FindInPageNext),
        button("Prev").on_press(Message::FindInPagePrev),
        button("Next").on_press(Message::FindInPageNext),
        text(format!("{} of {}", find_state.match_index + 1, find_state.match_count)),
        button("Close").on_press(Message::FindInPageClose),
    ]
    .into()
}
```

Mounted inside the tile pane's chrome (above the viewer body). Routes
all matching/highlighting via `ViewerIntent::FindInPage` to the focused
pane's viewer, which performs the actual search and exposes match count
plus active match index back via Subscription.

### 4.2 Find-in-graph entry — Node Finder (revised 2026-04-29)

Per the omnibar-split simplification, find-in-graph is the **Node
Finder**'s responsibility, not the omnibar's. See
[`iced_node_finder_spec.md`](iced_node_finder_spec.md) for the full
specification.

- **Trigger**: `Ctrl+P` (canonical, Zed/VSCode-shaped) opens the Node
  Finder Modal. There is no omnibar prefix syntax for graph search;
  the omnibar is URL-entry only.
- **Surface**: Modal overlay with `text_input` + flat ranked list of
  graph nodes. Results match across (title, tag, address, content
  snapshot).
- **Result rows**: each row shows node title, address, node-type
  badge, match-source badge (Title / Tag / URL / Content), optional
  content-match snippet.
- **Activation**: Enter on focused row emits a single
  `WorkbenchIntent::OpenNode { node_key, destination }` per the
  user's `WorkbenchProfile` rule (active Pane / new Pane / replace
  focused Pane).
- **Empty query**: shows recently-active nodes (recency-ranked).
- **Footer fallback**: "Open as URL…" routes the typed text to the
  omnibar in Input mode for the user who wanted URL-entry after all.
- **No tool-pane required for "show all results"** — the Node Finder
  Modal *is* the result surface; persistent saved searches are a
  Stage F enhancement (graphlet save) covered in §4.2 open items.

Earlier draft of this row had find-in-graph as an omnibar-prefix
behavior; the omnibar-split simplification moved it to the Node Finder.

### 4.2.1 Find-in-graph open items

- **Saved searches as graphlets**: persisting a Node Finder query +
  its result set as a graphlet (with `#search-result` tag) for
  later re-execution; runtime/settings concern.
- **Per-source filter chips** in the Node Finder: filter results by
  Title / Tag / URL / Content match source.
- **Cross-graph search**: searching nodes across multiple `GraphId`s
  in the active profile catalog. Deferred per the multi-window /
  multi-profile boundaries (§9 / §8).

---

## 5. Downloads

§4.6 reshape: *A subsystem pane (`TileKind::Tool`) addressable as
`verso://tool/downloads`. Each download is a graph node.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | Activity Log (download events) + tool pane for in-progress chrome |
| **Surface** | Tool pane at `verso://tool/downloads` shows the list of in-progress and recent downloads with progress bars, source URLs, file names, and cancel/retry/reveal-in-graph affordances. The Activity Log bucket also surfaces download events as entries (start, progress milestone, complete, fail). The omnibar shows a small downloads-active indicator when ≥ 1 download is in progress; clicking opens the tool pane. |
| **Data source** | The downloads subsystem owns in-progress state (bytes transferred, ETA, error state). Each completed download is materialized as a graph node with `address_kind = File` (or as a `verso://clip/<uuid>` for non-file content); the graph is the long-term record. |
| **Intent flow** | "Cancel download" → `DownloadIntent::Cancel { download_id }`. "Retry" → `DownloadIntent::Retry { download_id }`. "Reveal in graph" → `WorkbenchIntent::OpenNodeInPane { node_key }`. "Tombstone download record" → `GraphIntent::Tombstone { node_key }` (downloads, once completed, are graph nodes; their record follows the same lifecycle as any other node). |
| **`verso://` address** | `verso://tool/downloads` for the tool pane; per-download nodes follow the address-as-identity rule (file URL, http URL, or `verso://clip/<uuid>`). |

### 5.1 In-progress chrome

The downloads tool pane is the canonical place for in-progress UX:

- File name (and rename affordance if user-driven)
- Source URL (clickable to open the source page)
- Bytes transferred + total + ETA
- Speed indicator
- Cancel / Pause / Resume / Retry buttons
- "Reveal in graph" button (jumps to the node once download completes)

The toast subsystem (per [iced jump-ship plan §12.2](2026-04-28_iced_jump_ship_plan.md)) shows
short-lived toasts on download start, completion, and failure. The toast
includes a "Open downloads" link that opens `verso://tool/downloads`.

### 5.2 Coherence with the canonical guarantees

Downloads is a Tool Pane; per the [coherence guarantee for tool panes
in §4.10](2026-04-28_iced_jump_ship_plan.md):

> Tool panes are observers, not authorities. They surface state from
> their owning subsystems […] and emit intents to those subsystems'
> authorities; they never bypass the uphill rule.

The downloads pane shows the downloads subsystem's state and emits
`DownloadIntent`s. It does not mutate graph state directly except via
the standard graph-node materialization path (which is a graph
authority operation routed through `GraphReducerIntent`).

---

## 6. Devtools

§4.6 reshape: *A single tool pane with sections — graphshell-level UX
overview and inspector (backed by `ux-probes`/`ux-bridge`), plus
subsystem-specific sections. No separate devtools-family of panes.
Servo's devtools remain reachable via the Servo subsystem tool pane.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | n/a — tool pane, not a Navigator bucket |
| **Surface** | One tool pane at `verso://tool/devtools` with a sidebar of sections: UxTree Inspector (ux-probes-backed), Channel Inspector (DiagnosticsRegistry ring buffer), Intent Inspector (sanctioned-writes log), Engine Inspector (graphshell-runtime tick metrics). Each subsystem may expose its own tool pane (e.g., `verso://tool/diagnostics` is the Diagnostic Inspector subsystem pane); the devtools pane is a top-level shell over those. |
| **Data source** | Each section reads from its source subsystem via Subscription (`ux-probes` / `DiagnosticsRegistry` / `GraphshellRuntime`). The devtools pane owns no state itself. |
| **Intent flow** | Mostly read-only; some sections expose actions (clear ring buffer, export trace, snapshot UxTree). Each action emits a subsystem-specific intent (`DiagnosticsIntent::ClearChannel`, `UxProbeIntent::ExportTrace`, etc.). Servo's devtools (page-level web devtools) remain reachable through the Servo subsystem tool pane and run inside Servo's existing devtools pipeline; no Graphshell-level routing. |
| **`verso://` address** | `verso://tool/devtools` for the top-level pane; subsystem-specific tool panes use their own addresses (`verso://tool/diagnostics`, `verso://tool/intents`, etc.). |

The devtools pane is shell-shaped (sections, search, filters) but the
content of each section comes from its source subsystem, projected via
Subscription. The pane is a host, not an authority.

---

## 7. Sessions / Restore

§4.6 reshape: *The graph IS the session. Opening the app shows the graph;
tile state (active/inactive per node) is recoverable from graph + last-known
graphlet projection.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | Tree Spine (frametree restore) — the user sees the previous Frame composition rendered in the spine on app launch |
| **Surface** | No dedicated session-restore UI. On launch, Shell loads the most-recent Frame Snapshot (per TERMINOLOGY.md §Frame Snapshot) and renders it. The Tree Spine bucket shows the frametree as it was, with previously-Active tiles still Active. |
| **Data source** | Frame Snapshot persistence (Shell-owned writes to the persistence layer per [SUBSYSTEM_STORAGE.md](../subsystem_storage/SUBSYSTEM_STORAGE.md)). The graph itself is the long-term truth; Frame Snapshots are derived restore-points that reference graph nodes by address. |
| **Intent flow** | "Save Frame Snapshot" → `ShellIntent::SaveFrameSnapshot { frame_id }`. "Restore Frame Snapshot" → `ShellIntent::RestoreFrameSnapshot { snapshot_id }`. Auto-save on app exit + clean shutdown is a Shell responsibility (no user intent required). |
| **`verso://` address** | `verso://frame/<FrameId>` per TERMINOLOGY.md §Verso internal address scheme; the Frame node's `frame-member` edges identify the composed workbenches and member tiles. |

### 7.1 What "the graph IS the session" means

Per the iced jump-ship plan §1:

> Closing a tile does not close the node. The tile tree projects from
> the graph; the graph is not derived from the tile tree.

Concretely on launch:

- The graph loads from its persistence layer (full graph truth).
- The most-recent Frame Snapshot loads (Shell composition + per-Pane
  `GraphletId` + per-Pane pane type + tile presentation states).
- For each tile that was Active in its graphlet, the Pane re-opens the
  tile (creates the viewer, loads from graph snapshot for that node,
  begins rendering).
- For each tile that was Inactive, no viewer is created; the Tree
  Spine continues to show the node as Inactive.

There is no separate "saved tabs" concept that aliases graph truth.
The graph + the Frame Snapshot together fully determine the session.

### 7.2 Multiple Frame Snapshots

Frame Snapshots can persist; the user can save named snapshots ("Work",
"Research", "Reading list") and restore them later. Snapshots are
graph-bound (they reference the same graph by `GraphId`); cross-graph
snapshots are not supported in the first iced bring-up (see Multi-window
§9 for the multi-graph case).

The Frame Snapshot list is reachable via a Settings pane
(`verso://settings/frames`) and via a CommandPalette action ("Restore
Frame…").

---

## 8. Profiles

§4.6 reshape: *Multiple graphs, each with its own `GraphId`. Per-graph
settings already exist.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | n/a — Shell-owned chrome / settings |
| **Surface** | A profile picker in the Shell command bar (small avatar/initial chip in the top-right). Clicking opens a profile-switcher Modal listing available `GraphId`s with display names. Settings pane (`verso://settings/profiles`) for managing profiles (add, rename, delete). |
| **Data source** | Profile registry in `graphshell-core` settings persistence. Each profile is a `(GraphId, display_name, avatar?)` record plus its own settings tree (per [SHELL.md §3](SHELL.md): "global, per-graph, and per-view settings"). |
| **Intent flow** | "Switch profile" → `ShellIntent::SwitchProfile { graph_id }` — closes the current Frame, loads the selected `GraphId`, restores that profile's most-recent Frame Snapshot. "Add profile" → `ShellIntent::CreateProfile { display_name }` — creates a new `GraphId` with empty graph state. "Delete profile" → confirmation dialog → `ShellIntent::DeleteProfile { graph_id }`. |
| **`verso://` address** | `verso://settings/profiles` for the management pane. Profiles themselves are not separately addressed — they're identified by `GraphId`. |

### 8.1 Per-profile vs cross-profile state

Per profile (per `GraphId`):

- the graph (nodes, edges, graphlets, tile state)
- per-graph settings (theme override, default lens, layout
  preferences)
- Frame Snapshots
- per-graph cache (downloads, viewer caches)

Cross-profile (user-scoped):

- the profile registry itself
- shared keychain / identity material
- Verse / NostrCore / MatrixCore mod settings (community network
  identity is user-scoped, not graph-scoped)
- iced theme defaults

Switching profiles is a clean cut — no state from the previous
profile leaks into the new one except the cross-profile categories
above.

---

## 9. Multi-Window

§4.6 reshape: *Each OS window is one Frame (Shell-owned working context).
Multiple windows are a convenience (multi-monitor, pop-out) — not required
for multi-graphlet work, which multiple Panes in one Frame handle.*

| Question | Answer |
|---|---|
| **Presentation Bucket** | n/a — host-level composition |
| **Surface** | A "New window" command (CommandPalette + Shell menu + `Ctrl+Shift+N` shortcut) creates a new OS window. Each window is one Frame; the frametree visualization shows all open Frames across all windows. |
| **Data source** | Shell owns the Frame composition for each window. Multiple windows may share a `GraphId` (same Workbench composed into different working contexts) or hold different `GraphId`s (each window can be its own profile). |
| **Intent flow** | "New window with current Frame" → `ShellIntent::OpenWindow { frame_template: Current }`. "New window with empty Frame" → `ShellIntent::OpenWindow { frame_template: Empty(graph_id) }`. "Close window" → `ShellIntent::CloseWindow { window_id }` — saves the window's Frame Snapshot before closing. "Pop-out Pane" → `ShellIntent::PopOutPaneToWindow { pane_id }` — opens a new window containing only that Pane (with its `GraphletId` preserved). |
| **`verso://` address** | None for windows themselves; each window's Frame uses `verso://frame/<FrameId>`. Window-level addressing is not a graphshell concept — windows are an OS host artifact. |

### 9.1 Multiple windows on the same `GraphId`

When two windows share a `GraphId`:

- Graph mutations made in either window propagate to both via the
  shared graph runtime — there is one source of truth.
- Per-Pane camera state is per-instance (per `GraphCanvasProgram`'s
  `Program::State`), so the two windows can show different views of
  the same graph.
- Per-graphlet presentation state (Active/Inactive) is shared —
  toggling a tile Active in one window's tile pane shows it Active
  in any tile pane (in either window) bound to the same graphlet.

This works because `GraphletId` carries the per-graphlet state, and
both windows project the same `FrameViewModel` for that
`GraphletId`. No window-level isolation overhead.

### 9.2 Coordination between windows on the same `GraphId`

Per [iced jump-ship plan §10 Q4](2026-04-28_iced_jump_ship_plan.md):

> Coordination of Frames sharing a `GraphId` is TBD and not blocking
> S3/S4.

For the first bring-up, any coordination beyond shared graph truth is
deferred. Specifically not handled in S3/S4:

- "Move Pane to other window" gestures (cross-window drag-drop)
- "Sync camera between windows" or other synchronized-view modes
- Window-level focus / activation hand-off rules beyond the OS default

These are post-bring-up enhancements with their own design pass.

---

## 10. Frametree (the implicit ninth amenity)

The §4.6 update added a "Frametree" row to the amenities table. It's
included here for completeness; the Tree Spine bucket renders it.

| Question | Answer |
|---|---|
| **Presentation Bucket** | Tree Spine (frametree recipe) |
| **Surface** | A collapsible section in the Tree Spine bucket showing each open Frame as a top-level entry, with composed Workbenches and their Panes nested inside. Active Pane is highlighted. Clicking a Pane focuses it (and switches Frame if it's in a different Frame); clicking a Workbench scrolls to its Panes. |
| **Data source** | Shell's Frame composition state (`Frame::composed_workbenches`, `Frame::split_state`). The frametree is a derived projection of all open Frames across all OS windows. |
| **Intent flow** | "Switch to Pane in different Frame" → `ShellIntent::FocusFrameAndPane { frame_id, pane_id }`. "Save current Frame as snapshot" → `ShellIntent::SaveFrameSnapshot { frame_id, name }`. Drag operations within the frametree are deferred (per [iced jump-ship plan §10 Q4](2026-04-28_iced_jump_ship_plan.md) cross-window-drop is post-bring-up). |
| **`verso://` address** | `verso://frame/<FrameId>` per TERMINOLOGY.md (already canonical). |

### 10.1 Frametree row shape

```rust
fn frametree_section(view_model: &FrameViewModel) -> Element<Message> {
    column(view_model.open_frames().map(|frame| {
        column![
            frame_header_row(frame),
            indent(column(frame.composed_workbenches.iter().map(|wb| {
                column![
                    workbench_header_row(wb),
                    indent(column(wb.panes.iter().map(|pane| {
                        pane_row(pane, frame.focused_pane == Some(pane.pane_id))
                    }))),
                ]
            }))),
        ]
    })).into()
}
```

The frametree is a Tree Spine *recipe*, not a separate widget tree —
it slots into the existing Tree Spine bucket renderer per
[`iced_composition_skeleton_spec.md` §6.1](iced_composition_skeleton_spec.md).

### 10.2 Composition skeleton open item

This section closes the
[`iced_composition_skeleton_spec.md` §10 "Frametree visualization in Tree Spine"](iced_composition_skeleton_spec.md)
open item. The skeleton spec referenced this as deferred; it lives here.

---

## 11. Coherence Guarantees Cross-Reference

Each amenity carries a coherence guarantee per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md):

| Amenity | §4.10 row | Key invariant |
|---|---|---|
| History | Activity Log bucket | Read-only; clicking reveals without re-emitting; log records every mutation |
| Bookmarks | Tree Spine + Swatches | Bookmarks are graphlet tags; bookmarking emits `GraphIntent::TagGraphlet`; never bypasses the uphill rule |
| Find-in-page | n/a (modal) | Searches are read-only viewer operations; never mutate graph |
| Find-in-graph | Node Finder (per Node Finder guarantee) | Search is a read-only query; activation emits one `WorkbenchIntent::OpenNode` per selection; never mutates graph truth |
| Downloads | Tool panes guarantee | Tool pane is observer-only; downloads materialize as graph nodes via standard `GraphReducerIntent` path |
| Devtools | Tool panes guarantee | Read-only; section actions emit subsystem-specific intents |
| Sessions / restore | Tree Spine (frametree) | Frame Snapshots are derived restore-points; graph is long-term truth |
| Profiles | Settings panes guarantee | Profile switches are clean-cut between `GraphId`s; no cross-profile leakage |
| Multi-window | n/a (host) | Multiple windows share graph truth; per-instance Program state per window |
| Frametree | Tree Spine bucket guarantee | Read of Shell's Frame composition state; mutations via `ShellIntent::FocusFrameAndPane` |

Each guarantee is the contract S4 implementation receipts target.

---

## 12. Open Items

- **Browser-history import autotagging** (per §3.3 and the iced jump-ship plan §10 Q6): parsing-level semantic autotagging is deferred; the shape of "what tags get applied to imported history" needs its own design pass.
- **Multiple-Frame-Snapshot management UX** (per §7.2): the snapshot picker ergonomics, including diff vs current state, is open.
- **Profile avatar / customization** (per §8): visual chip styling and avatar source are Stage F polish.
- **Per-pane downloads context** (per §5): if a download is initiated from a specific Pane (e.g., a link click in a tile), should that Pane's chrome show download progress directly? Currently no; tool pane only.
- **Find-in-graph result tool pane** (per §4.2): full `verso://tool/search` result pane shape — sortable columns, filter chips, persistent saved searches — is open.
- **Cross-window Pane drop** (per §9.2): post-bring-up, with its own design pass.

These do not block S3 (host runtime closure) or S4 (per-surface bring-up).

---

## 13. Bottom Line

Each browser amenity has one canonical surface, one data source, one
intent flow, and (where applicable) one `verso://` address. Together
they cover the eight §4.6 reshape rows plus the Frametree implicit
ninth. None of them require new authorities or new state machines —
they reshape existing ones (graph, runtime, history, storage, security)
into surfaces the user can predict.

The amenities are now scoped enough for S4 implementation. Each will
land as its own sub-slice; the order is whichever subsystem services
are most portable already (per [iced jump-ship plan S4](2026-04-28_iced_jump_ship_plan.md)).
This closes the last explicit S2 checklist item.
