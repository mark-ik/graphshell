/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::{Color32, Context, Key, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Window};
use egui_tiles::Tree;

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchOrigin, GraphViewId, GraphViewLayoutDirection,
    PendingTileOpenMode, ViewGraphletPartition, WorkbenchIntent,
};
use crate::graph::{GraphletKind, NodeKey};
use crate::shell::desktop::runtime::registries::phase3_trusted_peers;
use crate::shell::desktop::ui::swatch::{
    GraphSwatchInteraction, GraphSwatchSpec, SwatchDensityPolicy, SwatchHostOptions,
    SwatchInteractionProfile, SwatchLayoutProfile, SwatchSizeClass, SwatchSourceScope,
    render_graph_swatch_card,
};
use crate::shell::desktop::ui::workbench_host::{
    GraphletRosterEntry, WorkbenchChromeProjection, WorkbenchNodeViewerSummary, WorkbenchPaneEntry,
    WorkbenchPaneKind,
};
#[cfg(test)]
use crate::shell::desktop::workbench::pane_model::PanePresentationMode;
use crate::shell::desktop::workbench::pane_model::{
    PaneId, TileRenderMode, ViewerId, ViewerSwitchReason,
};
use crate::shell::desktop::workbench::semantic_tabs::SemanticTabAffordance;
use crate::shell::desktop::workbench::tile_kind::TileKind;

const OVERVIEW_CELL_SIZE: Vec2 = Vec2::new(156.0, 92.0);
const OVERVIEW_CELL_GAP: f32 = 16.0;
const OVERVIEW_SWATCH_GAP: f32 = 8.0;
const NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH: f32 = 272.0;
const NAVIGATOR_OVERVIEW_TRANSFER_CELL_MIN_WIDTH: f32 = 60.0;
const NAVIGATOR_GRAPHLET_SWATCH_MAX_HEIGHT: f32 = 320.0;
const NAVIGATOR_GRAPHLET_PREVIEW_NODE_LIMIT: usize = 18;
const OVERVIEW_SELECTED_VIEW_ID_KEY: &str = "graphshell_overview_selected_view";
const NAVIGATOR_OVERVIEW_DRAG_SOURCE_VIEW_KEY: &str =
    "graphshell_navigator_overview_drag_source_view";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OverviewSlotSnapshot {
    pub(crate) view_id: GraphViewId,
    pub(crate) name: String,
    pub(crate) row: i32,
    pub(crate) col: i32,
    pub(crate) archived: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverviewSurfaceAction {
    FocusView(GraphViewId),
    OpenView(GraphViewId),
    TransferSelectionToView {
        source_view: GraphViewId,
        destination_view: GraphViewId,
    },
    ToggleOverviewPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigatorOverviewAction {
    Surface(OverviewSurfaceAction),
    SelectGraphletAnchor {
        view_id: GraphViewId,
        node_key: NodeKey,
    },
    OpenGraphletSpecialty {
        view_id: GraphViewId,
        node_key: NodeKey,
        kind: GraphletKind,
    },
}

impl From<OverviewSurfaceAction> for NavigatorOverviewAction {
    fn from(action: OverviewSurfaceAction) -> Self {
        Self::Surface(action)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverviewActionOwner {
    Graph,
    Workbench,
    Viewer,
    Runtime,
}

#[derive(Debug, Clone)]
enum OverviewQuickActionDispatch {
    Graph(GraphIntent),
    Workbench(WorkbenchIntent),
}

#[derive(Debug, Clone)]
struct OverviewQuickAction {
    label: String,
    owner: OverviewActionOwner,
    hover_text: String,
    dispatch: OverviewQuickActionDispatch,
}

fn overview_surface_action_to_workbench_intent(action: OverviewSurfaceAction) -> WorkbenchIntent {
    match action {
        OverviewSurfaceAction::FocusView(view_id) => WorkbenchIntent::FocusGraphView { view_id },
        OverviewSurfaceAction::OpenView(view_id) => WorkbenchIntent::OpenGraphViewPane {
            view_id,
            mode: PendingTileOpenMode::Tab,
        },
        OverviewSurfaceAction::TransferSelectionToView {
            source_view,
            destination_view,
        } => WorkbenchIntent::TransferSelectedNodesToGraphView {
            source_view,
            destination_view,
        },
        OverviewSurfaceAction::ToggleOverviewPlane => WorkbenchIntent::ToggleOverviewPlane,
    }
}

pub(crate) fn render_overview_plane(
    ctx: &Context,
    app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    active_pane: Option<PaneId>,
) {
    if !app.graph_view_layout_manager_active() {
        return;
    }

    let chrome_projection = WorkbenchChromeProjection::from_tree(app, tiles_tree, active_pane);
    let slots = sorted_slot_snapshots(app);
    let active_slots: Vec<_> = slots
        .iter()
        .filter(|slot| !slot.archived)
        .cloned()
        .collect();
    let archived_slots: Vec<_> = slots.iter().filter(|slot| slot.archived).cloned().collect();
    let selected_view_id = overview_surface_selected_view_id(ctx, app, &slots);
    let selected_slot = slots
        .iter()
        .find(|slot| Some(slot.view_id) == selected_view_id);
    let mut open = true;
    let mut close_requested = false;
    let mut pending_graph_intents = Vec::new();
    let mut pending_workbench_intents = Vec::new();
    let mut pending_surface_actions = Vec::new();

    let response = Window::new("Overview Plane")
        .id(egui::Id::new("graphshell_overview_plane"))
        .default_pos(overview_window_pos(app))
        .default_width(880.0)
        .default_height(560.0)
        .resizable(true)
        .open(&mut open)
        .show(ctx, |ui| {
            render_overview_active_context_strip(ui, app, &chrome_projection, selected_slot);
            ui.add_space(8.0);
            ui.columns(2, |columns| {
                render_overview_summary_card(
                    &mut columns[0],
                    "Graph Context",
                    &graph_context_lines(app, &chrome_projection, selected_slot),
                    &graph_context_actions(app, selected_slot),
                    &mut pending_graph_intents,
                    &mut pending_workbench_intents,
                );
                render_overview_summary_card(
                    &mut columns[1],
                    "Workbench Context",
                    &workbench_context_lines(&chrome_projection),
                    &workbench_context_actions(&chrome_projection, selected_slot),
                    &mut pending_graph_intents,
                    &mut pending_workbench_intents,
                );
            });
            ui.add_space(8.0);
            ui.columns(2, |columns| {
                render_overview_summary_card(
                    &mut columns[0],
                    "Viewer / Content",
                    &viewer_content_lines(&chrome_projection),
                    &viewer_content_actions(&chrome_projection),
                    &mut pending_graph_intents,
                    &mut pending_workbench_intents,
                );
                render_overview_summary_card(
                    &mut columns[1],
                    "Runtime / Attention",
                    &runtime_attention_lines(app),
                    &runtime_attention_actions(app),
                    &mut pending_graph_intents,
                    &mut pending_workbench_intents,
                );
            });
            ui.add_space(8.0);
            render_overview_suggested_actions(
                ui,
                app,
                selected_slot,
                archived_slots.len(),
                &mut pending_graph_intents,
                &mut pending_surface_actions,
            );
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Graph-owned graph-view management")
                        .small()
                        .italics(),
                );
                ui.separator();
                if ui.button("Create View").clicked() {
                    pending_graph_intents.push(GraphIntent::CreateGraphViewSlot {
                        anchor_view: selected_view_id,
                        direction: GraphViewLayoutDirection::Right,
                        open_mode: Some(PendingTileOpenMode::Tab),
                    });
                }
                if ui.button("Exit").clicked() {
                    close_requested = true;
                }
            });
            ui.small(
                "Click to focus a view, double-click to open it, Arrow keys to traverse, Space to focus, Enter to open, Ctrl+Enter to transfer, Alt+Arrow to move, and Ctrl+Shift+Arrow to create adjacent.",
            );
            ui.separator();

            ui.columns(2, |columns| {
                render_overview_grid(
                    &mut columns[0],
                    ctx,
                    &active_slots,
                    selected_view_id,
                    &mut pending_graph_intents,
                    &mut pending_surface_actions,
                );
                render_overview_details(
                    &mut columns[1],
                    app,
                    ctx,
                    selected_slot,
                    &archived_slots,
                    &mut pending_graph_intents,
                    &mut pending_surface_actions,
                );
            });
        });

    if let Some(rect) = response.as_ref().map(|inner| inner.response.rect)
        && ctx.input(|input| input.pointer.primary_clicked())
        && let Some(pointer) = ctx.input(|input| input.pointer.interact_pos())
        && !rect.contains(pointer)
    {
        close_requested = true;
    }

    let (keyboard_selected_view_id, keyboard_graph_intents, keyboard_surface_actions) =
        collect_overview_keyboard_intents(ctx, app, &slots, selected_view_id);
    if keyboard_selected_view_id != selected_view_id {
        set_overview_surface_selected_view_id(ctx, keyboard_selected_view_id);
    }
    pending_graph_intents.extend(keyboard_graph_intents);
    pending_surface_actions.extend(keyboard_surface_actions);

    if close_requested || !open {
        pending_surface_actions.push(OverviewSurfaceAction::ToggleOverviewPlane);
    }

    // Graph-view slot creation, rename, move, archive, and restore remain
    // graph-owned layout mutations; only surface routing goes through
    // WorkbenchIntent from the overview plane.
    if !pending_graph_intents.is_empty() {
        app.apply_reducer_intents(pending_graph_intents);
    }
    for intent in pending_workbench_intents {
        app.enqueue_workbench_intent(intent);
    }
    for action in pending_surface_actions {
        app.enqueue_workbench_intent(overview_surface_action_to_workbench_intent(action));
    }
}

fn render_overview_active_context_strip(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    chrome_projection: &WorkbenchChromeProjection,
    selected_slot: Option<&OverviewSlotSnapshot>,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Active Context").strong());
                ui.separator();
                ui.label(active_context_summary(
                    app,
                    chrome_projection,
                    selected_slot,
                ));
            });
        });
}

fn render_overview_summary_card(
    ui: &mut Ui,
    title: &str,
    lines: &[String],
    actions: &[OverviewQuickAction],
    pending_graph_intents: &mut Vec<GraphIntent>,
    pending_workbench_intents: &mut Vec<WorkbenchIntent>,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new(title).strong());
            ui.add_space(4.0);
            if lines.is_empty() {
                ui.small("No current state.");
                return;
            }
            for line in lines {
                ui.small(line);
            }
            if !actions.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.small(
                    RichText::new(format!(
                        "Routes: {}",
                        overview_action_owner_summary(actions)
                    ))
                    .weak(),
                );
                ui.add_space(4.0);
                render_overview_quick_actions(
                    ui,
                    actions,
                    pending_graph_intents,
                    pending_workbench_intents,
                );
            }
        });
}

fn render_overview_quick_actions(
    ui: &mut Ui,
    actions: &[OverviewQuickAction],
    pending_graph_intents: &mut Vec<GraphIntent>,
    pending_workbench_intents: &mut Vec<WorkbenchIntent>,
) {
    ui.horizontal_wrapped(|ui| {
        for action in actions {
            let owner = overview_action_owner_label(action.owner);
            if ui
                .small_button(&action.label)
                .on_hover_text(format!("Routes to {owner}: {}", action.hover_text))
                .clicked()
            {
                match &action.dispatch {
                    OverviewQuickActionDispatch::Graph(intent) => {
                        pending_graph_intents.push(intent.clone())
                    }
                    OverviewQuickActionDispatch::Workbench(intent) => {
                        pending_workbench_intents.push(intent.clone())
                    }
                }
            }
        }
    });
}

