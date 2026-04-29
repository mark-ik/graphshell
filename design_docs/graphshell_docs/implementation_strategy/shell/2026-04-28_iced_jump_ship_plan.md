<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Jump-Ship Plan (2026-04-28)

**Status**: Paused — deferred in favor of GPUI research. Initially supersedes the
[2026-04-28 egui_tiles retirement plan](2026-04-28_egui_tiles_retirement_plan.md)
and re-frames the host-migration target in
[2026-04-14 iced host migration plan](2026-04-14_iced_host_migration_execution_plan.md).
**Lane**: Jump ship from egui to iced (paused). Egui treated as broken, not
preserved. Iced built to a refined UX target, not to egui parity.

**Related**:

- [SHELL.md](SHELL.md) — five-domain model authority boundaries
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — Projection Rule,
  Address-as-Identity, Pane/Tile/Frame spatial model, Active/Inactive
  presentation state, Remove from graphlet, Tombstone
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

### 3.1 Example target file structure

This is an example target shape, not a requirement to move every file
in one sweep. During S3/S4, new code can land in the existing
`shell::desktop::ui::iced_*` modules when that is the lowest-friction
path. The important direction is the ownership boundary: shared code
does not live under an egui-shaped shell tree, and host crates consume
portable contracts instead of defining domain authority.

```text
graphshell/
  Cargo.toml                    # composition root; feature wiring only
  main.rs                       # selects the default host

  crates/
    graphshell-core/            # graph truth, ids, portable shell state,
                                # geometry/events/colors/time, sanctioned writes
    graphshell-runtime/         # host-neutral runtime.tick(), command dispatch,
                                # workbench/viewer/navigator services

    graphshell-graph/           # graph domain policy: nodes, edges,
                                # graphlets, analysis, lifecycle
    graphshell-navigator/       # projection rules, scoped search,
                                # breadcrumbs, graphlet navigation
    graphshell-workbench/       # arrangement + activation: frames,
                                # pane lifecycle, close/promote/demote policy
    graphshell-viewer/          # viewer routing, fallback policy,
                                # per-node surface authority

    graph-canvas/               # canvas scene, physics, hit-test,
                                # camera, Vello/wgpu-independent model
    graph-tree/                 # topology + Taffy layout for workbench projection

    middlenet/                  # CPU-side content/render model
    middlenet-viewer/           # host-neutral middlenet viewer contract

    hosts/
      iced-shell/               # first real host: iced app, widgets,
                                # winit/wgpu/accesskit/arboard adapters
      xilem-shell/              # future host, same runtime contracts
      gpui-shell/               # future host, same runtime contracts

    viewers/
      iced-graph-canvas-viewer/ # iced realization of graph canvas
      iced-middlenet-viewer/    # iced realization of middlenet content
      iced-wry-viewer/          # iced/wry web content surface
      servo-viewer/             # optional Servo realization, no shell authority

    diagnostics/                # host-neutral traces, snapshots, probes,
                                # with host-specific renderers below hosts/*

  design_docs/
  resources/
  tests/
```

Short-term landing rule:

- Domain authority moves toward `graphshell-core`,
  `graphshell-runtime`, and the domain crates above.
- Iced rendering/input lands under `shell::desktop::ui::iced_*` now,
  and can move to `crates/hosts/iced-shell` once the module boundary
  is mechanical.
- Existing egui modules are frozen. They can be edited only to remove
  shared coupling or unblock iced; they are not destination modules.
- Servo, wry, middlenet, and future content renderers are Viewer
  realizations. They do not own Shell, Graph, Navigator, or Workbench
  policy.

### 3.2 Portable crate decomposition plan

The jump-ship path should not replace egui-shaped god objects with
portable god files. On 2026-04-28, the current portable-crate
inventory had these Rust files over the 600 LOC threshold:

| File | Lines | Decomposition target |
|---|---:|---|
| `crates/graphshell-core/src/graph/mod.rs` | 4,937 | Split into `identity`, `node`, `edge`, `graphlet`, `lifecycle`, `mutation`, `query`, and `selection` modules. Keep `mod.rs` as re-exports plus the smallest possible facade |
| `crates/graph-cartography/src/lib.rs` | 3,623 | Split into `projection`, `mapping`, `layout_export`, `view_model`, `registry`, and `error` modules. Keep graph-to-map policy separate from export/render shapes |
| `crates/graph-tree/src/tree.rs` | 1,766 | Split tree storage, mutation commands, traversal, focus/activation helpers, and layout input/output adapters |
| `crates/graph-canvas/src/derive.rs` | 1,391 | Split projected-scene derivation into node projection, edge projection, selection/highlight enrichment, label/style derivation, and diagnostics summaries |
| `crates/graph-memory/src/lib.rs` | 1,372 | Split memory model, indexing, recall/query, persistence, and scoring/ranking into separate modules |
| `crates/graph-canvas/src/engine.rs` | 1,209 | Split engine state, tick/update loop, input commands, camera commands, and backend handoff |
| `crates/middlenet-core/src/document.rs` | 887 | Split document model, block tree, text ranges, annotations/metadata, and serialization helpers |
| `crates/graph-canvas/src/layout/rapier_adapter.rs` | 862 | Split body/collider construction, force application, constraints, and result extraction |
| `crates/graph-canvas/src/layout/extras.rs` | 804 | Split optional layout features by responsibility: clustering, pinning, viewport constraints, and debug aids |
| `crates/graph-canvas/src/layout/static_layouts.rs` | 801 | Split each static layout family into its own module with a shared registry-facing facade |
| `crates/graph-canvas/src/layout/registry.rs` | 793 | Split layout descriptors, profile registry, factory resolution, and validation |
| `crates/graph-tree/src/topology.rs` | 785 | Split topology model, adjacency queries, path operations, and invariant checks |
| `crates/graphshell-core/src/actions.rs` | 711 | Split action identifiers, command descriptors, dispatch metadata, and serialization |
| `crates/graphshell-core/src/graph/filter.rs` | 709 | Split AST/types, parser, evaluator, text matching, and diagnostics |
| `crates/graphshell-runtime/src/frame_projection.rs` | 708 | Split input collection, frame/view projection, overlay projection, and command-surface projection |
| `crates/graphshell-core/src/shell_state/frame_model.rs` | 673 | Split frame identity, frame tree/model, lifecycle commands, and persistence shape |
| `crates/graph-canvas/src/scene_physics.rs` | 652 | Split force model, integration, constraints, and scene-runtime adapters |
| `crates/middlenet-adapters/src/lib.rs` | 649 | Split adapter traits, iced adapter, wry/host adapters, and test fixtures |

