<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# GraphShell Keybindings

## Graph View

Default keybinds are configurable in `Settings -> Input`.

- `Home`: Toggle Graph / Detail view
- `Ctrl+Shift+O`: Toggle Overview Plane
- `N`: Create new node
- `Delete`: Remove selected node(s)
- `Ctrl+Shift+Delete`: Clear graph
- `T`: Toggle physics simulation
- `R`: Reheat physics simulation from current layout state
- `P`: Toggle physics settings panel
- `+` / `-` / `0`: Zoom in / out / reset
- `C`: Toggle position-fit lock for active graph pane
- `Z`: Toggle zoom-fit lock for active graph pane
- `F9`: Open Camera Controls in Settings > Physics
- `W` / `A` / `S` / `D` or `Arrow Keys`: Pan graph camera (when unlocked; key mode configurable in Settings > Physics)
- `Ctrl+Click`: Multi-select nodes
- `Right+Drag` (default): Lasso select (replace)
- `Right+Shift+Drag` (default): Lasso add to selection
- `Right+Ctrl+Drag` (default): Lasso add to selection
- `Right+Alt+Drag` (default): Lasso toggle selection
- `L`: Toggle pin on primary selected node
- `I` / `U`: Pin / Unpin selected node(s)
- `G`: Connect selected pair (source -> target)
- `Shift+G`: Connect both directions for selected pair
- `Alt+G`: Remove user edge for selected pair
- `F2` (default): Toggle command palette
- `F3` (default): Toggle radial palette mode
- `Ctrl+F`: Graph search
- `Ctrl+Z` / `Ctrl+Y`: Undo / Redo (workbench-structure scope)
- `Ctrl+Z` (hold): show undo preview indicator; release `Z` while holding `Ctrl` commits one undo step; releasing chord without commit path cancels preview

## Global Cancel / Back

- `Escape`: Dismiss or back out of the innermost transient surface (for example modal dialogs, command palette, graph search, tag panel, Overview Plane, or embedded-content focus reclaim). It is not a graph/detail view toggle.

## History Scope Semantics

- `Back` / `Forward`: traversal-driven navigation within the active tile context
- `Undo` / `Redo`: workbench-structure edits (tile/frame/split/reorder/open/close)
- `F1` / `?` (default): Toggle keyboard shortcut help

## Node Context Menu

- `Copy URL`: Copy source/selected node URL to clipboard
- `Copy Title`: Copy source/selected node title to clipboard

## Graph Search

- `ArrowUp` / `ArrowDown`: Cycle matches
- `Enter`: Select active match
- `Escape`: Clear query, then close when empty
