# Command Semantics Matrix

**Date**: 2026-03-02  
**Status**: Canonical deliverable (D1)  
**Purpose**: Single semantic matrix for `ActionRegistry` commands across command surfaces.

**Related**:
- `../implementation_strategy/aspect_command/command_surface_interaction_spec.md`
- `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md`
- `../research/2026-03-02_ux_integration_research.md`
- `../../../KEYBINDINGS.md`

---

## 1. Scope and authority

This matrix is the canonical command-semantics artifact for deliverable **D1**.

- Semantic command authority is `ActionRegistry` (`render/action_registry.rs`).
- Dispatch authority is shared `execute_action(...)` (`render/command_palette.rs`) used by command palette and radial menu.
- Keyboard exposure uses `KEYBINDINGS.md` and must preserve ActionRegistry semantics.
- Toolbar and Omnibar columns are included for parity; when no ActionRegistry route exists they are marked `N/A`.

---

## 2. Disabled-state policy

- `Visible + disabled`: action appears but is non-invokable in current context.
- `Greyed + reason`: action appears disabled with explicit contextual reason.
- `N/A`: surface does not expose that action family.

Canonical requirement: command surfaces may differ in presentation, but not in action meaning.

---

## 3. Matrix

| Action ID | Verb | Target | Scope | Preconditions | Side effects | Undoable | Surfaces | Disabled behavior | Shortcut |
|---|---|---|---|---|---|---|---|---|---|
| `NodeNew` | Create Node | Graph | per-Workbench | None | `GraphIntent::CreateNodeNearCenter` | Yes | Keyboard, Palette, Radial | N/A | `N` |
| `NodeNewAsTab` | Create Node as Tab | Tile | per-Frame | None | `GraphIntent::CreateNodeNearCenterAndOpen { Tab }` | Yes | Palette | Visible + disabled (surface unavailable) | None |
| `NodePinToggle` | Toggle Pin | Node | per-View | At least one selected or target node | Emits pin/unpin edge command based on selected-node pin state | Yes | Keyboard, Radial | Greyed + reason when no node context | `L` |
| `NodePinSelected` | Pin Selected | Node | per-View | At least one selected or target node | `GraphIntent::ExecuteEdgeCommand { PinSelected }` | Yes | Keyboard, Palette | Greyed + reason when no node context | `I` |
| `NodeUnpinSelected` | Unpin Selected | Node | per-View | At least one selected or target node | `GraphIntent::ExecuteEdgeCommand { UnpinSelected }` | Yes | Keyboard, Palette | Greyed + reason when no node context | `U` |
| `NodeDelete` | Delete Selected Node(s) | Node | per-View | At least one selected or target node | `GraphIntent::RemoveSelectedNodes` | Soft | Keyboard, Palette, Radial | Greyed + reason when no node context | `Delete` |
| `NodeChooseFrame` | Choose Frame | Frame | per-Workbench | Node context with one or more candidate frames | Opens frame chooser UI for target node | No | Palette, Radial | Greyed + reason when no node context | None |
| `NodeAddToFrame` | Add To Frame | Frame | per-Workbench | At least one selected or target node | Opens add-to-frame picker for target node | No | Palette, Radial | Greyed + reason when no node context | None |
| `NodeAddConnectedToFrame` | Add Connected To Frame | Frame | per-Workbench | At least one selected or target node | Opens add-connected-to-frame picker | No | Palette, Radial | Greyed + reason when no node context | None |
| `NodeOpenFrame` | Open via Frame Route | Node/Frame | per-Workbench | At least one selected or target node | `GraphIntent::OpenNodeFrameRouted` | Yes | Palette, Radial | Greyed + reason when no node context | None |
| `NodeOpenNeighbors` | Open with Neighbors | Node/Frame | per-Workbench | At least one selected or target node | Opens connected scope (`Neighbors`) in tab mode | Yes | Palette, Radial | Greyed + reason when no node context | None |
| `NodeOpenConnected` | Open with Connected | Node/Frame | per-Workbench | At least one selected or target node | Opens connected scope (`Connected`) in tab mode | Yes | Palette, Radial | Greyed + reason when no node context | None |
| `NodeOpenSplit` | Open Node in Split | Tile | per-Frame | At least one selected or target node | Requests node open in split-horizontal mode | Yes | Palette, Radial | Greyed + reason when no node context | None |
| `NodeDetachToSplit` | Detach Focused to Split | Tile | per-Frame | Focused node pane available | Requests focused pane detach into split | Yes | Palette | Greyed + reason when no focused pane | None |
| `NodeMoveToActivePane` | Move Node to Active Pane | Node/Tile | per-Frame | At least one selected or target node | Routes node into active pane context | Yes | Palette, Radial | Greyed + reason when no node context | None |
| `NodeCopyUrl` | Copy Node URL | Node | per-View | At least one selected or target node | Copies node URL to clipboard | No | Node context menu, Palette, Radial | Greyed + reason when no node context | None |
| `NodeCopyTitle` | Copy Node Title | Node | per-View | At least one selected or target node | Copies node title to clipboard | No | Node context menu, Palette, Radial | Greyed + reason when no node context | None |
| `EdgeConnectPair` | Connect Source -> Target | Edge | per-View | Pair context available | `GraphIntent::ExecuteEdgeCommand { ConnectPair }` | Yes | Keyboard, Palette, Radial | Greyed + reason when selected pair unavailable | `G` |
| `EdgeConnectBoth` | Connect Both Directions | Edge | per-View | Pair context available | `GraphIntent::ExecuteEdgeCommand { ConnectBothDirectionsPair }` | Yes | Keyboard, Palette, Radial | Greyed + reason when selected pair unavailable | `Shift+G` |
| `EdgeRemoveUser` | Remove User Edge | Edge | per-View | Pair context available | `GraphIntent::ExecuteEdgeCommand { RemoveUserEdgePair }` | Yes | Keyboard, Palette, Radial | Greyed + reason when selected pair unavailable | `Alt+G` |
| `GraphFit` | Fit Graph to Screen | Graph | per-View | None | `GraphIntent::RequestFitToScreen` | No | Palette, Radial | N/A | None |
| `GraphTogglePhysics` | Toggle Physics Simulation | Graph | per-View | None | `GraphIntent::TogglePhysics` | Yes | Keyboard, Palette, Radial | N/A | `T` |
| `GraphReheatPhysics` | Reheat Physics Simulation | Graph | per-View | None | `GraphIntent::ReheatPhysics` | No | Keyboard, Palette, Radial | N/A | `R` |
| `GraphToggleDetailView` | Toggle Graph / Detail View | View Surface | per-Frame | Focused view surface available | Toggles graph/detail projection for focused surface | No | Keyboard, Palette, Radial | Visible + disabled when no focusable view surface | `Home` |
| `GraphToggleOverviewPlane` | Toggle Overview Plane | Graph View Layout | per-Graph | Graph scope available | `GraphIntent::ToggleGraphViewLayoutManager` | No | Keyboard, Palette, Toolbar, Navigator sidebar | Visible + disabled when graph scope unavailable | `Ctrl+Shift+O` |
| `GraphSearchOpen` | Open Graph Search | Search surface | per-Frame | Graph view surface available | Opens graph search UI for current graph surface | No | Keyboard, Palette, Toolbar | Visible + disabled when graph search surface unavailable | `Ctrl+F` |
| `GraphTogglePositionFitLock` | Toggle Position-Fit Lock | Graph Camera | per-View | None | `GraphIntent::ToggleCameraPositionFitLock` | No | Keyboard, Palette | N/A | `C` |
| `GraphToggleZoomFitLock` | Toggle Zoom-Fit Lock | Graph Camera | per-View | None | `GraphIntent::ToggleCameraZoomFitLock` | No | Keyboard, Palette | N/A | `Z` |
| `GraphZoomIn` | Zoom In | Graph Camera | per-View | Graph view focus or resolvable single graph view | `GraphIntent::RequestZoomIn` | No | Keyboard, Palette | Visible + disabled when no graph view target | `+` |
| `GraphZoomOut` | Zoom Out | Graph Camera | per-View | Graph view focus or resolvable single graph view | `GraphIntent::RequestZoomOut` | No | Keyboard, Palette | Visible + disabled when no graph view target | `-` |
| `GraphZoomReset` | Reset Zoom | Graph Camera | per-View | Graph view focus or resolvable single graph view | `GraphIntent::RequestZoomReset` | No | Keyboard, Palette | Visible + disabled when no graph view target | `0` |
| `GraphNavigateBack` | Navigate Back | Node History | per-View | Focused input or active node/browser target available | `GraphIntent::TraverseBack` -> browser `Back` routing | No | Keyboard, Palette, Toolbar, Gamepad | Visible + disabled when no navigation target | `Back` |
| `GraphNavigateForward` | Navigate Forward | Node History | per-View | Focused input or active node/browser target available | `GraphIntent::TraverseForward` -> browser `Forward` routing | No | Keyboard, Palette, Toolbar, Gamepad | Visible + disabled when no navigation target | `Forward` |
| `GraphPhysicsConfig` | Open Physics Settings | Settings surface | per-Workbench | None | `WorkbenchIntent::OpenSettingsUrl { verso://settings/physics }` | No | Keyboard, Palette, Radial, Toolbar | N/A | `P` |
| `GraphCommandPalette` | Open Command Palette | Command surface | Global | None | `GraphIntent::ToggleCommandPalette` | No | Keyboard, Palette, Radial, Toolbar | N/A | `F2` |
| `WorkbenchHelpOpen` | Toggle Keyboard Shortcut Help | Help surface | per-Workbench | None | `GraphIntent::ToggleHelpPanel` | No | Keyboard, Palette, Toolbar | N/A | `F1` / `?` |
| `WorkbenchRadialPaletteToggle` | Toggle Radial Palette Mode | Command surface | per-Workbench | None | `GraphIntent::ToggleRadialMenu` | No | Keyboard, Palette, Toolbar | N/A | `F3` |
| `PersistUndo` | Undo | Graph/Workbench | per-Workbench | Undo stack available | `GraphIntent::Undo` | No | Keyboard, Palette, Radial | Visible + disabled when stack empty | `Ctrl+Z` |
| `PersistRedo` | Redo | Graph/Workbench | per-Workbench | Redo stack available | `GraphIntent::Redo` | No | Keyboard, Palette, Radial | Visible + disabled when stack empty | `Ctrl+Y` |
| `GraphClear` | Clear Graph | Graph | per-Workbench | Graph contains at least one node | `GraphIntent::ClearGraph` | Soft | Keyboard, Palette | Visible + disabled when graph empty | `Ctrl+Shift+Delete` |
| `PersistSaveSnapshot` | Save Frame Snapshot | Frame | per-Workbench | Frame state available | Requests frame snapshot save | No | Palette, Radial | Visible + disabled when no frame state | None |
| `PersistRestoreSession` | Restore Session Frame | Frame | per-Workbench | Session snapshot exists | Requests restore of session workspace layout | No | Palette, Radial | Visible + disabled when snapshot unavailable | None |
| `PersistSaveGraph` | Save Graph Snapshot | Graph | per-Workbench | Graph state available | Requests timestamped graph snapshot save | No | Palette, Radial | Visible + disabled when graph unavailable | None |
| `PersistRestoreLatestGraph` | Restore Latest Graph | Graph | per-Workbench | At least one graph snapshot exists | Requests latest graph snapshot restore | No | Palette, Radial | Visible + disabled when snapshot unavailable | None |
| `PersistOpenHub` | Open Persistence Hub | Tool Pane | per-Workbench | None | `WorkbenchIntent::OpenToolPane { Settings }` | No | Palette, Radial, Toolbar | N/A | None |
| `PersistOpenHistoryManager` | Open History Manager | Tool Pane | per-Workbench | None | `WorkbenchIntent::OpenToolPane { HistoryManager }` via runtime action dispatch | No | Keyboard, Palette | N/A | `Ctrl+H` |
| `WorkbenchActivateWorkflowDefault` | Activate Default Workflow | Workflow Runtime | per-Workbench | Workflow runtime available | `workbench:activate_workflow { workflow:default }` -> workflow profile activation | No | Palette | N/A | None |
| `WorkbenchActivateWorkflowResearch` | Activate Research Workflow | Workflow Runtime | per-Workbench | Workflow runtime available | `workbench:activate_workflow { workflow:research }` -> workflow profile activation | No | Palette | N/A | None |
| `WorkbenchActivateWorkflowReading` | Activate Reading Workflow | Workflow Runtime | per-Workbench | Workflow runtime available | `workbench:activate_workflow { workflow:reading }` -> workflow profile activation | No | Palette | N/A | None |

