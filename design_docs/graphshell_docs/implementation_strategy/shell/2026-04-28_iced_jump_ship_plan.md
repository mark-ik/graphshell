<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Jump-Ship Plan (2026-04-28)

**Status**: Active — supersedes the
[2026-04-28 egui_tiles retirement plan](2026-04-28_egui_tiles_retirement_plan.md)
and re-frames the host-migration target in
[2026-04-14 iced host migration plan](2026-04-14_iced_host_migration_execution_plan.md).
**Lane**: Jump ship from egui to iced. Egui treated as broken, not
preserved. Iced built to a refined UX target, not to egui parity.

**Related**:

- [SHELL.md](SHELL.md) — five-domain model authority boundaries
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — Projection Rule,
  Address-as-Identity, Pane Opening Modes, Promotion/Demotion
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — Navigator
  domain (projection + navigation)
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — Workbench
  domain (arrangement + activation)
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph domain (truth)
- [../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md](../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md)
- [shell_composition_model_spec.md](shell_composition_model_spec.md)
- [shell_overview_surface_spec.md](shell_overview_surface_spec.md)
- [PROJECT_DESCRIPTION.md](../../../PROJECT_DESCRIPTION.md)

---

## 1. Intent

Egui is treated as broken. We do not freeze it at "shippable." We do
not preserve its semantics. We do not write parity tests against it.
If a feature exists only in egui form today, it ports as part of the
relevant iced slice or it doesn't ship — its absence does not block
us from cutting egui.

Iced is built to a **refined UX target**, grounded in the canonical
five-domain model the design docs already establish, not to a
recreation of the current egui prototype's shape. Browser amenities
(history, bookmarks, find, downloads, devtools, sessions) are
reshaped to fit the graph paradigm — the graph is the session
state — rather than copied as Chrome/Firefox-shaped chrome.

**The graph is the session state.** Closing a tile does not close the
node. The tile tree projects from the graph; the graph is not derived
from the tile tree. Reopening the tile from the node is a graph-truth
operation, not a tile-tree restoration.

---

## 2. What we are not preserving

- The `egui_tiles::Tree<TileKind>` data model. Its retirement does
  not need a 5-slice migration; it needs to never be touched again.
- The `GraphshellTileBehavior` impl (1,642 LOC). Iced does not have
  `Behavior<TileKind>`. The workbench-essential parts of this
  (focus successors, close policy, lifecycle) are extracted into
  `graphshell-runtime` services, not ported as iced widgets.
- The `tile_compositor.rs` `TileId`-keyed state. Iced compositor
  surfaces are built `NodeKey`-native from the start.
- Egui-era persistence shapes. Existing local saves are throwaway —
  this is a prototype; nothing in current persistence is precious.
  A one-shot exporter to JSON is acceptable if any single user wants
  to keep state, but no migration loader.
- Dual-write / parity / sync glue: `tile_view_ops`, `tile_grouping`,
  `tile_invariants`, `semantic_tabs`, `graph_tree_dual_write`,
  `graph_tree_sync`. These exist because there are two authorities.
  Once egui is gone, there is one. Delete.
- Servoshell legacy disguised as features. If a behavior is in the
  tree because servoshell did it that way, that is not a reason to
  keep it. Audit each `EmbedderWindow`, `AppPreferences`,
  `AppEventLoop`, etc. surface for "is this Graphshell's, or is
  this servoshell residue we never reshaped?"
- Routine god-object decomposition. `Gui`, `GraphshellTileBehavior`,
  `RunningAppState` etc. are anti-patterns we have been continually
  decomposing. The iced host does not get to grow new ones. If any
  iced struct exceeds ~600 LOC or owns more than ~6 distinct
  responsibilities, it is a refactor candidate before it lands.

---

## 3. What survives — the portable contract

These already exist and are clean. They do not change as part of
this plan; they are the asset that makes jump-ship cheap.

