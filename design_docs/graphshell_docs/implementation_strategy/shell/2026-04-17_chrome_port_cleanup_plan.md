# Chrome Port + Cleanup Plan (M6 prelude)

**Status**: active plan, M6 prerequisite
**Date**: 2026-04-17
**Supersedes/extends**: [2026-04-14_iced_host_migration_execution_plan.md](2026-04-14_iced_host_migration_execution_plan.md) §M6

## 1. Why now

M5 proved the iced host can host the runtime. Before porting chrome
surfaces wholesale, we take one planning beat so chrome work lands with
three things improved, not just "duplicated in iced":

1. Legacy cruft in the egui chrome is removed, not carried forward.
2. Affected tests move off egui-specific fixtures onto host-neutral
   ones (`HostPorts` adapters, `FrameHostInput`/`FrameViewModel`).
3. Cross-cutting systems (uxtree, accessibility, diagnostics) get
   brought up to a consistent bar before chrome changes layer on top.

Porting + cleanup + refactor combined per surface is chaos. Porting
first on a thin adapter, then refactoring after both hosts compile,
is the only sequence that keeps bisect-friendly history.

## 2. Principle: Firefox where equivalent, own voice where novel

Where a chrome surface has a Firefox-equivalent, adopt Firefox's
interaction model and keyboard conventions verbatim unless we have a
specific reason not to. Free UX credibility, decades of refinement,
users already know it.

Where a surface has no Firefox equivalent — workbench layout, the
Navigator, the radial menu, graph-as-primary-surface — keep our own
voice. Force-fitting Firefox patterns erases the novel ideas.

**Test for "equivalent"**: if a new user could transfer their Firefox
muscle memory and not feel confused, it's equivalent. Toolbar URL bar,
tab strip, context menus, sidebar panels, about:config-style prefs,
find-in-page, and session restore all pass this test. Workbench tiling
does not.

## 3. Chrome surface inventory

| Surface | Current module(s) | Firefox equivalent | Known cruft | M6 shape |
|---|---|---|---|---|
| Toolbar (URL bar, back/forward, reload) | `toolbar/`, `gui/toolbar_status_sync.rs`, `toolbar_routing.rs` | Awesome Bar / main toolbar | Per-pane `ToolbarDraft` bookkeeping with shaky persistence seams; sync logic split across 3 files | Consolidate draft state onto runtime; single `ToolbarViewModel` drives both hosts |
| Omnibar / graph search | `graph_search_flow.rs`, `graph_search_ui.rs`, `gui/graph_search_orchestration.rs` | Awesome Bar suggestions | Flow/UI/orchestration triple-indirection; ad-hoc per-frame state | Collapse to one flow + one view function per host |
| Tab strip | `egui_tiles` behavior + `gui_frame/` | Firefox tab bar (tree-style tabs via TST) | Rendering + behavior tangled with tiles tree | Already graph-tree-keyed; surface the tabs from `FrameViewModel::tab_order` and retire tiles-tree coupling |
| Sidebar(s) | `navigator_context.rs`, `tag_panel.rs` | Sidebar panels (bookmarks, history) | Custom panel plumbing; no clear "sidebar slot" concept | Introduce a single `SidebarPaneKind` enum and a slot in `FrameViewModel`; render host-specifically |
| Dialogs | `dialog.rs`, `dialog_panels.rs`, `gui_frame/toolbar_dialog.rs` | Modal dialogs | 931 lines of mixed state + render + dismiss logic; scattered `ctx.request_repaint()` | Convert to `DialogsViewModel` flags + per-dialog view state owned on runtime; host renders from the flags |
| Toasts | `finalize_update_frame` + `egui_notify` | Doorhanger / notification bar | Already on `HostToastPort` ✓ | Keep; add an iced-side renderer |
| Radial menu | custom | None (graphshell-native) | — | Keep; port one-to-one to iced canvas |
| Command palette | dialog-based | Firefox command palette (Ctrl+Shift+P in DevTools) | Mixed state in gui_state.rs + dialog code | Treat as a dialog; inherits the dialog refactor |
| Settings | `persistence_ops.rs` + dialog | `about:preferences` | Routing split between `tiles_tree` settings route and dialog | Adopt an `about:config`-style in-page route; retire dialog form |
| Context menu (right-click) | scattered | Context menu | Inconsistent entry points | Central `ContextMenuSpec` + dispatcher; one host renderer per host |
| Status / degraded receipts | `tile_compositor` | Status bar | Deep in compositor pass | Lift to `FrameViewModel::degraded_receipts` (already a field — populate it) |
| Bookmark import | `BookmarkImportDialogState` | Bookmark import | Host-specific `EguiFileDialog` | Abstract behind a `HostFileDialogPort` or punt to a runtime-owned workflow |
| Find in page | not present | Firefox find bar | — | Out of scope for M6 |

## 4. Mess areas worth attacking

Ranked by cost × blast radius. Attack these in M6 before per-surface
porting begins — each is a cross-cutting cleanup whose output every
chrome surface inherits.

### 4.1 `gui_orchestration.rs` (1895 lines) — split responsibilities

Currently mixes clipboard handling, toast emission, pre-frame phase
orchestration, semantic lifecycle, intent translation, and a dozen
tiny helpers. Split into: `clipboard_flow.rs`, `toast_flow.rs`,
`pre_frame.rs`, `semantic_lifecycle.rs`, `intents.rs`. Each file
single-responsibility, each move preserves behavior.