fn render_overview_suggested_actions(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    selected_slot: Option<&OverviewSlotSnapshot>,
    archived_count: usize,
    pending_graph_intents: &mut Vec<GraphIntent>,
    pending_surface_actions: &mut Vec<OverviewSurfaceAction>,
) {
    let transfer_enabled =
        selected_slot.is_some_and(|slot| overview_transfer_affordance(app, slot.view_id).enabled);
    let preview_mode_active = app.history_health_summary().preview_mode_active;

    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new("Suggested Next Actions").strong());
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for label in overview_suggestion_labels(
                    selected_slot.is_some(),
                    transfer_enabled,
                    archived_count,
                    preview_mode_active,
                ) {
                    ui.label(RichText::new(label).small().weak());
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let Some(slot) = selected_slot else {
                    return;
                };

                if ui.button("Focus selected view").clicked() {
                    pending_surface_actions.push(OverviewSurfaceAction::FocusView(slot.view_id));
                }
                if ui.button("Open selected view").clicked() {
                    pending_surface_actions.push(OverviewSurfaceAction::OpenView(slot.view_id));
                }

                let transfer_affordance = overview_transfer_affordance(app, slot.view_id);
                let transfer_button = ui.add_enabled(
                    transfer_affordance.enabled,
                    egui::Button::new("Transfer selection"),
                );
                let transfer_button = if transfer_affordance.enabled {
                    transfer_button.on_hover_text(
                        "Transfer the current focused selection into the selected graph view",
                    )
                } else {
                    transfer_button.on_disabled_hover_text(transfer_affordance.disabled_reason)
                };
                if transfer_button.clicked()
                    && let Some(action) = overview_transfer_action(app, slot.view_id)
                {
                    pending_surface_actions.push(action);
                }

                if ui.button("Create adjacent view").clicked() {
                    pending_graph_intents.push(GraphIntent::CreateGraphViewSlot {
                        anchor_view: Some(slot.view_id),
                        direction: GraphViewLayoutDirection::Right,
                        open_mode: Some(PendingTileOpenMode::Tab),
                    });
                }
            });
        });
}

fn active_context_summary(
    app: &GraphBrowserApp,
    chrome_projection: &WorkbenchChromeProjection,
    selected_slot: Option<&OverviewSlotSnapshot>,
) -> String {
    let mut parts = Vec::new();
    parts.push(match selected_slot {
        Some(slot) => format!("View {}", slot.name),
        None => "View none".to_string(),
    });
    parts.push(format!(
        "Frame {}",
        chrome_projection
            .active_frame_name
            .as_deref()
            .unwrap_or("session")
    ));
    parts.push(format!(
        "Pane {}",
        chrome_projection
            .active_pane_title
            .as_deref()
            .unwrap_or("unfocused")
    ));
    parts.push(format!(
        "Focus {}",
        focus_authority_label(chrome_projection)
    ));
    if app.history_health_summary().preview_mode_active {
        parts.push("History preview".to_string());
    }
    parts.join(" · ")
}

fn graph_context_lines(
    app: &GraphBrowserApp,
    chrome_projection: &WorkbenchChromeProjection,
    selected_slot: Option<&OverviewSlotSnapshot>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let selection = app.focused_selection();
    if let Some(primary) = selection.primary() {
        lines.push(format!(
            "Primary target: {}",
            node_summary_label(app, primary)
        ));
        let member_count = app.graphlet_peers_for_active_projection(primary).len() + 1;
        lines.push(format!("Projected graphlet: {member_count} node(s)"));
    } else {
        lines.push("Primary target: none".to_string());
    }
    if selection.len() > 1 {
        lines.push(format!("Secondary targets: {}", selection.len() - 1));
    }
    if let Some(summary) = active_graphlet_roster_summary(&chrome_projection.active_graphlet_roster)
    {
        lines.push(summary);
    }
    if let Some(frontier) =
        active_graphlet_frontier_summary(&chrome_projection.active_graphlet_roster)
    {
        lines.push(frontier);
    }
    if let Some(slot) = selected_slot {
        let node_count = app.graph_view_owned_node_count(slot.view_id).unwrap_or(0);
        let external_links = app.graph_view_external_link_count(slot.view_id);
        lines.push(format!(
            "{}: {node_count} owned node(s) · {external_links} cross-view link(s)",
            slot.name
        ));
    }
    let query = app.workspace.graph_runtime.active_graph_search_query.trim();
    if !query.is_empty() {
        lines.push(format!(
            "Search: {query} · {} matches · {}",
            app.workspace.graph_runtime.active_graph_search_match_count,
            graph_search_origin_label(&app.workspace.graph_runtime.active_graph_search_origin)
        ));
    }
    lines
}

fn graph_context_actions(
    app: &GraphBrowserApp,
    selected_slot: Option<&OverviewSlotSnapshot>,
) -> Vec<OverviewQuickAction> {
    let mut actions = Vec::new();
    if let Some(slot) = selected_slot {
        actions.push(OverviewQuickAction {
            label: "Create adjacent view".to_string(),
            owner: OverviewActionOwner::Graph,
            hover_text:
                "Ask Graph to create a neighboring graph-view slot from the selected region."
                    .to_string(),
            dispatch: OverviewQuickActionDispatch::Graph(GraphIntent::CreateGraphViewSlot {
                anchor_view: Some(slot.view_id),
                direction: GraphViewLayoutDirection::Right,
                open_mode: Some(PendingTileOpenMode::Tab),
            }),
        });
        actions.push(OverviewQuickAction {
            label: "Focus region".to_string(),
            owner: OverviewActionOwner::Workbench,
            hover_text:
                "Ask Workbench to foreground the selected graph-view region without mutating graph truth."
                    .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::FocusGraphView {
                view_id: slot.view_id,
            }),
        });
    }
    if let Some(primary) = app.focused_selection().primary() {
        actions.push(OverviewQuickAction {
            label: "Open primary node".to_string(),
            owner: OverviewActionOwner::Workbench,
            hover_text: "Route the primary graph target into the Workbench node-pane open path."
                .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenNodeInPane {
                node: primary,
                pane: PaneId::new(),
            }),
        });
    }
    actions
}

fn workbench_context_lines(chrome_projection: &WorkbenchChromeProjection) -> Vec<String> {
    let mut lines = vec![format!(
        "Active frame: {}",
        chrome_projection
            .active_frame_name
            .as_deref()
            .unwrap_or("session")
    )];
    lines.push(format!(
        "Focused pane: {}",
        chrome_projection
            .active_pane_title
            .as_deref()
            .unwrap_or("none")
    ));
    lines.push(format!(
        "Open panes: {}",
        chrome_projection.pane_entries.len()
    ));
    lines.push(format!(
        "Saved frames: {}",
        chrome_projection.saved_frame_names.len()
    ));
    if let Some(binding) = active_workbench_binding_summary(chrome_projection) {
        lines.push(format!("Workbench binding: {binding}"));
    }
    if let Some(summary) = active_graphlet_roster_summary(&chrome_projection.active_graphlet_roster)
    {
        lines.push(summary);
    }
    lines
}

fn workbench_context_actions(
    chrome_projection: &WorkbenchChromeProjection,
    selected_slot: Option<&OverviewSlotSnapshot>,
) -> Vec<OverviewQuickAction> {
    let mut actions = Vec::new();
    if let Some(frame_name) = chrome_projection.active_frame_name.as_deref() {
        actions.push(OverviewQuickAction {
            label: "Open active frame".to_string(),
            owner: OverviewActionOwner::Workbench,
            hover_text:
                "Route the active frame summary through the canonical Workbench frame URL path."
                    .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenFrameUrl {
                url: crate::util::VersoAddress::frame(frame_name.to_string()).to_string(),
                focus_node: None,
            }),
        });
    }
    if let Some(slot) = selected_slot {
        actions.push(OverviewQuickAction {
            label: "Open selected view".to_string(),
            owner: OverviewActionOwner::Workbench,
            hover_text:
                "Open the selected graph-view summary in a Workbench pane via the shared intent path."
                    .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenGraphViewPane {
                view_id: slot.view_id,
                mode: PendingTileOpenMode::Tab,
            }),
        });
    }
    actions
}

fn viewer_content_lines(chrome_projection: &WorkbenchChromeProjection) -> Vec<String> {
    let Some(active_entry) = active_overview_pane_entry(chrome_projection) else {
        return vec!["Viewer backend: no active workbench pane".to_string()];
    };

    let mut lines = vec![format!(
        "Viewer backend: {}",
        viewer_backend_summary(active_entry)
    )];
    lines.push(format!("Content: {}", active_entry.title));
    if let Some(subtitle) = active_entry.subtitle.as_deref()
        && !subtitle.trim().is_empty()
    {
        lines.push(format!("Context: {subtitle}"));
    }
    if let Some(summary) = active_entry.node_viewer_summary.as_ref() {
        let selection_mode = match summary.viewer_override.as_deref() {
            Some(override_id) => format!(
                "Override: {override_id} · {}",
                viewer_switch_reason_label(summary.viewer_switch_reason)
            ),
            None => format!(
                "Selection: auto · {}",
                viewer_switch_reason_label(summary.viewer_switch_reason)
            ),
        };
        lines.push(selection_mode);
        if summary.runtime_crashed {
            lines.push("Degraded: runtime crash recorded for this node".to_string());
        }
        if summary.runtime_blocked {
            lines.push("Runtime blocked: startup or backpressure is holding this pane".to_string());
        }
        if let Some(reason) = summary.fallback_reason.as_deref() {
            lines.push(format!("Fallback: {reason}"));
        }
    }
    if !active_entry.arrangement_memberships.is_empty() {
        lines.push(format!(
            "Arrangement: {}",
            active_entry.arrangement_memberships.join(", ")
        ));
    }
    lines
}

fn viewer_content_actions(
    chrome_projection: &WorkbenchChromeProjection,
) -> Vec<OverviewQuickAction> {
    let Some(active_entry) = active_overview_pane_entry(chrome_projection) else {
        return Vec::new();
    };

    let mut actions = Vec::new();
    match &active_entry.kind {
        WorkbenchPaneKind::Graph { .. } => actions.push(OverviewQuickAction {
            label: "Graph settings".to_string(),
            owner: OverviewActionOwner::Viewer,
            hover_text:
                "Route graph-view presentation settings through the existing settings URL path."
                    .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl {
                url: crate::util::VersoAddress::settings(
                    crate::util::GraphshellSettingsPath::Physics,
                )
                .to_string(),
            }),
        }),
        WorkbenchPaneKind::Node { node_key } => {
            actions.push(OverviewQuickAction {
                label: "Render auto".to_string(),
                owner: OverviewActionOwner::Viewer,
                hover_text:
                    "Clear any viewer override and let the Viewer registry choose the canonical renderer."
                        .to_string(),
                dispatch: OverviewQuickActionDispatch::Workbench(
                    WorkbenchIntent::SwapViewerBackend {
                        pane: active_entry.pane_id,
                        node: *node_key,
                        viewer_id_override: None,
                    },
                ),
            });
            if active_entry
                .node_viewer_summary
                .as_ref()
                .is_some_and(|summary| {
                    summary
                        .available_viewer_ids
                        .iter()
                        .any(|viewer_id| viewer_id == "viewer:middlenet")
                        && summary.viewer_override.as_deref() != Some("viewer:middlenet")
                })
            {
                actions.push(OverviewQuickAction {
                    label: "Use MiddleNet".to_string(),
                    owner: OverviewActionOwner::Viewer,
                    hover_text:
                        "Pin the active node to the embedded MiddleNet renderer instead of relying on auto selection."
                            .to_string(),
                    dispatch: OverviewQuickActionDispatch::Workbench(
                        WorkbenchIntent::SwapViewerBackend {
                            pane: active_entry.pane_id,
                            node: *node_key,
                            viewer_id_override: Some(ViewerId::new("viewer:middlenet")),
                        },
                    ),
                });
            }
            actions.push(OverviewQuickAction {
                label: "Viewer settings".to_string(),
                owner: OverviewActionOwner::Viewer,
                hover_text:
                    "Open settings through the canonical settings-route bridge instead of mutating viewer state directly."
                        .to_string(),
                dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl {
                    url: crate::util::VersoAddress::settings(
                        crate::util::GraphshellSettingsPath::General,
                    )
                    .to_string(),
                }),
            });
            if active_entry
                .node_viewer_summary
                .as_ref()
                .is_some_and(viewer_summary_needs_diagnostics)
            {
                actions.push(OverviewQuickAction {
                    label: "Inspect diagnostics".to_string(),
                    owner: OverviewActionOwner::Viewer,
                    hover_text:
                        "Open the shared diagnostics surface for the active pane's viewer fallback or degraded state."
                            .to_string(),
                    dispatch: OverviewQuickActionDispatch::Workbench(
                        WorkbenchIntent::OpenToolUrl {
                            url: crate::util::VersoAddress::tool("diagnostics", None)
                                .to_string(),
                        },
                    ),
                });
            }
        }
        WorkbenchPaneKind::Tool { .. } => actions.push(OverviewQuickAction {
            label: "Tool settings".to_string(),
            owner: OverviewActionOwner::Viewer,
            hover_text: "Open the shared settings surface for the current tool-hosted content."
                .to_string(),
            dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl {
                url: crate::util::VersoAddress::settings(
                    crate::util::GraphshellSettingsPath::General,
                )
                .to_string(),
            }),
        }),
    }
    actions
}

