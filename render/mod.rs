/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module routed through the portable `graph-canvas` seam.
//!
//! `render_graph_canvas_in_ui` is the live graph-pane entry point: it
//! delegates scene derivation, hit testing, and interaction to
//! `graph-canvas` via `canvas_bridge::run_graph_canvas_frame`, and uses
//! `canvas_egui_painter` / `canvas_bridge::collect_canvas_events` as the
//! host-local paint + input adapters. The retired `egui_graphs` path is
//! preserved only in historical references.

use crate::app::{
    ChooseFramePickerMode, GraphBrowserApp, GraphIntent, SearchDisplayMode, SelectionUpdateMode,
    SimulateBehaviorPreset, UnsavedFramePromptAction, UnsavedFramePromptRequest, ViewAction,
};
use crate::graph::NodeKey;
use crate::graph::scene_runtime::{SceneCollisionPolicy, SceneRegionDragMode, SceneRegionDragState};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_LAYOUT_SYNC_BLOCKED_NO_STATE, CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT,
    CHANNEL_UX_NAVIGATION_TRANSITION, phase3_resolve_active_canvas_profile,
    phase3_resolve_active_theme,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::toolbar_routing;
use crate::util::CoordBridge;
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::{Ui, Window};
use euclid::default::Point2D;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use uuid::Uuid;

pub(crate) mod canvas_bridge;
pub(crate) mod canvas_egui_painter;
mod canvas_visuals;
mod graph_info;
mod panels;
mod reducer_bridge;
pub(crate) mod semantic_tags;
mod spatial_index;
#[cfg(test)]
use canvas_visuals::filtered_graph_for_search;
use canvas_visuals::{
    canvas_rect_from_view_frame, effective_graph_screen_rect, filtered_graph_for_visible_nodes,
    graph_visible_screen_rects, viewport_culled_graph, visible_nodes_for_view_filters,
};
use graph_info::draw_graph_info;
#[cfg(test)]
pub(crate) use panels::history_manager_entry_limit_for_tests;
pub use panels::{
    render_clip_inspector_panel, render_help_panel, render_history_manager_in_ui,
    render_navigator_tool_pane_in_ui, render_scene_overlay_panel,
    render_settings_node_viewer_in_ui, render_settings_overlay_panel,
    render_settings_tool_pane_in_ui_with_control_panel,
};
use reducer_bridge::{apply_reducer_graph_intents_hardened, apply_ui_intents_with_checkpoint};
use semantic_tags::semantic_badges_by_key;

#[cfg(test)]
use crate::app::{ThreeDMode, ViewDimension, ZSource};
#[cfg(test)]
use crate::graph::NodeLifecycle;
#[cfg(test)]
use canvas_visuals::{
    hovered_adjacency_set, lifecycle_color, viewport_culled_graph_for_canvas_rect,
    viewport_culling_metrics_for_canvas_rect, viewport_culling_selection_for_canvas_rect,
};
#[cfg(test)]
use graph_info::{graph_view_semantic_depth_status_badge, selected_node_enrichment_summary};
#[cfg(test)]
use semantic_tags::{ranked_tag_suggestions, reserved_tag_warning};
#[cfg(test)]
use std::time::{Duration, SystemTime};

pub(crate) mod action_registry;
mod command_palette;
mod command_profile;
pub(crate) mod radial_menu;

pub(crate) fn dispatch_action_id(
    app: &mut GraphBrowserApp,
    action_id: action_registry::ActionId,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
) {
    let mut intents = Vec::new();
    command_palette::execute_action(
        app,
        action_id,
        pair_context,
        source_context,
        &mut intents,
        focused_pane_node,
        focused_pane_id,
    );
    apply_ui_intents_with_checkpoint(app, intents);
}

pub use command_palette::render_command_palette_panel;
pub use radial_menu::render_radial_command_menu;

/// Graph interaction action (resolved from graph-canvas engine actions).
///
/// Decouples action production (graph-canvas `CanvasAction` emission inside
/// `canvas_bridge::run_graph_canvas_frame`) from action application (pure
/// state mutation here), keeping graph interactions testable without an
/// egui rendering context.
pub enum GraphAction {
    FocusNode(NodeKey),
    FocusFrame {
        frame_name: String,
        member_key: NodeKey,
    },
    FocusNodeSplit(NodeKey),
    DragStart,
    DragEnd(NodeKey, Point2D<f32>),
    MoveNode(NodeKey, Point2D<f32>),
    SelectNode {
        key: NodeKey,
        multi_select: bool,
    },
    SelectFrame(String),
    LassoSelect {
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    },
    SetHighlightedEdge {
        from: NodeKey,
        to: NodeKey,
    },
    ClearHighlightedEdge,
    ClearSelection,
    Zoom(f32),
}

fn set_focused_view_with_transition(
    app: &mut GraphBrowserApp,
    focused_view: Option<crate::app::GraphViewId>,
) {
    // Delegate to the canonical setter so the scope-close invariant
    // (action_surface closes on view change) runs consistently.
    app.set_workspace_focused_view_with_transition(focused_view);
}