---

## 3.1 Undo Implementation Status (W1)

`Undoable` in the matrix remains the semantic target contract. This subsection records runtime implementation status as of 2026-03-05.

Implemented (backed by reducer checkpoint capture + undo/redo regressions):

- `NodeNew`
- `NodeNewAsTab`
- `NodePinToggle`
- `NodePinSelected`
- `NodeUnpinSelected`
- `NodeDelete` (`Soft`)
- `EdgeConnectPair`
- `EdgeConnectBoth`
- `EdgeRemoveUser`

Planned (semantically undoable in matrix, but not yet in W1 runtime coverage):

- `NodeOpenFrame`
- `NodeOpenNeighbors`
- `NodeOpenConnected`
- `NodeOpenSplit`
- `NodeDetachToSplit`
- `NodeMoveToActivePane`
- `GraphTogglePhysics`

Notes:

- `PersistUndo` and `PersistRedo` are command-surface controls, not undoable domain actions themselves.
- W1 runtime coverage currently targets destructive graph mutation semantics first; tile/workbench presentation-state history remains follow-on scope.

---

## 3.2 Context Map (C2.1)

Context-eligible summon targets are:

- `node`
- `edge`
- `canvas_background`
- `pane_header`
- `tool_pane_body`

This table defines which actions are first-class candidates for each contextual summon.
Search Palette Mode may still expose broader fallback actions, but these are the
normative context-owned entries.