fn runtime_attention_lines(app: &GraphBrowserApp) -> Vec<String> {
    let health = app.history_health_summary();
    let mut lines = vec![format!(
        "History capture: {}",
        health.capture_status.as_str()
    )];
    if health.preview_mode_active {
        lines.push("History preview active: live runtime side effects suppressed".to_string());
    }
    if health.replay_in_progress {
        let cursor = health.replay_cursor.unwrap_or(0);
        let total = health.replay_total_steps.unwrap_or(0);
        lines.push(format!("Replay in progress: step {cursor}/{total}"));
    }
    if health.recent_traversal_append_failures > 0 {
        lines.push(format!(
            "Recent traversal append failures: {}",
            health.recent_traversal_append_failures
        ));
    }
    if let Some(error) = health.last_error.as_deref() {
        lines.push(format!("Last error: {error}"));
    }
    lines.push(format!("Trusted peers: {}", phase3_trusted_peers().len()));
    lines
}

fn runtime_attention_actions(app: &GraphBrowserApp) -> Vec<OverviewQuickAction> {
    let mut actions = vec![OverviewQuickAction {
        label: "Inspect diagnostics".to_string(),
        owner: OverviewActionOwner::Runtime,
        hover_text:
            "Route runtime attention into the diagnostics tool surface instead of opening a bespoke overview inspector."
                .to_string(),
        dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenToolUrl {
            url: crate::util::VersoAddress::tool("diagnostics", None).to_string(),
        }),
    }];
    let history_label = if app.history_health_summary().preview_mode_active {
        "Inspect history preview"
    } else {
        "Open history"
    };
    actions.push(OverviewQuickAction {
        label: history_label.to_string(),
        owner: OverviewActionOwner::Runtime,
        hover_text: "Route history/runtime state into the canonical settings/history surface."
            .to_string(),
        dispatch: OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl {
            url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
                .to_string(),
        }),
    });
    actions
}

fn active_overview_pane_entry(
    chrome_projection: &WorkbenchChromeProjection,
) -> Option<&WorkbenchPaneEntry> {
    chrome_projection
        .pane_entries
        .iter()
        .find(|entry| entry.is_active)
        .or_else(|| chrome_projection.pane_entries.first())
}

fn focus_authority_label(chrome_projection: &WorkbenchChromeProjection) -> &'static str {
    match active_overview_pane_entry(chrome_projection).map(|entry| &entry.kind) {
        Some(WorkbenchPaneKind::Graph { .. }) => "graph",
        Some(WorkbenchPaneKind::Node { .. }) => "node pane",
        Some(WorkbenchPaneKind::Tool { .. }) => "tool pane",
        None => "overview",
    }
}

fn pane_kind_summary_label(entry: &WorkbenchPaneEntry) -> String {
    match &entry.kind {
        WorkbenchPaneKind::Graph { .. } => "graph canvas".to_string(),
        WorkbenchPaneKind::Node { .. } => "node viewer".to_string(),
        WorkbenchPaneKind::Tool { kind } => format!("tool pane ({})", kind.title()),
    }
}

fn viewer_backend_summary(entry: &WorkbenchPaneEntry) -> String {
    let Some(summary) = entry.node_viewer_summary.as_ref() else {
        return pane_kind_summary_label(entry);
    };
    let viewer = summary
        .effective_viewer_id
        .as_deref()
        .unwrap_or("unresolved viewer");
    format!(
        "{viewer} · {}",
        viewer_render_mode_label(summary.render_mode)
    )
}

fn viewer_render_mode_label(render_mode: TileRenderMode) -> &'static str {
    match render_mode {
        TileRenderMode::CompositedTexture => "composited texture",
        TileRenderMode::NativeOverlay => "native overlay",
        TileRenderMode::EmbeddedEgui => "embedded egui",
        TileRenderMode::Placeholder => "placeholder",
    }
}

fn viewer_switch_reason_label(reason: ViewerSwitchReason) -> &'static str {
    match reason {
        ViewerSwitchReason::UserRequested => "user override",
        ViewerSwitchReason::RecoveryPromptAccepted => "recovery override",
        ViewerSwitchReason::PolicyPinned => "policy-selected",
    }
}

fn viewer_summary_needs_diagnostics(summary: &WorkbenchNodeViewerSummary) -> bool {
    summary.runtime_crashed || summary.runtime_blocked || summary.fallback_reason.is_some()
}

fn viewer_degraded_chip(summary: &WorkbenchNodeViewerSummary) -> Option<String> {
    if let Some(reason) = summary.fallback_reason.as_deref() {
        return Some(compact_overview_label(
            &format!("Viewer fallback: {reason}"),
            32,
        ));
    }
    if summary.runtime_crashed {
        return Some("Viewer crash recorded".to_string());
    }
    if summary.runtime_blocked {
        return Some("Viewer blocked".to_string());
    }
    None
}

fn node_summary_label(app: &GraphBrowserApp, node_key: crate::graph::NodeKey) -> String {
    app.domain_graph()
        .get_node(node_key)
        .map(|node| {
            let title = node.title.trim();
            if title.is_empty() {
                node.url().to_string()
            } else {
                title.to_string()
            }
        })
        .unwrap_or_else(|| format!("Node {node_key:?}"))
}

fn graph_search_origin_label(origin: &GraphSearchOrigin) -> &'static str {
    match origin {
        GraphSearchOrigin::Manual => "manual scope",
        GraphSearchOrigin::SemanticTag => "semantic-tag scope",
        GraphSearchOrigin::AnchorSlice => "anchor-slice scope",
    }
}

fn active_graphlet_roster_summary(roster: &[GraphletRosterEntry]) -> Option<String> {
    if roster.is_empty() {
        return None;
    }

    let warm_count = roster.iter().filter(|entry| !entry.is_cold).count();
    let cold_count = roster.iter().filter(|entry| entry.is_cold).count();
    let mut parts = Vec::new();
    if warm_count > 0 {
        parts.push(format!("{warm_count} warm node(s)"));
    }
    if cold_count > 0 {
        parts.push(format!("{cold_count} cold node(s)"));
    }
    if parts.is_empty() {
        parts.push("0 related node(s)".to_string());
    }

    Some(format!("Active pane graphlet: {}", parts.join(" · ")))
}

fn active_graphlet_frontier_summary(roster: &[GraphletRosterEntry]) -> Option<String> {
    let cold_titles: Vec<_> = roster
        .iter()
        .filter(|entry| entry.is_cold)
        .map(|entry| compact_overview_label(&entry.title, 18))
        .collect();
    if cold_titles.is_empty() {
        return None;
    }

    let preview: Vec<_> = cold_titles.iter().take(2).cloned().collect();
    let remainder = cold_titles.len().saturating_sub(preview.len());
    let mut line = format!("Frontier ready to open: {}", preview.join(", "));
    if remainder > 0 {
        line.push_str(&format!(" +{remainder} more"));
    }
    Some(line)
}

fn active_workbench_binding_summary(
    chrome_projection: &WorkbenchChromeProjection,
) -> Option<String> {
    let active_entry = active_overview_pane_entry(chrome_projection)?;
    if let Some(semantic_summary) =
        semantic_tab_affordance_summary(active_entry.semantic_tab_affordance)
    {
        return Some(semantic_summary);
    }
    if !active_entry.arrangement_memberships.is_empty() {
        return Some(format!(
            "arranged in {}",
            active_entry.arrangement_memberships.join(", ")
        ));
    }
    None
}

fn semantic_tab_affordance_summary(affordance: Option<SemanticTabAffordance>) -> Option<String> {
    match affordance {
        Some(SemanticTabAffordance::Collapse { member_count, .. }) => Some(format!(
            "linked semantic tab group ({member_count} pane(s))"
        )),
        Some(SemanticTabAffordance::Restore { member_count, .. }) => Some(format!(
            "detached from semantic tab group ({member_count} pane(s))"
        )),
        None => None,
    }
}

fn overview_action_owner_label(owner: OverviewActionOwner) -> &'static str {
    match owner {
        OverviewActionOwner::Graph => "Graph",
        OverviewActionOwner::Workbench => "Workbench",
        OverviewActionOwner::Viewer => "Viewer",
        OverviewActionOwner::Runtime => "Runtime",
    }
}

fn overview_action_owner_summary(actions: &[OverviewQuickAction]) -> String {
    let mut owners = Vec::new();
    for action in actions {
        let owner = overview_action_owner_label(action.owner);
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
    owners.join(" · ")
}

fn overview_suggestion_labels(
    has_selected_view: bool,
    transfer_enabled: bool,
    archived_count: usize,
    preview_mode_active: bool,
) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if has_selected_view {
        labels.push("Focus or open the selected region.");
    } else {
        labels.push("Select a region to unlock context-aware actions.");
    }
    if transfer_enabled {
        labels.push("Transfer the focused selection into the selected region.");
    }
    if archived_count > 0 {
        labels.push("Review archived regions before creating more layout sprawl.");
    }
    if preview_mode_active {
        labels.push("Return to present before relying on live runtime status.");
    }
    labels
}

pub(crate) fn sorted_slot_snapshots(app: &GraphBrowserApp) -> Vec<OverviewSlotSnapshot> {
    let mut slots: Vec<_> = app
        .workspace
        .graph_runtime
        .graph_view_layout_manager
        .slots
        .values()
        .map(|slot| OverviewSlotSnapshot {
            view_id: slot.view_id,
            name: slot.name.clone(),
            row: slot.row,
            col: slot.col,
            archived: slot.archived,
        })
        .collect();
    slots.sort_by(|left, right| {
        left.archived
            .cmp(&right.archived)
            .then(left.row.cmp(&right.row))
            .then(left.col.cmp(&right.col))
            .then(left.name.cmp(&right.name))
    });
    slots
}

