/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module using egui_graphs.
//!
//! Delegates graph visualization and interaction to the egui_graphs crate,
//! which provides built-in navigation (zoom/pan), node dragging, and selection.

use crate::app::{
    CameraCommand, ChooseFramePickerMode, GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry,
    GraphSearchOrigin, KeyboardPanInputMode, KeyboardZoomRequest, SearchDisplayMode,
    SelectionUpdateMode, TagPanelState, ThreeDMode, UnsavedFramePromptAction,
    UnsavedFramePromptRequest, ViewAction, ViewDimension, WorkbenchIntent, ZSource,
    graph_layout::{
        GRAPH_LAYOUT_FORCE_DIRECTED, GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT,
        layout_algorithm_id_for_mode,
    },
};
use crate::graph::badge::{Badge, BadgeVisual, badge_visuals, badges_for_node, is_archived_tag};
use crate::graph::egui_adapter::{EguiGraphState, GraphEdgeShape, GraphNodeShape};
use crate::graph::layouts::{ActiveLayout, ActiveLayoutKind, ActiveLayoutState};
use crate::graph::physics::apply_graph_physics_extensions;
use crate::graph::{EdgeKind, EdgePayload, NodeKey, NodeLifecycle};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::registries::domain::presentation::PresentationProfile;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::input::action_id;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_NO_BOUNDS, CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_ZERO_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_FIT_DEFERRED_NO_METADATA,
    CHANNEL_UI_GRAPH_CAMERA_ZOOM_DEFERRED_NO_METADATA, CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
    CHANNEL_UI_GRAPH_FIT_SELECTION_FALLBACK_TO_FIT, CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK,
    CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW,
    CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED_NO_METADATA, CHANNEL_UI_GRAPH_LASSO_BLOCKED_NO_STATE,
    CHANNEL_UI_GRAPH_LAYOUT_SYNC_BLOCKED_NO_STATE, CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT,
    CHANNEL_UI_GRAPH_WHEEL_ZOOM_BLOCKED_INVALID_FACTOR,
    CHANNEL_UI_GRAPH_WHEEL_ZOOM_DEFERRED_NO_METADATA, CHANNEL_UI_GRAPH_WHEEL_ZOOM_NOT_CAPTURED,
    CHANNEL_UX_NAVIGATION_TRANSITION, phase3_apply_layout_algorithm_to_graph,
    phase3_resolve_active_canvas_profile, phase3_resolve_active_presentation_profile,
    phase3_resolve_layout_algorithm,
};
use crate::util::CoordBridge;
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::{Color32, Stroke, Ui, Vec2, Window};
use egui_graphs::events::Event;
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
    get_layout_state, set_layout_state,
};
use euclid::default::Point2D;
use petgraph::stable_graph::NodeIndex;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

mod panels;
mod reducer_bridge;
mod spatial_index;
#[cfg(test)]
pub(crate) use panels::history_manager_entry_limit_for_tests;
pub use panels::{
    render_clip_inspector_panel, render_navigator_tool_pane_in_ui, render_help_panel,
    render_history_manager_in_ui, render_settings_overlay_panel,
    render_settings_node_viewer_in_ui, render_settings_tool_pane_in_ui_with_control_panel,
};
use reducer_bridge::{apply_reducer_graph_intents_hardened, apply_ui_intents_with_checkpoint};
use spatial_index::NodeSpatialIndex;

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
    FocusNodeSplit(NodeKey),
    DragStart,
    DragEnd(NodeKey, Point2D<f32>),
    MoveNode(NodeKey, Point2D<f32>),
    SelectNode {
        key: NodeKey,
        multi_select: bool,
    },
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
    let previous_focused_view = app.workspace.focused_view;
    app.workspace.focused_view = focused_view;
    if app.workspace.focused_view != previous_focused_view {
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
            | GraphAction::FocusNodeSplit(_)
            | GraphAction::DragStart
            | GraphAction::DragEnd(_, _)
            | GraphAction::MoveNode(_, _)
            | GraphAction::SelectNode { .. }
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
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.show_radial_menu {
                app.enqueue_workbench_intent(WorkbenchIntent::ToggleRadialMenu);
            }
            if let Some(pointer) = pointer {
                ctx.data_mut(|d| {
                    d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                });
            }
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            app.set_pending_node_context_target(Some(target));
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
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
        .focused_view
        .is_some_and(|focused| !app.workspace.views.contains_key(&focused))
    {
        set_focused_view_with_transition(app, None);
    }

    let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let right_button_down = ui.input(|i| i.pointer.secondary_down());
    let radial_open = app.workspace.show_radial_menu;
    let filtered_graph =
        if matches!(search_display_mode, SearchDisplayMode::Filter) && search_query_active {
            Some(filtered_graph_for_search(app, search_matches))
        } else {
            None
        };

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
                app.workspace.egui_state_dirty = true;
            }
        }
        if let Some(view) = app.workspace.views.get_mut(&view_id) {
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
        .unwrap_or(&app.workspace.domain.graph);

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
    let culled_set_changed = culled_node_keys != app.workspace.last_culled_node_keys;

    if app.workspace.egui_state.is_none()
        || app.workspace.egui_state_dirty
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
        app.workspace.egui_state = Some(EguiGraphState::from_graph_with_memberships_projection(
            graph_for_render,
            &view_selection,
            view_selection.primary(),
            &crashed_nodes,
            &memberships_by_uuid,
            &semantic_badges_by_key(app, graph_for_render),
            &archived_nodes,
            filtered_graph.is_none() && culled_graph.is_none(),
        ));
        app.workspace.egui_state_dirty = false;
        app.workspace.last_culled_node_keys = culled_node_keys;
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
    let (mut physics_state, lens_config) = if let Some(view) = app.workspace.views.get(&view_id) {
        (app.workspace.physics.clone(), Some(&view.lens))
    } else {
        (app.workspace.physics.clone(), None)
    };

    if dynamic_layout && let Some(lens) = lens_config {
        lens.physics.apply_to_state(&mut physics_state);
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
    if ui.rect_contains_pointer(graph_rect) {
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

    // Render the graph (nested scope for mutable borrow)
    let response = {
        let state = app
            .workspace
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
    }; // Drop mutable borrow of app.workspace.egui_state here

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
        app.workspace.physics = new_physics;
    }

    // Apply semantic clustering forces if enabled (UDC Phase 2)
    let physics_extensions = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.lens.physics.graph_physics_extensions());
    apply_graph_physics_extensions(app, physics_extensions);

    app.workspace.hovered_graph_node = app.workspace.egui_state.as_ref().and_then(|state| {
        state
            .graph
            .hovered_node()
            .and_then(|idx| state.get_key(idx))
    });
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
    );

    if ui.input(|i| i.pointer.secondary_clicked())
        && !lasso.suppress_context_menu
        && let Some(target) = app.workspace.hovered_graph_node
    {
        handle_hovered_node_secondary_click(
            ui.ctx(),
            app,
            view_id,
            target,
            ui.input(|i| i.pointer.latest_pos()),
        );
    }
    if ui.input(|i| i.pointer.primary_clicked())
        && let Some(target) = app.workspace.hovered_graph_node
        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        && let Some(state) = app.workspace.egui_state.as_ref()
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
    draw_hovered_node_tooltip(ui, app, response.id, metadata_id);
    draw_hovered_edge_tooltip(ui, app, response.id, metadata_id);

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
        app.workspace.graph_view_frames.insert(
            view_id,
            crate::app::GraphViewFrame {
                zoom: meta.zoom,
                pan_x: meta.pan.x,
                pan_y: meta.pan.y,
            },
        );
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
    let edge_click_eligible = ui.input(|i| i.pointer.primary_clicked())
        && app.workspace.hovered_graph_node.is_none()
        && !radial_open
        && lasso.action.is_none();
    if edge_click_eligible && let Some((from, to)) = edge_endpoints_at_pointer(ui, app, metadata_id)
    {
        actions.push(GraphAction::SetHighlightedEdge { from, to });
    }
    let graph_handled_primary_click = actions.iter().any(action_handles_primary_click);
    let clear_selection_on_background_click = ui.input(|i| {
        should_clear_selection_on_background_click(
            i.pointer.primary_clicked(),
            i.modifiers,
            app.workspace.hovered_graph_node,
            graph_handled_primary_click,
            radial_open,
            lasso.action.is_some(),
        )
    });
    if clear_selection_on_background_click {
        actions.push(GraphAction::ClearSelection);
        if app.workspace.highlighted_graph_edge.is_some() {
            actions.push(GraphAction::ClearHighlightedEdge);
        }
    }
    if let Some(lasso_action) = lasso.action {
        actions.push(lasso_action);
    }
    if let Some(zoom) = custom_zoom {
        actions.push(GraphAction::Zoom(zoom));
    }
    if clear_selection_on_background_click || !actions.is_empty() {
        set_focused_view_with_transition(app, Some(view_id));
    }
    actions
}

fn draw_hovered_edge_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    if app.workspace.hovered_graph_node.is_some() {
        return;
    }
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let Some((from, to)) = edge_endpoints_at_pointer(ui, app, metadata_id) else {
        return;
    };

    let ab_payload = app
        .domain_graph()
        .find_edge_key(from, to)
        .and_then(|k| app.domain_graph().get_edge(k));
    let ba_payload = app
        .domain_graph()
        .find_edge_key(to, from)
        .and_then(|k| app.domain_graph().get_edge(k));

    let ab_count = ab_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let ba_count = ba_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let total = ab_count + ba_count;
    if ab_payload.is_none() && ba_payload.is_none() {
        return;
    }

    let latest_ts = ab_payload
        .into_iter()
        .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms))
        .chain(
            ba_payload
                .into_iter()
                .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms)),
        )
        .max();

    let from_label = app
        .domain_graph()
        .get_node(from)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(from).map(|n| n.url.as_str()))
        .unwrap_or("unknown");
    let to_label = app
        .domain_graph()
        .get_node(to)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(to).map(|n| n.url.as_str()))
        .unwrap_or("unknown");

    let latest_text = latest_ts
        .and_then(|ms| {
            UNIX_EPOCH
                .checked_add(Duration::from_millis(ms))
                .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
                .map(|d| format!("{}s", d.as_secs()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    let mut family_rows: BTreeSet<String> = BTreeSet::new();
    for payload in [ab_payload, ba_payload].into_iter().flatten() {
        for row in edge_family_rows(payload) {
            family_rows.insert(row);
        }
    }

    egui::Area::new(widget_id.with("edge_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + Vec2::new(14.0, 14.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new("Edge Inspection").strong());
                ui.label(format!("{from_label} <-> {to_label}"));
                ui.separator();
                if total > 0 {
                    ui.label(format!("{from_label} -> {to_label}: {ab_count}"));
                    ui.label(format!("{to_label} -> {from_label}: {ba_count}"));
                    ui.label(format!("Total traversals: {total}"));
                    ui.label(format!("Latest traversal: {latest_text}"));
                    ui.separator();
                }
                ui.label(egui::RichText::new("Family | Durability | Provenance").small());
                for row in family_rows {
                    ui.label(egui::RichText::new(row).small());
                }
            });
        });
}

fn edge_family_rows(payload: &EdgePayload) -> Vec<String> {
    let mut rows = Vec::new();
    if payload.has_kind(EdgeKind::Hyperlink) {
        rows.push("hyperlink | durable | graph.link_extraction".to_string());
    }
    if payload.has_kind(EdgeKind::TraversalDerived) {
        rows.push("history | durable | runtime.navigation_log".to_string());
    }
    if payload.has_kind(EdgeKind::UserGrouped) {
        rows.push("user-grouped | durable | user.explicit_grouping".to_string());
    }
    if let Some(arrangement) = payload.arrangement_data() {
        for sub_kind in &arrangement.sub_kinds {
            rows.push(format!(
                "arrangement/{} | {} | {}",
                sub_kind.as_tag(),
                sub_kind.durability().as_tag(),
                sub_kind.provenance()
            ));
        }
    }
    if rows.is_empty() {
        rows.push("unknown | session | runtime.edge_probe".to_string());
    }
    rows
}

fn edge_endpoints_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<(NodeKey, NodeKey)> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let state = app.workspace.egui_state.as_ref()?;
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))?;
    let edge_id = state.graph.edge_by_screen_pos(&meta, pointer)?;
    state.graph.edge_endpoints(edge_id)
}