fn action_handles_primary_click(action: &GraphAction) -> bool {
    matches!(
        action,
        GraphAction::FocusNode(_)
            | GraphAction::FocusFrame { .. }
            | GraphAction::FocusNodeSplit(_)
            | GraphAction::DragStart
            | GraphAction::DragEnd(_, _)
            | GraphAction::MoveNode(_, _)
            | GraphAction::SelectNode { .. }
            | GraphAction::SelectFrame(_)
            | GraphAction::LassoSelect { .. }
            | GraphAction::SetHighlightedEdge { .. }
    )
}

fn should_clear_selection_on_background_click(
    pointer_primary_clicked: bool,
    modifiers: egui::Modifiers,
    hovered_graph_node: Option<NodeKey>,
    graph_handled_primary_click: bool,
    radial_open: bool,
    lasso_active: bool,
) -> bool {
    pointer_primary_clicked
        && !modifiers.any()
        && hovered_graph_node.is_none()
        && !graph_handled_primary_click
        && !radial_open
        && !lasso_active
}

fn scene_arrange_mode_active(app: &GraphBrowserApp, view_id: crate::app::GraphViewId) -> bool {
    app.graph_view_scene_mode(view_id) == crate::app::SceneMode::Arrange
}

fn simulate_scene_collision_policy(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    base: SceneCollisionPolicy,
) -> SceneCollisionPolicy {
    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Simulate {
        return base;
    }

    match app.graph_view_simulate_behavior_preset(view_id) {
        SimulateBehaviorPreset::Float => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(6.0),
            region_effect_scale: base.region_effect_scale.max(0.8),
            containment_response_scale: base.containment_response_scale.max(0.45),
        },
        SimulateBehaviorPreset::Packed => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(12.0),
            region_effect_scale: base.region_effect_scale.max(1.1),
            containment_response_scale: base.containment_response_scale.max(1.0),
        },
        SimulateBehaviorPreset::Magnetic => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(8.0),
            region_effect_scale: base.region_effect_scale.max(1.85),
            containment_response_scale: base.containment_response_scale.max(0.7),
        },
    }
}

fn node_key_or_emit_ambiguous_hit(node_key: Option<NodeKey>) -> Option<NodeKey> {
    if node_key.is_none() {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT,
            latency_us: 0,
        });
    }
    node_key
}

fn handle_hovered_node_secondary_click(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    target: NodeKey,
    pointer: Option<egui::Pos2>,
) {
    match app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(None);
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.chrome_ui.show_radial_menu {
                app.workspace.chrome_ui.surface_state = crate::app::ActionSurfaceState::Radial {
                    scope: crate::app::ActionScope::Graph {
                        view_id,
                        target: crate::app::ScopeTarget::Node(target),
                    },
                    anchor: crate::app::Anchor::TargetNode(target),
                };
                let _ = toolbar_routing::request_radial_menu_toggle(
                    app,
                    CommandBarFocusTarget::new(None, Some(target)),
                );
            }
            if let Some(pointer) = pointer {
                ctx.data_mut(|d| {
                    d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                });
            }
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            app.set_pending_node_context_target(Some(target));
            app.set_pending_frame_context_target(None);
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
            app.open_palette_contextual(
                crate::app::ActionScope::Graph {
                    view_id,
                    target: crate::app::ScopeTarget::Node(target),
                },
                crate::app::Anchor::TargetNode(target),
            );
        }
    }
}

fn handle_frame_backdrop_secondary_click(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    frame_name: String,
    pointer: Option<egui::Pos2>,
) {
    match app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(Some(frame_name.clone()));
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.chrome_ui.show_radial_menu {
                app.workspace.chrome_ui.surface_state = crate::app::ActionSurfaceState::Radial {
                    scope: crate::app::ActionScope::Graph {
                        view_id,
                        target: crate::app::ScopeTarget::Frame(frame_name.clone()),
                    },
                    anchor: crate::app::Anchor::TargetFrame(frame_name.clone()),
                };
                let _ = toolbar_routing::request_radial_menu_toggle(
                    app,
                    CommandBarFocusTarget::default(),
                );
            }
            if let Some(pointer) = pointer {
                ctx.data_mut(|d| {
                    d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                });
            }
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(Some(frame_name.clone()));
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
            app.open_palette_contextual(
                crate::app::ActionScope::Graph {
                    view_id,
                    target: crate::app::ScopeTarget::Frame(frame_name.clone()),
                },
                crate::app::Anchor::TargetFrame(frame_name),
            );
        }
    }
}

/// Render graph info and controls hint overlay text into the current UI.
pub fn render_graph_info_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) {
    draw_graph_info(ui, app, view_id);
}

pub(crate) fn canvas_lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right-Drag Lasso",
        CanvasLassoBinding::ShiftLeftDrag => "Shift+Left-Drag Lasso",
    }
}

/// Render graph content via the portable `graph-canvas` pipeline.
///
/// Live graph-pane entry point since M2 retired `egui_graphs`. Uses
/// `graph-canvas` for scene derivation, hit testing, and interaction;
/// the egui host only handles painting (via `canvas_egui_painter`) and
/// input translation (via `canvas_bridge`).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_graph_canvas_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    _active_search_match: Option<NodeKey>,
    _search_display_mode: SearchDisplayMode,
    _search_query_active: bool,
) -> Vec<GraphAction> {
    let graph_rect = ui.max_rect();
    let scale_factor = ui.ctx().pixels_per_point();
    let viewport = canvas_bridge::viewport_from_egui_rect(graph_rect, scale_factor);
    let events = canvas_bridge::collect_canvas_events(ui);
    let output = canvas_bridge::run_graph_canvas_frame(
        app,
        view_id,
        search_matches,
        _search_display_mode,
        _search_query_active,
        viewport,
        &events,
    );
    canvas_egui_painter::paint_projected_scene(ui, &output.scene);
    output.graph_actions
}


