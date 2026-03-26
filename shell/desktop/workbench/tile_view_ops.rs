/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::{Container, Tile, TileId, Tree};
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId};
use crate::graph::NodeKey;
use crate::registries::domain::layout::workbench_surface::FocusCycle;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_backpressure::{
    self, WebviewCreationBackpressureState,
};
use crate::shell::desktop::workbench::pane_model::{
    GraphPaneRef, NodePaneState, PaneId, PanePresentationMode, PaneViewState,
};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::workbench::pane_model::{ToolPaneRef, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileOpenMode {
    Tab,
    SplitHorizontal,
    QuarterPane,
    HalfPane,
}

fn tile_matches_node(tile: &TileKind, node_key: NodeKey) -> bool {
    matches!(tile.node_state(), Some(state) if state.node == node_key)
}

fn remove_unattached_tile(tiles_tree: &mut Tree<TileKind>, tile_id: TileId) {
    if tiles_tree.root() == Some(tile_id) {
        tiles_tree.root = None;
    }
    let _ = tiles_tree.tiles.remove(tile_id);
}

fn remove_all_floating_panes(tiles_tree: &mut Tree<TileKind>) {
    let floating_ids: Vec<TileId> = tiles_tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(kind)
                if kind.is_floating()
                    && tiles_tree.root() != Some(*tile_id)
                    && tiles_tree.tiles.parent_of(*tile_id).is_none() =>
            {
                Some(*tile_id)
            }
            _ => None,
        })
        .collect();
    for tile_id in floating_ids {
        remove_unattached_tile(tiles_tree, tile_id);
    }
}

fn insert_floating_node_pane(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
    remove_all_floating_panes(tiles_tree);
    let mut state = NodePaneState::for_node(node_key);
    state.presentation_mode = PanePresentationMode::Floating;
    let _ = tiles_tree
        .tiles
        .insert_pane(TileKind::Pane(PaneViewState::Node(state)));
}

pub(crate) struct ToggleTileViewArgs<'a> {
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) base_rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) lifecycle_intents: &'a mut Vec<GraphIntent>,
}

pub(crate) fn preferred_detail_node(graph_app: &GraphBrowserApp) -> Option<NodeKey> {
    graph_app
        .get_single_selected_node()
        .or_else(|| graph_app.domain_graph().nodes().next().map(|(key, _)| key))
}

pub(crate) fn active_graph_view_id(tiles_tree: &Tree<TileKind>) -> Option<GraphViewId> {
    let mut last_active_graph = None;
    for tile_id in tiles_tree.active_tiles() {
        if let Some(Tile::Pane(TileKind::Graph(view_ref))) = tiles_tree.tiles.get(tile_id) {
            last_active_graph = Some(view_ref.graph_view_id);
        }
    }
    last_active_graph
}

pub(crate) fn ensure_active_tile(tiles_tree: &mut Tree<TileKind>) -> bool {
    let mut has_active_pane = tiles_tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tiles_tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Graph(_)))
                | Some(Tile::Pane(TileKind::Node(_)))
                | Some(Tile::Pane(TileKind::Pane(PaneViewState::Graph(_))))
                | Some(Tile::Pane(TileKind::Pane(PaneViewState::Node(_))))
        )
    });

    #[cfg(feature = "diagnostics")]
    {
        has_active_pane = has_active_pane
            || tiles_tree.active_tiles().into_iter().any(|tile_id| {
                matches!(
                    tiles_tree.tiles.get(tile_id),
                    Some(Tile::Pane(TileKind::Tool(_)))
                        | Some(Tile::Pane(TileKind::Pane(PaneViewState::Tool(_))))
                )
            });
    }

    if has_active_pane {
        return false;
    }

    if tiles_tree.make_active(|_, tile| {
        matches!(
            tile,
            Tile::Pane(TileKind::Graph(_)) | Tile::Pane(TileKind::Pane(PaneViewState::Graph(_)))
        )
    }) {
        return true;
    }

    if tiles_tree.make_active(|_, tile| {
        matches!(
            tile,
            Tile::Pane(TileKind::Node(_)) | Tile::Pane(TileKind::Pane(PaneViewState::Node(_)))
        )
    }) {
        return true;
    }

    #[cfg(feature = "diagnostics")]
    if tiles_tree.make_active(|_, tile| {
        matches!(
            tile,
            Tile::Pane(TileKind::Tool(_)) | Tile::Pane(TileKind::Pane(PaneViewState::Tool(_)))
        )
    }) {
        return true;
    }

    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusCycleRegion {
    Graph,
    Node,
    #[cfg(feature = "diagnostics")]
    Tool,
}

fn active_focus_cycle_region(tiles_tree: &Tree<TileKind>) -> Option<FocusCycleRegion> {
    let mut active = None;
    for tile_id in tiles_tree.active_tiles() {
        match tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Graph(_)))
            | Some(Tile::Pane(TileKind::Pane(PaneViewState::Graph(_)))) => {
                active = Some(FocusCycleRegion::Graph)
            }
            Some(Tile::Pane(TileKind::Node(_)))
            | Some(Tile::Pane(TileKind::Pane(PaneViewState::Node(_)))) => {
                active = Some(FocusCycleRegion::Node)
            }
            #[cfg(feature = "diagnostics")]
            Some(Tile::Pane(TileKind::Tool(_)))
            | Some(Tile::Pane(TileKind::Pane(PaneViewState::Tool(_)))) => {
                active = Some(FocusCycleRegion::Tool)
            }
            _ => {}
        }
    }
    active
}

fn focus_cycle_region_is_present(tiles_tree: &Tree<TileKind>, region: FocusCycleRegion) -> bool {
    tiles_tree
        .tiles
        .iter()
        .any(|(_, tile)| match (region, tile) {
            (FocusCycleRegion::Graph, Tile::Pane(TileKind::Graph(_))) => true,
            (FocusCycleRegion::Graph, Tile::Pane(TileKind::Pane(PaneViewState::Graph(_)))) => true,
            (FocusCycleRegion::Node, Tile::Pane(TileKind::Node(_))) => true,
            (FocusCycleRegion::Node, Tile::Pane(TileKind::Pane(PaneViewState::Node(_)))) => true,
            #[cfg(feature = "diagnostics")]
            (FocusCycleRegion::Tool, Tile::Pane(TileKind::Tool(_))) => true,
            #[cfg(feature = "diagnostics")]
            (FocusCycleRegion::Tool, Tile::Pane(TileKind::Pane(PaneViewState::Tool(_)))) => true,
            _ => false,
        })
}

