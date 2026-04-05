/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module using egui_graphs.
//!
//! Delegates graph visualization and interaction to the egui_graphs crate,
//! which provides built-in navigation (zoom/pan), node dragging, and selection.

use crate::app::{
    ChooseFramePickerMode, GraphBrowserApp, GraphIntent, SearchDisplayMode, SelectionUpdateMode,
    SimulateBehaviorPreset, UnsavedFramePromptAction, UnsavedFramePromptRequest, ViewAction,
    graph_layout::{GRAPH_LAYOUT_FORCE_DIRECTED, GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT},
};
use crate::graph::NodeKey;
use crate::graph::badge::is_archived_tag;
use crate::graph::egui_adapter::{EguiGraphState, GraphEdgeShape, GraphNodeShape};
use crate::graph::layouts::{ActiveLayout, ActiveLayoutKind, ActiveLayoutState};
use crate::graph::physics::apply_graph_physics_extensions;
use crate::graph::scene_runtime::{
    SceneCollisionPolicy, SceneRegionDragMode, SceneRegionDragState, apply_scene_runtime_pass,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_LAYOUT_SYNC_BLOCKED_NO_STATE, CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT,
    CHANNEL_UI_GRAPH_WHEEL_ZOOM_NOT_CAPTURED, CHANNEL_UX_NAVIGATION_TRANSITION,
    phase3_apply_layout_algorithm_to_graph, phase3_resolve_active_canvas_profile,
    phase3_resolve_active_theme, phase3_resolve_layout_algorithm,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::toolbar_routing;
use crate::util::CoordBridge;
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::{Ui, Window};
use egui_graphs::events::Event;
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
    get_layout_state, set_layout_state,
};
use euclid::default::Point2D;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use uuid::Uuid;

mod canvas_camera;
mod canvas_input;
mod canvas_overlays;
mod canvas_visuals;
mod graph_info;
mod panels;
mod reducer_bridge;
pub(crate) mod semantic_tags;
mod spatial_index;
use canvas_camera::handle_custom_navigation;
use canvas_input::{
    collect_graph_actions, collect_graph_keyboard_traversal_action, collect_lasso_action,
    graph_canvas_accessibility_label,
};
use canvas_overlays::{
    draw_frame_affinity_backdrops, draw_highlighted_edge_overlay, draw_hovered_edge_tooltip,
    draw_hovered_node_tooltip, draw_scene_region_action_overlay, draw_scene_runtime_backdrops,
    draw_scene_simulate_overlays, edge_endpoints_at_pointer, frame_anchor_at_pointer,
    scene_region_at_pointer, scene_region_resize_handle_at_pointer,
};
#[cfg(test)]
use canvas_visuals::filtered_graph_for_search;
use canvas_visuals::{
    apply_search_node_visuals, canvas_rect_from_view_frame, effective_graph_screen_rect,
    filtered_graph_for_visible_nodes, graph_visible_screen_rects, viewport_culled_graph,
    visible_nodes_for_view_filters,
};
use graph_info::{draw_graph_info, requested_layout_algorithm_id, should_apply_layout_algorithm};
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
use crate::app::{CameraCommand, KeyboardPanInputMode, ThreeDMode, ViewDimension, ZSource};
#[cfg(test)]
use crate::graph::NodeLifecycle;
#[cfg(test)]
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK,
    CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW,
};
#[cfg(test)]
use canvas_camera::{
    KeyboardPanInputState, KeyboardPanKeys, apply_background_pan, apply_background_pan_inertia,
    apply_pending_camera_command, emit_keyboard_pan_blocked_if_needed,
    keyboard_pan_delta_from_keys, keyboard_pan_delta_from_state, pan_inertia_velocity_id,
    should_auto_fit_locked_camera,
};
#[cfg(test)]
use canvas_input::{
    lasso_state_ids, next_keyboard_traversal_node, normalize_lasso_keys,
    resolve_lasso_selection_mode,
};
#[cfg(test)]
use canvas_overlays::{format_elapsed_ago, format_last_visited_with_now};
#[cfg(test)]
use canvas_visuals::{
    hovered_adjacency_set, lifecycle_color, viewport_culled_graph_for_canvas_rect,
    viewport_culling_metrics_for_canvas_rect, viewport_culling_selection_for_canvas_rect,
};
#[cfg(test)]
use egui::Vec2;
#[cfg(test)]
use graph_info::{graph_view_semantic_depth_status_badge, selected_node_enrichment_summary};
#[cfg(test)]
use petgraph::stable_graph::NodeIndex;
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

/// Graph interaction action (resolved from egui_graphs events).
///
/// Decouples event conversion (needs `egui_state` for NodeIndexâ†’NodeKey
/// lookups) from action application (pure state mutation), making
/// graph interactions testable without an egui rendering context.
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
    let previous_focused_view = app.workspace.graph_runtime.focused_view;
    app.workspace.graph_runtime.focused_view = focused_view;
    if app.workspace.graph_runtime.focused_view != previous_focused_view {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
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

fn pointer_canvas_pos(ui: &Ui, metadata_id: egui::Id, screen_pos: egui::Pos2) -> egui::Pos2 {
    ui.ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
        .map(|meta| meta.screen_to_canvas_pos(screen_pos))
        .unwrap_or(screen_pos)
}

fn apply_active_scene_region_drag(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
) -> bool {
    let Some(drag) = app.workspace.graph_runtime.active_scene_region_drag else {
        return false;
    };
    let primary_down = ui.input(|i| i.pointer.primary_down());
    if !primary_down {
        app.workspace.graph_runtime.active_scene_region_drag = None;
        app.set_interacting(false);
        return false;
    }

    let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
        return false;
    };
    let pointer_canvas = pointer_canvas_pos(ui, metadata_id, pointer);
    app.workspace.graph_runtime.active_scene_region_drag = Some(SceneRegionDragState {
        last_pointer_canvas_pos: pointer_canvas,
        ..drag
    });
    match drag.mode {
        SceneRegionDragMode::Move => {
            let delta = pointer_canvas - drag.last_pointer_canvas_pos;
            app.translate_graph_view_scene_region(drag.view_id, drag.region_id, delta)
        }
        SceneRegionDragMode::Resize(handle) => app.resize_graph_view_scene_region_to_pointer(
            drag.view_id,
            drag.region_id,
            handle,
            pointer_canvas,
        ),
    }
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
            app.open_context_palette();
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
            app.set_pending_frame_context_target(Some(frame_name));
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.chrome_ui.show_radial_menu {
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
            app.set_pending_frame_context_target(Some(frame_name));
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
            app.open_context_palette();
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

/// Navigation contract for graph rendering.
///
/// Graphshell owns graph camera/navigation as application state. We use
/// `egui_graphs` for retained graph rendering and interaction hit-testing, but
/// not for camera authority. Its built-in fit/pan/zoom paths stay disabled and
/// Graphshell drives `MetadataFrame` directly through the custom camera
/// pipeline.
fn graphshell_owned_navigation_settings() -> SettingsNavigation {
    SettingsNavigation::new()
        // Graphshell, not egui_graphs, is the fit authority.
        .with_fit_to_screen_enabled(false)
        // Graphshell, not egui_graphs, is the pan/zoom authority.
        .with_zoom_and_pan_enabled(false)
}

fn canvas_interaction_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
    radial_open: bool,
    right_button_down: bool,
    multi_select_modifier: bool,
) -> SettingsInteraction {
    let is_tree_topology = profile.topology.policy_id == "topology:tree";
    let selection_enabled =
        profile.interaction.node_selection_enabled && !radial_open && !right_button_down;

    SettingsInteraction::new()
        .with_dragging_enabled(
            profile.interaction.dragging_enabled
                && !radial_open
                && !right_button_down
                && !is_tree_topology,
        )
        .with_node_selection_enabled(selection_enabled)
        .with_node_selection_multi_enabled(selection_enabled && multi_select_modifier)
        .with_node_clicking_enabled(profile.interaction.node_clicking_enabled && !radial_open)
}

fn canvas_style_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> SettingsStyle {
    SettingsStyle::new().with_labels_always(profile.style.labels_always)
}

fn canvas_lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right-Drag Lasso",
        CanvasLassoBinding::ShiftLeftDrag => "Shift+Left-Drag Lasso",
    }
}

