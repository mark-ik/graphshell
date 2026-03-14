/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;

use egui::{RichText, SidePanel};
use egui_tiles::{Container, LinearDir, Tile, TileId, Tree};

use crate::app::{GraphBrowserApp, GraphViewId, WorkbenchIntent};
use crate::graph::{ArrangementSubKind, NodeKey};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarNavAction};
use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchLayerState {
    GraphOnly,
    WorkbenchActive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchPaneKind {
    Graph { view_id: GraphViewId },
    Node { node_key: NodeKey },
    Tool { kind: ToolPaneState },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchPaneEntry {
    pub(crate) pane_id: PaneId,
    pub(crate) kind: WorkbenchPaneKind,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) is_active: bool,
    pub(crate) closable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchChromeProjection {
    pub(crate) layer_state: WorkbenchLayerState,
    pub(crate) active_pane_title: Option<String>,
    pub(crate) saved_frame_names: Vec<String>,
    pub(crate) navigator_groups: Vec<WorkbenchNavigatorGroup>,
    pub(crate) pane_entries: Vec<WorkbenchPaneEntry>,
    pub(crate) tree_root: Option<WorkbenchChromeNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchNavigatorGroup {
    pub(crate) title: String,
    pub(crate) sub_kind: ArrangementSubKind,
    pub(crate) members: Vec<WorkbenchNavigatorMember>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchNavigatorMember {
    pub(crate) node_key: NodeKey,
    pub(crate) title: String,
    pub(crate) is_selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchChromeNode {
    Pane(WorkbenchPaneEntry),
    Tabs {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
    Split {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
    Grid {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
}

impl WorkbenchChromeProjection {
    pub(crate) fn from_tree(
        graph_app: &GraphBrowserApp,
        tiles_tree: &Tree<TileKind>,
        active_pane: Option<PaneId>,
    ) -> Self {
        let mut saved_frame_names = graph_app
            .list_workspace_layout_names()
            .into_iter()
            .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
            .collect::<Vec<_>>();
        saved_frame_names.sort();
        let pane_entries = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(pane_entry_for_tile(graph_app, kind, active_pane)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let graph_pane_count = pane_entries
            .iter()
            .filter(|entry| matches!(entry.kind, WorkbenchPaneKind::Graph { .. }))
            .count();
        let layer_state = if pane_entries
            .iter()
            .any(|entry| !matches!(entry.kind, WorkbenchPaneKind::Graph { .. }))
            || graph_pane_count > 1
        {
            WorkbenchLayerState::WorkbenchActive
        } else {
            WorkbenchLayerState::GraphOnly
        };
        let active_pane_title = pane_entries
            .iter()
            .find(|entry| entry.is_active)
            .map(|entry| entry.title.clone());
        let navigator_groups = graph_app
            .arrangement_projection_groups()
            .into_iter()
            .map(|group| WorkbenchNavigatorGroup {
                title: match group.sub_kind {
                    ArrangementSubKind::FrameMember => format!("Frame: {}", group.title),
                    ArrangementSubKind::TileGroup => format!("Tile Group: {}", group.title),
                    ArrangementSubKind::SplitPair => format!("Split Pair: {}", group.title),
                },
                sub_kind: group.sub_kind,
                members: group
                    .member_keys
                    .into_iter()
                    .filter_map(|node_key| {
                        let node = graph_app.domain_graph().get_node(node_key)?;
                        let title = if node.title.trim().is_empty() {
                            node.url.clone()
                        } else {
                            node.title.clone()
                        };
                        Some(WorkbenchNavigatorMember {
                            node_key,
                            title,
                            is_selected: graph_app.focused_selection().contains(&node_key),
                        })
                    })
                    .collect(),
            })
            .collect();
        let tree_root = tiles_tree
            .root()
            .and_then(|root| build_tree_node(graph_app, tiles_tree, root, active_pane));
        Self {
            layer_state,
            active_pane_title,
            saved_frame_names,
            navigator_groups,
            pane_entries,
            tree_root,
        }
    }

    pub(crate) fn visible(&self) -> bool {
        matches!(self.layer_state, WorkbenchLayerState::WorkbenchActive)
    }
}

fn pane_entry_for_tile(
    graph_app: &GraphBrowserApp,
    kind: &TileKind,
    active_pane: Option<PaneId>,
) -> WorkbenchPaneEntry {
    match kind {
        TileKind::Graph(graph_ref) => WorkbenchPaneEntry {
            pane_id: graph_ref.pane_id,
            kind: WorkbenchPaneKind::Graph {
                view_id: graph_ref.graph_view_id,
            },
            title: graph_view_title(graph_app, graph_ref.graph_view_id),
            subtitle: Some("Graph".to_string()),
            is_active: active_pane == Some(graph_ref.pane_id),
            closable: false,
        },
        TileKind::Node(state) => {
            let title = graph_app
                .domain_graph()
                .get_node(state.node)
                .and_then(|node| {
                    let title = node.title.trim();
                    (!title.is_empty()).then(|| title.to_string())
                })
                .unwrap_or_else(|| format!("Node {}", state.node.index()));
            let subtitle = graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|node| node.url.clone())
                .filter(|url| !url.trim().is_empty());
            WorkbenchPaneEntry {
                pane_id: state.pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: state.node,
                },
                title,
                subtitle,
                is_active: active_pane == Some(state.pane_id),
                closable: true,
            }
        }
        #[cfg(feature = "diagnostics")]
        TileKind::Tool(tool) => WorkbenchPaneEntry {
            pane_id: tool.pane_id,
            kind: WorkbenchPaneKind::Tool {
                kind: tool.kind.clone(),
            },
            title: tool.title().to_string(),
            subtitle: Some("Tool".to_string()),
            is_active: active_pane == Some(tool.pane_id),
            closable: true,
        },
    }
}

fn graph_view_title(graph_app: &GraphBrowserApp, view_id: GraphViewId) -> String {
    graph_app
        .workspace
        .views
        .get(&view_id)
        .map(|view| view.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Graph View".to_string())
}

fn build_tree_node(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tile_id: TileId,
    active_pane: Option<PaneId>,
) -> Option<WorkbenchChromeNode> {
    let tile = tiles_tree.tiles.get(tile_id)?;
    match tile {
        Tile::Pane(kind) => Some(WorkbenchChromeNode::Pane(pane_entry_for_tile(
            graph_app,
            kind,
            active_pane,
        ))),
        Tile::Container(Container::Tabs(tabs)) => Some(WorkbenchChromeNode::Tabs {
            tile_id,
            label: format!("Tab Group ({})", tabs.children.len()),
            children: tabs
                .children
                .iter()
                .filter_map(|child| build_tree_node(graph_app, tiles_tree, *child, active_pane))
                .collect(),
        }),
        Tile::Container(Container::Linear(linear)) => {
            let dir_label = match linear.dir {
                LinearDir::Horizontal => "Split ↔",
                LinearDir::Vertical => "Split ↕",
            };
            Some(WorkbenchChromeNode::Split {
                tile_id,
                label: format!("{dir_label} ({})", linear.children.len()),
                children: linear
                    .children
                    .iter()
                    .filter_map(|child| build_tree_node(graph_app, tiles_tree, *child, active_pane))
                    .collect(),
            })
        }
        Tile::Container(Container::Grid(grid)) => Some(WorkbenchChromeNode::Grid {
            tile_id,
            label: format!("Grid ({})", grid.children().count()),
            children: grid
                .children()
                .filter_map(|child| build_tree_node(graph_app, tiles_tree, *child, active_pane))
                .collect(),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_workbench_sidebar(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    focused_toolbar_node: Option<NodeKey>,
    active_toolbar_pane: Option<PaneId>,
    can_go_back: bool,
    can_go_forward: bool,
    location_dirty: &mut bool,
) -> WorkbenchChromeProjection {
    let projection =
        WorkbenchChromeProjection::from_tree(graph_app, tiles_tree, active_toolbar_pane);
    if !projection.visible() {
        return projection;
    }

    let persisted_frame_names: HashSet<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .collect();
    let focused_pane_pin_name =
        focused_toolbar_node.and_then(|node| frame_pin_name_for_node(node, graph_app));
    let mut post_panel_action = None;

    SidePanel::right("workbench_sidebar")
        .resizable(true)
        .default_width(260.0)
        .min_width(180.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Workbench");
                if let Some(active_title) = &projection.active_pane_title {
                    ui.label(RichText::new(active_title).small().weak());
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    render_navigation_buttons(
                        ui,
                        graph_app,
                        window,
                        focused_toolbar_node,
                        can_go_back,
                        can_go_forward,
                        location_dirty,
                    );
                    render_frame_pin_controls(
                        ui,
                        graph_app,
                        true,
                        focused_pane_pin_name.as_deref(),
                        &persisted_frame_names,
                    );
                    ui.menu_button(
                        format!("Frames ({})", projection.saved_frame_names.len()),
                        |ui| {
                            if ui.button("Save Current Frame Snapshot").clicked() {
                                post_panel_action = Some(SidebarAction::SaveCurrentFrame);
                                ui.close();
                            }
                            if ui.button("Prune Empty Named Frames").clicked() {
                                post_panel_action = Some(SidebarAction::PruneEmptyFrames);
                                ui.close();
                            }
                            ui.separator();
                            if projection.saved_frame_names.is_empty() {
                                ui.label(RichText::new("No saved frames").small().weak());
                                return;
                            }
                            for frame_name in &projection.saved_frame_names {
                                if ui.button(frame_name).clicked() {
                                    post_panel_action =
                                        Some(SidebarAction::RestoreFrame(frame_name.clone()));
                                    ui.close();
                                }
                            }
                        },
                    );
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Settings").clicked() {
                        post_panel_action = Some(SidebarAction::OpenTool(ToolPaneState::Settings));
                    }
                    if ui.small_button("History").clicked() {
                        post_panel_action =
                            Some(SidebarAction::OpenTool(ToolPaneState::HistoryManager));
                    }
                    if ui.small_button("File Tree").clicked() {
                        post_panel_action = Some(SidebarAction::OpenTool(ToolPaneState::FileTree));
                    }
                });
                ui.separator();

                if !projection.navigator_groups.is_empty() {
                    ui.heading("Navigator");
                    for group in &projection.navigator_groups {
                        let header = egui::CollapsingHeader::new(
                            RichText::new(&group.title).small().strong(),
                        )
                        .id_salt(("workbench_sidebar_navigator", &group.title))
                        .default_open(true);
                        header.show(ui, |ui| {
                            for member in &group.members {
                                let response = ui.selectable_label(
                                    member.is_selected,
                                    RichText::new(&member.title).small(),
                                );
                                if response.clicked() {
                                    post_panel_action =
                                        Some(SidebarAction::SelectNode(member.node_key));
                                }
                            }
                        });
                    }
                    ui.separator();
                }

                ui.label(RichText::new("Tile Tree (legacy fallback)").small().weak());

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(root) = projection.tree_root.as_ref() {
                        render_tree_node(ui, root, 0, &mut post_panel_action);
                    }
                });
            });
        });

    if let Some(action) = post_panel_action {
        apply_sidebar_action(action, graph_app, tiles_tree);
    }

    projection
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SidebarAction {
    FocusPane(PaneId),
    SelectNode(NodeKey),
    SplitPane(PaneId, SplitDirection),
    ClosePane(PaneId),
    OpenTool(ToolPaneState),
    SaveCurrentFrame,
    PruneEmptyFrames,
    RestoreFrame(String),
}

fn render_tree_node(
    ui: &mut egui::Ui,
    node: &WorkbenchChromeNode,
    depth: usize,
    action: &mut Option<SidebarAction>,
) {
    match node {
        WorkbenchChromeNode::Pane(entry) => {
            ui.add_space((depth as f32) * 10.0);
            ui.horizontal(|ui| {
                let text = if entry.is_active {
                    RichText::new(&entry.title).strong()
                } else {
                    RichText::new(&entry.title)
                };
                let response = ui.selectable_label(entry.is_active, text);
                if response.clicked() {
                    *action = Some(SidebarAction::FocusPane(entry.pane_id));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if entry.closable && ui.small_button("x").on_hover_text("Close").clicked() {
                        *action = Some(SidebarAction::ClosePane(entry.pane_id));
                    }
                    if ui
                        .small_button("V")
                        .on_hover_text("Split vertically")
                        .clicked()
                    {
                        *action = Some(SidebarAction::SplitPane(
                            entry.pane_id,
                            SplitDirection::Vertical,
                        ));
                    }
                    if ui
                        .small_button("H")
                        .on_hover_text("Split horizontally")
                        .clicked()
                    {
                        *action = Some(SidebarAction::SplitPane(
                            entry.pane_id,
                            SplitDirection::Horizontal,
                        ));
                    }
                });
            });
            if let Some(subtitle) = &entry.subtitle {
                ui.add_space((depth as f32) * 10.0 + 2.0);
                ui.label(RichText::new(subtitle).small().weak());
            }
            ui.add_space(6.0);
        }
        WorkbenchChromeNode::Tabs {
            tile_id,
            label,
            children,
        }
        | WorkbenchChromeNode::Split {
            tile_id,
            label,
            children,
        }
        | WorkbenchChromeNode::Grid {
            tile_id,
            label,
            children,
        } => {
            let header = egui::CollapsingHeader::new(RichText::new(label).small().strong())
                .id_salt(("workbench_sidebar_container", tile_id))
                .default_open(true);
            header.show(ui, |ui| {
                for child in children {
                    render_tree_node(ui, child, depth + 1, action);
                }
            });
            ui.add_space(4.0);
        }
    }
}

fn apply_sidebar_action(
    action: SidebarAction,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    match action {
        SidebarAction::FocusPane(pane_id) => {
            let _ = tile_view_ops::focus_pane(tiles_tree, pane_id);
        }
        SidebarAction::SelectNode(node_key) => {
            graph_app.select_node(node_key, false);
        }
        SidebarAction::SplitPane(source_pane, direction) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SplitPane {
                source_pane,
                direction,
            });
        }
        SidebarAction::ClosePane(pane) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::ClosePane {
                pane,
                restore_previous_focus: true,
            });
        }
        SidebarAction::OpenTool(kind) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane { kind });
        }
        SidebarAction::SaveCurrentFrame => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            graph_app.request_save_frame_snapshot_named(format!("workspace:sidebar-{now}"));
        }
        SidebarAction::PruneEmptyFrames => {
            graph_app.request_prune_empty_frames();
        }
        SidebarAction::RestoreFrame(name) => {
            graph_app.request_restore_frame_snapshot_named(name);
        }
    }
}

