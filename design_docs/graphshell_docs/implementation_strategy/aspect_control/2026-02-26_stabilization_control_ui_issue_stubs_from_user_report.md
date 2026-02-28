# 2026-02-26 Stabilization + Control UI Issue Stubs (User Field Report Intake)

Purpose: convert the current user repro report into issue-ready stubs and lane-aligned slices without losing context.

Source: in-session user field report (Windows 11 primary runtime, plus WSL/WSLg environment observations).

## Triage Summary (by lane)

| Lane | Priority | Why |
| --- | --- | --- |
| `lane:stabilization` (`#88`) | Immediate / blocking | Graph canvas pan/zoom/fit controls still fail globally; tab/pane focus/render activation is inconsistent. |
| `lane:control-ui-settings` (`#89`) | Immediate planning / next execution | Command palette semantics/context behavior and theme toggle are user-facing IA gaps with clear direction. |
| `lane:embedder-debt` (`#90`) | Parallel root-cause lane | Legacy context menu/new-tab paths and render/focus activation races smell like servoshell inheritance paths. |
| `lane:layout-semantics` | Design follow-on (queue now) | Tile/pane/workspace/workbench semantic distinctions and overview UX need an explicit model. |
| `lane:viewer-platform` (`#92`) / `lane:spec-code-parity` (`#99`) | Secondary | Legacy webview context menu behavior does not match Graphshell command/pane semantics. |

## Recommended Issue Stack (Issue-Ready Stubs)

### 1. `lane:stabilization` child: Graph canvas camera controls fail globally (pan/zoom/fit)

Title:
- `Stabilization: graph canvas pan/wheel zoom/zoom commands no-op across contexts`

Summary:
- Graph canvas interaction is partially alive (`select node`, `lasso`), but camera/navigation controls fail broadly:
- `pan drag`, `wheel zoom`, `zoom in/out/reset`, and `zoom to fit` fail anywhere the user can currently interact with the graph.

Repro (current report):
- Launch app on Windows 11.
- Open default graph pane.
- Try drag-pan, mouse wheel zoom, command/shortcut zoom actions, and zoom-to-fit.
- Result: node creation and some shortcuts work, but graph camera controls do not.

Likely hotspots:
- `render/mod.rs`
- `app.rs`
- `input/mod.rs`
- `shell/desktop/ui/gui.rs`
- graph metadata/input ownership paths (`egui_graphs` integration)

Notes / hypotheses:
- Recent fixes addressed targeted camera command ownership and lasso metadata keying.
- Remaining failure likely sits in input gating/consumption, graph metadata availability, or a stuck UI-state gate (radial/palette/focus ownership).

Non-goals:
- Lasso UX improvements (track separately)
- Command palette feature redesign

Done gate:
- Drag-pan, wheel zoom, zoom in/out/reset, and zoom-to-fit all work in the default graph pane.
- Behavior remains correct after opening/closing node panes and after focus changes.
- Add targeted regression tests and diagnostics receipt (camera command application + input path visibility).

### 2. `lane:stabilization` child: Tab/pane focus activation race causes blank viewport until extra clicks

Title:
- `Stabilization: new tab/pane focus activation race leaves viewport blank until follow-up focus changes`

Summary:
- Newly opened tabs/panes sometimes do not render content immediately.
- Behavior varies with focus state: sometimes content appears after click/switch, sometimes not on first click.

Repro (current report):
- Open tabs from node/link flows.
- Observe that some tabs spawn unfocused or visually blank.
- Switching tabs or repeated clicks can cause render to appear later.
- Deleting another pane can leave the graph pane unfocused with an empty-looking workspace.

Likely hotspots:
- `shell/desktop/ui/gui.rs`
- `shell/desktop/ui/gui_frame.rs`
- `shell/desktop/workbench/*` focus/pane activation paths
- `shell/desktop/lifecycle/webview_controller.rs`
- `shell/desktop/workbench/tile_runtime.rs`

Architectural context:
- This looks like focus ownership + render activation ordering debt, not just a local paint bug.
- Likely overlaps with servoshell-derived host/frame assumptions (`lane:embedder-debt`).

Done gate:
- New tabs/panes consistently render on first spawn when intended to be focused.
- Focus transitions after pane deletion promote a sensible next pane (graph pane included) and render immediately.
- Repro captured in scenario test or diagnostics receipt.

### 3. `lane:stabilization` child: Selection/deselect consistency and click-away behavior

Title:
- `Stabilization: graph selection deselect-on-background-click is inconsistent`

Summary:
- Node selection works, but deselecting by clicking away feels inconsistent/funky and may hide deeper selection-state logic bugs.

Scope:
- Audit selection state transitions for:
- background click deselect
- multi-select interactions
- lasso + click-away transitions
- focus changes between graph pane and node pane

