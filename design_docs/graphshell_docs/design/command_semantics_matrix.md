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
| `GraphFit` | Fit Graph to Screen | Graph | per-View | None | `GraphIntent::RequestFitToScreen` | No | Keyboard, Palette, Radial | N/A | `Z` |
| `GraphTogglePhysics` | Toggle Physics Simulation | Graph | per-View | None | `GraphIntent::TogglePhysics` | Yes | Keyboard, Palette, Radial | N/A | `T` |
| `GraphPhysicsConfig` | Open Physics Settings | Settings surface | per-Workbench | None | `GraphIntent::OpenSettingsUrl { graphshell://settings/physics }` | No | Keyboard, Palette, Radial, Toolbar | N/A | `P` |
| `GraphCommandPalette` | Open Command Palette | Command surface | Global | None | `GraphIntent::ToggleCommandPalette` | No | Keyboard, Palette, Radial, Toolbar | N/A | `F2` |
| `PersistUndo` | Undo | Graph/Workbench | per-Workbench | Undo stack available | `GraphIntent::Undo` | No | Keyboard, Palette, Radial | Visible + disabled when stack empty | `Ctrl+Z` |
| `PersistRedo` | Redo | Graph/Workbench | per-Workbench | Redo stack available | `GraphIntent::Redo` | No | Keyboard, Palette, Radial | Visible + disabled when stack empty | `Ctrl+Y` |
| `PersistSaveSnapshot` | Save Frame Snapshot | Frame | per-Workbench | Frame state available | Requests frame snapshot save | No | Palette, Radial | Visible + disabled when no frame state | None |
| `PersistRestoreSession` | Restore Session Frame | Frame | per-Workbench | Session snapshot exists | Requests restore of session workspace layout | No | Palette, Radial | Visible + disabled when snapshot unavailable | None |
| `PersistSaveGraph` | Save Graph Snapshot | Graph | per-Workbench | Graph state available | Requests timestamped graph snapshot save | No | Palette, Radial | Visible + disabled when graph unavailable | None |
| `PersistRestoreLatestGraph` | Restore Latest Graph | Graph | per-Workbench | At least one graph snapshot exists | Requests latest graph snapshot restore | No | Palette, Radial | Visible + disabled when snapshot unavailable | None |
| `PersistOpenHub` | Open Persistence Hub | Tool Pane | per-Workbench | None | `GraphIntent::OpenToolPane { Settings }` | No | Palette, Radial, Toolbar | N/A | None |

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
- **Frame**: mutates frame routing or frame-scoped arrangement context.
- **Workbench**: mutates workbench/session-level persistence/control context.

Action-class audit summary:

| Scope class | Action IDs |
|---|---|
| Node | `NodePinToggle`, `NodePinSelected`, `NodeUnpinSelected`, `NodeDelete`, `NodeCopyUrl`, `NodeCopyTitle` |
| Tile | `NodeOpenSplit`, `NodeDetachToSplit`, `NodeMoveToActivePane`, `NodeNewAsTab` |
| Graph | `NodeNew`, `EdgeConnectPair`, `EdgeConnectBoth`, `EdgeRemoveUser`, `GraphFit`, `GraphTogglePhysics`, `GraphPhysicsConfig` |
| Frame | `NodeChooseFrame`, `NodeAddToFrame`, `NodeAddConnectedToFrame`, `NodeOpenFrame`, `NodeOpenNeighbors`, `NodeOpenConnected`, `PersistSaveSnapshot`, `PersistRestoreSession` |
| Workbench | `GraphCommandPalette`, `PersistUndo`, `PersistRedo`, `PersistSaveGraph`, `PersistRestoreLatestGraph`, `PersistOpenHub` |

Ambiguous-label audit (priority):

1. **Delete** includes target noun (`Delete Selected Node(s)`), never verb-only.
2. **Open** actions include destination semantics (`Open Node in Split`, `Open via Frame Route`).
3. **Move** actions include moved object (`Move Node to Active Pane`).
4. **Close** is reserved for tile/frame/window surfaces; **Delete** is reserved for graph-content mutation.

---

## 6. Maintenance rule

Any addition/removal/change to `ActionId` or `execute_action(...)` requires updating this matrix in the same PR.