fn make_focus_cycle_region_active(
    tiles_tree: &mut Tree<TileKind>,
    region: FocusCycleRegion,
) -> bool {
    match region {
        FocusCycleRegion::Graph => tiles_tree.make_active(|_, tile| {
            matches!(
                tile,
                Tile::Pane(TileKind::Graph(_))
                    | Tile::Pane(TileKind::Pane(PaneViewState::Graph(_)))
            )
        }),
        FocusCycleRegion::Node => tiles_tree.make_active(|_, tile| {
            matches!(
                tile,
                Tile::Pane(TileKind::Node(_)) | Tile::Pane(TileKind::Pane(PaneViewState::Node(_)))
            )
        }),
        #[cfg(feature = "diagnostics")]
        FocusCycleRegion::Tool => tiles_tree.make_active(|_, tile| {
            matches!(
                tile,
                Tile::Pane(TileKind::Tool(_)) | Tile::Pane(TileKind::Pane(PaneViewState::Tool(_)))
            )
        }),
    }
}

fn cycle_focus_region_by_kind(tiles_tree: &mut Tree<TileKind>) -> bool {
    let order = [
        FocusCycleRegion::Graph,
        FocusCycleRegion::Node,
        #[cfg(feature = "diagnostics")]
        FocusCycleRegion::Tool,
    ];

    let current_index = active_focus_cycle_region(tiles_tree)
        .and_then(|region| order.iter().position(|candidate| *candidate == region));

    let start_index = current_index.unwrap_or(order.len() - 1);
    for offset in 1..=order.len() {
        let idx = (start_index + offset) % order.len();
        let candidate = order[idx];
        if !focus_cycle_region_is_present(tiles_tree, candidate) {
            continue;
        }
        if make_focus_cycle_region_active(tiles_tree, candidate) {
            return true;
        }
    }

    false
}

fn cycle_active_tab_in_parent(tiles_tree: &mut Tree<TileKind>) -> bool {
    let active_tile_id = tiles_tree.active_tiles().into_iter().last();
    let Some(active_tile_id) = active_tile_id else {
        return false;
    };
    let Some(parent_id) = tiles_tree.tiles.parent_of(active_tile_id) else {
        return false;
    };
    let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(parent_id) else {
        return false;
    };
    let Some(index) = tabs
        .children
        .iter()
        .position(|child| *child == active_tile_id)
    else {
        return false;
    };
    if tabs.children.len() < 2 {
        return false;
    }

    let next_index = (index + 1) % tabs.children.len();
    let next_active = tabs.children[next_index];
    if next_active == active_tile_id {
        return false;
    }
    tabs.set_active(next_active);
    true
}

pub(crate) fn cycle_focus_region_with_policy(
    tiles_tree: &mut Tree<TileKind>,
    focus_cycle: FocusCycle,
) -> bool {
    match focus_cycle {
        FocusCycle::Tabs => cycle_active_tab_in_parent(tiles_tree),
        FocusCycle::Panes => cycle_focus_region_by_kind(tiles_tree),
        FocusCycle::Both => {
            cycle_active_tab_in_parent(tiles_tree) || cycle_focus_region_by_kind(tiles_tree)
        }
    }
}

pub(crate) fn cycle_focus_region(tiles_tree: &mut Tree<TileKind>) -> bool {
    cycle_focus_region_with_policy(tiles_tree, FocusCycle::Both)
}

pub(crate) fn open_or_focus_graph_pane(tiles_tree: &mut Tree<TileKind>, view_id: GraphViewId) {
    open_or_focus_graph_pane_with_mode(tiles_tree, view_id, TileOpenMode::Tab);
}

pub(crate) fn open_or_focus_graph_pane_with_mode(
    tiles_tree: &mut Tree<TileKind>,
    view_id: GraphViewId,
    mode: TileOpenMode,
) {
    log::debug!(
        "tile_view_ops: open_or_focus_graph_pane_with_mode view {:?} mode {:?}",
        view_id,
        mode
    );

    if tiles_tree.make_active(
        |_, tile| {
            matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == view_id)
        },
    ) {
        log::debug!(
            "tile_view_ops: focused existing graph pane for view {:?}",
            view_id
        );
        return;
    }

    let graph_pane_tile_id = tiles_tree
        .tiles
        .insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let split_leaf_tile_id = tiles_tree.tiles.insert_tab_tile(vec![graph_pane_tile_id]);
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(match mode {
            TileOpenMode::Tab => graph_pane_tile_id,
            TileOpenMode::SplitHorizontal => split_leaf_tile_id,
            TileOpenMode::QuarterPane | TileOpenMode::HalfPane => graph_pane_tile_id,
        });
        return;
    };

    match mode {
        TileOpenMode::Tab | TileOpenMode::QuarterPane | TileOpenMode::HalfPane => {
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(graph_pane_tile_id);
                tabs.set_active(graph_pane_tile_id);
                return;
            }

            let tabs_root = tiles_tree
                .tiles
                .insert_tab_tile(vec![root_id, graph_pane_tile_id]);
            tiles_tree.root = Some(tabs_root);
            let _ = tiles_tree.make_active(
                |_, tile| {
                    matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == view_id)
                },
            );
        }
        TileOpenMode::SplitHorizontal => {
            let split_lhs_id = if matches!(tiles_tree.tiles.get(root_id), Some(Tile::Pane(_))) {
                let wrapped = tiles_tree.tiles.insert_tab_tile(vec![root_id]);
                tiles_tree.root = Some(wrapped);
                wrapped
            } else {
                root_id
            };

            if let Some(Tile::Container(Container::Linear(linear))) =
                tiles_tree.tiles.get_mut(split_lhs_id)
            {
                linear.add_child(split_leaf_tile_id);
                let _ = tiles_tree.make_active(
                    |_, tile| {
                        matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == view_id)
                    },
                );
                return;
            }

            let split_root = tiles_tree
                .tiles
                .insert_horizontal_tile(vec![split_lhs_id, split_leaf_tile_id]);
            tiles_tree.root = Some(split_root);
            let _ = tiles_tree.make_active(
                |_, tile| {
                    matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == view_id)
                },
            );
        }
    }
}