fn draw_highlighted_edge_overlay(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    _widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    let presentation = active_presentation_profile(app);
    let Some((from, to)) = app.workspace.highlighted_graph_edge else {
        return;
    };
    let Some(state) = app.workspace.egui_state.as_ref() else {
        return;
    };
    let Some(from_node) = state.graph.node(from) else {
        return;
    };
    let Some(to_node) = state.graph.node(to) else {
        return;
    };
    let (from_screen, to_screen) = if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
    {
        (
            meta.canvas_to_screen_pos(from_node.location()),
            meta.canvas_to_screen_pos(to_node.location()),
        )
    } else {
        (from_node.location(), to_node.location())
    };
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(6.0, presentation.edge_highlight_backdrop.to_color32()),
    );
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(5.0, presentation.edge_highlight_foreground.to_color32()),
    );
    // Draw endpoint markers so edge-search selection is obvious even on dense graphs.
    ui.painter().circle_filled(
        from_screen,
        6.0,
        presentation.edge_highlight_foreground.to_color32(),
    );
    ui.painter().circle_filled(
        to_screen,
        6.0,
        presentation.edge_highlight_foreground.to_color32(),
    );
}

fn draw_hovered_node_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    let Some(key) = app.workspace.hovered_graph_node else {
        return;
    };
    let Some(node) = app.domain_graph().get_node(key) else {
        return;
    };
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());

    let lifecycle_text = if app.is_crash_blocked(key) {
        "Crashed".to_string()
    } else {
        match node.lifecycle {
            NodeLifecycle::Active => "Active".to_string(),
            NodeLifecycle::Warm => "Warm".to_string(),
            NodeLifecycle::Cold => "Cold".to_string(),
            NodeLifecycle::Tombstone => "Tombstone".to_string(),
        }
    };
    let last_visited_text = format_last_visited(node.last_visited);
    let workspace_memberships: Vec<String> =
        app.membership_for_node(node.id).iter().cloned().collect();
    let anchor = pointer_pos
        .or_else(|| {
            app.workspace.egui_state.as_ref().and_then(|state| {
                state.graph.node(key).map(|n| {
                    if let Some(meta) = ui
                        .ctx()
                        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
                    {
                        meta.canvas_to_screen_pos(n.location())
                    } else {
                        n.location()
                    }
                })
            })
        })
        .unwrap_or_else(|| ui.max_rect().center());

    egui::Area::new(widget_id.with("node_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(anchor + egui::vec2(14.0, 14.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.strong(if node.title.is_empty() {
                    &node.url
                } else {
                    &node.title
                });
                if !node.title.is_empty() && node.title != node.url {
                    ui.label(&node.url);
                }
                ui.small(format!("Last visited: {last_visited_text}"));
                ui.small(format!("Lifecycle: {lifecycle_text}"));
                if !workspace_memberships.is_empty() {
                    ui.separator();
                    ui.small(format!("Workspaces ({})", workspace_memberships.len()));
                    for workspace in &workspace_memberships {
                        ui.small(format!("- {workspace}"));
                    }
                }
            });
        });
}

fn format_last_visited(last_visited: SystemTime) -> String {
    let now = SystemTime::now();
    format_last_visited_with_now(last_visited, now)
}

fn format_last_visited_with_now(last_visited: SystemTime, now: SystemTime) -> String {
    let Ok(elapsed) = now.duration_since(last_visited) else {
        return "just now".to_string();
    };
    format_elapsed_ago(elapsed)
}

fn format_elapsed_ago(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 5 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    if secs < 60 * 60 {
        return format!("{}m ago", secs / 60);
    }
    if secs < 60 * 60 * 24 {
        return format!("{}h ago", secs / (60 * 60));
    }
    if secs < 60 * 60 * 24 * 7 {
        return format!("{}d ago", secs / (60 * 60 * 24));
    }
    format!("{}w ago", secs / (60 * 60 * 24 * 7))
}

fn filtered_graph_for_search(
    app: &GraphBrowserApp,
    search_matches: &HashSet<NodeKey>,
) -> crate::graph::Graph {
    let mut filtered = app.domain_graph().clone();
    let to_remove: Vec<NodeKey> = filtered
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !search_matches.contains(key))
        .collect();
    for key in to_remove {
        filtered.remove_node(key);
    }
    filtered
}

#[derive(Debug, Clone)]
struct ViewportCullingSelection {
    visible: HashSet<NodeKey>,
    extended: HashSet<NodeKey>,
}

#[derive(Debug, Clone, Copy)]
struct ViewportCullingMetrics {
    total_nodes: usize,
    visible_nodes: usize,
    submitted_nodes: usize,
    removed_nodes: usize,
    full_submission_units: usize,
    culled_submission_units: usize,
}

fn estimated_submission_units(graph: &crate::graph::Graph) -> usize {
    graph.node_count() + graph.edge_count()
}

fn viewport_culling_selection_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingSelection> {
    const DEFAULT_NODE_RADIUS: f32 = 5.0;
    let index = NodeSpatialIndex::build(graph.nodes().filter_map(|(key, _)| {
        graph.node_projected_position(key).map(|position| {
            let pos = position.to_pos2();
            (key, pos, DEFAULT_NODE_RADIUS)
        })
    }));

    let visible: HashSet<NodeKey> = index
        .nodes_in_canvas_rect(canvas_rect)
        .into_iter()
        .collect();
    if visible.is_empty() || visible.len() >= graph.node_count() {
        return None;
    }

    let mut extended = visible.clone();
    for edge in graph.edges() {
        if visible.contains(&edge.from) || visible.contains(&edge.to) {
            extended.insert(edge.from);
            extended.insert(edge.to);
        }
    }

    if extended.len() >= graph.node_count() {
        return None;
    }

    Some(ViewportCullingSelection { visible, extended })
}

fn viewport_culling_metrics_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingMetrics> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;
    let culled_edge_count = graph
        .edges()
        .filter(|edge| {
            selection.extended.contains(&edge.from) && selection.extended.contains(&edge.to)
        })
        .count();

    Some(ViewportCullingMetrics {
        total_nodes: graph.node_count(),
        visible_nodes: selection.visible.len(),
        submitted_nodes: selection.extended.len(),
        removed_nodes: graph.node_count().saturating_sub(selection.extended.len()),
        full_submission_units: estimated_submission_units(graph),
        culled_submission_units: selection.extended.len() + culled_edge_count,
    })
}

fn viewport_culled_graph_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<crate::graph::Graph> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;

    let mut culled = graph.clone();
    let to_remove: Vec<NodeKey> = culled
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !selection.extended.contains(key))
        .collect();
    for key in to_remove {
        culled.remove_node(key);
    }

    Some(culled)
}

/// Compute a viewport-culled graph containing only the nodes visible in the
/// current frame's viewport (plus any additional nodes needed to keep edges
/// intact).  Returns `None` when culling is not applicable — e.g. when there
/// is no previous-frame camera metadata yet, when the graph is small enough
/// that culling has no effect, or when the canvas rect cannot be computed.
///
/// **Edge ghost-endpoint policy**: If either endpoint of an edge is visible,
/// both endpoints are included in the culled graph so that the renderer
/// never sees an edge with a missing endpoint.
fn viewport_culled_graph(
    ui: &Ui,
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<crate::graph::Graph> {
    let frame = app.workspace.graph_view_frames.get(&view_id)?;
    let canvas_rect = canvas_rect_from_view_frame(ui.max_rect(), *frame)?;

    viewport_culled_graph_for_canvas_rect(&app.workspace.domain.graph, canvas_rect)
}

fn canvas_rect_from_view_frame(
    screen_rect: egui::Rect,
    frame: crate::app::GraphViewFrame,
) -> Option<egui::Rect> {
    if frame.zoom.abs() < f32::EPSILON {
        return None;
    }

    let pan = egui::vec2(frame.pan_x, frame.pan_y);
    let canvas_min = (screen_rect.min.to_vec2() - pan) / frame.zoom;
    let canvas_max = (screen_rect.max.to_vec2() - pan) / frame.zoom;
    let canvas_rect = egui::Rect::from_min_max(canvas_min.to_pos2(), canvas_max.to_pos2());
    Some(canvas_rect)
}

fn active_presentation_profile(app: &GraphBrowserApp) -> PresentationProfile {
    phase3_resolve_active_presentation_profile(app.default_registry_theme_id()).profile
}

fn lifecycle_color(presentation: &PresentationProfile, lifecycle: NodeLifecycle) -> Color32 {
    match lifecycle {
        NodeLifecycle::Active => presentation.lifecycle_active.to_color32(),
        NodeLifecycle::Warm => presentation.lifecycle_warm.to_color32(),
        NodeLifecycle::Cold => presentation.lifecycle_cold.to_color32(),
        NodeLifecycle::Tombstone => presentation.lifecycle_tombstone.to_color32(),
    }
}

fn apply_search_node_visuals(
    app: &mut GraphBrowserApp,
    selection: &crate::app::SelectionState,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_query_active: bool,
) {
    let hovered = app.workspace.hovered_graph_node;
    let highlighted_edge = app.workspace.highlighted_graph_edge;
    let search_mode = app.workspace.search_display_mode;
    let adjacency_set = hovered_adjacency_set(app, hovered);
    let presentation = active_presentation_profile(app);
    let colors: Vec<(NodeKey, Color32)> = app
        .workspace
        .domain
        .graph
        .nodes()
        .map(|(key, node)| {
            let mut color = lifecycle_color(&presentation, node.lifecycle);
            if app.is_crash_blocked(key) {
                color = presentation.crash_blocked.to_color32();
            }

            let search_match = search_query_active && search_matches.contains(&key);
            let search_miss = search_query_active && !search_matches.contains(&key);
            if search_match {
                color = if active_search_match == Some(key) {
                    presentation.search_match_active.to_color32()
                } else {
                    presentation.search_match.to_color32()
                };
            } else if search_miss && matches!(search_mode, SearchDisplayMode::Highlight) {
                color = color.gamma_multiply(0.45);
            }

            if hovered.is_some() && !adjacency_set.contains(&key) {
                color = color.gamma_multiply(0.4);
            }
            if hovered == Some(key) {
                // Visual cue for command-target disambiguation while hovering.
                color = presentation.hover_target.to_color32();
            }
            if let Some((from, to)) = highlighted_edge
                && (key == from || key == to)
            {
                color = presentation.edge_highlight_foreground.to_color32();
            }
            if selection.primary() == Some(key) {
                color = presentation.selection_primary.to_color32();
            } else if selection.contains(&key) && hovered != Some(key) {
                color = if app.is_crash_blocked(key) {
                    presentation.crash_blocked.to_color32()
                } else {
                    lifecycle_color(&presentation, node.lifecycle)
                };
            }
            (key, color)
        })
        .collect();

    let Some(state) = app.workspace.egui_state.as_mut() else {
        return;
    };
    for (key, color) in colors {
        if let Some(node) = state.graph.node_mut(key) {
            node.set_color(color);
        }
    }

    let edge_dimming: Vec<_> = state
        .graph
        .edges_iter()
        .map(|(edge_key, _)| {
            let mut dim = false;
            if hovered.is_some()
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
            {
                dim = !adjacency_set.contains(&from) && !adjacency_set.contains(&to);
            }
            if search_query_active
                && matches!(search_mode, SearchDisplayMode::Highlight)
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
                && (!search_matches.contains(&from) || !search_matches.contains(&to))
            {
                dim = true;
            }
            (edge_key, dim)
        })
        .collect();
    for (edge_key, dim) in edge_dimming {
        if let Some(edge) = state.graph.edge_mut(edge_key) {
            edge.display_mut().set_dimmed(dim);
        }
    }
}

fn hovered_adjacency_set(app: &GraphBrowserApp, hovered: Option<NodeKey>) -> HashSet<NodeKey> {
    hovered
        .map(|hover_key| {
            app.domain_graph()
                .out_neighbors(hover_key)
                .chain(app.domain_graph().in_neighbors(hover_key))
                .chain(std::iter::once(hover_key))
                .collect()
        })
        .unwrap_or_default()
}

/// Apply Graphshell-owned camera/navigation to the egui_graphs metadata frame.
///
/// This is the canonical camera path for graph panes. It is not a fallback for
/// egui_graphs navigation; egui_graphs navigation is intentionally disabled so
/// Graphshell can own fit-to-screen, focus-on-selection, keyboard pan/zoom,
/// and policy-driven camera behavior without a competing camera authority.
fn handle_custom_navigation(
    ui: &Ui,
    response: &egui::Response,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    enabled: bool,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
    radial_open: bool,
    right_button_down: bool,
) -> Option<f32> {
    if !enabled {
        return None;
    }

    let position_fit_locked = app.camera_position_fit_locked();
    let zoom_fit_locked = app.camera_zoom_fit_locked();

    if should_auto_fit_locked_camera(app) {
        app.request_camera_command_for_view(Some(view_id), CameraCommand::Fit);
        app.clear_pending_wheel_zoom_delta();
    }

    // Apply pending durable camera command.
    let camera_zoom = apply_pending_camera_command(ui, app, metadata_id, view_id, canvas_profile);

    // Apply keyboard zoom for this pane when a pending request explicitly targets it.
    let keyboard_zoom = if zoom_fit_locked {
        None
    } else {
        apply_pending_keyboard_zoom_request(
            ui,
            app,
            metadata_id,
            view_id,
            canvas_profile.navigation.keyboard_zoom_step,
        )
    };

    // Apply pre-intercepted wheel zoom delta.
    let wheel_zoom = if zoom_fit_locked {
        None
    } else {
        apply_pending_wheel_zoom(
            ui,
            response,
            metadata_id,
            app,
            view_id,
            &canvas_profile.navigation,
        )
    };

    let pointer_inside = response.contains_pointer() || response.dragged();
    let (primary_down, shift_down) = ui.input(|i| (i.pointer.primary_down(), i.modifiers.shift));
    let lasso_primary_drag_active = matches!(
        app.lasso_binding_preference(),
        CanvasLassoBinding::ShiftLeftDrag
    ) && shift_down;

    let wants_keyboard_input = ui.ctx().wants_keyboard_input();
    let keyboard_pan_allowed = keyboard_pan_allowed_for_view(app, view_id);
    let keyboard_pan_delta =
        keyboard_pan_delta_from_input(ui, app.keyboard_pan_step(), app.keyboard_pan_input_mode());
    let keyboard_pan_blocked = emit_keyboard_pan_blocked_if_needed(
        keyboard_pan_delta,
        wants_keyboard_input,
        position_fit_locked,
        keyboard_pan_allowed,
    );
    let mut manual_pan_applied = false;
    if !keyboard_pan_blocked && keyboard_pan_delta != Vec2::ZERO {
        manual_pan_applied =
            apply_background_pan(ui.ctx(), metadata_id, app, view_id, keyboard_pan_delta);
    }

    // Pan with Left Mouse Button on background
    // Note: We check if we are NOT hovering a node to allow node dragging.
    // app.workspace.hovered_graph_node is updated before this function in render_graph_in_ui_collect_actions.
    if !position_fit_locked
        && canvas_profile.allows_background_pan(
            app.workspace.hovered_graph_node.is_none(),
            pointer_inside,
            primary_down,
            lasso_primary_drag_active,
            radial_open,
            right_button_down,
        )
    {
        let delta = ui.input(|i| i.pointer.delta());
        if apply_background_pan(ui.ctx(), metadata_id, app, view_id, delta) {
            manual_pan_applied = true;
        }
    }

    if position_fit_locked {
        clear_pan_inertia_velocity(ui.ctx(), metadata_id);
    } else if !manual_pan_applied {
        apply_background_pan_inertia(ui.ctx(), metadata_id, app, view_id);
    }

    // Clamp zoom bounds using per-view camera if available, else global camera.
    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);
    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let clamped = meta.zoom.clamp(zoom_min, zoom_max);
            if (meta.zoom - clamped).abs() > f32::EPSILON {
                meta.zoom = clamped;
            }
            let current_zoom = meta.zoom;
            data.insert_persisted(metadata_id, meta);
            // Keep per-view current_zoom in sync.
            if let Some(view) = app.workspace.views.get_mut(&view_id) {
                view.camera.current_zoom = current_zoom;
            }
        }
    });

    camera_zoom.or(keyboard_zoom).or(wheel_zoom)
}