Decomposition rules:

- Do not introduce new files over 600 LOC in portable crates.
- When a slice touches an oversized file, either decompose the touched
  responsibility first or extract it in the same change.
- Preserve public crate APIs during the first split by re-exporting from
  the old module path; rename external APIs only in a follow-up with a
  focused migration.
- Keep extraction mechanical before changing behavior: move code,
  re-export, run the narrow tests/checks, then make semantic changes.
- Prefer domain names over implementation names. For example,
  `graphlet`, `lifecycle`, and `projection` are better module names
  than `helpers` or `utils`.

Done condition: every portable-crate Rust file is under 600 LOC, or has
an explicit exception in this document explaining why it must remain
larger. No exception should be granted for a file that mixes multiple
domain authorities.

### 3.3 2026-04-28 pruning receipt — iced host compiles without egui

The first portable-type cut landed:

- `graphshell-core::geometry` now includes `PortableVector`.
- Scene runtime geometry moved from egui `Pos2` / `Vec2` / `Rect` to
  `PortablePoint` / `PortableVector` / `PortableRect`.
- Simulate-release impulses are stored as `PortableVector`; egui drag
  release input converts at the egui boundary only.
- Frame-affinity runtime regions now derive portable centroids and
  portable packet colors instead of egui `Pos2` / `Color32`.
- Edge-style registry no longer imports egui or calls `egui::lerp`;
  theme color tokens use the host-neutral `Color32` shim.
- Egui-tile-only workbench commands and undo-boundary modules are
  gated out of the iced host path.

Validation:

```text
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry
```

All three pass as of this receipt. The all-target proof required one
more cleanup pass:

- Runtime scene-region tests now use `PortablePoint` /
  `PortableVector` / `PortableRect` instead of egui geometry.
- Graph-view scene runtime tests now use portable geometry at the
  app boundary.
- Legacy egui/Servo test harnesses are gated to their owning
  features instead of compiling under iced-only:
  - root URL parser tests stay with `servo-engine`
  - egui input tests stay with `egui-host`
  - GL/egui diagnostics tests stay with `egui-host + servo-engine`
  - egui_tiles workbench / ux-tree tests stay with the egui host path

Remaining warnings are pre-existing unused imports / dead code, not
blockers for the host split.

### 3.4 2026-04-28 pruning receipt — Servo no longer implies egui

The next feature-coupling cut landed:

- `servo-engine` no longer activates `egui-host` transitively.
- Default builds still request both features explicitly:
  `default = ["servo-engine", "egui-host", ...]`.
- Legacy modules that are really **Servo + egui shell** now gate on
  `all(feature = "servo-engine", feature = "egui-host")`, including:
  - the old desktop embedder host
  - the `render/` egui rendering layer
  - toolbar/dialog/gui/workbench-host UI modules
  - egui_tiles-backed workbench mutation/render/probe modules
  - the egui workbench-surface registry implementation
- Servo/wgpu plumbing that is not egui-specific remains on
  `servo-engine`.

Validation:

```text
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry
```

Both pass after the split. A direct Servo ownership proof was attempted:

```text
cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features servo-engine,js_jit,wry
```

That currently blocks in the sibling Servo/WebRender seam before
Graphshell can be checked: `servo-paint` expects WGPU WebRender API
symbols such as `WgpuExternalImageHandler`, `RendererBackend`,
`create_webrender_instance_with_backend`, and `WgpuTexture` that the
current `webrender-wgpu` checkout does not export. That is not an egui
feature-gate failure, but it means the Servo-only proof remains pending
until the Servo/WebRender WGPU API surface is synced.

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

(Closing a tile deactivates the node — a presentation-only
operation. Removing a node from a graphlet is a separate,
deliberate graph edit. Tombstoning is a third, destructive
operation requiring confirmation. The iced host treats these
as distinct operations with distinct affordances. See §4.4.)

### 4.3 Address-as-Identity (canonical)

Per [TERMINOLOGY.md §Pane Types](../../TERMINOLOGY.md):

> A tile's graph citizenship is determined solely by whether its
> address resolves to a live (non-tombstone) node in the graph.
> No separate mapping structure exists.

The iced host does not maintain an internal `(TileId, NodeKey)`
sidecar. It looks up node identity in `GraphTree` /
`graphshell-core::graph` by `NodeKey` directly.

### 4.4 Node lifecycle and presentation state (revised 2026-04-29)

**Every tile is a graph node.** There is no non-citizen tile class,
no ephemeral surface that exists outside the graph, and no
Promotion or Demotion lifecycle transition. Opening something
creates a node; that is the only path to a tile. Closing it
deactivates the node; the node persists in the graph.

**Pane** is a spatial term — a leaf region in the FrameTree (see
§4.5). It carries no graph-citizenship implication. Every pane
shows tiles that are graph nodes.

**Presentation state**

Each node in a graphlet has one of two presentation states:

| State | Meaning | Workbench effect |
|---|---|---|
| **Active** | Tile is shown in the pane | Rendered and interactive |
| **Inactive** | Node is in the graphlet; tile is not shown | Not rendered; node and graphlet membership unchanged |

Opening a graphlet shows only its active nodes' tiles. Inactive
nodes are accessible and can be activated at will — they do not
open automatically. Activation state is **per-graphlet**: the same
node has the same active/inactive state across every pane that
shows that graphlet.

The Navigator sidebar is the surface for discovering and toggling
activation: it lists all nodes in the focused pane's graphlet
(active and inactive) and lets the user control which tiles are
shown.

**Three distinct operations on nodes**

| Operation | Domain | Effect | Weight |
|---|---|---|---|
| **Close tile** | Presentation | Node → Inactive. Graph unchanged. | Safe; trivially reversible |
| **Remove from graphlet** | Graph (organizational) | Node leaves this graphlet; stays in full graph. | Deliberate edit |
| **Tombstone** | Graph (node lifecycle) | Node marked deleted. | Destructive; confirmation required |

Promotion and Demotion are retired vocabulary. "Promotion" has no
meaning when every tile is already a graph citizen. "Demotion" is
replaced by the more precise pair: Remove from graphlet
(organizational, non-destructive) and Tombstone (destructive,
confirmation required). TERMINOLOGY.md must be updated to reflect
this — see the S2 correction pass.

