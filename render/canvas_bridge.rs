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
    default_node_radius: f32,
) -> CanvasSceneInput<NodeKey> {
    let nodes: Vec<CanvasNode<NodeKey>> = graph
        .nodes()
        .filter(|(key, _)| visible_nodes.is_none_or(|mask| mask.contains(key)))
        .map(|(key, node)| CanvasNode {
            id: key,
            position: node.projected_position(),
            radius: default_node_radius,
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
    use graph_canvas::derive::{
        DeriveConfig, NodeVisualOverride, OverlayInputs, OverlayStyle, derive_scene_with_overlays,
    };
    use graph_canvas::engine::InteractionEngine;
    use graph_canvas::interaction::CanvasAction;
    use graph_canvas::layout::{ForceDirected, Layout, LayoutExtras};
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

    // Resolve the two user-tunable policies once per frame so the
    // render bridge reads the same values throughout: NodeStyle for
    // default node radius + selection/search visuals, NavigationPolicy
    // for camera/input/inertia knobs. Both honor per-view override
    // then per-graph default, so a settings surface can tune feel at
    // either scope.
    let node_style = app.resolve_node_style(view_id);
    let navigation_policy = app.resolve_navigation_policy(view_id);

    let scene_input = {
        let graph = app.render_graph();
        build_scene_input(
            graph,
            view_id,
            scene_mode_app,
            &ViewDimension::default(),
            filtered_visible_nodes.as_ref(),
            node_style.default_radius,
        )
    };

    // Force-directed tick: advance positions when physics is running and the
    // user isn't actively dragging. Writes per-node deltas straight into
    // petgraph; there is no mirror carrier.
    if app.workspace.graph_runtime.physics.is_running && !app.workspace.graph_runtime.is_interacting
    {
        let pinned: std::collections::HashSet<NodeKey> = app
            .domain_graph()
            .nodes()
            .filter_map(|(key, node)| node.is_pinned.then_some(key))
            .collect();
        let extras = LayoutExtras {
            pinned,
            ..Default::default()
        };
        let mut layout = ForceDirected::new();
        let physics_state = &mut app.workspace.graph_runtime.physics;
        let deltas = layout.step(&scene_input, physics_state, 0.0, &viewport, &extras);
        if !deltas.is_empty() {
            let positions: Vec<(NodeKey, euclid::default::Point2D<f32>)> = deltas
                .iter()
                .filter_map(|(key, delta)| {
                    let current = app.domain_graph().node_projected_position(*key)?;
                    Some((*key, current + *delta))
                })
                .collect();
            let domain = app.domain_graph_mut();
            for (key, pos) in positions {
                let _ = domain.set_node_projected_position(key, pos);
            }
        }
    }

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

    // Overlay inputs: portable types bridged from app-side state so the
    // graph-canvas derive pipeline can emit frame-affinity discs,
    // scene-region backdrops, and the highlighted-edge accent stroke.
    let frame_regions = build_portable_frame_regions(app);
    let scene_regions = build_portable_scene_regions(app, view_id);
    let highlighted_edge = app.workspace.graph_runtime.highlighted_graph_edge;
    let overlay_inputs = OverlayInputs {
        frame_regions: &frame_regions,
        scene_regions: &scene_regions,
        highlighted_edge,
    };
    let overlay_style = OverlayStyle::default();

    let scene = derive_scene_with_overlays(
        &scene_input,
        &camera,
        &viewport,
        &|_node| 0.0,
        &|_idx, node_id| {
            // Project the host's per-node interaction state (primary
            // selected / secondary selected / search-hit / neither)
            // into a `NodeVisualOverride` using the resolved
            // `NodeStyle`. Colors and stroke widths live entirely in
            // the policy — no hardcoded design decisions here.
            let is_selected = view_selection.contains(node_id);
            let is_primary = selected_primary == Some(*node_id);
            let is_in_search = search_matches.contains(node_id);
            let state_style = if is_primary {
                Some(&node_style.primary_selection)
            } else if is_selected {
                Some(&node_style.secondary_selection)
            } else if is_in_search {
                Some(&node_style.search_hit)
            } else {
                None
            };
            match state_style {
                Some(s) => NodeVisualOverride {
                    fill: Some(s.fill),
                    stroke: s.stroke,
                    label_color: s.label_color,
                },
                None => NodeVisualOverride::default(),
            }
        },
        &overlay_inputs,
        &overlay_style,
        &DeriveConfig::default(),
    );

    let mut engine = app
        .workspace
        .graph_runtime
        .canvas_interaction_engines
        .remove(&view_id)
        .unwrap_or_else(|| InteractionEngine::new(navigation_policy.to_interaction_config()));
    // Refresh the engine's config from the resolved policy every
    // frame so user tuning takes effect without engine rebuild.
    engine.config = navigation_policy.to_interaction_config();
    let mut graph_actions = Vec::new();

    for event in events {
        let actions = engine.process_event(event, &scene.hit_proxies, &camera, &viewport);
        for action in actions {
            match &action {
                CanvasAction::PanCamera(delta) => {
                    apply_pan(&mut camera, *delta);
                }
                CanvasAction::ZoomCamera { factor, focus } => {
                    apply_zoom(&mut camera, *factor, *focus, &viewport, &navigation_policy);
                    graph_actions.push(GraphAction::Zoom(*factor));
                }
                CanvasAction::SetPanInertia(velocity) => {
                    camera.pan_velocity = *velocity;
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
                    if maybe_key.is_some() {
                        app.workspace.graph_runtime.hovered_graph_edge = None;
                    }
                }
                CanvasAction::HoverEdge(maybe_edge) => {
                    app.workspace.graph_runtime.hovered_graph_edge =
                        maybe_edge.as_ref().map(|edge| (edge.source, edge.target));
                    if maybe_edge.is_some() {
                        app.workspace.graph_runtime.hovered_graph_node = None;
                    }
                }
                _ => {
                    graph_actions.extend(canvas_action_to_graph_actions(action));
                }
            }
        }
    }

    // Coast the camera on drag-release inertia using the user-tuned
    // damping. Assumes 60 Hz frame cadence — the host doesn't thread
    // real dt through this seam today; good enough for feel.
    let _ = camera.tick_inertia(1.0 / 60.0, navigation_policy.pan_damping_per_second);

    // Consume any pending `CameraCommand` targeted at this view. The
    // command is ordered after the inertia tick so a Fit always wins
    // over any residual coast; `fit_to_bounds` clears `pan_velocity`
    // as part of the jump.
    if app.pending_camera_command_target() == Some(view_id) {
        if let Some(command) = app.pending_camera_command() {
            let consumed = apply_fit_camera_command(
                app,
                view_id,
                &scene_input,
                &mut camera,
                &viewport,
                command,
                &navigation_policy,
            );
            if consumed {
                app.clear_pending_camera_command();
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
            graph_canvas::node_style::DEFAULT_NODE_RADIUS,
        );

        let node_ids: HashSet<NodeKey> = scene.nodes.iter().map(|node| node.id).collect();
        assert_eq!(node_ids, visible);
        // The a→b hyperlink must survive the mask; any derived containment
        // edges between the visible pair also pass the same filter. The b→c
        // hyperlink must be dropped since c is not in the mask.
        assert!(
            scene
                .edges
                .iter()
                .any(|edge| edge.source == a && edge.target == b),
            "expected a→b edge to survive the visible mask"
        );
        assert!(
            scene
                .edges
                .iter()
                .all(|edge| visible.contains(&edge.source) && visible.contains(&edge.target)),
            "no edge should reference a filtered-out node"
        );
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
        // Under the browser-style navigation default, plain scroll pans
        // (no zoom change), and `Ctrl`+scroll zooms. The test exercises
        // both legs on the same app so the canvas bridge wires scroll
        // through the engine + camera the way the memory-pinned
        // navigation defaults require.
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();

        // Leg 1: plain wheel → pan, no zoom.
        let output_pan = run_graph_canvas_frame(
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
            !output_pan
                .graph_actions
                .iter()
                .any(|action| matches!(action, GraphAction::Zoom(_))),
            "plain wheel must not zoom",
        );
        let frame_after_pan = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("frame should be persisted after running the canvas frame")
            .clone();
        assert!(
            (frame_after_pan.zoom - 1.0).abs() < 1e-4,
            "plain wheel must not change zoom, got {}",
            frame_after_pan.zoom,
        );

        // Leg 2: Ctrl+wheel → zoom.
        let ctrl_mods = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let output_zoom = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            test_viewport(),
            &[CanvasInputEvent::Scroll {
                delta: 1.0,
                position: Point2D::new(400.0, 300.0),
                modifiers: ctrl_mods,
            }],
        );
        assert!(
            output_zoom
                .graph_actions
                .iter()
                .any(|action| matches!(action, GraphAction::Zoom(factor) if *factor > 1.0)),
            "Ctrl+wheel must zoom",
        );
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("frame should be persisted after running the canvas frame");
        assert!(frame.zoom > 1.0);
    }

    // ── Fit command dispatch ──────────────────────────────────────────

    fn fit_test_app(view_id: GraphViewId) -> GraphBrowserApp {
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(view_id);
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app
    }

    fn fit_test_viewport() -> CanvasViewport {
        CanvasViewport::new(Point2D::origin(), Size2D::new(800.0, 600.0), 1.0)
    }

    #[test]
    fn run_graph_canvas_frame_consumes_pending_fit_over_populated_graph() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        // Scatter three nodes far from origin so a Fit produces a
        // visibly non-identity camera.
        app.add_node_and_sync("https://a.test/".into(), Point2D::new(500.0, 500.0));
        app.add_node_and_sync("https://b.test/".into(), Point2D::new(-500.0, -500.0));
        app.add_node_and_sync("https://c.test/".into(), Point2D::new(0.0, 0.0));
        app.request_camera_command(crate::app::CameraCommand::Fit);
        assert!(app.pending_camera_command().is_some());

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        assert!(
            app.pending_camera_command().is_none(),
            "Fit should be consumed after one frame"
        );
        // Populated graph → zoom should change from the default 1.0.
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("view frame persisted");
        assert!(
            (frame.zoom - 1.0).abs() > f32::EPSILON,
            "Fit on populated graph should adjust zoom (got {})",
            frame.zoom
        );
    }

    #[test]
    fn run_graph_canvas_frame_consumes_pending_fit_on_empty_graph() {
        // No nodes → fit has nothing to frame, but the pending command
        // must still be consumed so the host doesn't busy-loop.
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.request_camera_command(crate::app::CameraCommand::Fit);

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        assert!(app.pending_camera_command().is_none());
    }

    #[test]
    fn run_graph_canvas_frame_fit_selection_frames_only_selected_nodes() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        let near = app.add_node_and_sync("https://near.test/".into(), Point2D::new(10.0, 10.0));
        app.add_node_and_sync("https://far.test/".into(), Point2D::new(10_000.0, 10_000.0));
        app.select_node(near, false);
        app.request_camera_command(crate::app::CameraCommand::FitSelection);

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        assert!(app.pending_camera_command().is_none());
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("view frame persisted");
        // If we fit to everything, pan would end up near -5000 (midpoint of
        // the two nodes). Fitting the selection alone pans much closer to
        // -10 (the single selected node's position). Distinguish the two.
        assert!(
            frame.pan_x.abs() < 200.0 && frame.pan_y.abs() < 200.0,
            "FitSelection should center on the selected node, not the graph midpoint: pan=({}, {})",
            frame.pan_x,
            frame.pan_y,
        );
    }

    #[test]
    fn run_graph_canvas_frame_fit_selection_falls_back_to_fit_when_no_selection() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.add_node_and_sync("https://a.test/".into(), Point2D::new(100.0, 0.0));
        app.add_node_and_sync("https://b.test/".into(), Point2D::new(-100.0, 0.0));
        // No selection.
        app.request_camera_command(crate::app::CameraCommand::FitSelection);

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        assert!(app.pending_camera_command().is_none());
        // Camera should have been moved (Fit path ran) — at least one of
        // pan / zoom should differ from identity.
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("view frame persisted");
        // Midpoint of (100, 0) and (-100, 0) is (0, 0), which is the
        // identity pan, so check zoom — both nodes are at y=0 so the
        // span is 200 px wide, <800 viewport, zoom should be capped by
        // the ~4.0 bound and well above identity.
        assert!(
            frame.zoom > 1.01,
            "Fit fallback on populated graph should zoom in (got {})",
            frame.zoom
        );
    }

    #[test]
    fn run_graph_canvas_frame_set_zoom_clamps_and_clears_pending() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.add_node_and_sync("https://a.test/".into(), Point2D::new(0.0, 0.0));
        app.request_camera_command(crate::app::CameraCommand::SetZoom(50.0));

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        assert!(app.pending_camera_command().is_none());
        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("view frame persisted");
        assert!(frame.zoom <= 10.0 + 1e-4, "zoom must clamp to max");
        assert!(frame.zoom > 1.0, "zoom should move toward the request");
    }

    // ── NavigationPolicy resolution ───────────────────────────────────

    #[test]
    fn resolve_navigation_policy_falls_back_to_graph_default() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        // Set a distinctive per-graph default.
        app.set_navigation_policy_default(graph_canvas::navigation::NavigationPolicy {
            zoom_max: 4.0,
            ..graph_canvas::navigation::NavigationPolicy::default()
        });
        let resolved = app.resolve_navigation_policy(view_id);
        assert_eq!(
            resolved.zoom_max, 4.0,
            "view with no override inherits graph default"
        );
    }

    #[test]
    fn resolve_navigation_policy_prefers_view_override_over_graph_default() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.set_navigation_policy_default(graph_canvas::navigation::NavigationPolicy {
            zoom_max: 4.0,
            ..graph_canvas::navigation::NavigationPolicy::default()
        });
        app.set_graph_view_navigation_policy_override(
            view_id,
            Some(graph_canvas::navigation::NavigationPolicy {
                zoom_max: 16.0,
                ..graph_canvas::navigation::NavigationPolicy::default()
            }),
        );
        let resolved = app.resolve_navigation_policy(view_id);
        assert_eq!(
            resolved.zoom_max, 16.0,
            "view override wins over graph default"
        );
    }

    #[test]
    fn resolve_node_style_falls_back_to_graph_default() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.set_node_style_default(graph_canvas::node_style::NodeStyle {
            default_radius: 24.0,
            ..graph_canvas::node_style::NodeStyle::default()
        });
        let resolved = app.resolve_node_style(view_id);
        assert_eq!(
            resolved.default_radius, 24.0,
            "view with no override inherits the per-graph default radius"
        );
    }

    #[test]
    fn resolve_node_style_prefers_view_override_over_graph_default() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.set_node_style_default(graph_canvas::node_style::NodeStyle {
            default_radius: 24.0,
            ..graph_canvas::node_style::NodeStyle::default()
        });
        app.set_graph_view_node_style_override(
            view_id,
            Some(graph_canvas::node_style::NodeStyle {
                default_radius: 12.0,
                ..graph_canvas::node_style::NodeStyle::default()
            }),
        );
        let resolved = app.resolve_node_style(view_id);
        assert_eq!(resolved.default_radius, 12.0);
    }

    #[test]
    fn resolve_simulate_motion_profile_falls_back_to_preset() {
        // Default resolver path: no override, no per-graph default →
        // profile matches `for_preset(view.simulate_behavior_preset)`.
        // Verifies that pre-existing preset pickers keep working when
        // nothing is tuned.
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        // View simulate_behavior_preset defaults to Float; profile
        // must match Float's canonical values.
        let resolved = app.resolve_simulate_motion_profile(view_id);
        let expected = graph_canvas::scene_physics::SimulateMotionProfile::for_preset(
            graph_canvas::scene_physics::SimulateBehaviorPreset::Float,
        );
        assert_eq!(resolved, expected);
        let _ = &mut app; // silence unused-mut lint if any
    }

    #[test]
    fn resolve_simulate_motion_profile_prefers_view_override() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        let custom = graph_canvas::scene_physics::SimulateMotionProfile {
            release_impulse_scale: 2.0,
            release_decay: 0.5,
            min_impulse: 0.1,
        };
        app.set_graph_view_simulate_motion_override(view_id, Some(custom));
        assert_eq!(app.resolve_simulate_motion_profile(view_id), custom);
    }

    #[test]
    fn resolve_simulate_motion_profile_falls_back_to_per_graph_default() {
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        let custom = graph_canvas::scene_physics::SimulateMotionProfile {
            release_impulse_scale: 0.9,
            release_decay: 0.7,
            min_impulse: 0.02,
        };
        app.set_simulate_motion_default(Some(custom));
        assert_eq!(app.resolve_simulate_motion_profile(view_id), custom);
        // Per-view override wins over the per-graph default.
        let override_profile = graph_canvas::scene_physics::SimulateMotionProfile {
            release_impulse_scale: 1.5,
            release_decay: 0.9,
            min_impulse: 0.01,
        };
        app.set_graph_view_simulate_motion_override(view_id, Some(override_profile));
        assert_eq!(
            app.resolve_simulate_motion_profile(view_id),
            override_profile
        );
    }

    #[test]
    fn run_graph_canvas_frame_applies_per_view_node_radius_to_scene() {
        // Per-view override of the default node radius must flow
        // through `build_scene_input` and show up on each
        // `CanvasNode.radius` inside the projected scene.
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.add_node_and_sync("https://a.test/".into(), Point2D::new(0.0, 0.0));
        app.set_graph_view_node_style_override(
            view_id,
            Some(graph_canvas::node_style::NodeStyle {
                default_radius: 40.0,
                ..graph_canvas::node_style::NodeStyle::default()
            }),
        );

        let output = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        // The scene carries hit proxies; each node hit-proxy carries
        // the radius used at scene-input time.
        let node_proxy_radius = output.scene.hit_proxies.iter().find_map(|p| match p {
            graph_canvas::packet::HitProxy::Node { radius, .. } => Some(*radius),
            _ => None,
        });
        assert_eq!(
            node_proxy_radius,
            Some(40.0),
            "scene must pick up the per-view node radius override"
        );
    }

    #[test]
    fn run_graph_canvas_frame_honors_per_view_zoom_clamp() {
        // Override the view's zoom_max to 2.0, then request a big
        // zoom via SetZoom(50.0). The clamp should land at 2.0 instead
        // of the hardcoded 10.0.
        let view_id = GraphViewId::new();
        let mut app = fit_test_app(view_id);
        app.add_node_and_sync("https://a.test/".into(), Point2D::new(0.0, 0.0));
        app.set_graph_view_navigation_policy_override(
            view_id,
            Some(graph_canvas::navigation::NavigationPolicy {
                zoom_max: 2.0,
                ..graph_canvas::navigation::NavigationPolicy::default()
            }),
        );
        app.request_camera_command(crate::app::CameraCommand::SetZoom(50.0));

        let _ = run_graph_canvas_frame(
            &mut app,
            view_id,
            &HashSet::new(),
            SearchDisplayMode::Highlight,
            false,
            fit_test_viewport(),
            &[],
        );

        let frame = app
            .workspace
            .graph_runtime
            .graph_view_frames
            .get(&view_id)
            .expect("view frame persisted");
        assert!(
            frame.zoom <= 2.0 + 1e-4,
            "zoom must clamp to the per-view zoom_max override (got {})",
            frame.zoom
        );
    }
}