pub(crate) fn selected_overview_view_id(
    app: &GraphBrowserApp,
    slots: &[OverviewSlotSnapshot],
) -> Option<GraphViewId> {
    app.workspace
        .graph_runtime
        .focused_view
        .filter(|view_id| slots.iter().any(|slot| slot.view_id == *view_id))
        .or_else(|| {
            slots
                .iter()
                .find(|slot| !slot.archived)
                .map(|slot| slot.view_id)
        })
        .or_else(|| slots.first().map(|slot| slot.view_id))
}

fn overview_window_pos(app: &GraphBrowserApp) -> Pos2 {
    if let Some(view_id) = app.workspace.graph_runtime.focused_view
        && let Some(rect) = app
            .workspace
            .graph_runtime
            .graph_view_canvas_rects
            .get(&view_id)
    {
        return Pos2::new(rect.left() + 24.0, rect.top() + 24.0);
    }

    app.workspace
        .graph_runtime
        .graph_view_canvas_rects
        .values()
        .next()
        .map(|rect| Pos2::new(rect.left() + 24.0, rect.top() + 24.0))
        .unwrap_or_else(|| Pos2::new(48.0, 96.0))
}

pub(crate) fn render_navigator_overview_swatch(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    chrome_projection: &WorkbenchChromeProjection,
) -> Vec<NavigatorOverviewAction> {
    let slots = sorted_slot_snapshots(app);
    let active_slots: Vec<_> = slots
        .iter()
        .filter(|slot| !slot.archived)
        .cloned()
        .collect();
    let archived_count = slots.iter().filter(|slot| slot.archived).count();
    let selected_view_id = selected_overview_view_id(app, &slots);
    let selected_slot = slots
        .iter()
        .find(|slot| Some(slot.view_id) == selected_view_id);
    let mut actions = Vec::new();
    let show_archived_id = egui::Id::new("navigator_overview_show_archived");
    let mut show_archived = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<bool>(show_archived_id))
        .unwrap_or(false);
    let swatch_enabled = navigator_overview_swatch_enabled(ui.available_width());

    ui.horizontal(|ui| {
        ui.label(RichText::new("Views").small().strong());
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{} active · {} archived",
                active_slots.len(),
                archived_count
            ))
            .small()
            .weak(),
        );
        if archived_count > 0 {
            ui.separator();
            let archived_label = if show_archived {
                "Hide archived"
            } else {
                "Show archived"
            };
            if ui
                .small_button(archived_label)
                .on_hover_text("Toggle archived graph views in the Navigator list")
                .clicked()
            {
                show_archived = !show_archived;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let manage_label = if app.graph_view_layout_manager_active() {
                "Manage*"
            } else {
                "Manage"
            };
            if ui
                .small_button(manage_label)
                .on_hover_text("Open the full Overview Plane")
                .clicked()
            {
                actions.push(OverviewSurfaceAction::ToggleOverviewPlane.into());
            }
        });
    });
    ui.ctx()
        .data_mut(|data| data.insert_persisted(show_archived_id, show_archived));

    render_compact_overview_context_bar(
        ui,
        &compact_overview_chips(app, chrome_projection, selected_slot, archived_count),
    );

    if active_slots.is_empty() {
        ui.small("No active graph views yet.");
        return actions;
    }

    let list_slots: Vec<_> = if show_archived {
        slots.iter().collect()
    } else {
        active_slots.iter().collect()
    };
    ui.vertical(|ui| {
        for slot in list_slots {
            ui.horizontal_wrapped(|ui| {
                let label = if Some(slot.view_id) == selected_view_id {
                    RichText::new(&slot.name).small().strong()
                } else {
                    RichText::new(&slot.name).small()
                };
                if ui
                    .selectable_label(Some(slot.view_id) == selected_view_id, label)
                    .clicked()
                {
                    actions.push(OverviewSurfaceAction::FocusView(slot.view_id).into());
                }
                if ui.small_button("Open").clicked() {
                    actions.push(OverviewSurfaceAction::OpenView(slot.view_id).into());
                }

                let mut hints = vec![format!("r{} · c{}", slot.row, slot.col)];
                if slot.archived {
                    hints.push("archived".to_string());
                }
                if let Some(node_count) = app.graph_view_owned_node_count(slot.view_id) {
                    hints.push(format!("{node_count} nodes"));
                }
                let external_links = app.graph_view_external_link_count(slot.view_id);
                if external_links > 0 {
                    hints.push(format!("{external_links} x-links"));
                }
                ui.label(RichText::new(hints.join(" · ")).small().weak());
            });
        }
    });

    if swatch_enabled {
        ui.add_space(6.0);
        render_compact_overview_grid(ui, app, &active_slots, selected_view_id, &mut actions);
    }

    if let Some(selected_slot) = selected_slot {
        ui.horizontal_wrapped(|ui| {
            let mut summary = vec![format!(
                "Focused: {} (r{} · c{})",
                selected_slot.name, selected_slot.row, selected_slot.col
            )];
            if let Some(node_count) = app.graph_view_owned_node_count(selected_slot.view_id) {
                summary.push(format!("{node_count} nodes"));
            }
            ui.label(RichText::new(summary.join(" · ")).small().weak());
            if ui.small_button("Open").clicked() {
                actions.push(OverviewSurfaceAction::OpenView(selected_slot.view_id).into());
            }
        });

        ui.add_space(6.0);
        if swatch_enabled {
            render_navigator_graphlet_cards(ui, app, selected_slot, &mut actions);
        } else {
            ui.label(
                RichText::new("Graphlet swatches appear when the Navigator host is wide enough.")
                    .small()
                    .weak(),
            );
        }
    }

    actions
}

fn render_compact_overview_context_bar(ui: &mut Ui, chips: &[String]) {
    if chips.is_empty() {
        return;
    }

    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, chip) in chips.iter().enumerate() {
                    ui.label(RichText::new(chip).small().weak());
                    if index + 1 < chips.len() {
                        ui.separator();
                    }
                }
            });
        });
    ui.add_space(6.0);
}

fn compact_overview_chips(
    app: &GraphBrowserApp,
    chrome_projection: &WorkbenchChromeProjection,
    selected_slot: Option<&OverviewSlotSnapshot>,
    archived_count: usize,
) -> Vec<String> {
    let mut chips = vec![compact_overview_label(
        &active_context_summary(app, chrome_projection, selected_slot),
        40,
    )];
    chips.push(format!("Panes: {}", chrome_projection.pane_entries.len()));
    if !chrome_projection.saved_frame_names.is_empty() {
        chips.push(format!(
            "Saved: {}",
            chrome_projection.saved_frame_names.len()
        ));
    }
    if let Some(summary) = active_graphlet_roster_summary(&chrome_projection.active_graphlet_roster)
    {
        chips.push(compact_overview_label(&summary, 32));
    }
    if let Some(binding) = active_workbench_binding_summary(chrome_projection) {
        chips.push(compact_overview_label(&format!("Tabs: {binding}"), 32));
    }
    if let Some(summary) = active_overview_pane_entry(chrome_projection)
        .and_then(|entry| entry.node_viewer_summary.as_ref())
        .and_then(viewer_degraded_chip)
    {
        chips.push(summary);
    }
    let health = app.history_health_summary();
    if health.preview_mode_active {
        chips.push("History preview".to_string());
    } else {
        chips.push(format!("History: {}", health.capture_status.as_str()));
    }
    if archived_count > 0 {
        chips.push(format!("Archived: {archived_count}"));
    }
    chips
}

fn render_navigator_graphlet_cards(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    selected_slot: &OverviewSlotSnapshot,
    actions: &mut Vec<NavigatorOverviewAction>,
) {
    let partitions = app.graphlet_partitions_for_view(selected_slot.view_id);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Graphlets").small().strong());
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{} visible in {}",
                partitions.len(),
                compact_overview_label(&selected_slot.name, 20)
            ))
            .small()
            .weak(),
        );
    });

    if partitions.is_empty() {
        ui.small("No graphlets are visible under this view's current ownership and filter policy.");
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt((
            "navigator_overview_graphlets",
            selected_slot.view_id.as_uuid(),
        ))
        .max_height(NAVIGATOR_GRAPHLET_SWATCH_MAX_HEIGHT)
        .show(ui, |ui| {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                ui.horizontal_wrapped(|ui| {
                    for partition in &partitions {
                        render_navigator_graphlet_card(
                            ui,
                            app,
                            selected_slot.view_id,
                            partition,
                            actions,
                        );
                    }
                });
            });
        });
}

fn render_navigator_graphlet_card(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    partition: &ViewGraphletPartition,
    actions: &mut Vec<NavigatorOverviewAction>,
) {
    let title = compact_overview_label(&node_summary_label(app, partition.anchor), 20);
    let contains_primary = app
        .focused_selection()
        .primary()
        .is_some_and(|primary| partition.members.contains(&primary));
    let preview_nodes = partition
        .members
        .len()
        .min(NAVIGATOR_GRAPHLET_PREVIEW_NODE_LIMIT);
    let footer = if partition.members.len() > preview_nodes {
        format!(
            "+{} more member(s)",
            partition.members.len() - preview_nodes
        )
    } else if partition.members.len() == 1 {
        "singleton graphlet".to_string()
    } else {
        "preview of projected topology".to_string()
    };
    let swatch = GraphSwatchSpec {
        source_scope: SwatchSourceScope::Graphlet,
        layout_profile: SwatchLayoutProfile::LocalNeighborhood,
        density_policy: SwatchDensityPolicy {
            preview_node_limit: NAVIGATOR_GRAPHLET_PREVIEW_NODE_LIMIT,
            show_counts: true,
        },
        interaction_profile: SwatchInteractionProfile::SelectAndOpenDetail,
        host_options: SwatchHostOptions {
            size_class: SwatchSizeClass::Compact,
            card_size: Vec2::new(156.0, 128.0),
            preview_height: 74.0,
        },
        graphlet: partition,
        title,
        summary: format!(
            "{} nodes · {} edges",
            partition.members.len(),
            partition.internal_edges.len()
        ),
        badge: contains_primary.then_some("active"),
        footer,
        emphasized: contains_primary,
        hover_text: Some(
            "Click to select the anchor. Double-click to open a component specialty view.",
        ),
    };

    match render_graph_swatch_card(ui, &swatch) {
        Some(GraphSwatchInteraction::SelectSource) => {
            actions.push(NavigatorOverviewAction::SelectGraphletAnchor {
                view_id,
                node_key: partition.anchor,
            });
        }
        Some(GraphSwatchInteraction::OpenDetail) => {
            actions.push(NavigatorOverviewAction::OpenGraphletSpecialty {
                view_id,
                node_key: partition.anchor,
                kind: GraphletKind::Component,
            });
        }
        None => {}
    }
}