### 4.2 `dialog.rs` (931 lines) — dialogs are data, not code

Each dialog's open/close/dismiss logic is hand-rolled with direct
`ctx.request_repaint()` calls. Refactor so:

- Each dialog's state lives on `GraphshellRuntime` (already done for
  `bookmark_import_dialog`; do the rest).
- `DialogsViewModel` carries the dismissal/open flags.
- Host renders from the view-model, dispatches actions back through
  intents.

This makes dialogs trivially portable to iced and eliminates the
request_repaint calls from shared code paths.

### 4.3 Toolbar draft bookkeeping

Per-pane `ToolbarDraft` state lives on `GraphshellRuntime` but mutation
paths run through `sync_active_toolbar_draft`, `persist_active_toolbar_draft`,
and scattered egui input handlers. Consolidate into a single
"draft manager" on runtime with one entry point for edits and one for
pane-activation changes.

### 4.4 Frame-inbox drain split

Half the drains moved to `ingest_frame_input` in M4.5b; the other half
stay host-side because their consumers reach into `tiles_tree` or the
egui `Context`. Once `tiles_tree` retires (M7 prereq, but we can start
early by lifting the state consumers onto GraphTree), all drains
collapse into one path on the runtime.

### 4.5 `workbench_host.rs` (8129 lines)

The single largest file in the shell. Not a "port this" target — a
"read-audit this" target. Identify what's genuinely workbench
policy vs. accidental host coupling vs. dead. Plan a follow-on
split after the above four are done. Do not touch in M6 unless a
specific chrome surface requires it.

## 5. Cross-cutting prerequisites

These must land before per-surface chrome porting begins, because
every surface depends on them.

### 5.1 UxTree host neutrality — **blocker**

`build_snapshot` currently takes `&Tree<TileKind>`. This blocks:

- M5 parity tests from asserting on presentation/trace layers
  (currently only semantic + node count)
- M7 `egui_tiles` retirement
- Any iced chrome surface that wants to emit a uxtree entry

**Done gate**: `build_snapshot(graph_app, node_rects)` takes no
host-specific tree. Rects come from `FrameViewModel::active_pane_rects`.
Tile metadata comes from GraphTree directly.

### 5.2 `HostAccessibilityPort` real wiring — **blocker for any iced chrome**

Today it's a placeholder. Before any chrome surface lands in iced, the
port must route to `iced_accesskit` (or a hand-rolled tree update sink
if the bridge isn't ready). Without this, accessibility regresses on
iced — unacceptable.

**Done gate**: accesskit tree updates from Servo webviews reach the
iced window, and focus requests from the runtime realize in iced.

### 5.3 Diagnostics channel survival — **invariant**

Diagnostics are currently emitted from deep inside host phases
(`emit_event(...)` calls scattered throughout `gui_orchestration` and
`tile_compositor`). As phases migrate, those emit calls must not get
dropped or duplicated. Enforce via a test: after any migration
commit, the set of diagnostic channel IDs emitted per frame must not
shrink.

**Done gate**: a snapshot test pins the diagnostic-channel coverage
from the current pipeline; any PR that shrinks it fails CI.

## 6. Non-goals for M6

Explicitly **not** refactoring in this pass. Deferred to later phases
or left alone entirely.

- Workbench layout algorithm (GraphTree Taffy integration stays as-is).
- Navigator semantic system — the `navigator_specialty_*` machinery
  keeps its own shape.
- Radial menu interaction model — ports as-is, no redesign.
- Graph canvas interaction (M5.4 left it render-only; that's fine for
  M6 too).
- Command palette fuzzy search behavior.
- Persistence format changes.
- Any change to `GraphIntent` variants.
- Servo/webview lifecycle. That's M7 / content surface territory.

## 7. Suggested sequence

1. **Cross-cutting first** (5.1, 5.2, 5.3): unblocks everything else.
2. **Mess area 4.1**: `gui_orchestration.rs` split. Low-risk,
   high-clarity win.
3. **Mess area 4.2**: dialog refactor to view-model-driven rendering.
   Directly reduces port surface area.
4. **Mess area 4.3**: toolbar draft consolidation. Directly informs
   the toolbar chrome port.
5. **Mess area 4.4**: collapse remaining frame-inbox drains onto
   runtime.
6. **Per-surface chrome ports** (toolbar → sidebar → tab strip →
   dialogs → context menu → radial menu → command palette → settings).
   Each surface follows the same sub-sequence: port on adapter → verify
   parity → refactor layout → remove legacy.

## 8. Acceptance criteria

M6 ships when:

- All chrome surfaces in §3 render in both hosts with identical
  `FrameViewModel` consumption.
- M5 parity tests extend to presentation-layer uxtree equality.
- `HostAccessibilityPort` delegates to real accesskit wiring in both
  hosts.
- Diagnostic channel-coverage snapshot test passes.
- Line count of `shell/desktop/ui/` drops (net). If it grows, we
  carried cruft forward.

## 9. What this doc does not do

This is a planning pass, not a specification. Each mess area and each
chrome surface gets its own strategy doc only when its scope demands
it. Most should fit in a single PR with a terse description. Keep
this doc as the index; promote items out of it as they spawn their
own plans.

---

**Next action**: start with §5.1 (uxtree host neutrality). Short,
focused commit; unblocks everything downstream.
