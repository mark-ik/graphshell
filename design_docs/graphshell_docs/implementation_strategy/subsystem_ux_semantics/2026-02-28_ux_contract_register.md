# UX Contract Register

**Date**: 2026-02-28  
**Status**: Implementation-guiding  
**Purpose**: Define how Graphshell UX should be specified so behavior, ownership, and issue planning remain aligned with architectural boundaries.

**Relates to**:

- `2026-03-01_ux_execution_control_plane.md`
- `workbench/workbench_frame_tile_interaction_spec.md`
- `canvas/graph_node_edge_interaction_spec.md`
- `aspect_command/command_surface_interaction_spec.md`
- `subsystem_focus/focus_and_region_navigation_spec.md`
- `viewer/viewer_presentation_and_fallback_spec.md`
- `aspect_control/settings_and_control_surfaces_spec.md`
- `aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`
- `../research/2026-02-27_egui_stack_assessment.md`
- `2026-02-24_control_ui_ux_plan.md`

**Adopted standards** (see [standards report](../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7):

- **WCAG 2.2 Level AA** — the UX contract register and all canonical specs implement WCAG 2.2 AA as the cross-cutting conformance target for all interactive Graphshell surfaces
- **OpenTelemetry Semantic Conventions** — all diagnostics obligations in canonical specs follow OTel naming/severity conventions
- **OSGi R8** — capability declaration and registry vocabulary used in viewer, command, and settings contracts

**Cross-doc audit artifact**:

- `2026-03-04_model_boundary_control_matrix.md`

---

## 1B. Standard Control Paradigm Baseline

Before adding novel control semantics, Graphshell must cover the standard control paradigm that users expect from a spatial browser:

| Paradigm area | Standard expectation | Graphshell coverage |
| --- | --- | --- |
| Spatial graph interaction | Select, multi-select, inspect, activate/open, drag, lasso, pan, zoom, fit/reset, keyboard equivalents | `graph_node_edge_interaction_spec.md` §§3.1–3.5 |
| Workspace management | Create/switch panes, split, group/tab, reorder, close, recover, history | `workbench_frame_tile_interaction_spec.md` §§4.1–4.6 |
| Unified command invocation | Keyboard shortcuts, searchable command palette, contextual menu/palette, optional radial shortcuts | `command_surface_interaction_spec.md` §§4.1–4.6 |
| Focus/navigation model | One semantic focus owner, modal capture, deterministic return path, region cycling, skip links | `focus_and_region_navigation_spec.md` §§4.1–4.7 |
| Tool surfaces | Settings, history, diagnostics, import as first-class app surfaces | `settings_and_control_surfaces_spec.md` §§4.1–4.5 |
| Viewer-state clarity | Loading, partial, placeholder, fallback, retry/override affordances | `viewer_presentation_and_fallback_spec.md` §§4.1–4.5 |
| Accessibility baseline | All critical drag actions have non-drag alternatives, consistent labels, visible focus, no hidden failures | `focus_and_region_navigation_spec.md` §4.7, `design/accessibility_baseline_checklist.md` |

**Policy**: Graphshell must phrase its controls in these standard terms before layering novel semantics on top. Novelty that contradicts baseline expectations must be documented with a rationale in the relevant spec.

---

## 1. Why This Exists

Graphshell currently has UX intent spread across multiple documents and partial implementation paths.

That is useful for ideation, but insufficient for execution.

To build a coherent application, Graphshell needs one stable place that says:

- what the user should be able to do,
- what each interaction means,
- which subsystem owns the behavior,
- what the framework is allowed to do versus not do,
- how the behavior is verified,
- how UX work is translated into issues without violating architecture.

This register is that bridge.

It is not a visual design doc and not a widget spec. It is a behavior-and-ownership register.

---

## 1A. Canonical Spec Family

Graphshell should keep a small canonical UX spec family rather than proliferating one spec per micro-domain.

The current canonical set is:

1. `workbench/workbench_frame_tile_interaction_spec.md`
2. `canvas/graph_node_edge_interaction_spec.md`
3. `aspect_command/command_surface_interaction_spec.md`
4. `subsystem_focus/focus_and_region_navigation_spec.md`
5. `viewer/viewer_presentation_and_fallback_spec.md`
6. `aspect_control/settings_and_control_surfaces_spec.md`

This register is the meta-layer over that family.

- The six specs define canonical subsystem contracts.
- This register defines how those contracts are organized, owned, and translated into issues.
- The milestone checklist defines what must be implemented first.

The design rule is:

- keep subsystem boundaries explicit,
- keep the number of canonical specs small enough to maintain,
- fold narrow concerns into the nearest owning subsystem instead of creating standalone spec sprawl.

---

## 2. Core Rule

UX must be specified as **interaction contracts**, not as scattered UI wishes.

Every important UX behavior should be defined as:

1. intent
2. trigger
3. preconditions
4. semantic result
5. focus result
6. visual result
7. degradation result
8. owner
9. verification

If a behavior cannot be expressed in that form, it is not ready to become implementation work.

### 2A. Contract Template (normative)

All canonical subsystem specs must use this contract template for normative entries:

1. intent
2. trigger
3. preconditions
4. semantic result
5. focus result
6. visual result
7. degradation result
8. owner
9. verification

The template is mandatory for new contract sections and for significant contract rewrites.

---

## 3. Ownership Model

### Graphshell owns

If the behavior would still matter after swapping UI/rendering libraries, it belongs to Graphshell.

Graphshell owns:

- semantic graph meaning
- selection truth
- focus ownership
- camera policy
- pane lifecycle meaning
- tile/node creation semantics
- command semantics
- viewer routing and render-mode policy
- fallback/degradation policy
- persistence rules
- diagnostics policy

### The egui stack owns

The egui stack is an implementation backend, not a UX authority.

It may own:

- widget layout
- pane/tab geometry
- local hover/focus mechanics
- per-frame visual state
- raw event observation
- paint execution

It must not be the owner of semantic UX behavior.

### Contract implication

Every UX issue should explicitly say:

- what Graphshell decides,
- what the framework only helps render or observe.

That keeps the UX plan compatible with the current stack and with future migrations.

---

## 3A. Canonical Data, View, and Workspace Model

Graphshell UX depends on keeping content truth, navigation projections, and workspace arrangement distinct.

### Content model

- `GraphId` is the canonical content space and graph-truth boundary.
- Nodes and edges are durable app data, with stable identity and explicit relationship semantics.
- Hierarchical containment may exist as one constrained relationship type, but it is not the only valid organizing relation in the graph.
- The graph may represent a filesystem-like hierarchy, but Graphshell must not collapse the whole content model into a tree-only truth model unless a future subsystem explicitly adopts that constraint.

### View model

- `GraphViewId` is a scoped view instance within a `GraphId`.
- A `GraphViewId` includes:
  - scope definition (which node set, subgraph, or collection is in-play by default)
  - lens state (camera, selection memory, filters, and local focus memory)
- `GraphViewId` is independent of pane hosting. A pane may present a `GraphViewId`, but it does not define the view's identity.

### Navigation projections

- The file tree is a hierarchical navigator over graph-backed items exposed through a designated containment relation.
- The file tree may contain content nodes, saved views, collections, imported filesystem projections, or another explicitly declared subset of graph-backed artifacts.
- The file tree is a UI projection and navigation surface, not the canonical owner of content identity or graph truth.
- The graph canvas is the primary relational navigation surface over the broader relation space; the file tree is the lower-complexity hierarchical navigation surface.
- File-tree projection updates (source switch, projection rebuild, row selection changes) are reducer-owned semantic actions and must emit `ux:navigation_transition` diagnostics receipts when they change effective navigation state.

### Workspace model

- Workbench frames, tiles, and panes are workspace arrangement state.
- The workbench may host one or more `GraphViewId` instances plus tool and viewer surfaces.
- Workbench state may persist arrangement and return-path context, but it must never be the canonical owner of content identity or durable hierarchy.

### 3B. Model Boundary (normative shorthand)

- `GraphId` = truth boundary (durable content semantics).
- `GraphViewId` = scoped view state (camera/lens/selection memory/filter scope).
- file tree = graph-backed hierarchical projection (navigation surface, not content truth).
- workbench = arrangement boundary (pane/tile/frame hosting only).

All canonical specs in this register family inherit this shorthand and must not redefine it.

### 3C. Terminology Lock

- Never call tile order or frame arrangement a content hierarchy.
- Never call the file tree content truth or graph identity authority.
- Never call physics presets camera modes.

---

## 4. UX Domain Model

Graphshell UX should be planned by domain, not by screens.

The current recommended UX domains are grouped into the six canonical specs:

1. **Workbench / Frame / Tile**
   - pane lifecycle, split, reorder, close, history, recovery
2. **Graph / Node / Edge**
   - navigation, camera, selection, manipulation, graph-to-workbench routing
3. **Command Surfaces**
   - keyboard commands, command palette, radial menu, omnibar command dispatch, contextual invocation
4. **Focus and Region Navigation**
   - semantic focus ownership, region cycling, capture, return paths, escape hatches
5. **Viewer Presentation and Fallback**
   - content visibility, placeholder and degraded states, overlay affordances, viewer-state clarity
6. **Settings and Control Surfaces**
   - settings pages, history surfaces, diagnostics panes, import/control pages, apply and return-path behavior

Some previously separate concerns are intentionally folded into these owning specs:

- **Search and Retrieval** folds into **Command Surfaces** unless it grows into a substantially larger subsystem.
- **File Tree / Hierarchical Navigation** folds into **Graph / Node / Edge** as a navigation projection over graph-backed content unless it grows into a substantially larger subsystem.
- **Feedback, Diagnostics, and Recovery** is mandatory inside each subsystem spec rather than a standalone canonical spec.
- **Accessibility** is cross-cutting and must appear in every major spec, with **Focus and Region Navigation** carrying the explicit cross-app navigation contract.

These domains should remain stable enough to organize issues and tests, even when implementation changes.

---

## 5. UX Contract Template

Each UX behavior contract should use the following shape.

### Contract Header

- **Domain**
- **Behavior Name**
- **Priority** (`core`, `important`, `deferred`)
- **Current Status** (`implemented`, `partial`, `missing`, `unstable`)

### User Intent

- What the user is trying to accomplish.

### Trigger

- The exact initiating action:
  - pointer
  - keyboard
  - command
  - internal redirect

### Preconditions

- What must already be true for this behavior to apply.

### Semantic Result

- What changes in app meaning/state.

### Focus Result

- Who owns focus after the action.

### Visual Result

- What the user should visibly perceive.

### Degradation / Failure Result

- What happens if the ideal path is unavailable.

### Authority

- **Primary owner** in Graphshell
- **Observers**
- **Framework role** (`paint only`, `layout only`, `event source only`)

### Verification

- Which test type proves it
- Which diagnostics prove it
- Which docs must stay in parity

### Issue Linkage

- Which GitHub issues implement or track the behavior

---

## 6. Domain Register

The sections below define the app's current UX planning backbone.

### 6.1 Graph / Node / Edge

**Purpose**: The graph must be navigable, predictable, and user-controlled.

**Core behaviors**

- Pan the active graph pane
- Zoom the active graph pane
- Reset / fit the active graph pane
- Route camera commands to the correct graph view in multi-pane contexts

**Primary owner**

- Graphshell camera controller

**Framework role**

- event source and paint only

**Key failure to prevent**

- camera dual-authority or focus-dependent no-op behavior

This domain also includes:

- single node selection
- additive selection
- lasso selection
- node drag
- group drag

**Related spec**

- `canvas/graph_node_edge_interaction_spec.md`

### 6.2 Workbench / Frame / Tile

**Purpose**: Opening, splitting, focusing, and closing panes must be deterministic.

**Core behaviors**

- open a node into a pane
- split a pane
- close a pane
- focus handoff on spawn
- focus handoff on close
- first-render activation

**Primary owner**

- Graphshell workbench/pane controller

**Framework role**

- tile rects, tab layout, dock geometry

**Key failure to prevent**

- blank-on-first-open or ambiguous focus ownership

This domain also includes:

- workbench-local recovery
- structural history
- tile and frame identity semantics

**Related spec**

- `workbench/workbench_frame_tile_interaction_spec.md`

### 6.3 Content Opening and Routing

**Purpose**: All content-originating actions must route through Graphshell semantics, not legacy shortcuts.

**Core behaviors**

- open current selection into pane/tab/split
- open from web content link/context actions
- preserve node/tile creation invariants

**Primary owner**

- Graphshell routing + lifecycle authority

**Framework role**

- event source only

**Key failure to prevent**

- content paths bypassing graph/node/tile semantics

**Execution update (2026-03-01)**

- `#175` implemented content-originating child-webview open routing through Graphshell semantics (`OpenNodeFrameRouted`) without reducer-side direct selection shortcuts.
- Diagnostics receipt: `design_docs/archive_docs/checkpoint_2026-03-01/2026-03-01_issue_175_content_open_routing_receipt.md`

This remains a cross-cutting concern shared primarily by the graph and workbench specs.

### 6.4 Command Surfaces

**Purpose**: Commands should be semantically unified even if invoked from different surfaces.

**Core behaviors**

- keyboard commands
- palette commands
- context commands
- radial commands
- omnibar-initiated commands

**Primary owner**

- Graphshell command dispatcher / action registry boundary

**Framework role**

- command surface rendering and event capture

**Key failure to prevent**

- multiple command entry points with divergent semantics

This domain also includes:

- omnibar command invocation
- command-oriented retrieval and result execution

**Related spec**

- `aspect_command/command_surface_interaction_spec.md`

### 6.5 Viewer Presentation and Fallback

**Purpose**: Users should understand what kind of content they are seeing and why it appears that way.

**Core behaviors**

- render content in the correct pane form
- display overlays/affordances appropriately by render mode
- show placeholders and degraded states explicitly

**Primary owner**

- Graphshell viewer routing and compositor policy

**Framework role**

- paint only

**Key failure to prevent**

- hidden fallback ambiguity

This domain also includes:

- overlay and affordance policy
- fallback and degradation explanation
- operational vs partial vs deferred viewer clarity

**Related spec**

- `viewer/viewer_presentation_and_fallback_spec.md`

### 6.6 Settings and Control Surfaces

**Purpose**: Settings/control UI must be navigable and authoritative without floating compatibility debt.

**Core behaviors**

- open settings/history pages
- apply preference changes
- preserve focus and return paths

**Primary owner**

- Graphshell settings/control-surface controller

**Framework role**

- render pages, forms, lists

**Key failure to prevent**

- settings routes split across multiple competing ownership paths

This domain also includes:

- history manager surfaces
- diagnostics pages
- import and persistence control pages

**Related spec**

- `aspect_control/settings_and_control_surfaces_spec.md`

### 6.7 Focus and Region Navigation

**Purpose**: The app must remain navigable beyond pointer-first interaction.

**Core behaviors**

- region cycling
- focus return path
- alternative access to major app functions
- a11y semantics for non-visual workflows where applicable

**Primary owner**

- Graphshell accessibility controller

**Framework role**

- accesskit tree emission and widget semantics where available

**Key failure to prevent**

- region dead-ends or inaccessible critical flows

**Related spec**

- `subsystem_focus/focus_and_region_navigation_spec.md`

---

## 7. How To Turn UX Contracts Into Issues

Every UX issue should be a **contract slice**, not a vague improvement ticket.

Required fields for a UX issue:

1. **Outcome**
   - one user-visible sentence
2. **Contract**
   - trigger
   - preconditions
   - semantic result
   - focus result
   - visual result
   - degradation result
3. **Authority**
   - Graphshell owner
   - framework role
4. **Hotspots**
   - likely files/modules
5. **Non-goals**
   - what this issue must not absorb
6. **Done Gate**
   - tests
   - diagnostics
   - doc parity

This is the only issue shape that reliably respects the architecture.

---

## 8. Planning Workflow

Use this workflow for UX development:

1. Pick a user journey from a UX domain.
2. Write the interaction contract.
3. Identify the Graphshell owner.
4. Write a contract-slice issue.
5. Implement with tests and diagnostics.
6. Update the domain status in this register or the associated issue map.

This keeps UX planning cumulative instead of anecdotal.

---

## 9. Current Status and Next Planning Artifacts

### 9.1 Contract completeness status

| Spec | Standards block | Sub-domain contracts | Per-domain settings ref |
| --- | --- | --- | --- |
| `canvas/graph_node_edge_interaction_spec.md` | ✅ | ✅ complete | ✅ §5.5 |
| `workbench/workbench_frame_tile_interaction_spec.md` | ✅ | ✅ complete | ✅ §5.4 |
| `aspect_command/command_surface_interaction_spec.md` | ✅ | ✅ complete | ✅ §5 |
| `subsystem_focus/focus_and_region_navigation_spec.md` | ✅ | ✅ complete + §4.7 deterministic contract | ✅ §5 |
| `viewer/viewer_presentation_and_fallback_spec.md` | ✅ | ✅ complete | ✅ §5 |
| `aspect_control/settings_and_control_surfaces_spec.md` | ✅ | ✅ complete | ✅ owns settings surface |

### 9.2 Per-domain UX preferences in Settings

Each UX domain maps to a settings category in `settings_and_control_surfaces_spec.md §4.2`.
Settings pages expose per-domain user-configurable behavior:

| UX domain | Settings category | Representative preferences |
| --- | --- | --- |
| Graph / Node / Edge | **Graph** | physics preset (Liquid/Gas/Solid), fit strength, position-fit lock, zoom-fit lock, keyboard pan bindings |
| Workbench / Frame / Tile | **Workspaces** | default routing behavior, tile close policy, frame history depth |
| Command Surfaces | **Keybindings** + **General** | palette mode default (search/context/radial), radial sector presets, command aliases |
| Focus & Region Navigation | **Accessibility** | region cycle order, focus memory, skip-link visibility |
| Viewer Presentation | **General** | default viewer overrides, placeholder explanation verbosity, prewarm policy |
| Settings & Control Surfaces | *(self)* | — |

Per-domain preferences are Planned Extensions in each spec. The Settings surface is the canonical UI for all of them.

### 9.3 Issue domain map

Issues are categorized by UX domain via GitHub labels. Use these labels to filter the live issue map:

| UX domain | GitHub label | Lane |
| --- | --- | --- |
| Graph / Node / Edge | `domain:graph` | `lane:stabilization` |
| Workbench / Frame / Tile | `domain:workbench` | `lane:stabilization` |
| Command Surfaces | `domain:command` | `lane:control-ui-settings` |
| Focus & Region Navigation | `domain:focus` | `lane:accessibility` |
| Viewer Presentation | `domain:viewer` | `lane:viewer-platform` |
| Settings & Control Surfaces | `domain:settings` | `lane:control-ui-settings` |

Contract-slice issues use `.github/ISSUE_TEMPLATE/ux_contract_slice.md`.

### 9.4 Remaining work

1. WCAG 2.2 AA conformance checklist per domain — gated on accessibility subsystem work (`design_docs/graphshell_docs/design/accessibility_baseline_checklist.md` has the structure; per-surface pass/fail status pending implementation).

### 9.5 Per-control audit artifact

Control-level coverage is tracked in:

- `2026-03-04_per_control_audit_grid.md`

This artifact is the canonical surface/region/object/trigger audit grid with implementation status (`Implemented` / `Partial` / `Missing` / `Nonstandard`) bounded to adopted standards.