fn render_compact_overview_grid(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
    actions: &mut Vec<NavigatorOverviewAction>,
) {
    let Some((min_row, max_row, min_col, max_col)) = overview_grid_bounds(slots) else {
        return;
    };

    let rows = (max_row - min_row + 1).max(1) as f32;
    let cols = (max_col - min_col + 1).max(1) as f32;
    let available_width = ui.available_width().max(140.0);
    let cell_width =
        ((available_width - (cols - 1.0) * OVERVIEW_SWATCH_GAP) / cols).clamp(42.0, 76.0);
    let cell_height = (cell_width * 0.58).clamp(24.0, 44.0);
    let drag_transfer_enabled = navigator_overview_drag_transfer_enabled(cell_width);
    let grid_size = Vec2::new(
        cols * cell_width + (cols - 1.0) * OVERVIEW_SWATCH_GAP,
        rows * cell_height + (rows - 1.0) * OVERVIEW_SWATCH_GAP,
    );
    let (grid_rect, _) = ui.allocate_exact_size(grid_size, Sense::hover());
    let painter = ui.painter();
    let drag_source_id = egui::Id::new(NAVIGATOR_OVERVIEW_DRAG_SOURCE_VIEW_KEY);
    let mut drag_source_view = ui
        .ctx()
        .data_mut(|data| data.get_temp::<GraphViewId>(drag_source_id));
    let pointer_pos = ui.input(|input| input.pointer.interact_pos());
    let pointer_released = ui.input(|input| input.pointer.any_released());
    let mut cell_rects = Vec::with_capacity(slots.len());

    for slot in slots {
        let cell_rect = compact_slot_rect_for_coords(
            slot.row,
            slot.col,
            min_row,
            min_col,
            grid_rect.min,
            Vec2::new(cell_width, cell_height),
        );
        cell_rects.push((slot.view_id, cell_rect));
        let response = ui.interact(
            cell_rect,
            egui::Id::new(("navigator_overview_slot", slot.view_id.as_uuid())),
            Sense::click_and_drag(),
        );
        let is_selected = Some(slot.view_id) == selected_view_id;
        let is_drag_source = drag_source_view == Some(slot.view_id);
        let is_drop_target = drag_source_view.is_some_and(|source_view| {
            source_view != slot.view_id
                && pointer_pos.is_some_and(|pointer| cell_rect.contains(pointer))
                && navigator_overview_transfer_action(app, source_view, slot.view_id).is_some()
        });
        let fill = if is_drag_source {
            Color32::from_rgb(78, 88, 56)
        } else if is_selected {
            Color32::from_rgb(66, 88, 120)
        } else {
            Color32::from_rgb(40, 45, 54)
        };
        let stroke = if is_drop_target {
            Stroke::new(2.0, Color32::from_rgb(120, 210, 180))
        } else if is_selected {
            Stroke::new(2.0, Color32::from_rgb(180, 210, 255))
        } else {
            Stroke::new(1.0, Color32::from_gray(100))
        };
        painter.rect_filled(cell_rect, 6.0, fill);
        painter.rect_stroke(cell_rect, 6.0, stroke, StrokeKind::Outside);
        painter.text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            compact_overview_label(&slot.name, if cell_width >= 60.0 { 12 } else { 6 }),
            egui::TextStyle::Small.resolve(ui.style()),
            Color32::WHITE,
        );

        if response.clicked() {
            actions.push(OverviewSurfaceAction::FocusView(slot.view_id).into());
        }
        if response.double_clicked() {
            actions.push(OverviewSurfaceAction::OpenView(slot.view_id).into());
        }
        if response.drag_started()
            && drag_transfer_enabled
            && navigator_overview_drag_source_view(app) == Some(slot.view_id)
        {
            drag_source_view = Some(slot.view_id);
            ui.ctx()
                .data_mut(|data| data.insert_temp(drag_source_id, slot.view_id));
        }
    }

    if pointer_released {
        if let Some(source_view) = drag_source_view
            && let Some(pointer) = pointer_pos
            && let Some((destination_view, _)) = cell_rects
                .iter()
                .find(|(view_id, rect)| *view_id != source_view && rect.contains(pointer))
            && let Some(action) =
                navigator_overview_transfer_action(app, source_view, *destination_view)
        {
            actions.push(action.into());
        }
        ui.ctx()
            .data_mut(|data| data.remove::<GraphViewId>(drag_source_id));
    }
}

fn render_overview_grid(
    ui: &mut Ui,
    ctx: &Context,
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
    pending_graph_intents: &mut Vec<GraphIntent>,
    pending_surface_actions: &mut Vec<OverviewSurfaceAction>,
) {
    ui.label(RichText::new("View regions").strong());
    if slots.is_empty() {
        ui.small("No active graph views yet.");
        return;
    }

    let Some((min_row, max_row, min_col, max_col)) = overview_grid_bounds(slots) else {
        ui.small("No active graph views yet.");
        return;
    };

    let rows = (max_row - min_row + 1).max(1) as f32;
    let cols = (max_col - min_col + 1).max(1) as f32;
    let grid_size = Vec2::new(
        cols * OVERVIEW_CELL_SIZE.x + (cols - 1.0) * OVERVIEW_CELL_GAP,
        rows * OVERVIEW_CELL_SIZE.y + (rows - 1.0) * OVERVIEW_CELL_GAP,
    );
    let (grid_rect, _) = ui.allocate_exact_size(grid_size, Sense::hover());
    let painter = ui.painter();

    for slot in slots {
        let cell_rect = slot_rect(slot, min_row, min_col, grid_rect.min);
        let response = ui.interact(
            cell_rect,
            egui::Id::new(("overview_plane_slot", slot.view_id.as_uuid())),
            Sense::click_and_drag(),
        );
        let is_selected = Some(slot.view_id) == selected_view_id;
        let fill = if is_selected {
            Color32::from_rgb(66, 88, 120)
        } else {
            Color32::from_rgb(42, 48, 60)
        };
        let stroke = if is_selected {
            Stroke::new(2.0, Color32::from_rgb(180, 210, 255))
        } else {
            Stroke::new(1.0, Color32::from_gray(110))
        };
        painter.rect_filled(cell_rect, 8.0, fill);
        painter.rect_stroke(cell_rect, 8.0, stroke, StrokeKind::Outside);
        painter.text(
            cell_rect.left_top() + Vec2::new(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            &slot.name,
            egui::TextStyle::Button.resolve(ui.style()),
            Color32::WHITE,
        );
        painter.text(
            cell_rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("r{} · c{}", slot.row, slot.col),
            egui::TextStyle::Small.resolve(ui.style()),
            Color32::from_gray(220),
        );

        if response.clicked() {
            set_overview_surface_selected_view_id(ctx, Some(slot.view_id));
            pending_surface_actions.push(OverviewSurfaceAction::FocusView(slot.view_id));
        }
        if response.double_clicked() {
            pending_surface_actions.push(OverviewSurfaceAction::OpenView(slot.view_id));
        }
        if response.drag_stopped() {
            let (target_row, target_col) = drag_target_slot_position(slot, response.drag_delta());
            if target_row != slot.row || target_col != slot.col {
                pending_graph_intents.push(GraphIntent::MoveGraphViewSlot {
                    view_id: slot.view_id,
                    row: target_row,
                    col: target_col,
                });
            }
        }
        if response.dragged() {
            let (target_row, target_col) = drag_target_slot_position(slot, response.drag_delta());
            if target_row != slot.row || target_col != slot.col {
                let preview_rect =
                    slot_rect_for_coords(target_row, target_col, min_row, min_col, grid_rect.min);
                painter.rect_stroke(
                    preview_rect.expand(2.0),
                    10.0,
                    Stroke::new(2.0, Color32::from_rgb(120, 210, 180)),
                    StrokeKind::Outside,
                );
            }
        }
    }
}

fn render_overview_details(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    ctx: &Context,
    selected_slot: Option<&OverviewSlotSnapshot>,
    archived_slots: &[OverviewSlotSnapshot],
    pending_graph_intents: &mut Vec<GraphIntent>,
    pending_surface_actions: &mut Vec<OverviewSurfaceAction>,
) {
    ui.label(RichText::new("Details").strong());
    let Some(slot) = selected_slot else {
        ui.small("Select a graph view region to inspect it.");
        return;
    };

    let rename_id = egui::Id::new(("overview_plane_rename", slot.view_id.as_uuid()));
    let mut rename_draft = ctx
        .data_mut(|data| data.get_persisted::<String>(rename_id))
        .unwrap_or_else(|| slot.name.clone());

    ui.label(format!("Selected view: {}", slot.name));
    ui.small(format!("Slot position: row {}, col {}", slot.row, slot.col));
    if let Some(node_count) = app.graph_view_owned_node_count(slot.view_id) {
        ui.small(format!("Owned nodes: {node_count}"));
    }
    ui.small(
        "Keyboard: Arrow = select, Space = focus, Enter = open, Ctrl+Enter = transfer, Alt+Arrow = move, Ctrl+Shift+Arrow = create adjacent.",
    );
    ui.add_space(6.0);

    let rename_response = ui.text_edit_singleline(&mut rename_draft);
    ctx.data_mut(|data| data.insert_persisted(rename_id, rename_draft.clone()));
    if (rename_response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
        || ui.button("Rename").clicked()
    {
        let trimmed = rename_draft.trim();
        if !trimmed.is_empty() && trimmed != slot.name {
            pending_graph_intents.push(GraphIntent::RenameGraphViewSlot {
                view_id: slot.view_id,
                name: trimmed.to_string(),
            });
        }
    }

    ui.horizontal(|ui| {
        if ui.button("Open").clicked() {
            pending_surface_actions.push(OverviewSurfaceAction::OpenView(slot.view_id));
        }
        if ui.button("Focus").clicked() {
            pending_surface_actions.push(OverviewSurfaceAction::FocusView(slot.view_id));
        }
        if ui.button("Archive").clicked() {
            pending_graph_intents.push(GraphIntent::ArchiveGraphViewSlot {
                view_id: slot.view_id,
            });
        }
    });

    ui.separator();
    ui.small("Transfer focused selection");
    let transfer_affordance = overview_transfer_affordance(app, slot.view_id);
    let move_button = ui.add_enabled(
        transfer_affordance.enabled,
        egui::Button::new(format!(
            "Move {} selected node(s) here",
            transfer_affordance.selection_count
        )),
    );
    let move_button = if transfer_affordance.enabled {
        move_button.on_hover_text("Transfer the focused selection into this graph view")
    } else {
        move_button.on_disabled_hover_text(transfer_affordance.disabled_reason)
    };
    if move_button.clicked()
        && let Some(action) = overview_transfer_action(app, slot.view_id)
    {
        pending_surface_actions.push(action);
    }

    ui.separator();
    ui.small("Move slot");
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "Left",
            GraphViewLayoutDirection::Left,
            slot,
            pending_graph_intents,
            false,
        );
        directional_button(
            ui,
            "Right",
            GraphViewLayoutDirection::Right,
            slot,
            pending_graph_intents,
            false,
        );
    });
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "Up",
            GraphViewLayoutDirection::Up,
            slot,
            pending_graph_intents,
            false,
        );
        directional_button(
            ui,
            "Down",
            GraphViewLayoutDirection::Down,
            slot,
            pending_graph_intents,
            false,
        );
    });

    ui.separator();
    ui.small("Create adjacent view");
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "+ Left",
            GraphViewLayoutDirection::Left,
            slot,
            pending_graph_intents,
            true,
        );
        directional_button(
            ui,
            "+ Right",
            GraphViewLayoutDirection::Right,
            slot,
            pending_graph_intents,
            true,
        );
    });
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "+ Up",
            GraphViewLayoutDirection::Up,
            slot,
            pending_graph_intents,
            true,
        );
        directional_button(
            ui,
            "+ Down",
            GraphViewLayoutDirection::Down,
            slot,
            pending_graph_intents,
            true,
        );
    });

    if !archived_slots.is_empty() {
        ui.separator();
        ui.collapsing("Archived views", |ui| {
            for archived in archived_slots {
                ui.horizontal(|ui| {
                    ui.label(&archived.name);
                    if ui.button("Restore").clicked() {
                        pending_graph_intents.push(GraphIntent::RestoreGraphViewSlot {
                            view_id: archived.view_id,
                            row: archived.row,
                            col: archived.col,
                        });
                    }
                });
            }
        });
    }
}

fn directional_button(
    ui: &mut Ui,
    label: &str,
    direction: GraphViewLayoutDirection,
    slot: &OverviewSlotSnapshot,
    pending_intents: &mut Vec<GraphIntent>,
    create: bool,
) {
    if ui.button(label).clicked() {
        if create {
            pending_intents.push(GraphIntent::CreateGraphViewSlot {
                anchor_view: Some(slot.view_id),
                direction,
                open_mode: Some(PendingTileOpenMode::Tab),
            });
        } else {
            let (row, col) = shifted_slot_position(slot.row, slot.col, direction);
            pending_intents.push(GraphIntent::MoveGraphViewSlot {
                view_id: slot.view_id,
                row,
                col,
            });
        }
    }
}