fn frame_pin_name_for_node(node_key: NodeKey, graph_app: &GraphBrowserApp) -> Option<String> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let title = node.title.trim();
    let label = if title.is_empty() {
        node.url.trim()
    } else {
        title
    };
    let sanitized = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        Some(format!("pane-node-{}", node_key.index()))
    } else {
        Some(format!("pane-{sanitized}"))
    }
}

fn toolbar_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .frame(false)
        .min_size(egui::Vec2 { x: 20.0, y: 20.0 })
}

fn render_navigation_buttons(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    focused_toolbar_node: Option<NodeKey>,
    can_go_back: bool,
    can_go_forward: bool,
    location_dirty: &mut bool,
) {
    if ui
        .add_enabled(can_go_back, toolbar_button("<"))
        .on_hover_text("Back")
        .clicked()
    {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Back,
        );
    }
    if ui
        .add_enabled(can_go_forward, toolbar_button(">"))
        .on_hover_text("Forward")
        .clicked()
    {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Forward,
        );
    }
    if ui
        .add(toolbar_button("R"))
        .on_hover_text("Reload")
        .clicked()
    {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Reload,
        );
    }
}

fn render_frame_pin_controls(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    has_hosted_panes: bool,
    focused_pane_pin_name: Option<&str>,
    persisted_frame_names: &HashSet<String>,
) {
    if !has_hosted_panes {
        return;
    }

    if let Some(pane_pin_name) = focused_pane_pin_name {
        let pane_is_pinned = persisted_frame_names.contains(pane_pin_name);
        let pane_pin_label = if pane_is_pinned { "P-" } else { "P+" };
        let pane_pin_button =
            ui.add(toolbar_button(pane_pin_label))
                .on_hover_text(if pane_is_pinned {
                    "Unpin focused pane frame snapshot"
                } else {
                    "Pin focused pane frame snapshot"
                });
        if pane_pin_button.clicked() {
            if pane_is_pinned {
                if let Err(error) = graph_app.delete_workspace_layout(pane_pin_name) {
                    log::warn!("Failed to unpin focused pane workspace '{pane_pin_name}': {error}");
                }
            } else {
                graph_app.request_save_frame_snapshot_named(pane_pin_name.to_string());
            }
        }
    }

    let workspace_pin_name = "workspace:pin:space";
    let space_is_pinned = persisted_frame_names.contains(workspace_pin_name);
    let space_pin_label = if space_is_pinned { "W-" } else { "W+" };
    let space_pin_button =
        ui.add(toolbar_button(space_pin_label))
            .on_hover_text(if space_is_pinned {
                "Unpin current frame snapshot"
            } else {
                "Pin current frame snapshot"
            });
    if space_pin_button.clicked() {
        if space_is_pinned {
            if let Err(error) = graph_app.delete_workspace_layout(workspace_pin_name) {
                log::warn!("Failed to unpin frame snapshot '{workspace_pin_name}': {error}");
            }
        } else {
            graph_app.request_save_frame_snapshot_named(workspace_pin_name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState, ToolPaneRef};
    use egui_tiles::Tiles;

    #[test]
    fn projection_is_graph_only_when_tree_has_only_graph_panes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_sidebar_graph_only", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::GraphOnly);
        assert!(!projection.visible());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn projection_becomes_visible_for_tool_or_node_panes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let node_key = app.add_node_and_sync(
            "https://example.com".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let tool = tiles.insert_pane(TileKind::Tool(ToolPaneRef::new(ToolPaneState::Settings)));
        let root = tiles.insert_tab_tile(vec![graph, node, tool]);
        let tree = Tree::new("workbench_sidebar_visible", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::WorkbenchActive);
        assert!(projection.visible());
        assert_eq!(projection.pane_entries.len(), 3);
    }

    #[test]
    fn projection_preserves_split_and_tab_hierarchy() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let left_tabs = tiles.insert_tab_tile(vec![graph, left]);
        let right_tabs = tiles.insert_tab_tile(vec![right]);
        let root = tiles.insert_horizontal_tile(vec![left_tabs, right_tabs]);
        let tree = Tree::new("workbench_sidebar_hierarchy", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let root = projection.tree_root.as_ref().expect("hierarchy root");
        match root {
            WorkbenchChromeNode::Split { children, .. } => {
                assert_eq!(children.len(), 2);
                match &children[0] {
                    WorkbenchChromeNode::Tabs { children, .. } => {
                        assert_eq!(children.len(), 2);
                        assert!(matches!(children[0], WorkbenchChromeNode::Pane(_)));
                        assert!(matches!(children[1], WorkbenchChromeNode::Pane(_)));
                    }
                    other => panic!("expected left child tabs, got {other:?}"),
                }
                match &children[1] {
                    WorkbenchChromeNode::Tabs { children, .. } => {
                        assert_eq!(children.len(), 1);
                        assert!(matches!(children[0], WorkbenchChromeNode::Pane(_)));
                    }
                    other => panic!("expected right child tabs, got {other:?}"),
                }
            }
            other => panic!("expected split root, got {other:?}"),
        }
    }

    #[test]
    fn projection_includes_arrangement_navigator_groups() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let left_tabs = tiles.insert_tab_tile(vec![graph, left]);
        let right_tabs = tiles.insert_tab_tile(vec![right]);
        let root = tiles.insert_horizontal_tile(vec![left_tabs, right_tabs]);
        let tree = Tree::new("workbench_sidebar_navigator_groups", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-alpha", &tree);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.navigator_groups.len(), 1);
        assert_eq!(projection.navigator_groups[0].sub_kind, ArrangementSubKind::FrameMember);
        assert_eq!(projection.navigator_groups[0].members.len(), 3);
        assert!(projection.navigator_groups[0].title.contains("workspace-alpha"));
    }
}
