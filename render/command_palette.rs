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
    WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::render::action_registry::{
    ActionCategory, ActionContext, ActionEntry, ActionId, InputMode, list_actions_for_context,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION;
use crate::shell::desktop::workbench::pane_model::{
    NodePaneState, PaneId, PaneViewState, ToolPaneState, ViewerId,
};
use crate::util::{GraphshellAddress, GraphshellSettingsPath};
use egui::{Key, Window};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SearchPaletteScope {
    CurrentTarget,
    ActivePane,
    ActiveGraph,
    Workbench,
}

impl SearchPaletteScope {
    const ALL: [Self; 4] = [
        Self::CurrentTarget,
        Self::ActivePane,
        Self::ActiveGraph,
        Self::Workbench,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::CurrentTarget => "Current Target",
            Self::ActivePane => "Active Pane",
            Self::ActiveGraph => "Active Graph",
            Self::Workbench => "Workbench",
        }
    }
}

fn scope_ready(scope: SearchPaletteScope, action_context: &ActionContext) -> bool {
    match scope {
        SearchPaletteScope::CurrentTarget => action_context.target_node.is_some(),
        SearchPaletteScope::ActivePane => action_context.focused_pane_available,
        SearchPaletteScope::ActiveGraph | SearchPaletteScope::Workbench => true,
    }
}

fn scope_allows_action(
    entry: &ActionEntry,
    scope: SearchPaletteScope,
    action_context: &ActionContext,
) -> bool {
    if !scope_ready(scope, action_context) {
        return false;
    }

    match scope {
        SearchPaletteScope::CurrentTarget => {
            matches!(entry.id.category(), ActionCategory::Node | ActionCategory::Edge)
        }
        SearchPaletteScope::ActivePane => !matches!(entry.id, ActionId::PersistOpenHub),
        SearchPaletteScope::ActiveGraph => {
            !matches!(entry.id.category(), ActionCategory::Persistence)
        }
        SearchPaletteScope::Workbench => true,
    }
}

fn search_matches(entry: &ActionEntry, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }
    let q = trimmed.to_ascii_lowercase();
    entry.id.label().to_ascii_lowercase().contains(&q)
        || entry.id.short_label().to_ascii_lowercase().contains(&q)
}

fn filter_actions_for_search<'a>(
    actions: &'a [ActionEntry],
    query: &str,
    scope: SearchPaletteScope,
    action_context: &ActionContext,
) -> Vec<&'a ActionEntry> {
    actions
        .iter()
        .filter(|entry| scope_allows_action(entry, scope, action_context))
        .filter(|entry| search_matches(entry, query))
        .collect()
}

