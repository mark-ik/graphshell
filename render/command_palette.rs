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

use crate::app::workbench_layout_policy::FirstUseOutcome;
use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, GraphMutation, PendingConnectedOpenScope,
    PendingTileOpenMode, SurfaceFirstUsePolicy, SurfaceHostId, UxConfigMode, ViewAction,
    WorkbenchIntent,
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
use crate::shell::desktop::ui::{toolbar::toolbar_ui::CommandBarFocusTarget, toolbar_routing};
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

fn resolve_frame_context_member(app: &GraphBrowserApp, frame_name: &str) -> Option<NodeKey> {
    app.arrangement_projection_groups()
        .into_iter()
        .find(|group| {
            group.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                && group.id == frame_name
        })
        .and_then(|group| group.member_keys.into_iter().next())
}

fn resolve_frame_context_suppressed(app: &GraphBrowserApp, frame_name: &str) -> bool {
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    app.domain_graph()
        .get_node_by_url(&frame_url)
        .and_then(|(frame_key, _)| app.domain_graph().frame_split_offer_suppressed(frame_key))
        .unwrap_or(false)
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
        SearchPaletteScope::CurrentTarget => {
            action_context.target_node.is_some() || action_context.target_frame_name.is_some()
        }
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
            ) || (action_context.target_frame_name.is_some()
                && matches!(
                    entry.id,
                    ActionId::FrameSelect
                        | ActionId::FrameOpen
                        | ActionId::FrameOpenAsSplit
                        | ActionId::FrameRename
                        | ActionId::FrameSettings
                        | ActionId::FrameSuppressSplitOffer
                        | ActionId::FrameDelete
                        | ActionId::FrameEnableSplitOffer
                ))
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
        | ActionId::NodeEditTags
        | ActionId::NodeMarkTombstone
        | ActionId::NodeChooseFrame
        | ActionId::NodeAddToFrame
        | ActionId::NodeAddConnectedToFrame
        | ActionId::NodeOpenFrame
        | ActionId::NodeOpenNeighbors
        | ActionId::NodeOpenConnected
        | ActionId::NodeOpenSplit
        | ActionId::NodeMoveToActivePane
        | ActionId::NodeWarmSelect
        | ActionId::NodeRemoveFromGraphlet
        | ActionId::NodeImportWebFinger
        | ActionId::NodeResolveNip05
        | ActionId::NodeResolveMatrix
        | ActionId::NodeResolveActivityPub
        | ActionId::NodeRefreshPersonIdentity
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
        ActionId::FrameSelect
        | ActionId::FrameOpen
        | ActionId::FrameOpenAsSplit
        | ActionId::FrameRename
        | ActionId::FrameSettings
        | ActionId::FrameSuppressSplitOffer
        | ActionId::FrameDelete
        | ActionId::FrameEnableSplitOffer => {
            if action_context.target_frame_name.is_none() {
                Some(
                    "Requires a targeted frame. Right-click a frame backdrop or frame tab chip first.",
                )
            } else if matches!(action_id, ActionId::FrameOpen | ActionId::FrameOpenAsSplit)
                && action_context.target_frame_member.is_none()
            {
                Some("Frame has no resolvable member tile to open yet.")
            } else {
                None
            }
        }
        ActionId::WorkbenchUnlockSurfaceLayout => {
            if action_context.layout_surface_target_ambiguous {
                Some(
                    "Multiple Navigator hosts are visible. Unlock layout from the specific host chrome first or enter config mode on that host.",
                )
            } else if !action_context.layout_surface_host_available {
                Some(
                    "Layout host unavailable. Open or pin a Navigator host in the current workbench context first.",
                )
            } else if action_context.layout_surface_configuring {
                Some(
                    "Surface layout is already unlocked. Use Lock Surface Layout to finish configuration.",
                )
            } else {
                None
            }
        }
        ActionId::WorkbenchLockSurfaceLayout => {
            if action_context.layout_surface_target_ambiguous {
                Some(
                    "Multiple Navigator hosts are visible. Lock layout from the host that is currently being configured.",
                )
            } else if !action_context.layout_surface_host_available {
                Some(
                    "Layout host unavailable. Open or pin a Navigator host in the current workbench context first.",
                )
            } else if !action_context.layout_surface_configuring {
                Some("Surface layout is already locked. Unlock a Navigator host first.")
            } else {
                None
            }
        }
        ActionId::WorkbenchRememberLayoutPreference => {
            if action_context.layout_surface_target_ambiguous {
                Some(
                    "Multiple Navigator hosts are visible. Remember the layout from the specific host that owns the draft.",
                )
            } else if !action_context.layout_surface_host_available {
                Some(
                    "Layout host unavailable. Open or pin a Navigator host in the current workbench context first.",
                )
            } else if !action_context.layout_surface_configuring
                && !action_context.layout_surface_has_draft
            {
                Some(
                    "No in-progress layout draft is available to remember. Unlock and adjust a Navigator host first.",
                )
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

fn layout_surface_host_label(surface_host: &SurfaceHostId) -> &'static str {
    match surface_host {
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
            "Top Navigator Host"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom) => {
            "Bottom Navigator Host"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Left) => {
            "Left Navigator Host"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Right) => {
            "Right Navigator Host"
        }
        SurfaceHostId::Role(_) => "Workbench Host",
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
        execute_action_with_layout_target(
            app,
            entry.id,
            action_context.layout_surface_target_host.clone(),
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
    let frame_context = app.pending_frame_context_target().map(str::to_string);
    let focused_selection = app.focused_selection().clone();
    let any_selected = !focused_selection.is_empty();
    let graph_node_count = app.domain_graph().node_count();
    let contextual_mode = app.workspace.chrome_ui.show_context_palette
        || app.pending_node_context_target().is_some()
        || frame_context.is_some();
    let contextual_anchor = app.workspace.chrome_ui.context_palette_anchor;
    let search_query_id = egui::Id::new("command_palette_search_query");
    let search_scope_id = egui::Id::new("command_palette_search_scope");
    let layout_host_state_id = egui::Id::new("command_palette_layout_surface_host");

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        should_close = true;
    }

    let window_title = palette_window_title(contextual_mode);

    let mut render_palette_contents = |ui: &mut egui::Ui| {
        let visible_layout_surface_hosts = app.visible_navigator_surface_hosts();
        let configuring_layout_surface_host = match &app.workspace.workbench_session.ux_config_mode
        {
            UxConfigMode::Configuring { surface_host } => Some(surface_host.clone()),
            UxConfigMode::Locked => None,
        };
        let mut selected_layout_surface_host =
            if let Some(surface_host) = configuring_layout_surface_host.clone() {
                Some(surface_host)
            } else if visible_layout_surface_hosts.len() <= 1 {
                visible_layout_surface_hosts.first().cloned()
            } else {
                ctx.data_mut(|d| d.get_persisted::<String>(layout_host_state_id))
                    .and_then(|raw| raw.parse::<SurfaceHostId>().ok())
                    .filter(|surface_host| visible_layout_surface_hosts.contains(surface_host))
                    .or_else(|| visible_layout_surface_hosts.first().cloned())
            };

        if configuring_layout_surface_host.is_none() && visible_layout_surface_hosts.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                ui.label("Layout Host:");
                egui::ComboBox::from_id_salt("command_palette_layout_host_dropdown")
                    .selected_text(
                        selected_layout_surface_host
                            .as_ref()
                            .map(layout_surface_host_label)
                            .unwrap_or("Select Navigator Host"),
                    )
                    .show_ui(ui, |ui| {
                        for surface_host in &visible_layout_surface_hosts {
                            ui.selectable_value(
                                &mut selected_layout_surface_host,
                                Some(surface_host.clone()),
                                layout_surface_host_label(surface_host),
                            );
                        }
                    });
            });
            ctx.data_mut(|d| {
                if let Some(surface_host) = &selected_layout_surface_host {
                    d.insert_persisted(layout_host_state_id, surface_host.to_string());
                }
            });
            ui.small("Layout actions apply to the selected Navigator host.");
            ui.add_space(4.0);
        }

        let action_context = ActionContext {
            target_node: source_context,
            target_frame_member: frame_context
                .as_deref()
                .and_then(|frame_name| resolve_frame_context_member(app, frame_name)),
            target_frame_split_offer_suppressed: frame_context
                .as_deref()
                .is_some_and(|frame_name| resolve_frame_context_suppressed(app, frame_name)),
            target_frame_name: frame_context.clone(),
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
                && crate::registries::infrastructure::mod_loader::runtime_has_capability(
                    "viewer:wry",
                ),
            layout_surface_host_available: selected_layout_surface_host.is_some(),
            layout_surface_target_host: selected_layout_surface_host.clone(),
            layout_surface_target_ambiguous: app.has_ambiguous_navigator_surface_host_target()
                && selected_layout_surface_host.is_none(),
            layout_surface_configuring: matches!(
                app.workspace.workbench_session.ux_config_mode,
                UxConfigMode::Configuring { .. }
            ),
            layout_surface_has_draft: selected_layout_surface_host.as_ref().is_some_and(
                |surface_host| {
                    app.workbench_layout_constraint_draft_for_host(surface_host)
                        .is_some()
                },
            ),
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
                                    node.url()
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
        toolbar_routing::request_command_palette_close(app);
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
    execute_action_with_layout_target(
        app,
        action_id,
        None,
        pair_context,
        source_context,
        intents,
        focused_pane_node,
        focused_pane_id,
    );
}