| Context | Primary action set |
|---|---|
| `node` | `NodePinToggle`, `NodePinSelected`, `NodeUnpinSelected`, `NodeDelete`, `NodeChooseFrame`, `NodeAddToFrame`, `NodeAddConnectedToFrame`, `NodeOpenFrame`, `NodeOpenNeighbors`, `NodeOpenConnected`, `NodeOpenSplit`, `NodeDetachToSplit`, `NodeMoveToActivePane`, `NodeCopyUrl`, `NodeCopyTitle` |
| `edge` | `EdgeConnectPair`, `EdgeConnectBoth`, `EdgeRemoveUser` |
| `canvas_background` | `NodeNew`, `GraphFit`, `GraphTogglePhysics`, `GraphReheatPhysics`, `GraphTogglePositionFitLock`, `GraphToggleZoomFitLock`, `GraphZoomIn`, `GraphZoomOut`, `GraphZoomReset`, `GraphSearchOpen`, `GraphCommandPalette`, `WorkbenchRadialPaletteToggle`, `PersistUndo`, `PersistRedo`, `GraphClear` |
| `pane_header` | `NodeNewAsTab`, `NodeOpenSplit`, `NodeDetachToSplit`, `NodeMoveToActivePane`, `PersistSaveSnapshot`, `PersistRestoreSession`, `PersistSaveGraph`, `PersistRestoreLatestGraph`, `PersistOpenHub`, `PersistOpenHistoryManager`, `WorkbenchActivateWorkflowDefault`, `WorkbenchActivateWorkflowResearch`, `WorkbenchActivateWorkflowReading` |
| `tool_pane_body` | `GraphCommandPalette`, `WorkbenchHelpOpen`, `WorkbenchRadialPaletteToggle`, `PersistUndo`, `PersistRedo`, `PersistOpenHub`, `PersistOpenHistoryManager`, `WorkbenchActivateWorkflowDefault`, `WorkbenchActivateWorkflowResearch`, `WorkbenchActivateWorkflowReading` |

