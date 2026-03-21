/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command palette / context palette list surface — `ActionRegistry`-backed.
//!
//! Content is populated via [`super::action_registry::list_actions_for_context`]
//! rather than a hardcoded enum.
//!
//! Terminology:
//! - "Command Palette" = the search-first global list surface.
//! - "Context Palette" = the contextual list mode of the same authority.
//! - "Radial Palette" = the radial contextual presentation over the same action backend.
//!
//! The radial palette reuses [`execute_action`] for its own dispatch, ensuring both
//! surfaces share a single execution path.

use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, GraphMutation, PendingConnectedOpenScope,
    PendingTileOpenMode, ViewAction, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::render::action_registry::{
    ActionCategory, ActionContext, ActionEntry, ActionId, InputMode, category_from_persisted_name,
    category_persisted_name, default_category_order, list_actions_for_context,
    rank_categories_for_context,
};
use crate::render::command_profile::{
    load_category_recency, load_pinned_categories, record_recent_category, toggle_category_pin,
};
use crate::shell::desktop::runtime::registries::{
    self, action as runtime_action, workflow as runtime_workflow,
};
#[cfg(test)]
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::pane_model::{PaneId, ViewerId};
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::{Area, Frame, Key, Order, Window};
use std::time::{SystemTime, UNIX_EPOCH};

const RADIAL_FALLBACK_NOTICE_KEY: &str = "radial_mode_fallback_notice";

fn palette_window_title(contextual_mode: bool) -> &'static str {
    if contextual_mode {
        "Context Palette"
    } else {
        "Command Palette"
    }
}

fn palette_intro_label(contextual_mode: bool) -> &'static str {
    if contextual_mode {
        "Targeted commands"
    } else {
        "Workbench commands"
    }
}

fn active_theme_tokens(
    app: &GraphBrowserApp,
) -> crate::shell::desktop::runtime::registries::theme::ThemeTokenSet {
    crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
        app.default_registry_theme_id(),
    )
    .tokens
}

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
            matches!(
                entry.id.category(),
                ActionCategory::Node | ActionCategory::Edge
            )
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
    focused_pane_id: Option<PaneId>,
    should_close: &mut bool,
) {
    let shortcuts = entry.id.shortcut_hints();
    let button = egui::Button::new(if shortcuts.is_empty() {
        entry.id.label().to_string()
    } else {
        format!("{}    {}", entry.id.label(), shortcuts.join(" / "))
    });
    let mut response = ui.add_enabled(entry.enabled, button);
    if !entry.enabled
        && let Some(reason) = disabled_action_reason(entry.id, action_context)
    {
        response = response.on_hover_text(reason);
    }
    if response.clicked() {
        record_recent_category(ui.ctx(), entry.id.category());
        execute_action(
            app,
            entry.id,
            pair_context,
            source_context,
            intents,
            focused_pane_node,
            focused_pane_id,
        );
        *should_close = true;
    }
}