/// Convert resolved graph actions to graph intents without applying them.
pub fn intents_from_graph_actions(actions: Vec<GraphAction>) -> Vec<GraphIntent> {
    let mut intents = Vec::with_capacity(actions.len());
    for action in actions {
        match action {
            GraphAction::FocusNode(key) => {
                intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            }
            GraphAction::FocusFrame {
                frame_name,
                member_key,
            } => {
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name.clone()),
                    }
                    .into(),
                );
                intents.push(GraphIntent::OpenFrameTileGroup {
                    url: crate::util::VersoAddress::frame(&frame_name).to_string(),
                    focus_node: Some(member_key),
                });
            }
            GraphAction::FocusNodeSplit(key) => {
                intents.push(GraphIntent::SelectNode {
                    key,
                    multi_select: false,
                });
            }
            GraphAction::DragStart => {
                intents.push(GraphIntent::SetInteracting { interacting: true });
            }
            GraphAction::DragEnd(key, pos) => {
                intents.push(GraphIntent::SetInteracting { interacting: false });
                intents.push(ViewAction::SetNodePosition { key, position: pos }.into());
            }
            GraphAction::MoveNode(key, pos) => {
                intents.push(ViewAction::SetNodePosition { key, position: pos }.into());
            }
            GraphAction::SelectNode { key, multi_select } => {
                intents.push(GraphIntent::SelectNode { key, multi_select });
            }
            GraphAction::SelectFrame(frame_name) => {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: SelectionUpdateMode::Replace,
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
            GraphAction::LassoSelect { keys, mode } => {
                intents.push(ViewAction::UpdateSelection { keys, mode }.into());
            }
            GraphAction::SetHighlightedEdge { from, to } => {
                intents.push(ViewAction::SetHighlightedEdge { from, to }.into());
            }
            GraphAction::ClearHighlightedEdge => {
                intents.push(ViewAction::ClearHighlightedEdge.into());
            }
            GraphAction::ClearSelection => {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: SelectionUpdateMode::Replace,
                    }
                    .into(),
                );
            }
            GraphAction::Zoom(new_zoom) => {
                intents.push(ViewAction::SetZoom { zoom: new_zoom }.into());
            }
        }
    }
    intents
}

fn record_simulate_release_impulses(
    app: &mut GraphBrowserApp,
    impulses: HashMap<NodeKey, egui::Vec2>,
) {
    let Some(view_id) = app.workspace.graph_runtime.focused_view else {
        return;
    };

    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Simulate {
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .remove(&view_id);
        return;
    }

    if impulses.is_empty() {
        return;
    }

    app.workspace
        .graph_runtime
        .simulate_release_impulses
        .insert(view_id, impulses);
}

pub fn render_choose_frame_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    let open_settings_tool_pane = false;
    let Some(request) = app.choose_frame_picker_request() else {
        return false;
    };
    let target = request.node;
    if app.workspace.domain.graph.get_node(target).is_none() {
        app.clear_choose_frame_picker();
        return false;
    }
    let mut selected_frame: Option<String> = None;
    let mut close = false;
    let mut memberships = match request.mode {
        ChooseFramePickerMode::OpenNodeInFrame => app.sorted_frames_for_node_key(target),
        ChooseFramePickerMode::AddNodeToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
        ChooseFramePickerMode::AddConnectedSelectionToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
        ChooseFramePickerMode::AddExactSelectionToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
    };
    let title = app
        .workspace
        .domain
        .graph
        .get_node(target)
        .map(|node| format!("Choose Frame: {}", node.title))
        .unwrap_or_else(|| "Choose Frame".to_string());
    Window::new(title)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(300.0)
        .default_height(360.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
                    if memberships.is_empty() {
                        let msg = match request.mode {
                            ChooseFramePickerMode::OpenNodeInFrame => {
                                "No frame memberships for this node."
                            }
                            ChooseFramePickerMode::AddNodeToFrame => {
                                "No named frames available. Save one first."
                            }
                            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                                "No named frames available. Save one first."
                            }
                            ChooseFramePickerMode::AddExactSelectionToFrame => {
                                "No named frames available. Save one first."
                            }
                        };
                        ui.label(
                            egui::RichText::new(msg)
                                .small()
                                .color(theme_tokens.command_notice),
                        );
                    } else {
                        memberships.dedup();
                        let header = match request.mode {
                            ChooseFramePickerMode::OpenNodeInFrame => "Open in frame:",
                            ChooseFramePickerMode::AddNodeToFrame => "Add node to frame:",
                            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                                "Add connected nodes to frame:"
                            }
                            ChooseFramePickerMode::AddExactSelectionToFrame => {
                                "Add selected nodes to frame:"
                            }
                        };
                        ui.label(
                            egui::RichText::new(header)
                                .small()
                                .color(theme_tokens.radial_chrome_text),
                        );
                        for name in &memberships {
                            if ui.button(name).clicked() {
                                selected_frame = Some(name.clone());
                                close = true;
                            }
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Open Persistence Overlay").clicked() {
                            crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
                                &VersoAddress::settings(GraphshellSettingsPath::Persistence)
                                    .to_string(),
                            );
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
        });
    if let Some(name) = selected_frame {
        match request.mode {
            ChooseFramePickerMode::OpenNodeInFrame => {
                apply_reducer_graph_intents_hardened(
                    app,
                    [GraphIntent::OpenNodeFrameRouted {
                        key: target,
                        prefer_frame: Some(name),
                    }],
                );
            }
            ChooseFramePickerMode::AddNodeToFrame => {
                app.request_add_node_to_frame(target, name);
            }
            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                let focused_selection = app.focused_selection();
                let mut seed_nodes: Vec<NodeKey> = if focused_selection.is_empty() {
                    vec![target]
                } else {
                    focused_selection.iter().copied().collect()
                };
                if !seed_nodes.contains(&target) {
                    seed_nodes.push(target);
                }
                app.request_add_connected_to_frame(seed_nodes, name);
            }
            ChooseFramePickerMode::AddExactSelectionToFrame => {
                let mut nodes = app
                    .choose_frame_picker_exact_nodes()
                    .map(|keys| keys.to_vec())
                    .unwrap_or_else(|| vec![target]);
                nodes.retain(|key| app.workspace.domain.graph.get_node(*key).is_some());
                nodes.sort_by_key(|key| key.index());
                nodes.dedup();
                if !nodes.is_empty() {
                    app.request_add_exact_nodes_to_frame(nodes, name);
                }
            }
        }
    }
    if close {
        app.clear_choose_frame_picker();
    }
    open_settings_tool_pane
}

