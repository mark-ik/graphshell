/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command palette panel for the graph browser.
//!
//! Provides a keyboard-first, selection-driven panel of graph commands.

use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, PendingConnectedOpenScope, PendingTileOpenMode,
};
use crate::graph::NodeKey;
use egui::{Key, Window};

use super::{
    apply_ui_intents_with_checkpoint, resolve_pair_command_context, resolve_source_node_context,
};

/// Render edge command palette panel (keyboard-first palette; radial UI can reuse this dispatch).
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
    let pair_context = resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.workspace.selected_nodes.is_empty();
    let source_context = resolve_source_node_context(app, hovered_node, focused_pane_node);

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

            if ui
                .add_enabled(
                    pair_context.is_some(),
                    egui::Button::new("Connect Source -> Target"),
                )
                .clicked()
            {
                if let Some((from, to)) = pair_context {
                    intents.push(GraphIntent::ExecuteEdgeCommand {
                        command: EdgeCommand::ConnectPair { from, to },
                    });
                    should_close = true;
                }
            }
            if ui
                .add_enabled(
                    pair_context.is_some(),
                    egui::Button::new("Connect Both Directions"),
                )
                .clicked()
            {
                if let Some((a, b)) = pair_context {
                    intents.push(GraphIntent::ExecuteEdgeCommand {
                        command: EdgeCommand::ConnectBothDirectionsPair { a, b },
                    });
                    should_close = true;
                }
            }
            if ui
                .add_enabled(
                    pair_context.is_some(),
                    egui::Button::new("Remove User Edge"),
                )
                .clicked()
            {
                if let Some((a, b)) = pair_context {
                    intents.push(GraphIntent::ExecuteEdgeCommand {
                        command: EdgeCommand::RemoveUserEdgePair { a, b },
                    });
                    should_close = true;
                }
            }
            ui.separator();
            if ui
                .add_enabled(any_selected, egui::Button::new("Pin Selected"))
                .clicked()
            {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::PinSelected,
                });
                should_close = true;
            }
            if ui
                .add_enabled(any_selected, egui::Button::new("Unpin Selected"))
                .clicked()
            {
                intents.push(GraphIntent::ExecuteEdgeCommand {
                    command: EdgeCommand::UnpinSelected,
                });
                should_close = true;
            }
            ui.separator();
            if ui.button("Toggle Physics Panel").clicked() {
                intents.push(GraphIntent::TogglePhysicsPanel);
                should_close = true;
            }
            if ui.button("Toggle Physics Simulation").clicked() {
                intents.push(GraphIntent::TogglePhysics);
                should_close = true;
            }
            if ui.button("Fit Graph to Screen").clicked() {
                intents.push(GraphIntent::RequestFitToScreen);
                should_close = true;
            }
            if ui.button("Open Persistence Hub").clicked() {
                intents.push(GraphIntent::TogglePersistencePanel);
                should_close = true;
            }
            ui.separator();
            if ui
                .add_enabled(
                    focused_pane_node.is_some(),
                    egui::Button::new("Detach Focused to Split"),
                )
                .clicked()
                && let Some(focused) = focused_pane_node
            {
                app.request_detach_node_to_split(focused);
                should_close = true;
            }
            if ui.button("Create Node").clicked() {
                intents.push(GraphIntent::CreateNodeNearCenter);
                should_close = true;
            }
            if ui.button("Create Node as Tab").clicked() {
                intents.push(GraphIntent::CreateNodeNearCenterAndOpen {
                    mode: PendingTileOpenMode::Tab,
                });
                should_close = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        source_context
                            .is_some_and(|key| !app.workspaces_for_node_key(key).is_empty()),
                        egui::Button::new("Choose Workspace..."),
                    )
                    .clicked()
                    && let Some(source) = source_context
                {
                    app.request_choose_workspace_picker(source);
                    should_close = true;
                }
                if ui
                    .add_enabled(
                        source_context.is_some(),
                        egui::Button::new("Open with Neighbors"),
                    )
                    .clicked()
                    && let Some(source) = source_context
                {
                    app.request_open_connected_from(
                        source,
                        PendingTileOpenMode::Tab,
                        PendingConnectedOpenScope::Neighbors,
                    );
                    should_close = true;
                }
                if ui
                    .add_enabled(
                        source_context.is_some(),
                        egui::Button::new("Open with Connected"),
                    )
                    .clicked()
                    && let Some(source) = source_context
                {
                    app.request_open_connected_from(
                        source,
                        PendingTileOpenMode::Tab,
                        PendingConnectedOpenScope::Connected,
                    );
                    should_close = true;
                }
            });
            ui.separator();
            if ui.button("Close").clicked() {
                should_close = true;
            }
            ui.add_space(6.0);
            ui.small("Keyboard: G, Shift+G, Alt+G, I, U");
        });

    app.workspace.show_command_palette = open && !should_close;
    apply_ui_intents_with_checkpoint(app, intents);
}