Context-ordering rule:

- `node` summon => Node actions category first
- `edge` summon => Edge actions category first
- `canvas_background` summon => Graph actions category first
- `pane_header` summon => Tile/Frame actions category first
- `tool_pane_body` summon => Workbench actions category first

---

## 4. Surface parity notes

1. Palette and radial surfaces are semantically unified through shared `ActionId` dispatch.
2. Radial uses a curated subset but preserves action meaning for shared actions.
3. Keyboard shortcuts are semantic aliases of existing actions, not independent behaviors.
4. Omnibar currently does not expose `ActionRegistry` entries as a first-class action list (`N/A` in this matrix).

---

## 5. Object-action scope audit map (`#299`)

Canonical scope classes:

- **Node**: mutates or reads graph-node content identity/state.
- **Tile**: mutates tile-tree presentation arrangement only.
- **Graph**: mutates or controls graph-wide runtime behavior.
- **Graph Camera**: mutates or controls focused graph viewport camera/navigation behavior.
- **View Surface**: mutates or toggles the active graph/detail/search surface state.
- **Frame**: mutates frame routing or frame-scoped arrangement context.
- **Workbench**: mutates workbench/session-level persistence/control context.

Action-class audit summary:

| Scope class | Action IDs |
|---|---|
| Node | `NodePinToggle`, `NodePinSelected`, `NodeUnpinSelected`, `NodeDelete`, `NodeCopyUrl`, `NodeCopyTitle` |
| Tile | `NodeOpenSplit`, `NodeDetachToSplit`, `NodeMoveToActivePane`, `NodeNewAsTab` |
| Graph | `NodeNew`, `EdgeConnectPair`, `EdgeConnectBoth`, `EdgeRemoveUser`, `GraphFit`, `GraphTogglePhysics`, `GraphPhysicsConfig` |
| Frame | `NodeChooseFrame`, `NodeAddToFrame`, `NodeAddConnectedToFrame`, `NodeOpenFrame`, `NodeOpenNeighbors`, `NodeOpenConnected`, `PersistSaveSnapshot`, `PersistRestoreSession` |
| Graph Camera | `GraphReheatPhysics`, `GraphTogglePositionFitLock`, `GraphToggleZoomFitLock`, `GraphZoomIn`, `GraphZoomOut`, `GraphZoomReset`, `GraphNavigateBack`, `GraphNavigateForward` |
| View Surface | `GraphToggleDetailView`, `GraphSearchOpen` |
| Workbench | `GraphCommandPalette`, `WorkbenchHelpOpen`, `WorkbenchRadialPaletteToggle`, `PersistUndo`, `PersistRedo`, `GraphClear`, `PersistSaveGraph`, `PersistRestoreLatestGraph`, `PersistOpenHub`, `PersistOpenHistoryManager`, `WorkbenchActivateWorkflowDefault`, `WorkbenchActivateWorkflowResearch`, `WorkbenchActivateWorkflowReading` |

Ambiguous-label audit (priority):

1. **Delete** includes target noun (`Delete Selected Node(s)`), never verb-only.
2. **Open** actions include destination semantics (`Open Node in Split`, `Open via Frame Route`).
3. **Move** actions include moved object (`Move Node to Active Pane`).
4. **Close** is reserved for tile/frame/window surfaces; **Delete** is reserved for graph-content mutation.

---

## 6. Maintenance rule

Any addition/removal/change to `ActionId` or `execute_action(...)` requires updating this matrix in the same PR.
