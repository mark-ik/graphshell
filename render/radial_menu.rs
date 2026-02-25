/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Radial command menu rendering and dispatch.

use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, PendingConnectedOpenScope, PendingTileOpenMode,
    SelectionUpdateMode,
};
use crate::graph::NodeKey;
use egui::{Color32, Key, Stroke, Window};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{apply_ui_intents_with_checkpoint, resolve_pair_command_context, resolve_source_node_context};

pub fn render_radial_command_menu(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) {
    if !app.workspace.show_radial_menu {
        return;
    }

    let pair_context = resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.workspace.selected_nodes.is_empty();
    let source_context = resolve_source_node_context(app, hovered_node, focused_pane_node);
    let mut intents = Vec::new();
    let mut should_close = false;
    let center_id = egui::Id::new("radial_menu_center");
    let node_context_group_state_id = egui::Id::new("node_context_kbd_group");
    let node_context_command_state_id = egui::Id::new("node_context_kbd_command");
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let center = ctx
        .data_mut(|d| d.get_persisted::<egui::Pos2>(center_id))
        .or(pointer)
        .unwrap_or(egui::pos2(320.0, 220.0));
    ctx.data_mut(|d| d.insert_persisted(center_id, center));

    if app.pending_node_context_target().is_some() {
        let mut group_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(node_context_group_state_id))
            .unwrap_or(0);
        let mut command_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(node_context_command_state_id))
            .unwrap_or(0);
        group_idx %= NodeContextGroup::ALL.len();

        let mut group_changed = false;
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            group_idx = (group_idx + NodeContextGroup::ALL.len() - 1) % NodeContextGroup::ALL.len();
            group_changed = true;
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            group_idx = (group_idx + 1) % NodeContextGroup::ALL.len();
            group_changed = true;
        }

        let keyboard_group = NodeContextGroup::ALL[group_idx];
        let keyboard_commands = node_context_commands(keyboard_group);
        let close_idx = keyboard_commands.len();
        if group_changed {
            command_idx = 0;
        }
        if command_idx >= close_idx {
            command_idx = 0;
        }
        let keyboard_slot_count = keyboard_commands.len();
        if keyboard_slot_count > 0 && ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            command_idx = (command_idx + keyboard_slot_count - 1) % keyboard_slot_count;
        }
        if keyboard_slot_count > 0 && ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            command_idx = (command_idx + 1) % keyboard_slot_count;
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(cmd) = keyboard_commands.get(command_idx).copied()
                && is_command_enabled(cmd, pair_context, any_selected, source_context)
            {
                execute_radial_command(
                    app,
                    cmd,
                    pair_context,
                    any_selected,
                    source_context,
                    &mut intents,
                );
                should_close = true;
            }
        }
        ctx.data_mut(|d| {
            d.insert_persisted(node_context_group_state_id, group_idx);
            d.insert_persisted(node_context_command_state_id, command_idx);
        });

        let window_response = Window::new("Node Context")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(center + egui::vec2(12.0, 12.0))
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (idx, group) in NodeContextGroup::ALL.iter().enumerate() {
                        let heading = if idx == group_idx {
                            format!("[{}]", group.label())
                        } else {
                            group.label().to_string()
                        };
                        ui.menu_button(heading, |ui| {
                            for cmd in node_context_commands(*group) {
                                let enabled = is_command_enabled(
                                    *cmd,
                                    pair_context,
                                    any_selected,
                                    source_context,
                                );
                                if ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::new(context_menu_label(*cmd)),
                                    )
                                    .clicked()
                                {
                                    execute_radial_command(
                                        app,
                                        *cmd,
                                        pair_context,
                                        any_selected,
                                        source_context,
                                        &mut intents,
                                    );
                                    should_close = true;
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                ui.separator();
                ui.small("Keyboard: <- -> groups, Up/Down actions, Enter run");
                ui.small("Esc or click outside to close");
                let keyboard_group = NodeContextGroup::ALL[group_idx];
                let keyboard_commands = node_context_commands(keyboard_group);
                if let Some(current_cmd) = keyboard_commands.get(command_idx).copied() {
                    ui.small(format!(
                        "Focus: {} / {}",
                        keyboard_group.label(),
                        context_menu_label(current_cmd)
                    ));
                }
                for (idx, cmd) in keyboard_commands.iter().enumerate() {
                    let enabled =
                        is_command_enabled(*cmd, pair_context, any_selected, source_context);
                    let label = if idx == command_idx {
                        format!("> {}", context_menu_label(*cmd))
                    } else {
                        context_menu_label(*cmd).to_string()
                    };
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        command_idx = idx;
                        execute_radial_command(
                            app,
                            *cmd,
                            pair_context,
                            any_selected,
                            source_context,
                            &mut intents,
                        );
                        should_close = true;
                    }
                }
                ctx.data_mut(|d| d.insert_persisted(node_context_command_state_id, command_idx));
            });
        if let Some(response) = window_response {
            let clicked_outside = ctx.input(|i| {
                i.pointer.primary_clicked()
                    && i.pointer
                        .latest_pos()
                        .is_some_and(|pos| !response.response.rect.contains(pos))
            });
            if clicked_outside {
                intents.push(GraphIntent::UpdateSelection {
                    keys: Vec::new(),
                    mode: SelectionUpdateMode::Replace,
                });
                should_close = true;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }
    } else {
        let mut hovered_domain = None;
        let mut hovered_command = None;
        if let Some(pos) = pointer {
            let delta = pos - center;
            let r = delta.length();
            if r > 40.0 {
                let angle = delta.y.atan2(delta.x);
                hovered_domain = Some(domain_from_angle(angle));
                if r > 120.0
                    && let Some(domain) = hovered_domain
                {
                    hovered_command = nearest_command_for_pointer(
                        domain,
                        center,
                        pos,
                        pair_context,
                        any_selected,
                        source_context,
                    );
                }
            }
        }

        let mut clicked_command = None;
        if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            clicked_command = hovered_command;
            should_close = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }

        egui::Area::new("radial_command_menu".into())
            .fixed_pos(center - egui::vec2(220.0, 220.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(440.0, 440.0));
                let painter = ui.painter();
                painter.circle_filled(center, 36.0, Color32::from_rgb(28, 32, 36));
                painter.circle_stroke(
                    center,
                    36.0,
                    Stroke::new(2.0, Color32::from_rgb(90, 110, 125)),
                );
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "Cmd",
                    egui::FontId::proportional(16.0),
                    Color32::from_rgb(210, 230, 245),
                );

                for domain in RadialDomain::ALL {
                    let base = domain_anchor(center, domain, 92.0);
                    let color = if Some(domain) == hovered_domain {
                        Color32::from_rgb(70, 130, 170)
                    } else {
                        Color32::from_rgb(50, 66, 80)
                    };
                    painter.circle_filled(base, 26.0, color);
                    painter.text(
                        base,
                        egui::Align2::CENTER_CENTER,
                        domain.label(),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                if let Some(domain) = hovered_domain {
                    let cmds = commands_for_domain(domain);
                    for (idx, cmd) in cmds.iter().enumerate() {
                        let enabled =
                            is_command_enabled(*cmd, pair_context, any_selected, source_context);
                        let anchor = command_anchor(center, domain, idx, cmds.len());
                        let color = if Some(*cmd) == hovered_command {
                            Color32::from_rgb(80, 170, 215)
                        } else if enabled {
                            Color32::from_rgb(64, 82, 98)
                        } else {
                            Color32::from_rgb(42, 48, 54)
                        };
                        painter.circle_filled(anchor, 22.0, color);
                        painter.text(
                            anchor,
                            egui::Align2::CENTER_CENTER,
                            cmd.label(),
                            egui::FontId::proportional(10.0),
                            if enabled {
                                Color32::from_rgb(230, 240, 248)
                            } else {
                                Color32::from_rgb(120, 125, 130)
                            },
                        );
                    }
                }
            });

        if let Some(cmd) = clicked_command {
            execute_radial_command(
                app,
                cmd,
                pair_context,
                any_selected,
                source_context,
                &mut intents,
            );
        }
    }

    app.workspace.show_radial_menu = !should_close;
    if !app.workspace.show_radial_menu {
        app.set_pending_node_context_target(None);
        ctx.data_mut(|d| {
            d.remove::<egui::Pos2>(center_id);
            d.remove::<usize>(node_context_group_state_id);
            d.remove::<usize>(node_context_command_state_id);
        });
    }
    apply_ui_intents_with_checkpoint(app, intents);
}