fn should_auto_fit_locked_camera(app: &GraphBrowserApp) -> bool {
    app.camera_position_fit_locked()
        && !app.workspace.is_interacting
        && app.workspace.physics.base.is_running
}

fn keyboard_pan_allowed_for_view(app: &GraphBrowserApp, view_id: crate::app::GraphViewId) -> bool {
    if app.workspace.focused_view == Some(view_id) {
        return true;
    }

    app.workspace.focused_view.is_none()
        && app.workspace.views.len() == 1
        && app.workspace.views.contains_key(&view_id)
}

fn keyboard_pan_delta_from_input(ui: &Ui, step: f32, mode: KeyboardPanInputMode) -> Vec2 {
    let state = ui.input(|i| KeyboardPanInputState {
        wasd: KeyboardPanKeys {
            up: i.key_down(egui::Key::W),
            down: i.key_down(egui::Key::S),
            left: i.key_down(egui::Key::A),
            right: i.key_down(egui::Key::D),
        },
        arrows: KeyboardPanKeys {
            up: i.key_down(egui::Key::ArrowUp),
            down: i.key_down(egui::Key::ArrowDown),
            left: i.key_down(egui::Key::ArrowLeft),
            right: i.key_down(egui::Key::ArrowRight),
        },
    });
    keyboard_pan_delta_from_state(state, step, mode)
}

#[derive(Clone, Copy, Debug, Default)]
struct KeyboardPanKeys {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct KeyboardPanInputState {
    wasd: KeyboardPanKeys,
    arrows: KeyboardPanKeys,
}

fn keyboard_pan_delta_from_state(
    state: KeyboardPanInputState,
    step: f32,
    mode: KeyboardPanInputMode,
) -> Vec2 {
    let keys = match mode {
        KeyboardPanInputMode::WasdAndArrows => KeyboardPanKeys {
            up: state.wasd.up || state.arrows.up,
            down: state.wasd.down || state.arrows.down,
            left: state.wasd.left || state.arrows.left,
            right: state.wasd.right || state.arrows.right,
        },
        KeyboardPanInputMode::ArrowsOnly => state.arrows,
    };

    keyboard_pan_delta_from_keys(keys, step)
}

fn keyboard_pan_delta_from_keys(keys: KeyboardPanKeys, step: f32) -> Vec2 {
    let pan_step = step.max(1.0);
    let mut delta = Vec2::ZERO;

    if keys.left {
        delta.x += pan_step;
    }
    if keys.right {
        delta.x -= pan_step;
    }
    if keys.up {
        delta.y += pan_step;
    }
    if keys.down {
        delta.y -= pan_step;
    }

    delta
}

fn seeded_metadata_frame_for_view(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> MetadataFrame {
    let mut frame = MetadataFrame::default();
    if let Some(view_frame) = app.workspace.graph_view_frames.get(&view_id) {
        frame.zoom = view_frame.zoom.max(0.01);
        frame.pan = egui::vec2(view_frame.pan_x, view_frame.pan_y);
        return frame;
    }

    if let Some(view) = app.workspace.views.get(&view_id) {
        frame.zoom = view.camera.current_zoom.max(0.01);
    }

    frame
}

fn emit_keyboard_pan_blocked_if_needed(
    keyboard_pan_delta: Vec2,
    wants_keyboard_input: bool,
    camera_fit_locked: bool,
    keyboard_pan_allowed: bool,
) -> bool {
    if keyboard_pan_delta == Vec2::ZERO || wants_keyboard_input {
        return false;
    }

    let blocked_channel = if camera_fit_locked {
        Some(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK)
    } else if !keyboard_pan_allowed {
        Some(CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW)
    } else {
        None
    };

    if let Some(channel_id) = blocked_channel {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id,
            latency_us: 0,
        });
        return true;
    }

    false
}

fn apply_background_pan(
    ctx: &egui::Context,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    delta: Vec2,
) -> bool {
    if delta == Vec2::ZERO {
        return false;
    }

    set_focused_view_with_transition(app, Some(view_id));
    let seeded_frame = seeded_metadata_frame_for_view(app, view_id);
    let mut applied = false;
    ctx.data_mut(|data| {
        let mut meta = data
            .get_persisted::<MetadataFrame>(metadata_id)
            .unwrap_or(seeded_frame);
        meta.pan += delta;
        data.insert_persisted(metadata_id, meta);
        let velocity_id = pan_inertia_velocity_id(metadata_id);
        if app.camera_pan_inertia_enabled() {
            data.insert_persisted(velocity_id, delta);
        } else {
            data.remove::<Vec2>(velocity_id);
        }
        applied = true;
    });
    if applied && app.camera_pan_inertia_enabled() {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    applied
}

fn pan_inertia_velocity_id(metadata_id: egui::Id) -> egui::Id {
    metadata_id.with("pan_inertia_velocity")
}

fn clear_pan_inertia_velocity(ctx: &egui::Context, metadata_id: egui::Id) {
    let velocity_id = pan_inertia_velocity_id(metadata_id);
    ctx.data_mut(|data| data.remove::<Vec2>(velocity_id));
}

fn apply_background_pan_inertia(
    ctx: &egui::Context,
    metadata_id: egui::Id,
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> bool {
    if !app.camera_pan_inertia_enabled() {
        clear_pan_inertia_velocity(ctx, metadata_id);
        return false;
    }

    let velocity_id = pan_inertia_velocity_id(metadata_id);
    let mut velocity = ctx
        .data_mut(|data| data.get_persisted::<Vec2>(velocity_id))
        .unwrap_or(Vec2::ZERO);
    if velocity == Vec2::ZERO {
        return false;
    }

    let damping = app.camera_pan_inertia_damping();
    let min_velocity = 0.10_f32;
    let seeded_frame = seeded_metadata_frame_for_view(app, view_id);
    let mut applied = false;
    ctx.data_mut(|data| {
        let mut meta = data
            .get_persisted::<MetadataFrame>(metadata_id)
            .unwrap_or(seeded_frame);
        meta.pan += velocity;
        data.insert_persisted(metadata_id, meta);

        velocity *= damping;
        if velocity.length_sq() < min_velocity * min_velocity {
            velocity = Vec2::ZERO;
        }

        if velocity == Vec2::ZERO {
            data.remove::<Vec2>(velocity_id);
        } else {
            data.insert_persisted(velocity_id, velocity);
        }
        applied = true;
    });

    if velocity != Vec2::ZERO {
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    applied
}

fn apply_pending_keyboard_zoom_request(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
    view_id: crate::app::GraphViewId,
    keyboard_zoom_step: f32,
) -> Option<f32> {
    let Some(request) = app.take_pending_keyboard_zoom_request(view_id) else {
        return None;
    };

    let step = keyboard_zoom_step.max(1.01);
    let factor = match request {
        KeyboardZoomRequest::In => step,
        KeyboardZoomRequest::Out => 1.0 / step,
        KeyboardZoomRequest::Reset => 1.0,
    };

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    let graph_rect = ui.max_rect();
    let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size());
    let local_center = local_rect.center().to_vec2();
    let mut updated_zoom = None;
    let mut seeded_metadata = false;
    let seeded_frame = seeded_metadata_frame_for_view(app, view_id);

    ui.ctx().data_mut(|data| {
        let mut meta = if let Some(existing) = data.get_persisted::<MetadataFrame>(metadata_id) {
            existing
        } else {
            seeded_metadata = true;
            seeded_frame
        };
        let graph_center_pos = (local_center - meta.pan) / meta.zoom;
        let target = if matches!(request, KeyboardZoomRequest::Reset) {
            factor
        } else {
            meta.zoom * factor
        };
        let new_zoom = target.clamp(zoom_min, zoom_max);
        let pan_delta = graph_center_pos * meta.zoom - graph_center_pos * new_zoom;
        meta.pan += pan_delta;
        meta.zoom = new_zoom;
        data.insert_persisted(metadata_id, meta);
        updated_zoom = Some(new_zoom);
    });

    if seeded_metadata {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED_NO_METADATA,
            latency_us: 0,
        });
    }

    // Keep zoom in sync on the appropriate camera.
    if let Some(new_zoom) = updated_zoom {
        if let Some(view) = app.workspace.views.get_mut(&view_id) {
            view.camera.current_zoom = new_zoom;
        }
    }

    updated_zoom
}