// Node radius default now lives on the `NodeStyle` config — see
// `graph_canvas::node_style::NodeStyle` and `DEFAULT_NODE_RADIUS`.

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
        | CanvasAction::PanCamera(_)
        | CanvasAction::SetPanInertia(_) => Vec::new(),
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

/// Apply a `ZoomCamera` action to the canvas camera, clamped to the
/// navigation policy's zoom bounds.
///
/// Zooms toward the focus point so the world-space point under the cursor
/// stays visually fixed.
pub fn apply_zoom(
    camera: &mut CanvasCamera,
    factor: f32,
    focus: Point2D<f32>,
    viewport: &CanvasViewport,
    navigation_policy: &graph_canvas::navigation::NavigationPolicy,
) {
    let world_focus = camera.screen_to_world(focus, viewport);
    camera.zoom = navigation_policy.clamp_zoom(camera.zoom * factor);
    let new_screen = camera.world_to_screen(world_focus, viewport);
    let correction = focus - new_screen;
    camera.pan += Vector2D::new(correction.x / camera.zoom, correction.y / camera.zoom);
}

// ── Overlay-input builders ──────────────────────────────────────────────────

/// Build portable frame-affinity regions from the app's
/// `ArrangementRelation(FrameMember)` edges. Falls through the
/// existing [`crate::graph::frame_affinity::derive_frame_affinity_regions`]
/// helper so the rendered frames match the ones the physics pass uses.
fn build_portable_frame_regions(
    app: &GraphBrowserApp,
) -> Vec<graph_canvas::layout::extras::FrameRegion<NodeKey>> {
    crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph())
        .into_iter()
        .map(
            |region| graph_canvas::layout::extras::FrameRegion::<NodeKey> {
                anchor: region.frame_anchor,
                members: region.members,
                strength: region.strength,
            },
        )
        .collect()
}