| Crate / surface | Role | State |
|---|---|---|
| `graphshell-core` | Portable graph truth, shell state | Clean |
| `graphshell-runtime` | Host-neutral runtime kernel, `runtime.tick()` | Clean |
| `graph-canvas` | Force-directed canvas (scene, camera, hit-test, physics, Vello backend) | Clean |
| `graph-tree` | Workbench tree topology + Taffy layout | Clean |
| `middlenet-*` | CPU-side content rendering | Clean |
| `iced-middlenet-viewer`, `iced-graph-canvas-viewer`, `iced-wry-viewer` | Iced-side content viewers | Clean |
| `HostPorts` traits | Host-neutral runtime/host boundary | Portable types throughout (M3.6 landed) |
| `FrameViewModel` / `FrameHostInput` | Tick I/O contract | Portable types throughout |
| `OverlayAffordancePainter` / `ContentPassPainter` | Paint extraction seams | Trait-only; iced has stub impls |
| `ViewerSurfaceRegistry` | Per-node content surface authority | Portable, `NodeKey`-keyed |
| Sanctioned-writes contract | Durable mutation allowlist | Already enforced by tests |

The Cargo `egui-host` feature gate (landed 2026-04-28) means
`--no-default-features --features iced-host,wry` excludes the egui
crates from the dep graph. Code-level coupling is the remaining
blocker for that build to compile, and this plan removes that
coupling by not writing more egui code.

---

## 4. UX target — anchored to the canonical docs

This section is **synthesis, not invention**. Every claim has a
canonical-doc citation. Where the user's most recent statement
nuances or evolves a doc claim, that is called out as
"model evolution" and listed in §10 as a question to confirm.

### 4.1 Five-domain model (canonical)

Per [SHELL.md §5](SHELL.md):

| Domain | Owns | Does Not Own |
|---|---|---|
| **Shell** | Host + app-level control: command dispatch, top-level composition, settings surfaces, app-scope chrome | Graph truth, arrangement, projection rules, content rendering |
| **Graph** | Truth + analysis + management: node identity, relations, topology | Where or how nodes are arranged in the workbench |
| **Navigator** | Projection + navigation: graphlet derivation, projection rules, section model, scoped search | Node identity, arrangement structure, system settings |
| **Workbench** | Arrangement + activation: tile tree, frame layout, pane lifecycle, routing | What a node is or what its relations mean |
| **Viewer** | Realization: backend selection, fallback policy, render strategy | Graph truth, arrangement, command/control routing |

The iced host is a **Shell** implementation. It dispatches intents.
It does not own graph truth, projection rules, arrangement, or
content rendering. Those live in their respective domains, all
already portable.

### 4.2 Projection Rule (canonical)

Per [TERMINOLOGY.md §Tile Tree Architecture / Projection Rule](../../TERMINOLOGY.md):

- nodes **project as tiles** in workbench chrome
- graphlets **project as tile groups** in workbench chrome
- frames **project as frames** across graph, navigator, and workbench presentations

This is presentation correspondence, not term collapse. The graph is
the source; the workbench projection consumes it. **Closing a tile
is a workbench operation that does not touch graph truth.**

(Today's `Demotion` operation tombstones the node — that is a
distinct, deliberate user action with `Pane Open Event` undo
support. Tile close ≠ Demotion. The iced host distinguishes these
clearly.)

### 4.3 Address-as-Identity (canonical)

Per [TERMINOLOGY.md §Pane Types](../../TERMINOLOGY.md):

> A tile's graph citizenship is determined solely by whether its
> address resolves to a live (non-tombstone) node in the graph.
> No separate mapping structure exists.

The iced host does not maintain an internal `(TileId, NodeKey)`
sidecar. It looks up node identity in `GraphTree` /
`graphshell-core::graph` by `NodeKey` directly.

### 4.4 Pane Opening Modes (canonical)

