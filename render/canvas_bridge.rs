/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host bridge between the app's domain graph and the `graph-canvas` crate.
//!
//! This module provides the adapter layer that converts between Graphshell's
//! application-level types (`GraphBrowserApp`, `GraphViewState`, `NodeKey`,
//! `GraphAction`/`GraphIntent`) and graph-canvas's portable types
//! (`CanvasSceneInput`, `CanvasAction`, `CanvasCamera`, `CanvasInputEvent`).
//!
//! The bridge is intentionally thin — it does not contain rendering or
//! interaction logic. It maps between type systems so that graph-canvas can
//! consume host data and the host can consume graph-canvas outputs.
//!
//! This code is host-specific and will be replaced when migrating from egui
//! to iced. The graph-canvas crate itself remains framework-agnostic.

use euclid::default::{Point2D, Vector2D};
use petgraph::visit::{EdgeRef as PetgraphEdgeRef, IntoEdgeReferences};

use graph_canvas::camera::{CanvasCamera, CanvasViewport};
use graph_canvas::input::{CanvasInputEvent, Modifiers, PointerButton};
use graph_canvas::interaction::CanvasAction;
use graph_canvas::projection::{ProjectionMode, ViewDimension};
use graph_canvas::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, SceneMode, ViewId};

use crate::app::{GraphViewId, SelectionUpdateMode};
use crate::graph::{Graph, NodeKey};
use crate::render::GraphAction;

// ── Scene input construction ────────────────────────────────────────────────

/// Build a `CanvasSceneInput` from the domain graph and view state.
///
/// This is the primary host→canvas data path. The host calls this once per
/// frame for each active graph view, then feeds the result into
/// `derive_scene()`.
pub fn build_scene_input(
    graph: &Graph,
    view_id: GraphViewId,
    scene_mode: crate::app::SceneMode,
    dimension: &ViewDimension,
) -> CanvasSceneInput<NodeKey> {
    let nodes: Vec<CanvasNode<NodeKey>> = graph
        .nodes()
        .map(|(key, node)| CanvasNode {
            id: key,
            position: node.projected_position(),
            radius: default_node_radius(),
            label: Some(node.title.clone()),
        })
        .collect();

    // One CanvasEdge per petgraph edge (not per EdgeView/family).
    let edges: Vec<CanvasEdge<NodeKey>> = graph
        .inner
        .edge_references()
        .map(|e| CanvasEdge {
            source: e.source(),
            target: e.target(),
            weight: 1.0,
        })
        .collect();

    CanvasSceneInput {
        view_id: view_id_to_canvas(view_id),
        nodes,
        edges,
        scene_objects: Vec::new(),
        overlays: Vec::new(),
        scene_mode: scene_mode_to_canvas(scene_mode),
        projection: ProjectionMode::from_view_dimension(dimension),
    }
}

/// Default node radius in world units.
///
/// The current egui_graphs pipeline derives radius from `GraphNodeShape`. For
/// the bridge, we use a constant matching the default node radius.
fn default_node_radius() -> f32 {
    16.0
}

// ── Action translation ──────────────────────────────────────────────────────

/// Convert a `CanvasAction<NodeKey>` into zero or more `GraphAction`s.
///
/// This maps graph-canvas's portable actions back into the host's existing
/// action vocabulary. The caller feeds the returned `GraphAction`s into
/// `intents_from_graph_actions()` as usual.
pub fn canvas_action_to_graph_actions(action: CanvasAction<NodeKey>) -> Vec<GraphAction> {
    match action {
        CanvasAction::SelectNode(key) => vec![GraphAction::SelectNode {
            key,
            multi_select: false,
        }],
        CanvasAction::ToggleSelectNode(key) => vec![GraphAction::SelectNode {
            key,
            multi_select: true,
        }],
        CanvasAction::ClearSelection => vec![GraphAction::ClearSelection],
        CanvasAction::HoverSceneObject(_) | CanvasAction::ClickSceneObject(_) => vec![],
        CanvasAction::DragNode { node, delta } => {
            // DragNode carries a world-space delta per frame. The host needs
            // an absolute position for MoveNode. The caller must compute this
            // by adding the delta to the node's current projected position.
            vec![GraphAction::DragStart]
        }
        CanvasAction::LassoComplete { nodes } => vec![GraphAction::LassoSelect {
            keys: nodes,
            mode: SelectionUpdateMode::Replace,
        }],
        CanvasAction::ZoomCamera { factor, .. } => vec![GraphAction::Zoom(factor)],
        // Hover/Lasso lifecycle/Pan actions don't map to GraphActions — they
        // are handled by the interaction engine's state or applied directly
        // to the camera.
        CanvasAction::HoverNode(_)
        | CanvasAction::HoverEdge(_)
        | CanvasAction::DeselectNode(_)
        | CanvasAction::LassoBegin { .. }
        | CanvasAction::LassoUpdate { .. }
        | CanvasAction::LassoCancel
        | CanvasAction::PanCamera(_) => Vec::new(),
    }
}

