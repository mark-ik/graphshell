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
//! The bridge is intentionally thin — it does not contain rendering logic. It
//! maps between type systems so that graph-canvas can consume app data and the
//! host can consume graph-canvas outputs.
//!
//! Most of this module is host-neutral app↔canvas plumbing that future hosts
//! such as iced can reuse directly. The egui-only viewport and input helpers
//! near the bottom are transitional host glue.

use euclid::default::{Point2D, Vector2D};
use petgraph::visit::{EdgeRef as PetgraphEdgeRef, IntoEdgeReferences};
use std::collections::HashSet;

use graph_canvas::camera::{CanvasCamera, CanvasViewport};
use graph_canvas::input::{CanvasInputEvent, Modifiers, PointerButton};
use graph_canvas::interaction::CanvasAction;
use graph_canvas::projection::{ProjectionMode, ViewDimension};
use graph_canvas::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, SceneMode, ViewId};

use crate::app::{GraphBrowserApp, GraphViewId, SearchDisplayMode, SelectionUpdateMode};
use crate::graph::{Graph, NodeKey};
use crate::render::GraphAction;
use graph_canvas::packet::ProjectedScene;

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
    visible_nodes: Option<&HashSet<NodeKey>>,
) -> CanvasSceneInput<NodeKey> {
    let nodes: Vec<CanvasNode<NodeKey>> = graph
        .nodes()
        .filter(|(key, _)| visible_nodes.is_none_or(|mask| mask.contains(key)))
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
        .filter(|edge| {
            visible_nodes
                .is_none_or(|mask| mask.contains(&edge.source()) && mask.contains(&edge.target()))
        })
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

/// Output of one host-neutral graph-canvas frame.
///
/// The scene is derived before the frame's input events are applied, so hosts
/// can paint the returned packet immediately and let interaction updates land
/// on the next frame. This matches the current egui immediate-mode behavior.
pub struct GraphCanvasFrameOutput {
    pub scene: ProjectedScene<NodeKey>,
    pub graph_actions: Vec<GraphAction>,
}

/// Run one graph-canvas frame against portable viewport + input data.
///
/// This is the host-neutral seam that future hosts should call. Framework-
/// specific adapters are responsible only for:
/// - constructing a `CanvasViewport`
/// - translating host input into `CanvasInputEvent`s
/// - painting the returned `ProjectedScene`
pub fn run_graph_canvas_frame(
    app: &mut GraphBrowserApp,
    view_id: GraphViewId,
    search_matches: &HashSet<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
    viewport: CanvasViewport,
    events: &[CanvasInputEvent],
) -> GraphCanvasFrameOutput {
    use graph_canvas::derive::{DeriveConfig, NodeVisualOverride, derive_scene};
    use graph_canvas::engine::InteractionEngine;
    use graph_canvas::interaction::CanvasAction;
    use graph_canvas::packet::{Color, Stroke};
    use graph_canvas::projection::ViewDimension;

    app.ensure_graph_view_registered(view_id);

    let view_selection = app.selection_for_view(view_id).clone();
    let filtered_visible_nodes = super::canvas_visuals::visible_nodes_for_view_filters(
        app,
        view_id,
        search_matches,
        search_display_mode,
        search_query_active,
    );
    let scene_mode_app = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|view| view.scene_mode)
        .unwrap_or_default();
    let scene_input = {
        let graph = app.render_graph();
        build_scene_input(
            graph,
            view_id,
            scene_mode_app,
            &ViewDimension::default(),
            filtered_visible_nodes.as_ref(),
        )
    };

    let mut camera = app
        .workspace
        .graph_runtime
        .canvas_cameras
        .remove(&view_id)
        .or_else(|| {
            app.workspace
                .graph_runtime
                .graph_view_frames
                .get(&view_id)
                .map(|frame| camera_from_view_frame(frame.clone()))
        })
        .unwrap_or_default();

    let selected_primary = view_selection.primary();
    let scene = derive_scene(
        &scene_input,
        &camera,
        &viewport,
        &|_node| 0.0,
        &|_idx, node_id| {
            let is_selected = view_selection.contains(node_id);
            let is_primary = selected_primary == Some(*node_id);
            let is_in_search = search_matches.contains(node_id);
            if is_primary {
                NodeVisualOverride {
                    fill: Some(Color::new(0.3, 0.7, 1.0, 1.0)),
                    stroke: Some(Stroke {
                        color: Color::new(1.0, 1.0, 1.0, 1.0),
                        width: 2.5,
                    }),
                    label_color: Some(Color::WHITE),
                }
            } else if is_selected {
                NodeVisualOverride {
                    fill: Some(Color::new(0.3, 0.6, 0.9, 0.9)),
                    stroke: Some(Stroke {
                        color: Color::new(0.8, 0.9, 1.0, 0.8),
                        width: 1.5,
                    }),
                    label_color: None,
                }
            } else if is_in_search {
                NodeVisualOverride {
                    fill: Some(Color::new(0.9, 0.8, 0.2, 1.0)),
                    stroke: None,
                    label_color: None,
                }
            } else {
                NodeVisualOverride::default()
            }
        },
        &DeriveConfig::default(),
    );

    let mut engine = app
        .workspace
        .graph_runtime
        .canvas_interaction_engines
        .remove(&view_id)
        .unwrap_or_else(|| InteractionEngine::new(Default::default()));
    let mut graph_actions = Vec::new();

    for event in events {
        let actions = engine.process_event(event, &scene.hit_proxies, &camera, &viewport);
        for action in actions {
            match &action {
                CanvasAction::PanCamera(delta) => {
                    apply_pan(&mut camera, *delta);
                }
                CanvasAction::ZoomCamera { factor, focus } => {
                    apply_zoom(&mut camera, *factor, *focus, &viewport);
                }
                CanvasAction::DragNode { node, delta } => {
                    if let Some(graph_action) =
                        apply_drag_node_delta(app.domain_graph_mut(), *node, *delta)
                    {
                        graph_actions.push(graph_action);
                    }
                }
                CanvasAction::HoverNode(maybe_key) => {
                    app.workspace.graph_runtime.hovered_graph_node = *maybe_key;
                }
                _ => {
                    graph_actions.extend(canvas_action_to_graph_actions(action));
                }
            }
        }
    }

    let frame = camera_to_view_frame(&camera);
    app.workspace
        .graph_runtime
        .graph_view_frames
        .insert(view_id, frame);
    app.workspace
        .graph_runtime
        .canvas_cameras
        .insert(view_id, camera);
    app.workspace
        .graph_runtime
        .canvas_interaction_engines
        .insert(view_id, engine);

    GraphCanvasFrameOutput {
        scene,
        graph_actions,
    }
}

