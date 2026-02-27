/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command palette panel — keyboard-first, `ActionRegistry`-backed.
//!
//! Content is populated via [`super::action_registry::list_actions_for_context`]
//! rather than a hardcoded enum.  The radial menu reuses [`execute_action`]
//! for its own dispatch, ensuring both surfaces share a single execution path.

use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, PendingConnectedOpenScope, PendingTileOpenMode,
};
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::render::action_registry::{
    ActionCategory, ActionContext, ActionId, InputMode, list_actions_for_context,
};
use egui::{Key, Window};
use std::time::{SystemTime, UNIX_EPOCH};

/// Render the command palette panel.
///
/// Content is driven by [`list_actions_for_context`]; no hardcoded action
/// enum exists in this module.
pub fn render_command_palette_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) {
    if !app.workspace.show_command_palette {
        return;
    }

    let mut open = app.workspace.show_command_palette;
    let mut intents = Vec::new();
    let mut should_close = false;

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.workspace.selected_nodes.is_empty();

    let action_context = ActionContext {
        target_node: source_context,
        pair_context,
        any_selected,
        focused_pane_available: focused_pane_node.is_some(),
        input_mode: InputMode::MouseKeyboard,
        view_id: crate::app::GraphViewId::new(),
    };
    let actions = list_actions_for_context(&action_context);

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        should_close = true;
    }

    Window::new("Edge Commands")
        .open(&mut open)
        .default_width(320.0)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Selection-driven graph commands");
            ui.add_space(6.0);

            // Render actions grouped by category, using ActionRegistry content.
            let categories = [
                ActionCategory::Edge,
                ActionCategory::Node,
                ActionCategory::Graph,
                ActionCategory::Persistence,
            ];

            let mut first_category = true;
            for category in categories {
                let cat_actions: Vec<_> = actions
                    .iter()
                    .filter(|e| e.id.category() == category)
                    .collect();
                if cat_actions.is_empty() {
                    continue;
                }
                if !first_category {
                    ui.separator();
                }
                first_category = false;
                for entry in &cat_actions {
                    if ui
                        .add_enabled(entry.enabled, egui::Button::new(entry.id.label()))
                        .clicked()
                    {
                        execute_action(
                            app,
                            entry.id,
                            pair_context,
                            source_context,
                            &mut intents,
                            focused_pane_node,
                        );
                        should_close = true;
                    }
                }
            }

            ui.separator();
            if ui.button("Close").clicked() {
                should_close = true;
            }
            ui.add_space(6.0);
            ui.small("Keyboard: G, Shift+G, Alt+G, I, U");
        });

    app.workspace.show_command_palette = open && !should_close;
    super::apply_ui_intents_with_checkpoint(app, intents);
}

/// Dispatch an [`ActionId`] to the appropriate [`GraphIntent`]s or app call.
///
/// This is the single dispatch function shared by both the command palette
/// and the radial menu, eliminating the duplicate execution paths that
/// existed when each surface had its own hardcoded `match` arm set.
pub(super) fn execute_action(
    app: &mut GraphBrowserApp,
    action_id: ActionId,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    intents: &mut Vec<GraphIntent>,
    focused_pane_node: Option<NodeKey>,
) {
    let open_target = source_context.or_else(|| app.workspace.selected_nodes.primary());

    match action_id {
        ActionId::NodeNew => intents.push(GraphIntent::CreateNodeNearCenter),
        ActionId::NodeNewAsTab => intents.push(GraphIntent::CreateNodeNearCenterAndOpen {
            mode: PendingTileOpenMode::Tab,
        }),
        ActionId::NodePinToggle => {
            if app.workspace.selected_nodes.iter().copied().all(|key| {
                app.workspace
                    .graph
                    .get_node(key)
                    .is_some_and(|node| node.is_pinned)
            }) {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::UnpinSelected,
                });
            } else {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::PinSelected,
                });
            }
        }
        ActionId::NodePinSelected => intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::PinSelected,
        }),
        ActionId::NodeUnpinSelected => intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::UnpinSelected,
        }),
        ActionId::NodeDelete => intents.push(GraphIntent::RemoveSelectedNodes),
        ActionId::NodeChooseFrame => {
            if let Some(key) = open_target
                && !app.frames_for_node_key(key).is_empty()
            {
                app.request_choose_frame_picker(key);
            }
        }
        ActionId::NodeAddToFrame => {
            if let Some(key) = open_target {
                app.request_add_node_to_frame_picker(key);
            }
        }
        ActionId::NodeAddConnectedToFrame => {
            if let Some(key) = open_target {
                app.request_add_connected_to_frame_picker(key);
            }
        }
        ActionId::NodeOpenFrame => {
            if let Some(key) = open_target {
                intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            }
        }
        ActionId::NodeOpenNeighbors => {
            if let Some(key) = open_target {
                app.request_open_connected_from(
                    key,
                    PendingTileOpenMode::Tab,
                    PendingConnectedOpenScope::Neighbors,
                );
            }
        }
        ActionId::NodeOpenConnected => {
            if let Some(key) = open_target {
                app.request_open_connected_from(
                    key,
                    PendingTileOpenMode::Tab,
                    PendingConnectedOpenScope::Connected,
                );
            }
        }
        ActionId::NodeOpenSplit => {
            if let Some(key) = open_target {
                app.request_open_node_tile_mode(key, PendingTileOpenMode::SplitHorizontal);
            }
        }
        ActionId::NodeDetachToSplit => {
            if let Some(focused) = focused_pane_node {
                app.request_detach_node_to_split(focused);
            }
        }
        ActionId::NodeMoveToActivePane => {
            if let Some(key) = open_target {
                intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            }
        }
        ActionId::NodeCopyUrl => {
            if let Some(key) = open_target {
                app.request_copy_node_url(key);
            }
        }
        ActionId::NodeCopyTitle => {
            if let Some(key) = open_target {
                app.request_copy_node_title(key);
            }
        }
        ActionId::EdgeConnectPair => {
            if let Some((from, to)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::ConnectPair { from, to },
                });
            }
        }
        ActionId::EdgeConnectBoth => {
            if let Some((a, b)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::ConnectBothDirectionsPair { a, b },
                });
            }
        }
        ActionId::EdgeRemoveUser => {
            if let Some((a, b)) = pair_context {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::RemoveUserEdgePair { a, b },
                });
            }
        }
        ActionId::GraphFit => intents.push(GraphIntent::RequestFitToScreen),
        ActionId::GraphTogglePhysics => intents.push(GraphIntent::TogglePhysics),
        ActionId::GraphPhysicsConfig => intents.push(GraphIntent::OpenSettingsUrl {
            url: "graphshell://settings/physics".to_string(),
        }),
        ActionId::GraphCommandPalette => intents.push(GraphIntent::ToggleCommandPalette),
        ActionId::PersistUndo => intents.push(GraphIntent::Undo),
        ActionId::PersistRedo => intents.push(GraphIntent::Redo),
        ActionId::PersistSaveSnapshot => app.request_save_frame_snapshot(),
        ActionId::PersistRestoreSession => {
            app.request_restore_frame_snapshot_named(
                GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME.to_string(),
            );
        }
        ActionId::PersistSaveGraph => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            app.request_save_graph_snapshot_named(format!("radial-graph-{now}"));
        }
        ActionId::PersistRestoreLatestGraph => app.request_restore_graph_snapshot_latest(),
        ActionId::PersistOpenHub => intents.push(GraphIntent::OpenToolPane {
            kind: ToolPaneState::Settings,
        }),
    }
}
