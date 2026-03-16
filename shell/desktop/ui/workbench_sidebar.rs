/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use egui::{RichText, SidePanel};
use egui_tiles::{Container, LinearDir, Tile, TileId, Tree};
use uuid::Uuid;

use crate::app::{GraphBrowserApp, GraphViewId, WorkbenchIntent};
use crate::graph::{ArrangementSubKind, NodeKey};
use crate::services::persistence::types::LogEntry;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarNavAction};
use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops;
use crate::util::VersoAddress;

/// Maximum sidebar width as a fraction of screen width. Clamped so the sidebar
/// never exceeds one-fifth of the available screen, with an absolute floor of 180 px.
const SIDEBAR_MAX_FRACTION: f32 = 0.20;
const SIDEBAR_MAX_FLOOR: f32 = 180.0;
const SIDEBAR_LABEL_MAX_CHARS: usize = 40;
const NAVIGATOR_RECENT_LIMIT: usize = 12;

fn compact_sidebar_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= SIDEBAR_LABEL_MAX_CHARS {
        return trimmed.to_string();
    }
    let shortened: String = trimmed
        .chars()
        .take(SIDEBAR_LABEL_MAX_CHARS.saturating_sub(1))
        .collect();
    format!("{shortened}…")
}

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
    pub(crate) arrangement_memberships: Vec<String>,
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
    pub(crate) section: WorkbenchNavigatorSection,
    pub(crate) title: String,
    pub(crate) members: Vec<WorkbenchNavigatorMember>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchNavigatorSection {
    Arrangement(ArrangementSubKind),
    Recent,
    Unrelated,
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
        let arrangement_memberships = pane_arrangement_memberships(graph_app);
        let mut saved_frame_names = graph_app
            .list_workspace_layout_names()
            .into_iter()
            .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
            .collect::<Vec<_>>();
        for frame_name in arrangement_memberships
            .values()
            .flatten()
            .filter_map(|membership| membership.strip_prefix("Frame: "))
        {
            if !saved_frame_names.iter().any(|existing| existing == frame_name) {
                saved_frame_names.push(frame_name.to_string());
            }
        }
        saved_frame_names.sort();
        saved_frame_names.dedup();
        let pane_entries = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(pane_entry_for_tile(
                    graph_app,
                    kind,
                    active_pane,
                    &arrangement_memberships,
                )),
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
        let navigator_groups = navigator_groups(graph_app, &arrangement_memberships);
        let tree_root = tiles_tree
            .root()
            .and_then(|root| {
                build_tree_node(
                    graph_app,
                    tiles_tree,
                    root,
                    active_pane,
                    &arrangement_memberships,
                )
            });
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

fn navigator_groups(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> Vec<WorkbenchNavigatorGroup> {
    let mut groups = arrangement_navigator_groups(graph_app);
    let recent_keys = recent_navigator_members(graph_app, arrangement_memberships)
        .iter()
        .map(|member| member.node_key)
        .collect::<HashSet<_>>();
    groups.extend(unrelated_navigator_group(
        graph_app,
        arrangement_memberships,
        &recent_keys,
    ));
    groups.extend(recent_navigator_group(graph_app, arrangement_memberships));
    groups
}

fn arrangement_navigator_groups(graph_app: &GraphBrowserApp) -> Vec<WorkbenchNavigatorGroup> {
    graph_app
        .arrangement_projection_groups()
        .into_iter()
        .map(|group| WorkbenchNavigatorGroup {
            section: WorkbenchNavigatorSection::Arrangement(group.sub_kind),
            title: match group.sub_kind {
                ArrangementSubKind::FrameMember => format!("Frame: {}", group.title),
                ArrangementSubKind::TileGroup => format!("Tile Group: {}", group.title),
                ArrangementSubKind::SplitPair => format!("Split Pair: {}", group.title),
            },
            members: group
                .member_keys
                .into_iter()
                .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
                .collect(),
        })
        .collect()
}

fn recent_navigator_group(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> Option<WorkbenchNavigatorGroup> {
    let members = recent_navigator_members(graph_app, arrangement_memberships);
    if members.is_empty() {
        return None;
    }
    Some(WorkbenchNavigatorGroup {
        section: WorkbenchNavigatorSection::Recent,
        title: "Recent".to_string(),
        members,
    })
}

fn recent_navigator_members(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> Vec<WorkbenchNavigatorMember> {
    let mut recent: HashMap<NodeKey, (u64, usize)> = HashMap::new();
    for entry in graph_app.history_manager_timeline_entries(NAVIGATOR_RECENT_LIMIT * 4) {
        let LogEntry::AppendTraversal {
            to_node_id,
            timestamp_ms,
            ..
        } = entry
        else {
            continue;
        };
        let Ok(node_id) = Uuid::parse_str(&to_node_id) else {
            continue;
        };
        let Some(node_key) = graph_app.domain_graph().get_node_key_by_id(node_id) else {
            continue;
        };
        let Some(node) = graph_app.domain_graph().get_node(node_key) else {
            continue;
        };
        if arrangement_memberships.contains_key(&node_key) || is_internal_surface_node(node) {
            continue;
        }
        let stats = recent.entry(node_key).or_insert((timestamp_ms, 0));
        stats.0 = stats.0.max(timestamp_ms);
        stats.1 += 1;
    }

    let mut rows = recent.into_iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_stats), (right_key, right_stats)| {
        right_stats
            .0
            .cmp(&left_stats.0)
            .then_with(|| right_stats.1.cmp(&left_stats.1))
            .then_with(|| navigator_member_sort_key(graph_app, *left_key).cmp(&navigator_member_sort_key(graph_app, *right_key)))
    });
    rows.truncate(NAVIGATOR_RECENT_LIMIT);
    rows.into_iter()
        .filter_map(|(node_key, (_timestamp_ms, visit_count))| {
            let suffix = format!("({visit_count} visit{})", if visit_count == 1 { "" } else { "s" });
            navigator_member_for_node(graph_app, node_key, Some(suffix))
        })
        .collect()
}

fn unrelated_navigator_group(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
    recent_keys: &HashSet<NodeKey>,
) -> Option<WorkbenchNavigatorGroup> {
    let mut members = graph_app
        .domain_graph()
        .nodes()
        .filter(|(node_key, node)| {
            !arrangement_memberships.contains_key(node_key)
                && !recent_keys.contains(node_key)
                && !is_internal_surface_node(node)
        })
        .map(|(node_key, _)| node_key)
        .collect::<Vec<_>>();
    members.sort_by(|left, right| navigator_member_sort_key(graph_app, *left).cmp(&navigator_member_sort_key(graph_app, *right)));
    if members.is_empty() {
        return None;
    }
    Some(WorkbenchNavigatorGroup {
        section: WorkbenchNavigatorSection::Unrelated,
        title: "Unrelated".to_string(),
        members: members
            .into_iter()
            .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
            .collect(),
    })
}

fn navigator_member_for_node(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    suffix: Option<String>,
) -> Option<WorkbenchNavigatorMember> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let mut title = node_primary_label(node);
    if let Some(suffix) = suffix {
        title.push(' ');
        title.push_str(&suffix);
    }
    Some(WorkbenchNavigatorMember {
        node_key,
        title,
        is_selected: graph_app.focused_selection().contains(&node_key),
    })
}

fn navigator_member_sort_key(app: &GraphBrowserApp, key: NodeKey) -> (String, usize) {
    let label = app
        .domain_graph()
        .get_node(key)
        .map(node_primary_label)
        .unwrap_or_else(|| format!("Node {}", key.index()));
    (label, key.index())
}

fn node_primary_label(node: &crate::graph::Node) -> String {
    let title = node.title.trim();
    if !title.is_empty() {
        title.to_string()
    } else if !node.url.trim().is_empty() {
        node.url.clone()
    } else {
        "Untitled node".to_string()
    }
}

fn is_internal_surface_node(node: &crate::graph::Node) -> bool {
    matches!(
        VersoAddress::parse(&node.url),
        Some(
            VersoAddress::Frame(_)
                | VersoAddress::TileGroup(_)
                | VersoAddress::View(_)
                | VersoAddress::Tool { .. }
                | VersoAddress::Other { .. }
        )
    )
}

fn pane_entry_for_tile(
    graph_app: &GraphBrowserApp,
    kind: &TileKind,
    active_pane: Option<PaneId>,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> WorkbenchPaneEntry {
    match kind {
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref)) => WorkbenchPaneEntry {
            pane_id: graph_ref.pane_id,
            kind: WorkbenchPaneKind::Graph {
                view_id: graph_ref.graph_view_id,
            },
            title: graph_view_title(graph_app, graph_ref.graph_view_id),
            subtitle: Some("Graph".to_string()),
            arrangement_memberships: Vec::new(),
            is_active: active_pane == Some(graph_ref.pane_id),
            closable: false,
        },
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state)) => {
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
                arrangement_memberships: arrangement_memberships
                    .get(&state.node)
                    .cloned()
                    .unwrap_or_default(),
                is_active: active_pane == Some(state.pane_id),
                closable: true,
            }
        }
        #[cfg(feature = "diagnostics")]
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool)) => WorkbenchPaneEntry {
            pane_id: tool.pane_id,
            kind: WorkbenchPaneKind::Tool {
                kind: tool.kind.clone(),
            },
            title: tool.title().to_string(),
            subtitle: Some("Tool".to_string()),
            arrangement_memberships: Vec::new(),
            is_active: active_pane == Some(tool.pane_id),
            closable: true,
        },
        TileKind::Graph(graph_ref) => WorkbenchPaneEntry {
            pane_id: graph_ref.pane_id,
            kind: WorkbenchPaneKind::Graph {
                view_id: graph_ref.graph_view_id,
            },
            title: graph_view_title(graph_app, graph_ref.graph_view_id),
            subtitle: Some("Graph".to_string()),
            arrangement_memberships: Vec::new(),
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
                arrangement_memberships: arrangement_memberships
                    .get(&state.node)
                    .cloned()
                    .unwrap_or_default(),
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
            arrangement_memberships: Vec::new(),
            is_active: active_pane == Some(tool.pane_id),
            closable: true,
        },
    }
}