/// Convert an app-side `SceneRegionRuntime` (egui types) into a
/// portable `graph_canvas::scene_region::SceneRegion` (euclid types).
/// Only visible regions for the given view are emitted.
fn build_portable_scene_regions(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
) -> Vec<graph_canvas::scene_region::SceneRegion> {
    use graph_canvas::scene_region::{
        SceneRegion as PortableSceneRegion, SceneRegionEffect as PortableEffect,
        SceneRegionId as PortableId, SceneRegionShape as PortableShape,
    };

    let Some(runtime) = app.graph_view_scene_runtime(view_id) else {
        return Vec::new();
    };

    runtime
        .regions
        .iter()
        .map(|region| {
            // Hash the app-side uuid into a stable u64 for the portable
            // id — the portable side uses u64, the app uses Uuid. The
            // lower 64 bits are sufficient for diagnostic round-trips
            // and never collide in practice (128-bit uuid collapsed to
            // 64 bits has ~2^32 before a birthday-paradox collision).
            let portable_id = PortableId(region.id.as_u64_low());

            let shape = match region.shape {
                crate::graph::scene_runtime::SceneRegionShape::Circle { center, radius } => {
                    PortableShape::Circle {
                        center: euclid::default::Point2D::new(center.x, center.y),
                        radius,
                    }
                }
                crate::graph::scene_runtime::SceneRegionShape::Rect { rect } => {
                    PortableShape::Rect {
                        rect: euclid::default::Rect::new(
                            euclid::default::Point2D::new(rect.min.x, rect.min.y),
                            euclid::default::Size2D::new(rect.width(), rect.height()),
                        ),
                    }
                }
            };

            let effect = match region.effect {
                crate::graph::scene_runtime::SceneRegionEffect::Attractor { strength } => {
                    PortableEffect::Attractor { strength }
                }
                crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength } => {
                    PortableEffect::Repulsor { strength }
                }
                crate::graph::scene_runtime::SceneRegionEffect::Dampener { factor } => {
                    PortableEffect::Dampener { factor }
                }
                crate::graph::scene_runtime::SceneRegionEffect::Wall => PortableEffect::Wall,
            };

            PortableSceneRegion {
                id: portable_id,
                label: region.label.clone(),
                shape,
                effect,
                visible: region.visible,
            }
        })
        .collect()
}