/// Return the `TileId` of a `Container::Tabs` that contains at least one
/// warm, non-floating tile for a durable graphlet peer of `node_key`.
///
/// This is the routing oracle for graphlet-aware tile opening: if the caller
/// places a new tile into the returned container, the node joins the correct
/// durable graphlet group without any additional graph mutations.
pub(crate) fn warm_peer_tab_container(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    node_key: NodeKey,
) -> Option<TileId> {
    let peers = graph_app.graphlet_peers_for_view(
        node_key,
        active_graph_view_id(tiles_tree).or(graph_app.workspace.graph_runtime.focused_view),
    );
    for peer in peers {
        let Some(peer_tile_id) = tiles_tree
            .tiles
            .iter()
            .find_map(|(tile_id, tile)| match tile {
                Tile::Pane(kind) if tile_matches_node(kind, peer) && !kind.is_floating() => {
                    Some(*tile_id)
                }
                _ => None,
            })
        else {
            continue;
        };
        if let Some(parent_id) = tiles_tree.tiles.parent_of(peer_tile_id) {
            if matches!(
                tiles_tree.tiles.get(parent_id),
                Some(Tile::Container(Container::Tabs(_)))
            ) {
                return Some(parent_id);
            }
        }
    }
    None
}

/// Open a node pane using graphlet-aware routing.
///
/// 1. If the node already has a non-floating tile, focus it.
/// 2. If any durable graphlet peer has a warm tile in a tab container,
///    add the new tile to that same container.
/// 3. Otherwise fall back to [`open_or_focus_node_pane`].
///
/// This function does **not** create new graph edges; use
/// `handle_open_node_in_pane_intent` when edge creation is needed (Phase 5).
pub(crate) fn open_node_with_graphlet_routing(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) {
    if tiles_tree.make_active(|_, tile| match tile {
        Tile::Pane(kind) => tile_matches_node(kind, node_key) && !kind.is_floating(),
        _ => false,
    }) {
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    if let Some(container_id) = warm_peer_tab_container(graph_app, tiles_tree, node_key) {
        let node_pane_tile_id = tiles_tree
            .tiles
            .insert_pane(TileKind::Node(node_key.into()));
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(container_id)
        {
            tabs.add_child(node_pane_tile_id);
            tabs.set_active(node_pane_tile_id);
        }
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    open_or_focus_node_pane_with_mode(tiles_tree, graph_app, node_key, TileOpenMode::Tab);
}

pub(crate) fn open_or_focus_node_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) {
    open_or_focus_node_pane_with_mode(tiles_tree, graph_app, node_key, TileOpenMode::Tab);
}

#[cfg(feature = "diagnostics")]
pub(crate) fn open_or_focus_tool_pane(tiles_tree: &mut Tree<TileKind>, kind: ToolPaneState) {
    if tiles_tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Tool(tool)) if tool.kind == kind),
    ) {
        log::debug!("tile_view_ops: focused existing tool pane {:?}", kind);
        return;
    }

    let tool_tile_id = tiles_tree
        .tiles
        .insert_pane(TileKind::Tool(ToolPaneRef::new(kind.clone())));
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(tool_tile_id);
        return;
    };

    if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id) {
        tabs.add_child(tool_tile_id);
        tabs.set_active(tool_tile_id);
        return;
    }

    let tabs_root = tiles_tree
        .tiles
        .insert_tab_tile(vec![root_id, tool_tile_id]);
    tiles_tree.root = Some(tabs_root);
    let _ = tiles_tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Tool(tool)) if tool.kind == kind),
    );
}

#[cfg(not(feature = "diagnostics"))]
pub(crate) fn open_or_focus_tool_pane(
    _tiles_tree: &mut Tree<TileKind>,
    _kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
) {
}

#[cfg(feature = "diagnostics")]
pub(crate) fn close_tool_pane(tiles_tree: &mut Tree<TileKind>, kind: ToolPaneState) -> bool {
    let tool_tile_id = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Tool(existing)) if existing.kind == kind => Some(*tile_id),
            _ => None,
        });

    let Some(tile_id) = tool_tile_id else {
        return false;
    };

    tiles_tree.remove_recursively(tile_id);
    let _ = ensure_active_tile(tiles_tree);
    true
}

pub(crate) fn tile_id_for_pane(
    tiles_tree: &Tree<TileKind>,
    pane_id: PaneId,
) -> Option<egui_tiles::TileId> {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(pane) if pane.pane_id() == pane_id => Some(*tile_id),
            _ => None,
        })
}

pub(crate) fn focus_pane(tiles_tree: &mut Tree<TileKind>, pane_id: PaneId) -> bool {
    tiles_tree.make_active(|_, tile| matches!(tile, Tile::Pane(pane) if pane.pane_id() == pane_id))
}

pub(crate) fn close_pane(tiles_tree: &mut Tree<TileKind>, pane_id: PaneId) -> bool {
    let Some(tile_id) = tile_id_for_pane(tiles_tree, pane_id) else {
        return false;
    };

    tiles_tree.remove_recursively(tile_id);
    let _ = ensure_active_tile(tiles_tree);
    true
}

fn ordered_selected_pane_tile_ids(
    tiles_tree: &Tree<TileKind>,
    selected_tile_ids: &HashSet<TileId>,
    primary_tile_id: Option<TileId>,
) -> Vec<TileId> {
    let mut ordered: Vec<TileId> = tiles_tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| {
            (selected_tile_ids.contains(tile_id) && matches!(tile, Tile::Pane(_)))
                .then_some(*tile_id)
        })
        .collect();
    if let Some(primary_tile_id) = primary_tile_id
        && let Some(index) = ordered
            .iter()
            .position(|tile_id| *tile_id == primary_tile_id)
    {
        let primary = ordered.remove(index);
        ordered.insert(0, primary);
    }
    ordered
}

fn remove_child_from_container(
    tiles_tree: &mut Tree<TileKind>,
    parent_id: TileId,
    child_id: TileId,
) -> bool {
    let Some(parent_tile) = tiles_tree.tiles.get_mut(parent_id) else {
        return false;
    };

    match parent_tile {
        Tile::Container(Container::Tabs(tabs)) => {
            let Some(index) = tabs.children.iter().position(|child| *child == child_id) else {
                return false;
            };
            let was_active = tabs.active == Some(child_id);
            tabs.children.remove(index);
            if was_active {
                let next_active = tabs.children.get(index).copied().or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|left| tabs.children.get(left).copied())
                });
                tabs.active = None;
                if let Some(next_active) = next_active {
                    tabs.set_active(next_active);
                }
            }
            true
        }
        Tile::Container(Container::Linear(linear)) => {
            let Some(index) = linear.children.iter().position(|child| *child == child_id) else {
                return false;
            };
            linear.children.remove(index);
            true
        }
        Tile::Container(Container::Grid(_)) => false,
        Tile::Pane(_) => false,
    }
}

fn clear_container_children(tiles_tree: &mut Tree<TileKind>, container_id: TileId) {
    let Some(container) = tiles_tree.tiles.get_mut(container_id) else {
        return;
    };
    match container {
        Tile::Container(Container::Tabs(tabs)) => {
            tabs.children.clear();
            tabs.active = None;
        }
        Tile::Container(Container::Linear(linear)) => {
            linear.children.clear();
        }
        Tile::Container(Container::Grid(_)) | Tile::Pane(_) => {}
    }
}