/// Apply a `DragNode` action: update the node's projected position.
///
/// Separated from `canvas_action_to_graph_actions` because it needs mutable
/// graph access. Returns the resulting `MoveNode` `GraphAction`.
pub fn apply_drag_node_delta(
    graph: &mut Graph,
    node: NodeKey,
    delta: Vector2D<f32>,
) -> Option<GraphAction> {
    if let Some(n) = graph.get_node(node) {
        let old_pos = n.projected_position();
        let new_pos = old_pos + delta;
        graph.set_node_projected_position(node, new_pos);
        Some(GraphAction::MoveNode(node, new_pos))
    } else {
        None
    }
}

/// Apply a `PanCamera` action to the canvas camera.
pub fn apply_pan(camera: &mut CanvasCamera, delta: Vector2D<f32>) {
    camera.pan += delta;
}

/// Apply a `ZoomCamera` action to the canvas camera.
///
/// Zooms toward the focus point so the world-space point under the cursor
/// stays visually fixed.
pub fn apply_zoom(
    camera: &mut CanvasCamera,
    factor: f32,
    focus: Point2D<f32>,
    viewport: &CanvasViewport,
) {
    let world_focus = camera.screen_to_world(focus, viewport);
    camera.zoom *= factor;
    camera.zoom = camera.zoom.clamp(0.1, 10.0);
    let new_screen = camera.world_to_screen(world_focus, viewport);
    let correction = focus - new_screen;
    camera.pan += Vector2D::new(correction.x / camera.zoom, correction.y / camera.zoom);
}

// ── Camera sync ─────────────────────────────────────────────────────────────

/// Construct a `CanvasCamera` from the app's `GraphViewFrame`.
pub fn camera_from_view_frame(frame: crate::app::GraphViewFrame) -> CanvasCamera {
    CanvasCamera {
        pan: Vector2D::new(frame.pan_x, frame.pan_y),
        zoom: frame.zoom.max(0.01),
    }
}

/// Write a `CanvasCamera` back to a `GraphViewFrame`.
pub fn camera_to_view_frame(camera: &CanvasCamera) -> crate::app::GraphViewFrame {
    crate::app::GraphViewFrame {
        zoom: camera.zoom,
        pan_x: camera.pan.x,
        pan_y: camera.pan.y,
    }
}

/// Construct a `CanvasViewport` from an egui `Rect`.
pub fn viewport_from_egui_rect(rect: egui::Rect, scale_factor: f32) -> CanvasViewport {
    CanvasViewport {
        rect: euclid::default::Rect::new(
            Point2D::new(rect.min.x, rect.min.y),
            euclid::default::Size2D::new(rect.width(), rect.height()),
        ),
        scale_factor,
    }
}

// ── Event translation ───────────────────────────────────────────────────────

/// Translate egui input state for the current frame into `CanvasInputEvent`s.
///
/// Call this once per frame with the egui `InputState`. The function inspects
/// pointer position, button state, and scroll delta to produce the
/// corresponding portable events.
///
/// This is a stateless translation — the interaction engine handles
/// click-vs-drag and gesture lifecycle.
pub fn collect_canvas_events(ui: &egui::Ui) -> Vec<CanvasInputEvent> {
    let mut events = Vec::new();

    ui.input(|input| {
        let mods = Modifiers {
            ctrl: input.modifiers.ctrl || input.modifiers.command,
            shift: input.modifiers.shift,
            alt: input.modifiers.alt,
        };

        // Pointer position
        if let Some(pos) = input.pointer.latest_pos() {
            let position = Point2D::new(pos.x, pos.y);

            // Button press events
            if input.pointer.primary_pressed() {
                events.push(CanvasInputEvent::PointerPressed {
                    position,
                    button: PointerButton::Primary,
                    modifiers: mods,
                });
            }
            if input.pointer.secondary_pressed() {
                events.push(CanvasInputEvent::PointerPressed {
                    position,
                    button: PointerButton::Secondary,
                    modifiers: mods,
                });
            }
            if input.pointer.button_pressed(egui::PointerButton::Middle) {
                events.push(CanvasInputEvent::PointerPressed {
                    position,
                    button: PointerButton::Middle,
                    modifiers: mods,
                });
            }

            // Button release events
            if input.pointer.primary_released() {
                events.push(CanvasInputEvent::PointerReleased {
                    position,
                    button: PointerButton::Primary,
                    modifiers: mods,
                });
            }
            if input.pointer.secondary_released() {
                events.push(CanvasInputEvent::PointerReleased {
                    position,
                    button: PointerButton::Secondary,
                    modifiers: mods,
                });
            }
            if input.pointer.button_released(egui::PointerButton::Middle) {
                events.push(CanvasInputEvent::PointerReleased {
                    position,
                    button: PointerButton::Middle,
                    modifiers: mods,
                });
            }

            // Double-click
            if input
                .pointer
                .button_double_clicked(egui::PointerButton::Primary)
            {
                events.push(CanvasInputEvent::PointerDoubleClick {
                    position,
                    button: PointerButton::Primary,
                    modifiers: mods,
                });
            }

            // Pointer movement (emit if pointer is present and no buttons were
            // just pressed/released — those already carry position).
            if !input.pointer.primary_pressed()
                && !input.pointer.secondary_pressed()
                && !input.pointer.button_pressed(egui::PointerButton::Middle)
                && !input.pointer.primary_released()
                && !input.pointer.secondary_released()
                && !input.pointer.button_released(egui::PointerButton::Middle)
            {
                events.push(CanvasInputEvent::PointerMoved { position });
            }

            // Scroll/zoom
            let scroll_delta = input.smooth_scroll_delta.y;
            if scroll_delta.abs() > f32::EPSILON {
                events.push(CanvasInputEvent::Scroll {
                    delta: scroll_delta,
                    position,
                    modifiers: mods,
                });
            }
        } else {
            // Pointer left the window
            events.push(CanvasInputEvent::PointerLeft);
        }
    });

    events
}