// ── Fit helpers ─────────────────────────────────────────────────────────────
//
// All tuning constants formerly hardcoded here (zoom clamp, fit padding,
// fallback zoom) now resolve through
// `GraphBrowserApp::resolve_navigation_policy(view_id)` — see the
// `graph_canvas::navigation::NavigationPolicy` type for the per-knob
// semantics and [the NavigationPolicy plan](../design_docs/archive_docs/checkpoint_2026-04-20/graphshell_docs/implementation_strategy/shell/2026-04-20_navigation_policy_plan.md)
// (archived 2026-04-20) for the bridging story across the egui and
// future iced hosts.

/// Compute the world-space bounds of a set of nodes in the scene,
/// expanded by each node's radius so nodes aren't clipped at the
/// viewport edge. Returns `None` when the set is empty (after filtering
/// out nodes missing from the scene).
fn bounds_of_nodes(
    scene: &CanvasSceneInput<NodeKey>,
    keys: impl IntoIterator<Item = NodeKey>,
) -> Option<euclid::default::Rect<f32>> {
    use std::collections::HashSet as HS;
    let key_set: HS<NodeKey> = keys.into_iter().collect();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut seen = false;
    for node in &scene.nodes {
        if !key_set.is_empty() && !key_set.contains(&node.id) {
            continue;
        }
        let radius = node.radius.max(0.0);
        min_x = min_x.min(node.position.x - radius);
        min_y = min_y.min(node.position.y - radius);
        max_x = max_x.max(node.position.x + radius);
        max_y = max_y.max(node.position.y + radius);
        seen = true;
    }
    if !seen {
        return None;
    }
    Some(euclid::default::Rect::new(
        Point2D::new(min_x, min_y),
        euclid::default::Size2D::new(max_x - min_x, max_y - min_y),
    ))
}