fn graph_view_metadata_id(custom_id: Option<String>) -> egui::Id {
    // egui_graphs persists metadata as Id::new(frame.get_id()), so callers must
    // use the double-hashed key to target the same persisted frame.
    egui::Id::new(MetadataFrame::new(custom_id).get_id())
}

/// Render graph content and return resolved interaction actions.
///
/// This lets callers customize how specific actions are handled
/// (e.g. routing double-click to tile opening instead of detail view).
pub fn render_graph_in_ui_collect_actions(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
) -> Vec<GraphAction> {
    let view_selection = app.selection_for_view(view_id).clone();

    // Ensure graph-view identity has durable registration (view state + slot).
    app.ensure_graph_view_registered(view_id);

    if app
        .workspace
        .graph_runtime
        .focused_view
        .is_some_and(|focused| !app.workspace.graph_runtime.views.contains_key(&focused))
    {
        set_focused_view_with_transition(app, None);
    }

    let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let right_button_down = ui.input(|i| i.pointer.secondary_down());
    let radial_open = app.workspace.chrome_ui.show_radial_menu;
    let filtered_visible_nodes = visible_nodes_for_view_filters(
        app,
        view_id,
        search_matches,
        search_display_mode,
        search_query_active,
    );
    let filtered_graph = filtered_visible_nodes
        .as_ref()
        .map(|visible_nodes| filtered_graph_for_visible_nodes(app, visible_nodes));

    let active_canvas = phase3_resolve_active_canvas_profile();
    let canvas_profile = &active_canvas.profile;
    let requested_layout_id = requested_layout_algorithm_id(app, view_id, canvas_profile);
    let resolved_layout = phase3_resolve_layout_algorithm(Some(&requested_layout_id));
    let dynamic_layout = matches!(
        resolved_layout.resolved_id.as_str(),
        GRAPH_LAYOUT_FORCE_DIRECTED | GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
    );
    if should_apply_layout_algorithm(app, view_id, &resolved_layout.resolved_id) {
        if let Ok(execution) = phase3_apply_layout_algorithm_to_graph(
            app.domain_graph_mut(),
            Some(&requested_layout_id),
        ) {
            if execution.changed_positions > 0 {
                app.workspace.graph_runtime.egui_state_dirty = true;
            }
        }
        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.last_layout_algorithm_id = Some(resolved_layout.resolved_id.clone());
        }
    }

    // Viewport culling: compute visible node set from previous-frame camera
    // metadata and exclude off-screen nodes before rebuilding egui_state.
    // Gated by the canvas performance policy toggle.
    let culled_graph =
        if canvas_profile.performance.viewport_culling_enabled && filtered_graph.is_none() {
            viewport_culled_graph(ui, app, view_id)
        } else {
            None
        };

    let graph_for_render = culled_graph
        .as_ref()
        .or(filtered_graph.as_ref())
        .unwrap_or(app.render_graph());

    // Compute the current culled key set for change detection.
    let culled_node_keys: Option<HashSet<NodeKey>> = culled_graph
        .as_ref()
        .map(|g| g.nodes().map(|(key, _)| key).collect());

    // Only rebuild egui_state when:
    // - no state exists yet, or it was explicitly dirtied by a graph mutation,
    // - a search filter is active (filter graph changes every query frame),
    // - or the culled node set changed (nodes entered/left the viewport).
    // Do NOT rebuild every frame merely because culling is active — that would
    // reset egui_graphs' physics node state (FR velocity) every frame, causing
    // culled-then-visible nodes to "respawn" at unexpected positions.
    let culled_set_changed = culled_node_keys != app.workspace.graph_runtime.last_culled_node_keys;

    if app.workspace.graph_runtime.egui_state.is_none()
        || app.workspace.graph_runtime.egui_state_dirty
        || filtered_graph.is_some()
        || (culled_graph.is_some() && culled_set_changed)
    {
        let crashed_nodes: HashSet<NodeKey> = app.crash_blocked_node_keys().collect();
        let memberships_by_uuid: HashMap<Uuid, Vec<String>> = graph_for_render
            .nodes()
            .map(|(_, node)| {
                (
                    node.id,
                    app.membership_for_node(node.id)
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let archived_nodes: HashSet<NodeKey> = graph_for_render
            .nodes()
            .filter_map(|(key, _)| {
                crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &key)
                    .iter()
                    .any(|tag| is_archived_tag(tag))
                    .then_some(key)
            })
            .collect();
        let mut egui_state = EguiGraphState::from_graph_with_memberships_projection(
            graph_for_render,
            &view_selection,
            view_selection.primary(),
            &crashed_nodes,
            &memberships_by_uuid,
            &semantic_badges_by_key(app, graph_for_render),
            &archived_nodes,
            filtered_graph.is_none() && culled_graph.is_none(),
        );
        let theme_resolution = phase3_resolve_active_theme(app.default_registry_theme_id());
        egui_state.apply_edge_theme_tokens(theme_resolution.tokens.edge_tokens.clone());
        egui_state.apply_node_chrome_theme(theme_resolution.tokens.graph_node_chrome);
        app.workspace.graph_runtime.egui_state = Some(egui_state);
        app.workspace.graph_runtime.egui_state_dirty = false;
        app.workspace.graph_runtime.last_culled_node_keys = culled_node_keys;
    }

    apply_search_node_visuals(
        app,
        &view_selection,
        search_matches,
        active_search_match,
        search_query_active,
    );

    // Event collection buffer
    let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));

    // Graph settings resolved from layout domain canvas surface profile.
    let nav = graphshell_owned_navigation_settings();
    let interaction =
        canvas_interaction_settings(canvas_profile, radial_open, right_button_down, ctrl_pressed);
    let style = canvas_style_settings(canvas_profile);

    // `workspace.physics` is the canonical FR runtime state. egui_graphs gets a
    // transient copy each frame via `set_layout_state`, then we write back the
    // updated state after render. Per-view local simulation stores positions
    // only; it does not own a second physics state.
    let (mut physics_state, lens_config) =
        if let Some(view) = app.workspace.graph_runtime.views.get(&view_id) {
            (
                app.workspace.graph_runtime.physics.clone(),
                Some(view.resolved_physics_profile()),
            )
        } else {
            (app.workspace.graph_runtime.physics.clone(), None)
        };

    if dynamic_layout && let Some(lens) = lens_config {
        lens.apply_to_state(&mut physics_state);
    }

    if !dynamic_layout {
        physics_state.base.is_running = false;
    }

    // Keep egui_graphs layout cache aligned with app-owned FR state.
    let active_layout_kind =
        if resolved_layout.resolved_id == GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT {
            ActiveLayoutKind::BarnesHut
        } else {
            ActiveLayoutKind::ForceDirected
        };
    set_layout_state::<ActiveLayoutState>(
        ui,
        ActiveLayoutState {
            kind: active_layout_kind,
            physics: physics_state,
        },
        None,
    );

    // Intercept wheel input before GraphView renders so parent scroll handling
    // cannot consume the delta first.
    let graph_rect = ui.max_rect();
    let visible_graph_regions = graph_visible_screen_rects(graph_rect, app);
    let pointer_in_visible_graph_region = ui
        .input(|i| i.pointer.latest_pos())
        .is_some_and(|pointer| visible_graph_regions.contains_point(pointer));
    if pointer_in_visible_graph_region {
        let mut captured_wheel_zoom = false;
        ui.input_mut(|input| {
            let scroll_delta = if input.smooth_scroll_delta.y.abs() > f32::EPSILON {
                input.smooth_scroll_delta.y
            } else {
                input.raw_scroll_delta.y
            };

            if scroll_delta.abs() <= f32::EPSILON {
                return;
            }

            let ctrl_pressed = input.modifiers.ctrl || input.modifiers.command;
            let should_capture = canvas_profile.should_capture_wheel_zoom(ctrl_pressed);

            if should_capture {
                captured_wheel_zoom = true;
                let anchor = input.pointer.latest_pos().map(|p| (p.x, p.y));
                app.queue_pending_wheel_zoom_delta(view_id, scroll_delta, anchor);
                input.smooth_scroll_delta.y = 0.0;
                input.raw_scroll_delta.y = 0.0;
            } else {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UI_GRAPH_WHEEL_ZOOM_NOT_CAPTURED,
                    latency_us: 0,
                });
            }
        });
        if captured_wheel_zoom {
            set_focused_view_with_transition(app, Some(view_id));
        }
    }

    // Resolve canvas zone policy once — used for both backdrop rendering and force gating.
    let canvas_zones_enabled =
        crate::shell::desktop::runtime::registries::phase3_resolve_active_canvas_profile()
            .profile
            .zones_enabled();

    draw_scene_runtime_backdrops(ui, app, view_id, graph_view_metadata_id(None));

    // Render frame-affinity backdrops below nodes when zones are enabled.
    if canvas_zones_enabled {
        draw_frame_affinity_backdrops(ui, app, graph_view_metadata_id(None));
    }

    // Render the graph (nested scope for mutable borrow)
    let response = {
        let state = app
            .workspace
            .graph_runtime
            .egui_state
            .as_mut()
            .expect("egui_state should be initialized");

        ui.add(
            &mut GraphView::<
                _,
                _,
                _,
                _,
                GraphNodeShape,
                GraphEdgeShape,
                ActiveLayoutState,
                ActiveLayout,
            >::new(&mut state.graph)
            .with_navigations(&nav)
            .with_interactions(&interaction)
            .with_styles(&style)
            .with_event_sink(&events),
        )
    }; // Drop mutable borrow of app.workspace.graph_runtime.egui_state here

    if response.clicked() || response.secondary_clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::WidgetType::Button);
        info.label = Some(graph_canvas_accessibility_label(app, &view_selection));
        info
    });

    // Pull latest FR state from egui_graphs after this frame's layout step.
    let new_physics = get_layout_state::<ActiveLayoutState>(ui, None).physics;
    if dynamic_layout {
        app.workspace.graph_runtime.physics = new_physics;
    }

    // Apply semantic clustering and frame-affinity forces (UDC Phase 2 + lane:layout-semantics).
    let physics_extensions = app.workspace.graph_runtime.views.get(&view_id).map(|v| {
        v.resolved_physics_profile()
            .graph_physics_extensions(canvas_zones_enabled)
    });
    apply_graph_physics_extensions(app, physics_extensions);
    if let Some(collision_policy) = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.resolved_physics_profile().scene_collision_policy())
    {
        apply_scene_runtime_pass(
            app,
            view_id,
            simulate_scene_collision_policy(app, view_id, collision_policy),
        );
    }

    app.workspace.graph_runtime.hovered_graph_node = pointer_in_visible_graph_region
        .then(|| {
            app.workspace
                .graph_runtime
                .egui_state
                .as_ref()
                .and_then(|state| {
                    state
                        .graph
                        .hovered_node()
                        .and_then(|idx| state.get_key(idx))
                })
        })
        .flatten();
    // Match egui_graphs' internal MetadataFrame storage key exactly.
    // egui_graphs calls data.insert_persisted(Id::new(frame.get_id()), frame) where
    // get_id() returns Id::new("egui_graphs_metadata_") — so the stored key is
    // Id::new applied to that Id (double-hashed). custom_id=None matches the default
    // GraphView instance (no with_id() call).
    let metadata_id = graph_view_metadata_id(None);
    let lasso = collect_lasso_action(
        ui,
        app,
        !radial_open,
        metadata_id,
        app.lasso_binding_preference(),
        &visible_graph_regions,
    );
    let arrange_scene_active = scene_arrange_mode_active(app, view_id);
    app.workspace.graph_runtime.hovered_scene_region = if arrange_scene_active
        && pointer_in_visible_graph_region
        && app.workspace.graph_runtime.hovered_graph_node.is_none()
        && !radial_open
        && lasso.action.is_none()
    {
        scene_region_at_pointer(ui, app, view_id, metadata_id).map(|region_id| (view_id, region_id))
    } else {
        None
    };
    let scene_resize_handle_hit = if arrange_scene_active
        && pointer_in_visible_graph_region
        && app.workspace.graph_runtime.hovered_graph_node.is_none()
        && !radial_open
        && lasso.action.is_none()
    {
        scene_region_resize_handle_at_pointer(ui, app, view_id, metadata_id)
    } else {
        None
    };
    let scene_primary_click_eligible = arrange_scene_active
        && pointer_in_visible_graph_region
        && !radial_open
        && lasso.action.is_none()
        && app.workspace.graph_runtime.hovered_graph_node.is_none();
    let scene_handled_primary_click = if scene_primary_click_eligible
        && ui.input(|i| i.pointer.primary_clicked())
        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
    {
        let target = scene_resize_handle_hit
            .map(|handle_hit| {
                (
                    handle_hit.region_id,
                    SceneRegionDragMode::Resize(handle_hit.handle),
                )
            })
            .or_else(|| {
                app.workspace
                    .graph_runtime
                    .hovered_scene_region
                    .filter(|(hovered_view_id, _)| *hovered_view_id == view_id)
                    .map(|(_, hovered_region_id)| (hovered_region_id, SceneRegionDragMode::Move))
            });
        if let Some((region_id, mode)) = target {
            let pointer_canvas = pointer_canvas_pos(ui, metadata_id, pointer);
            app.set_graph_view_selected_scene_region(view_id, Some(region_id));
            app.workspace.graph_runtime.active_scene_region_drag = Some(SceneRegionDragState {
                view_id,
                region_id,
                mode,
                last_pointer_canvas_pos: pointer_canvas,
            });
            app.set_interacting(true);
            true
        } else {
            false
        }
    } else {
        false
    };
    let scene_drag_moved = if arrange_scene_active
        && app
            .workspace
            .graph_runtime
            .active_scene_region_drag
            .is_some_and(|drag| drag.view_id == view_id)
    {
        apply_active_scene_region_drag(ui, app, metadata_id)
    } else {
        false
    };

    if pointer_in_visible_graph_region
        && ui.input(|i| i.pointer.secondary_clicked())
        && !lasso.suppress_context_menu
        && let Some(target) = app.workspace.graph_runtime.hovered_graph_node
    {
        handle_hovered_node_secondary_click(
            ui.ctx(),
            app,
            view_id,
            target,
            ui.input(|i| i.pointer.latest_pos()),
        );
    }
    if pointer_in_visible_graph_region
        && ui.input(|i| i.pointer.secondary_clicked())
        && !lasso.suppress_context_menu
        && app.workspace.graph_runtime.hovered_graph_node.is_none()
        && app.workspace.graph_runtime.hovered_scene_region.is_none()
        && scene_resize_handle_hit.is_none()
        && let Some(frame_anchor) = frame_anchor_at_pointer(ui, app, metadata_id)
        && let Some(frame_node) = app.render_graph().get_node(frame_anchor)
    {
        handle_frame_backdrop_secondary_click(
            ui.ctx(),
            app,
            view_id,
            frame_node.title.clone(),
            ui.input(|i| i.pointer.latest_pos()),
        );
    }
    if pointer_in_visible_graph_region
        && ui.input(|i| i.pointer.primary_clicked())
        && let Some(target) = app.workspace.graph_runtime.hovered_graph_node
        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        && let Some(state) = app.workspace.graph_runtime.egui_state.as_ref()
        && let Some(node) = state.graph.node(target)
        && node.display().workspace_membership_count() > 0
    {
        let (circle_center, circle_radius) = if let Some(meta) = ui
            .ctx()
            .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
        {
            (
                meta.canvas_to_screen_pos(node.location()),
                meta.canvas_to_screen_size(node.display().radius()),
            )
        } else {
            (node.location(), node.display().radius())
        };
        if node
            .display()
            .workspace_badge_hit_rect_screen(circle_center, circle_radius)
            .is_some_and(|rect| rect.contains(pointer))
        {
            app.request_choose_frame_picker(target);
        }
    }
    draw_highlighted_edge_overlay(ui, app, response.id, metadata_id);
    draw_scene_simulate_overlays(ui, app, view_id, metadata_id);
    draw_hovered_node_tooltip(ui, app, response.id, metadata_id);
    draw_hovered_edge_tooltip(ui, app, response.id, metadata_id);
    let scene_action_overlay = draw_scene_region_action_overlay(ui, app, view_id, metadata_id);

    // Custom navigation handling (Zoom/Pan/Fit)
    // metadata_id targets the same slot egui_graphs uses, so writes are visible.
    // Do not let a sticky radial-menu flag disable graph camera controls.
    // The radial menu renderer runs later in the frame and should own its own clicks,
    // but pan/zoom/fit must remain available for stabilization.
    let custom_zoom = handle_custom_navigation(
        ui,
        &response,
        metadata_id,
        app,
        true,
        view_id,
        canvas_profile,
        radial_open,
        right_button_down,
    );

    if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
    {
        let frame = crate::app::GraphViewFrame {
            zoom: meta.zoom,
            pan_x: meta.pan.x,
            pan_y: meta.pan.y,
        };
        app.workspace
            .graph_runtime
            .graph_view_frames
            .insert(view_id, frame);
        if let Some(screen_rect) = effective_graph_screen_rect(ui.max_rect(), app)
            && let Some(canvas_rect) = canvas_rect_from_view_frame(screen_rect, frame)
        {
            app.workspace
                .graph_runtime
                .graph_view_canvas_rects
                .insert(view_id, canvas_rect);
        } else {
            app.workspace
                .graph_runtime
                .graph_view_canvas_rects
                .remove(&view_id);
        }
    }

    let split_open_modifier = ui.input(|i| i.modifiers.shift);
    let mut actions = collect_graph_actions(app, &events, split_open_modifier, ctrl_pressed);
    if let Some(keyboard_action) = collect_graph_keyboard_traversal_action(
        ui,
        &response,
        app,
        &view_selection,
        radial_open,
        lasso.action.is_some(),
    ) {
        actions.push(keyboard_action);
    }
    let edge_click_eligible = pointer_in_visible_graph_region
        && ui.input(|i| i.pointer.primary_clicked())
        && app.workspace.graph_runtime.hovered_graph_node.is_none()
        && app.workspace.graph_runtime.hovered_scene_region.is_none()
        && scene_resize_handle_hit.is_none()
        && !scene_action_overlay.pointer_over
        && !radial_open
        && lasso.action.is_none();
    if edge_click_eligible && let Some((from, to)) = edge_endpoints_at_pointer(ui, app, metadata_id)
    {
        actions.push(GraphAction::SetHighlightedEdge { from, to });
    }
    let frame_backdrop_click_eligible = pointer_in_visible_graph_region
        && app.workspace.graph_runtime.hovered_graph_node.is_none()
        && app.workspace.graph_runtime.hovered_scene_region.is_none()
        && scene_resize_handle_hit.is_none()
        && !scene_action_overlay.pointer_over
        && !radial_open
        && lasso.action.is_none();
    if frame_backdrop_click_eligible
        && ui.input(|i| i.pointer.primary_clicked())
        && let Some(frame_anchor) = frame_anchor_at_pointer(ui, app, metadata_id)
        && let Some(frame_node) = app.render_graph().get_node(frame_anchor)
    {
        let frame_name = frame_node.title.clone();
        actions.push(GraphAction::SelectFrame(frame_name));
    }
    if frame_backdrop_click_eligible
        && ui.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        })
        && let Some(frame_anchor) = frame_anchor_at_pointer(ui, app, metadata_id)
        && let Some(frame_node) = app.render_graph().get_node(frame_anchor)
    {
        let frame_name = frame_node.title.clone();
        if let Some(member_key) = app
            .arrangement_projection_groups()
            .into_iter()
            .find(|group| {
                group.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                    && group.container_key == frame_anchor
            })
            .and_then(|group| group.member_keys.into_iter().next())
        {
            actions.push(GraphAction::FocusFrame {
                frame_name,
                member_key,
            });
        }
    }
    let node_or_frame_primary_click = actions.iter().any(|action| {
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
        )
    });
    if node_or_frame_primary_click {
        app.set_graph_view_selected_scene_region(view_id, None);
    }
    let graph_handled_primary_click = scene_handled_primary_click
        || scene_action_overlay.action_invoked
        || actions.iter().any(action_handles_primary_click);
    let clear_selection_on_background_click = ui.input(|i| {
        should_clear_selection_on_background_click(
            pointer_in_visible_graph_region
                && i.pointer.primary_clicked()
                && !scene_action_overlay.pointer_over,
            i.modifiers,
            app.workspace.graph_runtime.hovered_graph_node,
            graph_handled_primary_click,
            radial_open,
            lasso.action.is_some(),
        )
    });
    if clear_selection_on_background_click {
        app.set_graph_view_selected_scene_region(view_id, None);
        actions.push(GraphAction::ClearSelection);
        if app.workspace.graph_runtime.highlighted_graph_edge.is_some() {
            actions.push(GraphAction::ClearHighlightedEdge);
        }
    }
    if let Some(lasso_action) = lasso.action {
        actions.push(lasso_action);
    }
    if let Some(zoom) = custom_zoom {
        actions.push(GraphAction::Zoom(zoom));
    }
    if clear_selection_on_background_click
        || scene_handled_primary_click
        || scene_drag_moved
        || scene_action_overlay.action_invoked
        || !actions.is_empty()
    {
        set_focused_view_with_transition(app, Some(view_id));
    }
    actions
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