fn context_menu_label(command: RadialCommand) -> &'static str {
    match command {
        RadialCommand::NodeOpenWorkspace => "Open via Workspace Route",
        RadialCommand::NodeChooseWorkspace => "Choose Workspace...",
        RadialCommand::NodeAddToWorkspace => "Add To Workspace...",
        RadialCommand::NodeAddConnectedToWorkspace => "Add Connected To Workspace...",
        RadialCommand::NodeOpenNeighbors => "Open with Neighbors",
        RadialCommand::NodeOpenConnected => "Open with Connected",
        RadialCommand::NodeOpenSplit => "Open Split",
        RadialCommand::NodePinToggle => "Toggle Pin",
        RadialCommand::NodeDelete => "Delete Selected",
        RadialCommand::EdgeConnectPair => "Connect Pair",
        RadialCommand::EdgeConnectBoth => "Connect Both Directions",
        RadialCommand::EdgeRemoveUser => "Remove User Edge",
        _ => command.label(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RadialDomain {
    Node,
    Edge,
    Graph,
    Persistence,
}

impl RadialDomain {
    const ALL: [Self; 4] = [Self::Node, Self::Edge, Self::Graph, Self::Persistence];

    fn label(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Edge => "Edge",
            Self::Graph => "Graph",
            Self::Persistence => "Persist",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeContextGroup {
    Workspace,
    Edge,
    Node,
}

impl NodeContextGroup {
    const ALL: [Self; 3] = [Self::Workspace, Self::Edge, Self::Node];

    fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Edge => "Edge",
            Self::Node => "Node",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RadialCommand {
    NodeNew,
    NodePinToggle,
    NodeDelete,
    NodeChooseWorkspace,
    NodeAddToWorkspace,
    NodeAddConnectedToWorkspace,
    NodeOpenWorkspace,
    NodeOpenNeighbors,
    NodeOpenConnected,
    NodeOpenSplit,
    NodeMoveToActivePane,
    NodeCopyUrl,
    NodeCopyTitle,
    EdgeConnectPair,
    EdgeConnectBoth,
    EdgeRemoveUser,
    GraphFit,
    GraphTogglePhysics,
    GraphPhysicsConfig,
    GraphCommandPalette,
    PersistUndo,
    PersistRedo,
    PersistSaveSnapshot,
    PersistRestoreSession,
    PersistSaveGraph,
    PersistRestoreLatestGraph,
    PersistOpenHub,
}

impl RadialCommand {
    fn label(self) -> &'static str {
        match self {
            Self::NodeNew => "New",
            Self::NodePinToggle => "Pin",
            Self::NodeDelete => "Delete",
            Self::NodeChooseWorkspace => "Choose WS",
            Self::NodeAddToWorkspace => "Add WS",
            Self::NodeAddConnectedToWorkspace => "Add Conn WS",
            Self::NodeOpenWorkspace => "Workspace",
            Self::NodeOpenNeighbors => "Neighbors",
            Self::NodeOpenConnected => "Connected",
            Self::NodeOpenSplit => "Split",
            Self::NodeMoveToActivePane => "Move",
            Self::NodeCopyUrl => "Copy URL",
            Self::NodeCopyTitle => "Copy Title",
            Self::EdgeConnectPair => "Pair",
            Self::EdgeConnectBoth => "Both",
            Self::EdgeRemoveUser => "Remove",
            Self::GraphFit => "Fit",
            Self::GraphTogglePhysics => "Physics",
            Self::GraphPhysicsConfig => "Config",
            Self::GraphCommandPalette => "Cmd",
            Self::PersistUndo => "Undo",
            Self::PersistRedo => "Redo",
            Self::PersistSaveSnapshot => "Save W",
            Self::PersistRestoreSession => "Restore W",
            Self::PersistSaveGraph => "Save G",
            Self::PersistRestoreLatestGraph => "Latest G",
            Self::PersistOpenHub => "Hub",
        }
    }
}

fn node_context_commands(group: NodeContextGroup) -> &'static [RadialCommand] {
    match group {
        NodeContextGroup::Workspace => &[
            RadialCommand::NodeOpenWorkspace,
            RadialCommand::NodeChooseWorkspace,
            RadialCommand::NodeAddToWorkspace,
            RadialCommand::NodeAddConnectedToWorkspace,
            RadialCommand::NodeOpenNeighbors,
            RadialCommand::NodeOpenConnected,
        ],
        NodeContextGroup::Edge => &[
            RadialCommand::EdgeConnectPair,
            RadialCommand::EdgeConnectBoth,
            RadialCommand::EdgeRemoveUser,
        ],
        NodeContextGroup::Node => &[
            RadialCommand::NodeOpenSplit,
            RadialCommand::NodePinToggle,
            RadialCommand::NodeDelete,
            RadialCommand::NodeCopyUrl,
            RadialCommand::NodeCopyTitle,
        ],
    }
}

fn commands_for_domain(domain: RadialDomain) -> &'static [RadialCommand] {
    match domain {
        RadialDomain::Node => &[
            RadialCommand::NodeNew,
            RadialCommand::NodePinToggle,
            RadialCommand::NodeDelete,
            RadialCommand::NodeChooseWorkspace,
            RadialCommand::NodeAddToWorkspace,
            RadialCommand::NodeAddConnectedToWorkspace,
            RadialCommand::NodeOpenWorkspace,
            RadialCommand::NodeOpenNeighbors,
            RadialCommand::NodeOpenConnected,
            RadialCommand::NodeOpenSplit,
            RadialCommand::NodeMoveToActivePane,
            RadialCommand::NodeCopyUrl,
            RadialCommand::NodeCopyTitle,
        ],
        RadialDomain::Edge => &[
            RadialCommand::EdgeConnectPair,
            RadialCommand::EdgeConnectBoth,
            RadialCommand::EdgeRemoveUser,
        ],
        RadialDomain::Graph => &[
            RadialCommand::GraphFit,
            RadialCommand::GraphTogglePhysics,
            RadialCommand::GraphPhysicsConfig,
            RadialCommand::GraphCommandPalette,
        ],
        RadialDomain::Persistence => &[
            RadialCommand::PersistUndo,
            RadialCommand::PersistRedo,
            RadialCommand::PersistSaveSnapshot,
            RadialCommand::PersistRestoreSession,
            RadialCommand::PersistSaveGraph,
            RadialCommand::PersistRestoreLatestGraph,
            RadialCommand::PersistOpenHub,
        ],
    }
}

fn is_command_enabled(
    command: RadialCommand,
    pair_context: Option<(NodeKey, NodeKey)>,
    any_selected: bool,
    source_context: Option<NodeKey>,
) -> bool {
    match command {
        RadialCommand::NodePinToggle
        | RadialCommand::NodeDelete
        | RadialCommand::NodeChooseWorkspace
        | RadialCommand::NodeAddToWorkspace
        | RadialCommand::NodeAddConnectedToWorkspace
        | RadialCommand::NodeOpenWorkspace
        | RadialCommand::NodeOpenNeighbors
        | RadialCommand::NodeOpenConnected
        | RadialCommand::NodeOpenSplit
        | RadialCommand::NodeMoveToActivePane
        | RadialCommand::NodeCopyUrl
        | RadialCommand::NodeCopyTitle => any_selected || source_context.is_some(),
        RadialCommand::EdgeConnectPair
        | RadialCommand::EdgeConnectBoth
        | RadialCommand::EdgeRemoveUser => pair_context.is_some(),
        _ => true,
    }
}

fn execute_radial_command(
    app: &mut GraphBrowserApp,
    command: RadialCommand,
    pair_context: Option<(NodeKey, NodeKey)>,
    any_selected: bool,
    source_context: Option<NodeKey>,
    intents: &mut Vec<GraphIntent>,
) {
    if !is_command_enabled(command, pair_context, any_selected, source_context) {
        return;
    }

    let open_target = source_context.or_else(|| app.workspace.selected_nodes.primary());

    match command {
        RadialCommand::NodeNew => intents.push(GraphIntent::CreateNodeNearCenter),
        RadialCommand::NodePinToggle => {
            if app
                .workspace
                .selected_nodes
                .iter()
                .copied()
                .all(|key| app.workspace.graph.get_node(key).is_some_and(|node| node.is_pinned))
            {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::UnpinSelected,
                });
            } else {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::PinSelected,
                });
            }
        },
        RadialCommand::NodeDelete => intents.push(GraphIntent::RemoveSelectedNodes),
        RadialCommand::NodeChooseWorkspace => {
            if let Some(key) = open_target
                && !app.workspaces_for_node_key(key).is_empty()
            {
                app.request_choose_workspace_picker(key);
            }
        },
        RadialCommand::NodeAddToWorkspace => {
            if let Some(key) = open_target {
                app.request_add_node_to_workspace_picker(key);
            }
        },
        RadialCommand::NodeAddConnectedToWorkspace => {
            if let Some(key) = open_target {
                app.request_add_connected_to_workspace_picker(key);
            }
        },
        RadialCommand::NodeOpenWorkspace => {
            if let Some(key) = open_target {
                intents.push(GraphIntent::OpenNodeWorkspaceRouted {
                    key,
                    prefer_workspace: None,
                });
            }
        },
        RadialCommand::NodeOpenNeighbors => {
            if let Some(key) = open_target {
                app.request_open_connected_from(
                    key,
                    PendingTileOpenMode::Tab,
                    PendingConnectedOpenScope::Neighbors,
                );
            }
        },
        RadialCommand::NodeOpenConnected => {
            if let Some(key) = open_target {
                app.request_open_connected_from(
                    key,
                    PendingTileOpenMode::Tab,
                    PendingConnectedOpenScope::Connected,
                );
            }
        },
        RadialCommand::NodeOpenSplit => {
            if let Some(key) = open_target {
                app.request_open_node_tile_mode(key, PendingTileOpenMode::SplitHorizontal);
            }
        },
        RadialCommand::NodeMoveToActivePane => {
            if let Some(key) = open_target {
                intents.push(GraphIntent::OpenNodeWorkspaceRouted {
                    key,
                    prefer_workspace: None,
                });
            }
        },
        RadialCommand::NodeCopyUrl => {
            if let Some(key) = open_target {
                app.request_copy_node_url(key);
            }
        },
        RadialCommand::NodeCopyTitle => {
            if let Some(key) = open_target {
                app.request_copy_node_title(key);
            }
        },
        RadialCommand::EdgeConnectPair => {
            if let Some((from, to)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::ConnectPair { from, to },
                });
            }
        },
        RadialCommand::EdgeConnectBoth => {
            if let Some((a, b)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::ConnectBothDirectionsPair { a, b },
                });
            }
        },
        RadialCommand::EdgeRemoveUser => {
            if let Some((a, b)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::RemoveUserEdgePair { a, b },
                });
            }
        },
        RadialCommand::GraphFit => intents.push(GraphIntent::RequestFitToScreen),
        RadialCommand::GraphTogglePhysics => intents.push(GraphIntent::TogglePhysics),
        RadialCommand::GraphPhysicsConfig => intents.push(GraphIntent::TogglePhysicsPanel),
        RadialCommand::GraphCommandPalette => intents.push(GraphIntent::ToggleCommandPalette),
        RadialCommand::PersistUndo => intents.push(GraphIntent::Undo),
        RadialCommand::PersistRedo => intents.push(GraphIntent::Redo),
        RadialCommand::PersistSaveSnapshot => app.request_save_workspace_snapshot(),
        RadialCommand::PersistRestoreSession => {
            app.request_restore_workspace_snapshot_named(
                GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME.to_string(),
            );
        },
        RadialCommand::PersistSaveGraph => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            app.request_save_graph_snapshot_named(format!("radial-graph-{now}"));
        },
        RadialCommand::PersistRestoreLatestGraph => app.request_restore_graph_snapshot_latest(),
        RadialCommand::PersistOpenHub => intents.push(GraphIntent::TogglePersistencePanel),
    }
}