fn normalize_parent_after_child_removal(
    tiles_tree: &mut Tree<TileKind>,
    parent_id: TileId,
) -> bool {
    let (child_count, only_child) = match tiles_tree.tiles.get(parent_id) {
        Some(Tile::Container(Container::Tabs(tabs))) => {
            (tabs.children.len(), tabs.children.first().copied())
        }
        Some(Tile::Container(Container::Linear(linear))) => {
            (linear.children.len(), linear.children.first().copied())
        }
        Some(Tile::Container(Container::Grid(_))) => return false,
        _ => return true,
    };

    match child_count {
        0 => {
            if tiles_tree.root() == Some(parent_id) {
                tiles_tree.root = None;
            } else if let Some(grandparent_id) = tiles_tree.tiles.parent_of(parent_id) {
                let _ = remove_child_from_container(tiles_tree, grandparent_id, parent_id);
                let _ = normalize_parent_after_child_removal(tiles_tree, grandparent_id);
            }
            clear_container_children(tiles_tree, parent_id);
            true
        }
        1 => {
            let Some(only_child) = only_child else {
                return false;
            };
            if tiles_tree.root() == Some(parent_id) {
                tiles_tree.root = Some(only_child);
            } else if let Some(grandparent_id) = tiles_tree.tiles.parent_of(parent_id) {
                replace_child_in_parent(tiles_tree, grandparent_id, parent_id, only_child);
            }
            clear_container_children(tiles_tree, parent_id);
            true
        }
        _ => true,
    }
}

fn detach_tile_for_reparent(tiles_tree: &mut Tree<TileKind>, tile_id: TileId) -> bool {
    if tiles_tree.root() == Some(tile_id) {
        tiles_tree.root = None;
        return true;
    }
    let Some(parent_id) = tiles_tree.tiles.parent_of(tile_id) else {
        return false;
    };
    if !remove_child_from_container(tiles_tree, parent_id, tile_id) {
        return false;
    }
    normalize_parent_after_child_removal(tiles_tree, parent_id)
}

pub(crate) fn group_selected_tiles(
    tiles_tree: &mut Tree<TileKind>,
    selected_tile_ids: &HashSet<TileId>,
    primary_tile_id: Option<TileId>,
) -> Option<(Vec<TileId>, TileId)> {
    let ordered_selected =
        ordered_selected_pane_tile_ids(tiles_tree, selected_tile_ids, primary_tile_id);
    if ordered_selected.len() < 2 {
        return None;
    }

    for tile_id in &ordered_selected {
        if !matches!(tiles_tree.tiles.get(*tile_id), Some(Tile::Pane(_))) {
            return None;
        }
    }
    for tile_id in &ordered_selected {
        if !detach_tile_for_reparent(tiles_tree, *tile_id) {
            return None;
        }
    }

    let primary_grouped_tile_id = ordered_selected[0];
    let tile_group_id = tiles_tree.tiles.insert_tab_tile(ordered_selected.clone());
    if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(tile_group_id) {
        tabs.set_active(primary_grouped_tile_id);
    }

    match tiles_tree.root() {
        Some(root_id) => {
            let split_root = tiles_tree
                .tiles
                .insert_horizontal_tile(vec![root_id, tile_group_id]);
            tiles_tree.root = Some(split_root);
        }
        None => {
            tiles_tree.root = Some(tile_group_id);
        }
    }

    let _ = tiles_tree.make_active(|tile_id, _| tile_id == primary_grouped_tile_id);
    Some((ordered_selected, primary_grouped_tile_id))
}