fn apply_pending_camera_command(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> Option<f32> {
    let Some(command) = app.pending_camera_command() else {
        return None;
    };
    if let Some(target_view) = app.pending_camera_command_target_raw() {
        if !app.workspace.views.contains_key(&target_view) {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
                latency_us: 0,
            });
            app.clear_pending_camera_command();
            return None;
        }
        if target_view != view_id {
            return None;
        }
    }

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    match command {
        CameraCommand::SetZoom(target_zoom) => {
            let mut updated_zoom = None;
            let mut seeded_metadata = false;
            let seeded_frame = seeded_metadata_frame_for_view(app, view_id);
            ui.ctx().data_mut(|data| {
                let mut meta =
                    if let Some(existing) = data.get_persisted::<MetadataFrame>(metadata_id) {
                        existing
                    } else {
                        seeded_metadata = true;
                        seeded_frame
                    };
                let new_zoom = target_zoom.clamp(zoom_min, zoom_max);
                meta.zoom = new_zoom;
                data.insert_persisted(metadata_id, meta);
                updated_zoom = Some(new_zoom);
            });
            if seeded_metadata {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UI_GRAPH_CAMERA_ZOOM_DEFERRED_NO_METADATA,
                    latency_us: 0,
                });
            }
            if let Some(new_zoom) = updated_zoom {
                if let Some(view) = app.workspace.views.get_mut(&view_id) {
                    view.camera.current_zoom = new_zoom;
                }
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
        CameraCommand::Fit | CameraCommand::FitSelection => {
            let graph_rect = ui.max_rect();
            let view_size = graph_rect.size();
            if view_size.x <= f32::EPSILON || view_size.y <= f32::EPSILON {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_ZERO_VIEW,
                    latency_us: 0,
                });
                return None;
            }

            let bounds = if matches!(command, CameraCommand::FitSelection) {
                node_bounds_for_selection(app, app.selection_for_view(view_id))
            } else {
                node_bounds_for_all(app)
            };

            let Some((min_x, max_x, min_y, max_y)) = bounds else {
                if matches!(command, CameraCommand::FitSelection) {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_FIT_SELECTION_FALLBACK_TO_FIT,
                        latency_us: 0,
                    });
                    app.request_camera_command_for_view(Some(view_id), CameraCommand::Fit);
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_NO_BOUNDS,
                        latency_us: 0,
                    });
                    app.clear_pending_camera_command();
                }
                return None;
            };

            let width = (max_x - min_x).abs().max(1.0);
            let height = (max_y - min_y).abs().max(1.0);
            let padding = if matches!(command, CameraCommand::FitSelection) {
                canvas_profile.navigation.camera_focus_selection_padding
            } else {
                canvas_profile.navigation.camera_fit_padding
            };
            let padded_width = width * padding;
            let padded_height = height * padding;
            let fit_zoom = (view_size.x / padded_width).min(view_size.y / padded_height);
            let raw_target = if matches!(command, CameraCommand::FitSelection) {
                fit_zoom
            } else {
                fit_zoom * canvas_profile.navigation.camera_fit_relax
            };
            let target_zoom = raw_target.clamp(zoom_min, zoom_max);

            let center = egui::pos2((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            let viewport_center = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size())
                .center()
                .to_vec2();
            let target_pan = viewport_center - center.to_vec2() * target_zoom;

            let mut updated_zoom = None;
            let mut seeded_metadata = false;
            let seeded_frame = seeded_metadata_frame_for_view(app, view_id);
            ui.ctx().data_mut(|data| {
                let mut meta =
                    if let Some(existing) = data.get_persisted::<MetadataFrame>(metadata_id) {
                        existing
                    } else {
                        seeded_metadata = true;
                        seeded_frame
                    };
                meta.zoom = target_zoom;
                meta.pan = target_pan;
                data.insert_persisted(metadata_id, meta);
                updated_zoom = Some(target_zoom);
            });

            if seeded_metadata {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UI_GRAPH_CAMERA_FIT_DEFERRED_NO_METADATA,
                    latency_us: 0,
                });
            }

            if let Some(new_zoom) = updated_zoom {
                if let Some(view) = app.workspace.views.get_mut(&view_id) {
                    view.camera.current_zoom = new_zoom;
                }
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
    }
}

fn apply_pending_wheel_zoom(
    ui: &Ui,
    response: &egui::Response,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    navigation_policy: &crate::registries::domain::layout::canvas::CanvasNavigationPolicy,
) -> Option<f32> {
    let scroll_delta = app.pending_wheel_zoom_delta(view_id);
    if scroll_delta.abs() <= f32::EPSILON {
        return None;
    }

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    let velocity_id = metadata_id.with("scroll_zoom_velocity");
    let mut velocity = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<f32>(velocity_id))
        .unwrap_or(0.0);

    let impulse =
        navigation_policy.wheel_zoom_impulse_scale * (scroll_delta / 60.0).clamp(-1.0, 1.0);
    velocity += impulse;

    let mut updated_zoom = None;
    if velocity.abs() >= navigation_policy.wheel_zoom_inertia_min_abs {
        let factor = 1.0 + velocity;
        if factor > 0.0 {
            let graph_rect = response.rect;
            let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size());
            let pointer_pos = app
                .pending_wheel_zoom_anchor_screen(view_id)
                .map(|(x, y)| egui::pos2(x, y))
                .or_else(|| ui.input(|i| i.pointer.latest_pos()));
            let local_anchor = pointer_pos
                .map(|p| egui::pos2(p.x - graph_rect.min.x, p.y - graph_rect.min.y))
                .unwrap_or(local_rect.center())
                .to_vec2();
            let mut seeded_metadata = false;
            let seeded_frame = seeded_metadata_frame_for_view(app, view_id);

            ui.ctx().data_mut(|data| {
                let mut meta =
                    if let Some(existing) = data.get_persisted::<MetadataFrame>(metadata_id) {
                        existing
                    } else {
                        seeded_metadata = true;
                        seeded_frame
                    };
                let graph_anchor_pos = (local_anchor - meta.pan) / meta.zoom;
                let new_zoom = (meta.zoom * factor).clamp(zoom_min, zoom_max);
                let pan_delta = graph_anchor_pos * meta.zoom - graph_anchor_pos * new_zoom;
                meta.pan += pan_delta;
                meta.zoom = new_zoom;
                data.insert_persisted(metadata_id, meta);
                updated_zoom = Some(new_zoom);
            });

            if seeded_metadata {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UI_GRAPH_WHEEL_ZOOM_DEFERRED_NO_METADATA,
                    latency_us: 0,
                });
            }

            if let Some(new_zoom) = updated_zoom
                && let Some(view) = app.workspace.views.get_mut(&view_id)
            {
                view.camera.current_zoom = new_zoom;
            }
        } else {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_WHEEL_ZOOM_BLOCKED_INVALID_FACTOR,
                latency_us: 0,
            });
            app.clear_pending_wheel_zoom_delta();
        }
    }

    if updated_zoom.is_some() {
        app.clear_pending_wheel_zoom_delta();
    }

    velocity *= navigation_policy.wheel_zoom_inertia_damping;
    if velocity.abs() < navigation_policy.wheel_zoom_inertia_min_abs {
        velocity = 0.0;
    }
    ui.ctx()
        .data_mut(|d| d.insert_persisted(velocity_id, velocity));
    if velocity != 0.0 {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }

    updated_zoom
}

fn node_bounds_for_all(app: &GraphBrowserApp) -> Option<(f32, f32, f32, f32)> {
    let state = app.workspace.egui_state.as_ref()?;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut has_nodes = false;

    for (_, node) in state.graph.nodes_iter() {
        let pos = node.location();
        min_x = min_x.min(pos.x);
        max_x = max_x.max(pos.x);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y);
        has_nodes = true;
    }

    if !has_nodes
        || !min_x.is_finite()
        || !max_x.is_finite()
        || !min_y.is_finite()
        || !max_y.is_finite()
    {
        return None;
    }

    Some((min_x, max_x, min_y, max_y))
}

fn node_bounds_for_selection(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for key in selection.iter().copied() {
        if let Some(position) = app.domain_graph().node_projected_position(key) {
            min_x = min_x.min(position.x);
            max_x = max_x.max(position.x);
            min_y = min_y.min(position.y);
            max_y = max_y.max(position.y);
        }
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return None;
    }

    Some((min_x, max_x, min_y, max_y))
}

struct LassoGestureResult {
    action: Option<GraphAction>,
    suppress_context_menu: bool,
}

fn resolve_lasso_selection_mode(
    lasso_binding: CanvasLassoBinding,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> SelectionUpdateMode {
    let add_mode = ctrl || (matches!(lasso_binding, CanvasLassoBinding::RightDrag) && shift);
    if alt {
        SelectionUpdateMode::Toggle
    } else if add_mode {
        SelectionUpdateMode::Add
    } else {
        SelectionUpdateMode::Replace
    }
}

fn normalize_lasso_keys(mut keys: Vec<NodeKey>) -> Vec<NodeKey> {
    keys.sort_by_key(|key| key.index());
    keys.dedup_by_key(|key| key.index());
    keys
}

fn collect_lasso_action(
    ui: &Ui,
    app: &GraphBrowserApp,
    enabled: bool,
    metadata_id: egui::Id,
    lasso_binding: CanvasLassoBinding,
) -> LassoGestureResult {
    let presentation = active_presentation_profile(app);
    let (start_id, moved_id) = lasso_state_ids(metadata_id);
    let threshold_px = 6.0_f32;
    if !enabled {
        ui.ctx().data_mut(|d| {
            d.remove::<egui::Pos2>(start_id);
            d.remove::<bool>(moved_id);
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let graph_rect = ui.max_rect();
    let (pointer_pos, pressed, down, released, ctrl, shift, alt) = ui.input(|i| {
        let (pressed, down, released) = match lasso_binding {
            CanvasLassoBinding::RightDrag => (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_down(),
                i.pointer.secondary_released(),
            ),
            CanvasLassoBinding::ShiftLeftDrag => (
                i.pointer.primary_pressed() && i.modifiers.shift,
                i.pointer.primary_down() && i.modifiers.shift,
                i.pointer.primary_released(),
            ),
        };
        (
            i.pointer.latest_pos(),
            pressed,
            down,
            released,
            i.modifiers.ctrl,
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });

    if pressed
        && let Some(pos) = pointer_pos
        && graph_rect.contains(pos)
    {
        ui.ctx().data_mut(|d| {
            d.insert_persisted(start_id, pos);
            d.insert_persisted(moved_id, false);
        });
    }

    let start = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<egui::Pos2>(start_id));
    let mut moved = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(moved_id))
        .unwrap_or(false);
    if down && let (Some(a), Some(b)) = (start, pointer_pos) {
        if !moved && (b - a).length() >= threshold_px {
            moved = true;
            ui.ctx().data_mut(|d| d.insert_persisted(moved_id, true));
        }
        if moved {
            let rect = egui::Rect::from_two_pos(a, b);
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.5, presentation.lasso_stroke.to_color32()),
                egui::epaint::StrokeKind::Outside,
            );
            ui.painter()
                .rect_filled(rect, 0.0, presentation.lasso_fill.to_color32());
        }
    }

    if !released {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }
    ui.ctx().data_mut(|d| {
        d.remove::<egui::Pos2>(start_id);
        d.remove::<bool>(moved_id);
    });

    let (Some(a), Some(b)) = (start, pointer_pos) else {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    };
    if !moved {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let rect = egui::Rect::from_two_pos(a, b);
    let mode = resolve_lasso_selection_mode(lasso_binding, ctrl, shift, alt);
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
        .unwrap_or_default();
    let Some(state) = app.workspace.egui_state.as_ref() else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_GRAPH_LASSO_BLOCKED_NO_STATE,
            latency_us: 0,
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag),
        };
    };
    // Build an R*-tree index in canvas (world) space and query with the
    // lasso rect inverted from screen space.  This avoids a full O(n) scan
    // and keeps hit-testing in a stable coordinate system.
    let index = NodeSpatialIndex::build(
        state
            .graph
            .nodes_iter()
            .map(|(key, node)| (key, node.location(), node.display().radius())),
    );
    let canvas_min = meta.screen_to_canvas_pos(rect.min);
    let canvas_max = meta.screen_to_canvas_pos(rect.max);
    let canvas_rect = egui::Rect::from_min_max(canvas_min, canvas_max);
    let keys = normalize_lasso_keys(index.nodes_with_center_in_canvas_rect(canvas_rect));

    LassoGestureResult {
        action: Some(GraphAction::LassoSelect { keys, mode }),
        suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag) && moved,
    }
}