fn disabled_action_reason(
    action_id: ActionId,
    action_context: &ActionContext,
) -> Option<&'static str> {
    match action_id {
        ActionId::PersistUndo => {
            if !action_context.undo_available {
                Some("Undo unavailable. No prior graph mutation is available to revert.")
            } else {
                None
            }
        }
        ActionId::PersistRedo => {
            if !action_context.redo_available {
                Some("Redo unavailable. Perform an undo first to create redo history.")
            } else {
                None
            }
        }
        ActionId::EdgeConnectPair | ActionId::EdgeConnectBoth | ActionId::EdgeRemoveUser => {
            if action_context.pair_context.is_none() {
                Some("Requires exactly two nodes selected. Select a source and target node first.")
            } else {
                None
            }
        }
        ActionId::NodeDetachToSplit => {
            if !action_context.focused_pane_available {
                Some("Requires a focused node pane. Focus a node pane, then retry.")
            } else {
                None
            }
        }
        ActionId::NodePinSelected
        | ActionId::NodeUnpinSelected
        | ActionId::NodeDelete
        | ActionId::NodeChooseFrame
        | ActionId::NodeAddToFrame
        | ActionId::NodeAddConnectedToFrame
        | ActionId::NodeOpenFrame
        | ActionId::NodeOpenNeighbors
        | ActionId::NodeOpenConnected
        | ActionId::NodeOpenSplit
        | ActionId::NodeMoveToActivePane
        | ActionId::NodeCopyUrl
        | ActionId::NodeCopyTitle => {
            if !action_context.any_selected && action_context.target_node.is_none() {
                Some("Requires a selected or targeted node. Select a node first.")
            } else {
                None
            }
        }
        ActionId::NodeRenderWry => {
            if !action_context.wry_override_allowed {
                Some(
                    "Wry backend unavailable. Enable build/runtime support and Settings -> Viewer Backends.",
                )
            } else if !action_context.any_selected && action_context.target_node.is_none() {
                Some("Requires a selected or targeted node. Select a node first.")
            } else {
                None
            }
        }
        ActionId::NodeRenderAuto | ActionId::NodeRenderWebView => {
            if !action_context.any_selected && action_context.target_node.is_none() {
                Some("Requires a selected or targeted node. Select a node first.")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn empty_graph_message(node_count: usize) -> Option<&'static str> {
    if node_count == 0 {
        Some("Graph is empty. Create your first node to unlock node and edge actions.")
    } else {
        None
    }
}

fn render_action_entry_button(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    entry: &crate::render::action_registry::ActionEntry,
    action_context: &ActionContext,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    intents: &mut Vec<GraphIntent>,
    focused_pane_node: Option<NodeKey>,
    should_close: &mut bool,
) {
    let mut response = ui.add_enabled(entry.enabled, egui::Button::new(entry.id.label()));
    if !entry.enabled
        && let Some(reason) = disabled_action_reason(entry.id, action_context)
    {
        response = response.on_hover_text(reason);
    }
    if response.clicked() {
        execute_action(
            app,
            entry.id,
            pair_context,
            source_context,
            intents,
            focused_pane_node,
        );
        *should_close = true;
    }
}

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
    let was_open = app.workspace.show_command_palette;
    if !was_open {
        return;
    }

    let mut open = app.workspace.show_command_palette;
    let mut intents = Vec::new();
    let mut should_close = false;

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let focused_selection = app.focused_selection().clone();
    let any_selected = !focused_selection.is_empty();
    let graph_node_count = app.workspace.graph.node_count();

    let action_context = ActionContext {
        target_node: source_context,
        pair_context,
        any_selected,
        focused_pane_available: focused_pane_node.is_some(),
        undo_available: app.undo_stack_len() > 0,
        redo_available: app.redo_stack_len() > 0,
        input_mode: InputMode::MouseKeyboard,
        view_id: app
            .workspace
            .focused_view
            .unwrap_or_else(crate::app::GraphViewId::new),
        wry_override_allowed: cfg!(feature = "wry")
            && app.wry_enabled()
            && crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry"),
    };
    let actions = list_actions_for_context(&action_context);
    let contextual_mode = app.pending_node_context_target().is_some();
    let search_query_id = egui::Id::new("command_palette_search_query");
    let search_scope_id = egui::Id::new("command_palette_search_scope");

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        should_close = true;
    }

    Window::new("Command Palette")
        .open(&mut open)
        .default_width(320.0)
        .default_height(420.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label("Node, tile, edge, graph, and persistence commands");
                    ui.small("Delete Node(s) is graph content mutation; tile close remains a tile-tree operation.");
                    if contextual_mode {
                        ui.small("Mode: Context Palette (Tier 1 categories + Tier 2 commands)");
                    } else {
                        ui.small("Mode: Search Palette (global grouped list)");
                    }
                    if let Some(message) = empty_graph_message(graph_node_count) {
                        ui.add_space(4.0);
                        ui.small(message);
                        if ui.button("Create First Node").clicked() {
                            intents.push(GraphIntent::CreateNodeNearCenter);
                            should_close = true;
                        }
                    }
                    ui.add_space(6.0);

                    // Context Palette Mode: two-tier scaffold.
                    // Tier 1 selects a category; Tier 2 lists actions for that category.
                    let categories = [
                        ActionCategory::Node,
                        ActionCategory::Edge,
                        ActionCategory::Graph,
                        ActionCategory::Persistence,
                    ];

                    if contextual_mode {
                        let category_state_id = egui::Id::new("command_palette_tier1_category");
                        let mut selected_index = ctx
                            .data_mut(|d| d.get_persisted::<usize>(category_state_id))
                            .unwrap_or(0)
                            % categories.len();

                        ui.horizontal_wrapped(|ui| {
                            ui.label("Tier 1:");
                            for (idx, category) in categories.iter().copied().enumerate() {
                                if ui
                                    .selectable_label(selected_index == idx, category.label())
                                    .clicked()
                                {
                                    selected_index = idx;
                                }
                            }
                        });
                        ctx.data_mut(|d| d.insert_persisted(category_state_id, selected_index));

                        ui.add_space(4.0);
                        ui.small("Tier 2:");
                        let selected_category = categories[selected_index];
                        let tier2_actions: Vec<_> = actions
                            .iter()
                            .filter(|e| e.id.category() == selected_category)
                            .collect();
                        if tier2_actions.is_empty() {
                            ui.small("No actions available in this category for the current context.");
                        } else {
                            for entry in &tier2_actions {
                                render_action_entry_button(
                                    ui,
                                    app,
                                    entry,
                                    &action_context,
                                    pair_context,
                                    source_context,
                                    &mut intents,
                                    focused_pane_node,
                                    &mut should_close,
                                );
                            }
                        }
                    } else {
                        // Search Palette Mode scaffold: grouped category list.
                        let mut search_query = ctx
                            .data_mut(|d| d.get_persisted::<String>(search_query_id))
                            .unwrap_or_default();
                        let mut search_scope = ctx
                            .data_mut(|d| d.get_persisted::<SearchPaletteScope>(search_scope_id))
                            .unwrap_or(SearchPaletteScope::Workbench);

                        ui.horizontal_wrapped(|ui| {
                            ui.label("Search:");
                            ui.add(
                                egui::TextEdit::singleline(&mut search_query)
                                    .desired_width(160.0)
                                    .hint_text("Type action name..."),
                            );
                            egui::ComboBox::from_id_salt("command_palette_scope_dropdown")
                                .selected_text(search_scope.label())
                                .show_ui(ui, |ui| {
                                    for candidate in SearchPaletteScope::ALL {
                                        ui.selectable_value(
                                            &mut search_scope,
                                            candidate,
                                            candidate.label(),
                                        );
                                    }
                                });
                        });
                        ctx.data_mut(|d| {
                            d.insert_persisted(search_query_id, search_query.clone());
                            d.insert_persisted(search_scope_id, search_scope);
                        });

                        if !scope_ready(search_scope, &action_context) {
                            ui.small(
                                "Selected scope is unavailable in current context. Choose Workbench or Current Target.",
                            );
                        }

                        let filtered =
                            filter_actions_for_search(&actions, &search_query, search_scope, &action_context);

                        let categories = [
                            ActionCategory::Edge,
                            ActionCategory::Node,
                            ActionCategory::Graph,
                            ActionCategory::Persistence,
                        ];
                        let mut first_category = true;
                        for category in categories {
                            let cat_actions: Vec<_> = filtered
                                .iter()
                                .filter(|e| e.id.category() == category)
                                .copied()
                                .collect();
                            if cat_actions.is_empty() {
                                continue;
                            }
                            if !first_category {
                                ui.separator();
                            }
                            first_category = false;
                            for entry in &cat_actions {
                                render_action_entry_button(
                                    ui,
                                    app,
                                    entry,
                                    &action_context,
                                    pair_context,
                                    source_context,
                                    &mut intents,
                                    focused_pane_node,
                                    &mut should_close,
                                );
                            }
                        }

                        if filtered.is_empty() {
                            ui.small("No actions match the current search/scope filters.");
                        }
                    }

                    ui.separator();
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                    ui.add_space(6.0);
                    ui.small("Keyboard: G, Shift+G, Alt+G, I, U");
                });
        });

    app.workspace.show_command_palette = open && !should_close;
    if app.workspace.show_command_palette != was_open {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
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
    let focused_selection = app.focused_selection().clone();
    let open_target = source_context.or_else(|| focused_selection.primary());

    match action_id {
        ActionId::NodeNew => intents.push(GraphIntent::CreateNodeNearCenter),
        ActionId::NodeNewAsTab => intents.push(GraphIntent::CreateNodeNearCenterAndOpen {
            mode: PendingTileOpenMode::Tab,
        }),
        ActionId::NodePinToggle => {
            if focused_selection.iter().copied().all(|key| {
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
        ActionId::NodeRenderAuto | ActionId::NodeRenderWebView | ActionId::NodeRenderWry => {
            if let Some(key) = open_target {
                let viewer_id_override = match action_id {
                    ActionId::NodeRenderAuto => None,
                    ActionId::NodeRenderWebView => Some(ViewerId::new("viewer:webview")),
                    ActionId::NodeRenderWry => Some(ViewerId::new("viewer:wry")),
                    _ => unreachable!("non-render action reached render action branch"),
                };
                let mut state = NodePaneState::for_node(key);
                state.viewer_id_override = viewer_id_override;
                app.enqueue_workbench_intent(WorkbenchIntent::SetPaneView {
                    pane: PaneId::new(),
                    view: PaneViewState::Node(state),
                });
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
        ActionId::GraphPhysicsConfig => {
            app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                url: GraphshellAddress::settings(GraphshellSettingsPath::Physics).to_string(),
            });
        }
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
        ActionId::PersistOpenHub => app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
            kind: ToolPaneState::Settings,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;

    fn default_action_context() -> ActionContext {
        ActionContext {
            target_node: None,
            pair_context: None,
            any_selected: false,
            focused_pane_available: false,
            undo_available: false,
            redo_available: false,
            input_mode: InputMode::MouseKeyboard,
            view_id: GraphViewId::new(),
            wry_override_allowed: false,
        }
    }

    #[test]
    fn disabled_node_delete_exposes_precondition_reason() {
        let reason = disabled_action_reason(ActionId::NodeDelete, &default_action_context());
        assert_eq!(
            reason,
            Some("Requires a selected or targeted node. Select a node first.")
        );
    }

    #[test]
    fn empty_graph_message_present_when_graph_has_no_nodes() {
        assert_eq!(
            empty_graph_message(0),
            Some("Graph is empty. Create your first node to unlock node and edge actions.")
        );
        assert_eq!(empty_graph_message(1), None);
    }

    #[test]
    fn all_disabled_actions_expose_textual_precondition_reason_in_default_context() {
        let context = default_action_context();
        let entries = list_actions_for_context(&context);

        for entry in entries.into_iter().filter(|entry| !entry.enabled) {
            let reason = disabled_action_reason(entry.id, &context);
            assert!(
                reason.is_some(),
                "disabled action {:?} should expose a textual precondition reason",
                entry.id
            );
        }
    }

    #[test]
    fn disabled_action_reasons_use_actionable_text_not_color_cues() {
        let context = default_action_context();
        let entries = list_actions_for_context(&context);
        let disallowed_color_terms = [" color", "colour", "color ", "colour "];

        for entry in entries.into_iter().filter(|entry| !entry.enabled) {
            let reason = disabled_action_reason(entry.id, &context)
                .expect("disabled action should expose reason text");
            let reason_lower = reason.to_ascii_lowercase();
            assert!(
                reason.contains("Requires")
                    || reason.contains("unavailable")
                    || reason.contains("Perform"),
                "reason should provide actionable precondition guidance: {reason}"
            );
            assert!(
                !disallowed_color_terms
                    .iter()
                    .any(|term| reason_lower.contains(term)),
                "reason should not rely on color terms: {reason}"
            );
        }
    }

    #[test]
    fn search_matches_uses_case_insensitive_label_matching() {
        let entry = ActionEntry {
            id: ActionId::NodeDelete,
            enabled: true,
        };
        assert!(search_matches(&entry, "delete"));
        assert!(search_matches(&entry, "NoDe"));
        assert!(!search_matches(&entry, "physics"));
    }

    #[test]
    fn scope_ready_requires_target_for_current_target_scope() {
        let mut context = default_action_context();
        assert!(!scope_ready(SearchPaletteScope::CurrentTarget, &context));
        context.target_node = Some(NodeKey::new(7));
        assert!(scope_ready(SearchPaletteScope::CurrentTarget, &context));
    }

    #[test]
    fn filter_actions_for_search_respects_scope_and_query() {
        let context = ActionContext {
            target_node: Some(NodeKey::new(11)),
            ..default_action_context()
        };
        let actions = vec![
            ActionEntry {
                id: ActionId::NodeDelete,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::GraphTogglePhysics,
                enabled: true,
            },
        ];

        let target_only = filter_actions_for_search(
            &actions,
            "delete",
            SearchPaletteScope::CurrentTarget,
            &context,
        );
        assert_eq!(target_only.len(), 1);
        assert_eq!(target_only[0].id, ActionId::NodeDelete);

        let graph_scope =
            filter_actions_for_search(&actions, "physics", SearchPaletteScope::ActiveGraph, &context);
        assert_eq!(graph_scope.len(), 1);
        assert_eq!(graph_scope[0].id, ActionId::GraphTogglePhysics);
    }
}