/// Sync projected node positions from egui_graphs layout state back into app graph state.
///
/// Pinned nodes keep their app-authored projected positions; their visual positions are
/// restored after layout so FR simulation does not move them.
///
/// **Group drag**: when the user is actively dragging (`is_interacting`) with
/// 2+ nodes selected, the dragged node's per-frame delta is detected by comparing
/// its egui_graphs position to its last-known `app.workspace.domain.graph` position.  That same
/// delta is then applied to every other selected (non-pinned) node in both
/// `egui_state` and the graph's projected-position lane, keeping the group moving
/// together without committing durable node positions every frame.
pub(crate) fn sync_graph_positions_from_layout(app: &mut GraphBrowserApp) {
    let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        if app.workspace.graph_runtime.is_interacting {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_LAYOUT_SYNC_BLOCKED_NO_STATE,
                latency_us: 0,
            });
        }
        return;
    };

    let layout_positions: Vec<(NodeKey, Point2D<f32>)> = app
        .workspace
        .domain
        .graph
        .nodes()
        .filter_map(|(key, _)| {
            state
                .graph
                .node(key)
                .map(|n| (key, Point2D::new(n.location().x, n.location().y)))
        })
        .collect();

    // Detect group drag: during active interaction with 2+ selected nodes, find
    // the node whose egui_graphs position diverged from app.workspace.domain.graph this frame.
    // This is the node the user is physically dragging.
    let focused_selection = app.focused_selection().clone();
    let selection_drag_deltas: HashMap<NodeKey, egui::Vec2> =
        if app.workspace.graph_runtime.is_interacting && !focused_selection.is_empty() {
            layout_positions
                .iter()
                .filter_map(|(key, egui_pos)| {
                    if !focused_selection.contains(key) {
                        return None;
                    }
                    let app_pos = app.domain_graph().node_projected_position(*key)?;
                    let delta = egui::Vec2::new(egui_pos.x - app_pos.x, egui_pos.y - app_pos.y);
                    (delta.length() > 0.01).then_some((*key, delta))
                })
                .collect()
        } else {
            HashMap::new()
        };
    let group_drag_delta: Option<(NodeKey, egui::Vec2)> =
        if app.workspace.graph_runtime.is_interacting && focused_selection.len() > 1 {
            selection_drag_deltas
                .iter()
                .next()
                .map(|(key, delta)| (*key, *delta))
        } else {
            None
        };

    let mut pinned_positions = Vec::new();
    for (key, pos) in layout_positions {
        if let Some(node) = app.domain_graph().get_node(key) {
            if node.is_pinned {
                if let Some(position) = app.domain_graph().node_projected_position(key) {
                    pinned_positions.push((key, position));
                }
            } else {
                let _ = app.domain_graph_mut().set_node_projected_position(key, pos);
            }
        }
    }

    // Propagate the drag delta to secondary selected nodes.
    let mut simulate_release_impulses = selection_drag_deltas;
    if let Some((dragged_key, delta)) = group_drag_delta {
        let secondary_keys: Vec<NodeKey> = focused_selection
            .iter()
            .filter(|&&k| k != dragged_key)
            .copied()
            .collect();

        let mut secondary_updates: Vec<(NodeKey, egui::Pos2)> = Vec::new();
        for other_key in secondary_keys {
            if let Some(node) = app.domain_graph().get_node(other_key)
                && !node.is_pinned
                && let Some(position) = app.domain_graph().node_projected_position(other_key)
            {
                let next_pos =
                    euclid::default::Point2D::new(position.x + delta.x, position.y + delta.y);
                let _ = app
                    .domain_graph_mut()
                    .set_node_projected_position(other_key, next_pos);
                secondary_updates.push((other_key, next_pos.to_pos2()));
                simulate_release_impulses.insert(other_key, delta);
            }
        }
        if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut() {
            for (key, pos) in secondary_updates {
                if let Some(egui_node) = state_mut.graph.node_mut(key) {
                    egui_node.set_location(pos);
                }
            }
        }
    }

    record_simulate_release_impulses(app, simulate_release_impulses);

    if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut() {
        for (key, pos) in pinned_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(pos.to_pos2());
            }
        }
    }
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
                                true,
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
                    true,
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
    fn requested_layout_algorithm_prefers_lens_override_when_present() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Layout");
        view.lens_state.base_lens_id = Some("lens:default".to_string());
        view.layout_policy.mode = LayoutMode::Grid { gap: 24.0 };
        app.workspace.graph_runtime.views.insert(view_id, view);

        let canvas_profile = crate::registries::domain::layout::canvas::CanvasRegistry::default()
            .resolve(crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT)
            .profile;

        let requested = requested_layout_algorithm_id(&app, view_id, &canvas_profile);

        assert_eq!(requested, crate::app::graph_layout::GRAPH_LAYOUT_GRID);
    }

    #[test]
    fn requested_layout_algorithm_prefers_free_layout_algorithm_override() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Layout");
        view.layout_policy.mode = LayoutMode::Free;
        view.layout_policy.algorithm_id =
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT.to_string();
        app.workspace.graph_runtime.views.insert(view_id, view);

        let canvas_profile = crate::registries::domain::layout::canvas::CanvasRegistry::default()
            .resolve(crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT)
            .profile;

        let requested = requested_layout_algorithm_id(&app, view_id, &canvas_profile);

        assert_eq!(
            requested,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
        );
    }

    #[test]
    fn should_apply_layout_algorithm_detects_view_layout_change() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Layout");
        view.last_layout_algorithm_id =
            Some(crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED.to_string());
        app.workspace.graph_runtime.views.insert(view_id, view);
        app.workspace.graph_runtime.egui_state = Some(EguiGraphState::from_graph(
            app.domain_graph(),
            &HashSet::new(),
        ));
        app.workspace.graph_runtime.egui_state_dirty = false;

        assert!(should_apply_layout_algorithm(
            &app,
            view_id,
            crate::app::graph_layout::GRAPH_LAYOUT_GRID
        ));
        assert!(!should_apply_layout_algorithm(
            &app,
            view_id,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED
        ));
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
    fn locked_camera_autofit_requires_physics_running_and_not_dragging() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new("AutoFit Lock Test"),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.set_camera_fit_locked(true);

        app.workspace.graph_runtime.physics.base.is_running = false;
        app.workspace.graph_runtime.is_interacting = false;
        assert!(
            !should_auto_fit_locked_camera(&app),
            "fit-lock should not auto-fit when physics is idle"
        );

        app.workspace.graph_runtime.physics.base.is_running = true;
        app.workspace.graph_runtime.is_interacting = true;
        assert!(
            !should_auto_fit_locked_camera(&app),
            "fit-lock should not auto-fit during active drag interaction"
        );

        app.workspace.graph_runtime.physics.base.is_running = true;
        app.workspace.graph_runtime.is_interacting = false;
        assert!(
            should_auto_fit_locked_camera(&app),
            "fit-lock should auto-fit while physics is running and interaction is idle"
        );
    }

    #[test]
    fn unlocked_camera_never_autofits() {
        let mut app = test_app();
        app.set_camera_fit_locked(false);
        app.workspace.graph_runtime.physics.base.is_running = true;
        app.workspace.graph_runtime.is_interacting = false;

        assert!(
            !should_auto_fit_locked_camera(&app),
            "unlocked camera should never auto-fit from lock-mode path"
        );
    }

    #[test]
    fn camera_fit_lock_recenter_requires_explicit_fit_command() {
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::default();
        let metadata_id = graph_view_metadata_id(None);
        let mut app = test_app();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, crate::app::GraphViewState::new("Fit Guardrail"));
        app.workspace.graph_runtime.focused_view = Some(view_id);

        app.add_node_and_sync("https://a.example".into(), Point2D::new(-120.0, -80.0));
        app.add_node_and_sync("https://b.example".into(), Point2D::new(180.0, 140.0));
        app.workspace.graph_runtime.egui_state =
            Some(EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            ));

        let mut zoom_without_fit = None;
        let mut zoom_with_fit = None;
        let mut pan_after_manual = Vec2::ZERO;
        let mut pan_after_fit = Vec2::ZERO;

        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        ));
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let active_canvas = phase3_resolve_active_canvas_profile();
                let canvas_profile = &active_canvas.profile;

                ctx.data_mut(|data| {
                    let mut frame = MetadataFrame::default();
                    frame.zoom = 1.0;
                    frame.pan = egui::vec2(30.0, -20.0);
                    data.insert_persisted(metadata_id, frame);
                });

                assert!(apply_background_pan(
                    ctx,
                    metadata_id,
                    &mut app,
                    view_id,
                    egui::vec2(15.0, -5.0)
                ));

                zoom_without_fit = apply_pending_camera_command(
                    ui,
                    &mut app,
                    metadata_id,
                    view_id,
                    canvas_profile,
                );

                ctx.data_mut(|data| {
                    pan_after_manual = data
                        .get_persisted::<MetadataFrame>(metadata_id)
                        .expect("metadata should stay persisted after manual pan")
                        .pan;
                });

                app.request_camera_command_for_view(Some(view_id), CameraCommand::Fit);
                zoom_with_fit = apply_pending_camera_command(
                    ui,
                    &mut app,
                    metadata_id,
                    view_id,
                    canvas_profile,
                );

                ctx.data_mut(|data| {
                    pan_after_fit = data
                        .get_persisted::<MetadataFrame>(metadata_id)
                        .expect("metadata should stay persisted after explicit fit")
                        .pan;
                });
            });
        });

        assert!(zoom_without_fit.is_none());
        assert!(zoom_with_fit.is_some());
        assert_ne!(pan_after_manual, pan_after_fit);
        assert!(app.pending_camera_command().is_none());
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
    fn lasso_mode_resolution_right_drag_is_deterministic() {
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::RightDrag, false, false, false),
            SelectionUpdateMode::Replace
        );
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::RightDrag, true, false, false),
            SelectionUpdateMode::Add
        );
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::RightDrag, false, true, false),
            SelectionUpdateMode::Add
        );
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::RightDrag, false, false, true),
            SelectionUpdateMode::Toggle
        );
    }

    #[test]
    fn lasso_mode_resolution_shift_left_drag_is_deterministic() {
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::ShiftLeftDrag, false, true, false),
            SelectionUpdateMode::Replace
        );
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::ShiftLeftDrag, true, true, false),
            SelectionUpdateMode::Add
        );
        assert_eq!(
            resolve_lasso_selection_mode(CanvasLassoBinding::ShiftLeftDrag, false, true, true),
            SelectionUpdateMode::Toggle
        );
    }

    #[test]
    fn normalize_lasso_keys_sorts_and_deduplicates() {
        let k0 = NodeKey::new(0);
        let k1 = NodeKey::new(1);
        let k2 = NodeKey::new(2);
        let normalized = normalize_lasso_keys(vec![k2, k0, k1, k0, k2]);
        assert_eq!(normalized, vec![k0, k1, k2]);
    }

    #[test]
    fn keyboard_traversal_advances_and_wraps_in_deterministic_order() {
        let mut app = test_app();
        let k0 = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let k1 = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let k2 = app.add_node_and_sync("c".into(), Point2D::new(20.0, 0.0));

        assert_eq!(
            next_keyboard_traversal_node(&app, app.focused_selection(), false),
            Some(k0)
        );

        app.select_node(k0, false);
        assert_eq!(
            next_keyboard_traversal_node(&app, app.focused_selection(), false),
            Some(k1)
        );

        app.select_node(k2, false);
        assert_eq!(
            next_keyboard_traversal_node(&app, app.focused_selection(), false),
            Some(k0)
        );
    }

    #[test]
    fn keyboard_traversal_reverse_wraps_to_last_when_unfocused() {
        let mut app = test_app();
        app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let k1 = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        assert_eq!(
            next_keyboard_traversal_node(&app, app.focused_selection(), true),
            Some(k1)
        );
    }

    #[test]
    fn graph_canvas_accessibility_label_includes_focused_node_name() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com/path".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example title".to_string();
        }
        app.select_node(key, false);

        let label = graph_canvas_accessibility_label(&app, app.focused_selection());
        assert!(label.contains("Focused node: Example title"));
        assert!(label.contains("Tab"));
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
            key: NodeIndex::new(0),
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
    fn test_keyboard_pan_delta_from_keys_basic_directions() {
        let left = keyboard_pan_delta_from_keys(
            KeyboardPanKeys {
                left: true,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(left, Vec2::new(10.0, 0.0));

        let right = keyboard_pan_delta_from_keys(
            KeyboardPanKeys {
                right: true,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(right, Vec2::new(-10.0, 0.0));

        let up = keyboard_pan_delta_from_keys(
            KeyboardPanKeys {
                up: true,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(up, Vec2::new(0.0, 10.0));

        let down = keyboard_pan_delta_from_keys(
            KeyboardPanKeys {
                down: true,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(down, Vec2::new(0.0, -10.0));
    }

    #[test]
    fn test_keyboard_pan_delta_from_keys_opposite_cancel_out() {
        let delta = keyboard_pan_delta_from_keys(
            KeyboardPanKeys {
                left: true,
                right: true,
                up: true,
                down: true,
            },
            10.0,
        );
        assert_eq!(delta, Vec2::ZERO);
    }

    #[test]
    fn test_keyboard_pan_delta_from_state_respects_input_mode() {
        let state = KeyboardPanInputState {
            wasd: KeyboardPanKeys {
                left: true,
                ..Default::default()
            },
            arrows: KeyboardPanKeys::default(),
        };

        let arrows_only =
            keyboard_pan_delta_from_state(state, 10.0, KeyboardPanInputMode::ArrowsOnly);
        assert_eq!(arrows_only, Vec2::ZERO);

        let both = keyboard_pan_delta_from_state(state, 10.0, KeyboardPanInputMode::WasdAndArrows);
        assert_eq!(both, Vec2::new(10.0, 0.0));
    }

    #[test]
    fn test_keyboard_pan_blocked_emits_fit_lock_diagnostic() {
        let mut diagnostics = DiagnosticsState::new();

        let blocked = emit_keyboard_pan_blocked_if_needed(Vec2::new(12.0, 0.0), false, true, true);

        assert!(blocked);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 1);
    }

    #[test]
    fn test_keyboard_pan_blocked_emits_inactive_view_diagnostic() {
        let mut diagnostics = DiagnosticsState::new();

        let blocked = emit_keyboard_pan_blocked_if_needed(Vec2::new(0.0, 8.0), false, false, false);

        assert!(blocked);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 1);
    }

    #[test]
    fn test_keyboard_pan_blocked_ignores_zero_delta_or_text_input_capture() {
        let mut diagnostics = DiagnosticsState::new();

        let zero_delta_blocked =
            emit_keyboard_pan_blocked_if_needed(Vec2::ZERO, false, true, false);
        let text_capture_blocked =
            emit_keyboard_pan_blocked_if_needed(Vec2::new(5.0, 0.0), true, true, false);

        assert!(!zero_delta_blocked);
        assert!(!text_capture_blocked);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let fit_lock_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let inactive_view_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(fit_lock_count, 0);
        assert_eq!(inactive_view_count, 0);
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
    fn test_format_elapsed_ago_units() {
        assert_eq!(format_elapsed_ago(Duration::from_secs(2)), "just now");
        assert_eq!(format_elapsed_ago(Duration::from_secs(12)), "12s ago");
        assert_eq!(format_elapsed_ago(Duration::from_secs(120)), "2m ago");
        assert_eq!(
            format_elapsed_ago(Duration::from_secs(60 * 60 * 3)),
            "3h ago"
        );
        assert_eq!(
            format_elapsed_ago(Duration::from_secs(60 * 60 * 24 * 2)),
            "2d ago"
        );
    }

    #[test]
    fn test_format_last_visited_with_future_timestamp_is_just_now() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(10);
        assert_eq!(format_last_visited_with_now(future, now), "just now");
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
    fn test_search_highlight_mode_dims_not_hides() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.search_display_mode = SearchDisplayMode::Highlight;
        app.workspace.graph_runtime.egui_state =
            Some(EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            ));
        let matches = HashSet::from([a]);
        let selection = app.focused_selection().clone();
        apply_search_node_visuals(&mut app, &selection, &matches, Some(a), true);

        let state = app.workspace.graph_runtime.egui_state.as_ref().unwrap();
        assert!(state.graph.node(a).is_some());
        assert!(state.graph.node(b).is_some());
        let b_color = state.graph.node(b).unwrap().color().unwrap();
        let presentation =
            crate::registries::domain::presentation::PresentationDomainRegistry::default()
                .resolve_profile("physics:default", "theme:default")
                .profile;
        assert!(b_color != lifecycle_color(&presentation, NodeLifecycle::Cold));
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

    /// Simulate the sync conditions for group drag:
    /// Build egui_state from the graph, move the dragged node in egui_state,
    /// then run sync and assert secondary selected nodes follow.
    fn setup_group_drag_sync(app: &mut GraphBrowserApp, dragged_key: NodeKey, delta: egui::Vec2) {
        use crate::graph::egui_adapter::EguiGraphState;
        // Build egui_state seeded from current app.workspace.domain.graph positions.
        app.workspace.graph_runtime.egui_state = Some(EguiGraphState::from_graph(
            &app.workspace.domain.graph,
            &std::collections::HashSet::new(),
        ));
        // Simulate egui_graphs moving the dragged node by delta.
        if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut() {
            if let Some(node) = state_mut.graph.node_mut(dragged_key) {
                let old = node.location();
                node.set_location(
                    euclid::default::Point2D::new(old.x + delta.x, old.y + delta.y).to_pos2(),
                );
            }
        }
        app.workspace.graph_runtime.is_interacting = true;
    }

    #[test]
    fn test_group_drag_moves_secondary_selected_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(200.0, 0.0));

        // Select A (primary) and B (secondary); C is unselected.
        app.select_node(a, false);
        app.select_node(b, true);

        let delta = egui::Vec2::new(10.0, 20.0);
        setup_group_drag_sync(&mut app, a, delta);
        sync_graph_positions_from_layout(&mut app);

        // A moved to its dragged position.
        let a_pos = app
            .workspace
            .domain
            .graph
            .get_node(a)
            .unwrap()
            .projected_position();
        assert!((a_pos.x - 10.0).abs() < 0.1, "a.x={}", a_pos.x);
        assert!((a_pos.y - 20.0).abs() < 0.1, "a.y={}", a_pos.y);

        // B followed by the same delta.
        let b_pos = app
            .workspace
            .domain
            .graph
            .get_node(b)
            .unwrap()
            .projected_position();
        assert!((b_pos.x - 110.0).abs() < 0.1, "b.x={}", b_pos.x);
        assert!((b_pos.y - 20.0).abs() < 0.1, "b.y={}", b_pos.y);

        // C was not selected — stays put.
        let c_pos = app
            .workspace
            .domain
            .graph
            .get_node(c)
            .unwrap()
            .projected_position();
        assert!((c_pos.x - 200.0).abs() < 0.1, "c.x={}", c_pos.x);
        assert!((c_pos.y - 0.0).abs() < 0.1, "c.y={}", c_pos.y);
    }

    #[test]
    fn test_group_drag_no_propagation_when_single_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Only A selected.
        app.select_node(a, false);

        let delta = egui::Vec2::new(10.0, 20.0);
        setup_group_drag_sync(&mut app, a, delta);
        sync_graph_positions_from_layout(&mut app);

        // B must not move (single selection — no group drag).
        let b_pos = app
            .workspace
            .domain
            .graph
            .get_node(b)
            .unwrap()
            .projected_position();
        assert!((b_pos.x - 100.0).abs() < 0.1, "b.x={}", b_pos.x);
        assert!((b_pos.y - 0.0).abs() < 0.1, "b.y={}", b_pos.y);
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
    fn viewport_culling_benchmark_reports_lower_prep_time_than_full_rebuild() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for row in 0..30 {
            for col in 0..30 {
                let key = app.add_node_and_sync(
                    format!("https://bench.example/{row}/{col}"),
                    Point2D::new(col as f32 * 30.0, row as f32 * 30.0),
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

        let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(260.0, 260.0));
        let culled_graph =
            viewport_culled_graph_for_canvas_rect(&app.workspace.domain.graph, canvas_rect)
                .expect("expected culled graph for benchmark viewport");

        let full_start = Instant::now();
        for _ in 0..12 {
            let state = EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            );
            black_box(state.graph.node_count());
        }
        let full_elapsed = full_start.elapsed();

        let culled_start = Instant::now();
        for _ in 0..12 {
            let state = EguiGraphState::from_graph_with_visual_state(
                &culled_graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            );
            black_box(state.graph.node_count());
        }
        let culled_elapsed = culled_start.elapsed();

        assert!(
            culled_elapsed < full_elapsed,
            "expected culled prep to be faster; full={:?}, culled={:?}",
            full_elapsed,
            culled_elapsed
        );
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
    fn background_pan_updates_metadata_and_focus_for_non_zero_delta() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, crate::app::GraphViewState::new("Pan Test"));

        ctx.data_mut(|data| {
            let mut frame = MetadataFrame::default();
            frame.zoom = 1.0;
            frame.pan = egui::vec2(10.0, 20.0);
            data.insert_persisted(metadata_id, frame);
        });

        let changed =
            apply_background_pan(&ctx, metadata_id, &mut app, view_id, egui::vec2(15.0, -5.0));

        assert!(changed);
        assert_eq!(app.workspace.graph_runtime.focused_view, Some(view_id));
        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("background pan should keep metadata persisted");
            assert!((meta.pan.x - 25.0).abs() < 0.001);
            assert!((meta.pan.y - 15.0).abs() < 0.001);
        });
    }

    #[test]
    fn background_pan_is_noop_for_zero_delta() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan-zero");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, crate::app::GraphViewState::new("Zero Pan Test"));

        ctx.data_mut(|data| {
            let mut frame = MetadataFrame::default();
            frame.zoom = 1.0;
            frame.pan = egui::vec2(10.0, 20.0);
            data.insert_persisted(metadata_id, frame);
        });

        let changed = apply_background_pan(&ctx, metadata_id, &mut app, view_id, Vec2::ZERO);

        assert!(!changed);
        assert_eq!(app.workspace.graph_runtime.focused_view, None);
        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("zero-delta pan should leave metadata intact");
            assert!((meta.pan.x - 10.0).abs() < 0.001);
            assert!((meta.pan.y - 20.0).abs() < 0.001);
        });
    }

    #[test]
    fn background_pan_seeds_metadata_when_missing() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan-missing-meta");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, crate::app::GraphViewState::new("Seed Pan Test"));
        app.workspace.graph_runtime.graph_view_frames.insert(
            view_id,
            crate::app::GraphViewFrame {
                zoom: 1.0,
                pan_x: 5.0,
                pan_y: 7.0,
            },
        );

        let changed =
            apply_background_pan(&ctx, metadata_id, &mut app, view_id, egui::vec2(3.0, -2.0));

        assert!(changed);
        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("missing metadata should be seeded on pan");
            assert!((meta.pan.x - 8.0).abs() < 0.001);
            assert!((meta.pan.y - 5.0).abs() < 0.001);
        });
    }

    #[test]
    fn background_pan_records_inertia_velocity_when_enabled() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan-inertia-enabled");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, crate::app::GraphViewState::new("Inertia Pan Test"));

        let changed =
            apply_background_pan(&ctx, metadata_id, &mut app, view_id, egui::vec2(6.0, -4.0));
        assert!(changed);

        ctx.data_mut(|data| {
            let velocity = data
                .get_persisted::<Vec2>(pan_inertia_velocity_id(metadata_id))
                .expect("pan inertia velocity should be recorded when enabled");
            assert!((velocity.x - 6.0).abs() < 0.001);
            assert!((velocity.y + 4.0).abs() < 0.001);
        });
    }

    #[test]
    fn pan_inertia_decay_moves_camera_when_manual_pan_is_idle() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-pan-inertia-decay");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new("Inertia Decay Test"),
        );
        app.set_camera_pan_inertia_enabled(true);
        app.set_camera_pan_inertia_damping(0.80);

        ctx.data_mut(|data| {
            let mut frame = MetadataFrame::default();
            frame.zoom = 1.0;
            frame.pan = egui::vec2(0.0, 0.0);
            data.insert_persisted(metadata_id, frame);
            data.insert_persisted(pan_inertia_velocity_id(metadata_id), egui::vec2(10.0, 0.0));
        });

        let applied = apply_background_pan_inertia(&ctx, metadata_id, &app, view_id);
        assert!(applied);

        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("metadata frame should remain persisted after inertia");
            assert!((meta.pan.x - 10.0).abs() < 0.001);
            assert!((meta.pan.y - 0.0).abs() < 0.001);

            let velocity = data
                .get_persisted::<Vec2>(pan_inertia_velocity_id(metadata_id))
                .expect("damped velocity should remain for subsequent inertia frames");
            assert!(velocity.x < 10.0);
            assert!(velocity.x > 0.0);
        });
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

    #[test]
    fn lasso_state_ids_are_scoped_per_metadata_id() {
        let metadata_a = egui::Id::new("view-a").with("metadata");
        let metadata_b = egui::Id::new("view-b").with("metadata");

        let (start_a, moved_a) = lasso_state_ids(metadata_a);
        let (start_a_repeat, moved_a_repeat) = lasso_state_ids(metadata_a);
        let (start_b, moved_b) = lasso_state_ids(metadata_b);

        assert_eq!(start_a, start_a_repeat);
        assert_eq!(moved_a, moved_a_repeat);
        assert_ne!(start_a, start_b);
        assert_ne!(moved_a, moved_b);
    }

    #[test]
    fn camera_fit_lock_uses_egui_graphs_double_hashed_metadata_id() {
        let raw_metadata_id = MetadataFrame::new(None).get_id();
        let metadata_id = graph_view_metadata_id(None);

        assert_eq!(metadata_id, egui::Id::new(raw_metadata_id));
        assert_ne!(metadata_id, raw_metadata_id);
    }

    #[test]
    fn camera_fit_lock_metadata_id_scopes_with_custom_graph_view_id() {
        let left = graph_view_metadata_id(Some("left-view".to_string()));
        let right = graph_view_metadata_id(Some("right-view".to_string()));

        assert_ne!(left, right);
    }
}