// ── Type conversions ────────────────────────────────────────────────────────

fn scene_mode_to_canvas(mode: crate::app::SceneMode) -> SceneMode {
    match mode {
        crate::app::SceneMode::Browse => SceneMode::Browse,
        crate::app::SceneMode::Arrange => SceneMode::Arrange,
        crate::app::SceneMode::Simulate => SceneMode::Simulate,
    }
}

fn view_id_to_canvas(id: GraphViewId) -> ViewId {
    // ViewId is an opaque u64. Use the UUID's lower 64 bits.
    let uuid = id.as_uuid();
    let bytes = uuid.as_bytes();
    let lower = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    ViewId(lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_roundtrip() {
        let frame = crate::app::GraphViewFrame {
            zoom: 1.5,
            pan_x: 100.0,
            pan_y: -50.0,
        };
        let camera = camera_from_view_frame(frame);
        assert_eq!(camera.zoom, 1.5);
        assert_eq!(camera.pan.x, 100.0);
        assert_eq!(camera.pan.y, -50.0);

        let back = camera_to_view_frame(&camera);
        assert_eq!(back.zoom, 1.5);
        assert_eq!(back.pan_x, 100.0);
        assert_eq!(back.pan_y, -50.0);
    }

    #[test]
    fn scene_mode_conversion() {
        assert_eq!(
            scene_mode_to_canvas(crate::app::SceneMode::Browse),
            SceneMode::Browse
        );
        assert_eq!(
            scene_mode_to_canvas(crate::app::SceneMode::Arrange),
            SceneMode::Arrange
        );
        assert_eq!(
            scene_mode_to_canvas(crate::app::SceneMode::Simulate),
            SceneMode::Simulate
        );
    }

    #[test]
    fn zoom_preserves_focus_point() {
        let mut camera = CanvasCamera::default();
        camera.zoom = 1.0;
        let viewport = CanvasViewport::default();
        let focus = Point2D::new(400.0, 300.0);

        let world_before = camera.screen_to_world(focus, &viewport);
        apply_zoom(&mut camera, 1.5, focus, &viewport);
        let world_after = camera.screen_to_world(focus, &viewport);

        assert!((world_before.x - world_after.x).abs() < 0.1);
        assert!((world_before.y - world_after.y).abs() < 0.1);
    }

    #[test]
    fn clear_selection_maps() {
        let actions = canvas_action_to_graph_actions(CanvasAction::ClearSelection);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], GraphAction::ClearSelection));
    }

    #[test]
    fn select_node_maps() {
        let key = NodeKey::new(5);
        let actions = canvas_action_to_graph_actions(CanvasAction::SelectNode(key));
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            GraphAction::SelectNode {
                multi_select: false,
                ..
            }
        ));
    }

    #[test]
    fn toggle_select_maps_to_multi() {
        let key = NodeKey::new(3);
        let actions = canvas_action_to_graph_actions(CanvasAction::ToggleSelectNode(key));
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            GraphAction::SelectNode {
                multi_select: true,
                ..
            }
        ));
    }

    #[test]
    fn lasso_maps_to_replace() {
        let keys = vec![NodeKey::new(1), NodeKey::new(2)];
        let actions =
            canvas_action_to_graph_actions(CanvasAction::LassoComplete { nodes: keys.clone() });
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            GraphAction::LassoSelect {
                mode: SelectionUpdateMode::Replace,
                ..
            }
        ));
    }

    #[test]
    fn hover_maps_to_empty() {
        let actions = canvas_action_to_graph_actions(CanvasAction::HoverNode(Some(NodeKey::new(0))));
        assert!(actions.is_empty());
    }
}