/// Apply a pending `CameraCommand` to the camera given the current scene,
/// app state, and the resolved navigation policy. Returns `true` when a
/// command was consumed so the caller can clear `pending_camera_command`.
/// `SetZoom` is handled here too for completeness, even though it doesn't
/// use Fit math.
///
/// Semantics per variant:
/// - `Fit` — bounds of every node in the current scene.
/// - `FitSelection` — bounds of the focused selection. Falls back to
///   `Fit` when the selection is empty.
/// - `FitGraphlet` — bounds of the view's `graphlet_node_mask`. Falls
///   back to `FitSelection` when no mask is active, then `Fit`.
/// - `SetZoom(factor)` — snap zoom to `factor` clamped into the same
///   range as drag-zoom; pan is preserved.
pub fn apply_fit_camera_command(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    scene: &CanvasSceneInput<NodeKey>,
    camera: &mut CanvasCamera,
    viewport: &CanvasViewport,
    command: crate::app::CameraCommand,
    navigation_policy: &graph_canvas::navigation::NavigationPolicy,
) -> bool {
    use crate::app::CameraCommand;

    match command {
        CameraCommand::SetZoom(factor) => {
            camera.zoom = navigation_policy.clamp_zoom(factor);
            camera.pan_velocity = Vector2D::zero();
            true
        }
        CameraCommand::Fit => fit_to_all_nodes(scene, camera, viewport, navigation_policy),
        CameraCommand::FitSelection => {
            let selection: Vec<NodeKey> = app.focused_selection().iter().copied().collect();
            if selection.is_empty() {
                return fit_to_all_nodes(scene, camera, viewport, navigation_policy);
            }
            if let Some(bounds) = bounds_of_nodes(scene, selection) {
                camera.fit_to_bounds(
                    bounds,
                    viewport,
                    navigation_policy.fit_padding_ratio,
                    navigation_policy.zoom_min,
                    navigation_policy.zoom_max,
                    navigation_policy.fit_fallback_zoom,
                )
            } else {
                fit_to_all_nodes(scene, camera, viewport, navigation_policy)
            }
        }
        CameraCommand::FitGraphlet => {
            let mask = app
                .workspace
                .graph_runtime
                .views
                .get(&view_id)
                .and_then(|view| view.graphlet_node_mask.as_ref())
                .cloned();
            if let Some(mask) = mask {
                if mask.is_empty() {
                    return fit_to_all_nodes(scene, camera, viewport, navigation_policy);
                }
                if let Some(bounds) = bounds_of_nodes(scene, mask.into_iter()) {
                    return camera.fit_to_bounds(
                        bounds,
                        viewport,
                        navigation_policy.fit_padding_ratio,
                        navigation_policy.zoom_min,
                        navigation_policy.zoom_max,
                        navigation_policy.fit_fallback_zoom,
                    );
                }
            }
            // No mask / empty mask / no overlap with scene → fall back.
            apply_fit_camera_command(
                app,
                view_id,
                scene,
                camera,
                viewport,
                CameraCommand::FitSelection,
                navigation_policy,
            )
        }
    }
}