/// Render the list-based command surface.
///
/// Content is driven by [`list_actions_for_context`]; no hardcoded action
/// enum exists in this module.
pub fn render_command_palette_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<PaneId>,
) {
    let was_open = app.workspace.chrome_ui.show_command_palette
        || app.workspace.chrome_ui.show_context_palette;
    if !was_open {
        return;
    }

    let mut open = app.workspace.chrome_ui.show_command_palette;
    let mut intents = Vec::new();
    let mut should_close = false;
    let theme_tokens = active_theme_tokens(app);

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let focused_selection = app.focused_selection().clone();
    let any_selected = !focused_selection.is_empty();
    let graph_node_count = app.domain_graph().node_count();

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
            .graph_runtime
            .focused_view
            .unwrap_or_else(crate::app::GraphViewId::new),
        wry_override_allowed: cfg!(feature = "wry")
            && app.wry_enabled()
            && crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry"),
    };
    let actions = list_actions_for_context(&action_context);
    let categories_present: Vec<ActionCategory> = default_category_order()
        .into_iter()
        .filter(|category| actions.iter().any(|entry| entry.id.category() == *category))
        .collect();
    let ordered_categories = rank_categories_for_context(
        &categories_present,
        &action_context,
        &load_category_recency(ctx),
        &load_pinned_categories(ctx),
    );
    let contextual_mode =
        app.workspace.chrome_ui.show_context_palette || app.pending_node_context_target().is_some();
    let contextual_anchor = app.workspace.chrome_ui.context_palette_anchor;
    let search_query_id = egui::Id::new("command_palette_search_query");
    let search_scope_id = egui::Id::new("command_palette_search_scope");

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        should_close = true;
    }

    let window_title = palette_window_title(contextual_mode);

    let mut render_palette_contents = |ui: &mut egui::Ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                    let fallback_notice_id = egui::Id::new(RADIAL_FALLBACK_NOTICE_KEY);
                    let fallback_notice = ctx
                        .data_mut(|d| d.get_persisted::<bool>(fallback_notice_id))
                        .unwrap_or(false);
                    if fallback_notice {
                        ui.colored_label(
                            theme_tokens.command_notice,
                            "Radial palette constrained; opened context palette for reliable selection.",
                        );
                        ctx.data_mut(|d| d.remove::<bool>(fallback_notice_id));
                    }
                    if contextual_mode {
                        ui.label(palette_intro_label(true));
                        if let Some(target) = source_context.or(hovered_node) {
                            if let Some(node) = app.domain_graph().get_node(target) {
                                let label = if node.title.trim().is_empty() {
                                    node.url.as_str()
                                } else {
                                    node.title.as_str()
                                };
                                ui.small(format!("Target: {label}"));
                            }
                        }
                        ui.small("Same command model as F2, scoped to the current target.");
                    } else {
                        ui.label(palette_intro_label(false));
                        ui.small("Node, edge, graph, pane, and persistence actions in one surface.");
                        ui.small("Delete Node(s) is graph content mutation; tile close remains a tile-tree operation.");
                        ui.small("Mode: Search Interaction (global grouped list)");
                    }
                    if let Some(message) = empty_graph_message(graph_node_count) {
                        ui.add_space(4.0);
                        ui.small(message);
                        if ui.button("Create First Node").clicked() {
                            intents.push(GraphMutation::CreateNodeNearCenter.into());
                            should_close = true;
                        }
                    }
                    ui.add_space(6.0);

                    // Context Palette Mode: two-tier scaffold.
                    // Tier 1 selects a category; Tier 2 lists actions for that category.
                    if contextual_mode {
                        let category_state_id = egui::Id::new("command_palette_tier1_category");
                        let mut selected_category = ctx
                            .data_mut(|d| d.get_persisted::<String>(category_state_id))
                            .and_then(|raw| category_from_persisted_name(&raw))
                            .filter(|category| ordered_categories.contains(category))
                            .unwrap_or_else(|| {
                                ordered_categories
                                    .first()
                                    .copied()
                                    .unwrap_or(ActionCategory::Graph)
                            });

                        ui.horizontal_wrapped(|ui| {
                            ui.label("Tier 1:");
                            for category in ordered_categories.iter().copied() {
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(
                                            selected_category == category,
                                            category.label(),
                                        )
                                        .clicked()
                                    {
                                        selected_category = category;
                                    }
                                    let pinned = load_pinned_categories(ui.ctx()).contains(&category);
                                    let pin_label = if pinned { "Unpin" } else { "Pin" };
                                    if ui.small_button(pin_label).clicked() {
                                        toggle_category_pin(ui.ctx(), category);
                                    }
                                });
                            }
                        });
                        ctx.data_mut(|d| {
                            d.insert_persisted(
                                category_state_id,
                                category_persisted_name(selected_category).to_string(),
                            )
                        });

                        ui.add_space(4.0);
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
                                    focused_pane_id,
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

                        let mut first_category = true;
                        for category in ordered_categories.iter().copied() {
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
                                    focused_pane_id,
                                    &mut should_close,
                                );
                            }
                        }

                        if filtered.is_empty() {
                            ui.small("No actions match the current search/scope filters.");
                        }
                    }

                    ui.separator();
                    if contextual_mode {
                        ui.horizontal(|ui| {
                            if ui.small_button("Close").clicked() {
                                should_close = true;
                            }
                        });
                    } else {
                        if ui.button("Close").clicked() {
                            should_close = true;
                        }
                        ui.add_space(6.0);
                        ui.small("Keyboard: G, Shift+G, Alt+G, I, U");
                    }
                });
    };

    if contextual_mode {
        let anchor = contextual_anchor
            .map(|[x, y]| egui::pos2(x, y))
            .or_else(|| ctx.input(|i| i.pointer.latest_pos()))
            .unwrap_or_else(|| egui::pos2(120.0, 120.0));
        let inner = Area::new("context_palette_popup".into())
            .order(Order::Foreground)
            .fixed_pos(anchor + egui::vec2(10.0, 10.0))
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.set_max_width(320.0);
                Frame::popup(ui.style()).show(ui, |ui| {
                    render_palette_contents(ui);
                });
            });

        if ctx.input(|i| i.pointer.primary_clicked() || i.pointer.secondary_clicked()) {
            if let Some(pointer) = ctx.input(|i| i.pointer.latest_pos())
                && !inner.response.rect.contains(pointer)
            {
                should_close = true;
            }
        }
    } else {
        Window::new(window_title)
            .open(&mut open)
            .default_width(320.0)
            .default_height(420.0)
            .resizable(true)
            .show(ctx, |ui| {
                render_palette_contents(ui);
            });
    }

    if (!contextual_mode && !open) || should_close {
        if contextual_mode {
            app.close_command_palette();
        } else {
            app.enqueue_workbench_intent(crate::app::WorkbenchIntent::ToggleCommandPalette);
        }
    }
    super::apply_ui_intents_with_checkpoint(app, intents);
}