fn replace_child_in_parent(
    tiles_tree: &mut Tree<TileKind>,
    parent_id: TileId,
    old_child: TileId,
    new_child: TileId,
) {
    let Some(parent_tile) = tiles_tree.tiles.get_mut(parent_id) else {
        return;
    };

    match parent_tile {
        Tile::Container(Container::Tabs(tabs)) => {
            let was_active = tabs.active == Some(old_child);
            if let Some(index) = tabs.children.iter().position(|child| *child == old_child) {
                tabs.children[index] = new_child;
                if was_active {
                    tabs.set_active(new_child);
                }
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            if let Some(index) = linear.children.iter().position(|child| *child == old_child) {
                linear.children[index] = new_child;
                linear.shares.replace_with(old_child, new_child);
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            let index = grid
                .children()
                .enumerate()
                .find_map(|(index, child)| (*child == old_child).then_some(index));
            if let Some(index) = index {
                let _ = grid.replace_at(index, new_child);
            }
        }
        Tile::Pane(_) => {}
    }
}

fn wrap_pane_in_split_container(
    tiles_tree: &mut Tree<TileKind>,
    source_pane: PaneId,
    inserted_tile_id: TileId,
    direction: crate::shell::desktop::workbench::pane_model::SplitDirection,
) -> bool {
    let Some(source_tile_id) = tile_id_for_pane(tiles_tree, source_pane) else {
        return false;
    };
    let inserted_pane_id = match tiles_tree.tiles.get(inserted_tile_id) {
        Some(Tile::Pane(pane)) => Some(pane.pane_id()),
        _ => None,
    };
    let source_parent_id = tiles_tree.tiles.parent_of(source_tile_id);

    let source_leaf_tile_id = if matches!(tiles_tree.tiles.get(source_tile_id), Some(Tile::Pane(_)))
    {
        let wrapped = tiles_tree.tiles.insert_tab_tile(vec![source_tile_id]);
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(wrapped) {
            tabs.set_active(source_tile_id);
        }
        if let Some(parent_id) = source_parent_id {
            replace_child_in_parent(tiles_tree, parent_id, source_tile_id, wrapped);
        } else {
            tiles_tree.root = Some(wrapped);
        }
        wrapped
    } else {
        source_tile_id
    };

    let inserted_leaf_tile_id =
        if matches!(tiles_tree.tiles.get(inserted_tile_id), Some(Tile::Pane(_))) {
            let wrapped = tiles_tree.tiles.insert_tab_tile(vec![inserted_tile_id]);
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(wrapped)
            {
                tabs.set_active(inserted_tile_id);
            }
            wrapped
        } else {
            inserted_tile_id
        };
    let source_leaf_parent_id = tiles_tree.tiles.parent_of(source_leaf_tile_id);

    let split_tile_id = match direction {
        crate::shell::desktop::workbench::pane_model::SplitDirection::Horizontal => tiles_tree
            .tiles
            .insert_horizontal_tile(vec![source_leaf_tile_id, inserted_leaf_tile_id]),
        crate::shell::desktop::workbench::pane_model::SplitDirection::Vertical => tiles_tree
            .tiles
            .insert_vertical_tile(vec![source_leaf_tile_id, inserted_leaf_tile_id]),
    };

    if let Some(parent_id) = source_leaf_parent_id {
        replace_child_in_parent(tiles_tree, parent_id, source_leaf_tile_id, split_tile_id);
    } else {
        tiles_tree.root = Some(split_tile_id);
    }

    if let Some(inserted_pane_id) = inserted_pane_id {
        let _ = focus_pane(tiles_tree, inserted_pane_id);
    } else {
        let _ = tiles_tree.make_active(|tile_id, _| tile_id == inserted_leaf_tile_id);
    }
    true
}

pub(crate) fn split_pane_with_new_graph_view(
    tiles_tree: &mut Tree<TileKind>,
    source_pane: PaneId,
    direction: crate::shell::desktop::workbench::pane_model::SplitDirection,
    view_id: GraphViewId,
) -> bool {
    let inserted_tile_id = tiles_tree
        .tiles
        .insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    wrap_pane_in_split_container(tiles_tree, source_pane, inserted_tile_id, direction)
}

#[cfg(not(feature = "diagnostics"))]
pub(crate) fn close_tool_pane(
    _tiles_tree: &mut Tree<TileKind>,
    _kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
) -> bool {
    false
}

pub(crate) fn open_or_focus_node_pane_with_mode(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    mode: TileOpenMode,
) {
    log::debug!(
        "tile_view_ops: open_or_focus_node_pane_with_mode node {:?} mode {:?}",
        node_key,
        mode
    );
    if tiles_tree.make_active(|_, tile| match tile {
        Tile::Pane(kind) => tile_matches_node(kind, node_key) && !kind.is_floating(),
        _ => false,
    }) {
        log::debug!(
            "tile_view_ops: focused existing node pane for node {:?}",
            node_key
        );
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    if matches!(mode, TileOpenMode::QuarterPane | TileOpenMode::HalfPane) {
        insert_floating_node_pane(tiles_tree, node_key);
        return;
    }

    let node_pane_tile_id = tiles_tree
        .tiles
        .insert_pane(TileKind::Node(node_key.into()));
    let split_leaf_tile_id = tiles_tree.tiles.insert_tab_tile(vec![node_pane_tile_id]);
    log::debug!(
        "tile_view_ops: inserted node pane {:?} (split leaf {:?}) for node {:?}",
        node_pane_tile_id,
        split_leaf_tile_id,
        node_key
    );
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(match mode {
            TileOpenMode::Tab => node_pane_tile_id,
            TileOpenMode::SplitHorizontal => split_leaf_tile_id,
            TileOpenMode::QuarterPane | TileOpenMode::HalfPane => node_pane_tile_id,
        });
        log::debug!("tile_view_ops: no root, set root to {:?}", tiles_tree.root);
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    };

    match mode {
        TileOpenMode::Tab | TileOpenMode::QuarterPane | TileOpenMode::HalfPane => {
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(node_pane_tile_id);
                tabs.set_active(node_pane_tile_id);
                tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
                return;
            }

            let tabs_root = tiles_tree
                .tiles
                .insert_tab_tile(vec![root_id, node_pane_tile_id]);
            tiles_tree.root = Some(tabs_root);
            let _ = tiles_tree.make_active(|_, tile| match tile {
                Tile::Pane(kind) => tile_matches_node(kind, node_key) && !kind.is_floating(),
                _ => false,
            });
            tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        }
        TileOpenMode::SplitHorizontal => {
            let split_lhs_id = if matches!(tiles_tree.tiles.get(root_id), Some(Tile::Pane(_))) {
                let wrapped = tiles_tree.tiles.insert_tab_tile(vec![root_id]);
                tiles_tree.root = Some(wrapped);
                wrapped
            } else {
                root_id
            };

            if let Some(Tile::Container(Container::Linear(linear))) =
                tiles_tree.tiles.get_mut(split_lhs_id)
            {
                linear.add_child(split_leaf_tile_id);
                let _ = tiles_tree.make_active(|_, tile| match tile {
                    Tile::Pane(kind) => tile_matches_node(kind, node_key) && !kind.is_floating(),
                    _ => false,
                });
                tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
                return;
            }
            let split_root = tiles_tree
                .tiles
                .insert_horizontal_tile(vec![split_lhs_id, split_leaf_tile_id]);
            tiles_tree.root = Some(split_root);
            let _ = tiles_tree.make_active(|_, tile| match tile {
                Tile::Pane(kind) => tile_matches_node(kind, node_key) && !kind.is_floating(),
                _ => false,
            });
            tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        }
    }
}

pub(crate) fn promote_floating_node_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    mode: TileOpenMode,
) -> Option<NodeKey> {
    let floating = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Pane(PaneViewState::Node(state)))
                if state.presentation_mode == PanePresentationMode::Floating =>
            {
                Some((*tile_id, state.clone()))
            }
            _ => None,
        })?;

    let (floating_tile_id, mut state) = floating;
    remove_unattached_tile(tiles_tree, floating_tile_id);

    if tiles_tree.make_active(|_, tile| match tile {
        Tile::Pane(kind) => tile_matches_node(kind, state.node) && !kind.is_floating(),
        _ => false,
    }) {
        return Some(state.node);
    }

    state.presentation_mode = PanePresentationMode::Tiled;
    let promoted_tile_id = tiles_tree.tiles.insert_pane(TileKind::Node(state.clone()));
    let split_leaf_tile_id = tiles_tree.tiles.insert_tab_tile(vec![promoted_tile_id]);

    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(match mode {
            TileOpenMode::SplitHorizontal => split_leaf_tile_id,
            TileOpenMode::Tab | TileOpenMode::QuarterPane | TileOpenMode::HalfPane => {
                promoted_tile_id
            }
        });
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return Some(state.node);
    };

    match mode {
        TileOpenMode::Tab | TileOpenMode::QuarterPane | TileOpenMode::HalfPane => {
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(promoted_tile_id);
                tabs.set_active(promoted_tile_id);
            } else {
                let tabs_root = tiles_tree
                    .tiles
                    .insert_tab_tile(vec![root_id, promoted_tile_id]);
                tiles_tree.root = Some(tabs_root);
            }
        }
        TileOpenMode::SplitHorizontal => {
            let split_lhs_id = if matches!(tiles_tree.tiles.get(root_id), Some(Tile::Pane(_))) {
                let wrapped = tiles_tree.tiles.insert_tab_tile(vec![root_id]);
                tiles_tree.root = Some(wrapped);
                wrapped
            } else {
                root_id
            };

            if let Some(Tile::Container(Container::Linear(linear))) =
                tiles_tree.tiles.get_mut(split_lhs_id)
            {
                linear.add_child(split_leaf_tile_id);
            } else {
                let split_root = tiles_tree
                    .tiles
                    .insert_horizontal_tile(vec![split_lhs_id, split_leaf_tile_id]);
                tiles_tree.root = Some(split_root);
            }
        }
    }

    let _ = tiles_tree.make_active(|_, tile| match tile {
        Tile::Pane(kind) => tile_matches_node(kind, state.node) && !kind.is_floating(),
        _ => false,
    });
    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
    Some(state.node)
}