fn lasso_state_ids(metadata_id: egui::Id) -> (egui::Id, egui::Id) {
    (
        metadata_id.with("lasso_start_screen"),
        metadata_id.with("lasso_moved"),
    )
}

fn node_key_traversal_order(app: &GraphBrowserApp) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app.domain_graph().nodes().map(|(key, _)| key).collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn next_keyboard_traversal_node(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
    reverse: bool,
) -> Option<NodeKey> {
    let ordered_keys = node_key_traversal_order(app);
    if ordered_keys.is_empty() {
        return None;
    }

    let current = selection.primary();
    let len = ordered_keys.len();
    let next_index = current
        .and_then(|key| ordered_keys.iter().position(|candidate| *candidate == key))
        .map(|idx| {
            if reverse {
                (idx + len - 1) % len
            } else {
                (idx + 1) % len
            }
        })
        .unwrap_or_else(|| if reverse { len - 1 } else { 0 });

    ordered_keys.get(next_index).copied()
}

fn collect_graph_keyboard_traversal_action(
    ui: &Ui,
    response: &egui::Response,
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
    radial_open: bool,
    lasso_active: bool,
) -> Option<GraphAction> {
    if radial_open || lasso_active || !response.has_focus() {
        return None;
    }

    let cycle_backward = ui.input(|i| {
        i.clone()
            .consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
    });
    let cycle_forward = if cycle_backward {
        false
    } else {
        ui.input(|i| i.clone().consume_key(egui::Modifiers::NONE, egui::Key::Tab))
    };

    if !(cycle_backward || cycle_forward) {
        return None;
    }

    let reverse = cycle_backward;
    let key = next_keyboard_traversal_node(app, selection, reverse)?;
    Some(GraphAction::SelectNode {
        key,
        multi_select: false,
    })
}

fn graph_node_accessibility_name(node: &crate::graph::Node) -> String {
    if !node.title.trim().is_empty() {
        node.title.clone()
    } else if !node.url.trim().is_empty() {
        node.url.clone()
    } else {
        "Untitled node".to_string()
    }
}

fn graph_canvas_accessibility_label(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
) -> String {
    if let Some(primary) = selection.primary()
        && let Some(node) = app.domain_graph().get_node(primary)
    {
        return format!(
            "Graph canvas. Focused node: {}. Press Tab or Shift+Tab to move between nodes.",
            graph_node_accessibility_name(node)
        );
    }

    "Graph canvas. No node focused. Press Tab to focus the first node.".to_string()
}