/// Dispatch an [`ActionId`] to the appropriate [`GraphIntent`]s or app call.
///
/// This is the single dispatch function shared by the list-based command
/// surface and the radial menu, eliminating the duplicate execution paths
/// that existed when each surface had its own hardcoded `match` arm set.
pub(crate) fn execute_action(
    app: &mut GraphBrowserApp,
    action_id: ActionId,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    intents: &mut Vec<GraphIntent>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<PaneId>,
) {
    let focused_selection = app.focused_selection().clone();
    let open_target = source_context.or_else(|| focused_selection.primary());

    match action_id {
        ActionId::NodeNew => intents.push(GraphMutation::CreateNodeNearCenter.into()),
        ActionId::NodeNewAsTab => intents.push(
            GraphMutation::CreateNodeNearCenterAndOpen {
                mode: PendingTileOpenMode::Tab,
            }
            .into(),
        ),
        ActionId::NodePinToggle => {
            if focused_selection.iter().copied().all(|key| {
                app.workspace
                    .domain
                    .graph
                    .get_node(key)
                    .is_some_and(|node| node.is_pinned)
            }) {
                intents.push(
                    GraphMutation::ExecuteEdgeCommand {
                        command: EdgeCommand::UnpinSelected,
                    }
                    .into(),
                );
            } else {
                intents.push(
                    GraphMutation::ExecuteEdgeCommand {
                        command: EdgeCommand::PinSelected,
                    }
                    .into(),
                );
            }
        }
        ActionId::NodePinSelected => intents.push(
            GraphMutation::ExecuteEdgeCommand {
                command: EdgeCommand::PinSelected,
            }
            .into(),
        ),
        ActionId::NodeUnpinSelected => intents.push(
            GraphMutation::ExecuteEdgeCommand {
                command: EdgeCommand::UnpinSelected,
            }
            .into(),
        ),
        ActionId::NodeDelete => intents.push(GraphMutation::RemoveSelectedNodes.into()),
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
                app.enqueue_workbench_intent(WorkbenchIntent::SwapViewerBackend {
                    pane: focused_pane_id
                        .filter(|_| focused_pane_node == Some(key))
                        .unwrap_or_else(PaneId::new),
                    node: key,
                    viewer_id_override,
                });
            }
        }
        ActionId::EdgeConnectPair => {
            if let Some((from, to)) = pair_context {
                intents.push(
                    GraphMutation::ExecuteEdgeCommand {
                        command: EdgeCommand::ConnectPair { from, to },
                    }
                    .into(),
                );
            }
        }
        ActionId::EdgeConnectBoth => {
            if let Some((a, b)) = pair_context {
                intents.push(
                    GraphMutation::ExecuteEdgeCommand {
                        command: EdgeCommand::ConnectBothDirectionsPair { a, b },
                    }
                    .into(),
                );
            }
        }
        ActionId::EdgeRemoveUser => {
            if let Some((a, b)) = pair_context {
                intents.push(
                    GraphMutation::ExecuteEdgeCommand {
                        command: EdgeCommand::RemoveUserEdgePair { a, b },
                    }
                    .into(),
                );
            }
        }
        ActionId::GraphFit => intents.push(ViewAction::RequestFitToScreen.into()),
        ActionId::GraphCycleFocusRegion => {
            app.enqueue_workbench_intent(WorkbenchIntent::CycleFocusRegion);
        }
        ActionId::GraphTogglePhysics => intents.push(GraphIntent::TogglePhysics),
        ActionId::GraphPhysicsConfig => {
            registries::phase3_publish_settings_route_requested(
                &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string(),
                true,
            );
        }
        ActionId::GraphCommandPalette => {
            app.enqueue_workbench_intent(WorkbenchIntent::OpenCommandPalette);
        }
        ActionId::GraphRadialMenu => intents.push(GraphIntent::ToggleRadialMenu),
        ActionId::WorkbenchGroupSelectedTiles => {
            app.enqueue_workbench_intent(WorkbenchIntent::GroupSelectedTiles);
        }
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
        ActionId::PersistOpenHub => registries::phase3_publish_settings_route_requested(
            &VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
            true,
        ),
        ActionId::WorkbenchOpenSettingsPane => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_SETTINGS_PANE_OPEN,
                runtime_action::ActionPayload::WorkbenchSettingsPaneOpen,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_WORKBENCH_SETTINGS_PANE_OPEN,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::WorkbenchOpenSettingsOverlay => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN,
                runtime_action::ActionPayload::WorkbenchSettingsOverlayOpen,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::PersistOpenHistoryManager => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_OPEN_HISTORY_MANAGER,
                runtime_action::ActionPayload::WorkbenchOpenHistoryManager,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_WORKBENCH_OPEN_HISTORY_MANAGER,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::WorkbenchActivateWorkflowDefault => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
                runtime_action::ActionPayload::WorkbenchActivateWorkflow {
                    workflow_id: runtime_workflow::WORKFLOW_DEFAULT.to_string(),
                },
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute workflow '{}': {}",
                    runtime_workflow::WORKFLOW_DEFAULT,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::WorkbenchActivateWorkflowResearch => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
                runtime_action::ActionPayload::WorkbenchActivateWorkflow {
                    workflow_id: runtime_workflow::WORKFLOW_RESEARCH.to_string(),
                },
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute workflow '{}': {}",
                    runtime_workflow::WORKFLOW_RESEARCH,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::WorkbenchActivateWorkflowReading => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
                runtime_action::ActionPayload::WorkbenchActivateWorkflow {
                    workflow_id: runtime_workflow::WORKFLOW_READING.to_string(),
                },
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute workflow '{}': {}",
                    runtime_workflow::WORKFLOW_READING,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::NodeWarmSelect => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_GRAPH_SELECTION_WARM_SELECT,
                runtime_action::ActionPayload::GraphDeselectAll,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_GRAPH_SELECTION_WARM_SELECT,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
        ActionId::NodeRemoveFromGraphlet => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET,
                runtime_action::ActionPayload::GraphDeselectAll,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET,
                    error.reason
                );
                Vec::new()
            });
            intents.extend(runtime_intents);
        }
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
    fn palette_titles_use_unified_command_surface_naming() {
        assert_eq!(palette_window_title(false), "Command Palette");
        assert_eq!(palette_window_title(true), "Context Palette");
        assert_ne!(palette_window_title(false), "Edge Commands");
    }

    #[test]
    fn palette_intro_copy_distinguishes_global_and_contextual_modes() {
        assert_eq!(palette_intro_label(false), "Workbench commands");
        assert_eq!(palette_intro_label(true), "Targeted commands");
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
    fn node_render_action_targets_focused_pane_when_node_matches() {
        let mut app = GraphBrowserApp::new_for_testing();
        let target_node = app.add_node_and_sync(
            "https://focused-pane.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let pane_id = PaneId::new();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::NodeRenderWry,
            None,
            Some(target_node),
            &mut intents,
            Some(target_node),
            Some(pane_id),
        );

        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::SwapViewerBackend { pane, node, viewer_id_override }]
                if *pane == pane_id
                    && *node == target_node
                    && viewer_id_override.as_ref().map(|id| id.as_str()) == Some("viewer:wry")
        ));
    }

    #[test]
    fn execute_action_history_manager_routes_through_runtime_dispatch() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::PersistOpenHistoryManager,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::HistoryManager
            }]
        ));
    }

    #[test]
    fn execute_action_persistence_hub_publishes_settings_route_signal() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                    crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::SettingsRouteRequested {
                        url,
                        prefer_overlay,
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push((url.clone(), *prefer_overlay));
                }
                Ok(())
            },
        );

        execute_action(
            &mut app,
            ActionId::PersistOpenHub,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(app.take_pending_workbench_intents().is_empty());
        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &(
                            VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
                            true,
                        )
                })
        );
        assert!(registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn execute_action_graph_physics_config_publishes_settings_route_signal() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                    crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::SettingsRouteRequested {
                        url,
                        prefer_overlay,
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push((url.clone(), *prefer_overlay));
                }
                Ok(())
            },
        );

        execute_action(
            &mut app,
            ActionId::GraphPhysicsConfig,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(app.take_pending_workbench_intents().is_empty());
        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &(
                            VersoAddress::settings(GraphshellSettingsPath::Physics).to_string(),
                            true,
                        )
                })
        );
        assert!(registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn execute_action_settings_pane_routes_through_runtime_dispatch() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchOpenSettingsPane,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::Settings
            }]
        ));
    }

    #[test]
    fn execute_action_settings_overlay_publishes_settings_route_signal() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                    crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::SettingsRouteRequested {
                        url,
                        prefer_overlay,
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push((url.clone(), *prefer_overlay));
                }
                Ok(())
            },
        );

        execute_action(
            &mut app,
            ActionId::WorkbenchOpenSettingsOverlay,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(app.take_pending_workbench_intents().is_empty());
        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &(
                            VersoAddress::settings(GraphshellSettingsPath::General).to_string(),
                            true,
                        )
                })
        );
        assert!(registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn execute_action_activate_workflow_routes_through_runtime_dispatch() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchActivateWorkflowResearch,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert_eq!(
            crate::shell::desktop::runtime::registries::phase3_resolve_active_workbench_surface_profile().resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_COMPARE
        );
        assert_eq!(app.default_registry_physics_id(), Some("physics:gas"));
        assert!(app.take_pending_workbench_intents().is_empty());
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

        let graph_scope = filter_actions_for_search(
            &actions,
            "physics",
            SearchPaletteScope::ActiveGraph,
            &context,
        );
        assert_eq!(graph_scope.len(), 1);
        assert_eq!(graph_scope[0].id, ActionId::GraphTogglePhysics);
    }

    #[test]
    fn rank_categories_prioritizes_node_context_then_recency() {
        let context = ActionContext {
            target_node: Some(NodeKey::new(1)),
            ..default_action_context()
        };
        let categories = vec![
            ActionCategory::Node,
            ActionCategory::Edge,
            ActionCategory::Graph,
            ActionCategory::Persistence,
        ];
        let ordered = rank_categories_for_context(
            &categories,
            &context,
            &[ActionCategory::Persistence, ActionCategory::Graph],
            &[],
        );
        assert_eq!(ordered[0], ActionCategory::Node);
        assert_eq!(ordered[1], ActionCategory::Persistence);
    }

    #[test]
    fn toggle_pin_round_trip_preserves_pin_order() {
        let ctx = egui::Context::default();
        assert!(load_pinned_categories(&ctx).is_empty());

        toggle_category_pin(&ctx, ActionCategory::Graph);
        toggle_category_pin(&ctx, ActionCategory::Node);
        assert_eq!(
            load_pinned_categories(&ctx),
            vec![ActionCategory::Graph, ActionCategory::Node]
        );

        // Toggling a pinned category removes it without affecting others.
        toggle_category_pin(&ctx, ActionCategory::Graph);
        assert_eq!(load_pinned_categories(&ctx), vec![ActionCategory::Node]);
    }
}