#[cfg(test)]
mod scene_input_tests {
    use super::*;
    use crate::app::GraphBrowserApp;
    use crate::graph::{EdgeAssertion, SemanticSubKind};
    use euclid::default::Size2D;
    use graph_canvas::packet::HitProxy;

    #[test]
    fn build_scene_input_respects_visible_node_mask() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let a = app.add_node_and_sync("https://example.com/a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://example.com/b".into(), Point2D::new(50.0, 0.0));
        let c = app.add_node_and_sync("https://example.com/c".into(), Point2D::new(100.0, 0.0));
        let _ = app.assert_relation_and_sync(
            a,
            b,
            EdgeAssertion::Semantic {
                sub_kind: SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );
        let _ = app.assert_relation_and_sync(
            b,
            c,
            EdgeAssertion::Semantic {
                sub_kind: SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        let visible = HashSet::from([a, b]);
        let scene = build_scene_input(
            app.render_graph(),
            view_id,
            crate::app::SceneMode::Browse,
            &ViewDimension::default(),
            Some(&visible),
        );

        let node_ids: HashSet<NodeKey> = scene.nodes.iter().map(|node| node.id).collect();
        assert_eq!(node_ids, visible);
        assert_eq!(scene.edges.len(), 1);
        assert_eq!(scene.edges[0].source, a);
        assert_eq!(scene.edges[0].target, b);
    }

    fn test_viewport() -> CanvasViewport {
        CanvasViewport {
            rect: euclid::default::Rect::new(Point2D::new(0.0, 0.0), Size2D::new(800.0, 600.0)),
            scale_factor: 1.0,
        }
    }

    #[test]
    fn run_graph_canvas_frame_applies_search_filter_to_scene() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let a = app.add_node_and_sync("https://example.com/a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://example.com/b".into(), Point2D::new(80.0, 0.0));
        let matches = HashSet::from([a]);

        let output = run_graph_canvas_frame(
            &mut app,
            view_id,
            &matches,
            SearchDisplayMode::Filter,
            true,
            test_viewport(),
            &[],
        );

        let visible_nodes: HashSet<NodeKey> = output
            .scene
            .hit_proxies
            .iter()
            .filter_map(|proxy| match proxy {
                HitProxy::Node { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(visible_nodes, matches);
        assert!(!visible_nodes.contains(&b));
    }

    #[test]
    fn run_graph_canvas_frame_updates_camera_from_scroll_events() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let output = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            test_viewport(),
            &[CanvasInputEvent::Scroll {
                delta: 1.0,
                position: Point2D::new(400.0, 300.0),
                modifiers: Modifiers::default(),
            }],
        );

        assert!(
            output
                .graph_actions
                .iter()
                .any(|action| matches!(action, GraphAction::Zoom(factor) if *factor > 1.0))
        );
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("frame should be persisted after running the canvas frame");
        assert!(frame.zoom > 1.0);
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
        CanvasAction::DragNode { .. } => {
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
        let actions = canvas_action_to_graph_actions(CanvasAction::LassoComplete {
            nodes: keys.clone(),
        });
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
        let actions =
            canvas_action_to_graph_actions(CanvasAction::HoverNode(Some(NodeKey::new(0))));
        assert!(actions.is_empty());
    }
}