pub(crate) fn dismiss_floating_panes(tiles_tree: &mut Tree<TileKind>) {
    remove_all_floating_panes(tiles_tree);
}

/// Open all members of a frame as a tile group, or focus the existing group.
///
/// Implements the frame → tile-group 1:1 cardinality contract:
/// if a tabs container already holds tiles for any member of `frame_anchor`,
/// that container is treated as the frame's tile group and is focused instead
/// of creating a second group.
///
/// If `focus_key` is `Some`, the tile for that node is made active within
/// the group after opening/focusing.
pub(crate) fn open_or_focus_frame_tile_group(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    frame_anchor: NodeKey,
    focus_key: Option<NodeKey>,
) {
    let member_keys = graph_app.outgoing_membership_nodes(frame_anchor);
    if member_keys.is_empty() {
        log::debug!(
            "tile_view_ops: open_or_focus_frame_tile_group: frame anchor {:?} has no members",
            frame_anchor
        );
        return;
    }
    let focus_key = focus_key.unwrap_or(member_keys[0]);

    // 1:1 cardinality: find an existing tabs container that already holds
    // a tile for any member of this frame anchor.
    let existing_group_id = find_frame_tile_group(tiles_tree, &member_keys);

    if let Some(group_id) = existing_group_id {
        // Focus the group container, then focus the specific member's tile.
        let _ = tiles_tree.make_active(|tile_id, _| tile_id == group_id);
        focus_member_tile_in_group(tiles_tree, group_id, focus_key);
        log::debug!(
            "tile_view_ops: focused existing frame tile group {:?} for anchor {:?}",
            group_id,
            frame_anchor
        );
        return;
    }

    // Create new tabs container with one tile per member.
    let member_tile_ids: Vec<TileId> = member_keys
        .iter()
        .map(|&key| tiles_tree.tiles.insert_pane(TileKind::Node(key.into())))
        .collect();

    let group_id = tiles_tree.tiles.insert_tab_tile(member_tile_ids.clone());

    // Focus the desired member tile within the new group.
    if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(group_id) {
        if let Some(&focus_tile_id) = member_keys
            .iter()
            .zip(member_tile_ids.iter())
            .find_map(|(&key, tile_id)| (key == focus_key).then_some(tile_id))
        {
            tabs.set_active(focus_tile_id);
        }
    }

    // Insert the group into the tree.
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(group_id);
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        log::debug!(
            "tile_view_ops: opened new frame tile group {:?} for anchor {:?} (was empty tree)",
            group_id,
            frame_anchor
        );
        return;
    };

    match tiles_tree.tiles.get_mut(root_id) {
        Some(Tile::Container(Container::Tabs(tabs))) => {
            tabs.add_child(group_id);
            tabs.set_active(group_id);
        }
        _ => {
            let tabs_root = tiles_tree
                .tiles
                .insert_tab_tile(vec![root_id, group_id]);
            tiles_tree.root = Some(tabs_root);
            let _ = tiles_tree.make_active(|tile_id, _| tile_id == group_id);
        }
    }

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
    log::debug!(
        "tile_view_ops: opened new frame tile group {:?} for anchor {:?}",
        group_id,
        frame_anchor
    );
}

/// Find a tabs container that holds tiles for any member of this frame.
///
/// Iterates over all member keys and returns the parent `TileId` of the first
/// non-floating pane that is a direct child of a `Container::Tabs`.
fn find_frame_tile_group(tiles_tree: &Tree<TileKind>, member_keys: &[NodeKey]) -> Option<TileId> {
    for &member_key in member_keys {
        let Some(member_tile_id) = tiles_tree.tiles.iter().find_map(|(tile_id, tile)| {
            matches!(tile, Tile::Pane(kind) if tile_matches_node(kind, member_key) && !kind.is_floating())
                .then_some(*tile_id)
        }) else {
            continue;
        };
        let Some(parent_id) = tiles_tree.tiles.parent_of(member_tile_id) else {
            continue;
        };
        if matches!(
            tiles_tree.tiles.get(parent_id),
            Some(Tile::Container(Container::Tabs(_)))
        ) {
            return Some(parent_id);
        }
    }
    None
}

/// Focus the tile for `focus_key` within the given tabs container.
fn focus_member_tile_in_group(
    tiles_tree: &mut Tree<TileKind>,
    group_id: TileId,
    focus_key: NodeKey,
) {
    let focus_tile_id = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| {
            if let Tile::Pane(kind) = tile {
                if tile_matches_node(kind, focus_key)
                    && !kind.is_floating()
                    && tiles_tree.tiles.parent_of(*tile_id) == Some(group_id)
                {
                    return Some(*tile_id);
                }
            }
            None
        });
    let Some(focus_tile_id) = focus_tile_id else {
        return;
    };
    if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(group_id) {
        tabs.set_active(focus_tile_id);
    }
}

pub(crate) fn detach_node_pane_to_split(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) {
    let existing_tile_id = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
            _ => None,
        });

    if let Some(tile_id) = existing_tile_id {
        tiles_tree.remove_recursively(tile_id);
    }
    open_or_focus_node_pane_with_mode(
        tiles_tree,
        graph_app,
        node_key,
        TileOpenMode::SplitHorizontal,
    );
}