Per [TERMINOLOGY.md §Pane Types](../../TERMINOLOGY.md): four modes —
`QuarterPane`, `HalfPane`, `FullPane` (ephemeral, no graph
citizenship), `Tile` (promoted, full graph citizen with tab bar).

The iced host renders ephemeral panes inline (no chrome) and
promoted tiles with tab bars. Promotion is a deliberate user
action that emits a graph intent. The iced host has no built-in
notion that "every pane needs a tile."

### 4.5 Workbench instance ↔ graphlet binding (model evolution)

The canonical [TERMINOLOGY.md §Tile Tree Architecture](../../TERMINOLOGY.md)
says: "App Scope owns workbench switching/navigation. **Workbench**
is a global container within App Scope paired to one complete graph
dataset (`GraphId`)."

The user's 2026-04-28 framing: "each workbench instance reflects
the relevant subgraph (graphlet); so the set of nodes in a graphlet
are the set of tiles in the tile group of that workbench instance."

Two readings, both compatible with the existing Projection Rule:

- **Reading A** (compatible with current spec): a Workbench is still
  `GraphId`-bound; at any moment, the **active graphlet projection**
  determines which tile group is visible. "Workbench instance" =
  active runtime state of the same Workbench.
- **Reading B** (model evolution): there can be many Workbench
  instances per process, each one a graphlet view of the underlying
  graph, with the App Scope holding the complete graph and routing
  between graphlet-shaped Workbench instances.

§10 lists this as a question to confirm before §6 slice S2 picks
between them. Both readings keep the Projection Rule (graph →
workbench) intact.

### 4.6 Browser amenities, reshaped (synthesis)

Per [PROJECT_DESCRIPTION.md](../../../PROJECT_DESCRIPTION.md), the
graph is "your tabs as a map you can arrange, save, and share,
instead of a strip at the top of the window." This is the framing
we keep when reshaping browser amenities:

| Amenity | Chrome/Firefox shape | Graph-paradigm shape |
|---|---|---|
| **History** | Linear time-ordered list; back/forward | The graph already records history as edges (visit/traversal/origin). History "view" is a graph projection, not a separate database. Per-node visit timeline is a node attribute, not a global stack |
| **Bookmarks** | Folder-organized URL list | A persistent, named graphlet — a saved subgraph the user has curated. Per [TERMINOLOGY.md](../../TERMINOLOGY.md), graphlets can be "promoted into a named saved structure" |
| **Find-in-page** | Within current document | Graph-scoped find: search-in-pane (current viewer's content) and search-in-graph (across nodes). Both Shell-owned commands per [SHELL.md §6](SHELL.md) |
| **Downloads** | Modal list | A subsystem pane (`TileKind::Tool`) per [TERMINOLOGY.md](../../TERMINOLOGY.md), addressable as `verso://tool/downloads`. Each download can promote to a graph node if the user wants it persisted |
| **Devtools** | Browser-internal panel | Graphshell-level inspector backed by the existing `ux-probes`/`ux-bridge` semantic infra; subsystem panes per [SHELL.md §3](SHELL.md). Servo's devtools remain available for Servo-rendered content but are not the only inspection surface |
| **Sessions / restore** | Reopen-last-tabs | The graph IS the session. There is no separate session restore — opening the app shows the graph, and tile state is derivable from graph + last-known projection |
| **Profiles** | Multiple browser users | Multiple Workbenches per app; per-Workbench `GraphId`. Per-graph settings already exist per [SHELL.md §3](SHELL.md) |
| **Multi-window** | Multiple OS windows showing pages | Multiple OS windows showing different graphlet projections of the same `GraphId` — "windows" in the iced sense rather than "tabs in different windows" |

None of these are net-new design decisions; all of them are present
in some form in the canonical docs. The plan's job is to actually
build them as portable services in `graphshell-runtime`, not to
keep them as stubs.

---

## 5. Anti-patterns to avoid

These are concrete rules that constrain how iced code lands.

- **No god objects.** No iced struct exceeds ~600 LOC or owns more
  than ~6 responsibilities without refactor. `Gui` and
  `GraphshellTileBehavior` are cautionary tales, not templates.
- **No host lock-in.** Iced widgets consume `FrameViewModel` and
  emit `HostIntent`s. Domain authority lives in
  `graphshell-runtime` / `graphshell-core`, never in iced widget
  state. Per the
  [iced migration plan §4](2026-04-14_iced_host_migration_execution_plan.md):
  "iced types must not leak into `graph-tree`, `graph-canvas`,
  compositor boundaries, or future presenter/runtime layers."
- **No servoshell legacy disguised as feature.** Each
  `EmbedderWindow`, `AppPreferences`, `AppEventLoop`,
  `AppGamepadProvider` etc. surface gets one of two outcomes:
  (1) it expresses Graphshell-shaped intent and stays, or (2) it
  is servoshell residue and goes away. No third option.
- **No egui parity tests.** The runtime parity surface
  (`iced_parity.rs` cross-host scalar test) was useful while egui
  was the reference. Going forward, parity is verified against the
  refined UX target, not against the egui implementation.
- **No new dual-authority.** Every durable mutation goes through
  the sanctioned-writes allowlist enforced by the existing
  contract test (see [iced migration plan §M4](2026-04-14_iced_host_migration_execution_plan.md)).
  The iced host adds entries to that allowlist; it does not bypass
  the lane.
- **No "iced-shaped egui."** When iced has a native idiom that's
  different from the egui equivalent (canvas-local `Program::State`
  for camera, direct view-model consumption in `view`, inline
  painting in canvas `draw`), iced uses it. Per the
  [iced migration plan §4](2026-04-14_iced_host_migration_execution_plan.md):
  "the two hosts need not share implementation shape."
- **No new code in `shell::desktop::ui::gui*` or
  `shell::desktop::workbench`.** Those modules are frozen at
  current state (broken or not). Bug fixes only if they unblock
  iced work, never feature additions. New work lands in
  `shell::desktop::ui::iced_*` or in `graphshell-runtime`.

---

## 6. Slice plan

Each slice is independently shippable and produces a real artifact.
Slice ordering reflects what unblocks the next slice's design work.
S0 already landed; S6 is the receipt.

### S0 — Cargo gate (LANDED 2026-04-28)

`egui-host` feature added to `Cargo.toml`; six egui crates marked
`optional = true`. `cargo tree` confirms iced-only build excludes
egui from the dep graph. Code-level coupling persists, but is the
boundary this plan removes.

### S1 — Freeze egui in place

**Done condition**: a CI check or workflow rule prevents new code
from being added to egui-host paths.

Checklist:

- [ ] Add a CODEOWNERS-style rule or pre-commit check rejecting new
  files in `shell::desktop::ui::gui*`,
  `shell::desktop::ui::render::*`, `shell::desktop::workbench::*`,
  `shell::desktop::host::*` (or document the policy in
  `CONTRIBUTING.md` if a hard gate is heavier than warranted)
- [ ] Mark `2026-04-28_egui_tiles_retirement_plan.md` superseded
  by this doc (link forward to here)
- [ ] Mark M5/M6 of the
  [iced host migration plan](2026-04-14_iced_host_migration_execution_plan.md)
  as superseded by this plan; the §M5/M6 receipts are absorbed into
  the slices below
- [ ] No code change to egui paths in this slice

### S2 — UX target document

**Done condition**: a single design doc that the iced slices read
from for "what should this surface look like."

Checklist:

- [ ] Confirm Reading A vs. Reading B for §4.5
  (Workbench/graphlet binding); update the doc and
  [TERMINOLOGY.md](../../TERMINOLOGY.md) accordingly
- [ ] Confirm tile-close vs. demotion semantics: a "close tile"
  action is a workbench operation that leaves the node alive; a
  "demote tile" action tombstones the node (existing behavior).
  Update [TERMINOLOGY.md §Pane Types](../../TERMINOLOGY.md) if
  the current Demotion text conflates them
- [ ] Define the iced-side composition skeleton (the slot model
  for the iced equivalent of
  [shell_composition_model_spec.md](shell_composition_model_spec.md);
  iced uses panes/`row!`/`column!` instead of egui's named
  panels — the slot identities and authority assignments stay
  the same)
- [ ] Specify the omnibar shape for iced: Shell-owned input/dispatch,
  Navigator-owned breadcrumb projection, both rendered through the
  same iced widget per [SHELL.md §6](SHELL.md)
- [ ] Specify command palette behavior in iced terms (the
  `command_palette_state` portable state already exists; this is
  rendering only)
- [ ] Specify each browser amenity per §4.6: which surface, what
  data, what intent flow, what `verso://` address (where applicable)

This is the slice that needs your input. Section 10 lists the
specific questions; everything else here is grounded in canonical
docs already.

### S3 — Iced host runtime closure

**Done condition**: the 24 `todo(m5)` markers in
`shell::desktop::ui::iced_host_ports.rs` are all resolved with
real implementations; `cargo build --no-default-features
--features iced-host,wry` compiles.

Checklist:

- [ ] Implement `HostInputPort` event translation
  (winit → `HostEvent`)
- [ ] Implement `HostSurfacePort` register / unregister / retire /
  present using iced's wgpu device
- [ ] Implement `HostClipboardPort` via `arboard` (same as egui)
- [ ] Implement `HostToastPort` rendering iced toasts from the
  view-model toast queue
- [ ] Implement `HostAccessibilityPort` via `accesskit` direct
  (no egui-winit accesskit bridge)
- [ ] Drop egui crates from the dep graph for the iced-only build
  (already true post-S0; verify post-S3 still holds)
- [ ] First receipt: `--features iced-host,wry --no-default-features`
  compiles and produces a binary that opens an empty window with
  the Shell composition skeleton

### S4 — Iced surfaces to UX target

**Done condition**: each surface in §4.6 has a real iced
implementation backed by a portable runtime service.

Each surface is a sub-slice. Order picks itself: do the ones whose
runtime services are most portable already.

- [ ] **Omnibar + command palette** — `command_palette_state`
  exists in `graphshell-core`; this is render + input wiring only
- [ ] **Graph canvas** — `iced_graph_canvas.rs` is a starting
  point; promote it to a real `CanvasBackend<NodeKey>` impl
  consuming `ProjectedScene`
- [ ] **Workbench tile rendering** — read `LayoutResult` from
  `GraphTree::compute_layout()`; render `(NodeKey, Rect)`
  iter directly. No `egui_tiles::Tree` touched ever
- [ ] **Tile chrome** (close button, drag handle, lifecycle
  badge) — emits `WorkbenchIntent`s
- [ ] **Frame switching** — frames are a runtime concept already;
  iced renders the frame switcher widget
- [ ] **Settings panes** — `verso://settings/<section>` already
  routes; iced renders the inner panel
- [ ] **History view** — new portable service in
  `graphshell-runtime` (or thin wrapper over existing graph
  edges per §4.6); iced renders
- [ ] **Bookmarks** — promote graphlet save/load; iced renders
- [ ] **Find-in-page / find-in-graph** — Shell command +
  per-viewer search delegate
- [ ] **Downloads** — `verso://tool/downloads` subsystem pane
- [ ] **Devtools** — Graphshell-level inspector (`ux-probes`
  consumer); Servo devtools remain reachable for Servo content

### S5 — Graphlet projection plumbing

**Done condition**: the active workbench tile group is a
**projected graphlet**, not a free-floating tile arrangement.

This is the slice that materializes the user's most recent
framing: tile groups are graphlet projections. It depends on §4.5
being resolved.

Checklist:

- [ ] Define the runtime data flow: `Graph` → `Navigator
  projection pipeline` → `LayoutResult` → iced render
- [ ] Replace any tile-group state that exists today as a
  free-standing structure with a derived view of the active
  graphlet projection
- [ ] Demonstrate: closing a tile leaves the node in the graph;
  reopening from the node restores the tile in the same graphlet
  context
- [ ] Demonstrate: switching graphlets switches the tile group
  without losing graph state
- [ ] Demonstrate: a Workbench (Reading A) or new Workbench
  instance (Reading B) showing a different graphlet of the same
  graph

### S6 — Delete egui

**Done condition**: `egui*` crates are uncited in `Cargo.toml`,
nothing imports them, all egui-host source code is deleted, and
the binary still works.

Checklist:

- [ ] Delete `shell::desktop::ui::gui*`
- [ ] Delete `shell::desktop::ui::render::*`
- [ ] Delete `shell::desktop::workbench::*` (the workbench
  *runtime* logic moved to `graphshell-runtime` in S3/S4; this
  slice deletes the egui-side rendering glue that became
  unreachable)
- [ ] Delete `shell::desktop::host::window.rs` and adjacent
  servoshell-shaped surfaces, replacing references with
  iced-native equivalents
- [ ] Drop `egui-host` feature; drop `egui*` deps; drop
  `servo-engine`'s transitive activation of `egui-host`
- [ ] Drop the source-code mobile `cfg` gates (12 .rs files
  plus `build.rs`) since they're orphaned residue
- [ ] `cargo tree -e features` returns no `egui*` crates under
  any feature combination
- [ ] Default build is iced

---

## 7. Cross-reference: what this saves vs. the retirement plan

The [retirement plan](2026-04-28_egui_tiles_retirement_plan.md)
estimated five preservation-shaped slices. Jump-ship deletes the
preservation work outright:

| Retirement slice | Jump-ship treatment |
|---|---|
| S1 — Extract `Behavior` methods to standalone functions | Skipped. The trait impl freezes in place and gets deleted in S6 |
| S2 — Replace `Tree::ui()` with direct iteration + parity receipts | Skipped. Iced never had `Tree::ui()`; receipts are against UX target, not egui |
| S3 — Rekey compositor `TileId → NodeKey` | Skipped. Iced compositor surfaces are `NodeKey`-native from S3 of this plan |
| S4 — Persistence migration | Skipped. Egui-era saves are throwaway |
| S5 — Delete egui_tiles dep + dual-write/sync glue | Folded into S6 of this plan, with no preceding migration cycle |

The work that has to happen either way (build iced chrome, build
portable browser-amenity services) is ordered differently here and
is anchored to UX-target receipts instead of egui-parity receipts.

---

## 8. Receipts and parity

Receipts are against the UX target document (S2 deliverable), not
against egui behavior.

- **S3 receipt**: iced-only build compiles, opens an empty window
  with the Shell composition skeleton
- **S4 receipt** (per surface): visual + interaction match the UX
  target spec for that surface; portable service unit-tested
- **S5 receipt**: graphlet projection round-trip — close tile,
  reopen from node, switch graphlets — all preserve graph truth
- **S6 receipt**: `cargo tree | grep egui` returns nothing; default
  build is iced; running the binary works

Parity testing across hosts — the `iced_parity.rs` cross-host
scalar tests — stops being useful here because there is no second
host to compare against. The `runtime.tick()` portable contract is
still tested with unit tests in `graphshell-runtime` /
`graphshell-core`; that's where parity actually matters.

---

## 9. Risks

- **Risk: UX target underspecified.** Slices S3+ depend on S2.
  If the UX target doc is hand-wavy, the slices land in the
  prototype-quality zone we're trying to escape. **Mitigation**:
  S2 has a question list (§10); answer enough of them to ground
  S3/S4 surface work, defer the rest.
- **Risk: iced widget gaps.** Iced's widget set is leaner than
  egui's. Some chrome surfaces will need custom widgets.
  **Mitigation**: pick iced patterns where they're idiomatic
  (canvas `Program`, `shader` widget) before reaching for custom
  widget infrastructure. Custom widgets are fine when warranted;
  they aren't a default choice.
- **Risk: Servo content surface integration in iced.** The
  [iced content surface scoping](2026-04-24_iced_content_surface_scoping.md)
  doc tracks middlenet-first → wry second → Servo-shared-wgpu
  third. With wgpu 29 parity now reached (iced vendored),
  shared-wgpu is no longer blocked. **Mitigation**: keep
  middlenet/wry as the default content paths during S3/S4; bring
  Servo in S5 alongside graphlet work.
- **Risk: model evolution churn.** Reading A vs. Reading B for
  Workbench/graphlet binding changes how `GraphId` works.
  **Mitigation**: defer the choice until S5; S3/S4 only need a
  single Workbench-per-process, which both readings support.
- **Risk: testing during the transition.** With egui frozen and
  iced-still-building, neither host is fully usable for a span.
  **Mitigation**: the runtime layer (`graphshell-core`,
  `graphshell-runtime`) is testable without any host; ride that
  for unit + integration tests during S3/S4. Manual exploration
  resumes when S3 produces a runnable iced binary.

---

## 10. Open questions

These belong to S2. They are the questions where the user offered
to answer rather than the model answer being inferable from
canonical docs.

1. **Workbench/graphlet binding.** Reading A (Workbench is
   `GraphId`-bound; active graphlet projection determines visible
   tile group) or Reading B (multiple Workbench instances per
   process, each a graphlet view of the underlying graph)?
2. **Tile-close vs. demotion semantics.** Confirm: closing a tile
   leaves the node alive in the graph. Demotion (the deliberate
   user action) tombstones the node. Current
   [TERMINOLOGY.md §Pane Types](../../TERMINOLOGY.md) uses
   "Demotion" as the inverse of Promotion and ties it to
   tombstoning — does this need a separate "Close Tile" term, or
   is "close" implicit in current Pane Open Event semantics?
3. **Frame ↔ Workbench composition.** Frames (per
   [TERMINOLOGY.md](../../TERMINOLOGY.md)) are persisted branches
   of the Workbench tile tree. With graphlet-shaped Workbenches,
   does Frame become "named graphlet sub-arrangement" or stay as
   today (sibling structural concept)?
4. **Multi-window scope.** Multi-window (per §4.6) — does each OS
   window host a Workbench instance, or do windows share a
   Workbench and show different graphlets of it?
5. **History reshape.** §4.6 says "the graph already records
   history as edges." Confirm: do we need a separate History
   service in `graphshell-runtime`, or is the history surface
   purely a Navigator projection of existing edge data?
6. **Bookmarks reshape.** §4.6 says bookmarks are persistent
   named graphlets. Confirm: does this replace any current
   bookmark-import path, or does it sit alongside?
7. **Devtools surface.** §4.6 says Graphshell-level inspector
   backed by `ux-probes`. Is this a single tool pane, or a family
   of tool panes per inspected subsystem?
8. **Refined UI fidelity.** "Refined" was the user's word. Some
   reasonable readings: (a) browser-comparable polish on existing
   surfaces, (b) UX-design pass against the canonical Projection
   Rule (graph + workbench feel like one continuous experience,
   not two separate views), (c) accessibility/keyboard parity
   with desktop apps. Which of these (or all) is the bar for S4
   surfaces to clear?

---

## 11. Bottom line

Stop patching the boat. Egui is broken, freeze it where it is, build
iced to a refined UX target grounded in the existing five-domain
docs, reshape browser amenities for the graph paradigm rather than
copying chrome, and delete egui when the iced binary covers the
target. The portable contract (graphshell-core, graphshell-runtime,
graph-canvas, graph-tree, HostPorts, FrameViewModel) is the asset
that makes this cheap.

Receipts in §8 are the done condition. Questions in §10 are the
gating input for S2. Everything else is ordered work.
