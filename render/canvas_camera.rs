/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas camera: keyboard pan, wheel zoom, camera commands, pan inertia, and
//! metadata-frame seeding for graph pane views.

use crate::app::{CameraCommand, GraphBrowserApp, KeyboardPanInputMode, KeyboardZoomRequest};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_NO_BOUNDS, CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_ZERO_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_FIT_DEFERRED_NO_METADATA,
    CHANNEL_UI_GRAPH_CAMERA_ZOOM_DEFERRED_NO_METADATA,
    CHANNEL_UI_GRAPH_FIT_SELECTION_FALLBACK_TO_FIT, CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK,
    CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW,
    CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED_NO_METADATA,
    CHANNEL_UI_GRAPH_WHEEL_ZOOM_BLOCKED_INVALID_FACTOR,
    CHANNEL_UI_GRAPH_WHEEL_ZOOM_DEFERRED_NO_METADATA,
};
use egui::{Ui, Vec2};
use egui_graphs::MetadataFrame;
use std::time::Duration;

use super::canvas_visuals::node_bounds_for_selection;

// ── Internal helpers ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct KeyboardPanKeys {
    pub(super) up: bool,
    pub(super) down: bool,
    pub(super) left: bool,
    pub(super) right: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct KeyboardPanInputState {
    pub(super) wasd: KeyboardPanKeys,
    pub(super) arrows: KeyboardPanKeys,
}

pub(super) fn keyboard_pan_delta_from_state(
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

pub(super) fn keyboard_pan_delta_from_keys(keys: KeyboardPanKeys, step: f32) -> Vec2 {
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

pub(super) fn pan_inertia_velocity_id(metadata_id: egui::Id) -> egui::Id {
    metadata_id.with("pan_inertia_velocity")
}

fn clear_pan_inertia_velocity(ctx: &egui::Context, metadata_id: egui::Id) {
    let velocity_id = pan_inertia_velocity_id(metadata_id);
    ctx.data_mut(|data| data.remove::<Vec2>(velocity_id));
}

fn node_bounds_for_all(app: &GraphBrowserApp) -> Option<(f32, f32, f32, f32)> {
    let state = app.workspace.graph_runtime.egui_state.as_ref()?;
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

// ── pub(super) API ────────────────────────────────────────────────────────────

pub(super) fn should_auto_fit_locked_camera(app: &GraphBrowserApp) -> bool {
    app.camera_position_fit_locked()
        && !app.workspace.graph_runtime.is_interacting
        && app.workspace.graph_runtime.physics.base.is_running
}

pub(super) fn keyboard_pan_allowed_for_view(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> bool {
    if app.workspace.graph_runtime.focused_view == Some(view_id) {
        return true;
    }

    app.workspace.graph_runtime.focused_view.is_none()
        && app.workspace.graph_runtime.views.len() == 1
        && app.workspace.graph_runtime.views.contains_key(&view_id)
}

pub(super) fn keyboard_pan_delta_from_input(
    ui: &Ui,
    step: f32,
    mode: KeyboardPanInputMode,
) -> Vec2 {
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

pub(super) fn seeded_metadata_frame_for_view(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> MetadataFrame {
    let mut frame = MetadataFrame::default();
    if let Some(view_frame) = app.workspace.graph_runtime.graph_view_frames.get(&view_id) {
        frame.zoom = view_frame.zoom.max(0.01);
        frame.pan = egui::vec2(view_frame.pan_x, view_frame.pan_y);
        return frame;
    }

    if let Some(view) = app.workspace.graph_runtime.views.get(&view_id) {
        frame.zoom = view.camera.current_zoom.max(0.01);
    }

    frame
}

pub(super) fn emit_keyboard_pan_blocked_if_needed(
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

pub(super) fn apply_background_pan(
    ctx: &egui::Context,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    delta: Vec2,
) -> bool {
    if delta == Vec2::ZERO {
        return false;
    }

    super::set_focused_view_with_transition(app, Some(view_id));
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

pub(super) fn apply_background_pan_inertia(
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

pub(super) fn apply_pending_keyboard_zoom_request(
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
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_min);
    let zoom_max = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_max);

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

    if let Some(new_zoom) = updated_zoom {
        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.camera.current_zoom = new_zoom;
        }
    }

    updated_zoom
}

pub(super) fn apply_pending_camera_command(
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
        if !app.workspace.graph_runtime.views.contains_key(&target_view) {
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
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_min);
    let zoom_max = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_max);

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
                if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
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
                if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
                    view.camera.current_zoom = new_zoom;
                }
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
    }
}

pub(super) fn apply_pending_wheel_zoom(
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
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_min);
    let zoom_max = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_max);

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
                && let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id)
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

/// Apply Graphshell-owned camera/navigation to the egui_graphs metadata frame.
///
/// This is the canonical camera path for graph panes. It is not a fallback for
/// egui_graphs navigation; egui_graphs navigation is intentionally disabled so
/// Graphshell can own fit-to-screen, focus-on-selection, keyboard pan/zoom,
/// and policy-driven camera behavior without a competing camera authority.
pub(super) fn handle_custom_navigation(
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

    let camera_zoom = apply_pending_camera_command(ui, app, metadata_id, view_id, canvas_profile);

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

    if !position_fit_locked
        && canvas_profile.allows_background_pan(
            app.workspace.graph_runtime.hovered_graph_node.is_none(),
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

    let zoom_min = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_min);
    let zoom_max = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.graph_runtime.camera.zoom_max);
    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let clamped = meta.zoom.clamp(zoom_min, zoom_max);
            if (meta.zoom - clamped).abs() > f32::EPSILON {
                meta.zoom = clamped;
            }
            let current_zoom = meta.zoom;
            data.insert_persisted(metadata_id, meta);
            if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
                view.camera.current_zoom = current_zoom;
            }
        }
    });

    camera_zoom.or(keyboard_zoom).or(wheel_zoom)
}