fn execute_identity_import_action<Execute>(
    app: &mut GraphBrowserApp,
    key: NodeKey,
    protocol: crate::middlenet::capabilities::MiddlenetProtocol,
    execute: Execute,
) where
    Execute: FnOnce(&mut GraphBrowserApp, &str, Option<NodeKey>) -> Result<NodeKey, String>,
{
    let descriptor = crate::middlenet::capabilities::descriptor(protocol);
    let action_name = descriptor
        .action_name
        .unwrap_or(descriptor.display_name);
    let raw_resource = app
        .domain_graph()
        .get_node(key)
        .map(|node| node.url().trim().to_string())
        .unwrap_or_default();

    if raw_resource.is_empty() {
        app.request_node_status_notice(
            key,
            crate::app::UiNotificationLevel::Error,
            format!("{action_name} failed: node URL is empty"),
            Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                action: action_name.to_string(),
                detail: "failed: node URL is empty".to_string(),
            }),
        );
        return;
    }

    let resource = match crate::middlenet::capabilities::normalize_identity_action_resource(
        protocol,
        &raw_resource,
    ) {
        Ok(resource) => resource,
        Err(error) => {
            let detail = format!("failed: {}: {}", raw_resource, error);
            app.request_node_status_notice(
                key,
                crate::app::UiNotificationLevel::Error,
                format!("{action_name} failed for {}: {}", raw_resource, error),
                Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action: action_name.to_string(),
                    detail,
                }),
            );
            return;
        }
    };
    let node_count_before = app.domain_graph().node_count();
    match execute(app, &resource, Some(key)) {
        Ok(subject_key) => {
            let node_count_after = app.domain_graph().node_count();
            let new_nodes = node_count_after.saturating_sub(node_count_before);
            let subject_url = app
                .domain_graph()
                .get_node(subject_key)
                .map(|node| node.url().to_string())
                .unwrap_or_else(|| resource.clone());
            let success_prefix = descriptor.success_prefix.unwrap_or(action_name);
            let message = if new_nodes == 0 {
                format!("{success_prefix} for {resource}")
            } else {
                format!("{success_prefix} for {resource} (+{new_nodes} node(s))")
            };
            let detail = if new_nodes == 0 {
                format!("{} -> {}", resource, subject_url)
            } else {
                format!("{} -> {}; +{} node(s)", resource, subject_url, new_nodes)
            };
            app.request_node_status_notice(
                subject_key,
                crate::app::UiNotificationLevel::Success,
                message,
                Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action: action_name.to_string(),
                    detail,
                }),
            );
        }
        Err(error) => {
            let detail = format!("failed: {}: {}", resource, error);
            app.request_node_status_notice(
                key,
                crate::app::UiNotificationLevel::Error,
                format!("{action_name} failed for {}: {}", resource, error),
                Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action: action_name.to_string(),
                    detail,
                }),
            );
        }
    }
}