/// Convert egui_graphs events to resolved GraphActions and apply them.
fn collect_graph_actions(
    app: &GraphBrowserApp,
    events: &Rc<RefCell<Vec<Event>>>,
    split_open_modifier: bool,
    multi_select_modifier: bool,
) -> Vec<GraphAction> {
    let mut actions = Vec::new();

    for event in events.borrow_mut().drain(..) {
        match event {
            Event::NodeDoubleClick(p) => {
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = state.get_key(idx) {
                        if split_open_modifier {
                            actions.push(GraphAction::FocusNodeSplit(key));
                        } else {
                            actions.push(GraphAction::FocusNode(key));
                        }
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDragStart(_) => {
                actions.push(GraphAction::DragStart);
            }
            Event::NodeDragEnd(p) => {
                // Resolve final position from egui_state
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        let pos = state
                            .graph
                            .node(idx)
                            .map(|n| Point2D::new(n.location().x, n.location().y))
                            .unwrap_or_default();
                        actions.push(GraphAction::DragEnd(key, pos));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeMove(p) => {
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::MoveNode(
                            key,
                            Point2D::new(p.new_pos[0], p.new_pos[1]),
                        ));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeSelect(p) => {
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = node_key_or_emit_ambiguous_hit(state.get_key(idx)) {
                        actions.push(GraphAction::SelectNode {
                            key,
                            multi_select: multi_select_modifier,
                        });
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDeselect(p) => {
                // When Ctrl is held, a NodeDeselect means the user Ctrl+Clicked an
                // already-selected node to toggle it out of the multi-selection.
                if multi_select_modifier {
                    if let Some(state) = app.workspace.egui_state.as_ref() {
                        let idx = NodeIndex::new(p.id);
                        if let Some(key) = node_key_or_emit_ambiguous_hit(state.get_key(idx)) {
                            actions.push(GraphAction::SelectNode {
                                key,
                                multi_select: true,
                            });
                        }
                    } else {
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                            latency_us: 0,
                        });
                    }
                }
                // Without modifier: selection clearing is handled by the next SelectNode action.
            }
            Event::Zoom(p) => {
                actions.push(GraphAction::Zoom(p.new_zoom));
            }
            _ => {}
        }
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
    let Some(state) = app.workspace.egui_state.as_ref() else {
        if app.workspace.is_interacting {
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
    let group_drag_delta: Option<(NodeKey, egui::Vec2)> =
        if app.workspace.is_interacting && focused_selection.len() > 1 {
            layout_positions.iter().find_map(|(key, egui_pos)| {
                if !focused_selection.contains(key) {
                    return None;
                }
                let app_pos = app.domain_graph().node_projected_position(*key)?;
                let delta = egui::Vec2::new(egui_pos.x - app_pos.x, egui_pos.y - app_pos.y);
                // Only consider it a drag if it actually moved (filter float noise).
                if delta.length() > 0.01 {
                    Some((*key, delta))
                } else {
                    None
                }
            })
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
            }
        }
        if let Some(state_mut) = app.workspace.egui_state.as_mut() {
            for (key, pos) in secondary_updates {
                if let Some(egui_node) = state_mut.graph.node_mut(key) {
                    egui_node.set_location(pos);
                }
            }
        }
    }

    if let Some(state_mut) = app.workspace.egui_state.as_mut() {
        for (key, pos) in pinned_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(pos.to_pos2());
            }
        }
    }
}

/// Draw graph information overlay
fn draw_graph_info(ui: &mut egui::Ui, app: &mut GraphBrowserApp, view_id: crate::app::GraphViewId) {
    let presentation = active_presentation_profile(app);
    let info_text = format!(
        "Nodes: {} | Edges: {} | Physics: {} | Zoom: {:.1}x",
        app.domain_graph().node_count(),
        app.domain_graph().edge_count(),
        if app.workspace.physics.base.is_running {
            "Running"
        } else {
            "Paused"
        },
        app.workspace.camera.current_zoom
    );

    ui.painter().text(
        ui.available_rect_before_wrap().left_top() + Vec2::new(10.0, 10.0),
        egui::Align2::LEFT_TOP,
        info_text,
        egui::FontId::monospace(12.0),
        presentation.info_text.to_color32(),
    );

    let mut top_left_overlay_y = 28.0;
    if !app.workspace.active_graph_search_query.is_empty() {
        let query = app.workspace.active_graph_search_query.clone();
        let filter_mode = matches!(app.workspace.search_display_mode, SearchDisplayMode::Filter);
        let match_count = app.workspace.active_graph_search_match_count;
        let current_entry = GraphSearchHistoryEntry {
            query: query.clone(),
            filter_mode,
            origin: app.workspace.active_graph_search_origin.clone(),
            neighborhood_anchor: app.workspace.active_graph_search_neighborhood_anchor,
            neighborhood_depth: app.workspace.active_graph_search_neighborhood_depth,
        };
        let pinned_entry = app.workspace.pinned_graph_search.clone();
        let is_pinned = pinned_entry.as_ref() == Some(&current_entry);
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new("graph_active_search_status"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, 26.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            render_graph_search_origin_badge(
                                ui,
                                &app.workspace.active_graph_search_origin,
                            );
                            let scope_label = graph_search_scope_label(
                                app.workspace.active_graph_search_neighborhood_anchor,
                                app.workspace.active_graph_search_neighborhood_depth,
                            );
                            ui.small(format!(
                                "Search: {query} | {match_count} matches{scope_label}"
                            ));
                            if !app.workspace.graph_search_history.is_empty()
                                && ui.small_button("Back").clicked()
                            {
                                if let Some(entry) = app.workspace.graph_search_history.pop() {
                                    request_graph_search_entry(app, entry, false, None);
                                }
                            }
                            if ui
                                .small_button(if is_pinned { "Unpin" } else { "Pin" })
                                .clicked()
                            {
                                if is_pinned {
                                    app.workspace.pinned_graph_search = None;
                                } else {
                                    app.workspace.pinned_graph_search = Some(current_entry.clone());
                                }
                                app.workspace.egui_state_dirty = true;
                            }
                            if ui.selectable_label(!filter_mode, "Highlight").clicked() {
                                app.request_graph_search_with_options(
                                    query.clone(),
                                    false,
                                    app.workspace.active_graph_search_origin.clone(),
                                    app.workspace.active_graph_search_neighborhood_anchor,
                                    app.workspace.active_graph_search_neighborhood_depth,
                                    true,
                                    None,
                                );
                            }
                            if ui.selectable_label(filter_mode, "Filter").clicked() {
                                app.request_graph_search_with_options(
                                    query.clone(),
                                    true,
                                    app.workspace.active_graph_search_origin.clone(),
                                    app.workspace.active_graph_search_neighborhood_anchor,
                                    app.workspace.active_graph_search_neighborhood_depth,
                                    true,
                                    None,
                                );
                            }
                            if app
                                .workspace
                                .active_graph_search_neighborhood_anchor
                                .is_some()
                            {
                                let depth = app.workspace.active_graph_search_neighborhood_depth;
                                if ui.selectable_label(depth == 1, "1-hop").clicked() {
                                    app.request_graph_search_with_options(
                                        query.clone(),
                                        filter_mode,
                                        app.workspace.active_graph_search_origin.clone(),
                                        app.workspace.active_graph_search_neighborhood_anchor,
                                        1,
                                        false,
                                        None,
                                    );
                                }
                                if ui.selectable_label(depth == 2, "2-hop").clicked() {
                                    app.request_graph_search_with_options(
                                        query.clone(),
                                        filter_mode,
                                        app.workspace.active_graph_search_origin.clone(),
                                        app.workspace.active_graph_search_neighborhood_anchor,
                                        2,
                                        false,
                                        None,
                                    );
                                }
                            }
                            if ui.small_button("Clear").clicked() {
                                app.request_graph_search(String::new(), false);
                            }
                        });
                        if let Some(entry) = pinned_entry.filter(|entry| entry != &current_entry) {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.small("Pinned:");
                                if ui
                                    .small_button(graph_search_history_label(&entry))
                                    .clicked()
                                {
                                    request_graph_search_entry(app, entry, false, None);
                                }
                            });
                        }
                        if !app.workspace.graph_search_history.is_empty() {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.small("Recent:");
                                let recent_entries = app
                                    .workspace
                                    .graph_search_history
                                    .iter()
                                    .rev()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                for entry in recent_entries {
                                    let label = graph_search_history_label(&entry);
                                    if ui.small_button(label).clicked() {
                                        request_graph_search_entry(app, entry, false, None);
                                    }
                                }
                            });
                        }
                    });
            });
        top_left_overlay_y = 58.0;
    } else if let Some(entry) = app.workspace.pinned_graph_search.clone() {
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new("graph_pinned_search_status"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, 26.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("PIN").small().strong());
                            render_graph_search_origin_badge(ui, &entry.origin);
                            ui.small(graph_search_history_label(&entry));
                            if ui.small_button("Restore").clicked() {
                                request_graph_search_entry(app, entry.clone(), false, None);
                            }
                            if ui.small_button("X").clicked() {
                                app.workspace.pinned_graph_search = None;
                                app.workspace.egui_state_dirty = true;
                            }
                        });
                    });
            });
        top_left_overlay_y = 54.0;
    }

    if let Some((label, tooltip)) = graph_view_semantic_depth_status_badge(app, view_id) {
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new(("graph_semantic_depth_status", view_id)))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, top_left_overlay_y))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(label).small().strong())
                            .on_hover_text(tooltip);
                    });
            });
        top_left_overlay_y += 28.0;
    }

    if let Some(selected_key) = app.get_single_selected_node() {
        let suggestions = app.suggested_semantic_tags_for_node(selected_key);
        if !suggestions.is_empty() {
            ui.painter().text(
                ui.available_rect_before_wrap().left_top() + Vec2::new(10.0, top_left_overlay_y),
                egui::Align2::LEFT_TOP,
                format!("Suggested tags: {}", suggestions.join(", ")),
                egui::FontId::proportional(11.0),
                presentation.controls_text.to_color32(),
            );
        }

        if let Some(summary) = selected_node_enrichment_summary(app, selected_key) {
            let area_rect = ui.available_rect_before_wrap();
            egui::Area::new(egui::Id::new("graph_selected_node_enrichment"))
                .order(egui::Order::Foreground)
                .fixed_pos(area_rect.right_top() + Vec2::new(-288.0, 12.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::window(ui.style())
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(276.0);
                            ui.horizontal(|ui| {
                                ui.heading("Selected Node");
                                if ui.small_button("Edit Tags").clicked() {
                                    app.workspace.tag_panel_state = Some(TagPanelState {
                                        node_key: selected_key,
                                        text_input: String::new(),
                                        icon_picker_open: false,
                                        pending_icon_override: None,
                                    });
                                }
                            });
                            ui.small(&summary.title);
                            if summary.show_url {
                                ui.small(&summary.url);
                            }
                            ui.separator();
                            ui.small(format!("Lifecycle: {}", summary.lifecycle));
                            if let Some(anchor) = summary.placement_anchor.as_ref() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.small(format!("Semantic anchor: {}", anchor.label));
                                    if ui.small_button("Jump").clicked() {
                                        apply_ui_intents_with_checkpoint(
                                            app,
                                            vec![
                                                GraphIntent::SelectNode {
                                                    key: anchor.key,
                                                    multi_select: false,
                                                },
                                                ViewAction::RequestZoomToSelected.into(),
                                            ],
                                        );
                                    }
                                    if let Some(tag) = anchor.slice_tag.as_ref()
                                        && ui.small_button("Anchor Slice").clicked()
                                    {
                                        app.request_graph_search_with_options(
                                            tag.clone(),
                                            true,
                                            GraphSearchOrigin::AnchorSlice,
                                            None,
                                            1,
                                            true,
                                            Some(
                                                crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                                    GraphSearchOrigin::AnchorSlice,
                                                    tag,
                                                    0,
                                                ),
                                            ),
                                        );
                                    }
                                    if let Some(tag) = anchor.slice_tag.as_ref()
                                        && ui.small_button("Slice + Neighborhood").clicked()
                                    {
                                        app.request_graph_search_with_options(
                                            tag.clone(),
                                            true,
                                            GraphSearchOrigin::AnchorSlice,
                                            Some(anchor.key),
                                            1,
                                            true,
                                            Some(
                                                crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                                    GraphSearchOrigin::AnchorSlice,
                                                    tag,
                                                    1,
                                                ),
                                            ),
                                        );
                                    }
                                });
                            }
                            if summary.semantic_lens_available {
                                ui.horizontal_wrapped(|ui| {
                                    ui.small("Semantic view");
                                    if ui
                                        .small_button(if summary.semantic_lens_active {
                                            "Restore View"
                                        } else {
                                            "UDC Depth View"
                                        })
                                        .clicked()
                                        && let Some(target_view_id) = app.workspace.focused_view
                                    {
                                        apply_ui_intents_with_checkpoint(
                                            app,
                                            vec![GraphIntent::ToggleSemanticDepthView {
                                                view_id: target_view_id,
                                            }],
                                        );
                                    }
                                });
                                ui.small(if summary.semantic_lens_active {
                                    "Restore the previous Graph View dimension and leave semantic depth mode."
                                } else {
                                    "Visible payoff: the current Graph View lifts nodes into UDC depth layers using semantic class ancestry."
                                });
                            }
                            if !summary.workspace_memberships.is_empty() {
                                ui.small(format!(
                                    "Frames: {}",
                                    summary.workspace_memberships.join(", ")
                                ));
                            }
                            ui.separator();
                            ui.small("Semantic tags");
                            if summary.display_tags.is_empty() {
                                ui.small("No semantic tags on this node yet.");
                            } else {
                                render_semantic_tag_status_buttons(ui, app, &summary.display_tags);
                                if summary.hidden_tag_count > 0 {
                                    ui.small(format!("+{} more", summary.hidden_tag_count));
                                }
                            }
                            if !summary.suggested_tags.is_empty() {
                                ui.separator();
                                ui.small("Suggestions");
                                render_semantic_suggestion_buttons(ui, app, &summary.suggested_tags);
                            }
                            ui.separator();
                            ui.small("Click a tag to filter the graph by that semantic slice. Status shows whether the tag is canonical knowledge state or a looser node-local tag; suggestion text explains why the tag suggester surfaced it.");
                        });
                });
        }

        render_selected_node_tag_panel(ui.ctx(), app, selected_key);
    } else if app.workspace.tag_panel_state.is_some() {
        app.workspace.tag_panel_state = None;
    }

    // Draw controls hint
    let lasso_hint = canvas_lasso_binding_label(app.lasso_binding_preference());
    let command_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            action_id::graph::COMMAND_PALETTE_OPEN,
        )
        .join(" / ");
    let radial_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            action_id::graph::RADIAL_MENU_OPEN,
        )
        .join(" / ");
    let help_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            action_id::workbench::HELP_OPEN,
        )
        .join(" / ");
    let controls_text = format!(
        "Shortcuts: Ctrl+Click Multi-select | {lasso_hint} | Double-click Open | Drag tab out to split | N New Node | Del Remove | T Physics | R Reheat | +/-/0 Zoom | C Position-Lock | Z Zoom-Lock | WASD/Arrows Pan | F9 Camera Controls | L Toggle Pin | Ctrl+F Search | G Edge Ops | {command_hint} Commands | {radial_hint} Radial | Ctrl+Z/Y Undo/Redo | {help_hint} Help"
    );
    ui.painter().text(
        ui.available_rect_before_wrap().left_bottom() + Vec2::new(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        controls_text,
        egui::FontId::proportional(10.0),
        presentation.controls_text.to_color32(),
    );
}

fn semantic_badges_by_key(
    app: &GraphBrowserApp,
    graph: &crate::graph::Graph,
) -> HashMap<NodeKey, Vec<BadgeVisual>> {
    graph
        .nodes()
        .filter_map(|(key, node)| {
            let badges = badges_for_node(
                node,
                app.membership_for_node(node.id).len(),
                app.crash_blocked_node_keys().any(|crashed| crashed == key),
            )
            .into_iter()
            .filter(|badge| !matches!(badge, Badge::WorkspaceCount(_)))
            .collect::<Vec<_>>();
            let badge_visuals = badge_visuals(&badges);
            (!badge_visuals.is_empty()).then_some((key, badge_visuals))
        })
        .collect()
}

fn semantic_tag_display_label(tag: &str) -> String {
    if let Some(code) = tag.strip_prefix("udc:") {
        return match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(code)
        {
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                canonical_code,
                display_label,
            } => format!("{display_label} (udc:{canonical_code})"),
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. }
            | crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {
                format!("udc:{code}")
            }
        };
    }

    tag.to_string()
}

struct SelectedNodeEnrichmentSummary {
    title: String,
    url: String,
    show_url: bool,
    lifecycle: &'static str,
    workspace_memberships: Vec<String>,
    display_tags: Vec<SemanticTagStatusChip>,
    hidden_tag_count: usize,
    suggested_tags: Vec<SemanticSuggestionChip>,
    placement_anchor: Option<PlacementAnchorSummary>,
    semantic_lens_available: bool,
    semantic_lens_active: bool,
}

struct PlacementAnchorSummary {
    key: NodeKey,
    label: String,
    slice_tag: Option<String>,
}

struct SemanticTagChip {
    query: String,
    label: String,
}

struct SemanticTagStatusChip {
    chip: SemanticTagChip,
    status: String,
}

struct SemanticSuggestionChip {
    chip: SemanticTagChip,
    reason: String,
}

fn icon_picker_presets() -> Vec<crate::graph::badge::BadgeIcon> {
    use crate::graph::badge::BadgeIcon;
    vec![
        BadgeIcon::None,
        BadgeIcon::Emoji("🔖".to_string()),
        BadgeIcon::Emoji("📚".to_string()),
        BadgeIcon::Emoji("🔬".to_string()),
        BadgeIcon::Emoji("🧠".to_string()),
        BadgeIcon::Emoji("📝".to_string()),
        BadgeIcon::Emoji("🌟".to_string()),
        BadgeIcon::Emoji("📎".to_string()),
    ]
}

fn badge_icon_label(icon: &crate::graph::badge::BadgeIcon) -> String {
    match icon {
        crate::graph::badge::BadgeIcon::Emoji(value) => value.clone(),
        crate::graph::badge::BadgeIcon::Lucide(value) => value.clone(),
        crate::graph::badge::BadgeIcon::None => "None".to_string(),
    }
}

fn normalize_tag_entry_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if trimmed.starts_with('#') {
        trimmed.to_ascii_lowercase()
    } else {
        trimmed.to_string()
    })
}

fn is_reserved_system_tag(tag: &str) -> bool {
    matches!(
        tag,
        GraphBrowserApp::TAG_PIN
            | GraphBrowserApp::TAG_STARRED
            | GraphBrowserApp::TAG_ARCHIVE
            | GraphBrowserApp::TAG_RESIDENT
            | GraphBrowserApp::TAG_PRIVATE
            | GraphBrowserApp::TAG_NOHISTORY
            | GraphBrowserApp::TAG_MONITOR
            | GraphBrowserApp::TAG_UNREAD
            | GraphBrowserApp::TAG_FOCUS
            | GraphBrowserApp::TAG_CLIP
    )
}

fn reserved_tag_warning(query: &str) -> Option<String> {
    let normalized = normalize_tag_entry_input(query)?;
    if normalized.starts_with('#') && !is_reserved_system_tag(&normalized) {
        return Some("Unknown #tag will be accepted as user-defined.".to_string());
    }
    None
}