Likely hotspots:
- `render/mod.rs`
- `input/mod.rs`
- selection state in `app.rs`

Done gate:
- Background click deselect behavior is deterministic and documented.
- Selection state transitions are covered by targeted tests for single-select, multi-select, and lasso-to-click sequences.

### 4. `lane:stabilization` child: Lasso edge hit-testing misses boundary nodes

Title:
- `Stabilization: lasso boundary hit-testing misses nodes at selection edge`

Summary:
- Lasso works and is visually strong, but it sometimes misses nodes near the edge of the lasso box.
- User expectation: node center inside lasso should count as a hit, even if visual radius crosses boundary.

Likely hotspots:
- `render/mod.rs`
- `render/spatial_index.rs`
- graph node visual bounds / hit proxy calculations

Done gate:
- Define and document lasso inclusion semantics (center-point inclusive minimum).
- Boundary/edge-node regressions covered by tests.

### 5. `lane:control-ui-settings` child: Lasso live capture preview (highlight while dragging)

Title:
- `Control UI: lasso drag preview highlights nodes entering/leaving capture set in real time`

Summary:
- Add live visual feedback during lasso drag so nodes highlight when captured and unhighlight when released before mouse-up.

Why now:
- This improves confidence and makes lasso hit-testing problems more visible.
- It is a UX improvement, not a blocker for lasso correctness.

Likely hotspots:
- `render/mod.rs`
- graph styling/selection preview state

Done gate:
- During lasso drag, preview highlight updates continuously and matches final selection result.
- Reduced-motion / visual clarity considered.

### 6. `lane:control-ui-settings` child: Command palette/context menu unification across UI contexts

Title:
- `Control UI: unify F2 command palette and right-click context command surface across canvas/panes/workbench`

Summary:
- Current command surfaces are fragmented:
- F2 summons a menu labeled `Edge Commands`
- right-click often shows a short legacy context menu
- context availability varies by hover target (too node-biased)

Desired direction (from report):
- F2: larger global command palette
- Right-click: compact contextual command palette
- Both backed by the same action/registry-driven command surface model

Likely hotspots:
- `render/command_palette.rs`
- `render/mod.rs`
- `input/mod.rs`
- action registry / runtime registries integration
- webview context menu bridge paths

Done gate:
- One command surface model supports both global and contextual invocation variants.
- Labeling no longer says `Edge Commands` unless truly edge-specific.
- Palette can be invoked from canvas, nodes, edges, panes, and workbench/workspace chrome (with context-appropriate command enablement).

### 7. `lane:control-ui-settings` child: Contextual command categories and disabled-state policy

Title:
- `Control UI: contextual command palette categories (node/edge/tile/pane/workbench/canvas) with disabled-state policy`

Summary:
- Formalize the contextual command palette information architecture:
- categories represent actionable entities in the current UI context
- unavailable commands/categories are shown disabled or deprioritized instead of disappearing unpredictably

Architectural value:
- Serves as a concrete manifestation of action/layout/mod registries in UI.
- Reduces context-menu inconsistency and discoverability debt.

Done gate:
- Category model documented and implemented for core contexts.
- Disabled/hidden policy is consistent and testable.

### 8. `lane:control-ui-settings` child: Theme mode toggle (dark / light / system)

Title:
- `Control UI / Settings: add theme mode toggle (dark, light, system)`

Summary:
- Add a user-facing settings toggle for theme mode selection:
- `System`, `Light`, `Dark`

Likely hotspots:
- settings UI surface
- theme registry / theme application path
- persistence/preferences

Done gate:
- Theme mode can be changed in settings and persists across restart.
- `System` mode follows OS preference on supported platforms.

### 9. `lane:control-ui-settings` child: Radial menu spacing/readability pass

Title:
- `Control UI: radial menu option spacing/readability pass`

Summary:
- Radial menu works but options are visually crowded.
- Needs spacing/legibility polish before it becomes a primary interaction surface.

Done gate:
- Option spacing and hit targets are improved on desktop DPI ranges.
- No regression to command dispatch behavior.

### 10. `lane:control-ui-settings` child: Omnibar node-search result cycling retains input focus after Enter

Title:
- `Control UI: omnibar node-search Enter action retains focus for result iteration`

Summary:
- `Ctrl+F` search is useful but loses omnibar text focus after Enter when iterating node search results, forcing repeated re-clicks.

Likely hotspots:
- `shell/desktop/ui/toolbar/toolbar_ui/toolbar_omnibar.rs`
- toolbar focus routing

Done gate:
- Enter-based result iteration keeps focus in omnibar when in search mode (unless explicitly committing navigation).
- Behavior is documented/tested for node-search vs navigation submit modes.