fn pane_arrangement_memberships(graph_app: &GraphBrowserApp) -> HashMap<NodeKey, Vec<String>> {
    let mut index: HashMap<NodeKey, Vec<String>> = HashMap::new();
    for group in graph_app.arrangement_projection_groups() {
        let label = match group.sub_kind {
            ArrangementSubKind::FrameMember => format!("Frame: {}", group.title),
            ArrangementSubKind::TileGroup => format!("Tile Group: {}", group.title),
            ArrangementSubKind::SplitPair => format!("Split Pair: {}", group.title),
        };
        for node_key in group.member_keys {
            index.entry(node_key).or_default().push(label.clone());
        }
    }
    for memberships in index.values_mut() {
        memberships.sort();
        memberships.dedup();
    }
    index
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
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> Option<WorkbenchChromeNode> {
    let tile = tiles_tree.tiles.get(tile_id)?;
    match tile {
        Tile::Pane(kind) => Some(WorkbenchChromeNode::Pane(pane_entry_for_tile(
            graph_app,
            kind,
            active_pane,
            arrangement_memberships,
        ))),
        Tile::Container(Container::Tabs(tabs)) => Some(WorkbenchChromeNode::Tabs {
            tile_id,
            label: format!("Tab Group ({})", tabs.children.len()),
            children: tabs
                .children
                .iter()
                .filter_map(|child| {
                    build_tree_node(
                        graph_app,
                        tiles_tree,
                        *child,
                        active_pane,
                        arrangement_memberships,
                    )
                })
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
                    .filter_map(|child| {
                        build_tree_node(
                            graph_app,
                            tiles_tree,
                            *child,
                            active_pane,
                            arrangement_memberships,
                        )
                    })
                    .collect(),
            })
        }
        Tile::Container(Container::Grid(grid)) => Some(WorkbenchChromeNode::Grid {
            tile_id,
            label: format!("Grid ({})", grid.children().count()),
            children: grid
                .children()
                .filter_map(|child| {
                    build_tree_node(
                        graph_app,
                        tiles_tree,
                        *child,
                        active_pane,
                        arrangement_memberships,
                    )
                })
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
    let sidebar_max_width = (ctx.content_rect().width() * SIDEBAR_MAX_FRACTION).max(SIDEBAR_MAX_FLOOR);
    let sidebar_default_width = (sidebar_max_width * 0.75).clamp(SIDEBAR_MAX_FLOOR, sidebar_max_width);

    SidePanel::right("workbench_sidebar")
        .resizable(true)
        .default_width(sidebar_default_width)
        .min_width(SIDEBAR_MAX_FLOOR)
        .max_width(sidebar_max_width)
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
                    if ui.small_button("Navigator").clicked() {
                        post_panel_action =
                            Some(SidebarAction::OpenTool(ToolPaneState::navigator_surface()));
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
                        .default_open(!matches!(group.section, WorkbenchNavigatorSection::Recent));
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

                ui.label(RichText::new("Open Panes").small().strong());
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_pane_list")
                    .show(ui, |ui| {
                        for entry in &projection.pane_entries {
                            render_pane_row(ui, entry, &mut post_panel_action);
                        }
                        ui.add_space(4.0);
                        egui::CollapsingHeader::new(
                            RichText::new("Tile Structure").small().weak(),
                        )
                        .id_salt("workbench_sidebar_tile_structure")
                        .default_open(false)
                        .show(ui, |ui| {
                            if let Some(root) = projection.tree_root.as_ref() {
                                render_tree_node(ui, root, 0, &mut post_panel_action);
                            }
                        });
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
                let compact_title = compact_sidebar_text(&entry.title);
                let text = if entry.is_active {
                    RichText::new(&compact_title).strong()
                } else {
                    RichText::new(&compact_title)
                };
                let response = ui
                    .selectable_label(entry.is_active, text)
                    .on_hover_text(&entry.title);
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
                let compact_subtitle = compact_sidebar_text(subtitle);
                ui.add_space((depth as f32) * 10.0 + 2.0);
                ui.label(RichText::new(compact_subtitle).small().weak())
                    .on_hover_text(subtitle);
            }
            if !entry.arrangement_memberships.is_empty() {
                ui.add_space((depth as f32) * 10.0 + 2.0);
                ui.label(
                    RichText::new(format!(
                        "Memberships: {}",
                        entry.arrangement_memberships.join(", ")
                    ))
                    .small()
                    .weak(),
                );
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
            let compact_label = compact_sidebar_text(label);
            let header =
                egui::CollapsingHeader::new(RichText::new(compact_label).small().strong())
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

fn render_pane_row(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    action: &mut Option<SidebarAction>,
) {
    ui.horizontal(|ui| {
        let compact_title = compact_sidebar_text(&entry.title);
        let text = if entry.is_active {
            RichText::new(&compact_title).strong()
        } else {
            RichText::new(&compact_title)
        };
        let response = ui.selectable_label(entry.is_active, text);
        let response = if let Some(subtitle) = &entry.subtitle {
            response.on_hover_text(subtitle)
        } else {
            response
        };
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
    ui.add_space(2.0);
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
    use crate::services::persistence::types::LogEntry;
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState, ToolPaneRef};
    use egui_tiles::Tiles;
    use uuid::Uuid;

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

        let arrangement_group = projection
            .navigator_groups
            .iter()
            .find(|group| {
                group.section == WorkbenchNavigatorSection::Arrangement(ArrangementSubKind::FrameMember)
            })
            .expect("arrangement group");
        assert_eq!(arrangement_group.members.len(), 3);
        assert!(arrangement_group.title.contains("workspace-alpha"));
    }

    #[test]
    fn projection_adds_unrelated_group_for_nodes_without_arrangement_membership() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let unrelated_key = app.add_node_and_sync(
            "https://example.com/unrelated".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(unrelated_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let tree = Tree::new("workbench_sidebar_unrelated", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let unrelated_group = projection
            .navigator_groups
            .iter()
            .find(|group| group.section == WorkbenchNavigatorSection::Unrelated)
            .expect("unrelated group");
        assert_eq!(unrelated_group.members.len(), 1);
        assert_eq!(unrelated_group.members[0].node_key, unrelated_key);
    }

    #[test]
    fn recent_navigator_members_count_visits_and_skip_arranged_nodes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let recent_key = app.add_node_and_sync(
            "https://example.com/recent".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let arranged_key = app.add_node_and_sync(
            "https://example.com/arranged".to_string(),
            euclid::default::Point2D::new(1.0, 0.0),
        );
        let recent_id = app.domain_graph().get_node(recent_key).expect("recent node").id;
        let arranged_id = app.domain_graph().get_node(arranged_key).expect("arranged node").id;

        let arrangement_memberships = HashMap::from([(arranged_key, vec!["Frame: alpha".to_string()])]);
        let entries = vec![
            LogEntry::AppendTraversal {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: recent_id.to_string(),
                timestamp_ms: 20,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::LinkClick,
            },
            LogEntry::AppendTraversal {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: recent_id.to_string(),
                timestamp_ms: 10,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::LinkClick,
            },
            LogEntry::AppendTraversal {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: arranged_id.to_string(),
                timestamp_ms: 30,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::LinkClick,
            },
        ];

        let mut recent: HashMap<NodeKey, (u64, usize)> = HashMap::new();
        for entry in entries {
            let LogEntry::AppendTraversal {
                to_node_id,
                timestamp_ms,
                ..
            } = entry else {
                continue;
            };
            let node_id = Uuid::parse_str(&to_node_id).expect("valid node uuid");
            let node_key = app
                .domain_graph()
                .get_node_key_by_id(node_id)
                .expect("node key");
            let node = app.domain_graph().get_node(node_key).expect("node");
            if arrangement_memberships.contains_key(&node_key) || is_internal_surface_node(node) {
                continue;
            }
            let stats = recent.entry(node_key).or_insert((timestamp_ms, 0));
            stats.0 = stats.0.max(timestamp_ms);
            stats.1 += 1;
        }

        let mut rows = recent.into_iter().collect::<Vec<_>>();
        rows.sort_by(|(left_key, left_stats), (right_key, right_stats)| {
            right_stats
                .0
                .cmp(&left_stats.0)
                .then_with(|| right_stats.1.cmp(&left_stats.1))
                .then_with(|| navigator_member_sort_key(&app, *left_key).cmp(&navigator_member_sort_key(&app, *right_key)))
        });
        let members = rows
            .into_iter()
            .filter_map(|(node_key, (_timestamp_ms, visit_count))| {
                navigator_member_for_node(
                    &app,
                    node_key,
                    Some(format!(
                        "({visit_count} visit{})",
                        if visit_count == 1 { "" } else { "s" }
                    )),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].node_key, recent_key);
        assert!(members[0].title.contains("2 visits"));
    }
}