fn default_tag_suggestion_candidates() -> Vec<String> {
    [
        GraphBrowserApp::TAG_PIN,
        GraphBrowserApp::TAG_STARRED,
        GraphBrowserApp::TAG_UNREAD,
        GraphBrowserApp::TAG_FOCUS,
        GraphBrowserApp::TAG_MONITOR,
        GraphBrowserApp::TAG_PRIVATE,
        GraphBrowserApp::TAG_ARCHIVE,
        GraphBrowserApp::TAG_RESIDENT,
        GraphBrowserApp::TAG_NOHISTORY,
        GraphBrowserApp::TAG_CLIP,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn ranked_tag_suggestions(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
    query: &str,
) -> Vec<SemanticTagChip> {
    let current_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key)
            .into_iter()
            .collect::<HashSet<_>>();
    let mut candidates = default_tag_suggestion_candidates();
    candidates.extend(
        app.domain_graph()
            .nodes()
            .flat_map(|(_, node)| node.tags.iter().cloned()),
    );
    candidates.extend(app.suggested_semantic_tags_for_node(selected_key));
    candidates.sort();
    candidates.dedup();

    let mut ranked = Vec::new();
    let mut seen = HashSet::new();
    let mut push_candidate = |tag: String, ranked: &mut Vec<SemanticTagChip>| {
        if current_tags.contains(&tag) || !seen.insert(tag.clone()) {
            return;
        }
        ranked.push(semantic_tag_chip(tag));
    };

    if let Some(normalized_query) = normalize_tag_entry_input(query) {
        push_candidate(normalized_query.clone(), &mut ranked);

        for matched in crate::services::search::fuzzy_match_items(candidates, &normalized_query) {
            push_candidate(matched, &mut ranked);
            if ranked.len() >= 5 {
                return ranked;
            }
        }

        let knowledge_registry =
            crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default();
        for entry in knowledge_registry.search(&normalized_query) {
            push_candidate(format!("udc:{}", entry.code), &mut ranked);
            if ranked.len() >= 5 {
                return ranked;
            }
        }
    } else {
        for candidate in candidates {
            push_candidate(candidate, &mut ranked);
            if ranked.len() >= 5 {
                break;
            }
        }
    }

    ranked
}

fn render_selected_node_tag_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    selected_key: NodeKey,
) {
    let Some((panel_node_key, panel_text_input)) = app
        .workspace
        .tag_panel_state
        .as_ref()
        .map(|state| (state.node_key, state.text_input.clone()))
    else {
        return;
    };
    if panel_node_key != selected_key {
        app.workspace.tag_panel_state = None;
        return;
    }
    let Some(node) = app.domain_graph().get_node(selected_key) else {
        app.workspace.tag_panel_state = None;
        return;
    };

    let title = if node.title.is_empty() {
        node.url.clone()
    } else {
        node.title.clone()
    };
    let current_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key);
    let mut text_input = panel_text_input;
    let mut open = true;
    let mut close_requested = false;
    let mut pending_intents = Vec::new();
    let mut pending_icon_write: Option<(String, Option<crate::graph::badge::BadgeIcon>)> = None;
    let warning = reserved_tag_warning(&text_input);
    let suggestions = ranked_tag_suggestions(app, selected_key, &text_input);

    Window::new(format!("Tags for {}", title))
        .id(egui::Id::new((
            "graph_node_tag_panel",
            selected_key.index(),
        )))
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.small("Current tags");
            if current_tags.is_empty() {
                ui.small("No tags yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for tag in &current_tags {
                        let label = semantic_tag_display_label(tag);
                        if ui.small_button(format!("{label} ×")).clicked() {
                            pending_intents.push(GraphIntent::UntagNode {
                                key: selected_key,
                                tag: tag.clone(),
                            });
                        }
                    }
                });
            }

            ui.separator();
            ui.small("Add tag");
            let response = ui.add(
                egui::TextEdit::singleline(&mut text_input).hint_text("Add tag or semantic code…"),
            );
            let submit =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let picker_label = app
                    .workspace
                    .tag_panel_state
                    .as_ref()
                    .and_then(|state| state.pending_icon_override.as_ref())
                    .map(badge_icon_label)
                    .unwrap_or_else(|| "⊞".to_string());
                if ui.small_button(picker_label).clicked()
                    && let Some(panel_state) = app.workspace.tag_panel_state.as_mut()
                {
                    panel_state.icon_picker_open = !panel_state.icon_picker_open;
                }
                if ui.small_button("Add").clicked() || submit {
                    if let Some(tag) = normalize_tag_entry_input(&text_input) {
                        pending_intents.push(GraphIntent::TagNode {
                            key: selected_key,
                            tag,
                        });
                        text_input.clear();
                    }
                }
                if ui.small_button("Close").clicked() {
                    close_requested = true;
                }
            });
            if app
                .workspace
                .tag_panel_state
                .as_ref()
                .is_some_and(|state| state.icon_picker_open)
            {
                ui.small("Icon picker");
                ui.horizontal_wrapped(|ui| {
                    for icon in icon_picker_presets() {
                        let label = badge_icon_label(&icon);
                        if ui.small_button(label).clicked()
                            && let Some(panel_state) = app.workspace.tag_panel_state.as_mut()
                        {
                            panel_state.pending_icon_override =
                                (!matches!(icon, crate::graph::badge::BadgeIcon::None))
                                    .then_some(icon.clone());
                            panel_state.icon_picker_open = false;
                        }
                    }
                });
            }
            if let Some(warning) = warning.as_ref() {
                ui.small(warning);
            }

            ui.separator();
            ui.small("Suggestions");
            if suggestions.is_empty() {
                ui.small("No suggestions yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for chip in &suggestions {
                        let button = egui::Button::new(egui::RichText::new(&chip.label).small());
                        if ui.add(button).clicked() {
                            pending_intents.push(GraphIntent::TagNode {
                                key: selected_key,
                                tag: chip.query.clone(),
                            });
                            text_input.clear();
                        }
                    }
                });
            }
        });

    if close_requested || !open {
        app.workspace.tag_panel_state = None;
    } else if let Some(panel_state) = app.workspace.tag_panel_state.as_mut() {
        panel_state.text_input = text_input;
    }

    if !pending_intents.is_empty() {
        let tag_for_icon_write = pending_intents.iter().find_map(|intent| match intent {
            GraphIntent::TagNode { tag, .. } => Some(tag.clone()),
            _ => None,
        });
        apply_reducer_graph_intents_hardened(app, pending_intents);
        if let Some(tag) = tag_for_icon_write
            && !is_reserved_system_tag(&tag)
            && !tag.starts_with("udc:")
        {
            let icon = app
                .workspace
                .tag_panel_state
                .as_ref()
                .and_then(|state| state.pending_icon_override.clone());
            let _ = app.set_node_tag_icon_override(selected_key, &tag, icon.clone());
            pending_icon_write = Some((tag, icon));
        }
    }

    if let Some((_tag, _icon)) = pending_icon_write
        && let Some(panel_state) = app.workspace.tag_panel_state.as_mut()
    {
        panel_state.pending_icon_override = None;
    }
}

fn semantic_tag_chip(tag: String) -> SemanticTagChip {
    SemanticTagChip {
        label: semantic_tag_display_label(&tag),
        query: tag,
    }
}

fn semantic_tag_status_chip(tag: String) -> SemanticTagStatusChip {
    let status = semantic_tag_status_label(&tag);
    SemanticTagStatusChip {
        chip: semantic_tag_chip(tag),
        status,
    }
}

fn semantic_tag_status_label(tag: &str) -> String {
    if let Some(code) = tag.strip_prefix("udc:") {
        return match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(code)
        {
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                canonical_code,
                ..
            } => format!("KnowledgeRegistry canonical: udc:{canonical_code}"),
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. } => {
                "KnowledgeRegistry descendant/unknown code".to_string()
            }
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {
                "Unrecognized semantic code".to_string()
            }
        };
    }

    if tag.starts_with('#') {
        "User tag".to_string()
    } else {
        "Node tag".to_string()
    }
}

fn semantic_suggestion_chip(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
    tag: String,
) -> SemanticSuggestionChip {
    let reason = explain_semantic_suggestion(app, selected_key, &tag);
    SemanticSuggestionChip {
        chip: semantic_tag_chip(tag),
        reason,
    }
}

fn explain_semantic_suggestion(app: &GraphBrowserApp, selected_key: NodeKey, tag: &str) -> String {
    let Some(node) = app.domain_graph().get_node(selected_key) else {
        return "Suggested by the tag suggester agent.".to_string();
    };

    let code = tag.strip_prefix("udc:").unwrap_or(tag);
    let registry =
        crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default();
    let title = node.title.trim();
    if !title.is_empty()
        && registry
            .search(title)
            .into_iter()
            .take(3)
            .any(|entry| entry.code == code)
    {
        return format!("Suggested by the tag suggester agent from the node title '{title}'.");
    }

    let url_text = node.url.replace([':', '/', '.', '-', '_'], " ");
    if registry
        .search(&url_text)
        .into_iter()
        .take(3)
        .any(|entry| entry.code == code)
    {
        return "Suggested by the tag suggester agent from URL/domain tokens.".to_string();
    }

    if let Some(anchor_key) =
        crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
            app,
            selected_key,
        )
    {
        let anchor_tags =
            crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &anchor_key);
        if anchor_tags.iter().any(|anchor_tag| {
            anchor_tag == tag
                || (anchor_tag.starts_with("udc:")
                    && tag.starts_with(anchor_tag)
                    && tag.len() > anchor_tag.len())
        }) {
            return "Suggested because the node sits near a matching semantic anchor slice."
                .to_string();
        }
    }

    "Suggested by the tag suggester agent from title/URL semantic matches.".to_string()
}

fn render_semantic_tag_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticTagChip],
) {
    ui.horizontal_wrapped(|ui| {
        for chip in chips {
            let button = egui::Button::new(egui::RichText::new(&chip.label).small());
            if ui.add(button).clicked() {
                app.request_graph_search_with_options(
                    chip.query.clone(),
                    true,
                    GraphSearchOrigin::SemanticTag,
                    None,
                    1,
                    true,
                    Some(
                        crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                            GraphSearchOrigin::SemanticTag,
                            &chip.query,
                            0,
                        ),
                    ),
                );
            }
        }
    });
}

fn render_semantic_tag_status_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticTagStatusChip],
) {
    ui.horizontal_wrapped(|ui| {
        for entry in chips {
            ui.vertical(|ui| {
                let button = egui::Button::new(egui::RichText::new(&entry.chip.label).small());
                if ui.add(button).clicked() {
                    app.request_graph_search_with_options(
                        entry.chip.query.clone(),
                        true,
                        GraphSearchOrigin::SemanticTag,
                        None,
                        1,
                        true,
                        Some(
                            crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                GraphSearchOrigin::SemanticTag,
                                &entry.chip.query,
                                0,
                            ),
                        ),
                    );
                }
                ui.small(&entry.status);
            });
        }
    });
}

fn render_semantic_suggestion_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticSuggestionChip],
) {
    ui.horizontal_wrapped(|ui| {
        for entry in chips {
            ui.vertical(|ui| {
                let button = egui::Button::new(egui::RichText::new(&entry.chip.label).small());
                if ui.add(button).clicked() {
                    app.request_graph_search_with_options(
                        entry.chip.query.clone(),
                        true,
                        GraphSearchOrigin::SemanticTag,
                        None,
                        1,
                        true,
                        Some(
                            crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                GraphSearchOrigin::SemanticTag,
                                &entry.chip.query,
                                0,
                            ),
                        ),
                    );
                }
                ui.small(&entry.reason);
            });
        }
    });
}