fn overview_grid_bounds(slots: &[OverviewSlotSnapshot]) -> Option<(i32, i32, i32, i32)> {
    let mut iter = slots.iter();
    let first = iter.next()?;
    let mut min_row = first.row;
    let mut max_row = first.row;
    let mut min_col = first.col;
    let mut max_col = first.col;
    for slot in iter {
        min_row = min_row.min(slot.row);
        max_row = max_row.max(slot.row);
        min_col = min_col.min(slot.col);
        max_col = max_col.max(slot.col);
    }
    Some((min_row, max_row, min_col, max_col))
}

fn slot_rect(slot: &OverviewSlotSnapshot, min_row: i32, min_col: i32, origin: Pos2) -> egui::Rect {
    slot_rect_for_coords(slot.row, slot.col, min_row, min_col, origin)
}

fn slot_rect_for_coords(
    row: i32,
    col: i32,
    min_row: i32,
    min_col: i32,
    origin: Pos2,
) -> egui::Rect {
    let x = origin.x + (col - min_col) as f32 * (OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP);
    let y = origin.y + (row - min_row) as f32 * (OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP);
    egui::Rect::from_min_size(Pos2::new(x, y), OVERVIEW_CELL_SIZE)
}

fn compact_slot_rect_for_coords(
    row: i32,
    col: i32,
    min_row: i32,
    min_col: i32,
    origin: Pos2,
    cell_size: Vec2,
) -> egui::Rect {
    let x = origin.x + (col - min_col) as f32 * (cell_size.x + OVERVIEW_SWATCH_GAP);
    let y = origin.y + (row - min_row) as f32 * (cell_size.y + OVERVIEW_SWATCH_GAP);
    egui::Rect::from_min_size(Pos2::new(x, y), cell_size)
}

fn compact_overview_label(name: &str, max_chars: usize) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "View".to_string();
    }
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut compact: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    compact.push('…');
    compact
}

fn navigator_overview_swatch_enabled(available_width: f32) -> bool {
    available_width >= NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH
}

fn navigator_overview_drag_transfer_enabled(cell_width: f32) -> bool {
    cell_width >= NAVIGATOR_OVERVIEW_TRANSFER_CELL_MIN_WIDTH
}

fn navigator_overview_drag_source_view(app: &GraphBrowserApp) -> Option<GraphViewId> {
    let source_view = app.workspace.graph_runtime.focused_view?;
    let source_slot = app
        .workspace
        .graph_runtime
        .graph_view_layout_manager
        .slots
        .get(&source_view)?;
    if source_slot.archived || app.focused_selection().is_empty() {
        return None;
    }
    let source_state = app.workspace.graph_runtime.views.get(&source_view)?;
    if source_state.graphlet_node_mask.is_some() {
        return None;
    }
    Some(source_view)
}

fn navigator_overview_transfer_action(
    app: &GraphBrowserApp,
    source_view: GraphViewId,
    destination_view: GraphViewId,
) -> Option<OverviewSurfaceAction> {
    if navigator_overview_drag_source_view(app) != Some(source_view) {
        return None;
    }
    match overview_transfer_intent(app, destination_view)? {
        GraphIntent::TransferSelectedNodesToGraphView {
            source_view: intent_source,
            destination_view: intent_destination,
        } if intent_source == source_view && intent_destination == destination_view => {
            Some(OverviewSurfaceAction::TransferSelectionToView {
                source_view,
                destination_view,
            })
        }
        _ => None,
    }
}

fn overview_surface_selected_view_id(
    ctx: &Context,
    app: &GraphBrowserApp,
    slots: &[OverviewSlotSnapshot],
) -> Option<GraphViewId> {
    let stored = ctx.data_mut(|data| {
        data.get_persisted::<GraphViewId>(egui::Id::new(OVERVIEW_SELECTED_VIEW_ID_KEY))
    });
    let selected = stored
        .filter(|view_id| slots.iter().any(|slot| slot.view_id == *view_id))
        .or_else(|| selected_overview_view_id(app, slots));
    if let Some(view_id) = selected {
        set_overview_surface_selected_view_id(ctx, Some(view_id));
    }
    selected
}

fn set_overview_surface_selected_view_id(ctx: &Context, selected_view_id: Option<GraphViewId>) {
    if let Some(view_id) = selected_view_id {
        ctx.data_mut(|data| {
            data.insert_persisted(egui::Id::new(OVERVIEW_SELECTED_VIEW_ID_KEY), view_id)
        });
    }
}