### 4.5 Shell-owned FrameTree, Panes, and the canvas base layer (revised 2026-04-29)

**The Shell host owns the FrameTree.** A prior architecture mixed
graph and workbench concerns in a way that gave the Workbench too
much authority over graph structure. The Shell host (iced) owns and
renders the FrameTree — the spatial composition of the OS window.
Workbench-domain concerns express through it, but the FrameTree is
Shell-side infrastructure, not a graph-owned structure.

**Structure: Window → FrameTree → Frames → Panes**

An OS window is a FrameTree. The FrameTree is a nestable tree of
H/V splits. Its internal nodes are **Frames** (split containers);
its leaves are **Panes**. Each Pane has a `GraphletId` and a type.

Frames carry a split axis (H or V) and proportions. They are
adjustable (drag to resize) and nestable to arbitrary depth.
Closing a split collapses the Frame; the remaining sibling expands.

**Two pane types**

| Type | Renders | Scoped to |
|---|---|---|
| **Tile pane** | The active tiles of a graphlet | One `GraphletId`; Navigator controls activation |
| **Canvas pane** | Force-directed graph canvas | One graphlet, the full graph, or a query result |

Both types coexist freely in a FrameTree. A canvas pane and a tile
pane can be split siblings in the same window.

**Canvas base layer**

When the FrameTree has no open Panes — on first launch or after
closing everything — the Shell renders the graph canvas for the
current `GraphId` as the default home state. This is not a
persistent underlayer beneath all Panes; it is the fallback for
an empty FrameTree. Opening a Pane covers it; closing the last
Pane reveals it again.

**Multi-window is convenience, not necessity**

A second OS window is a second FrameTree — useful for multi-monitor
work, popping a Pane into its own window, or separating two
unrelated working contexts. It is never required for seeing multiple
graphlets simultaneously; multiple Panes within one FrameTree
handle that within a single window. (Zed does not require two OS
windows for two split editors. Same principle here.)

**The graph is the full structure**

The graph contains everything: multiple graphlets, orphan nodes,
nodes belonging to several graphlets at once, loose subgraphs. The
FrameTree shows a slice of it; it does not constrain what the graph
contains. The canvas base layer and canvas panes are the surfaces
for seeing and navigating the full structure.

| User action | Implementation |
|---|---|
| Open a new window | Create a new OS window with its own FrameTree |
| Switch graphlets in a pane | Set a new `GraphletId` on that Pane |
| Change pane type | Toggle a Pane between tile and canvas mode |
| Split the workbench | Add a Frame split; assign `GraphletId` and type to new Pane |
| Resize a split | Adjust proportions in the Frame |
| Close a pane | Remove the Pane; parent Frame collapses if one child remains |
| Return to home | Close all Panes; Shell falls back to canvas base layer |

### 4.6 Browser amenities, reshaped (synthesis)

Per [PROJECT_DESCRIPTION.md](../../../PROJECT_DESCRIPTION.md), the
graph is "your tabs as a map you can arrange, save, and share,
instead of a strip at the top of the window." This is the framing
we keep when reshaping browser amenities:

| Amenity | Chrome/Firefox shape | Graph-paradigm shape |
|---|---|---|
| **History** | Linear time-ordered list; back/forward | Two graph-native systems: (1) **edge history** — traversal/origin edges written on navigation events, projected by Navigator as a time-ordered view; (2) **graph memory** — a per-node memory tree with graph affordances (`graph-memory` crate), with potential enrichment into node-memory and edge-memory as distinct concepts. History view is a Navigator projection; no separate history service lives in `graphshell-runtime`. |
| **Bookmarks** | Folder-organized URL list | Tagged graphlets. A "bookmark" is a graphlet tagged as such, with node references that can recreate nodes across graphs. Folder hierarchy maps to nested graphlets. Importing browser bookmarks = a graph population command: create nodes from URL list, organize into a tagged graphlet. Importing browser history is a related path requiring parsing-level semantic autotagging. No separate bookmark-import service; the import path is a one-shot graph write. |
| **Find-in-page** | Within current document | Graph-scoped find: search-in-pane (current viewer's content) and search-in-graph (across nodes). Both Shell-owned commands per [SHELL.md §6](SHELL.md) |
| **Downloads** | Modal list | A subsystem pane (`TileKind::Tool`) addressable as `verso://tool/downloads`. Each download is a graph node; the user can organize it into a graphlet or tombstone it when done. |
| **Devtools** | Browser-internal panel | A single tool pane with sections: a graphshell-level UX overview and inspector (backed by `ux-probes`/`ux-bridge`), plus any subsystem-specific diagnostic sections. Each subsystem may also expose its own tool pane. No separate devtools-family of panes. Servo's devtools remain reachable via the Servo subsystem tool pane. |
| **Sessions / restore** | Reopen-last-tabs | The graph IS the session. There is no separate session restore — opening the app shows the graph, and tile state (active/inactive per node) is recoverable from graph + last-known graphlet projection |
| **Profiles** | Multiple browser users | Multiple graphs, each with its own `GraphId`. Per-graph settings already exist per [SHELL.md §3](SHELL.md) |
| **Multi-window** | Multiple OS windows showing pages | Each OS window is a FrameTree. Multiple windows are a convenience (multi-monitor, pop-out) — not required for multi-graphlet work, which multiple Panes in one FrameTree handle. |

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

- [x] **Workbench model confirmed and revised (2026-04-29)**:
  GraphletView retired. OS window = Shell-owned FrameTree. Leaves
  are Panes (each with a `GraphletId`). Multiple Panes per window
  = multiple graphlets, no second window needed. Canvas base layer
  when FrameTree is empty. §4.5 is the canonical update.
- [x] **Tile lifecycle confirmed and revised (2026-04-29)**:
  every tile is a graph node — no ephemeral non-citizen surface.
  Promotion and Demotion are retired. Three operations: Close tile
  (deactivate), Remove from graphlet (organizational), Tombstone
  (destructive). §4.4 is the canonical update.
- [ ] **TERMINOLOGY.md correction pass** (prerequisite for all
  S2 slice work):
  - Remove GraphletView as a named concept
  - Redefine Pane as a spatial concept (leaf in a Shell-owned
    FrameTree), not a graph-citizenship concept
  - Replace Frame definition with H/V split container (FrameTree
    internal node; Shell-owned)
  - Remove Promotion and Demotion as lifecycle terms entirely
  - Add Active/Inactive as the two presentation states for tiles
    in a graphlet (per-graphlet, Navigator-controlled)
  - Add "Remove from graphlet" as the non-destructive
    organizational graph edit
  - Confirm Tombstone as a separate, confirmation-gated,
    destructive operation
  - Update Workbench: domain authority expresses through the
    Shell-owned FrameTree; remove any singleton/global-container
    framing
- [ ] Define the iced-side composition skeleton (the slot model
  for the iced equivalent of
  [shell_composition_model_spec.md](shell_composition_model_spec.md);
  iced uses panes/`row!`/`column!` instead of egui's named
  panels — the slot identities and authority assignments stay
  the same). Include the FrameTree rendering slot (H/V splits
  within the workbench area) and the active/inactive toggle
  surface for nodes in a graphlet.
- [ ] Specify the omnibar shape for iced: Shell-owned input/dispatch,
  Navigator-owned breadcrumb projection, both rendered through the
  same iced widget per [SHELL.md §6](SHELL.md)
- [ ] Specify command palette behavior in iced terms (the
  `command_palette_state` portable state already exists; this is
  rendering only)
- [ ] Specify each browser amenity per §4.6: which surface, what
  data, what intent flow, what `verso://` address (where applicable).
  For History: confirm what constitutes a "visit" event for the
  edge-history writer (every tile open, explicit navigation only,
  or deduplicated by recency). For Bookmarks: confirm graphlet-tag
  schema and whether cross-graph node references need a stable
  identifier beyond `NodeKey`.
- [ ] Specify the graph coherence guarantee per surface (per §10
  Q8): one sentence per surface stating what graph invariant it
  preserves and how the UI makes it visible. Draw from best-in-
  class browser UX examples reshaped for the graph paradigm.

§10 Q1–Q8 are all confirmed; the specific questions no longer
block slice work. The TERMINOLOGY.md correction pass is the
remaining prerequisite.

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

**Done condition**: each OS window hosts a FrameTree (Shell-owned);
tile panes render active nodes' tiles; canvas panes render a
force-directed graph canvas; inactive nodes are accessible via
the Navigator sidebar but not auto-opened; frame splits are
renderable and adjustable; closing all Panes reveals the canvas
base layer.

This slice materializes §4.4 and §4.5.

Checklist:

- [ ] Define the runtime data flow: `Graph` → `Navigator
  projection pipeline` → `LayoutResult` (active nodes only) →
  iced render via FrameTree Pane
- [ ] Replace any tile-group state that exists today as a
  free-standing structure with a derived view of the active
  graphlet projection
- [ ] Implement FrameTree render: H/V splits with adjustable
  proportions; each leaf is a Pane (tile or canvas type)
- [ ] Implement Navigator sidebar: structured list of focused
  pane's graphlet (all nodes, active and inactive); toggle
  activation from the list
- [ ] Implement canvas pane: force-directed canvas scoped to a
  graphlet, the full graph, or a query result
- [ ] Implement canvas base layer: Shell renders graph canvas
  when FrameTree has no open Panes
- [ ] Demonstrate: closing a tile deactivates the node (Active →
  Inactive); node and graphlet membership unchanged; node does
  not auto-open on next graphlet load
- [ ] Demonstrate: activating an inactive node via Navigator
  opens its tile without changing graph truth
- [ ] Demonstrate: switching graphlets (setting a new `GraphletId`
  on a Pane) switches the tile group without losing graph state
- [ ] Demonstrate: opening a second OS window creates a new
  independent FrameTree; the original window’s state is unaffected
- [ ] Demonstrate: closing all Panes in a window reveals the
  canvas base layer for the current `GraphId`

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

### 8.1 2026-04-28 pruning receipt — embedded host terminology

The viewer/pane render-mode contract no longer describes the
host-drawn path as egui-specific:

- `ViewerRenderMode::EmbeddedEgui` became
  `ViewerRenderMode::EmbeddedHost`.
- `TileRenderMode::EmbeddedEgui` became
  `TileRenderMode::EmbeddedHost`.
- diagnostics and compositor channels now report
  `embedded_host` / `CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_HOST`.
- serde aliases still accept legacy `"EmbeddedEgui"` payloads so
  saved state and older capability fixtures do not break.

Validation:

- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets`
- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry`

### 8.2 2026-04-28 pruning receipt — viewer docs use EmbeddedHost

The authoritative viewer specs now match the code-level render-mode
contract:

- `viewer_presentation_and_fallback_spec.md` lists `EmbeddedHost`
  as the canonical host-drawn render mode.
- `universal_content_model_spec.md` describes non-web viewers as
  `EmbeddedHost` viewers and no longer defines their host-only
  implementation as egui widget code.
- `node_lifecycle_and_runtime_reconcile_spec.md` uses
  `EmbeddedHost` in the representable lifecycle-mode invariant.

Validation:

- `rg -n "EmbeddedEgui|embedded egui|egui::Ui|egui widget" design_docs/graphshell_docs/implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md design_docs/graphshell_docs/implementation_strategy/viewer/universal_content_model_spec.md design_docs/graphshell_docs/implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md`
  returns no stale hits.

### 8.3 2026-04-28 pruning receipt — content surface GL wording

The runtime/content-surface contract now frames shared-wgpu/native
texture presentation as the model and GL as a named legacy callback
fallback:

- `ContentSurfaceHandle::ImportedWgpu` is described as a
  wgpu-compatible host texture, not a Servo GL framebuffer.
- `CallbackFallback` is described as the legacy callback/GL-compat
  path.
- `BackendViewportInPixels` keeps `from_bottom_px` for legacy
  bottom-origin callback math without making OpenGL the host-neutral
  coordinate model.
- `ViewerSurfaceRegistry` comments now identify backing ownership and
  legacy callback contexts instead of making "has GL context" the
  authority check.

Validation:

- `rg -n "GL framebuffer|OpenGL convention|register a GL callback|compat GL|has GL context" crates/graphshell-runtime/src/content_surface.rs crates/graphshell-runtime/src/ports.rs shell/desktop/workbench/compositor_adapter.rs`
  returns no stale hits.

### 8.4 2026-04-28 pruning receipt — app selection prune seam

The app-layer workbench selection prune no longer requires callers to
think in egui_tiles terms:

- `GraphBrowserApp::prune_workbench_pane_selection_to_live_set`
  accepts a plain `HashSet<PaneId>` and owns the selection-retain /
  primary-pane repair logic.
- the egui-host-only `prune_workbench_pane_selection` method now just
  extracts live `PaneId`s from the temporary `egui_tiles::Tree` adapter
  and delegates to the host-neutral helper.
- `workspace_state.rs` and `persistence_facade.rs` comments now refer
  to host layout trees / workbench-layout JSON where the old wording
  implied egui_tiles was the lasting app-layer authority.

Validation:

- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets`
- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry`

### 8.5 2026-04-29 pruning receipt — app selection update seam

Workbench pane-selection update validity now has the same app-owned
shape as selection pruning:

- `GraphBrowserApp::update_workbench_pane_selection_if_live`
  accepts a plain `HashSet<PaneId>`, prunes stale selection, verifies
  the requested pane is still live, and applies the selection update.
- `workbench_surface::selection_ops` computes the live `PaneId` set
  from the temporary egui_tiles adapter and delegates to the
  app-owned helper instead of open-coding the validity check against
  tile-tree internals.
- group-selection handling now also delegates pruning to
  `prune_workbench_pane_selection_to_live_set`, leaving the tile tree
  traversal as adapter code.

Validation:

- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets`
- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry`

### 8.6 2026-04-29 pruning receipt — adapter boundaries and retired input debt

This pass took the requested slices in order:

- `ux_tree_and_probe_spec.md` now uses `EmbeddedHost` and describes
  UxTree's structural source as the active workbench layout source:
  GraphTree walker target, egui_tiles adapter during transition.
- Register-layer Sector B docs no longer treat inherited gamepad
  bindings as active debt. Gamepad can return only through a new
  Graphshell-native input design.
- `pane_model.rs` no longer names `egui_tiles::Tree<TileKind>` as the
  durable layout authority; it frames `TileKind` as current host-layout
  adapter payload.
- `workbench_surface::selection_ops` now isolates PaneId/TileId adapter
  translations in helper functions.
- `graph_tree_sync.rs` derives node-pane maps from one shared adapter
  snapshot instead of three separate tile-tree scans.

Validation:

- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features iced-host,wry --all-targets`
- `cargo check --manifest-path repos/graphshell/Cargo.toml --no-default-features --features egui-host,wry`

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
- **Risk: TERMINOLOGY.md correction lag.** §4.4 and §4.5
  retire GraphletView, Promotion, Demotion, and the ephemeral
  pane concept, and introduce Shell-owned FrameTree, Pane as
  spatial, Active/Inactive, Remove from graphlet, and canvas base
  layer. Until TERMINOLOGY.md is updated, code review, PR
  descriptions, and new design docs will use inconsistent
  vocabulary. **Mitigation**: the TERMINOLOGY.md correction pass
  is the first item in the S2 checklist and a hard prerequisite
  for any S2 slice work. It is a doc-only change that can land
  immediately, before any implementation work begins.
- **Risk: testing during the transition.** With egui frozen and
  iced-still-building, neither host is fully usable for a span.
  **Mitigation**: the runtime layer (`graphshell-core`,
  `graphshell-runtime`) is testable without any host; ride that
  for unit + integration tests during S3/S4. Manual exploration
  resumes when S3 produces a runnable iced binary.

---

## 10. Open questions

All eight questions from 2026-04-28 are confirmed as of 2026-04-29.
Resolutions are summarised here; the canonical updates are in
§4.4 and §4.5. TERMINOLOGY.md requires a correction pass before S2
slice work begins (noted per question below).

1. **Workbench/graphlet binding.** ✅ **Confirmed and further
   revised (2026-04-29)**: GraphletView is retired. An OS window
   is a FrameTree (Shell-owned). FrameTree leaves are Panes; each
   Pane carries a `GraphletId`. Multiple Panes in one window
   handle multiple simultaneous graphlets without requiring
   multiple OS windows. See §4.5. TERMINOLOGY.md: remove
   GraphletView; update Workbench to reflect Shell-owned FrameTree.

2. **Tile lifecycle.** ✅ **Confirmed and revised (2026-04-29)**:
   every tile is a graph node — no ephemeral non-citizen surface
   exists. Closing a tile deactivates the node (Active →
   Inactive). Promotion and Demotion are retired terms. The three
   operations are: Close tile (deactivate, safe), Remove from
   graphlet (organizational edit, deliberate), Tombstone
   (destructive, confirmation required). See §4.4.
   TERMINOLOGY.md: remove Promotion and Demotion; add Remove from
   graphlet and Active/Inactive; redefine Pane as spatial.

3. **Frame ↔ Workbench composition.** ✅ **Confirmed**: Frames are
   nestable H/V split containers — the spatial composition
   mechanism for workbenches. A Frame is not a saved layout preset
   or a pane type; it is a split node in the FrameTree. The frame
   arrangement is reflected in the graph as a bounded region. See
   §4.5. TERMINOLOGY.md: replace the prior Frame definition with
   this spatial meaning.

4. **Multi-window scope.** ✅ **Confirmed and revised
   (2026-04-29)**: each OS window is a FrameTree. Multiple windows
   are a convenience (multi-monitor, pop-out) — not required for
   multi-graphlet work, which multiple Panes in one FrameTree
   handle. Coordination of windows to the same graph is TBD and
   not blocking S3/S4. See §4.5.

5. **History reshape.** ✅ **Confirmed**: two graph-native history
   systems. (1) Edge history: traversal/origin edges written on
   navigation, projected by Navigator as a time-ordered view.
   (2) Graph memory: per-node memory tree with graph affordances
   (`graph-memory` crate); potential direction is enriching it
   with node-memory and edge-memory as distinct concepts. Neither
   system lives in `graphshell-runtime`; Navigator owns the
   read projection. More design work needed on both fronts before
   S4 history surface work begins.

6. **Bookmarks reshape.** ✅ **Confirmed**: bookmarks are tagged
   graphlets with node references that can recreate nodes across
   graphs. Import = a one-shot graph population command: nodes
   from a URL list, organized into a tagged graphlet, folder
   hierarchy → nested graphlets. Browser history import is a
   related path requiring parsing-level semantic autotagging. No
   separate bookmark-import service; the import path is a
   one-shot graph write.

7. **Devtools surface.** ✅ **Confirmed**: a single tool pane with
   sections — a graphshell-level UX overview and inspector
   (backed by `ux-probes`/`ux-bridge`), plus subsystem-specific
   sections as needed. Each subsystem may additionally expose its
   own tool pane. No separate devtools-family of panes. Servo's
   devtools remain reachable via the Servo subsystem tool pane.
   See updated §4.6 row.

8. **Refined UI fidelity.** ✅ **Confirmed**: coherence (b) is the
   primary bar — graph + workbench feel like one continuous
   experience; no surface forces the user to forget the graph to
   accomplish a task. Polish (a) and accessibility/keyboard
   parity (c) are downstream bars layered on top. The S2 UX
   target document should specify a **graph coherence guarantee**
   per surface: a one-sentence statement of what graph invariant
   the surface preserves and how the UI makes it visible. Best-
   in-class browser UX examples (from the GOATs) are reference
   material for the coherence bar, reshaped for the graph
   paradigm rather than copied from Chrome/Firefox.

---

## 11. Gap register

Cross-referenced against the [browser subsystem taxonomy and
Graphshell mapping](../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md).
Items are blind spots not addressed in the slice plan above — design
decisions that must be made before the relevant tier's slices can
land. Each item carries an owning domain and a taxonomy § reference.
Items the plan explicitly addresses are excluded here; see §4 and
§10 for those.

### Tier 1 — Resolve before S2 (gates the UX target document)

| # | Gap | Domain | Taxonomy |
|---|---|---|---|
| **G1** | **Six-track focus model reconciliation.** The existing six-track `RuntimeFocusAuthorityState` (SemanticRegion / PaneActivation / GraphView / LocalWidget / EmbeddedContent / ReturnCapture) has no direct mapping in iced's widget-focus model. Must be reconciled before the composition skeleton, Navigator sidebar, or command surfaces can be specified. | Input / Shell | §3.7, §4.5 |
| **G2** | **All three command surfaces.** S2 specifies command palette only. Context palette (right-click, context-dependent) and Radial palette (positional radial menu) need iced designs. Context palette shape depends on the right-click target — tile, canvas node, Frame border, Navigator item, or canvas base layer — each with a different action surface. | Shell | §3.6 |
| **G3** | **Navigator breadcrumb shape.** The omnibar spec says "Navigator-owned breadcrumb projection" but what does the breadcrumb represent in the new model? The focused tile's URL? The graphlet name? The path from root graph to current graphlet? Different data, different visual shape. Must be resolved before the omnibar is specifiable. | Navigator / Shell | §3.5, §3.6 |
| **G4** | **Omnibar scope.** The old `OmnibarSearchSession` used per-pane drafts. With multiple Panes visible simultaneously, is the omnibar per-pane (tracks focused pane) or global (one bar, always reflects the focused pane)? Both are defensible; S2 must pick one. | Shell / Navigator | §3.5, §3.6 |

### Tier 2 — Resolve before S3 (gates host runtime closure)

| # | Gap | Domain | Taxonomy |
|---|---|---|---|
| **G5** | **`WebViewSurface<NodeKey>` widget design.** S3 lists `HostSurfacePort` implementation but the actual work is the iced-native `WebViewSurface` widget: texture lifecycle, content generation signals, pointer / keyboard / IME event forwarding to Servo. Covered in depth by the [content-surface scoping doc](2026-04-24_iced_content_surface_scoping.md) §4.3 but not yet in the slice plan. | Shell (host) / Viewer | §3.1, §3.6 |
| **G6** | **`BackendTextureToken` retirement.** The shared-wgpu path currently wraps `egui::TextureId`. The iced path uses direct `wgpu::Texture` references inside widget state; the egui-flavoured token must be retired before the iced content-surface path works. See [content-surface scoping doc](2026-04-24_iced_content_surface_scoping.md) §4.1 option C. | Shell (host) | §3.1 |
| **G7** | **Servo wgpu dependency.** The iced content-surface path is shared-wgpu-only — no GL callback fallback for iced. It depends on Servo producing wgpu textures cleanly via the `webrender-wgpu` migration. GL callback fallback is explicitly out of scope for the iced host; this should be a stated constraint in S3's done condition. | Shell (host) / Viewer | §3.1 |
| **G8** | **IME handling.** Not mentioned in the plan. Egui delegated to its own text-input widget state; iced needs winit IME events routed through iced's input pathway. In-page IME for Servo content must additionally be forwarded through `WebViewSurface`. CJK / Arabic input is invisible in testing and catastrophic in production without it. | Input / Shell | §3.7 |
| **G9** | **AccessKit tree for FrameTree / Pane model.** S3 says "implement `HostAccessibilityPort` via `accesskit` direct" but the AccessKit tree for the new model does not exist: Pane, Frame split, active tile, and inactive node have no accessibility roles defined. Hosted webview a11y forwarding through `WebViewSurface` also needs explicit design. | Accessibility / Shell | §3.7 |
| **G10** | **Shutdown persistence wiring.** Graph snapshot is currently wired to `EguiHost::drop`. The iced host needs an equivalent shutdown hook so graph state is not lost on clean exit or forced quit. | Shell (host) / Storage | §3.4 |

### Tier 3 — Resolve before S4 (gates surface implementation)

| # | Gap | Domain | Taxonomy |
|---|---|---|---|
| **G11** | **FrameTree persistence schema.** FrameTree (Frame split axes, proportions, Pane types, `GraphletId` per leaf) is a new concept with no entry in the current persistence model. Without it, layout state is lost on restart. Requires a schema decision before session restore is specifiable. | Shell / Storage | §3.4, §3.5 |
| **G12** | **Active / Inactive state persistence.** Per-graphlet activation state is new. Where does it live persistently — graph WAL (node property), workbench manifest, or a separate layer? The answer affects session restore and cross-device sync semantics. | Graph / Storage | §3.4, §3.5 |
| **G13** | **Node creation timing (Bet 2 mechanics).** Every navigation creates a node — but at what point? URL entry, first byte, load completion? What happens on failed navigation (404, cert error, timeout)? What is the node's initial state while loading? What happens when the user navigates away before load completes? | Graph | §3.5 |
| **G14** | **Find-in-pane implementation shape.** §4.6 names it but leaves the shape unspecified: Servo's find-in-page API, iced toolbar overlay placement, Ctrl+F intercept routing before iced consumes it. Distinct from graph search (Ctrl+G); requires explicit design. | Shell / Viewer | §3.6, taxonomy §6 #10 |
| **G15** | **Downloads progress surface.** The plan makes downloads graph nodes but does not address in-progress state (filename, source, bytes, ETA, cancel). The ❏ in the taxonomy was never about persistence — it was about the live progress UI. | Shell | §3.6, taxonomy §6 #2 |

### Tier 4 — Open through S5; need eventual decisions

| # | Gap | Domain | Taxonomy |
|---|---|---|---|
| **G16** | **Graph Reader.** Planned virtual accessibility tree that lets screen readers navigate nodes and edges rather than rendered pixels. WCAG 2.2 AA target is load-bearing on this; not referenced in the plan. | Accessibility | §3.7 |
| **G17** | **UxTree population for iced host.** The `UxTree` runtime snapshot must be populated from the iced host's render tree for `UxProbeSet` tests and the planned WebDriver bridge. No mechanism is specified for iced. | Shell (host) / UX semantics | §3.8, §4.6 |
| **G18** | **Permission prompts.** Servo triggers camera / microphone / location / notification prompts for web content. These need to route through `WebViewSurface` to iced-native prompt surfaces. Currently egui-hosted with no iced equivalent specified. | Shell / Security | §3.10 |
| **G19** | **Nostr signing prompt.** The `nip07_bridge` signing boundary (🔨 Active) needs a signing prompt UI in the iced host. Not mentioned anywhere in the plan. | Shell (host) | §3.13 |
| **G20** | **Canvas pane scope and performance policy.** A canvas pane showing the full graph for a large graph is an unaddressed performance question. Default scope for a newly opened canvas pane, level-of-detail / culling policy, and whether `graph-memory` relevance affects visibility are all open. | Graph / Shell | §4.1, §4.2 |
| **G21** | **Verso pairing UI.** P2P sync (🔨 Active) requires a pairing flow (QR code, confirmation, session capsule display). The iced host must present this. Not mentioned. | Shell (host) | §3.13 |
| **G22** | **Diagnostics Inspector tool pane spec.** The Diagnostics Inspector (🔨 Active, `ChannelRegistry` + ring buffer) needs a concrete iced tool-pane spec. "Each subsystem may additionally expose its own tool pane" in §4.6 is insufficient. | Shell / Diagnostics | §3.8, §3.11 |
| **G23** | **Touch input stance.** ❏ in taxonomy; not addressed in the plan. Needs a decision before S4 input work is considered done, particularly if graphshell targets Linux tablets or other touch-primary form factors. | Input | §3.7 |

---

## 12. Idiomatic iced — programming model and stages

Sections §1–§11 above describe what to build (slices), what to avoid
(anti-patterns), and where the gaps are. This section is the positive
corollary to §5: what idiomatic iced *does* look like, mapped to the
[browser subsystem taxonomy](../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md).
It is structured as the parallel to the
[gpui plan's "Idiomatic GPUI Adaptation"](2026-04-27_gpui_host_integration_plan.md)
section so the host-framework comparison stays symmetric.

### 12.1 Programming-model summary

iced is The Elm Architecture: one `Application`, single mutation point
in `update`, pure derivation in `view`, `Subscription` for clocks /
winit / async streams. The portable contract (`runtime.tick()`,
`FrameViewModel`, `HostPorts`) was designed for exactly this shape —
that's why iced was picked. "Designed for" is not the same as
"idiomatic by default"; the scaffold has several choices to make.

The defining iced idioms:

| Idiom | What it does |
|---|---|
| `Application::view(&self) -> Element<Message>` | Pure derivation; runs per frame |
| `Application::update(&mut self, msg) -> Command<Message>` | Single mutation point; emits async via `Command` |
| `Subscription` | Time / winit / async streams merged into one Message stream |
| `pane_grid::PaneGrid<Pane>` | Native resizable, nestable layout primitive |
| `canvas::Program<Message>` + `Program::State` | Retained 2D drawing with hit-testing; widget-local state |
| `shader` widget | Direct wgpu access; Vello and external textures live here |
| `Element<Message>` | Universal composition currency |
| `widget::Operation` | Imperative widget-level actions (focus moves, scroll-to) |
| `iced::Theme` | Theming system; `libcosmic` extends |
| `text_input` (iced 0.14) | IME-aware text entry |
| `iced_accessibility` | AccessKit bridge |
| `iced_aw` widgets | Tabs, ContextMenu, Sidebar, Modal |
| `iced_webview` | Web content embedding (Servo / Blitz / litehtml / CEF feature flags) |
| `libcosmic` widgets | List/grid views, drag-drop, theme extensions |

### 12.2 Per-subsystem mapping — idiomatic shape

Cross-referenced against the
[browser subsystem taxonomy](../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md).
This is the iced-side detail; the
[gpui plan §Idiomatic GPUI Adaptation](2026-04-27_gpui_host_integration_plan.md)
holds the parallel gpui-side detail.

| Taxonomy subsystem | Idiomatic iced shape |
|---|---|
| §3.6 FrameTree (Window → splits → Panes) | `pane_grid::State<Pane>` *is* the FrameTree authority; `pane_grid` widget renders it; resize and drag are built in |
| §3.6 Tab bar over active tiles | `iced_aw::Tabs` inside each tile pane |
| §3.6 Omnibar | `text_input` + `Subscription` for focus/results; Navigator-projected breadcrumb in a `row!`; per-pane drafts via existing `OmnibarSearchSession` |
| §3.6 Command palette | `Modal` overlay + filtered list driven by Messages routed through `ActionRegistry` |
| §3.6 Context palette | `iced_aw::ContextMenu` triggered by mouse-right Message |
| §3.6 Radial palette | Custom `canvas::Program` overlay (radial geometry isn't a built-in widget) |
| §3.6 Toasts | `Stack` widget + custom toast Element + `Subscription` for timeout |
| §3.1 Graph canvas (Vello) | `canvas::Program` for hit-testing; camera/hover/drag state in `Program::State` (**not** Application); Vello scene rendered via `shader` widget |
| §3.1 `WebViewSurface<NodeKey>` | Custom widget consuming `iced_webview` Servo feature, or direct `shader` widget over Servo's wgpu external image API per [iced-rs/iced#183](https://github.com/iced-rs/iced/pull/183); texture lifecycle in widget state |
| §3.1 Wry viewer | `iced-wry-viewer` already exists in the Cargo tree; consume directly |
| §3.7 Six-track focus (G1) | LocalWidget = iced per-widget focus via `widget::focus()` `Operation`; the other five tracks live in `graphshell-runtime`; iced widgets read runtime focus state in `view` |
| §3.7 IME (G8) | `text_input` IME (iced 0.14); Servo IME forwarded through `WebViewSurface` |
| §3.7 AccessKit (G9) | `iced_accessibility` integration |
| §3.5 Navigator sidebar | `scrollable` + lazy `column!` of items derived from `FrameViewModel`; activation toggle dispatches HostIntent |
| §3.5 History view | `lazy` + `scrollable` for virtualization |
| §3.8 Diagnostics Inspector | Same shape — `lazy` + `scrollable` over the channel ring buffer |
| §3.6 Settings panes | `column!` of forms; route through `verso://settings/<section>` Messages |
| §4.5 Canvas base layer | Application root `view` returns either `pane_grid` or graph canvas widget when FrameTree is empty |
| §3.6 Theming | `iced::Theme` extension with palette derived from `settings_persistence` |
| §3.13 Async work | `Command::perform` for one-shots, `Subscription` for streams, `cosmic-time` for animations |

### 12.3 Stages of idiomatic adoption

§6 organizes work by *what to build* (S0–S6). The stages below organize
by *how iced should be used* — they cut across slices. Each stage
tightens iced idiom; the previous stage stays valid.

#### Stage A — Application + Subscription closure (S3 done condition)

`Application` owns Runtime; `update` dispatches Messages → HostIntents;
`view` consumes `FrameViewModel`; `Subscription` drives tick (60Hz),
winit input, async results. All 24 `todo(m5)` markers in
`iced_host_ports.rs` resolved.

This is where the S3 done condition lands.

#### Stage B — `pane_grid` for FrameTree (overlaps S5)

`pane_grid::State<Pane>` is the FrameTree authority — not a
side-structure, not a hand-rolled split tree. H/V splits are
`pane_grid` splits; resize is built-in. Each Pane renders as a tile
pane (active tiles in a graphlet) or canvas pane (graph canvas).

Buys: native split rendering and resize without writing a layout
engine; pane focus integrates with iced's built-in focus model.

#### Stage C — Canvas Program with local state (overlaps S4)

The graph canvas uses a `canvas::Program<Message>`. Camera (pan/zoom),
hover state, drag interaction state live in `Program::State`, not in
`Application`. Vello renders through the `shader` widget; canvas
Program handles overlay drawing + input.

This is the §5 anti-pattern correction made concrete: don't thread
camera state through Messages each frame. The egui scaffold's
camera-in-app-state is the failure mode being avoided.

#### Stage D — `WebViewSurface` as custom widget

Custom widget with explicit `wgpu::Texture` lifecycle, focus
integration via `widget::Operation`, IME forwarding to Servo through
`web_runtime`. Built on `iced_webview`'s Servo feature path or a
direct shader-widget impl. G5 / G6 in §11 are the home of this work.

Buys: native focus integration (Tab cycles into web content), iced
input event flow to Servo, no shader-widget wrapper at the chrome
boundary.

#### Stage E — AccessKit + IME closure

`iced_accessibility` integration; `text_input` IME shipped in chrome
inputs; Servo IME forwarding through `WebViewSurface`. G8 / G9 in §11.

Buys: WCAG 2.2 AA path; CJK / Arabic users covered without leaving
testing-invisible regressions.

#### Stage F — Theme + style consolidation

Inline styles consolidated into an `iced::Theme` extension; settings
drive theme; `libcosmic` compatibility considered if COSMIC DE
first-class citizenship becomes a target.

Buys: dark/light/high-contrast variants without per-widget edits;
themable distribution if Graphshell ever ships as a libcosmic
applet-like surface.

### 12.4 Cross-reference to existing slices

| Jump-ship slice | Idiomatic stage |
|---|---|
| S0 (Cargo gate, landed) | pre-stage |
| S1 (Freeze egui) | pre-stage |
| S2 (UX target doc) | pre-stage |
| S3 (Iced host runtime closure) | **Stage A** |
| S4 (Iced surfaces to UX target) | **Stages B + C + D + E**, surface-by-surface |
| S5 (Graphlet projection plumbing) | **Stages B + C** primarily |
| S6 (Delete egui) | post-stage receipt |

The stages are an orthogonal lens on the same work. Reading §6
slice-by-slice tells you *what to build*; reading §12 stage-by-stage
tells you *how it should look once built*. Both are necessary.

### 12.5 Where the scaffold lands today

The scaffold is mid-Stage A. The 24 `todo(m5)` markers in
`shell::desktop::ui::iced_host_ports.rs` are the Stage A closure.
`iced_graph_canvas.rs` is described in S4 as "a starting point;
promote it to a real `CanvasBackend<NodeKey>` impl consuming
`ProjectedScene`" — that promotion *is* the Stage C move.

Stages B (pane_grid for FrameTree), D (WebViewSurface widget), E
(AccessKit + IME), and F (Theme consolidation) are S4 / S5 work that
hasn't started.

### 12.6 Relationship to §5 anti-patterns

§5 says what not to do; §12 says what to do instead. The two together
close the circle.

The single biggest "iced-shaped egui" risk in the current scaffold is
**camera state in `Application`**. Egui's immediate-mode habits push
toward putting camera/hover/drag state at the top and threading deltas
through messages each frame. Iced's `canvas::Program::State` is the
right home; that is the Stage C correction. The §5 rule names it; the
stage makes it concrete.

A second risk is **polling `runtime` for state instead of subscribing**
to its events. Per-frame polling works in iced because `view` runs
every frame, but it leaves the runtime's event stream unused. When
the runtime emits an event (graph mutation, network result,
diagnostics channel push), prefer a `Subscription` so `update` only
runs on real changes. This becomes load-bearing at Stage A's done
condition (the 60Hz tick is a `Subscription`; per-frame polling on
top of it is doubly redundant).

A third risk is **manual tabs replicating egui_tiles**. The
`egui_tiles::Tree<TileKind>` model is what we're escaping; do not
reimplement its tab semantics on top of iced. Use `iced_aw::Tabs`
inside tile panes and let the FrameTree (`pane_grid`) handle
splits — the two abstractions are orthogonal in iced, while
egui_tiles conflated them.

---

## 13. Bottom line

Stop patching the boat. Egui is broken, freeze it where it is, build
iced to a refined UX target grounded in the existing five-domain
docs, reshape browser amenities for the graph paradigm rather than
copying chrome, and delete egui when the iced binary covers the
target. The portable contract (graphshell-core, graphshell-runtime,
graph-canvas, graph-tree, HostPorts, FrameViewModel) is the asset
that makes this cheap.

Receipts in §8 are the done condition. Gaps in §11 are the
comprehensive blind-spot register. Stages in §12 are how iced should
look once built. Questions in §10 are the gating input for S2.
Everything else is ordered work.