fn graph_search_history_label(entry: &GraphSearchHistoryEntry) -> String {
    let origin = match entry.origin {
        GraphSearchOrigin::Manual => "manual",
        GraphSearchOrigin::SemanticTag => "tag",
        GraphSearchOrigin::AnchorSlice => "anchor",
    };
    let scope_suffix = if entry.neighborhood_anchor.is_some() && entry.neighborhood_depth > 1 {
        " + 2-hop"
    } else if entry.neighborhood_anchor.is_some() {
        " + nbr"
    } else {
        ""
    };
    format!("{origin}{scope_suffix}: {}", entry.query)
}

fn request_graph_search_entry(
    app: &mut GraphBrowserApp,
    entry: GraphSearchHistoryEntry,
    record_history: bool,
    toast_message: Option<String>,
) {
    app.request_graph_search_with_options(
        entry.query,
        entry.filter_mode,
        entry.origin,
        entry.neighborhood_anchor,
        entry.neighborhood_depth,
        record_history,
        toast_message,
    );
}

fn graph_search_scope_label(
    neighborhood_anchor: Option<NodeKey>,
    neighborhood_depth: u8,
) -> String {
    if neighborhood_anchor.is_none() {
        return String::new();
    }
    if neighborhood_depth > 1 {
        return format!(" | {}-hop neighborhood", neighborhood_depth);
    }
    " | 1-hop neighborhood".to_string()
}

fn render_graph_search_origin_badge(ui: &mut egui::Ui, origin: &GraphSearchOrigin) {
    let (label, color) = match origin {
        GraphSearchOrigin::Manual => ("manual", egui::Color32::from_rgb(120, 170, 255)),
        GraphSearchOrigin::SemanticTag => ("semantic", egui::Color32::from_rgb(76, 175, 80)),
        GraphSearchOrigin::AnchorSlice => ("anchor", egui::Color32::from_rgb(255, 167, 38)),
    };
    ui.label(egui::RichText::new("●").small().color(color));
    ui.small(label);
}

fn graph_view_semantic_depth_status_badge(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<(&'static str, &'static str)> {
    app.workspace.views.get(&view_id).and_then(|view| {
        matches!(
            view.dimension,
            ViewDimension::ThreeD {
                mode: ThreeDMode::TwoPointFive,
                z_source: ZSource::UdcLevel { .. }
            }
        )
        .then_some((
            "UDC Depth",
            "Semantic depth view is active for this Graph View. Nodes are lifted into UDC depth layers until you restore the previous view.",
        ))
    })
}

fn selected_node_enrichment_summary(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
) -> Option<SelectedNodeEnrichmentSummary> {
    let node = app.domain_graph().get_node(selected_key)?;
    let mut display_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key)
            .into_iter()
            .map(semantic_tag_status_chip)
            .collect::<Vec<_>>();
    let hidden_tag_count = display_tags.len().saturating_sub(4);
    display_tags.truncate(4);

    let suggested_tags = app
        .suggested_semantic_tags_for_node(selected_key)
        .into_iter()
        .map(|tag| semantic_suggestion_chip(app, selected_key, tag))
        .collect::<Vec<_>>();

    let placement_anchor =
        crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
            app,
            selected_key,
        )
        .and_then(|anchor_key| {
            app.domain_graph().get_node(anchor_key).map(|anchor| {
                let label = if anchor.title.is_empty() {
                    anchor.url.clone()
                } else {
                    anchor.title.clone()
                };
                let slice_tag =
                    crate::shell::desktop::runtime::registries::knowledge::tags_for_node(
                        app,
                        &anchor_key,
                    )
                    .into_iter()
                    .next();
                PlacementAnchorSummary {
                    key: anchor_key,
                    label,
                    slice_tag,
                }
            })
        });

    Some(SelectedNodeEnrichmentSummary {
        title: if node.title.is_empty() {
            node.url.clone()
        } else {
            node.title.clone()
        },
        url: node.url.clone(),
        show_url: !node.title.is_empty() && node.title != node.url,
        lifecycle: match node.lifecycle {
            NodeLifecycle::Active => "Active",
            NodeLifecycle::Warm => "Warm",
            NodeLifecycle::Cold => "Cold",
            NodeLifecycle::Tombstone => "Ghost Node",
        },
        workspace_memberships: app.membership_for_node(node.id).iter().cloned().collect(),
        display_tags,
        hidden_tag_count,
        suggested_tags,
        placement_anchor,
        semantic_lens_available: app.workspace.semantic_index.contains_key(&selected_key),
        semantic_lens_active: app
            .workspace
            .focused_view
            .and_then(|view_id| app.workspace.views.get(&view_id))
            .is_some_and(|view| {
                matches!(
                    view.dimension,
                    ViewDimension::ThreeD {
                        mode: ThreeDMode::TwoPointFive,
                        z_source: ZSource::UdcLevel { .. }
                    }
                )
            }),
    })
}

fn requested_layout_algorithm_id(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> String {
    app.workspace
        .views
        .get(&view_id)
        .map(|view| match view.lens.layout {
            crate::registries::atomic::lens::LayoutMode::Free => {
                view.lens.layout_algorithm_id.clone()
            }
            _ => layout_algorithm_id_for_mode(&view.lens.layout).to_string(),
        })
        .unwrap_or_else(|| canvas_profile.layout_algorithm.algorithm_id.clone())
}

fn should_apply_layout_algorithm(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    resolved_layout_id: &str,
) -> bool {
    let layout_changed = app
        .workspace
        .views
        .get(&view_id)
        .and_then(|view| view.last_layout_algorithm_id.as_deref())
        != Some(resolved_layout_id);

    app.workspace.egui_state.is_none() || app.workspace.egui_state_dirty || layout_changed
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
                        ui.small(msg);
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
                        ui.small(header);
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
                            app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                                url: VersoAddress::settings(GraphshellSettingsPath::Persistence)
                                    .to_string(),
                            });
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
                app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                    url: VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
                });
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
    use crate::app::SearchDisplayMode;
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
        view.lens.lens_id = Some("lens:default".to_string());
        view.lens.layout = LayoutMode::Grid { gap: 24.0 };
        app.workspace.views.insert(view_id, view);

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
        view.lens.layout = LayoutMode::Free;
        view.lens.layout_algorithm_id =
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT.to_string();
        app.workspace.views.insert(view_id, view);

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
        app.workspace.views.insert(view_id, view);
        app.workspace.egui_state = Some(EguiGraphState::from_graph(
            app.domain_graph(),
            &HashSet::new(),
        ));
        app.workspace.egui_state_dirty = false;

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
        assert!(!app.workspace.is_interacting);

        let intents = intents_from_graph_actions(vec![GraphAction::DragStart]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.is_interacting);
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

        assert!(!app.workspace.is_interacting);
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.projected_position(), Point2D::new(150.0, 250.0));
    }

    #[test]
    fn locked_camera_autofit_requires_physics_running_and_not_dragging() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.views.insert(
            view_id,
            crate::app::GraphViewState::new("AutoFit Lock Test"),
        );
        app.workspace.focused_view = Some(view_id);
        app.set_camera_fit_locked(true);

        app.workspace.physics.base.is_running = false;
        app.workspace.is_interacting = false;
        assert!(
            !should_auto_fit_locked_camera(&app),
            "fit-lock should not auto-fit when physics is idle"
        );

        app.workspace.physics.base.is_running = true;
        app.workspace.is_interacting = true;
        assert!(
            !should_auto_fit_locked_camera(&app),
            "fit-lock should not auto-fit during active drag interaction"
        );

        app.workspace.physics.base.is_running = true;
        app.workspace.is_interacting = false;
        assert!(
            should_auto_fit_locked_camera(&app),
            "fit-lock should auto-fit while physics is running and interaction is idle"
        );
    }

    #[test]
    fn unlocked_camera_never_autofits() {
        let mut app = test_app();
        app.set_camera_fit_locked(false);
        app.workspace.physics.base.is_running = true;
        app.workspace.is_interacting = false;

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
            .views
            .insert(view_id, crate::app::GraphViewState::new("Fit Guardrail"));
        app.workspace.focused_view = Some(view_id);

        app.add_node_and_sync("https://a.example".into(), Point2D::new(-120.0, -80.0));
        app.add_node_and_sync("https://b.example".into(), Point2D::new(180.0, 140.0));
        app.workspace.egui_state = Some(EguiGraphState::from_graph_with_visual_state(
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
    fn test_set_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::SetHighlightedEdge { from, to }]);
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.highlighted_graph_edge, Some((from, to)));
    }

    #[test]
    fn test_clear_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));
        app.workspace.highlighted_graph_edge = Some((from, to));

        let intents = intents_from_graph_actions(vec![GraphAction::ClearHighlightedEdge]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.highlighted_graph_edge.is_none());
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
    fn test_zoom_action_clamps() {
        let mut app = test_app();

        let intents = intents_from_graph_actions(vec![GraphAction::Zoom(0.01)]);
        app.apply_reducer_intents(intents);

        // Should be clamped to min zoom
        assert!(app.workspace.camera.current_zoom >= app.workspace.camera.zoom_min);
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
        app.workspace.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.workspace.focused_view = Some(view_id);
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
        assert!((app.workspace.views[&view_id].camera.current_zoom - 1.5).abs() < 0.01);
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
        app.workspace.show_command_palette = true;
        app.workspace.command_palette_contextual_mode = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, Some(pointer));

        assert!(!app.workspace.show_radial_menu);
        assert!(app.workspace.show_command_palette);
        assert!(app.pending_node_context_target().is_none());
        assert!(app.workspace.command_palette_contextual_mode);
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
        app.workspace.show_radial_menu = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, None);

        assert!(app.workspace.show_command_palette);
        assert!(!app.workspace.show_radial_menu);
        assert!(app.workspace.command_palette_contextual_mode);
        assert_eq!(app.pending_node_context_target(), Some(target));
        assert_eq!(
            app.pending_command_surface_return_target(),
            Some(crate::app::ToolSurfaceReturnTarget::Graph(view_id))
        );
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
        let _ = app.add_edge_and_sync(a, b, crate::graph::EdgeType::Hyperlink, None);
        let _ = app.add_edge_and_sync(c, a, crate::graph::EdgeType::Hyperlink, None);

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
        app.workspace.search_display_mode = SearchDisplayMode::Highlight;
        app.workspace.egui_state = Some(EguiGraphState::from_graph_with_visual_state(
            &app.workspace.domain.graph,
            app.focused_selection(),
            app.focused_selection().primary(),
            &HashSet::new(),
        ));
        let matches = HashSet::from([a]);
        let selection = app.focused_selection().clone();
        apply_search_node_visuals(&mut app, &selection, &matches, Some(a), true);

        let state = app.workspace.egui_state.as_ref().unwrap();
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
        app.workspace.search_display_mode = SearchDisplayMode::Filter;
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
            .suggested_semantic_tags
            .insert(numerical, vec!["udc:519.8".to_string()]);
        app.workspace.semantic_index_dirty = true;
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
        app.workspace.views.insert(view_id, view);

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
        app.workspace.views.insert(view_id, view);

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
        app.workspace.egui_state = Some(EguiGraphState::from_graph(
            &app.workspace.domain.graph,
            &std::collections::HashSet::new(),
        ));
        // Simulate egui_graphs moving the dragged node by delta.
        if let Some(state_mut) = app.workspace.egui_state.as_mut() {
            if let Some(node) = state_mut.graph.node_mut(dragged_key) {
                let old = node.location();
                node.set_location(
                    euclid::default::Point2D::new(old.x + delta.x, old.y + delta.y).to_pos2(),
                );
            }
        }
        app.workspace.is_interacting = true;
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
            let _ =
                app.add_edge_and_sync(pair[0], pair[1], crate::graph::EdgeType::Hyperlink, None);
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
            let _ =
                app.add_edge_and_sync(pair[0], pair[1], crate::graph::EdgeType::Hyperlink, None);
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
        assert_eq!(app.workspace.focused_view, Some(view_id));
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
        assert_eq!(app.workspace.focused_view, None);
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
            .views
            .insert(view_id, crate::app::GraphViewState::new("Seed Pan Test"));
        app.workspace.graph_view_frames.insert(
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
        app.workspace.views.insert(
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