fn fit_to_all_nodes(
    scene: &CanvasSceneInput<NodeKey>,
    camera: &mut CanvasCamera,
    viewport: &CanvasViewport,
    navigation_policy: &graph_canvas::navigation::NavigationPolicy,
) -> bool {
    let Some(bounds) = bounds_of_nodes(scene, Vec::<NodeKey>::new()) else {
        // Empty scene: nothing to fit. Leave camera untouched; caller
        // still clears the pending command (no point retrying when
        // there's nothing to show).
        return true;
    };
    camera.fit_to_bounds(
        bounds,
        viewport,
        navigation_policy.fit_padding_ratio,
        navigation_policy.zoom_min,
        navigation_policy.zoom_max,
        navigation_policy.fit_fallback_zoom,
    )
}

// ── Camera sync ─────────────────────────────────────────────────────────────

/// Construct a `CanvasCamera` from the app's `GraphViewFrame`.
pub fn camera_from_view_frame(frame: crate::app::GraphViewFrame) -> CanvasCamera {
    CanvasCamera {
        pan: Vector2D::new(frame.pan_x, frame.pan_y),
        zoom: frame.zoom.max(0.01),
        pan_velocity: Vector2D::zero(),
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
        apply_zoom(
            &mut camera,
            1.5,
            focus,
            &viewport,
            &graph_canvas::navigation::NavigationPolicy::default(),
        );
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