### 11. `lane:control-ui-settings` + `lane:layout-semantics` design slice: Scoped undo/redo model

Title:
- `Design slice: scoped undo/redo model (pane history vs graph/workbench action history)`

Summary:
- User reports current undo/redo behavior as interesting but buggy and semantically ambiguous.
- Need a design decision on whether to separate:
- pane-local back/forward navigation
- graph/workbench edit undo/redo
- potentially workspace layout history

Deliverable:
- Decision note defining history scopes, UI affordances, and command bindings.

Done gate:
- Chosen model documented with examples and migration path from current behavior.
- Follow-on implementation issues seeded.

### 12. `lane:embedder-debt` child: Legacy web content new-tab/context-menu path bypasses Graphshell node creation

Title:
- `Embedder debt: web content new-tab/context-menu path bypasses Graphshell node creation and pane semantics`

Summary:
- Right-click/ctrl-clicking links in webpage content (e.g., `servo.org`) can open a tile/tab path that appears to bypass node creation and may produce panes without a mapped node.
- A short legacy context menu still appears instead of the Graphshell command surface.

Why this matters:
- Breaks the graph-as-authoritative model.
- Makes split/workspace testing harder because behavior depends on legacy webview paths.

Likely hotspots:
- `shell/desktop/ui/gui.rs`
- `shell/desktop/host/*`
- `shell/desktop/lifecycle/webview_controller.rs`
- `shell/desktop/workbench/tile_runtime.rs`
- webview context menu / open-new-view handlers

Done gate:
- Link open actions from web content route through Graphshell node/pane creation semantics.
- Legacy fallback context menu path is either removed, bridged into command palette, or explicitly constrained/deferred.

### 13. `lane:embedder-debt` (environment tracker) / platform note: WSLg window shadow artifact

Title:
- `Embedder/platform tracker: WSLg window drop shadow remains at initial viewport bounds after resize/fullscreen`

Summary:
- In VS Code WSL session (`cargo run` via bash), app window shadow/decorations remain stuck to original launch bounds and position.
- Repro persists across window operations and fullscreen/maximize.
- Does not repro in native Windows run.

Notes:
- Likely WSLg/Xwayland/Wayland window-decoration/compositor interaction, not core Graphshell logic.
- Track separately so it does not block cross-platform app logic stabilization.

Done gate:
- Repro is documented with environment details.
- Marked as upstream/toolkit/platform issue or mitigated if app-side knob exists.

## Derived Insights / Feature Plan Seeds

### A. Command palette should be the UI manifestation of registries

The report gives a concrete product requirement that aligns with current architecture:
- action/mod/layout registries should become the source of command availability and grouping
- command palette is not just a search box; it is a contextual command surface

Action:
- Fold this into `lane:control-ui-settings` issue scopes and `2026-02-24_control_ui_ux_plan.md`.

### B. Two command-surface modalities are valid (not one)

The report describes a good split:
- `F2`: larger global command palette
- right-click: compact contextual variant near pointer

This reduces the false choice between “palette” and “context menu” and lets both share one backend model.

### C. Workbench/workspace/tile/pane semantics are a real UX architecture gap

Questions raised in the report are not polish questions; they are model questions:
- what is a new tile vs a new workspace
- how does a user get back to the root workbench overview
- how should workspaces map to the tile tree and graph

Action:
- Track as `lane:layout-semantics` design/UX slice (with `lane:control-ui-settings` overlap for command surfacing).

### D. Undo/redo likely needs explicit scope separation

The report points toward a multi-scope model:
- pane-local navigation history (`Back`/`Forward`)
- graph/workbench structural edits (`Undo`/`Redo`)

Action:
- Do a design slice first before more bugfix patching in this area.

### E. Testability is being blocked by interaction/focus debt traps

The report explicitly notes difficulty testing split-view behavior because focus and legacy paths interfere.

Action:
- Add stabilization diagnostics/receipts for focus ownership and pane activation to reduce manual repro ambiguity.

## Suggested Sequencing (Next 5 Child Issues)

1. `lane:stabilization`: graph canvas camera controls fail globally
2. `lane:stabilization`: tab/pane focus activation race (blank viewport)
3. `lane:embedder-debt`: web content new-tab/context-menu bypasses node creation
4. `lane:control-ui-settings`: command palette/context menu unification
5. `lane:control-ui-settings`: theme mode toggle + omnibar focus retention (can be split)

## Notes for GitHub Intake

- Prefer creating these as child issues under `#88`, `#89`, and `#90`.
- Cross-link the lasso preview issue to the lasso hit-testing stabilization issue so UX work does not hide correctness regressions.
- Keep the WSLg shadow artifact as a platform tracker, not a core stabilization blocker.