pub(crate) fn toggle_tile_view(args: ToggleTileViewArgs<'_>) {
    if tile_runtime::has_any_node_panes(args.tiles_tree) {
        let node_pane_nodes = tile_runtime::all_node_pane_keys(args.tiles_tree);
        let tile_ids: Vec<_> = args
            .tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(_)) => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            args.tiles_tree.remove_recursively(tile_id);
        }
        for node_key in node_pane_nodes.iter().copied() {
            tile_runtime::release_node_runtime_for_pane(
                args.graph_app,
                args.window,
                args.tile_rendering_contexts,
                node_key,
                args.lifecycle_intents,
            );
        }
    } else if let Some(node_key) = preferred_detail_node(args.graph_app) {
        open_or_focus_node_pane(args.tiles_tree, args.graph_app, node_key);
        let opened_node_pane = NodePaneState::for_node(node_key);
        if tile_runtime::node_pane_uses_composited_runtime(&opened_node_pane, args.graph_app) {
            webview_backpressure::ensure_webview_for_node(
                args.graph_app,
                args.window,
                args.app_state,
                args.base_rendering_context,
                args.window_rendering_context,
                args.tile_rendering_contexts,
                None,
                node_key,
                args.responsive_webviews,
                args.webview_creation_backpressure,
                args.lifecycle_intents,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphBrowserApp;
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, ToolPaneRef, ToolPaneState};
    use egui_tiles::Tiles;

    fn graph_pane(view_id: GraphViewId) -> TileKind {
        TileKind::Graph(GraphPaneRef::new(view_id))
    }

    #[cfg(feature = "diagnostics")]
    fn tool_pane(kind: ToolPaneState) -> TileKind {
        TileKind::Tool(ToolPaneRef::new(kind))
    }

    fn count_graph_panes(tiles_tree: &Tree<TileKind>) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
            .count()
    }

    fn count_node_panes(tiles_tree: &Tree<TileKind>) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
            .count()
    }

    fn active_graph_view(tiles_tree: &Tree<TileKind>) -> Option<GraphViewId> {
        active_graph_view_id(tiles_tree)
    }

    fn active_region_name(tiles_tree: &Tree<TileKind>) -> Option<&'static str> {
        tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
            match tiles_tree.tiles.get(tile_id) {
                Some(Tile::Pane(TileKind::Graph(_))) => Some("graph"),
                Some(Tile::Pane(TileKind::Node(_))) => Some("node"),
                Some(Tile::Pane(TileKind::Pane(PaneViewState::Graph(_)))) => Some("graph"),
                Some(Tile::Pane(TileKind::Pane(PaneViewState::Node(_)))) => Some("node"),
                #[cfg(feature = "diagnostics")]
                Some(Tile::Pane(TileKind::Tool(_))) => Some("tool"),
                #[cfg(feature = "diagnostics")]
                Some(Tile::Pane(TileKind::Pane(PaneViewState::Tool(_)))) => Some("tool"),
                _ => None,
            }
        })
    }

    fn count_floating_node_panes(tiles_tree: &Tree<TileKind>) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(|(_, tile)| {
                matches!(
                    tile,
                    Tile::Pane(TileKind::Pane(PaneViewState::Node(state)))
                        if state.presentation_mode == PanePresentationMode::Floating
                )
            })
            .count()
    }

    #[test]
    fn open_or_focus_graph_pane_focuses_existing_graph_in_mixed_tree() {
        let graph_a = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(0).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("graph_focus_existing", root, tiles);

        assert_eq!(count_graph_panes(&tree), 1);
        assert_eq!(count_node_panes(&tree), 1);

        open_or_focus_graph_pane(&mut tree, graph_a);

        assert_eq!(count_graph_panes(&tree), 1);
        assert_eq!(count_node_panes(&tree), 1);
        assert_eq!(active_graph_view(&tree), Some(graph_a));
    }

    #[test]
    fn open_or_focus_graph_pane_inserts_new_graph_tab_with_requested_id() {
        let graph_a = GraphViewId::new();
        let graph_b = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(1).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("graph_open_new_tab", root, tiles);

        open_or_focus_graph_pane(&mut tree, graph_b);

        assert_eq!(count_graph_panes(&tree), 2);
        assert_eq!(count_node_panes(&tree), 1);
        assert_eq!(active_graph_view(&tree), Some(graph_b));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == graph_b)));
    }

    #[test]
    fn open_or_focus_graph_pane_split_preserves_ids_and_focuses_new_graph() {
        let graph_a = GraphViewId::new();
        let graph_b = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_a));
        let mut tree = Tree::new("graph_split", graph_tile, tiles);

        open_or_focus_graph_pane_with_mode(&mut tree, graph_b, TileOpenMode::SplitHorizontal);

        assert_eq!(count_graph_panes(&tree), 2);
        assert_eq!(active_graph_view(&tree), Some(graph_b));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == graph_a)));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == graph_b)));
    }

    #[test]
    fn ensure_active_tile_is_noop_when_tree_already_has_active_tile() {
        let graph_a = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(2).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("ensure_active_tile", root, tiles);

        assert!(!ensure_active_tile(&mut tree));
        assert_eq!(active_graph_view(&tree), Some(graph_a));
    }

    #[test]
    fn ensure_active_tile_recovers_after_active_node_tile_is_removed() {
        let graph_a = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(5).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("ensure_active_tile_after_node_close", root, tiles);

        let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Node(_))));
        tree.remove_recursively(node_tile);

        assert!(ensure_active_tile(&mut tree));
        assert_eq!(active_graph_view(&tree), Some(graph_a));
    }

    #[test]
    fn open_or_focus_node_pane_split_wraps_leaf_root_before_split() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://example.com/split-root-wrap".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root_graph = tiles.insert_pane(graph_pane(GraphViewId::new()));
        let mut tree = Tree::new("node_split_wrap", root_graph, tiles);

        open_or_focus_node_pane_with_mode(&mut tree, &app, node_key, TileOpenMode::SplitHorizontal);

        let root_id = tree.root().expect("split root should exist");
        let linear = match tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Linear(linear))) => linear,
            other => panic!("expected split root container, got {other:?}"),
        };
        assert_eq!(linear.children.len(), 2);
        for child in &linear.children {
            assert!(matches!(
                tree.tiles.get(*child),
                Some(Tile::Container(Container::Tabs(_)))
            ));
        }
        assert!(tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Node(state))) if state.node == node_key
            )
        }));
    }

    #[test]
    fn open_or_focus_node_pane_quarter_creates_single_floating_ephemeral_pane() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://example.com/floating-quarter".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root_graph = tiles.insert_pane(graph_pane(GraphViewId::new()));
        let mut tree = Tree::new("floating_quarter", root_graph, tiles);

        open_or_focus_node_pane_with_mode(&mut tree, &app, node_key, TileOpenMode::QuarterPane);
        assert_eq!(count_floating_node_panes(&tree), 1);

        open_or_focus_node_pane_with_mode(&mut tree, &app, node_key, TileOpenMode::QuarterPane);
        assert_eq!(count_floating_node_panes(&tree), 1);
    }

    #[test]
    fn promote_floating_node_pane_converts_to_tiled_node_pane() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://example.com/floating-promote".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root_graph = tiles.insert_pane(graph_pane(GraphViewId::new()));
        let mut tree = Tree::new("floating_promote", root_graph, tiles);

        open_or_focus_node_pane_with_mode(&mut tree, &app, node_key, TileOpenMode::HalfPane);
        assert_eq!(count_floating_node_panes(&tree), 1);

        let promoted = promote_floating_node_pane(&mut tree, &app, TileOpenMode::Tab);
        assert_eq!(promoted, Some(node_key));
        assert_eq!(count_floating_node_panes(&tree), 0);
        assert!(tree.tiles.iter().any(
            |(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
        ));
    }

    #[test]
    fn dismiss_floating_panes_removes_ephemeral_carriers() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://example.com/floating-dismiss".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root_graph = tiles.insert_pane(graph_pane(GraphViewId::new()));
        let mut tree = Tree::new("floating_dismiss", root_graph, tiles);

        open_or_focus_node_pane_with_mode(&mut tree, &app, node_key, TileOpenMode::QuarterPane);
        assert_eq!(count_floating_node_panes(&tree), 1);

        dismiss_floating_panes(&mut tree);
        assert_eq!(count_floating_node_panes(&tree), 0);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn cycle_focus_region_rotates_graph_node_tool_deterministically() {
        let graph_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_view));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(7).into()));
        let tool_tile = tiles.insert_pane(tool_pane(ToolPaneState::Diagnostics));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile, tool_tile]);
        let mut tree = Tree::new("cycle_focus_regions", root, tiles);

        let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Graph(_))));
        assert_eq!(active_region_name(&tree), Some("graph"));

        assert!(cycle_focus_region(&mut tree));
        assert_eq!(active_region_name(&tree), Some("node"));

        assert!(cycle_focus_region(&mut tree));
        assert_eq!(active_region_name(&tree), Some("tool"));

        assert!(cycle_focus_region(&mut tree));
        assert_eq!(active_region_name(&tree), Some("graph"));
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn cycle_focus_region_skips_absent_regions() {
        let graph_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(graph_pane(graph_view));
        let tool_tile = tiles.insert_pane(tool_pane(ToolPaneState::Settings));
        let root = tiles.insert_tab_tile(vec![graph_tile, tool_tile]);
        let mut tree = Tree::new("cycle_focus_skip_absent_node", root, tiles);

        let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Graph(_))));
        assert_eq!(active_region_name(&tree), Some("graph"));

        assert!(cycle_focus_region(&mut tree));
        assert_eq!(active_region_name(&tree), Some("tool"));

        assert!(cycle_focus_region(&mut tree));
        assert_eq!(active_region_name(&tree), Some("graph"));
    }

    /// Helper: create an app with a frame anchor having `n` members.
    ///
    /// Returns `(app, frame_anchor_key, [member_key_0, ..., member_key_n-1])`.
    fn make_frame_with_members(
        n: usize,
    ) -> (GraphBrowserApp, crate::graph::NodeKey, Vec<crate::graph::NodeKey>) {
        let mut app = GraphBrowserApp::new_for_testing();
        let member_keys: Vec<crate::graph::NodeKey> = (0..n)
            .map(|i| {
                app.add_node_and_sync(
                    format!("https://example.com/member-{i}"),
                    euclid::default::Point2D::new(i as f32, 0.0),
                )
            })
            .collect();

        // Build a temporary tile tree containing all member nodes and use
        // the high-level bridge method to register the frame in the graph.
        let mut setup_tiles = Tiles::default();
        let tile_ids: Vec<_> = member_keys
            .iter()
            .map(|&key| setup_tiles.insert_pane(TileKind::Node(key.into())))
            .collect();
        let root = if tile_ids.len() == 1 {
            tile_ids[0]
        } else {
            setup_tiles.insert_tab_tile(tile_ids)
        };
        let setup_tree = Tree::new("frame_setup", root, setup_tiles);
        let frame_anchor =
            app.sync_named_workbench_frame_graph_representation("test-frame", &setup_tree);
        (app, frame_anchor, member_keys)
    }

    #[test]
    fn open_frame_tile_group_creates_tabs_container_for_all_members() {
        let (app, frame_anchor, member_keys) = make_frame_with_members(2);
        let graph_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(graph_pane(graph_view));
        let mut tree = Tree::new("frame_group_create", root, tiles);

        open_or_focus_frame_tile_group(&mut tree, &app, frame_anchor, None);

        // Both member node panes must exist.
        assert_eq!(count_node_panes(&tree), 2);

        // Exactly one tabs container holds both member tiles.
        let group_id = find_frame_tile_group(&tree, &member_keys);
        assert!(group_id.is_some(), "expected a tabs container for frame members");

        let group_id = group_id.unwrap();
        if let Some(Tile::Container(Container::Tabs(tabs))) = tree.tiles.get(group_id) {
            assert_eq!(tabs.children.len(), 2);
        } else {
            panic!("expected a Tabs container for the frame tile group");
        }
    }

    #[test]
    fn open_frame_tile_group_focuses_existing_group_on_second_call() {
        let (app, frame_anchor, member_keys) = make_frame_with_members(2);
        let graph_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(graph_pane(graph_view));
        let mut tree = Tree::new("frame_group_idempotent", root, tiles);

        open_or_focus_frame_tile_group(&mut tree, &app, frame_anchor, None);
        let node_pane_count_after_first = count_node_panes(&tree);
        let group_id_after_first = find_frame_tile_group(&tree, &member_keys);

        open_or_focus_frame_tile_group(&mut tree, &app, frame_anchor, None);
        let node_pane_count_after_second = count_node_panes(&tree);
        let group_id_after_second = find_frame_tile_group(&tree, &member_keys);

        // Second call must not create new panes — 1:1 cardinality.
        assert_eq!(
            node_pane_count_after_first, node_pane_count_after_second,
            "second call must not duplicate frame members"
        );
        assert_eq!(node_pane_count_after_second, 2);

        // The frame tile group container is the same object both times.
        assert!(group_id_after_first.is_some(), "frame tile group must exist after first call");
        assert_eq!(
            group_id_after_first, group_id_after_second,
            "expected the same frame tile group after two calls (1:1 cardinality)"
        );
    }

    #[test]
    fn open_frame_tile_group_with_focus_key_makes_correct_tile_active() {
        let (app, frame_anchor, member_keys) = make_frame_with_members(2);
        let focus_key = member_keys[1]; // Focus the second member.
        let graph_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(graph_pane(graph_view));
        let mut tree = Tree::new("frame_group_focus", root, tiles);

        open_or_focus_frame_tile_group(&mut tree, &app, frame_anchor, Some(focus_key));

        // The active tile should be the pane for focus_key.
        let active_is_focus_key = tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Node(state))) if state.node == focus_key
            )
        });
        assert!(
            active_is_focus_key,
            "the focused member's tile should be active"
        );
    }
}