fn domain_from_angle(angle: f32) -> RadialDomain {
    let mut best = RadialDomain::Node;
    let mut best_dist = f32::MAX;
    for domain in RadialDomain::ALL {
        let target = domain_angle(domain);
        let mut d = (angle - target).abs();
        if d > std::f32::consts::PI {
            d = 2.0 * std::f32::consts::PI - d;
        }
        if d < best_dist {
            best_dist = d;
            best = domain;
        }
    }
    best
}

fn domain_angle(domain: RadialDomain) -> f32 {
    match domain {
        RadialDomain::Node => -std::f32::consts::FRAC_PI_2,
        RadialDomain::Edge => -0.25,
        RadialDomain::Graph => 1.45,
        RadialDomain::Persistence => 2.7,
    }
}

fn domain_anchor(center: egui::Pos2, domain: RadialDomain, radius: f32) -> egui::Pos2 {
    let a = domain_angle(domain);
    center + egui::vec2(a.cos() * radius, a.sin() * radius)
}

fn command_anchor(center: egui::Pos2, domain: RadialDomain, idx: usize, len: usize) -> egui::Pos2 {
    let base = domain_angle(domain);
    let spread = 0.8_f32;
    let t = if len <= 1 {
        0.0
    } else {
        idx as f32 / (len.saturating_sub(1) as f32) - 0.5
    };
    let angle = base + t * spread;
    center + egui::vec2(angle.cos() * 165.0, angle.sin() * 165.0)
}

fn nearest_command_for_pointer(
    domain: RadialDomain,
    center: egui::Pos2,
    pointer: egui::Pos2,
    pair_context: Option<(NodeKey, NodeKey)>,
    any_selected: bool,
    source_context: Option<NodeKey>,
) -> Option<RadialCommand> {
    let cmds = commands_for_domain(domain);
    let mut best: Option<(f32, RadialCommand)> = None;
    for (idx, cmd) in cmds.iter().enumerate() {
        if !is_command_enabled(*cmd, pair_context, any_selected, source_context) {
            continue;
        }
        let anchor = command_anchor(center, domain, idx, cmds.len());
        let d = (pointer - anchor).length_sq();
        match best {
            Some((best_d, _)) if d >= best_d => {},
            _ => best = Some((d, *cmd)),
        }
    }
    best.map(|(_, cmd)| cmd)
}