pub fn render_unsaved_frame_prompt(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    let open_settings_tool_pane = false;
    let Some(request) = app.unsaved_frame_prompt_request().cloned() else {
        return false;
    };
    let mut action: Option<UnsavedFramePromptAction> = None;
    Window::new("Unsaved Frame Changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(380.0)
        .show(ctx, |ui| {
            match &request {
                UnsavedFramePromptRequest::FrameSwitch { name, .. } => {
                    ui.label(format!(
                        "This frame has unsaved graph changes.\nSwitch to '{name}' without saving?"
                    ));
                }
            }
            ui.separator();
            if ui.button("Open Persistence Overlay").clicked() {
                crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
                    &VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
                );
            }
            ui.horizontal(|ui| {
                if ui.button("Proceed Without Saving").clicked() {
                    action = Some(UnsavedFramePromptAction::ProceedWithoutSaving);
                }
                if ui.button("Cancel").clicked() {
                    action = Some(UnsavedFramePromptAction::Cancel);
                }
            });
        });
    if let Some(action) = action {
        app.set_unsaved_frame_prompt_action(action);
    }
    open_settings_tool_pane
}

pub fn render_choose_workspace_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    render_choose_frame_picker(ctx, app)
}

pub fn render_unsaved_workspace_prompt(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    render_unsaved_frame_prompt(ctx, app)
}

/// Resolve pair edge command context using precedence:
/// selected pair > (selected primary + explicit context target) > (selected primary + hovered node)
/// > (selected primary + focused pane node).
fn resolve_pair_command_context(
    app: &GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) -> Option<(NodeKey, NodeKey)> {
    let selection = app.focused_selection();

    if let Some((from, to)) = selection.ordered_pair() {
        return Some((from, to));
    }

    if selection.len() == 1 {
        let from = selection.primary()?;
        if let Some(to) = app.pending_node_context_target().filter(|to| *to != from) {
            return Some((from, to));
        }
        if let Some(to) = hovered_node.filter(|to| *to != from) {
            return Some((from, to));
        }
        if let Some(to) = focused_pane_node.filter(|to| *to != from) {
            return Some((from, to));
        }
    }

    None
}