pub(crate) fn execute_action_with_layout_target(
    app: &mut GraphBrowserApp,
    action_id: ActionId,
    layout_surface_target_host: Option<SurfaceHostId>,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    intents: &mut Vec<GraphIntent>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<PaneId>,
) {
    let focused_selection = app.focused_selection().clone();
    let open_target = source_context.or_else(|| focused_selection.primary());
    let command_bar_focus_target = CommandBarFocusTarget::new(focused_pane_id, open_target.or(focused_pane_node));
    let frame_target = app.pending_frame_context_target().map(str::to_string);
    let active_layout_surface_host =
        layout_surface_target_host.or_else(|| app.targetable_navigator_surface_host());

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
        ActionId::NodeEditTags => {
            if let Some(key) = open_target.or(focused_pane_node) {
                crate::shell::desktop::ui::tag_panel::open_node_tag_panel(
                    app,
                    key,
                    focused_pane_node == Some(key),
                );
            }
        }
        ActionId::NodeMarkTombstone => intents.push(GraphMutation::MarkTombstoneForSelected.into()),
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
        ActionId::GraphFitGraphlet => intents.push(ViewAction::RequestZoomToGraphlet.into()),
        ActionId::GraphCycleFocusRegion => {
            let _ = toolbar_routing::request_cycle_focus_region(app, command_bar_focus_target);
        }
        ActionId::GraphToggleOverviewPlane => {
            intents.push(GraphIntent::ToggleGraphViewLayoutManager);
        }
        ActionId::GraphTogglePhysics => intents.push(GraphIntent::TogglePhysics),
        ActionId::GraphToggleGhostNodes => intents.push(GraphIntent::ToggleGhostNodes),
        ActionId::GraphPhysicsConfig => {
            registries::phase3_publish_settings_route_requested(
                &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string(),
            );
        }
        ActionId::GraphCommandPalette => {
            toolbar_routing::request_command_palette_open(app);
        }
        ActionId::GraphRadialMenu => {
            let _ = toolbar_routing::request_radial_menu_toggle(app, command_bar_focus_target);
        }
        ActionId::WorkbenchToggleOverlay => {
            let _ = toolbar_routing::request_workbench_overlay_toggle(app, command_bar_focus_target);
        }
        ActionId::FrameSelect => {
            if let Some(frame_name) = frame_target {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: crate::app::SelectionUpdateMode::Replace,
                    }
                    .into(),
                );
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name),
                    }
                    .into(),
                );
            }
        }
        ActionId::FrameOpen | ActionId::FrameOpenAsSplit => {
            if let Some(frame_name) = frame_target
                && let Some(member_key) = resolve_frame_context_member(app, &frame_name)
            {
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name.clone()),
                    }
                    .into(),
                );
                intents.push(GraphIntent::OpenNodeFrameRouted {
                    key: member_key,
                    prefer_frame: Some(frame_name),
                });
            }
        }
        ActionId::FrameRename => {
            if let Some(frame_name) = frame_target {
                app.set_workbench_host_pinned(true);
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name),
                    }
                    .into(),
                );
            }
        }
        ActionId::FrameSettings => {
            if let Some(frame_name) = frame_target {
                app.set_workbench_host_pinned(true);
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name),
                    }
                    .into(),
                );
            }
        }
        ActionId::FrameSuppressSplitOffer | ActionId::FrameEnableSplitOffer => {
            if let Some(frame_name) = frame_target {
                let frame_url = VersoAddress::frame(frame_name.clone()).to_string();
                if let Some((frame_key, _)) = app.domain_graph().get_node_by_url(&frame_url) {
                    intents.push(GraphIntent::SetFrameSplitOfferSuppressed {
                        frame: frame_key,
                        suppressed: matches!(action_id, ActionId::FrameSuppressSplitOffer),
                    });
                }
            }
        }
        ActionId::FrameDelete => {
            if let Some(frame_name) = frame_target
                && let Err(error) = app.delete_workspace_layout(&frame_name)
            {
                log::warn!("command palette failed to delete frame '{frame_name}': {error}");
            }
        }
        ActionId::WorkbenchUnlockSurfaceLayout => {
            if let Some(active_layout_surface_host) = active_layout_surface_host {
                app.enqueue_workbench_intent(WorkbenchIntent::SetSurfaceConfigMode {
                    surface_host: active_layout_surface_host.clone(),
                    mode: UxConfigMode::Configuring {
                        surface_host: active_layout_surface_host,
                    },
                });
            }
        }
        ActionId::WorkbenchLockSurfaceLayout => {
            if let Some(active_layout_surface_host) = active_layout_surface_host {
                app.enqueue_workbench_intent(WorkbenchIntent::SetSurfaceConfigMode {
                    surface_host: active_layout_surface_host,
                    mode: UxConfigMode::Locked,
                });
            }
        }
        ActionId::WorkbenchRememberLayoutPreference => {
            if let Some(active_layout_surface_host) = active_layout_surface_host {
                if let Some(constraint) = app
                    .workbench_layout_constraint_for_host(&active_layout_surface_host)
                    .cloned()
                {
                    app.set_workbench_layout_constraint(
                        active_layout_surface_host.clone(),
                        constraint.clone(),
                    );
                    app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
                        surface_host: active_layout_surface_host.clone(),
                        prompt_shown: true,
                        outcome: Some(FirstUseOutcome::RememberedConstraint(constraint)),
                    });
                }
                if matches!(
                    app.workspace.workbench_session.ux_config_mode,
                    UxConfigMode::Configuring { .. }
                ) {
                    app.enqueue_workbench_intent(WorkbenchIntent::SetSurfaceConfigMode {
                        surface_host: active_layout_surface_host,
                        mode: UxConfigMode::Locked,
                    });
                }
            }
        }
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
        ActionId::PersistImportBookmarks => {
            let runtime_intents = registries::phase3_execute_registry_action(
                app,
                runtime_action::ACTION_IMPORT_BOOKMARKS_FROM_FILE,
                runtime_action::ActionPayload::ImportBookmarksFromFile,
            )
            .unwrap_or_else(|error| {
                log::warn!(
                    "command palette failed to execute '{}': {}",
                    runtime_action::ACTION_IMPORT_BOOKMARKS_FROM_FILE,
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
        ActionId::NodeImportWebFinger => {
            if let Some(key) = open_target {
                execute_identity_import_action(
                    app,
                    key,
                    crate::middlenet::capabilities::MiddlenetProtocol::WebFinger,
                    |app, resource, anchor| {
                        app.fetch_and_import_person_identity_from_webfinger(resource, anchor)
                    },
                );
            }
        }
        ActionId::NodeResolveNip05 => {
            if let Some(key) = open_target {
                execute_identity_import_action(
                    app,
                    key,
                    crate::middlenet::capabilities::MiddlenetProtocol::Nip05,
                    |app, resource, anchor| {
                        app.resolve_and_import_person_identity_from_nip05(resource, anchor)
                    },
                );
            }
        }
        ActionId::NodeResolveMatrix => {
            if let Some(key) = open_target {
                execute_identity_import_action(
                    app,
                    key,
                    crate::middlenet::capabilities::MiddlenetProtocol::Matrix,
                    |app, resource, anchor| {
                        app.resolve_and_import_person_identity_from_matrix(resource, anchor)
                    },
                );
            }
        }
        ActionId::NodeResolveActivityPub => {
            if let Some(key) = open_target {
                execute_identity_import_action(
                    app,
                    key,
                    crate::middlenet::capabilities::MiddlenetProtocol::ActivityPub,
                    |app, resource, anchor| {
                        app.resolve_and_import_person_identity_from_activitypub(resource, anchor)
                    },
                );
            }
        }
        ActionId::NodeRefreshPersonIdentity => {
            if let Some(key) = open_target {
                match app.refresh_person_identity_resolutions(key) {
                    Ok(outcome) => {
                        let message = if outcome.changed {
                            format!(
                                "Refreshed {} identity resolution(s) for person (+changes)",
                                outcome.refreshed_protocols
                            )
                        } else {
                            format!(
                                "Refreshed {} identity resolution(s) for person (no changes)",
                                outcome.refreshed_protocols
                            )
                        };
                        let detail = format!(
                            "person={} refreshed_protocols={} changed={}",
                            outcome.person_key.index(),
                            outcome.refreshed_protocols,
                            if outcome.changed { "yes" } else { "no" }
                        );
                        app.request_node_status_notice(
                            outcome.person_key,
                            crate::app::UiNotificationLevel::Success,
                            message,
                            Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                                action: "Identity refresh".to_string(),
                                detail,
                            }),
                        );
                    }
                    Err(error) => {
                        app.request_node_status_notice(
                            key,
                            crate::app::UiNotificationLevel::Error,
                            format!("Identity refresh failed: {error}"),
                            Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                                action: "Identity refresh".to_string(),
                                detail: format!("failed: {error}"),
                            }),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;
    use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, install_global_sender};
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED,
        CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS,
        CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED,
    };

    fn default_action_context() -> ActionContext {
        ActionContext {
            target_node: None,
            target_frame_name: None,
            target_frame_member: None,
            target_frame_split_offer_suppressed: false,
            pair_context: None,
            any_selected: false,
            focused_pane_available: false,
            undo_available: false,
            redo_available: false,
            input_mode: InputMode::MouseKeyboard,
            view_id: GraphViewId::new(),
            wry_override_allowed: false,
            layout_surface_host_available: false,
            layout_surface_target_host: None,
            layout_surface_target_ambiguous: false,
            layout_surface_configuring: false,
            layout_surface_has_draft: false,
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
    fn cycle_focus_action_uses_shared_focus_gate() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::GraphCycleFocusRegion,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(app.take_pending_workbench_intents().is_empty());

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED
            )),
            "expected workbench-command requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSentStructured { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED
            )),
            "expected structured route-blocked diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS
            )),
            "expected blocked-by-focus diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn disabled_layout_actions_expose_ambiguous_host_reason() {
        let reason = disabled_action_reason(
            ActionId::WorkbenchUnlockSurfaceLayout,
            &ActionContext {
                layout_surface_target_ambiguous: true,
                ..default_action_context()
            },
        );
        assert_eq!(
            reason,
            Some(
                "Multiple Navigator hosts are visible. Unlock layout from the specific host chrome first or enter config mode on that host."
            )
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
    fn execute_action_workbench_overlay_routes_through_toolbar_dispatch() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchToggleOverlay,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::SetWorkbenchOverlayVisible { visible: true }]
        ));
    }

    #[test]
    fn execute_action_unlock_surface_layout_enqueues_configuring_intent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchUnlockSurfaceLayout,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::SetSurfaceConfigMode {
                surface_host,
                mode: UxConfigMode::Configuring { surface_host: configuring_host },
            }] if surface_host == configuring_host
        ));
    }

    #[test]
    fn execute_action_unlock_surface_layout_skips_when_host_target_is_ambiguous() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.15,
            ),
        );
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                0.15,
            ),
        );
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchUnlockSurfaceLayout,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(app.take_pending_workbench_intents().is_empty());
    }

    #[test]
    fn execute_action_with_layout_target_unlocks_selected_host_when_multiple_hosts_visible() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.15,
            ),
        );
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                0.15,
            ),
        );
        let selected_host = crate::app::SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
        );
        let mut intents = Vec::new();

        execute_action_with_layout_target(
            &mut app,
            ActionId::WorkbenchUnlockSurfaceLayout,
            Some(selected_host.clone()),
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::SetSurfaceConfigMode {
                surface_host,
                mode: UxConfigMode::Configuring { surface_host: configuring_host },
            }] if surface_host == &selected_host && configuring_host == &selected_host
        ));
    }

    #[test]
    fn execute_action_with_layout_target_remembers_selected_host_draft() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.15,
            ),
        );
        let selected_host = crate::app::SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
        );
        app.set_workbench_layout_constraint_draft(
            selected_host.clone(),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                selected_host.clone(),
                crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                0.21,
            ),
        );
        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: selected_host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        app.set_workbench_surface_config_mode(UxConfigMode::Configuring {
            surface_host: selected_host.clone(),
        });
        let mut intents = Vec::new();

        execute_action_with_layout_target(
            &mut app,
            ActionId::WorkbenchRememberLayoutPreference,
            Some(selected_host.clone()),
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(
            app.workbench_layout_constraint_draft_for_host(&selected_host)
                .is_none()
        );
        assert!(matches!(
            app.workbench_profile()
                .first_use_policies
                .get(&selected_host)
                .and_then(|policy| policy.outcome.as_ref()),
            Some(FirstUseOutcome::RememberedConstraint(
                crate::app::WorkbenchLayoutConstraint::AnchoredSplit {
                    surface_host,
                    anchor_edge: crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                    anchor_size_fraction,
                    ..
                }
            )) if surface_host == &selected_host && (*anchor_size_fraction - 0.21).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn execute_action_remember_layout_preference_commits_draft_and_marks_policy() {
        let mut app = GraphBrowserApp::new_for_testing();
        let host = app.primary_navigator_surface_host();
        app.set_workbench_layout_constraint_draft(
            host.clone(),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                host.clone(),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.18,
            ),
        );
        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        app.set_workbench_surface_config_mode(UxConfigMode::Configuring {
            surface_host: host.clone(),
        });
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::WorkbenchRememberLayoutPreference,
            None,
            None,
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert!(
            app.workbench_layout_constraint_draft_for_host(&host)
                .is_none()
        );
        assert!(app.workbench_layout_constraint_for_host(&host).is_some());
        assert!(matches!(
            app.workbench_profile()
                .first_use_policies
                .get(&host)
                .and_then(|policy| policy.outcome.as_ref()),
            Some(FirstUseOutcome::RememberedConstraint(
                crate::app::WorkbenchLayoutConstraint::AnchoredSplit {
                    surface_host,
                    anchor_edge: crate::app::workbench_layout_policy::AnchorEdge::Top,
                    anchor_size_fraction,
                    cross_axis_margin_start_px,
                    cross_axis_margin_end_px,
                    resizable,
                }
            )) if surface_host == &host
                && (*anchor_size_fraction - 0.18).abs() < f32::EPSILON
                && (*cross_axis_margin_start_px).abs() < f32::EPSILON
                && (*cross_axis_margin_end_px).abs() < f32::EPSILON
                && *resizable
        ));
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::SetSurfaceConfigMode {
                surface_host,
                mode: UxConfigMode::Locked,
            }] if surface_host == &host
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
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push(url.clone());
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
                        == &VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string()
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
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push(url.clone());
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
                        == &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string()
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
                    },
                ) = &signal.kind
                {
                    seen.lock()
                        .expect("observer lock poisoned")
                        .push(url.clone());
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
                        == &VersoAddress::settings(GraphshellSettingsPath::General).to_string()
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
        assert_eq!(app.default_registry_physics_id(), Some("physics:scatter"));
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
    fn execute_action_opens_tag_panel_for_target_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://tags.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::NodeEditTags,
            None,
            Some(node),
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        assert_eq!(
            app.workspace
                .graph_runtime
                .tag_panel_state
                .as_ref()
                .map(|state| state.node_key),
            Some(node)
        );
    }

    #[test]
    fn execute_action_webfinger_import_queues_error_notice_for_unsupported_resource() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "misfin://friend@example.net".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut intents = Vec::new();

        execute_action(
            &mut app,
            ActionId::NodeImportWebFinger,
            None,
            Some(node),
            &mut intents,
            None,
            None,
        );

        assert!(intents.is_empty());
        let request = app
            .take_pending_node_status_notice()
            .expect("webfinger import should queue a notice");
        assert_eq!(request.key, node);
        assert_eq!(request.level, crate::app::UiNotificationLevel::Error);
        assert!(request.message.contains("WebFinger import failed"));
        assert!(request.message.contains("misfin://friend@example.net"));
        assert!(matches!(
            request.audit_event,
            Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                action,
                detail,
            }) if action == "WebFinger import"
                && detail.contains("failed:")
                && detail.contains("misfin://friend@example.net")
        ));
    }

    #[test]
    fn execute_action_webfinger_import_imports_graph_and_queues_success_notice() {
        crate::middlenet::identity::with_test_identity_resolution_cache_scope(|| {
            let mut app = GraphBrowserApp::new_for_testing();
            let resource = "https://social.example/users/mark";
            let node = app.add_node_and_sync(
                resource.to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );
            let mut intents = Vec::new();
            let import = crate::middlenet::webfinger::WebFingerImport {
                subject: "acct:mark@social.example".to_string(),
                aliases: Vec::new(),
                profile_pages: vec!["https://social.example/profile".to_string()],
                gemini_capsules: vec!["gemini://social.example/~mark".to_string()],
                gopher_resources: Vec::new(),
                misfin_mailboxes: Vec::new(),
                nostr_identities: Vec::new(),
                activitypub_actors: Vec::new(),
                other_endpoints: Vec::new(),
            };

            crate::middlenet::webfinger::with_test_fetch_import_override(
                resource,
                Ok(import),
                || {
                    execute_action(
                        &mut app,
                        ActionId::NodeImportWebFinger,
                        None,
                        Some(node),
                        &mut intents,
                        None,
                        None,
                    );
                },
            );

            assert!(intents.is_empty());
            let subject_key = app
                .get_single_selected_node()
                .expect("person node should be selected after import");
            let subject_node = app
                .domain_graph()
                .get_node(subject_key)
                .expect("person node should be created");
            assert!(subject_node.url().starts_with("verso://person/"));
            assert_eq!(subject_node.title, "Person: mark@social.example");
            assert!(app.node_has_canonical_tag(subject_key, "#person"));
            assert!(app.node_has_canonical_tag(subject_key, "#webfinger"));
            assert!(app.node_has_canonical_tag(subject_key, "#identity"));

            let (acct_key, acct_node) = app
                .domain_graph()
                .get_node_by_url("acct:mark@social.example")
                .expect("acct identity node should be created");
            assert_eq!(acct_node.title, "WebFinger identity: mark@social.example");
            assert!(app.node_has_canonical_tag(acct_key, "#webfinger"));

            let (_, profile_node) = app
                .domain_graph()
                .get_node_by_url("https://social.example/profile")
                .expect("profile node should be created");
            assert_eq!(profile_node.title, "Profile: https://social.example/profile");

            let (alias_key, alias_node) = app
                .domain_graph()
                .get_node_by_url(resource)
                .expect("resource alias node should still exist");
            assert_eq!(alias_key, node);
            assert_eq!(alias_node.title, format!("Alias: {resource}"));
            assert!(app.node_has_canonical_tag(alias_key, "#alias"));

            assert!(app
                .domain_graph()
                .get_node_by_url("gemini://social.example/~mark")
                .is_some());
            let request = app
                .take_pending_node_status_notice()
                .expect("webfinger import should queue a success notice");
            assert_eq!(request.key, subject_key);
            assert_eq!(request.level, crate::app::UiNotificationLevel::Success);
            assert!(request.message.contains("Imported WebFinger discovery for https://social.example/users/mark"));
            assert!(request.message.contains("+4 node(s)"));
            assert!(matches!(
                request.audit_event,
                Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action,
                    detail,
                }) if action == "WebFinger import"
                    && detail.contains("https://social.example/users/mark -> verso://person/")
                    && detail.contains("+4 node(s)")
            ));
        });
    }

    #[test]
    fn execute_action_resolve_nip05_imports_person_and_queues_success_notice() {
        crate::middlenet::identity::with_test_identity_resolution_cache_scope(|| {
            let mut app = GraphBrowserApp::new_for_testing();
            let node = app.add_node_and_sync(
                "nip05:mark@example.net".to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );
            let mut intents = Vec::new();
            let profile = crate::middlenet::identity::PersonIdentityProfile {
                human_handle: Some("mark@example.net".to_string()),
                nip05_identifier: Some("mark@example.net".to_string()),
                nostr_identities: vec!["nostr:npub1example".to_string()],
                ..Default::default()
            };

            crate::middlenet::identity::with_test_resolve_nip05_override(
                "mark@example.net",
                Ok(profile),
                || {
                    execute_action(
                        &mut app,
                        ActionId::NodeResolveNip05,
                        None,
                        Some(node),
                        &mut intents,
                        None,
                        None,
                    );
                },
            );

            assert!(intents.is_empty());
            let subject_key = app
                .get_single_selected_node()
                .expect("person node should be selected after NIP-05 resolve");
            let subject_node = app
                .domain_graph()
                .get_node(subject_key)
                .expect("person node should exist");
            assert!(subject_node.url().starts_with("verso://person/"));
            assert_eq!(subject_node.title, "Person: mark@example.net");
            let request = app
                .take_pending_node_status_notice()
                .expect("nip-05 resolve should queue a success notice");
            assert_eq!(request.key, subject_key);
            assert_eq!(request.level, crate::app::UiNotificationLevel::Success);
            assert!(request.message.contains("Resolved NIP-05 identity for mark@example.net"));
            assert!(matches!(
                request.audit_event,
                Some(crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action,
                    detail,
                }) if action == "NIP-05 resolve"
                    && detail.contains("mark@example.net -> verso://person/")
            ));
        });
    }

    #[test]
    fn execute_action_resolve_matrix_imports_person_and_queues_success_notice() {
        crate::middlenet::identity::with_test_identity_resolution_cache_scope(|| {
            let mut app = GraphBrowserApp::new_for_testing();
            let node = app.add_node_and_sync(
                "mxid:@mark:matrix.example".to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );
            let mut intents = Vec::new();
            let mut profile = crate::middlenet::identity::PersonIdentityProfile {
                human_handle: Some("mark@matrix.example".to_string()),
                ..Default::default()
            };
            profile
                .push_matrix_mxid("@mark:matrix.example")
                .expect("matrix mxid should normalize");

            crate::middlenet::identity::with_test_resolve_matrix_override(
                "@mark:matrix.example",
                Ok(profile),
                || {
                    execute_action(
                        &mut app,
                        ActionId::NodeResolveMatrix,
                        None,
                        Some(node),
                        &mut intents,
                        None,
                        None,
                    );
                },
            );

            assert!(intents.is_empty());
            let subject_key = app
                .get_single_selected_node()
                .expect("person node should be selected after Matrix resolve");
            let subject_node = app
                .domain_graph()
                .get_node(subject_key)
                .expect("person node should exist");
            assert!(subject_node.url().starts_with("verso://person/"));
            assert_eq!(subject_node.title, "Person: mark@matrix.example");
            let request = app
                .take_pending_node_status_notice()
                .expect("matrix resolve should queue a success notice");
            assert_eq!(request.key, subject_key);
            assert_eq!(request.level, crate::app::UiNotificationLevel::Success);
            assert!(request.message.contains("Resolved Matrix profile for @mark:matrix.example"));
        });
    }

    #[test]
    fn execute_action_import_activitypub_imports_person_and_queues_success_notice() {
        crate::middlenet::identity::with_test_identity_resolution_cache_scope(|| {
            let mut app = GraphBrowserApp::new_for_testing();
            let resource = "https://social.example/users/mark";
            let node = app.add_node_and_sync(
                resource.to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );
            let mut intents = Vec::new();
            let mut profile = crate::middlenet::identity::PersonIdentityProfile {
                human_handle: Some("mark@social.example".to_string()),
                ..Default::default()
            };
            profile
                .push_activitypub_actor(resource)
                .expect("activitypub actor should normalize");
            profile
                .push_profile_page("https://social.example/@mark")
                .expect("profile page should normalize");

            crate::middlenet::identity::with_test_resolve_activitypub_override(
                resource,
                Ok(profile),
                || {
                    execute_action(
                        &mut app,
                        ActionId::NodeResolveActivityPub,
                        None,
                        Some(node),
                        &mut intents,
                        None,
                        None,
                    );
                },
            );

            assert!(intents.is_empty());
            let subject_key = app
                .get_single_selected_node()
                .expect("person node should be selected after ActivityPub import");
            let request = app
                .take_pending_node_status_notice()
                .expect("activitypub import should queue a success notice");
            assert_eq!(request.key, subject_key);
            assert_eq!(request.level, crate::app::UiNotificationLevel::Success);
            assert!(request
                .message
                .contains("Imported ActivityPub actor for https://social.example/users/mark"));
        });
    }

    #[test]
    fn execute_action_refresh_person_identity_queues_refresh_notice() {
        crate::middlenet::identity::with_test_identity_resolution_cache_scope(|| {
            let mut app = GraphBrowserApp::new_for_testing();
            let mut profile = crate::middlenet::identity::PersonIdentityProfile {
                human_handle: Some("mark@example.net".to_string()),
                nip05_identifier: Some("mark@example.net".to_string()),
                ..Default::default()
            };
            profile
                .push_gemini_capsule("gemini://capsule.example/~mark")
                .expect("capsule should normalize");
            let person = crate::middlenet::identity::with_test_resolve_nip05_override(
                "mark@example.net",
                Ok(profile.clone()),
                || {
                    app.resolve_and_import_person_identity_from_nip05("mark@example.net", None)
                        .expect("initial import should succeed")
                },
            );
            let mut enriched = profile;
            enriched
                .push_profile_page("https://example.net/~mark")
                .expect("profile page should normalize");
            let mut intents = Vec::new();

            crate::middlenet::identity::with_test_resolve_nip05_override(
                "mark@example.net",
                Ok(enriched),
                || {
                    execute_action(
                        &mut app,
                        ActionId::NodeRefreshPersonIdentity,
                        None,
                        Some(person),
                        &mut intents,
                        None,
                        None,
                    );
                },
            );

            assert!(intents.is_empty());
            let request = app
                .take_pending_node_status_notice()
                .expect("identity refresh should queue a success notice");
            assert_eq!(request.key, person);
            assert_eq!(request.level, crate::app::UiNotificationLevel::Success);
            assert!(request.message.contains("Refreshed 1 identity resolution(s) for person"));
            assert!(request.message.contains("+changes"));
        });
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