fn overview_slot_for_view(
    slots: &[OverviewSlotSnapshot],
    view_id: GraphViewId,
) -> Option<&OverviewSlotSnapshot> {
    slots.iter().find(|slot| slot.view_id == view_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OverviewTransferAffordance {
    enabled: bool,
    source_view: Option<GraphViewId>,
    selection_count: usize,
    disabled_reason: &'static str,
}

fn overview_transfer_affordance(
    app: &GraphBrowserApp,
    destination_view: GraphViewId,
) -> OverviewTransferAffordance {
    let source_view = app.workspace.graph_runtime.focused_view;
    let selection_count = app.focused_selection().len();
    if source_view.is_none() {
        return OverviewTransferAffordance {
            enabled: false,
            source_view,
            selection_count,
            disabled_reason: "Focus a source graph view first.",
        };
    }
    if selection_count == 0 {
        return OverviewTransferAffordance {
            enabled: false,
            source_view,
            selection_count,
            disabled_reason: "Select one or more nodes in the focused graph view first.",
        };
    }
    if source_view == Some(destination_view) {
        return OverviewTransferAffordance {
            enabled: false,
            source_view,
            selection_count,
            disabled_reason: "Selected view already owns the focused selection.",
        };
    }
    OverviewTransferAffordance {
        enabled: true,
        source_view,
        selection_count,
        disabled_reason: "Transfer the focused selection into this graph view.",
    }
}

fn overview_transfer_intent(
    app: &GraphBrowserApp,
    destination_view: GraphViewId,
) -> Option<GraphIntent> {
    let affordance = overview_transfer_affordance(app, destination_view);
    affordance
        .enabled
        .then_some(GraphIntent::TransferSelectedNodesToGraphView {
            source_view: affordance.source_view?,
            destination_view,
        })
}

fn overview_transfer_action(
    app: &GraphBrowserApp,
    destination_view: GraphViewId,
) -> Option<OverviewSurfaceAction> {
    let affordance = overview_transfer_affordance(app, destination_view);
    affordance
        .enabled
        .then_some(OverviewSurfaceAction::TransferSelectionToView {
            source_view: affordance.source_view?,
            destination_view,
        })
}

fn next_overview_selected_view_id(
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
    direction: GraphViewLayoutDirection,
) -> Option<GraphViewId> {
    let selected_view_id = selected_view_id.or_else(|| slots.first().map(|slot| slot.view_id))?;
    let current = overview_slot_for_view(slots, selected_view_id)?;
    let next = slots
        .iter()
        .filter(|slot| slot.view_id != current.view_id)
        .filter_map(|slot| match direction {
            GraphViewLayoutDirection::Left if slot.col < current.col => Some((
                (
                    current.col - slot.col,
                    (slot.row - current.row).abs(),
                    slot.row,
                    slot.col,
                ),
                slot.view_id,
            )),
            GraphViewLayoutDirection::Right if slot.col > current.col => Some((
                (
                    slot.col - current.col,
                    (slot.row - current.row).abs(),
                    slot.row,
                    slot.col,
                ),
                slot.view_id,
            )),
            GraphViewLayoutDirection::Up if slot.row < current.row => Some((
                (
                    current.row - slot.row,
                    (slot.col - current.col).abs(),
                    slot.col,
                    slot.row,
                ),
                slot.view_id,
            )),
            GraphViewLayoutDirection::Down if slot.row > current.row => Some((
                (
                    slot.row - current.row,
                    (slot.col - current.col).abs(),
                    slot.col,
                    slot.row,
                ),
                slot.view_id,
            )),
            _ => None,
        })
        .min_by_key(|(key, _)| *key);
    next.map(|(_, view_id)| view_id).or(Some(selected_view_id))
}

fn collect_overview_keyboard_intents(
    ctx: &Context,
    app: &GraphBrowserApp,
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
) -> (
    Option<GraphViewId>,
    Vec<GraphIntent>,
    Vec<OverviewSurfaceAction>,
) {
    let mut selected_view_id = selected_view_id;
    let mut graph_intents = Vec::new();
    let mut surface_actions = Vec::new();
    if slots.is_empty() || ctx.wants_keyboard_input() {
        return (selected_view_id, graph_intents, surface_actions);
    }

    ctx.input(|input| {
        let ctrl = input.modifiers.ctrl || input.modifiers.command;
        let shift = input.modifiers.shift;
        let alt = input.modifiers.alt;
        for (key, direction) in [
            (Key::ArrowLeft, GraphViewLayoutDirection::Left),
            (Key::ArrowRight, GraphViewLayoutDirection::Right),
            (Key::ArrowUp, GraphViewLayoutDirection::Up),
            (Key::ArrowDown, GraphViewLayoutDirection::Down),
        ] {
            if !input.key_pressed(key) {
                continue;
            }

            if ctrl && shift {
                if let Some(view_id) = selected_view_id {
                    graph_intents.push(GraphIntent::CreateGraphViewSlot {
                        anchor_view: Some(view_id),
                        direction,
                        open_mode: Some(PendingTileOpenMode::Tab),
                    });
                }
                continue;
            }

            if alt {
                if let Some(slot) =
                    selected_view_id.and_then(|view_id| overview_slot_for_view(slots, view_id))
                {
                    let (row, col) = shifted_slot_position(slot.row, slot.col, direction);
                    graph_intents.push(GraphIntent::MoveGraphViewSlot {
                        view_id: slot.view_id,
                        row,
                        col,
                    });
                }
                continue;
            }

            selected_view_id = next_overview_selected_view_id(slots, selected_view_id, direction);
        }

        if input.key_pressed(Key::Space)
            && let Some(view_id) = selected_view_id
        {
            surface_actions.push(OverviewSurfaceAction::FocusView(view_id));
        }

        if input.key_pressed(Key::Enter) {
            if ctrl {
                if let Some(view_id) = selected_view_id
                    && let Some(action) = overview_transfer_action(app, view_id)
                {
                    surface_actions.push(action);
                }
            } else if let Some(view_id) = selected_view_id {
                surface_actions.push(OverviewSurfaceAction::OpenView(view_id));
            }
        }
    });

    (selected_view_id, graph_intents, surface_actions)
}

fn drag_target_slot_position(slot: &OverviewSlotSnapshot, drag_delta: Vec2) -> (i32, i32) {
    let col_delta = (drag_delta.x / (OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP)).round() as i32;
    let row_delta = (drag_delta.y / (OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP)).round() as i32;
    (slot.row + row_delta, slot.col + col_delta)
}

fn shifted_slot_position(row: i32, col: i32, direction: GraphViewLayoutDirection) -> (i32, i32) {
    match direction {
        GraphViewLayoutDirection::Up => (row - 1, col),
        GraphViewLayoutDirection::Down => (row + 1, col),
        GraphViewLayoutDirection::Left => (row, col - 1),
        GraphViewLayoutDirection::Right => (row, col + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_host_layout() -> crate::shell::desktop::ui::workbench_host::WorkbenchHostLayout {
        crate::shell::desktop::ui::workbench_host::WorkbenchHostLayout {
            host: crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ),
            anchor_edge: crate::app::workbench_layout_policy::AnchorEdge::Right,
            form_factor:
                crate::shell::desktop::ui::workbench_host::WorkbenchHostFormFactor::Sidebar,
            configured_scope: crate::app::NavigatorHostScope::Both,
            resolved_scope: crate::app::NavigatorHostScope::Both,
            size_fraction: 0.3,
            cross_axis_margin_start_px: 0.0,
            cross_axis_margin_end_px: 0.0,
            resizable: true,
        }
    }

    fn empty_projection() -> WorkbenchChromeProjection {
        let host_layout = test_host_layout();
        WorkbenchChromeProjection {
            layer_state:
                crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy:
                crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: None,
            active_frame_name: None,
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![],
            tree_root: None,
            active_graphlet_roster: vec![],
        }
    }

    #[test]
    fn drag_target_slot_position_rounds_to_nearest_grid_cell() {
        let slot = OverviewSlotSnapshot {
            view_id: GraphViewId::new(),
            name: "View".to_string(),
            row: 4,
            col: 7,
            archived: false,
        };

        let horizontal_step = OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP;
        let vertical_step = OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP;

        assert_eq!(
            drag_target_slot_position(
                &slot,
                Vec2::new(horizontal_step * 1.1, -vertical_step * 0.9)
            ),
            (3, 8)
        );
    }

    #[test]
    fn selected_overview_view_id_prefers_focused_view() {
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(view_a);
        app.ensure_graph_view_registered(view_b);
        app.workspace.graph_runtime.focused_view = Some(view_b);

        let slots = sorted_slot_snapshots(&app);
        assert_eq!(selected_overview_view_id(&app, &slots), Some(view_b));
    }

    #[test]
    fn navigator_overview_swatch_enabled_requires_sidebar_width() {
        assert!(!navigator_overview_swatch_enabled(240.0));
        assert!(navigator_overview_swatch_enabled(
            NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH
        ));
    }

    #[test]
    fn navigator_overview_drag_transfer_enabled_requires_spacious_cells() {
        assert!(!navigator_overview_drag_transfer_enabled(58.0));
        assert!(navigator_overview_drag_transfer_enabled(
            NAVIGATOR_OVERVIEW_TRANSFER_CELL_MIN_WIDTH
        ));
    }

    #[test]
    fn overview_transfer_affordance_reports_missing_focus_reason() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);

        let affordance = overview_transfer_affordance(&app, view_id);
        assert!(!affordance.enabled);
        assert_eq!(
            affordance.disabled_reason,
            "Focus a source graph view first."
        );
    }

    #[test]
    fn overview_transfer_affordance_reports_empty_selection_reason() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = GraphViewId::new();
        let destination = GraphViewId::new();
        app.ensure_graph_view_registered(source);
        app.ensure_graph_view_registered(destination);
        app.workspace.graph_runtime.focused_view = Some(source);

        let affordance = overview_transfer_affordance(&app, destination);
        assert!(!affordance.enabled);
        assert_eq!(
            affordance.disabled_reason,
            "Select one or more nodes in the focused graph view first."
        );
    }

    #[test]
    fn navigator_overview_transfer_action_matches_overview_transfer_parity() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = GraphViewId::new();
        let destination = GraphViewId::new();
        app.ensure_graph_view_registered(source);
        app.ensure_graph_view_registered(destination);
        app.workspace.graph_runtime.focused_view = Some(source);

        let node = app.workspace.domain.graph.add_node(
            "https://navigator-transfer.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        app.select_in_focused_view(node, false);

        assert_eq!(
            navigator_overview_transfer_action(&app, source, destination),
            Some(OverviewSurfaceAction::TransferSelectionToView {
                source_view: source,
                destination_view: destination,
            })
        );
    }

    #[test]
    fn navigator_overview_transfer_action_rejects_non_focused_source() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = GraphViewId::new();
        let destination = GraphViewId::new();
        app.ensure_graph_view_registered(source);
        app.ensure_graph_view_registered(destination);
        app.workspace.graph_runtime.focused_view = Some(destination);

        let node = app.workspace.domain.graph.add_node(
            "https://navigator-transfer-mismatch.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        app.select_in_focused_view(node, false);

        assert_eq!(
            navigator_overview_transfer_action(&app, source, destination),
            None
        );
    }

    #[test]
    fn overview_surface_action_maps_to_workbench_surface_contract() {
        let source = GraphViewId::new();
        let destination = GraphViewId::new();

        assert!(matches!(
            overview_surface_action_to_workbench_intent(OverviewSurfaceAction::FocusView(source)),
            WorkbenchIntent::FocusGraphView { view_id } if view_id == source
        ));
        assert!(matches!(
            overview_surface_action_to_workbench_intent(OverviewSurfaceAction::OpenView(source)),
            WorkbenchIntent::OpenGraphViewPane {
                view_id,
                mode: PendingTileOpenMode::Tab,
            } if view_id == source
        ));
        assert!(matches!(
            overview_surface_action_to_workbench_intent(
                OverviewSurfaceAction::TransferSelectionToView {
                    source_view: source,
                    destination_view: destination,
                },
            ),
            WorkbenchIntent::TransferSelectedNodesToGraphView {
                source_view,
                destination_view,
            } if source_view == source && destination_view == destination
        ));
        assert!(matches!(
            overview_surface_action_to_workbench_intent(OverviewSurfaceAction::ToggleOverviewPlane),
            WorkbenchIntent::ToggleOverviewPlane
        ));
    }

    #[test]
    fn graph_search_origin_label_matches_surface_copy() {
        assert_eq!(
            graph_search_origin_label(&GraphSearchOrigin::Manual),
            "manual scope"
        );
        assert_eq!(
            graph_search_origin_label(&GraphSearchOrigin::SemanticTag),
            "semantic-tag scope"
        );
        assert_eq!(
            graph_search_origin_label(&GraphSearchOrigin::AnchorSlice),
            "anchor-slice scope"
        );
    }

    #[test]
    fn overview_suggestion_labels_include_transfer_archive_and_preview_hints() {
        let labels = overview_suggestion_labels(true, true, 2, true);
        assert!(labels.contains(&"Focus or open the selected region."));
        assert!(labels.contains(&"Transfer the focused selection into the selected region."));
        assert!(labels.contains(&"Review archived regions before creating more layout sprawl."));
        assert!(labels.contains(&"Return to present before relying on live runtime status."));
    }

    #[test]
    fn node_summary_label_prefers_node_title_over_url() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://overview-node.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        let _ = app
            .workspace
            .domain
            .graph
            .set_node_title(key, "Overview Node".to_string());

        assert_eq!(node_summary_label(&app, key), "Overview Node");
    }

    #[test]
    fn graph_context_lines_report_active_search_summary() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.active_graph_search_query = "anchor".to_string();
        app.workspace.graph_runtime.active_graph_search_match_count = 3;
        app.workspace.graph_runtime.active_graph_search_origin = GraphSearchOrigin::AnchorSlice;

        let projection = empty_projection();
        let lines = graph_context_lines(&app, &projection, None);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Search: anchor · 3 matches"))
        );
        assert!(lines.iter().any(|line| line.contains("anchor-slice scope")));
    }

    #[test]
    fn graph_context_actions_expose_graph_and_workbench_routes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);
        app.set_workspace_focused_view_with_transition(Some(view_id));
        let node = app.workspace.domain.graph.add_node(
            "https://overview-actions.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        app.select_in_focused_view(node, false);
        let slot = OverviewSlotSnapshot {
            view_id,
            name: "Focus".to_string(),
            row: 0,
            col: 0,
            archived: false,
        };

        let actions = graph_context_actions(&app, Some(&slot));

        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Graph
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Graph(GraphIntent::CreateGraphViewSlot {
                        anchor_view: Some(anchor_view),
                        direction: GraphViewLayoutDirection::Right,
                        open_mode: Some(PendingTileOpenMode::Tab),
                    }) if anchor_view == view_id
                )
        }));
        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Workbench
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenNodeInPane {
                        node: opened_node,
                        ..
                    }) if opened_node == node
                )
        }));
    }

    #[test]
    fn viewer_content_actions_for_node_entry_include_viewer_routes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let pane_id = PaneId::new();
        let node_key = app.workspace.domain.graph.add_node(
            "https://viewer-actions.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node { node_key },
                title: "Node".to_string(),
                subtitle: None,
                arrangement_memberships: vec![],
                semantic_tab_affordance: None,
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:webview".to_string()),
                    viewer_override: None,
                    viewer_switch_reason: ViewerSwitchReason::PolicyPinned,
                    render_mode: TileRenderMode::EmbeddedEgui,
                    runtime_blocked: false,
                    runtime_crashed: false,
                    fallback_reason: None,
                    available_viewer_ids: vec!["viewer:webview".to_string()],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![],
        };

        let actions = viewer_content_actions(&projection);

        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Viewer
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::SwapViewerBackend {
                        pane,
                        node,
                        viewer_id_override: None,
                    }) if pane == pane_id && node == node_key
                )
        }));
        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Viewer
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl { .. })
                )
        }));
        assert!(!actions.iter().any(|action| {
            matches!(
                action.dispatch,
                OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenToolUrl { .. })
            )
        }));
    }

    #[test]
    fn viewer_content_actions_offer_explicit_middlenet_override_when_available() {
        let pane_id = PaneId::new();
        let node_key = crate::graph::NodeKey::new(21);
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Feed".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node { node_key },
                title: "Feed".to_string(),
                subtitle: Some("application/rss+xml".to_string()),
                arrangement_memberships: vec![],
                semantic_tab_affordance: None,
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:webview".to_string()),
                    viewer_override: None,
                    viewer_switch_reason: ViewerSwitchReason::PolicyPinned,
                    render_mode: TileRenderMode::EmbeddedEgui,
                    runtime_blocked: false,
                    runtime_crashed: false,
                    fallback_reason: None,
                    available_viewer_ids: vec![
                        "viewer:webview".to_string(),
                        "viewer:middlenet".to_string(),
                    ],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![],
        };

        let actions = viewer_content_actions(&projection);

        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Viewer
                && action.label == "Use MiddleNet"
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::SwapViewerBackend {
                        pane,
                        node,
                        viewer_id_override: Some(ref viewer_id_override),
                    }) if pane == pane_id
                        && node == node_key
                        && viewer_id_override.as_str() == "viewer:middlenet"
                )
        }));
    }

    #[test]
    fn viewer_content_lines_surface_fallback_and_runtime_diagnostics() {
        let pane_id = PaneId::new();
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: crate::graph::NodeKey::new(12),
                },
                title: "Node".to_string(),
                subtitle: Some("text/html".to_string()),
                arrangement_memberships: vec![],
                semantic_tab_affordance: None,
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:wry".to_string()),
                    viewer_override: Some("viewer:wry".to_string()),
                    viewer_switch_reason: ViewerSwitchReason::UserRequested,
                    render_mode: TileRenderMode::Placeholder,
                    runtime_blocked: true,
                    runtime_crashed: true,
                    fallback_reason: Some(
                        "Wry backend is disabled. Enable it in Settings -> Viewer Backends."
                            .to_string(),
                    ),
                    available_viewer_ids: vec![],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![],
        };

        let lines = viewer_content_lines(&projection);

        assert!(
            lines
                .iter()
                .any(|line| line == "Viewer backend: viewer:wry · placeholder")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "Override: viewer:wry · user override")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "Degraded: runtime crash recorded for this node")
        );
        assert!(lines.iter().any(|line| line == "Runtime blocked: startup or backpressure is holding this pane"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Fallback: Wry backend is disabled"))
        );
    }

    #[test]
    fn viewer_content_actions_for_degraded_node_include_diagnostics_route() {
        let pane_id = PaneId::new();
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: crate::graph::NodeKey::new(14),
                },
                title: "Node".to_string(),
                subtitle: None,
                arrangement_memberships: vec![],
                semantic_tab_affordance: None,
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:unknown".to_string()),
                    viewer_override: Some("viewer:unknown".to_string()),
                    viewer_switch_reason: ViewerSwitchReason::UserRequested,
                    render_mode: TileRenderMode::Placeholder,
                    runtime_blocked: false,
                    runtime_crashed: false,
                    fallback_reason: Some(
                        "Viewer 'viewer:unknown' is unresolved for this build path.".to_string(),
                    ),
                    available_viewer_ids: vec![],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![],
        };

        let actions = viewer_content_actions(&projection);

        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Viewer
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenToolUrl { .. })
                )
        }));
    }

    #[test]
    fn di05_overview_surface_reorients_across_domain_cards() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.history_preview_mode_active = true;
        app.workspace.graph_runtime.active_graph_search_query = "focus".to_string();
        app.workspace.graph_runtime.active_graph_search_match_count = 2;
        app.workspace.graph_runtime.active_graph_search_origin = GraphSearchOrigin::Manual;

        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);
        app.set_workspace_focused_view_with_transition(Some(view_id));
        let node_key = app
            .workspace
            .domain
            .graph
            .add_node("https://di05.example".to_string(), euclid::point2(0.0, 0.0));
        app.select_in_focused_view(node_key, false);

        let pane_id = PaneId::new();
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node Pane".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec!["frame-a".to_string()],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node { node_key },
                title: "Node".to_string(),
                subtitle: Some("text/html".to_string()),
                arrangement_memberships: vec!["triage".to_string()],
                semantic_tab_affordance: Some(SemanticTabAffordance::Collapse {
                    group_id: uuid::Uuid::nil(),
                    member_count: 2,
                }),
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:wry".to_string()),
                    viewer_override: Some("viewer:wry".to_string()),
                    viewer_switch_reason: ViewerSwitchReason::UserRequested,
                    render_mode: TileRenderMode::Placeholder,
                    runtime_blocked: true,
                    runtime_crashed: false,
                    fallback_reason: Some(
                        "Wry backend is disabled. Enable it in Settings -> Viewer Backends."
                            .to_string(),
                    ),
                    available_viewer_ids: vec![],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![
                GraphletRosterEntry {
                    node_key,
                    title: "Warm seed".to_string(),
                    is_cold: false,
                },
                GraphletRosterEntry {
                    node_key,
                    title: "Cold peer".to_string(),
                    is_cold: true,
                },
            ],
        };
        let slot = OverviewSlotSnapshot {
            view_id,
            name: "Focus".to_string(),
            row: 0,
            col: 0,
            archived: false,
        };

        let graph_lines = graph_context_lines(&app, &projection, Some(&slot));
        let workbench_lines = workbench_context_lines(&projection);
        let viewer_lines = viewer_content_lines(&projection);
        let runtime_lines = runtime_attention_lines(&app);
        let graph_actions = graph_context_actions(&app, Some(&slot));
        let workbench_actions = workbench_context_actions(&projection, Some(&slot));
        let viewer_actions = viewer_content_actions(&projection);
        let chips = compact_overview_chips(&app, &projection, Some(&slot), 1);

        assert!(
            graph_lines
                .iter()
                .any(|line| line.contains("Active pane graphlet: 1 warm node(s) · 1 cold node(s)"))
        );
        assert!(
            graph_lines
                .iter()
                .any(|line| line.contains("Search: focus · 2 matches"))
        );
        assert!(
            workbench_lines
                .iter()
                .any(|line| line == "Workbench binding: linked semantic tab group (2 pane(s))")
        );
        assert!(
            viewer_lines
                .iter()
                .any(|line| line == "Viewer backend: viewer:wry · placeholder")
        );
        assert!(
            viewer_lines
                .iter()
                .any(|line| line.contains("Fallback: Wry backend is disabled"))
        );
        assert!(
            runtime_lines
                .iter()
                .any(|line| line == "History preview active: live runtime side effects suppressed")
        );
        assert!(
            graph_actions
                .iter()
                .any(|action| action.owner == OverviewActionOwner::Graph)
        );
        assert!(
            workbench_actions
                .iter()
                .any(|action| action.owner == OverviewActionOwner::Workbench)
        );
        assert!(viewer_actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Viewer
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenToolUrl { .. })
                )
        }));
        assert!(chips.iter().any(|chip| chip.contains("Viewer fallback")));
        assert!(
            chips
                .iter()
                .any(|chip| chip.contains("Tabs: linked semantic"))
        );
    }

    #[test]
    fn graph_context_lines_surface_graphlet_frontier_breakdown() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);
        app.set_workspace_focused_view_with_transition(Some(view_id));
        let node_key = app.workspace.domain.graph.add_node(
            "https://graphlet-overview.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        app.select_in_focused_view(node_key, false);
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![],
            tree_root: None,
            active_graphlet_roster: vec![
                GraphletRosterEntry {
                    node_key,
                    title: "Warm seed".to_string(),
                    is_cold: false,
                },
                GraphletRosterEntry {
                    node_key,
                    title: "Cold peer that will truncate".to_string(),
                    is_cold: true,
                },
            ],
        };

        let lines = graph_context_lines(&app, &projection, None);

        assert!(
            lines
                .iter()
                .any(|line| line == "Active pane graphlet: 1 warm node(s) · 1 cold node(s)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Frontier ready to open: Cold peer that wi…"))
        );
    }

    #[test]
    fn workbench_context_lines_surface_semantic_tab_binding_state() {
        let pane_id = PaneId::new();
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec!["frame-a".to_string()],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: crate::graph::NodeKey::new(7),
                },
                title: "Node".to_string(),
                subtitle: None,
                arrangement_memberships: vec!["triage".to_string()],
                semantic_tab_affordance: Some(SemanticTabAffordance::Restore {
                    group_id: uuid::Uuid::nil(),
                    member_count: 3,
                }),
                node_viewer_summary: None,
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![],
        };

        let lines = workbench_context_lines(&projection);

        assert!(
            lines
                .iter()
                .any(|line| line
                    == "Workbench binding: detached from semantic tab group (3 pane(s))")
        );
    }

    #[test]
    fn runtime_attention_actions_route_to_runtime_owned_surfaces() {
        let app = GraphBrowserApp::new_for_testing();

        let actions = runtime_attention_actions(&app);

        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Runtime
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenToolUrl { .. })
                )
        }));
        assert!(actions.iter().any(|action| {
            action.owner == OverviewActionOwner::Runtime
                && matches!(
                    action.dispatch,
                    OverviewQuickActionDispatch::Workbench(WorkbenchIntent::OpenSettingsUrl { .. })
                )
        }));
    }

    #[test]
    fn compact_overview_chips_surface_active_context_and_history_status() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.history_preview_mode_active = true;
        let view_id = GraphViewId::new();
        let pane_id = PaneId::new();
        let node_key = app.workspace.domain.graph.add_node(
            "https://compact-viewer.example".to_string(),
            euclid::point2(0.0, 0.0),
        );
        let host_layout = test_host_layout();
        let projection = WorkbenchChromeProjection {
            layer_state: crate::shell::desktop::ui::workbench_host::WorkbenchLayerState::WorkbenchActive,
            chrome_policy: crate::shell::desktop::ui::workbench_host::ChromeExposurePolicy::GraphPlusWorkbenchHost,
            host_layout: host_layout.clone(),
            host_layouts: vec![host_layout],
            active_graph_view: None,
            extra_graph_views: vec![],
            active_pane_title: Some("Node Pane".to_string()),
            active_frame_name: Some("frame-a".to_string()),
            saved_frame_names: vec!["frame-a".to_string()],
            navigator_groups: vec![],
            pane_entries: vec![WorkbenchPaneEntry {
                pane_id,
                kind: WorkbenchPaneKind::Node { node_key },
                title: "Node".to_string(),
                subtitle: None,
                arrangement_memberships: vec![],
                semantic_tab_affordance: Some(SemanticTabAffordance::Collapse {
                    group_id: uuid::Uuid::nil(),
                    member_count: 2,
                }),
                node_viewer_summary: Some(WorkbenchNodeViewerSummary {
                    effective_viewer_id: Some("viewer:unknown".to_string()),
                    viewer_override: Some("viewer:unknown".to_string()),
                    viewer_switch_reason: ViewerSwitchReason::UserRequested,
                    render_mode: TileRenderMode::Placeholder,
                    runtime_blocked: false,
                    runtime_crashed: false,
                    fallback_reason: Some(
                        "Viewer 'viewer:unknown' is unresolved for this build path.".to_string(),
                    ),
                    available_viewer_ids: vec![],
                }),
                presentation_mode: PanePresentationMode::Tiled,
                is_active: true,
                closable: true,
            }],
            tree_root: None,
            active_graphlet_roster: vec![GraphletRosterEntry {
                node_key: crate::graph::NodeKey::new(9),
                title: "Warm seed".to_string(),
                is_cold: false,
            }],
        };
        let slot = OverviewSlotSnapshot {
            view_id,
            name: "Focus".to_string(),
            row: 1,
            col: 2,
            archived: false,
        };

        let chips = compact_overview_chips(&app, &projection, Some(&slot), 2);

        assert!(chips.iter().any(|chip| chip.contains("View Focus")));
        assert!(chips.iter().any(|chip| chip == "Panes: 1"));
        assert!(
            chips
                .iter()
                .any(|chip| chip.contains("Active pane graphlet"))
        );
        assert!(
            chips
                .iter()
                .any(|chip| chip.contains("Tabs: linked semantic"))
        );
        assert!(chips.iter().any(|chip| chip.contains("Viewer fallback")));
        assert!(chips.iter().any(|chip| chip == "History preview"));
        assert!(chips.iter().any(|chip| chip == "Archived: 2"));
    }

    #[test]
    fn next_overview_selected_view_id_moves_to_nearest_neighbor() {
        let selected = GraphViewId::new();
        let right = GraphViewId::new();
        let down = GraphViewId::new();
        let diagonal = GraphViewId::new();
        let slots = vec![
            OverviewSlotSnapshot {
                view_id: selected,
                name: "Selected".to_string(),
                row: 0,
                col: 0,
                archived: false,
            },
            OverviewSlotSnapshot {
                view_id: right,
                name: "Right".to_string(),
                row: 0,
                col: 1,
                archived: false,
            },
            OverviewSlotSnapshot {
                view_id: down,
                name: "Down".to_string(),
                row: 1,
                col: 0,
                archived: false,
            },
            OverviewSlotSnapshot {
                view_id: diagonal,
                name: "Diagonal".to_string(),
                row: 1,
                col: 1,
                archived: false,
            },
        ];

        assert_eq!(
            next_overview_selected_view_id(&slots, Some(selected), GraphViewLayoutDirection::Right),
            Some(right)
        );
        assert_eq!(
            next_overview_selected_view_id(&slots, Some(selected), GraphViewLayoutDirection::Down),
            Some(down)
        );
    }

    #[test]
    fn sorted_slot_snapshots_lists_active_before_archived() {
        let active = GraphViewId::new();
        let archived = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(active);
        app.ensure_graph_view_registered(archived);
        app.archive_graph_view_slot(archived);

        let slots = sorted_slot_snapshots(&app);

        assert_eq!(slots.first().map(|slot| slot.view_id), Some(active));
        assert_eq!(slots.last().map(|slot| slot.view_id), Some(archived));
    }
}