fn resolve_source_node_context(
    app: &GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) -> Option<NodeKey> {
    app.pending_node_context_target()
        .or(app.focused_selection().primary())
        .or(hovered_node)
        .or(focused_pane_node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{SearchDisplayMode, WorkbenchIntent};
    use crate::registries::atomic::lens::LayoutMode;
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use std::hint::black_box;
    use std::time::Instant;

    fn test_app() -> GraphBrowserApp {
        GraphBrowserApp::new_for_testing()
    }

    #[test]
    fn test_focus_node_action() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::FocusNode(key)]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&key));
    }

    #[test]
    fn test_drag_start_sets_interacting() {
        let mut app = test_app();
        assert!(!app.workspace.graph_runtime.is_interacting);

        let intents = intents_from_graph_actions(vec![GraphAction::DragStart]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.graph_runtime.is_interacting);
    }

    #[test]
    fn focused_view_change_emits_ux_navigation_transition_channel() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut diagnostics = DiagnosticsState::new();

        set_focused_view_with_transition(&mut app, Some(view_id));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when focused view changes"
        );
    }

    #[test]
    fn test_drag_end_clears_interacting_and_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        app.set_interacting(true);

        let intents =
            intents_from_graph_actions(vec![GraphAction::DragEnd(key, Point2D::new(150.0, 250.0))]);
        app.apply_reducer_intents(intents);

        assert!(!app.workspace.graph_runtime.is_interacting);
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.projected_position(), Point2D::new(150.0, 250.0));
    }

    #[test]
    fn test_move_node_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::MoveNode(key, Point2D::new(42.0, 84.0))]);
        app.apply_reducer_intents(intents);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.projected_position(), Point2D::new(42.0, 84.0));
    }

    #[test]
    fn test_select_node_action() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key,
            multi_select: false,
        }]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&key));
    }

    #[test]
    fn test_select_node_action_ctrl_click_adds_to_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Single-click selects a.
        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: a,
            multi_select: false,
        }]);
        app.apply_reducer_intents(intents);

        // Ctrl+Click adds b without deselecting a.
        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: b,
            multi_select: true,
        }]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().len(), 2);
    }

    #[test]
    fn test_select_node_action_ctrl_click_toggles_off_selected() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Select both nodes.
        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: a,
                multi_select: false,
            },
            GraphAction::SelectNode {
                key: b,
                multi_select: true,
            },
        ]));
        assert_eq!(app.focused_selection().len(), 2);

        // Ctrl+Click a again → toggles a out of the selection.
        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: a,
            multi_select: true,
        }]));

        assert!(!app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().len(), 1);
    }

    #[test]
    fn test_select_node_single_click_does_not_affect_multi_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(200.0, 0.0));

        // Select a and b via multi-select.
        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: a,
                multi_select: false,
            },
            GraphAction::SelectNode {
                key: b,
                multi_select: true,
            },
        ]));

        // Single click c (no modifier): replaces selection with just c.
        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: c,
            multi_select: false,
        }]));

        assert!(!app.focused_selection().contains(&a));
        assert!(!app.focused_selection().contains(&b));
        assert!(app.focused_selection().contains(&c));
        assert_eq!(app.focused_selection().len(), 1);
    }

    #[test]
    fn test_lasso_select_action_maps_to_intent() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::LassoSelect {
            keys: vec![a, b],
            mode: SelectionUpdateMode::Replace,
        }]);
        app.apply_reducer_intents(intents);

        assert_eq!(app.focused_selection().len(), 2);
        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().primary(), Some(b));
    }

    #[test]
    fn test_clear_selection_action_maps_to_intent() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::LassoSelect {
            keys: vec![a, b],
            mode: SelectionUpdateMode::Replace,
        }]));
        assert_eq!(app.focused_selection().len(), 2);

        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::ClearSelection,
        ]));
        assert!(app.focused_selection().is_empty());
        assert_eq!(app.focused_selection().primary(), None);
    }

    #[test]
    fn test_select_frame_action_maps_to_intent_and_clears_node_selection() {
        let mut app = test_app();
        let key = app.add_node_and_sync(
            "https://example.com/selected".into(),
            Point2D::new(0.0, 0.0),
        );
        app.select_node(key, false);

        let intents = intents_from_graph_actions(vec![GraphAction::SelectFrame(
            "workspace-alpha".to_string(),
        )]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().is_empty());
        assert_eq!(app.selected_frame_name(), Some("workspace-alpha"));
    }

    #[test]
    fn test_focus_frame_action_maps_to_selected_frame_then_open_tile_group() {
        let member_key = NodeKey::new(9);
        let expected_url = crate::util::VersoAddress::frame("workspace-alpha").to_string();
        let intents = intents_from_graph_actions(vec![GraphAction::FocusFrame {
            frame_name: "workspace-alpha".to_string(),
            member_key,
        }]);

        assert_eq!(intents.len(), 2);
        assert!(matches!(
            &intents[0],
            GraphIntent::SetSelectedFrame { frame_name }
                if frame_name.as_deref() == Some("workspace-alpha")
        ));
        assert!(
            matches!(
                &intents[1],
                GraphIntent::OpenFrameTileGroup { url, focus_node }
                    if url == &expected_url && *focus_node == Some(member_key)
            ),
            "expected OpenFrameTileGroup with url={expected_url} and focus_node=Some({member_key:?}), got {:?}",
            &intents[1]
        );
    }

    #[test]
    fn test_set_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::SetHighlightedEdge { from, to }]);
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.workspace.graph_runtime.highlighted_graph_edge,
            Some((from, to))
        );
    }

    #[test]
    fn test_clear_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.highlighted_graph_edge = Some((from, to));

        let intents = intents_from_graph_actions(vec![GraphAction::ClearHighlightedEdge]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.graph_runtime.highlighted_graph_edge.is_none());
    }

    #[test]
    fn test_primary_click_handler_detects_selection_action() {
        let actions = vec![GraphAction::SelectNode {
            key: NodeKey::new(0),
            multi_select: false,
        }];

        assert!(actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_primary_click_handler_detects_frame_actions() {
        let actions = vec![
            GraphAction::SelectFrame("workspace-alpha".to_string()),
            GraphAction::FocusFrame {
                frame_name: "workspace-alpha".to_string(),
                member_key: NodeKey::new(7),
            },
        ];

        assert!(actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_primary_click_handler_ignores_zoom_only_actions() {
        let actions = vec![GraphAction::Zoom(1.2)];

        assert!(!actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_node_key_or_emit_ambiguous_hit_emits_diagnostic_on_none() {
        let mut diagnostics = DiagnosticsState::new();

        let resolved = node_key_or_emit_ambiguous_hit(None);

        assert!(resolved.is_none());
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 1);
    }

    #[test]
    fn test_node_key_or_emit_ambiguous_hit_does_not_emit_for_valid_node() {
        let mut diagnostics = DiagnosticsState::new();
        let key = NodeKey::new(42);

        let resolved = node_key_or_emit_ambiguous_hit(Some(key));

        assert_eq!(resolved, Some(key));
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 0);
    }

    #[test]
    fn test_background_click_clear_selection_requires_no_modifiers() {
        let modifiers = egui::Modifiers::CTRL;

        assert!(!should_clear_selection_on_background_click(
            true, modifiers, None, false, false, false,
        ));
    }

    #[test]
    fn test_background_click_clear_selection_blocked_when_radial_open() {
        assert!(!should_clear_selection_on_background_click(
            true,
            egui::Modifiers::NONE,
            None,
            false,
            true,
            false,
        ));
    }

    #[test]
    fn test_background_click_clear_selection_allowed_when_plain_click() {
        assert!(should_clear_selection_on_background_click(
            true,
            egui::Modifiers::NONE,
            None,
            false,
            false,
            false,
        ));
    }

    #[test]
    fn visible_graph_screen_rects_clip_to_navigation_geometry() {
        let mut app = GraphBrowserApp::new_for_testing();
        let screen_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0));
        app.workspace.graph_runtime.workbench_navigation_geometry =
            Some(crate::app::WorkbenchNavigationGeometry::from_content_rect(
                screen_rect,
                vec![egui::Rect::from_min_max(
                    egui::pos2(320.0, 40.0),
                    egui::pos2(400.0, 260.0),
                )],
            ));

        let visible_regions = graph_visible_screen_rects(screen_rect, &app);

        assert_eq!(visible_regions.len(), 3);
        assert!(visible_regions.contains_rect(egui::Rect::from_min_max(
            egui::pos2(0.0, 40.0),
            egui::pos2(320.0, 260.0),
        )));
    }

    #[test]
    fn effective_graph_screen_rect_prefers_largest_visible_navigation_region() {
        let mut app = GraphBrowserApp::new_for_testing();
        let screen_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0));
        app.workspace.graph_runtime.workbench_navigation_geometry =
            Some(crate::app::WorkbenchNavigationGeometry::from_content_rect(
                screen_rect,
                vec![egui::Rect::from_min_max(
                    egui::pos2(320.0, 40.0),
                    egui::pos2(400.0, 260.0),
                )],
            ));

        assert_eq!(
            effective_graph_screen_rect(screen_rect, &app),
            Some(egui::Rect::from_min_max(
                egui::pos2(0.0, 40.0),
                egui::pos2(320.0, 260.0),
            )),
        );
    }

    #[test]
    fn test_zoom_action_clamps() {
        let mut app = test_app();

        let intents = intents_from_graph_actions(vec![GraphAction::Zoom(0.01)]);
        app.apply_reducer_intents(intents);

        // Should be clamped to min zoom
        assert!(
            app.workspace.graph_runtime.camera.current_zoom
                >= app.workspace.graph_runtime.camera.zoom_min
        );
    }

    #[test]
    fn test_multiple_actions_sequence() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        let k1 = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let k2 = app.add_node_and_sync("b".into(), Point2D::new(100.0, 100.0));

        let intents = intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: k1,
                multi_select: false,
            },
            GraphAction::MoveNode(k2, Point2D::new(200.0, 300.0)),
            GraphAction::Zoom(1.5),
        ]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&k1));
        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(k2)
                .unwrap()
                .projected_position(),
            Point2D::new(200.0, 300.0)
        );
        assert!(
            (app.workspace.graph_runtime.views[&view_id]
                .camera
                .current_zoom
                - 1.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn test_empty_actions_is_noop() {
        let mut app = test_app();
        let key = app.add_node_and_sync("a".into(), Point2D::new(50.0, 60.0));
        let pos_before = app
            .workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .projected_position();

        let intents = intents_from_graph_actions(vec![]);
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .unwrap()
                .projected_position(),
            pos_before
        );
    }

    #[test]
    fn test_pair_command_context_prefers_selected_pair() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        app.select_node(b, false);
        app.select_node(a, true);

        let resolved = resolve_pair_command_context(&app, Some(b), Some(b));
        assert_eq!(resolved, Some((b, a)));
    }

    #[test]
    fn test_pair_command_context_falls_back_to_focused_node() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        app.select_node(a, false);

        let resolved = resolve_pair_command_context(&app, None, Some(b));
        assert_eq!(resolved, Some((a, b)));
    }

    #[test]
    fn test_pair_command_context_prefers_hover_over_focused_for_single_select() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(100.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(200.0, 0.0));
        app.select_node(a, false);

        let resolved = resolve_pair_command_context(&app, Some(hovered), Some(focused));
        assert_eq!(resolved, Some((a, hovered)));
    }

    #[test]
    fn test_pair_command_context_prefers_pending_target_over_hover_and_focused() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let pending = app.add_node_and_sync("pending".into(), Point2D::new(50.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(100.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(200.0, 0.0));
        app.select_node(a, false);
        app.set_pending_node_context_target(Some(pending));

        let resolved = resolve_pair_command_context(&app, Some(hovered), Some(focused));
        assert_eq!(resolved, Some((a, pending)));
    }

    #[test]
    fn test_source_context_prefers_pending_then_selected_then_hover_then_focused() {
        let mut app = test_app();
        let pending = app.add_node_and_sync("pending".into(), Point2D::new(-10.0, 0.0));
        let selected = app.add_node_and_sync("selected".into(), Point2D::new(0.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(10.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(20.0, 0.0));

        app.set_pending_node_context_target(Some(pending));
        app.select_node(selected, false);
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(pending)
        );
        app.set_pending_node_context_target(None);
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(selected)
        );
        app.clear_selection();
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(hovered)
        );
        assert_eq!(
            resolve_source_node_context(&app, None, Some(focused)),
            Some(focused)
        );
    }

    #[test]
    fn secondary_click_on_node_opens_radial_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let target = NodeKey::new(21);
        let pointer = egui::pos2(144.0, 233.0);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::RadialPalette,
        );
        app.workspace.chrome_ui.show_context_palette = true;
        app.workspace.chrome_ui.command_palette_contextual_mode = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, Some(pointer));

        assert!(!app.workspace.chrome_ui.show_radial_menu);
        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(app.pending_node_context_target().is_none());
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleRadialMenu]
        ));
        let stored_center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(stored_center, Some(pointer));
    }

    #[test]
    fn secondary_click_on_node_opens_context_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let target = NodeKey::new(22);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::ContextPalette,
        );
        app.workspace.chrome_ui.show_radial_menu = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, None);

        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert_eq!(app.pending_node_context_target(), Some(target));
        assert_eq!(
            app.pending_command_surface_return_target(),
            Some(crate::app::ToolSurfaceReturnTarget::Graph(view_id))
        );
    }

    #[test]
    fn secondary_click_on_frame_opens_context_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::ContextPalette,
        );

        handle_frame_backdrop_secondary_click(
            &ctx,
            &mut app,
            view_id,
            "workspace-alpha".to_string(),
            None,
        );

        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert_eq!(app.pending_frame_context_target(), Some("workspace-alpha"));
        assert_eq!(app.pending_node_context_target(), None);
    }

    #[test]
    fn secondary_click_on_frame_opens_radial_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let pointer = egui::pos2(88.0, 144.0);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::RadialPalette,
        );
        app.workspace.chrome_ui.show_context_palette = true;
        app.workspace.chrome_ui.command_palette_contextual_mode = true;

        handle_frame_backdrop_secondary_click(
            &ctx,
            &mut app,
            view_id,
            "workspace-alpha".to_string(),
            Some(pointer),
        );

        assert_eq!(app.pending_frame_context_target(), Some("workspace-alpha"));
        assert_eq!(app.pending_node_context_target(), None);
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleRadialMenu]
        ));
        let stored_center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(stored_center, Some(pointer));
    }

    #[test]
    fn test_neighbor_set_computation() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(20.0, 0.0));
        let _ = app.assert_relation_and_sync(
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );
        let _ = app.assert_relation_and_sync(
            c,
            a,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        let set = hovered_adjacency_set(&app, Some(a));
        assert!(set.contains(&a));
        assert!(set.contains(&b));
        assert!(set.contains(&c));
    }

    #[test]
    fn test_search_filter_mode_hides_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.search_display_mode = SearchDisplayMode::Filter;
        let matches = HashSet::from([a]);
        let filtered = filtered_graph_for_search(&app, &matches);
        assert!(filtered.get_node(a).is_some());
        assert!(filtered.get_node(b).is_none());
    }

    #[test]
    fn selected_node_enrichment_summary_includes_tags_suggestions_and_anchor() {
        let mut app = test_app();
        let math = app.add_node_and_sync("https://example.com/math".into(), Point2D::new(0.0, 0.0));
        let numerical = app.add_node_and_sync(
            "https://example.com/numerical".into(),
            Point2D::new(10.0, 0.0),
        );
        if let Some(node) = app.workspace.domain.graph.get_node_mut(math) {
            node.title = "Mathematics".into();
        }
        if let Some(node) = app.workspace.domain.graph.get_node_mut(numerical) {
            node.title = "Numerical Methods".into();
        }

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(numerical, "udc:519.6".to_string());
        app.workspace
            .graph_runtime
            .suggested_semantic_tags
            .insert(numerical, vec!["udc:519.8".to_string()]);
        app.workspace.graph_runtime.semantic_index_dirty = true;
        let _ = crate::shell::desktop::runtime::registries::knowledge::reconcile_semantics(
            &mut app,
            &crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default(),
        );

        let summary = selected_node_enrichment_summary(&app, numerical).expect("summary");

        assert!(
            summary
                .display_tags
                .iter()
                .any(|tag| tag.chip.label.contains("Computational mathematics"))
        );
        assert!(
            summary
                .display_tags
                .iter()
                .any(|tag| tag.status.contains("KnowledgeRegistry canonical"))
        );
        assert!(
            summary
                .suggested_tags
                .iter()
                .any(|tag| tag.chip.label.contains("Control"))
                || !summary.suggested_tags.is_empty()
        );
        assert!(
            summary
                .suggested_tags
                .iter()
                .all(|tag| !tag.reason.is_empty())
        );
        let anchor = summary.placement_anchor.expect("placement anchor");
        assert_eq!(anchor.key, math);
        assert_eq!(anchor.label, "Mathematics");
        assert_eq!(anchor.slice_tag.as_deref(), Some("udc:51"));
        assert!(summary.semantic_lens_available);
        assert!(!summary.semantic_lens_active);
    }

    #[test]
    fn graph_view_semantic_depth_status_badge_only_reports_udc_depth_mode() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Semantic");
        view.dimension = ViewDimension::ThreeD {
            mode: ThreeDMode::TwoPointFive,
            z_source: ZSource::UdcLevel { scale: 48.0 },
        };
        app.workspace.graph_runtime.views.insert(view_id, view);

        let badge = graph_view_semantic_depth_status_badge(&app, view_id);

        assert_eq!(badge.map(|(label, _)| label), Some("UDC Depth"));
    }

    #[test]
    fn graph_view_semantic_depth_status_badge_ignores_other_view_dimensions() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Standard");
        view.dimension = ViewDimension::ThreeD {
            mode: ThreeDMode::Isometric,
            z_source: ZSource::BfsDepth { scale: 12.0 },
        };
        app.workspace.graph_runtime.views.insert(view_id, view);

        assert_eq!(graph_view_semantic_depth_status_badge(&app, view_id), None);
    }

    #[test]
    fn ranked_tag_suggestions_include_existing_and_knowledge_matches() {
        let mut app = test_app();
        let node = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(node, "#starred".to_string());
        let other =
            app.add_node_and_sync("https://example.com/math".into(), Point2D::new(10.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(other, "research".to_string());

        let existing_tag_suggestions = ranked_tag_suggestions(&app, node, "rese");
        let existing_queries = existing_tag_suggestions
            .iter()
            .map(|chip| chip.query.clone())
            .collect::<Vec<_>>();
        assert!(existing_queries.contains(&"research".to_string()));

        let knowledge_suggestions = ranked_tag_suggestions(&app, node, "math");
        let knowledge_queries = knowledge_suggestions
            .iter()
            .map(|chip| chip.query.clone())
            .collect::<Vec<_>>();
        assert!(
            knowledge_queries
                .iter()
                .any(|query| query.starts_with("udc:"))
        );
    }

    #[test]
    fn reserved_tag_warning_flags_unknown_hash_tag() {
        assert_eq!(
            reserved_tag_warning("#custom"),
            Some("Unknown #tag will be accepted as user-defined.".to_string())
        );
        assert_eq!(reserved_tag_warning(GraphBrowserApp::TAG_PIN), None);
        assert_eq!(reserved_tag_warning("research"), None);
    }

    #[test]
    fn viewport_culling_metrics_reduce_visible_set_and_submission_units() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for row in 0..20 {
            for col in 0..20 {
                let key = app.add_node_and_sync(
                    format!("https://example.com/{row}/{col}"),
                    Point2D::new(col as f32 * 50.0, row as f32 * 50.0),
                );
                keys.push(key);
            }
        }

        for pair in keys.windows(2) {
            let _ = app.assert_relation_and_sync(
                pair[0],
                pair[1],
                crate::graph::EdgeAssertion::Semantic {
                    sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                    label: None,
                    decay_progress: None,
                },
            );
        }

        let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 300.0));
        let metrics =
            viewport_culling_metrics_for_canvas_rect(&app.workspace.domain.graph, canvas_rect)
                .expect("culling metrics should be available for dense graph viewport");

        assert!(metrics.visible_nodes < metrics.total_nodes);
        assert!(metrics.submitted_nodes < metrics.total_nodes);
        assert!(metrics.removed_nodes > 0);
        assert!(metrics.culled_submission_units < metrics.full_submission_units);
    }


    #[test]
    fn canvas_rect_from_view_frame_respects_pan_and_zoom() {
        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));
        let frame = crate::app::GraphViewFrame {
            zoom: 2.0,
            pan_x: 50.0,
            pan_y: 20.0,
        };

        let canvas = canvas_rect_from_view_frame(screen, frame)
            .expect("non-zero zoom should produce a canvas rect");

        assert!((canvas.min.x + 25.0).abs() < 0.001);
        assert!((canvas.min.y + 10.0).abs() < 0.001);
        assert!((canvas.max.x - 75.0).abs() < 0.001);
        assert!((canvas.max.y - 90.0).abs() < 0.001);
    }

    #[test]
    fn viewport_culling_differs_for_distinct_graph_view_frames() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for idx in 0..20 {
            keys.push(app.add_node_and_sync(
                format!("https://pane.example/{idx}"),
                Point2D::new(idx as f32 * 120.0, 0.0),
            ));
        }

        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 200.0));
        let near_origin = crate::app::GraphViewFrame {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
        let shifted = crate::app::GraphViewFrame {
            zoom: 1.0,
            pan_x: -1200.0,
            pan_y: 0.0,
        };

        let rect_a = canvas_rect_from_view_frame(screen, near_origin).unwrap();
        let rect_b = canvas_rect_from_view_frame(screen, shifted).unwrap();
        let selection_a =
            viewport_culling_selection_for_canvas_rect(&app.workspace.domain.graph, rect_a)
                .expect("expected culling selection for first view frame");
        let selection_b =
            viewport_culling_selection_for_canvas_rect(&app.workspace.domain.graph, rect_b)
                .expect("expected culling selection for second view frame");

        assert!(
            selection_a.visible.contains(&keys[0]),
            "origin view should include early nodes"
        );
        assert!(
            !selection_b.visible.contains(&keys[0]),
            "shifted view should exclude early nodes"
        );
        assert!(
            selection_b.visible.contains(&keys[10]),
            "shifted view should include later nodes"
        );
    }

}
